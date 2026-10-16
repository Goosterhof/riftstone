//! The Catalog: every root scanned, every package classified, the tree
//! resolved, the search index built — plus the cache that makes the second
//! launch instant (unchanged packages are not reopened).
//!
//! ## A scan
//!
//! 1. Every root is listed ([`scan::list_root`]), in root order.
//! 2. Every candidate is stamped — size and modification time, the cache key
//!    (a folder is walked for both; see [`crate::scan`]). When the cache holds
//!    the same path with the same stamp, the package comes from the cache
//!    unopened (in a Downloads-style root only while it still looks like a
//!    Dragon's Dogma 2 mod). Candidates of such a root that are not DD2 mods
//!    are remembered by their stamp alone ([`PassedOverCandidate`]) and passed
//!    over unopened while it holds.
//! 3. The rest are opened and classified in parallel on a rayon pool of at
//!    most [`MAX_SCAN_THREADS`] threads (the work mostly waits on the disk, and
//!    every solid-7z decoder holds its dictionary in memory). Each package
//!    reports [`ScanProgress`] from the calling thread the moment it lands.
//! 4. Packages line up in root order, then name. The same archive in two roots
//!    (same file name and size — the same package id) is kept once: at the
//!    first place that could read it, else at its first place. A broken copy
//!    (an interrupted copy keeps the full length) never hides a readable one.
//! 5. [`tree::resolve`] runs over all of them, then the [`SearchIndex`] is
//!    built — without Nexus labels: those come from the spine's update
//!    checks, through [`Catalog::set_labels`], which never rebuilds it.
//!
//! A package that cannot be opened still appears — no options, one
//! `Unreadable` problem — and is never cached: a file another program held
//! locked reads fine next time with the very same stamp. A reader that panics
//! on a damaged archive makes that package unreadable instead of ending the
//! scan. The one exception is a Downloads-style root: nothing says an archive
//! there that cannot be opened is a Dragon's Dogma 2 mod at all, and that
//! folder holds the player's own private files too (a password-protected
//! zip of tax papers is no mod), so it is passed over unshown and tried again
//! next scan (main-session ruling, 2026-10-16).
//!
//! [`Catalog::from_cache`] applies the same rules without the disk, so the
//! startup preview shows what the scan will.

use std::collections::{HashMap, HashSet};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use rift_core::{
    lower_key, ArchiveEntry, LibraryRoot, ModOption, OptionId, Package, PackageId, RiftError,
};

use crate::classify;
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

/// A candidate of a `dd2_shaped_only` root that is no Dragon's Dogma 2 mod,
/// remembered by its stamp alone: the next scan passes it over without
/// opening it again, for as long as its size and time stay the same.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PassedOverCandidate {
    pub path: PathBuf,
    pub size: u64,
    pub modified_ms: u64,
}

/// Persisted by the window crate as JSON in the app data dir.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogCache {
    pub schema: u32,
    pub packages: Vec<CachedPackage>,
    /// What `dd2_shaped_only` roots hold besides DD2 mods (absent from caches
    /// written before it existed: those candidates are simply opened once).
    #[serde(default)]
    pub passed_over: Vec<PassedOverCandidate>,
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
    pub const SCHEMA: u32 = 4;
}

/// What the last scan did, for logs and the bench.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStats {
    /// Candidates the roots listed.
    pub candidates: usize,
    /// Packages opened and classified.
    pub opened: usize,
    /// Packages taken from the cache unopened.
    pub reused: usize,
    /// Candidates of `dd2_shaped_only` roots that are not DD2 mods (never
    /// classified; remembered by stamp only).
    pub passed_over: usize,
    /// Of `passed_over`: candidates the cache already knew with the same
    /// stamp, passed over without being opened again.
    pub passed_over_from_cache: usize,
    /// Candidates that could not be opened. Each appears with an
    /// `Unreadable` problem, unless a readable copy of the same archive sits
    /// elsewhere (it is then one of the `duplicates`) or it lies in a
    /// `dd2_shaped_only` root (`unreadable_passed_over`).
    pub unreadable: usize,
    /// Of `unreadable`: candidates of `dd2_shaped_only` roots, passed over
    /// unshown — nothing says they are mods — and tried again next scan.
    pub unreadable_passed_over: usize,
    /// Copies of a package kept at another place: a second readable copy (the
    /// first place wins), or a broken copy beside a readable one (the readable
    /// one wins).
    pub duplicates: usize,
    /// Candidates that vanished between the listing and the scan.
    pub gone: usize,
}

#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub roots: Vec<LibraryRoot>,
    packages: Vec<Package>,
    entries: HashMap<PackageId, Vec<ArchiveEntry>>,
    index: SearchIndex,
    stats: ScanStats,
}

impl Catalog {
    /// Scan every root. Packages whose (path, size, mtime) match the cache are
    /// taken from it unopened. Unreadable packages still appear, carrying an
    /// `Unreadable` problem. Returns the catalog and the cache to persist.
    pub fn scan(
        roots: &[LibraryRoot],
        cache: Option<&CatalogCache>,
        progress: &mut dyn FnMut(ScanProgress),
    ) -> (Catalog, CatalogCache) {
        let jobs: Vec<Job> = roots
            .iter()
            .enumerate()
            .flat_map(|(root_index, root)| {
                scan::list_root(root)
                    .into_iter()
                    .map(move |path| Job { root_index, path })
            })
            .collect();
        let memory = Memory::of(cache);
        let total = jobs.len();

        let mut outcomes: Vec<Option<Outcome>> =
            std::iter::repeat_with(|| None).take(total).collect();
        let mut done = 0;
        run_parallel(
            &jobs,
            &|job: &Job| examine(&roots[job.root_index], &job.path, &memory),
            &mut |slot, outcome| {
                done += 1;
                progress(ScanProgress {
                    done,
                    total,
                    current: display_file_name(&jobs[slot].path),
                });
                outcomes[slot] = Some(outcome);
            },
        );

        assemble(roots, &jobs, outcomes)
    }

    /// The catalog a cache describes, without touching the disk — what the
    /// window shows at startup while the first real scan runs. Packages of
    /// roots no longer configured are left out, and so are packages a
    /// `dd2_shaped_only` root would pass over (a root switched to that
    /// setting since the cache was written); a cache of another schema gives
    /// an empty catalog.
    pub fn from_cache(roots: &[LibraryRoot], cache: &CatalogCache) -> Catalog {
        let mut catalog = Catalog {
            roots: roots.to_vec(),
            ..Catalog::default()
        };
        if cache.schema != CatalogCache::SCHEMA {
            return catalog;
        }
        let mut picked: Vec<(usize, String, &CachedPackage)> = cache
            .packages
            .iter()
            .filter_map(|cached| {
                let (index, root) = roots
                    .iter()
                    .enumerate()
                    .find(|(_, root)| root.id == cached.package.root_id)?;
                if root.dd2_shaped_only && !classify::is_dd2_shaped(&cached.entries) {
                    return None;
                }
                Some((index, lower_key(&cached.package.file_name), cached))
            })
            .collect();
        picked.sort_by(|a, b| (a.0, &a.1, &a.2.path).cmp(&(b.0, &b.1, &b.2.path)));

        let mut seen = HashSet::new();
        for (_, _, cached) in picked {
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

    /// Options whose display name equals `name` (case-insensitive, trimmed) —
    /// how Fluffy's installed.ini sections and `requires=` lines find options.
    /// Library order (root, then package name, then the package's own option
    /// order).
    pub fn options_named(&self, name: &str) -> Vec<&ModOption> {
        let wanted = lower_key(name.trim());
        if wanted.is_empty() {
            return Vec::new();
        }
        self.packages
            .iter()
            .flat_map(|package| &package.options)
            .filter(|option| lower_key(option.name.trim()) == wanted)
            .collect()
    }

    /// What the scan that built this catalog did.
    pub fn stats(&self) -> ScanStats {
        self.stats
    }
}

// ---------------------------------------------------------------------------
// The scan's moving parts
// ---------------------------------------------------------------------------

struct Job {
    root_index: usize,
    path: PathBuf,
}

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
    /// Not a DD2 mod, in a root that lists only DD2 mods.
    PassedOver { stamp: Stamp, from_cache: bool },
    /// Deleted or moved since the root was listed.
    Gone,
    Failed {
        stamp: Option<Stamp>,
        error: RiftError,
    },
}

/// The previous scan's packages and passed-over candidates, when the cache
/// is of this schema.
#[derive(Default)]
struct Memory<'a> {
    packages: HashMap<&'a Path, &'a CachedPackage>,
    passed_over: HashMap<&'a Path, &'a PassedOverCandidate>,
}

impl<'a> Memory<'a> {
    fn of(cache: Option<&'a CatalogCache>) -> Self {
        let Some(cache) = cache.filter(|cache| cache.schema == CatalogCache::SCHEMA) else {
            return Memory::default();
        };
        Memory {
            packages: cache
                .packages
                .iter()
                .map(|cached| (cached.path.as_path(), cached))
                .collect(),
            passed_over: cache
                .passed_over
                .iter()
                .map(|candidate| (candidate.path.as_path(), candidate))
                .collect(),
        }
    }

    fn package(&self, path: &Path, stamp: Stamp) -> Option<&'a CachedPackage> {
        self.packages
            .get(path)
            .copied()
            .filter(|cached| cached.size == stamp.size && cached.modified_ms == stamp.modified_ms)
    }

    /// Was this candidate passed over before, with this very stamp?
    fn passed_over(&self, path: &Path, stamp: Stamp) -> bool {
        self.passed_over.get(path).is_some_and(|candidate| {
            candidate.size == stamp.size && candidate.modified_ms == stamp.modified_ms
        })
    }
}

/// Decide one candidate: the cache's answer when the stamp matches, else a
/// fresh scan. Runs on the scan pool.
fn examine(root: &LibraryRoot, path: &Path, memory: &Memory<'_>) -> Outcome {
    let stamp = match scan::stamp(path) {
        Ok(stamp) => stamp,
        Err(StampError::Gone(_)) => return Outcome::Gone,
        Err(StampError::Unreadable(error)) => {
            return Outcome::Failed { stamp: None, error };
        }
    };

    if let Some(cached) = memory.package(path, stamp) {
        if root.dd2_shaped_only && !classify::is_dd2_shaped(&cached.entries) {
            return Outcome::PassedOver {
                stamp,
                from_cache: true,
            };
        }
        let mut package = Box::new(cached.package.clone());
        package.root_id.clone_from(&root.id);
        return Outcome::Ready {
            stamp,
            package,
            entries: cached.entries.clone(),
            from_cache: true,
        };
    }
    if root.dd2_shaped_only && memory.passed_over(path, stamp) {
        return Outcome::PassedOver {
            stamp,
            from_cache: true,
        };
    }
    match panic::catch_unwind(AssertUnwindSafe(|| scan::scan_stamped(root, path, stamp))) {
        Ok(Ok(Some(ScannedPackage { package, entries }))) => Outcome::Ready {
            stamp,
            package: Box::new(package),
            entries,
            from_cache: false,
        },
        Ok(Ok(None)) => Outcome::PassedOver {
            stamp,
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

/// A package the scan found, readable or not, before copies are settled.
struct Found {
    package: Package,
    listing: Vec<ArchiveEntry>,
    /// Where to remember it from — `None` for a package that could not be
    /// read, which is never cached.
    cache_key: Option<(PathBuf, Stamp)>,
}

impl Found {
    fn readable(&self) -> bool {
        self.cache_key.is_some()
    }
}

/// Line the outcomes up in root order, then name; keep one copy of each
/// package; resolve the tree; build the index and the cache to persist.
fn assemble(
    roots: &[LibraryRoot],
    jobs: &[Job],
    outcomes: Vec<Option<Outcome>>,
) -> (Catalog, CatalogCache) {
    let mut stats = ScanStats {
        candidates: jobs.len(),
        ..ScanStats::default()
    };
    let mut cache = CatalogCache {
        schema: CatalogCache::SCHEMA,
        ..CatalogCache::default()
    };

    let mut found: Vec<Found> = Vec::with_capacity(jobs.len());
    for (job, outcome) in jobs.iter().zip(outcomes) {
        let Some(outcome) = outcome else {
            continue; // every job lands; this is only the type's way out
        };
        match outcome {
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
                found.push(Found {
                    package: *package,
                    listing,
                    cache_key: Some((job.path.clone(), stamp)),
                });
            }
            Outcome::PassedOver { stamp, from_cache } => {
                stats.passed_over += 1;
                stats.passed_over_from_cache += usize::from(from_cache);
                cache.passed_over.push(PassedOverCandidate {
                    path: job.path.clone(),
                    size: stamp.size,
                    modified_ms: stamp.modified_ms,
                });
            }
            Outcome::Gone => stats.gone += 1,
            Outcome::Failed { stamp, error } => {
                stats.unreadable += 1;
                if roots[job.root_index].dd2_shaped_only {
                    stats.unreadable_passed_over += 1;
                    log::debug!(
                        "{}: passed over, it could not be opened ({error})",
                        job.path.display()
                    );
                    continue;
                }
                found.push(Found {
                    package: scan::unreadable_package(
                        &roots[job.root_index],
                        &job.path,
                        stamp,
                        &error,
                    ),
                    listing: Vec::new(),
                    cache_key: None,
                });
            }
        }
    }

    // One place per package id: the first copy that could be read, else the
    // first copy at all. A broken copy in an earlier root (an interrupted
    // copy keeps the full length, so the same id) never hides a readable one.
    let mut keeper: HashMap<&PackageId, usize> = HashMap::with_capacity(found.len());
    for (at, copy) in found.iter().enumerate() {
        let kept = keeper.entry(&copy.package.id).or_insert(at);
        if !found[*kept].readable() && copy.readable() {
            *kept = at;
        }
    }
    let kept: Vec<bool> = found
        .iter()
        .enumerate()
        .map(|(at, copy)| keeper.get(&copy.package.id) == Some(&at))
        .collect();

    let mut packages: Vec<Package> = Vec::with_capacity(found.len());
    // Per package: where to remember it from (unreadable ones are never cached).
    let mut cache_keys: Vec<Option<(PathBuf, Stamp)>> = Vec::with_capacity(found.len());
    let mut entries: HashMap<PackageId, Vec<ArchiveEntry>> = HashMap::new();
    let mut duplicates: Vec<CachedPackage> = Vec::new();
    for (copy, kept) in found.into_iter().zip(kept) {
        let Found {
            package,
            listing,
            cache_key,
        } = copy;
        if !kept {
            stats.duplicates += 1;
            // A readable copy is remembered by its own path, so it is not
            // reopened next time either.
            if let Some((path, stamp)) = cache_key {
                duplicates.push(cached(&path, stamp, package, listing));
            }
            continue;
        }
        if cache_key.is_some() {
            entries.insert(package.id.clone(), listing);
        }
        cache_keys.push(cache_key);
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
    cache.packages.extend(duplicates);

    let index = SearchIndex::build(&packages, &entries);
    let catalog = Catalog {
        roots: roots.to_vec(),
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

fn display_file_name(path: &Path) -> String {
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
    use crate::test_support::{folder_at, modinfo, root, touch_later, wreck_keeping_stamp, zip_at};

    const MESH: &[u8] = b"mesh bytes";
    const ERMINE_TRIM: &str = "Ermine Trim 2207 8 2026-10-14T14-23Z Rk4vLmN2Q.zip";

    /// Two roots. `fluffy`: a cloak collection (a DummyMod menu and a variant
    /// carrying top_314) and a folder pak mod. `downloads` (DD2-shaped only):
    /// holiday photos and an add-on that hangs under the collection's menu.
    struct Library {
        _dir: TempDir,
        collection: PathBuf,
        photos: PathBuf,
        trim: PathBuf,
        roots: Vec<LibraryRoot>,
    }

    fn library() -> Library {
        let dir = tempfile::tempdir().unwrap();
        let fluffy = dir.path().join("Mods");
        let downloads = dir.path().join("Downloads");
        fs::create_dir_all(&fluffy).unwrap();
        fs::create_dir_all(&downloads).unwrap();

        let collection = fluffy.join("Vernworth Cloaks Collection.zip");
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
            &fluffy.join("Lantern Glow Recolor"),
            &[
                ("modinfo.ini", &modinfo(&[("name", "Lantern Glow Recolor")])),
                ("Lantern Glow Recolor.pak", b"KPKA"),
            ],
        );

        let photos = downloads.join("Holiday Photos.zip");
        zip_at(&photos, &[("beach.jpg", b"jpg")]);
        let trim = downloads.join(ERMINE_TRIM);
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
            roots: vec![
                root("fluffy", &fluffy, false),
                root("downloads", &downloads, true),
            ],
            _dir: dir,
            collection,
            photos,
            trim,
        }
    }

    fn quiet() -> impl FnMut(ScanProgress) {
        |_| {}
    }

    fn option_named<'a>(catalog: &'a Catalog, name: &str) -> &'a ModOption {
        let found = catalog.options_named(name);
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
    fn a_scan_lines_up_roots_then_names_and_resolves_across_packages() {
        let library = library();
        let caller = std::thread::current().id();
        let mut events = Vec::new();
        let (catalog, cache) = Catalog::scan(&library.roots, None, &mut |event| {
            assert_eq!(
                std::thread::current().id(),
                caller,
                "progress comes from the caller's thread"
            );
            events.push(event);
        });

        let files: Vec<&str> = catalog
            .packages()
            .iter()
            .map(|p| p.file_name.as_str())
            .collect();
        assert_eq!(
            files,
            [
                "Lantern Glow Recolor",
                "Vernworth Cloaks Collection.zip",
                ERMINE_TRIM
            ],
            "root order, then name; the photos are passed over"
        );
        assert_eq!(catalog.roots, library.roots);
        assert_eq!(catalog.packages()[0].format, PackageFormat::Folder);
        assert_eq!(catalog.packages()[2].display_name, "Ermine Trim");
        assert_eq!(catalog.packages()[2].root_id, "downloads");

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
        assert!(catalog.options_named("").is_empty());
        assert!(catalog.options_named("Nobody").is_empty());

        let hits = search(&catalog, "top_314");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].option_id, option_named(&catalog, "Ermine Cape").id);
        let (package, option) = catalog.option(&hits[0].option_id).unwrap();
        assert_eq!(package.file_name, "Vernworth Cloaks Collection.zip");
        assert_eq!(option.name, "Ermine Cape");
        assert_eq!(catalog.entries(&package.id).len(), 3);
        assert!(catalog.package(&package.id).is_some());

        assert_eq!(cache.schema, CatalogCache::SCHEMA);
        assert_eq!(cache.packages.len(), 3, "the photos are no package");
        assert!(cache
            .packages
            .iter()
            .all(|cached| cached.path != library.photos));
        let photos_size = fs::metadata(&library.photos).unwrap().len();
        assert_eq!(
            cache.passed_over,
            [PassedOverCandidate {
                path: library.photos.clone(),
                size: photos_size,
                modified_ms: scan::stamp(&library.photos).unwrap().modified_ms,
            }],
            "the photos are remembered by their stamp alone"
        );
        assert_eq!(
            catalog.stats(),
            ScanStats {
                candidates: 4,
                opened: 3,
                passed_over: 1,
                ..ScanStats::default()
            }
        );
    }

    #[test]
    fn cached_packages_are_not_reopened() {
        let library = library();
        let (first, cache) = Catalog::scan(&library.roots, None, &mut quiet());

        // Same size, same time, unreadable bytes: only a reopen could notice.
        for path in [&library.collection, &library.trim] {
            wreck_keeping_stamp(path);
        }
        let (second, cache_again) = Catalog::scan(&library.roots, Some(&cache), &mut quiet());
        assert_eq!(
            second.stats(),
            ScanStats {
                candidates: 4,
                reused: 3,
                passed_over: 1,
                passed_over_from_cache: 1,
                ..ScanStats::default()
            },
            "nothing was opened: three packages and the photos all came from the cache"
        );
        assert_eq!(second.packages(), first.packages());
        assert_eq!(search(&second, "top_314").len(), 1);
        assert_eq!(cache_again.packages.len(), 3);

        // The proof that the bytes really are wrecked: without the cache both fail.
        let (cold, cold_cache) = Catalog::scan(&library.roots, None, &mut quiet());
        assert_eq!(cold.stats().unreadable, 2);
        assert_eq!(
            cold_cache.packages.len(),
            1,
            "only the untouched folder is cached"
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
        let (touched, _) = Catalog::scan(&library.roots, Some(&cache), &mut quiet());
        assert_eq!((touched.stats().reused, touched.stats().unreadable), (2, 1));
    }

    #[test]
    fn a_package_that_cannot_be_opened_still_appears_and_is_never_cached() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("Broken Cape.zip"),
            b"PK\x03\x04 and then nothing",
        )
        .unwrap();
        let roots = [root("fluffy", dir.path(), false)];
        let (catalog, cache) = Catalog::scan(&roots, None, &mut quiet());
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
    }

    #[test]
    fn the_same_archive_in_two_roots_is_kept_once_at_its_first_place() {
        let dir = tempfile::tempdir().unwrap();
        let (first, second) = (dir.path().join("Mods"), dir.path().join("Downloads"));
        for folder in [&first, &second] {
            fs::create_dir_all(folder).unwrap();
            zip_at(
                &folder.join("Lantern Glow.zip"),
                &[("Lantern Glow/Lantern Glow.pak", b"KPKA")],
            );
        }
        let roots = [
            root("fluffy", &first, false),
            root("downloads", &second, true),
        ];
        let (catalog, cache) = Catalog::scan(&roots, None, &mut quiet());
        assert_eq!(catalog.packages().len(), 1);
        assert_eq!(catalog.packages()[0].root_id, "fluffy");
        assert_eq!(catalog.stats().duplicates, 1);
        assert_eq!(
            cache.packages.len(),
            2,
            "both copies are remembered by path"
        );
        let (again, _) = Catalog::scan(&roots, Some(&cache), &mut quiet());
        assert_eq!((again.stats().reused, again.stats().duplicates), (2, 1));
        assert_eq!(again.packages(), catalog.packages());
    }

    /// An interrupted copy: the archive's full length with its tail zeroed,
    /// so its central directory is gone. Same name, same size: the same
    /// package id as the archive it was copied from.
    fn interrupted_copy(from: &Path, to: &Path) {
        let mut bytes = fs::read(from).unwrap();
        let half = bytes.len() / 2;
        bytes[half..].fill(0);
        fs::write(to, bytes).unwrap();
    }

    #[test]
    fn a_broken_copy_never_hides_a_readable_copy_of_the_same_archive() {
        let dir = tempfile::tempdir().unwrap();
        let (mods, downloads) = (dir.path().join("Mods"), dir.path().join("Downloads"));
        for folder in [&mods, &downloads] {
            fs::create_dir_all(folder).unwrap();
        }
        let readable = downloads.join("Lantern Glow.zip");
        zip_at(&readable, &[("Lantern Glow/Lantern Glow.pak", b"KPKA")]);
        let broken = mods.join("Lantern Glow.zip");
        interrupted_copy(&readable, &broken);
        let roots = [
            root("fluffy", &mods, false),
            root("downloads", &downloads, true),
        ];

        let (catalog, cache) = Catalog::scan(&roots, None, &mut quiet());
        assert_eq!(catalog.packages().len(), 1);
        let kept = &catalog.packages()[0];
        assert_eq!(
            (kept.root_id.as_str(), &kept.path),
            ("downloads", &readable),
            "the readable copy wins, though the broken one sits in the first root"
        );
        assert_eq!(kept.options.len(), 1);
        assert!(kept.problems.is_empty(), "{:?}", kept.problems);
        assert_eq!(
            catalog.stats(),
            ScanStats {
                candidates: 2,
                opened: 1,
                unreadable: 1,
                duplicates: 1,
                ..ScanStats::default()
            },
            "the broken copy was tried, failed, and is the hidden duplicate"
        );
        let remembered: Vec<&Path> = cache.packages.iter().map(|c| c.path.as_path()).collect();
        assert_eq!(remembered, [readable.as_path()]);

        // Every later scan agrees, and so does the startup preview.
        let (again, _) = Catalog::scan(&roots, Some(&cache), &mut quiet());
        assert_eq!(again.packages(), catalog.packages());
        assert_eq!(
            (
                again.stats().reused,
                again.stats().unreadable,
                again.stats().duplicates
            ),
            (1, 1, 1)
        );
        assert_eq!(
            Catalog::from_cache(&roots, &cache).packages(),
            catalog.packages()
        );

        // Readable first, broken second: the first place keeps it, as ever.
        let swapped = [roots[1].clone(), roots[0].clone()];
        let (swapped, _) = Catalog::scan(&swapped, None, &mut quiet());
        assert_eq!(swapped.packages().len(), 1);
        assert_eq!(swapped.packages()[0].path, readable);

        // Two broken copies: the first is shown, unreadable, and nothing is cached.
        interrupted_copy(&readable, &readable);
        let (both_broken, cache) = Catalog::scan(&roots, None, &mut quiet());
        assert_eq!(both_broken.packages().len(), 1);
        assert_eq!(both_broken.packages()[0].path, broken);
        assert_eq!(
            both_broken.packages()[0].problems[0].kind,
            ProblemKind::Unreadable
        );
        assert_eq!(
            (
                both_broken.stats().unreadable,
                both_broken.stats().duplicates,
                both_broken.stats().unreadable_passed_over
            ),
            (2, 0, 1),
            "the Downloads copy is passed over unshown, not kept as a hidden duplicate"
        );
        assert!(cache.packages.is_empty());
    }

    #[test]
    fn what_a_downloads_root_passed_over_is_not_opened_again() {
        let library = library();
        let (_, cache) = Catalog::scan(&library.roots, None, &mut quiet());

        // Same size, same time, unreadable bytes: only a reopen could notice.
        wreck_keeping_stamp(&library.photos);
        let (second, cache_again) = Catalog::scan(&library.roots, Some(&cache), &mut quiet());
        assert!(
            second.packages().iter().all(|p| p.path != library.photos),
            "the photos were passed over from memory, not reopened and found broken"
        );
        assert_eq!(
            (
                second.stats().passed_over,
                second.stats().passed_over_from_cache,
                second.stats().unreadable
            ),
            (1, 1, 0)
        );
        assert_eq!(
            cache_again.passed_over, cache.passed_over,
            "remembered again"
        );

        // An older cache without the field loads, and simply opens them once.
        let mut json = serde_json::to_value(&cache).unwrap();
        json.as_object_mut().unwrap().remove("passedOver").unwrap();
        let older: CatalogCache = serde_json::from_value(json).unwrap();
        assert!(older.passed_over.is_empty());
        let (from_older, _) = Catalog::scan(&library.roots, Some(&older), &mut quiet());
        assert_eq!(
            (
                from_older.stats().passed_over_from_cache,
                from_older.stats().unreadable
            ),
            (0, 1),
            "opened again, the wrecked photos are unreadable"
        );

        // A new stamp means a changed file: opened again.
        touch_later(&library.photos);
        let (touched, touched_cache) = Catalog::scan(&library.roots, Some(&cache), &mut quiet());
        assert_eq!(touched.stats().unreadable, 1);
        assert!(touched_cache.passed_over.is_empty());

        // An ordinary root never passes anything over, remembered or not.
        let dir = tempfile::tempdir().unwrap();
        zip_at(
            &dir.path().join("Holiday Photos.zip"),
            &[("beach.jpg", b"jpg")],
        );
        let downloads = [root("custom", dir.path(), true)];
        let (_, cache) = Catalog::scan(&downloads, None, &mut quiet());
        assert_eq!(cache.passed_over.len(), 1);
        let ordinary = [root("custom", dir.path(), false)];
        let (catalog, _) = Catalog::scan(&ordinary, Some(&cache), &mut quiet());
        assert_eq!(catalog.packages().len(), 1);
        assert_eq!(
            (catalog.stats().opened, catalog.stats().passed_over),
            (1, 0)
        );
    }

    #[test]
    fn what_a_downloads_root_cannot_open_is_passed_over_unshown_and_retried() {
        let dir = tempfile::tempdir().unwrap();
        let papers = dir.path().join("Tax Return 2026.zip");
        fs::write(&papers, b"PK\x03\x04 locked papers, no zip directory").unwrap();
        zip_at(
            &dir.path().join("Lantern Glow.zip"),
            &[("natives/stm/lantern/glow.tex.251211553", b"tex")],
        );

        let downloads = [root("downloads", dir.path(), true)];
        let (catalog, cache) = Catalog::scan(&downloads, None, &mut quiet());
        let names: Vec<&str> = catalog
            .packages()
            .iter()
            .map(|p| p.file_name.as_str())
            .collect();
        assert_eq!(
            names,
            ["Lantern Glow.zip"],
            "the papers are nowhere in the library"
        );
        assert_eq!(
            (
                catalog.stats().unreadable,
                catalog.stats().unreadable_passed_over
            ),
            (1, 1)
        );
        assert!(
            cache
                .packages
                .iter()
                .all(|cached| cached.package.path != papers)
                && cache.passed_over.iter().all(|seen| seen.path != papers),
            "never remembered: the next scan tries it again"
        );
        let (again, _) = Catalog::scan(&downloads, Some(&cache), &mut quiet());
        assert_eq!(again.stats().unreadable_passed_over, 1, "tried again");

        // The same file in an ordinary root is shown, with its problem.
        let ordinary = [root("fluffy", dir.path(), false)];
        let (catalog, _) = Catalog::scan(&ordinary, None, &mut quiet());
        let shown = catalog
            .packages()
            .iter()
            .find(|p| p.path == papers)
            .expect("an ordinary root shows what it cannot open");
        assert_eq!(shown.problems[0].kind, rift_core::ProblemKind::Unreadable);
        assert_eq!(catalog.stats().unreadable_passed_over, 0);
    }

    #[test]
    fn the_startup_preview_passes_over_what_the_scan_would() {
        let dir = tempfile::tempdir().unwrap();
        zip_at(
            &dir.path().join("Holiday Photos.zip"),
            &[("beach.jpg", b"jpg")],
        );
        let ordinary = [root("custom", dir.path(), false)];
        let (_, cache) = Catalog::scan(&ordinary, None, &mut quiet());
        assert_eq!(Catalog::from_cache(&ordinary, &cache).packages().len(), 1);

        // The same folder, now listing only DD2 mods.
        let downloads = [root("custom", dir.path(), true)];
        let preview = Catalog::from_cache(&downloads, &cache);
        let (scanned, _) = Catalog::scan(&downloads, Some(&cache), &mut quiet());
        assert!(scanned.packages().is_empty());
        assert_eq!(
            preview.packages(),
            scanned.packages(),
            "the preview shows what the scan will"
        );
    }

    #[test]
    fn a_cache_of_another_schema_is_not_trusted() {
        let library = library();
        let (_, mut cache) = Catalog::scan(&library.roots, None, &mut quiet());
        cache.schema = CatalogCache::SCHEMA - 1;
        let (catalog, fresh) = Catalog::scan(&library.roots, Some(&cache), &mut quiet());
        assert_eq!((catalog.stats().reused, catalog.stats().opened), (0, 3));
        assert_eq!(fresh.schema, CatalogCache::SCHEMA);
        assert!(Catalog::from_cache(&library.roots, &cache)
            .packages()
            .is_empty());
    }

    #[test]
    fn the_cache_round_trips_through_json_and_rebuilds_the_catalog_without_the_disk() {
        let library = library();
        let (scanned, cache) = Catalog::scan(&library.roots, None, &mut quiet());
        let json = serde_json::to_string(&cache).unwrap();
        let cache: CatalogCache = serde_json::from_str(&json).unwrap();
        drop(library._dir); // every file is gone now

        let rebuilt = Catalog::from_cache(&scanned.roots, &cache);
        assert_eq!(rebuilt.packages(), scanned.packages());
        assert_eq!(search(&rebuilt, "top_314").len(), 1);
        assert_eq!(rebuilt.stats().reused, 3);

        let downloads_only = Catalog::from_cache(&scanned.roots[1..], &cache);
        let files: Vec<&str> = downloads_only
            .packages()
            .iter()
            .map(|p| p.file_name.as_str())
            .collect();
        assert_eq!(files, [ERMINE_TRIM]);
        assert_eq!(
            downloads_only.packages()[0].problems[0].kind,
            ProblemKind::MissingParent,
            "its menu lives in a root that is gone"
        );

        // The persisted shape: camelCase, the schema beside the packages.
        let json: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(json["schema"], CatalogCache::SCHEMA);
        assert!(json["packages"][0]["modifiedMs"].is_u64());
        assert!(json["packages"][0]["package"]["displayName"].is_string());
    }

    #[test]
    fn a_reused_package_follows_its_root_and_its_root_kind() {
        let library = library();
        let (_, cache) = Catalog::scan(&library.roots, None, &mut quiet());

        // The same folder under a new root id.
        let mut renamed = library.roots.clone();
        renamed[0].id = "custom-mods".to_string();
        let (catalog, _) = Catalog::scan(&renamed, Some(&cache), &mut quiet());
        assert_eq!(catalog.stats().reused, 3);
        assert!(catalog.packages()[..2]
            .iter()
            .all(|p| p.root_id == "custom-mods"));

        // A cached package that is no DD2 mod, now in a DD2-shaped-only root.
        let dir = tempfile::tempdir().unwrap();
        zip_at(
            &dir.path().join("Holiday Photos.zip"),
            &[("beach.jpg", b"jpg")],
        );
        let custom = [root("custom", dir.path(), false)];
        let (catalog, cache) = Catalog::scan(&custom, None, &mut quiet());
        assert_eq!(catalog.packages().len(), 1, "an ordinary root keeps it");
        let downloads = [root("custom", dir.path(), true)];
        let (catalog, _) = Catalog::scan(&downloads, Some(&cache), &mut quiet());
        assert!(catalog.packages().is_empty());
        assert_eq!(catalog.stats().passed_over, 1);
    }

    #[test]
    fn a_candidate_that_vanished_is_simply_gone() {
        let dir = tempfile::tempdir().unwrap();
        let fluffy = root("fluffy", dir.path(), false);
        let memory = Memory::of(None);
        assert!(matches!(
            examine(&fluffy, &dir.path().join("Lantern Glow.zip"), &memory),
            Outcome::Gone
        ));
        let (catalog, cache) = Catalog::scan(&[], None, &mut quiet());
        assert!(catalog.packages().is_empty() && cache.packages.is_empty());
    }

    #[test]
    fn nexus_labels_reach_the_catalog_search_without_a_rescan() {
        let library = library();
        let (mut catalog, _) = Catalog::scan(&library.roots, None, &mut quiet());
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
            "the folder pak mod is not linked"
        );

        let (rescanned, _) = Catalog::scan(&library.roots, None, &mut quiet());
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
