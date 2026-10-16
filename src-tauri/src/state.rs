//! AppState — everything the spine holds, and the startup sequence
//! (docs/WINDOW-CONTRACT.md § Startup sequence).
//!
//! **The lock rule: no lock is held across archive reads, game-folder walks,
//! network calls or file writes.** Readers copy what they need and let go:
//! the library ([`Shelf`]) and the journal sit behind `Arc`s, [`AppState::world`]
//! hands out an owned copy of the game, its journal and Fluffy, and the
//! player's files are saved through [`persist::SavedFile`], which asks for the
//! state to write only inside its own turn. A scan builds a whole new catalog
//! before swapping it in. The one long-held lock is `deploy_lock`, on purpose:
//! one writer in the game folder at a time. When more than one lock is held,
//! the order is `deploy_lock` → `world` → everything else, and `shelf` →
//! `nexus`: a shelf is put in place (or relabeled) with the Nexus labels the
//! store holds at that moment, so nothing may wait for the shelf while it
//! holds the store.
//!
//! Every lock is taken through [`lock`], [`read`] or [`write`], which recover
//! from poisoning: a panic in one command must not wedge every command after it.

use std::collections::{HashMap, HashSet};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{
    Arc, Condvar, Mutex, MutexGuard, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard,
};

use rift_catalog::{Catalog, CatalogCache, ScanProgress};
use rift_core::{lower_key, LibraryRoot, OptionId, RiftError, RootKind};
use rift_deploy::{
    DeployContext, FluffyInstall, FluffyLedger, GameInstall, GameSource, GameState, Journal,
    RefStatus,
};
use rift_nexus::NexusClient;

use crate::conflicts::{self, PakHashCache};
use crate::dto::{ConflictView, RecoveryNote, ScanProgressEvent, ScanState};
use crate::events::{self, EventSink};
use crate::fresh::Fresh;
use crate::keys::KeyStore;
use crate::library::{match_fluffy, Shelf, SourcePool};
use crate::nexus::NexusStore;
use crate::persist::{self, DataDirs, Loaded, SavedFile, StoredRoots, StoredSettings};
use crate::watch::{self, LibraryWatcher};

pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

pub fn read<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    lock.read().unwrap_or_else(PoisonError::into_inner)
}

pub fn write<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write().unwrap_or_else(PoisonError::into_inner)
}

/// How the spine finds the game and Fluffy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Discovery {
    /// The app: `RIFTSTONE_GAME_ROOT`, then a hand-picked folder, then Steam;
    /// `RIFTSTONE_FLUFFY_ROOT`, then the bounded search (rift-deploy).
    System,
    /// Tests and harnesses: exactly these folders; nothing is searched, no
    /// environment variable is read.
    Fixed {
        game_root: Option<PathBuf>,
        fluffy_root: Option<PathBuf>,
    },
}

/// Everything the spine needs to know before it opens.
#[derive(Debug, Clone)]
pub struct SpineConfig {
    pub dirs: DataDirs,
    pub discovery: Discovery,
    /// The player's Downloads folder — a default root on the first launch.
    pub downloads_dir: Option<PathBuf>,
    /// Announced to Nexus on every request.
    pub app_version: String,
    /// Point the Nexus client elsewhere (GraphQL URL, v1 base) — tests only.
    pub nexus_endpoints: Option<(String, String)>,
    /// Arm the library watcher (off in tests that do not exercise it).
    pub watch_roots: bool,
}

impl SpineConfig {
    /// The app's own configuration: discovery from the system, the real
    /// Downloads folder, the crate's version.
    pub fn system(dirs: DataDirs) -> SpineConfig {
        SpineConfig {
            dirs,
            discovery: Discovery::System,
            downloads_dir: dirs::download_dir(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            nexus_endpoints: None,
            watch_roots: true,
        }
    }
}

/// The journal of the game folder in use.
#[derive(Debug, Clone)]
pub enum JournalSlot {
    NoGame,
    /// Shared: readers take a copy of the `Arc`; a deploy works on its own
    /// clone and swaps the result in.
    Ready(Arc<Journal>),
    /// It exists and cannot be read. Never reset: it is the only record of
    /// what the Riftstone put in the game folder. Deploys are refused.
    Damaged(RiftError),
}

impl JournalSlot {
    pub fn ready(&self) -> Option<&Journal> {
        match self {
            JournalSlot::Ready(journal) => Some(journal.as_ref()),
            _ => None,
        }
    }
}

/// The game, its journal, Fluffy. Cheap to clone (the journal is an `Arc`):
/// [`AppState::world`] hands out copies so nobody reads the disk under the lock.
#[derive(Debug, Clone)]
pub struct World {
    pub game: Option<GameInstall>,
    pub journal: JournalSlot,
    pub fluffy: Option<FluffyInstall>,
}

/// What the game folder looks like right now, derived from disk. Built once
/// per change and shared ([`Fresh`]): searches and option details take the
/// one built since the last change; the window's re-reads (`get_overview`,
/// `get_installed`) take one built a moment ago at most.
#[derive(Debug, Default)]
pub struct Snapshot {
    pub ledger: Option<FluffyLedger>,
    pub ref_status: Option<RefStatus>,
    pub running: bool,
    pub game_state: Option<GameState>,
    /// Lower-cased Fluffy section → the library option it matched
    /// (un-adopted installs only).
    pub fluffy_matches: HashMap<String, OptionId>,
    /// Options an un-adopted Fluffy install matched.
    pub fluffy_installed: HashSet<OptionId>,
    pub conflicts: Vec<ConflictView>,
    /// Option ids taking part in any conflict.
    pub conflicted: HashSet<String>,
}

/// One deploy operation's view of the world, owned.
pub struct Scene {
    pub game: GameInstall,
    pub ref_status: RefStatus,
    pub ledger: Option<FluffyLedger>,
    pub running: bool,
}

impl Scene {
    pub fn ctx<'a>(&'a self, state_dir: &'a Path) -> DeployContext<'a> {
        DeployContext {
            game: &self.game,
            state_dir,
            ref_status: &self.ref_status,
            fluffy: self.ledger.as_ref(),
            game_running: self.running,
        }
    }
}

/// One of the player's files that could not be read at launch, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldFile {
    /// `settings`, `roots` or `nexus`.
    pub what: &'static str,
    pub path: PathBuf,
    pub why: String,
}

pub struct AppState {
    pub(crate) config: SpineConfig,
    pub(crate) events: Arc<dyn EventSink>,
    pub(crate) keys: Box<dyn KeyStore>,
    pub(crate) settings: RwLock<StoredSettings>,
    pub(crate) roots: RwLock<Vec<LibraryRoot>>,
    first_launch: AtomicBool,
    settings_file: SavedFile,
    roots_file: SavedFile,
    nexus_file: SavedFile,
    shelf: RwLock<Arc<Shelf>>,
    catalog_cache: Mutex<Arc<CatalogCache>>,
    pub(crate) last_scan_ms: Mutex<Option<u64>>,
    scan: Mutex<Option<ScanProgress>>,
    /// Signalled whenever a scan ends.
    scan_idle: Condvar,
    scan_again: AtomicBool,
    pub(crate) world: RwLock<World>,
    pub(crate) deploy_lock: Mutex<()>,
    pub(crate) recovered: Mutex<Option<RecoveryNote>>,
    snapshot: Fresh<Snapshot>,
    pub(crate) paks: PakHashCache,
    pub(crate) sources: SourcePool,
    pub(crate) nexus: Mutex<NexusStore>,
    pub(crate) nexus_client: RwLock<NexusClient>,
    pub(crate) nexus_check: Mutex<()>,
    pub(crate) nexus_checking: AtomicBool,
    pub(crate) has_api_key: AtomicBool,
    watcher: Mutex<Option<LibraryWatcher>>,
}

impl AppState {
    // -- startup ------------------------------------------------------------

    /// Startup steps 1–3, on the caller's thread and fast: settings, roots,
    /// the catalog cache, the Nexus cache, the game, its journal — and the
    /// recovery of an install a crash interrupted. Fluffy's bounded search and
    /// everything slow happen in [`AppState::wake`].
    ///
    /// A file of the player's that exists but cannot be read starts that part
    /// from its defaults and is held: never written over this session.
    pub fn open(
        config: SpineConfig,
        events: Arc<dyn EventSink>,
        keys: Box<dyn KeyStore>,
    ) -> Arc<AppState> {
        let dirs = &config.dirs;
        let settings_loaded: Loaded<StoredSettings> = persist::load_json(&dirs.settings_file());
        let settings_file = SavedFile::new(dirs.settings_file(), settings_loaded.unreadable());
        let settings = settings_loaded.found().unwrap_or_default();
        let roots_loaded: Loaded<StoredRoots> = persist::load_json(&dirs.roots_file());
        // A roots.json that is not there — or was damaged and set aside —
        // makes this a first launch (the default roots are made). One that is
        // there but unreadable is not: it still holds the player's folders.
        let first_launch = matches!(roots_loaded, Loaded::Absent | Loaded::Damaged);
        let roots_file = SavedFile::new(dirs.roots_file(), roots_loaded.unreadable());
        let roots = roots_loaded
            .found()
            .map(|stored| stored.roots)
            .unwrap_or_default();
        let cache_file = dirs.catalog_cache_file();
        let cache: Option<CatalogCache> = persist::read_json::<CatalogCache>(&cache_file)
            .filter(|cache| cache.schema == CatalogCache::SCHEMA);
        let last_scan_ms = cache
            .as_ref()
            .and_then(|_| persist::modified_ms(&cache_file));
        let cache = cache.unwrap_or_default();
        let nexus_loaded: Loaded<NexusStore> = persist::load_json(&dirs.nexus_file());
        let nexus_file = SavedFile::new(dirs.nexus_file(), nexus_loaded.unreadable());
        // `new`, not `default`: a fresh store carries the current schema.
        let nexus = match nexus_loaded.found() {
            Some(store) => store,
            None => NexusStore::new(),
        };

        let api_key = match keys.read() {
            Ok(key) => key,
            Err(err) => {
                log::warn!("{err}");
                None
            }
        };
        let has_api_key = api_key.is_some();
        let mut client = NexusClient::new(&config.app_version);
        if let Some((graphql, v1)) = &config.nexus_endpoints {
            client = client.with_endpoints(graphql, v1);
        }
        let client = client.with_api_key(api_key);

        let game = find_game(&config.discovery, settings.manual_game_root.as_deref());
        let journal = load_journal(dirs.state_dir(), game.as_ref());
        // The labels nexus.json kept: the window filters by them at once.
        let shelf = Shelf::from_cache(&roots, &cache);
        let shelf = {
            let labels = nexus.package_labels(shelf.packages());
            shelf.with_labels(labels)
        };

        let state = Arc::new(AppState {
            events,
            keys,
            settings: RwLock::new(settings),
            roots: RwLock::new(roots),
            first_launch: AtomicBool::new(first_launch),
            settings_file,
            roots_file,
            nexus_file,
            shelf: RwLock::new(Arc::new(shelf)),
            catalog_cache: Mutex::new(Arc::new(cache)),
            last_scan_ms: Mutex::new(last_scan_ms),
            scan: Mutex::new(None),
            scan_idle: Condvar::new(),
            scan_again: AtomicBool::new(false),
            world: RwLock::new(World {
                game,
                journal,
                fluffy: None,
            }),
            deploy_lock: Mutex::new(()),
            recovered: Mutex::new(None),
            snapshot: Fresh::default(),
            paks: PakHashCache::default(),
            sources: SourcePool::default(),
            nexus: Mutex::new(nexus),
            nexus_client: RwLock::new(client),
            nexus_check: Mutex::new(()),
            nexus_checking: AtomicBool::new(false),
            has_api_key: AtomicBool::new(has_api_key),
            watcher: Mutex::new(None),
            config,
        });
        state.recover_journal();
        state
    }

    /// Startup steps 3–6, on a background thread: find Fluffy, create the
    /// default roots on the first launch, arm the watcher, scan, and check
    /// Nexus when the last check is stale.
    pub fn wake(self: &Arc<Self>) {
        let me = Arc::clone(self);
        let spawned = std::thread::Builder::new()
            .name("riftstone-wake".into())
            .spawn(move || {
                me.find_fluffy();
                me.arm_watcher();
                // When a scan already runs (the window's, the watcher's, a
                // new root's), that one is the startup scan: the auto-check
                // waits for it, so it judges the library the scan found.
                if !me.scan_blocking() {
                    me.wait_until_scanned();
                }
                crate::commands::nexus::auto_check(&me);
            });
        if let Err(err) = spawned {
            log::error!("could not start the background wake-up: {err}");
        }
    }

    /// Discover Fluffy; on the first launch add the default roots (its Mods
    /// folder when found, Downloads as a DD2-shaped-only root) in front of
    /// any folder the player added meanwhile — never in place of one.
    pub fn find_fluffy(&self) {
        let fluffy = match &self.config.discovery {
            Discovery::System => rift_deploy::discover_fluffy(),
            Discovery::Fixed { fluffy_root, .. } => fluffy_root
                .as_ref()
                .and_then(|root| rift_deploy::discover_fluffy_in(std::slice::from_ref(root))),
        };
        write(&self.world).fluffy = fluffy.clone();
        if self.first_launch.swap(false, Ordering::SeqCst) {
            let mut defaults = Vec::new();
            if let Some(fluffy) = fluffy.as_ref().filter(|f| f.mods_dir.is_dir()) {
                defaults.push(fluffy_root(&fluffy.mods_dir));
            }
            if let Some(downloads) = self.config.downloads_dir.as_ref().filter(|d| d.is_dir()) {
                defaults.push(downloads_root(downloads));
            }
            {
                let mut roots = write(&self.roots);
                let mut merged: Vec<LibraryRoot> = defaults
                    .into_iter()
                    .filter(|default| {
                        !roots
                            .iter()
                            .any(|r| r.id == default.id || same_folder(&r.path, &default.path))
                    })
                    .collect();
                merged.append(&mut roots);
                *roots = merged;
            }
            self.save_roots();
            self.reshelve_from_cache();
        }
        self.invalidate_snapshot();
        self.events.changed(events::GAME_CHANGED);
    }

    // -- discovery ------------------------------------------------------------

    /// Roll back an operation a crash left in the journal (startup step 2).
    pub fn recover_journal(&self) {
        let _turn = lock(&self.deploy_lock);
        self.recover_locked();
    }

    /// [`AppState::recover_journal`] with the deploy lock already held. A
    /// rollback that stops unexpectedly leaves its intent in the journal (and
    /// every change refused) for the next launch, instead of taking the
    /// window down with it.
    fn recover_locked(&self) {
        let Ok(result) = panic::catch_unwind(AssertUnwindSafe(|| {
            self.deploy_unlocked(rift_deploy::recover)
        })) else {
            log::error!(
                "rolling back the interrupted change stopped unexpectedly; it stays in the journal, and every change waits for the next launch"
            );
            return;
        };
        match result {
            Ok(Some(report)) => {
                log::warn!(
                    "rolled back an interrupted change to {} ({} file(s))",
                    report.option_name,
                    report.files_rolled_back
                );
                *lock(&self.recovered) = Some((&report).into());
            }
            Ok(None) => {}
            Err(RiftError::GameNotFound) => {}
            Err(err) => log::error!("could not recover the journal: {err}"),
        }
    }

    /// Point the Riftstone at another game folder (already validated): load
    /// its journal, recover it, remember the choice.
    pub fn switch_game(&self, game: GameInstall) {
        let _turn = lock(&self.deploy_lock);
        let journal = load_journal(self.config.dirs.state_dir(), Some(&game));
        {
            let mut world = write(&self.world);
            world.game = Some(game.clone());
            world.journal = journal;
        }
        if game.source == GameSource::Manual {
            write(&self.settings).manual_game_root = Some(game.root.clone());
            self.save_settings();
        }
        *lock(&self.recovered) = None;
        self.recover_locked();
        self.invalidate_snapshot();
    }

    // -- settings and roots --------------------------------------------------

    /// Persist the settings as they stand now (never under the settings lock).
    pub fn save_settings(&self) {
        self.settings_file
            .save(true, || read(&self.settings).clone());
    }

    /// Persist the roots as they stand now (never under the roots lock).
    pub fn save_roots(&self) {
        self.roots_file
            .save(true, || StoredRoots::new(read(&self.roots).clone()));
    }

    /// Persist the Nexus store as it stands now (never under its lock).
    pub fn save_nexus(&self) {
        self.nexus_file.save(true, || lock(&self.nexus).clone());
    }

    /// The player's files that could not be read at launch: each starts from
    /// its defaults and is not written this session.
    pub fn held_files(&self) -> Vec<HeldFile> {
        [
            ("settings", &self.settings_file),
            ("roots", &self.roots_file),
            ("nexus", &self.nexus_file),
        ]
        .into_iter()
        .filter_map(|(what, file)| {
            file.held().map(|why| HeldFile {
                what,
                path: file.path().to_path_buf(),
                why: why.to_string(),
            })
        })
        .collect()
    }

    pub fn roots_snapshot(&self) -> Vec<LibraryRoot> {
        read(&self.roots).clone()
    }

    /// (Re-)arm the library watcher on the current roots.
    pub fn arm_watcher(self: &Arc<Self>) {
        if !self.config.watch_roots {
            return;
        }
        let roots = self.roots_snapshot();
        let own = [
            self.config.dirs.data.clone(),
            self.config.dirs.cache.clone(),
        ];
        let weak = Arc::downgrade(self);
        let armed = watch::arm(&roots, &own, watch::DEBOUNCE, move || {
            if let Some(state) = weak.upgrade() {
                state.request_scan();
            }
        });
        *lock(&self.watcher) = armed;
    }

    // -- the library -----------------------------------------------------------

    pub fn shelf(&self) -> Arc<Shelf> {
        Arc::clone(&read(&self.shelf))
    }

    fn catalog_cache_snapshot(&self) -> Arc<CatalogCache> {
        Arc::clone(&lock(&self.catalog_cache))
    }

    /// Rebuild the shelf from the catalog cache for the current roots — no
    /// archive is opened (a removed root's packages simply drop out).
    pub fn reshelve_from_cache(&self) {
        let roots = self.roots_snapshot();
        let cache = self.catalog_cache_snapshot();
        self.shelve(Shelf::from_cache(&roots, &cache));
        self.sources.clear();
        self.invalidate_snapshot();
    }

    /// Put a newly built shelf in place, labeled with what the Nexus store
    /// holds at this moment. Both happen under the shelf's write lock, so a
    /// check that ends meanwhile either is seen here or relabels this shelf
    /// right after ([`AppState::relabel`]) — never lost under an older swap.
    fn shelve(&self, shelf: Shelf) {
        let mut current = write(&self.shelf);
        let labels = lock(&self.nexus).package_labels(shelf.packages());
        *current = Arc::new(shelf.with_labels(labels));
    }

    /// The Nexus store's labels changed (a check heard them, a link changed):
    /// hand them to the shelf in place — only the index's label layer is
    /// rebuilt, never the library. Call it before telling the window
    /// (`rift://updates-changed`), and with no shelf of your own in hand: a
    /// reader that still holds the shelf keeps reading the old one, so the
    /// labeled shelf is a copy then.
    pub fn relabel(&self) {
        let mut current = write(&self.shelf);
        let labels = lock(&self.nexus).package_labels(current.packages());
        if current.carries_labels(&labels) {
            return;
        }
        Arc::make_mut(&mut *current).set_labels(labels);
    }

    /// What the overview says about the scan.
    pub fn scan_state(&self) -> ScanState {
        match lock(&self.scan).as_ref() {
            None => ScanState::Idle,
            Some(progress) => ScanState::Scanning {
                done: progress.done,
                total: progress.total,
                current: progress.current.clone(),
            },
        }
    }

    fn try_begin_scan(&self) -> bool {
        let mut scan = lock(&self.scan);
        if scan.is_some() {
            return false;
        }
        *scan = Some(ScanProgress {
            done: 0,
            total: 0,
            current: String::new(),
        });
        true
    }

    /// Start a background scan; `false` (and nothing happens) when one runs.
    pub fn start_scan(self: &Arc<Self>) -> bool {
        if !self.try_begin_scan() {
            return false;
        }
        let me = Arc::clone(self);
        let spawned = std::thread::Builder::new()
            .name("riftstone-scan".into())
            .spawn(move || me.scan_loop());
        if let Err(err) = spawned {
            log::error!("could not start the library scan: {err}");
            *lock(&self.scan) = None;
            self.scan_idle.notify_all();
            return false;
        }
        true
    }

    /// Wait until no scan runs (one that a request chains on is waited for too).
    pub fn wait_until_scanned(&self) {
        let mut scan = lock(&self.scan);
        while scan.is_some() {
            scan = self
                .scan_idle
                .wait(scan)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// The watcher's request: scan now, or once more when the running scan ends.
    pub fn request_scan(self: &Arc<Self>) {
        if !self.start_scan() {
            self.scan_again.store(true, Ordering::SeqCst);
        }
    }

    /// Scan on this thread (the wake-up thread, the tests). `false` when a
    /// scan was already running.
    pub fn scan_blocking(&self) -> bool {
        if !self.try_begin_scan() {
            return false;
        }
        self.scan_loop();
        true
    }

    fn scan_loop(&self) {
        loop {
            self.scan_once();
            if self.scan_again.swap(false, Ordering::SeqCst) {
                continue;
            }
            *lock(&self.scan) = None;
            self.events.changed(events::LIBRARY_CHANGED);
            // Woken after the event: what waits for the scan (the launch
            // auto-check) follows the window's library-changed, never leads it.
            self.scan_idle.notify_all();
            // A request that slipped in after the check above runs now.
            if self.scan_again.swap(false, Ordering::SeqCst) && self.try_begin_scan() {
                continue;
            }
            break;
        }
    }

    /// One pass: a fresh catalog from every root (cache-backed), swapped in
    /// whole; the cache persisted.
    fn scan_once(&self) {
        let roots = self.roots_snapshot();
        let cache = self.catalog_cache_snapshot();
        let events = &self.events;
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
            Catalog::scan(&roots, Some(&cache), &mut |progress: ScanProgress| {
                events.emit(
                    events::SCAN_PROGRESS,
                    &ScanProgressEvent {
                        done: progress.done,
                        total: progress.total,
                        current: progress.current.clone(),
                    },
                );
                *lock(&self.scan) = Some(progress);
            })
        }));
        match outcome {
            Ok((catalog, fresh)) => {
                self.shelve(Shelf::scanned(catalog));
                // Written only when it changed: the watcher rescans after every
                // change in a Downloads folder, and most leave the library as it was.
                let unchanged = matches!(
                    (serde_json::to_value(&fresh), serde_json::to_value(&*cache)),
                    (Ok(new), Ok(old)) if new == old
                );
                if !unchanged {
                    if let Err(err) =
                        persist::write_json(&self.config.dirs.catalog_cache_file(), &fresh, false)
                    {
                        log::warn!("{err}");
                    }
                }
                *lock(&self.catalog_cache) = Arc::new(fresh);
                *lock(&self.last_scan_ms) = Some(persist::now_ms());
                self.sources.clear();
                self.invalidate_snapshot();
                // A root added or removed mid-scan: scan again with the new set.
                if self.roots_snapshot() != roots {
                    self.scan_again.store(true, Ordering::SeqCst);
                }
            }
            Err(_) => log::error!(
                "the library scan stopped unexpectedly; the library stays as it was before it"
            ),
        }
    }

    // -- the game, derived -------------------------------------------------------

    /// The game, its journal and Fluffy as they stand — an owned copy, so the
    /// caller reads the disk with no lock held.
    pub fn world(&self) -> World {
        read(&self.world).clone()
    }

    /// Something the snapshot is made of changed (an install, a scan, a
    /// switch, a root, Fluffy found): the next ask reads the game folder again.
    pub fn invalidate_snapshot(&self) {
        self.snapshot.invalidate();
    }

    /// The snapshot built since the last change — shared, so a burst of asks
    /// after one change reads the game folder once (searches, option details).
    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.snapshot.get(|| self.compute_snapshot())
    }

    /// The game folder read again, at most a moment ago: what the window's
    /// re-reads show (`get_overview`, `get_installed`), so changes the spine
    /// did not make itself (Fluffy, the game starting, REFramework copied in)
    /// appear too.
    pub fn refresh_snapshot(&self) -> Arc<Snapshot> {
        self.snapshot.get_recent(|| self.compute_snapshot())
    }

    /// How many times the game folder has been read for a snapshot.
    pub fn snapshot_builds(&self) -> u64 {
        self.snapshot.builds()
    }

    fn compute_snapshot(&self) -> Snapshot {
        let world = self.world();
        let shelf = self.shelf();
        let ledger = world.fluffy.as_ref().and_then(read_fluffy_ledger);
        let journal = world.journal.ready();
        let adopted = journal.map(Journal::adopted_sections).unwrap_or_default();

        let mut fluffy_matches = HashMap::new();
        let mut fluffy_installed = HashSet::new();
        if let Some(ledger) = &ledger {
            for fluffy_mod in ledger.unadopted(&adopted) {
                if let Some(option) = match_fluffy(&shelf, fluffy_mod) {
                    fluffy_matches.insert(lower_key(&fluffy_mod.section), option.id.clone());
                    fluffy_installed.insert(option.id.clone());
                }
            }
        }

        let mut snapshot = Snapshot {
            fluffy_matches,
            fluffy_installed,
            ..Snapshot::default()
        };
        if let Some(game) = &world.game {
            let ref_status = rift_deploy::read_status(&game.root);
            let running = rift_deploy::is_game_running();
            if let Some(journal) = journal {
                let ctx = DeployContext {
                    game,
                    state_dir: self.config.dirs.state_dir(),
                    ref_status: &ref_status,
                    fluffy: ledger.as_ref(),
                    game_running: running,
                };
                let game_state = rift_deploy::read_game_state(&ctx, journal);
                let layers = conflicts::game_layers(
                    &game.root,
                    journal,
                    &game_state,
                    ledger.as_ref(),
                    &snapshot.fluffy_matches,
                    &self.paks,
                );
                snapshot.conflicts = conflicts::detect(&layers);
                snapshot.conflicted = conflicts::conflicted_ids(&snapshot.conflicts);
                snapshot.game_state = Some(game_state);
            }
            snapshot.ref_status = Some(ref_status);
            snapshot.running = running;
        }
        snapshot.ledger = ledger;
        snapshot
    }

    // -- deploying ---------------------------------------------------------------

    fn scene(world: &World) -> Result<Scene, RiftError> {
        let game = world.game.clone().ok_or(RiftError::GameNotFound)?;
        Ok(Scene {
            ref_status: rift_deploy::read_status(&game.root),
            ledger: world.fluffy.as_ref().and_then(read_fluffy_ledger),
            running: rift_deploy::is_game_running(),
            game,
        })
    }

    /// Plan against the current journal; nothing is written. The plan reads
    /// the game folder on a copy of the world — no lock held.
    pub fn plan<R>(&self, op: impl FnOnce(&DeployContext, &Journal) -> R) -> Result<R, RiftError> {
        let world = self.world();
        let journal = match &world.journal {
            JournalSlot::Ready(journal) => Arc::clone(journal),
            JournalSlot::Damaged(err) => return Err(err.clone()),
            JournalSlot::NoGame => return Err(RiftError::GameNotFound),
        };
        let scene = Self::scene(&world)?;
        Ok(op(&scene.ctx(self.config.dirs.state_dir()), &journal))
    }

    /// Run one journal-changing operation: one at a time, against a copy of
    /// the journal that replaces the original when the engine is done (the
    /// engine saved its intent and its commit to disk itself, success or not).
    pub fn deploy<R>(
        &self,
        op: impl FnOnce(&DeployContext, &mut Journal) -> Result<R, RiftError>,
    ) -> Result<R, RiftError> {
        let _turn = lock(&self.deploy_lock);
        self.deploy_unlocked(op)
    }

    fn deploy_unlocked<R>(
        &self,
        op: impl FnOnce(&DeployContext, &mut Journal) -> Result<R, RiftError>,
    ) -> Result<R, RiftError> {
        let world = self.world();
        let mut journal = match &world.journal {
            JournalSlot::Ready(journal) => Journal::clone(journal),
            JournalSlot::Damaged(err) => return Err(err.clone()),
            JournalSlot::NoGame => return Err(RiftError::GameNotFound),
        };
        let scene = Self::scene(&world)?;
        let ctx = scene.ctx(self.config.dirs.state_dir());
        let result = match panic::catch_unwind(AssertUnwindSafe(|| op(&ctx, &mut journal))) {
            Ok(result) => result,
            Err(stopped) => {
                // The engine saved its intent before the first byte moved; the
                // copy in hand stopped somewhere mid-operation. The journal on
                // disk goes back into the world, so the next change is refused
                // and nothing writes over the intent its rollback needs (RS-4).
                // The next launch rolls it back (startup step 2).
                let on_disk = load_journal(self.config.dirs.state_dir(), Some(&scene.game));
                {
                    let mut world = write(&self.world);
                    if world
                        .game
                        .as_ref()
                        .is_some_and(|game| game.root == scene.game.root)
                    {
                        world.journal = on_disk;
                    }
                }
                self.invalidate_snapshot();
                panic::resume_unwind(stopped);
            }
        };
        {
            let mut world = write(&self.world);
            if world
                .game
                .as_ref()
                .is_some_and(|game| game.root == scene.game.root)
            {
                world.journal = JournalSlot::Ready(Arc::new(journal));
            }
        }
        self.invalidate_snapshot();
        result
    }

    /// The installed options and how they got there, for search badges.
    pub fn journal_installs(&self) -> HashMap<OptionId, (bool, String)> {
        let journal = match &read(&self.world).journal {
            JournalSlot::Ready(journal) => Arc::clone(journal),
            _ => return HashMap::new(),
        };
        journal
            .installs
            .iter()
            .map(|i| {
                (
                    i.option_id.clone(),
                    (i.is_adopted(), i.installed_at.clone()),
                )
            })
            .collect()
    }
}

/// Find the game: the fixed folder (tests), or `RIFTSTONE_GAME_ROOT`, then a
/// hand-picked folder, then Steam.
fn find_game(discovery: &Discovery, manual: Option<&Path>) -> Option<GameInstall> {
    match discovery {
        Discovery::Fixed { game_root, .. } => game_root
            .as_ref()
            .and_then(|root| rift_deploy::validate_game_root(root).ok())
            .map(|game| GameInstall {
                source: GameSource::Env,
                ..game
            }),
        Discovery::System => {
            let env_set = std::env::var_os(rift_deploy::game::ENV_GAME_ROOT)
                .is_some_and(|value| !value.is_empty());
            if !env_set {
                if let Some(game) =
                    manual.and_then(|root| rift_deploy::validate_game_root(root).ok())
                {
                    return Some(game);
                }
            }
            rift_deploy::discover_game()
        }
    }
}

fn load_journal(state_dir: &Path, game: Option<&GameInstall>) -> JournalSlot {
    match game {
        None => JournalSlot::NoGame,
        Some(game) => match Journal::load(state_dir, &game.root) {
            Ok(journal) => JournalSlot::Ready(Arc::new(journal)),
            Err(err) => {
                log::error!("{err}");
                JournalSlot::Damaged(err)
            }
        },
    }
}

/// Fluffy's `installed.ini`, when Fluffy has one. Read fresh each time: the
/// player may use Fluffy beside the Riftstone.
pub fn read_fluffy_ledger(fluffy: &FluffyInstall) -> Option<FluffyLedger> {
    let ini = fluffy
        .installed_ini
        .clone()
        .or_else(|| Some(fluffy.game_dir.join("installed.ini")).filter(|p| p.is_file()))?;
    match rift_deploy::read_ledger(&ini) {
        Ok(ledger) => Some(ledger),
        Err(err) => {
            log::warn!("{err}");
            None
        }
    }
}

/// The built-in root over Fluffy's Mods folder.
pub fn fluffy_root(mods_dir: &Path) -> LibraryRoot {
    LibraryRoot {
        id: "fluffy".into(),
        path: mods_dir.to_path_buf(),
        label: "Fluffy's mods".into(),
        kind: RootKind::Fluffy,
        dd2_shaped_only: false,
    }
}

/// The built-in root over the Downloads folder: only DD2-shaped archives.
pub fn downloads_root(downloads: &Path) -> LibraryRoot {
    LibraryRoot {
        id: "downloads".into(),
        path: downloads.to_path_buf(),
        label: "Downloads".into(),
        kind: RootKind::Downloads,
        dd2_shaped_only: true,
    }
}

/// Two folder paths name the same place: case-insensitive, either slash,
/// no trailing separator (the Windows rules).
pub fn same_folder(a: &Path, b: &Path) -> bool {
    let key = |p: &Path| {
        let text = p.to_string_lossy().replace('\\', "/");
        lower_key(text.trim_end_matches('/'))
    };
    key(a) == key(b)
}
