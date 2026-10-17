//! The library as the spine holds it between scans.
//!
//! - [`Shelf`] — what search, `get_option` and `get_package` read: the
//!   [`Catalog`] the last scan built, or, before the first scan of a launch has
//!   finished, the same library rebuilt from the catalog cache without opening
//!   a single archive (`Catalog::from_cache` — the window shows it at once,
//!   startup step 4). Its index carries the Nexus labels (v0.2), set apart
//!   from the rest: a check relabels the shelf without rebuilding it.
//! - [`SourcePool`] — a few packages kept open between reads (thumbnails,
//!   pak heads), so an archive's listing and headers are read once, not per
//!   card. Of a solid 7z or a RAR, rift-archive keeps the small same-named
//!   entries a pass goes by (every `screenshot.png` of 256 KiB or less), so a
//!   grid of small screenshots decompresses once; a larger entry decompresses
//!   its block again on every read. Idle packages are closed by a timer.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, TryLockError};
use std::time::{Duration, Instant};

use rift_catalog::{Catalog, CatalogCache, LabelFacets, PackageLabels, SearchHit, SearchQuery};
use rift_core::{
    ArchiveEntry, ModOption, OptionId, Package, PackageFormat, PackageId, PackageSource, RiftError,
};

/// What the window searches and browses: one catalog, swapped whole — or,
/// for new Nexus labels alone, relabeled in place ([`Shelf::set_labels`]).
#[derive(Default, Clone)]
pub struct Shelf {
    catalog: Catalog,
    /// The labels last handed to the catalog, as given.
    labels: HashMap<PackageId, PackageLabels>,
}

impl Shelf {
    /// Nothing yet.
    pub fn empty() -> Shelf {
        Shelf::default()
    }

    /// A catalog a scan just built (no labels yet: [`Shelf::with_labels`]).
    pub fn scanned(catalog: Catalog) -> Shelf {
        Shelf {
            catalog,
            labels: HashMap::new(),
        }
    }

    /// The mods folder as the last scan left it, without touching the disk
    /// (no labels yet: [`Shelf::with_labels`]). A cache written beside
    /// another mods folder shows nothing.
    pub fn from_cache(library: &Path, cache: &CatalogCache) -> Shelf {
        Shelf::scanned(Catalog::from_cache(library, cache))
    }

    /// This shelf with these labels ([`Shelf::set_labels`]).
    pub fn with_labels(mut self, labels: HashMap<PackageId, PackageLabels>) -> Shelf {
        self.set_labels(labels);
        self
    }

    /// Give each package the labels of its Nexus mod (v0.2), replacing the
    /// last ones; a package left out has none. Only the index's label layer is
    /// rebuilt (under a millisecond at 10k options).
    pub fn set_labels(&mut self, labels: HashMap<PackageId, PackageLabels>) {
        if self.labels == labels {
            return;
        }
        self.catalog.set_labels(labels.clone());
        self.labels = labels;
    }

    /// Are these the labels the shelf carries already?
    pub fn carries_labels(&self, labels: &HashMap<PackageId, PackageLabels>) -> bool {
        &self.labels == labels
    }

    /// A package's labels as the index holds them (tidied): `None` when it
    /// has none (unlinked, or no check has heard of its mod).
    pub fn labels(&self, id: &PackageId) -> Option<&PackageLabels> {
        self.catalog.labels(id)
    }

    /// Every category and tag with its installable-option count, and how
    /// many packages a label filter hides.
    pub fn label_facets(&self) -> LabelFacets {
        self.catalog.label_facets()
    }

    pub fn packages(&self) -> &[Package] {
        self.catalog.packages()
    }

    pub fn package(&self, id: &PackageId) -> Option<&Package> {
        self.catalog.package(id)
    }

    pub fn option(&self, id: &OptionId) -> Option<(&Package, &ModOption)> {
        self.catalog.option(id)
    }

    pub fn entries(&self, id: &PackageId) -> &[ArchiveEntry] {
        self.catalog.entries(id)
    }

    pub fn search(
        &self,
        query: &SearchQuery,
        is_installed: &dyn Fn(&OptionId) -> bool,
    ) -> Vec<SearchHit> {
        self.catalog.search(query, is_installed)
    }

    /// Every option in library order.
    pub fn options(&self) -> impl Iterator<Item = (&Package, &ModOption)> {
        self.packages()
            .iter()
            .flat_map(|package| package.options.iter().map(move |option| (package, option)))
    }

    pub fn option_count(&self) -> usize {
        self.packages().iter().map(|p| p.options.len()).sum()
    }

    pub fn entry_count(&self) -> usize {
        self.packages().iter().map(|p| p.entry_count).sum()
    }
}

// ---------------------------------------------------------------------------
// Open packages
// ---------------------------------------------------------------------------

/// How many packages stay open at once.
const POOL_SIZE: usize = 3;
/// How long an unused package stays open (an open file cannot be deleted on
/// Windows; the player's own clean-up must not wait on the Riftstone).
const POOL_IDLE: Duration = Duration::from_secs(60);
/// How often the pool closes what has sat unused for [`POOL_IDLE`].
const POOL_SWEEP: Duration = Duration::from_secs(10);

type SharedSource = Arc<Mutex<Box<dyn PackageSource>>>;

struct Pooled {
    package: PackageId,
    path: PathBuf,
    source: SharedSource,
    used: Instant,
}

struct Pool {
    open: Mutex<Vec<Pooled>>,
    idle: Duration,
}

impl Pool {
    fn open(&self) -> std::sync::MutexGuard<'_, Vec<Pooled>> {
        self.open.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn close_idle(&self, now: Instant) {
        let idle = self.idle;
        self.open()
            .retain(|p| now.saturating_duration_since(p.used) < idle);
    }
}

/// Can one entry be read without decoding the ones before it?
fn random_access(format: PackageFormat) -> bool {
    matches!(format, PackageFormat::Zip | PackageFormat::Folder)
}

/// A few packages kept open between reads (thumbnails, pak heads, listings).
///
/// No read waits on another for a zip or a folder: when the pooled handle is
/// busy, the read opens a second one. A 7z or a RAR decodes from the start of
/// a block (a RAR from the start of the archive), so its reads take turns on
/// one handle instead — a second decoder would hold a second dictionary in
/// memory to repeat the same work.
pub struct SourcePool {
    pool: Arc<Pool>,
    sweep: Duration,
    sweeping: AtomicBool,
}

impl Default for SourcePool {
    fn default() -> Self {
        SourcePool::new(POOL_IDLE, POOL_SWEEP)
    }
}

impl SourcePool {
    /// A pool closing packages unused for `idle`, looking every `sweep`.
    pub fn new(idle: Duration, sweep: Duration) -> SourcePool {
        SourcePool {
            pool: Arc::new(Pool {
                open: Mutex::new(Vec::new()),
                idle,
            }),
            sweep,
            sweeping: AtomicBool::new(false),
        }
    }

    /// Run `read` against `package`, opened once and reused while it is used.
    pub fn with<R>(
        &self,
        package: &Package,
        read: impl FnOnce(&mut dyn PackageSource) -> Result<R, RiftError>,
    ) -> Result<R, RiftError> {
        let source = self.checkout(package)?;
        if random_access(package.format) {
            match source.try_lock() {
                Ok(mut guard) => return read(guard.as_mut()),
                Err(TryLockError::Poisoned(poisoned)) => {
                    return read(poisoned.into_inner().as_mut())
                }
                Err(TryLockError::WouldBlock) => {
                    let mut second = rift_archive::open(&package.path)?;
                    return read(second.as_mut());
                }
            }
        }
        let mut guard = source.lock().unwrap_or_else(PoisonError::into_inner);
        read(guard.as_mut())
    }

    fn checkout(&self, package: &Package) -> Result<SharedSource, RiftError> {
        self.start_sweeping();
        let now = Instant::now();
        let same = |p: &Pooled| p.package == package.id && p.path == package.path;
        {
            let mut open = self.pool.open();
            if let Some(pooled) = open.iter_mut().find(|p| same(p)) {
                pooled.used = now;
                return Ok(Arc::clone(&pooled.source));
            }
        }
        // Opened outside the lock: a slow open must not stall other readers.
        let source: SharedSource = Arc::new(Mutex::new(rift_archive::open(&package.path)?));
        let mut open = self.pool.open();
        if let Some(pooled) = open.iter_mut().find(|p| same(p)) {
            // Another read opened it meanwhile: share that one.
            pooled.used = now;
            return Ok(Arc::clone(&pooled.source));
        }
        if open.len() >= POOL_SIZE {
            if let Some(oldest) = open
                .iter()
                .enumerate()
                .min_by_key(|(_, p)| p.used)
                .map(|(i, _)| i)
            {
                open.remove(oldest);
            }
        }
        open.push(Pooled {
            package: package.id.clone(),
            path: package.path.clone(),
            source: Arc::clone(&source),
            used: now,
        });
        Ok(source)
    }

    /// The sweeper: a thread that closes idle packages, started with the
    /// first package opened and gone soon after the pool is.
    fn start_sweeping(&self) {
        if self.sweeping.swap(true, Ordering::SeqCst) {
            return;
        }
        let pool = Arc::downgrade(&self.pool);
        let sweep = self.sweep;
        let spawned = std::thread::Builder::new()
            .name("riftstone-pool".into())
            .spawn(move || loop {
                std::thread::sleep(sweep);
                let Some(pool) = pool.upgrade() else {
                    return;
                };
                pool.close_idle(Instant::now());
            });
        if let Err(err) = spawned {
            log::warn!("idle packages stay open until the next read: {err}");
            self.sweeping.store(false, Ordering::SeqCst);
        }
    }

    /// How many packages are open now.
    pub fn open_count(&self) -> usize {
        self.pool.open().len()
    }

    /// Close everything (a rescan may have replaced the files).
    pub fn clear(&self) {
        self.pool.open().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rift_catalog::catalog::CachedPackage;
    use rift_core::{DeployTarget, ModInfo, PackageFormat, PayloadFile, PayloadKinds};

    fn option(package: &PackageId, root: &str, name: &str, menu_only: bool) -> ModOption {
        ModOption {
            id: OptionId::new(package, root),
            package_id: package.clone(),
            root: root.into(),
            name: name.into(),
            info: Some(ModInfo {
                name: Some(name.into()),
                ..ModInfo::default()
            }),
            screenshot: None,
            // `tree::resolve` calls an option without payload a menu entry.
            payload: if menu_only {
                Vec::new()
            } else {
                vec![PayloadFile {
                    entry: format!("{root}/natives/stm/{name}.mesh"),
                    target: DeployTarget::Loose {
                        game_path: format!("natives/stm/{name}.mesh"),
                    },
                    size: 1,
                }]
            },
            skipped: Vec::new(),
            kinds: PayloadKinds::default(),
            parent: None,
            addon_for: None,
            bundle: None,
            menu_only,
            trail: Vec::new(),
        }
    }

    fn package(dir: &str, file: &str, modified_ms: u64, options: &[(&str, &str, bool)]) -> Package {
        let id = PackageId::for_archive(file, modified_ms);
        Package {
            id: id.clone(),
            path: PathBuf::from(format!("/{dir}/{file}")),
            file_name: file.into(),
            display_name: file.trim_end_matches(".zip").into(),
            format: PackageFormat::Zip,
            size: modified_ms,
            modified_ms,
            entry_count: options.len(),
            options: options
                .iter()
                .map(|(root, name, menu)| option(&id, root, name, *menu))
                .collect(),
            nexus: None,
            problems: Vec::new(),
        }
    }

    // Built field by field: the cache's shape is rift-catalog's to grow.
    #[allow(clippy::field_reassign_with_default)]
    fn cache(packages: Vec<Package>) -> CatalogCache {
        let mut cache = CatalogCache::default();
        cache.schema = CatalogCache::SCHEMA;
        cache.packages = packages
            .into_iter()
            .map(|package| CachedPackage {
                path: package.path.clone(),
                size: package.size,
                modified_ms: package.modified_ms,
                entries: Vec::new(),
                package,
            })
            .collect();
        cache
    }

    #[test]
    fn a_cached_shelf_holds_the_mods_folder_only_in_name_order() {
        let cache = cache(vec![
            package("mods", "Zeta.zip", 1, &[("Zeta", "Zeta", false)]),
            package(
                "mods",
                "Vernworth Cloaks.zip",
                2,
                &[("Cloaks", "Ermine Cape", false)],
            ),
            package(
                "mods",
                "Lantern Glow.zip",
                3,
                &[("Glow", "Lantern Glow Recolor", false)],
            ),
            // Cached beside another mods folder: not this library.
            package("elsewhere", "Old.zip", 4, &[("Old", "Old", false)]),
        ]);
        let shelf = Shelf::from_cache(Path::new("/mods"), &cache);
        let names: Vec<&str> = shelf
            .packages()
            .iter()
            .map(|p| p.file_name.as_str())
            .collect();
        assert_eq!(
            names,
            vec!["Lantern Glow.zip", "Vernworth Cloaks.zip", "Zeta.zip"]
        );
        assert_eq!(shelf.option_count(), 3);
        let hits = shelf.search(
            &SearchQuery {
                text: "ermine".into(),
                ..SearchQuery::default()
            },
            &|_| false,
        );
        assert_eq!(hits.len(), 1);
        let (_, found) = shelf.option(&hits[0].option_id).unwrap();
        assert_eq!(found.name, "Ermine Cape");
        assert!(Shelf::empty().packages().is_empty());
    }

    /// A folder package on disk with one file.
    fn folder_package(dir: &std::path::Path) -> Package {
        let root = dir.join("Lantern Glow Recolor");
        std::fs::create_dir_all(root.join("natives/stm")).unwrap();
        std::fs::write(root.join("natives/stm/glow.tex.251211553"), b"glow").unwrap();
        Package {
            id: PackageId::for_folder("Lantern Glow Recolor"),
            path: root,
            file_name: "Lantern Glow Recolor".into(),
            display_name: "Lantern Glow Recolor".into(),
            format: PackageFormat::Folder,
            size: 4,
            modified_ms: 1,
            entry_count: 1,
            options: Vec::new(),
            nexus: None,
            problems: Vec::new(),
        }
    }

    #[test]
    fn a_busy_zip_or_folder_never_makes_another_read_wait() {
        let dir = tempfile::tempdir().unwrap();
        let package = folder_package(dir.path());
        let pool = Arc::new(SourcePool::default());
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let long_read = {
            let pool = Arc::clone(&pool);
            let package = package.clone();
            std::thread::spawn(move || {
                pool.with(&package, |source| {
                    started_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    source.read_entry("natives/stm/glow.tex.251211553", None)
                })
            })
        };
        started_rx.recv().unwrap();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        {
            let pool = Arc::clone(&pool);
            let package = package.clone();
            std::thread::spawn(move || {
                let read = pool.with(&package, |source| {
                    source.read_entry("natives/stm/glow.tex.251211553", None)
                });
                done_tx.send(read).unwrap();
            });
        }
        let second = done_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("the second read did not wait for the first");
        release_tx.send(()).unwrap();
        assert_eq!(second.unwrap(), b"glow");
        assert_eq!(long_read.join().unwrap().unwrap(), b"glow");
        assert_eq!(pool.open_count(), 1, "the second handle was not pooled");
    }

    #[test]
    fn idle_packages_close_without_another_read() {
        let dir = tempfile::tempdir().unwrap();
        let package = folder_package(dir.path());
        let pool = SourcePool::new(Duration::from_millis(60), Duration::from_millis(20));
        pool.with(&package, |source| source.entries()).unwrap();
        assert_eq!(pool.open_count(), 1);
        let deadline = Instant::now() + Duration::from_secs(10);
        while pool.open_count() > 0 {
            assert!(
                Instant::now() < deadline,
                "the idle package was never closed"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
