//! The Catalog: the mods folder scanned, every package classified, the tree
//! resolved, the search index built — plus the cache that makes the second
//! launch instant (unchanged packages are not reopened).
//!
//! ## A scan
//!
//! 1. The mods folder is listed ([`scan::list_library`]): every archive and
//!    every folder directly inside it, by name (case-blind). Nothing is held
//!    back for not looking like a mod: an archive that holds none is a package
//!    with nothing to install.
//! 2. Every candidate is stamped — size and modification time, the cache key
//!    (a folder is walked for both; see [`crate::scan`]). When the cache holds
//!    the same path with the same stamp, the package comes from the cache
//!    unopened.
//! 3. The rest are opened and classified in parallel on a rayon pool of at
//!    most [`MAX_SCAN_THREADS`] threads (the work mostly waits on the disk, and
//!    every solid-7z decoder holds its dictionary in memory). Each package
//!    reports [`ScanProgress`] from the calling thread the moment it lands.
//! 4. Packages keep the listing's order, and every package id is unique. Two
//!    names that differ only in case (a case-sensitive folder, such as one a
//!    Linux bench made) would share an id; the first by name keeps it, and the
//!    other is left out.
//! 5. [`tree::resolve`] runs over all of them, then the [`SearchIndex`] is
//!    built — without Nexus labels: those come from the spine's update
//!    checks, through [`Catalog::set_labels`], which never rebuilds it.
//!
//! A package that cannot be opened still appears — no options, one
//! `Unreadable` problem — and is never cached: a file another program held
//! locked reads fine next time with the very same stamp. A reader that panics
//! on a damaged archive makes that package unreadable instead of ending the
//! scan.
//!
//! [`Catalog::from_cache`] applies the same rules without the disk, so the
//! startup preview shows what the scan will: the cached packages that sit
//! directly in the mods folder, in the same order.

use std::collections::{HashMap, HashSet};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use rift_core::{lower_key, ArchiveEntry, ModOption, OptionId, Package, PackageId, RiftError};

use crate::scan::{self, ScannedPackage, Stamp, StampError};
use crate::search::{LabelFacets, PackageLabels, SearchHit, SearchIndex, SearchQuery};
use crate::tree;

/// The most threads one scan opens packages on.
pub const MAX_SCAN_THREADS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub done: usize,
    pub total: usize,
    /// File name of the package just finished.
    pub current: String,
}

/// One package as remembered between launches.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedPackage {
    pub path: PathBuf,
    pub size: u64,
    pub modified_ms: u64,
    pub package: Package,
    pub entries: Vec<ArchiveEntry>,
}

/// Persisted by the window crate as JSON in the app data dir. Fields an older
/// version wrote and this one no longer knows are ignored on reading; the
/// schema decides whether the rest is trusted.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogCache {
    pub schema: u32,
    pub packages: Vec<CachedPackage>,
}

impl CatalogCache {
    /// Bump when the Package shape or classification rules change, so stale
    /// caches are rebuilt instead of trusted.
    ///
    /// 2 — nested variants under payload-less modinfo roots, the DD2 Pak
    /// Bridge sentence on `OutdatedFormats`, `ModinfoGarbled` problems, and
    /// folder stamps that walk the folder (size = sum of files, time = newest).
    ///
    /// 3 — the `OutdatedFormats` sentence for a single file reads "as it is"
    /// (cached packages carried "as they are").
    ///
    /// 4 — a very old Nexus file name (no upload time) links only when a
    /// `homepage=` line of the package names the same mod (cached packages
    /// carried the name's link unvouched).
    ///
    /// 5 — one mods folder (v0.3): a package no longer names the place it was
    /// found in, a folder package is known by its name
    /// ([`PackageId::for_folder`]), and a folder package holding archives of
    /// its own says so. The first scan after an older cache reopens every
    /// package once.
    pub const SCHEMA: u32 = 5;
}

/// What the last scan did, for logs and the bench. Every candidate lands in
/// exactly one of `opened`, `reused`, `unreadable` and `gone`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStats {
    /// Candidates the mods folder listed.
    pub candidates: usize,
    /// Packages opened and classified.
    pub opened: usize,
    /// Packages taken from the cache unopened.
    pub reused: usize,
    /// Candidates that could not be opened. Each appears with an
    /// `Unreadable` problem.
    pub unreadable: usize,
    /// Candidates that vanished between the listing and the scan.
    pub gone: usize,
}

#[derive(Debug, Clone, Default)]
pub struct Catalog {
    packages: Vec<Package>,
    entries: HashMap<PackageId, Vec<ArchiveEntry>>,
    index: SearchIndex,
    stats: ScanStats,
}

impl Catalog {
    /// Scan the mods folder. Packages whose (path, size, mtime) match the
    /// cache are taken from it unopened; a cache of another schema — or
    /// `CatalogCache::default()`, when there is none — answers nothing.
    /// Unreadable packages still appear, carrying an `Unreadable` problem.
    /// Returns the catalog and the cache to persist.
    pub fn scan(
        library: &Path,
        cache: &CatalogCache,
        progress: &mut dyn FnMut(ScanProgress),
    ) -> (Catalog, CatalogCache) {
        let paths = scan::list_library(library);
        let memory = Memory::of(cache);
        let total = paths.len();

        let mut outcomes: Vec<Option<Outcome>> =
            std::iter::repeat_with(|| None).take(total).collect();
        let mut done = 0;
        run_parallel(
            &paths,
            &|path: &PathBuf| examine(path, &memory),
            &mut |slot, outcome| {
                done += 1;
                progress(ScanProgress {
                    done,
                    total,
                    current: display_file_name(&paths[slot]),
                });
                outcomes[slot] = Some(outcome);
            },
        );

        assemble(&paths, outcomes)
    }

    /// The catalog a cache describes, without touching the disk — what the
    /// window shows at startup while the first real scan runs. Only cached
    /// packages whose path sits directly in `library` are kept (a cache
    /// written beside another mods folder shows nothing), in the scan's
    /// order; a cache of another schema gives an empty catalog.
    pub fn from_cache(library: &Path, cache: &CatalogCache) -> Catalog {
        let mut catalog = Catalog::default();
        if cache.schema != CatalogCache::SCHEMA {
            return catalog;
        }
        let mut picked: Vec<(String, &CachedPackage)> = cache
            .packages
            .iter()
            .filter(|cached| cached.path.parent() == Some(library))
            .map(|cached| (lower_key(&cached.package.file_name), cached))
            .collect();
        picked.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.path.cmp(&b.1.path)));

        let mut seen = HashSet::new();
        for (_, cached) in picked {
            if seen.insert(cached.package.id.clone()) {
                catalog
                    .entries
                    .insert(cached.package.id.clone(), cached.entries.clone());
                catalog.packages.push(cached.package.clone());
            }
        }
        tree::resolve(&mut catalog.packages);
        catalog.index = SearchIndex::build(&catalog.packages, &catalog.entries);
        catalog.stats = ScanStats {
            candidates: catalog.packages.len(),
            reused: catalog.packages.len(),
            ..ScanStats::default()
        };
        catalog
    }

    /// Every package, in library order (by name, case-blind).
    pub fn packages(&self) -> &[Package] {
        &self.packages
    }

    pub fn package(&self, id: &PackageId) -> Option<&Package> {
        self.packages.iter().find(|p| &p.id == id)
    }

    pub fn option(&self, id: &OptionId) -> Option<(&Package, &ModOption)> {
        let package = self.package(&id.package())?;
        let option = package.options.iter().find(|o| &o.id == id)?;
        Some((package, option))
    }

    pub fn entries(&self, id: &PackageId) -> &[ArchiveEntry] {
        self.entries.get(id).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Run a query ([`SearchIndex::search`]): `limit` 0 returns every match.
    pub fn search(
        &self,
        query: &SearchQuery,
        is_installed: &dyn Fn(&OptionId) -> bool,
    ) -> Vec<SearchHit> {
        self.index.search(query, is_installed)
    }

    /// Give each package the labels of its Nexus mod (v0.2), replacing the
    /// last call's; the index is not rebuilt ([`SearchIndex::set_labels`]). A
    /// fresh catalog — [`Catalog::scan`] or [`Catalog::from_cache`] — has
    /// none until this is called.
    pub fn set_labels(&mut self, labels: HashMap<PackageId, PackageLabels>) {
        self.index.set_labels(labels);
    }

    /// A package's labels as the last [`Catalog::set_labels`] gave them.
    pub fn labels(&self, id: &PackageId) -> Option<&PackageLabels> {
        self.index.labels(id)
    }

    /// The categories and tags with their installable-option counts, and the
    /// packages a label filter hides ([`SearchIndex::label_facets`]).
    pub fn label_facets(&self) -> LabelFacets {
        self.index.label_facets()
    }

    /// What the scan that built this catalog did.
    pub fn stats(&self) -> ScanStats {
        self.stats
    }
}

// ---------------------------------------------------------------------------
// The scan's moving parts
// ---------------------------------------------------------------------------

/// What became of one candidate.
enum Outcome {
    /// A package, scanned now or taken from the cache (boxed: a package is
    /// far bigger than the other outcomes).
    Ready {
        stamp: Stamp,
        package: Box<Package>,
        entries: Vec<ArchiveEntry>,
        from_cache: bool,
    },
    /// Deleted or moved since the mods folder was listed.
    Gone,
    Failed {
        stamp: Option<Stamp>,
        error: RiftError,
    },
}

/// The previous scan's packages, by path, when the cache is of this schema.
#[derive(Default)]
struct Memory<'a> {
    packages: HashMap<&'a Path, &'a CachedPackage>,
}

impl<'a> Memory<'a> {
    fn of(cache: &'a CatalogCache) -> Self {
        if cache.schema != CatalogCache::SCHEMA {
            return Memory::default();
        }
        Memory {
            packages: cache
                .packages
                .iter()
                .map(|cached| (cached.path.as_path(), cached))
                .collect(),
        }
    }

    fn package(&self, path: &Path, stamp: Stamp) -> Option<&'a CachedPackage> {
        self.packages
            .get(path)
            .copied()
            .filter(|cached| cached.size == stamp.size && cached.modified_ms == stamp.modified_ms)
    }
}

/// Decide one candidate: the cache's answer when the stamp matches, else a
/// fresh scan. Runs on the scan pool.
fn examine(path: &Path, memory: &Memory<'_>) -> Outcome {
    let stamp = match scan::stamp(path) {
        Ok(stamp) => stamp,
        Err(StampError::Gone(_)) => return Outcome::Gone,
        Err(StampError::Unreadable(error)) => {
            return Outcome::Failed { stamp: None, error };
        }
    };

    if let Some(cached) = memory.package(path, stamp) {
        return Outcome::Ready {
            stamp,
            package: Box::new(cached.package.clone()),
            entries: cached.entries.clone(),
            from_cache: true,
        };
    }
    match panic::catch_unwind(AssertUnwindSafe(|| scan::scan_stamped(path, stamp))) {
        Ok(Ok(ScannedPackage { package, entries })) => Outcome::Ready {
            stamp,
            package: Box::new(package),
            entries,
            from_cache: false,
        },
        Ok(Err(error)) => Outcome::Failed {
            stamp: Some(stamp),
            error,
        },
        Err(_) => Outcome::Failed {
            stamp: Some(stamp),
            error: RiftError::corrupt(path, "the reader stopped unexpectedly on this archive"),
        },
    }
}

/// Run `work` over `jobs` on a bounded rayon pool and hand each result to
/// `land` on THIS thread, as it finishes (slot = the job's index).
fn run_parallel<J, O>(jobs: &[J], work: &(dyn Fn(&J) -> O + Sync), land: &mut dyn FnMut(usize, O))
where
    J: Sync,
    O: Send,
{
    if jobs.is_empty() {
        return;
    }
    let threads = std::thread::available_parallelism()
        .map_or(4, std::num::NonZeroUsize::get)
        .clamp(1, MAX_SCAN_THREADS)
        .min(jobs.len());
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|i| format!("rift-scan-{i}"))
        .build();
    let (sender, receiver) = mpsc::channel::<(usize, O)>();
    std::thread::scope(|scope| {
        scope.spawn(move || {
            let all = move || {
                jobs.par_iter()
                    .enumerate()
                    .for_each_with(sender, |sender, (slot, job)| {
                        // The receiver only goes away if the caller's progress
                        // callback panicked; the scan is over then anyway.
                        let _ = sender.send((slot, work(job)));
                    });
            };
            match &pool {
                Ok(pool) => pool.install(all),
                Err(err) => {
                    log::warn!("no scan pool ({err}); scanning on rayon's global pool");
                    all();
                }
            }
        });
        for (slot, outcome) in receiver {
            land(slot, outcome);
        }
    });
}

/// Line the outcomes up in the listing's order, keep one package per id,
/// resolve the tree, build the index and the cache to persist.
fn assemble(paths: &[PathBuf], outcomes: Vec<Option<Outcome>>) -> (Catalog, CatalogCache) {
    let mut stats = ScanStats {
        candidates: paths.len(),
        ..ScanStats::default()
    };
    let mut cache = CatalogCache {
        schema: CatalogCache::SCHEMA,
        ..CatalogCache::default()
    };

    let mut packages: Vec<Package> = Vec::with_capacity(paths.len());
    // Per package: where to remember it from (unreadable ones are never cached).
    let mut cache_keys: Vec<Option<(&Path, Stamp)>> = Vec::with_capacity(paths.len());
    let mut entries: HashMap<PackageId, Vec<ArchiveEntry>> = HashMap::new();
    let mut ids: HashSet<PackageId> = HashSet::with_capacity(paths.len());
    for (path, outcome) in paths.iter().zip(outcomes) {
        let Some(outcome) = outcome else {
            continue; // every job lands; this is only the type's way out
        };
        let (package, listing, stamp) = match outcome {
            Outcome::Ready {
                stamp,
                package,
                entries: listing,
                from_cache,
            } => {
                if from_cache {
                    stats.reused += 1;
                } else {
                    stats.opened += 1;
                }
                (*package, listing, Some(stamp))
            }
            Outcome::Gone => {
                stats.gone += 1;
                continue;
            }
            Outcome::Failed { stamp, error } => {
                stats.unreadable += 1;
                (
                    scan::unreadable_package(path, stamp, &error),
                    Vec::new(),
                    None,
                )
            }
        };
        if !ids.insert(package.id.clone()) {
            log::warn!(
                "{}: its name differs from an earlier package's only in case, so the two share \
                 one id; it is left out",
                path.display()
            );
            continue;
        }
        if stamp.is_some() {
            entries.insert(package.id.clone(), listing);
        }
        cache_keys.push(stamp.map(|stamp| (path.as_path(), stamp)));
        packages.push(package);
    }

    tree::resolve(&mut packages);

    for (package, key) in packages.iter().zip(&cache_keys) {
        if let Some((path, stamp)) = key {
            let listing = entries.get(&package.id).cloned().unwrap_or_default();
            cache
                .packages
                .push(cached(path, *stamp, package.clone(), listing));
        }
    }

    let index = SearchIndex::build(&packages, &entries);
    let catalog = Catalog {
        packages,
        entries,
        index,
        stats,
    };
    (catalog, cache)
}

fn cached(
    path: &Path,
    stamp: Stamp,
    package: Package,
    entries: Vec<ArchiveEntry>,
) -> CachedPackage {
    CachedPackage {
        path: path.to_path_buf(),
        size: stamp.size,
        modified_ms: stamp.modified_ms,
        package,
        entries,
    }
}

/// A package's file name as the scan names it (its progress's `current`):
/// the last component of its path, else the whole path.
pub fn display_file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.to_string_lossy().into_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rift_core::{PackageFormat, ProblemKind};
    use tempfile::TempDir;

    use super::*;
    use crate::search::{LabelCount, SearchFilters, SearchQuery};
    use crate::test_support::{folder_at, modinfo, touch_later, wreck_keeping_stamp, zip_at};

    const MESH: &[u8] = b"mesh bytes";
    const ERMINE_TRIM: &str = "Ermine Trim 2207 8 2026-10-14T14-23Z Rk4vLmN2Q.zip";

    /// One mods folder: a cloak collection (a DummyMod menu and a variant
    /// carrying top_314), a folder pak mod, an add-on that hangs under the
    /// collection's menu, and holiday photos that are no mod at all.
    struct Library {
        dir: TempDir,
        collection: PathBuf,
        photos: PathBuf,
        trim: PathBuf,
    }

    impl Library {
        fn path(&self) -> &Path {
            self.dir.path()
        }
    }

    fn library() -> Library {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path();

        let collection = mods.join("Vernworth Cloaks Collection.zip");
        zip_at(
            &collection,
            &[
                (
                    "Collection/Cloaks/modinfo.ini",
                    &modinfo(&[("name", "Cloaks"), ("DummyMod", "true")]),
                ),
                (
                    "Collection/Cloaks/Ermine/modinfo.ini",
                    &modinfo(&[("name", "Ermine Cape"), ("addonfor", "Cloaks")]),
                ),
                (
                    "Collection/Cloaks/Ermine/natives/stm/appsystem/topswb/mesh/top_314_wb_f.mesh.260421070",
                    MESH,
                ),
            ],
        );
        folder_at(
            &mods.join("Lantern Glow Recolor"),
            &[
                ("modinfo.ini", &modinfo(&[("name", "Lantern Glow Recolor")])),
                ("Lantern Glow Recolor.pak", b"KPKA"),
            ],
        );
        let photos = mods.join("Holiday Photos.zip");
        zip_at(&photos, &[("beach.jpg", b"jpg")]);
        let trim = mods.join(ERMINE_TRIM);
        zip_at(
            &trim,
            &[
                (
                    "Ermine Trim/modinfo.ini",
                    &modinfo(&[("name", "Ermine Trim"), ("addonfor", "Cloaks")]),
                ),
                ("Ermine Trim/natives/stm/trim.mesh.260421070", MESH),
            ],
        );

        Library {
            dir,
            collection,
            photos,
            trim,
        }
    }

    /// The library's names, in the order the scan lines them up.
    const IN_ORDER: [&str; 4] = [
        ERMINE_TRIM,
        "Holiday Photos.zip",
        "Lantern Glow Recolor",
        "Vernworth Cloaks Collection.zip",
    ];

    fn none() -> CatalogCache {
        CatalogCache::default()
    }

    fn quiet() -> impl FnMut(ScanProgress) {
        |_| {}
    }

    fn file_names(catalog: &Catalog) -> Vec<&str> {
        catalog
            .packages()
            .iter()
            .map(|p| p.file_name.as_str())
            .collect()
    }

    /// The one option carrying `name` (case-blind, trimmed).
    fn option_named<'a>(catalog: &'a Catalog, name: &str) -> &'a ModOption {
        let wanted = lower_key(name.trim());
        let found: Vec<&ModOption> = catalog
            .packages()
            .iter()
            .flat_map(|package| &package.options)
            .filter(|option| lower_key(option.name.trim()) == wanted)
            .collect();
        assert_eq!(found.len(), 1, "{name}");
        found[0]
    }

    fn search(catalog: &Catalog, text: &str) -> Vec<SearchHit> {
        let query = SearchQuery {
            text: text.to_string(),
            ..SearchQuery::default()
        };
        catalog.search(&query, &|_| false)
    }

    #[test]
    fn a_scan_lists_the_mods_folder_by_name_and_resolves_across_packages() {
        let library = library();
        let caller = std::thread::current().id();
        let mut events = Vec::new();
        let (catalog, cache) = Catalog::scan(library.path(), &none(), &mut |event| {
            assert_eq!(
                std::thread::current().id(),
                caller,
                "progress comes from the caller's thread"
            );
            events.push(event);
        });

        assert_eq!(file_names(&catalog), IN_ORDER, "by name, case-blind");
        let folder = &catalog.packages()[2];
        assert_eq!(folder.format, PackageFormat::Folder);
        assert_eq!(folder.id, PackageId::for_folder("Lantern Glow Recolor"));
        assert_eq!(catalog.packages()[0].display_name, "Ermine Trim");

        let photos = &catalog.packages()[1];
        assert!(photos.options.is_empty());
        assert_eq!(
            photos.problems.iter().map(|p| p.kind).collect::<Vec<_>>(),
            [ProblemKind::NothingToInstall],
            "nothing is held back for not looking like a mod"
        );

        assert_eq!(events.len(), 4, "one event per candidate");
        assert!(events.iter().all(|e| e.total == 4));
        assert_eq!(
            events.iter().map(|e| e.done).collect::<Vec<_>>(),
            [1, 2, 3, 4]
        );
        let mut current: Vec<&str> = events.iter().map(|e| e.current.as_str()).collect();
        current.sort_unstable();
        assert_eq!(
            current,
            [
                ERMINE_TRIM,
                "Holiday Photos.zip",
                "Lantern Glow Recolor",
                "Vernworth Cloaks Collection.zip"
            ]
        );

        let menu = option_named(&catalog, " CLOAKS ");
        let trim = option_named(&catalog, "Ermine Trim");
        assert_eq!(
            trim.parent.as_ref(),
            Some(&menu.id),
            "a parent in another package"
        );
        assert_eq!(trim.trail, ["Ermine Trim", "Cloaks"]);
        assert_eq!(
            option_named(&catalog, "Ermine Cape").parent.as_ref(),
            Some(&menu.id)
        );

        let hits = search(&catalog, "top_314");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].option_id, option_named(&catalog, "Ermine Cape").id);
        let (package, option) = catalog.option(&hits[0].option_id).unwrap();
        assert_eq!(package.file_name, "Vernworth Cloaks Collection.zip");
        assert_eq!(option.name, "Ermine Cape");
        assert_eq!(catalog.entries(&package.id).len(), 3);
        assert!(catalog.package(&package.id).is_some());

        assert_eq!(cache.schema, CatalogCache::SCHEMA);
        assert_eq!(cache.packages.len(), 4, "every readable package");
        assert!(cache.packages.iter().any(|c| c.path == library.photos));
        assert_eq!(
            catalog.stats(),
            ScanStats {
                candidates: 4,
                opened: 4,
                ..ScanStats::default()
            }
        );
    }

    #[test]
    fn cached_packages_are_not_reopened() {
        let library = library();
        let (first, cache) = Catalog::scan(library.path(), &none(), &mut quiet());

        // Same size, same time, unreadable bytes: only a reopen could notice.
        for path in [&library.collection, &library.trim] {
            wreck_keeping_stamp(path);
        }
        let (second, cache_again) = Catalog::scan(library.path(), &cache, &mut quiet());
        assert_eq!(
            second.stats(),
            ScanStats {
                candidates: 4,
                reused: 4,
                ..ScanStats::default()
            },
            "nothing was opened: every package came from the cache"
        );
        assert_eq!(second.packages(), first.packages());
        assert_eq!(search(&second, "top_314").len(), 1);
        assert_eq!(cache_again.packages.len(), 4);
        assert_eq!(
            Catalog::from_cache(library.path(), &cache).packages(),
            second.packages(),
            "the startup preview shows what the scan will"
        );

        // The proof that the bytes really are wrecked: without the cache both fail.
        let (cold, cold_cache) = Catalog::scan(library.path(), &none(), &mut quiet());
        assert_eq!(cold.stats().unreadable, 2);
        assert_eq!(
            cold_cache.packages.len(),
            2,
            "only the untouched folder and the photos are cached"
        );
        let collection = cold
            .packages()
            .iter()
            .find(|p| p.path == library.collection)
            .unwrap();
        assert!(collection.options.is_empty());
        assert_eq!(collection.problems[0].kind, ProblemKind::Unreadable);

        // A newer time is a changed package: reopened (and now unreadable).
        touch_later(&library.collection);
        let (touched, _) = Catalog::scan(library.path(), &cache, &mut quiet());
        assert_eq!((touched.stats().reused, touched.stats().unreadable), (3, 1));
    }

    #[test]
    fn a_package_that_cannot_be_opened_still_appears_and_is_never_cached() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("Broken Cape.zip"),
            b"PK\x03\x04 and then nothing",
        )
        .unwrap();
        let (catalog, cache) = Catalog::scan(dir.path(), &none(), &mut quiet());
        let broken = &catalog.packages()[0];
        assert_eq!(broken.display_name, "Broken Cape");
        assert_eq!(broken.format, PackageFormat::Zip);
        assert!(broken.options.is_empty());
        assert_eq!(broken.entry_count, 0);
        assert_eq!(broken.problems.len(), 1);
        assert_eq!(broken.problems[0].kind, ProblemKind::Unreadable);
        assert!(catalog.entries(&broken.id).is_empty());
        assert!(
            cache.packages.is_empty(),
            "a locked file may read fine next time"
        );
        assert_eq!(catalog.stats().unreadable, 1);

        // Every later scan tries it again, and shows it again.
        let (again, _) = Catalog::scan(dir.path(), &cache, &mut quiet());
        assert_eq!(again.packages(), catalog.packages());
        assert_eq!((again.stats().unreadable, again.stats().reused), (1, 0));
    }

    #[test]
    fn a_cached_package_outside_the_mods_folder_is_never_shown() {
        let library = library();
        let (scanned, cache) = Catalog::scan(library.path(), &none(), &mut quiet());

        // The stone moved: its mods folder is somewhere else now.
        let elsewhere = tempfile::tempdir().unwrap();
        assert!(Catalog::from_cache(elsewhere.path(), &cache)
            .packages()
            .is_empty());

        // A cached package one level too deep (a folder package's own
        // archive, say) is no package of the mods folder.
        let mut deeper = cache.clone();
        let trim = deeper
            .packages
            .iter_mut()
            .find(|c| c.path == library.trim)
            .unwrap();
        trim.path = library
            .path()
            .join("Lantern Glow Recolor")
            .join(ERMINE_TRIM);
        trim.package.path.clone_from(&trim.path);
        let preview = Catalog::from_cache(library.path(), &deeper);
        assert_eq!(
            file_names(&preview),
            [
                "Holiday Photos.zip",
                "Lantern Glow Recolor",
                "Vernworth Cloaks Collection.zip"
            ]
        );
        assert!(
            preview
                .packages()
                .iter()
                .all(|p| p.path.parent() == Some(library.path())),
            "only what sits directly in the mods folder"
        );

        // The whole folder copied to the new place: nothing in the old cache
        // answers for it (its paths are new), yet every id is the same,
        // folders included: they are known by their names.
        for name in IN_ORDER {
            let from = library.path().join(name);
            if from.is_dir() {
                folder_at(
                    &elsewhere.path().join(name),
                    &[
                        ("modinfo.ini", &modinfo(&[("name", "Lantern Glow Recolor")])),
                        ("Lantern Glow Recolor.pak", b"KPKA"),
                    ],
                );
            } else {
                fs::copy(&from, elsewhere.path().join(name)).unwrap();
            }
        }
        let (moved, moved_cache) = Catalog::scan(elsewhere.path(), &cache, &mut quiet());
        assert_eq!(
            (moved.stats().reused, moved.stats().opened),
            (0, 4),
            "the old cache answers nothing for the new place"
        );
        let ids = |catalog: &Catalog| -> Vec<PackageId> {
            catalog.packages().iter().map(|p| p.id.clone()).collect()
        };
        assert_eq!(ids(&moved), ids(&scanned));
        assert!(moved_cache
            .packages
            .iter()
            .all(|c| c.path.parent() == Some(elsewhere.path())));
    }

    #[test]
    fn two_names_that_differ_only_in_case_share_an_id_and_show_once() {
        let dir = tempfile::tempdir().unwrap();
        let files: &[(&str, &[u8])] = &[("Lantern Glow/Lantern Glow.pak", b"KPKA")];
        zip_at(&dir.path().join("Lantern Glow.zip"), files);
        zip_at(&dir.path().join("lantern glow.zip"), files);
        folder_at(&dir.path().join("Pawn Banter Quieter"), files);
        folder_at(&dir.path().join("PAWN BANTER QUIETER"), files);
        if scan::list_library(dir.path()).len() < 4 {
            return; // a case-blind file system: the names are one file each
        }

        let (catalog, cache) = Catalog::scan(dir.path(), &none(), &mut quiet());
        assert_eq!(
            file_names(&catalog),
            ["Lantern Glow.zip", "PAWN BANTER QUIETER"],
            "the first by name keeps the id"
        );
        let ids: HashSet<&PackageId> = catalog.packages().iter().map(|p| &p.id).collect();
        assert_eq!(ids.len(), 2);
        assert_eq!((catalog.stats().candidates, catalog.stats().opened), (4, 4));
        assert_eq!(cache.packages.len(), 2);
        assert_eq!(
            Catalog::from_cache(dir.path(), &cache).packages(),
            catalog.packages()
        );
    }

    #[test]
    fn a_cache_of_another_schema_is_not_trusted() {
        let library = library();
        let (_, mut cache) = Catalog::scan(library.path(), &none(), &mut quiet());
        cache.schema = CatalogCache::SCHEMA - 1;
        let (catalog, fresh) = Catalog::scan(library.path(), &cache, &mut quiet());
        assert_eq!((catalog.stats().reused, catalog.stats().opened), (0, 4));
        assert_eq!(fresh.schema, CatalogCache::SCHEMA);
        assert!(Catalog::from_cache(library.path(), &cache)
            .packages()
            .is_empty());

        // A v0.2 cache on disk (schema 4, each package naming its rootId, and
        // a passedOver list): it still reads, and nothing in it is trusted.
        let mut json = serde_json::to_value(&fresh).unwrap();
        json["schema"] = 4.into();
        json["passedOver"] = serde_json::json!([{ "path": "/x", "size": 1, "modifiedMs": 1 }]);
        for cached in json["packages"].as_array_mut().unwrap() {
            cached["package"]["rootId"] = "mods".into();
        }
        let older: CatalogCache = serde_json::from_value(json).unwrap();
        assert_eq!((older.schema, older.packages.len()), (4, 4));
        let (reopened, _) = Catalog::scan(library.path(), &older, &mut quiet());
        assert_eq!(
            (reopened.stats().reused, reopened.stats().opened),
            (0, 4),
            "the first v0.3 scan reopens every package once"
        );
        assert!(Catalog::from_cache(library.path(), &older)
            .packages()
            .is_empty());
    }

    #[test]
    fn the_cache_round_trips_through_json_and_rebuilds_the_catalog_without_the_disk() {
        let library = library();
        let (scanned, cache) = Catalog::scan(library.path(), &none(), &mut quiet());
        let json = serde_json::to_string(&cache).unwrap();
        let cache: CatalogCache = serde_json::from_str(&json).unwrap();
        let mods = library.path().to_path_buf();
        drop(library); // every file is gone now

        let rebuilt = Catalog::from_cache(&mods, &cache);
        assert_eq!(rebuilt.packages(), scanned.packages());
        assert_eq!(search(&rebuilt, "top_314").len(), 1);
        assert_eq!(
            rebuilt.stats(),
            ScanStats {
                candidates: 4,
                reused: 4,
                ..ScanStats::default()
            }
        );

        // Without the collection, the trim's menu is gone from the folder.
        let mut without = cache.clone();
        without
            .packages
            .retain(|c| c.package.file_name != "Vernworth Cloaks Collection.zip");
        let rebuilt = Catalog::from_cache(&mods, &without);
        let trim = rebuilt
            .packages()
            .iter()
            .find(|p| p.file_name == ERMINE_TRIM)
            .unwrap();
        assert_eq!(trim.problems[0].kind, ProblemKind::MissingParent);
        assert!(
            trim.problems[0]
                .message
                .contains("Add the mod that provides \"Cloaks\" to the mods folder"),
            "{}",
            trim.problems[0].message
        );

        // The persisted shape: camelCase, the schema beside the packages.
        let json: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(json["schema"], CatalogCache::SCHEMA);
        assert!(json["packages"][0]["modifiedMs"].is_u64());
        assert!(json["packages"][0]["package"]["displayName"].is_string());
        let mut keys: Vec<&String> = json.as_object().unwrap().keys().collect();
        keys.sort_unstable();
        assert_eq!(keys, ["packages", "schema"]);
        assert!(json["packages"][0]["package"].get("rootId").is_none());
    }

    #[test]
    fn a_candidate_that_vanished_is_simply_gone() {
        let dir = tempfile::tempdir().unwrap();
        let cache = none();
        let memory = Memory::of(&cache);
        assert!(matches!(
            examine(&dir.path().join("Lantern Glow.zip"), &memory),
            Outcome::Gone
        ));
        let (catalog, cache) = Catalog::scan(dir.path(), &none(), &mut quiet());
        assert!(catalog.packages().is_empty() && cache.packages.is_empty());
        let (catalog, _) = Catalog::scan(&dir.path().join("no such folder"), &none(), &mut quiet());
        assert!(catalog.packages().is_empty());
        assert_eq!(catalog.stats(), ScanStats::default());
    }

    #[test]
    fn nexus_labels_reach_the_catalog_search_without_a_rescan() {
        let library = library();
        let (mut catalog, _) = Catalog::scan(library.path(), &none(), &mut quiet());
        let cape = option_named(&catalog, "Ermine Cape").clone();
        let trim = option_named(&catalog, "Ermine Trim").package_id.clone();
        let armour = |tags: &[&str]| PackageLabels {
            category: Some("Armour".to_string()),
            tags: tags.iter().map(|tag| (*tag).to_string()).collect(),
            adult: false,
        };
        catalog.set_labels(HashMap::from([
            (cape.package_id.clone(), armour(&["Lore Friendly"])),
            (trim.clone(), armour(&[])),
        ]));

        let query = SearchQuery {
            filters: SearchFilters {
                tags: vec!["lore friendly".to_string()],
                ..SearchFilters::default()
            },
            ..SearchQuery::default()
        };
        let hits = catalog.search(&query, &|_| false);
        assert_eq!(
            hits.iter().map(|hit| &hit.option_id).collect::<Vec<_>>(),
            [&cape.id],
            "the collection's menu entry stays hidden; the trim lacks the tag"
        );
        assert_eq!(hits[0].labels, Some(armour(&["Lore Friendly"])));
        assert_eq!(catalog.labels(&trim), Some(&armour(&[])));
        assert_eq!(
            catalog.label_facets(),
            LabelFacets {
                categories: vec![LabelCount {
                    name: "Armour".to_string(),
                    options: 2
                }],
                tags: vec![LabelCount {
                    name: "Lore Friendly".to_string(),
                    options: 1
                }],
                unlabeled_packages: 1,
            },
            "the folder pak mod is not linked; the photos install nothing"
        );

        let (rescanned, _) = Catalog::scan(library.path(), &none(), &mut quiet());
        assert_eq!(
            rescanned.labels(&cape.package_id),
            None,
            "a fresh catalog has none until the spine sets them again"
        );
    }

    #[test]
    fn run_parallel_lands_every_job_once() {
        let jobs: Vec<u32> = (0..100).collect();
        let mut landed = vec![0u32; 100];
        run_parallel(&jobs, &|job: &u32| job * 2, &mut |slot, doubled| {
            assert_eq!(doubled, jobs[slot] * 2);
            landed[slot] += 1;
        });
        assert!(landed.iter().all(|&count| count == 1));
    }
}
