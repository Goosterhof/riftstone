//! AppState — everything the spine holds, and the startup sequence
//! (docs/WINDOW-CONTRACT.md § Startup sequence).
//!
//! **The lock rule: no lock is held across archive reads, game-folder walks,
//! network calls or file writes.** Readers copy what they need and let go:
//! the library ([`Shelf`]) and the journal sit behind `Arc`s, [`AppState::world`]
//! hands out an owned copy of the game and its journal, and the player's
//! files are saved through [`persist::SavedFile`], which asks for the state
//! to write only inside its own turn. A scan builds a whole new catalog
//! before swapping it in. The one long-held lock is `deploy_lock`, on purpose:
//! one writer in the game folder at a time. When more than one lock is held,
//! the order is `deploy_lock` → `world` → everything else, and `shelf` →
//! `nexus`: a shelf is put in place (or relabeled) with the Nexus labels the
//! store holds at that moment, so nothing may wait for the shelf while it
//! holds the store.
//!
//! **The one folder (v0.3, RS-5).** The library is the mods folder
//! ([`SpineConfig::library_dir`]): made at launch when it is missing, cleared
//! of the stone's own leftover partial downloads, scanned, and watched.
//! After every scan the migration ([`AppState::migrate`]) gives the journal
//! and `nexus.json` the v0.3 ids of what they knew under older ones. Its
//! relinks stand on the bytes: before the migration takes the deploy lock,
//! the scan reads, with no lock held, the package entries that confirm them
//! ([`AppState::confirm_relinks`], bounded per scan), and the migration
//! applies only what they confirmed.
//!
//! **The game (v0.3).** [`GameWord`] holds what the spine believes about the
//! game: the game watcher ([`AppState::watch_game`]) samples [`GameWatch`]
//! and only a start or a stop changes the belief. The snapshot reads the
//! belief, never the process list, so a burst of reads after one change
//! still reads the game folder once; a change to the game folder (a deploy,
//! Play) asks the watch afresh ([`AppState::observe_game`]).
//!
//! **The key's extras (v0.3).** The stored key's account is a word of its own
//! (`account`, [`crate::account`]), validated at the wake on a thread of its
//! own (the first scan never delays it); nxm links that arrive before that
//! validation is done, or before the window listens, wait at the link door
//! ([`AppState::hold_link`]); the standings it heard are a session cache
//! (`standings`); downloads live in their own list with their own worker
//! (`downloads`, [`crate::downloads`]), which holds the spine only weakly
//! while it waits. None of these locks is held across a network call or a
//! disk write, none is taken together with `deploy_lock` or `nexus_check`,
//! and a download never waits for an install or an update check (nor they
//! for it).
//!
//! Every lock is taken through [`lock`], [`read`] or [`write()`], which recover
//! from poisoning: a panic in one command must not wedge every command after it.

use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::fs;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{
    Arc, Condvar, Mutex, MutexGuard, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard,
    TryLockError,
};
use std::time::Instant;

use rift_catalog::{Catalog, CatalogCache, ScanProgress};
use rift_core::{OptionId, PackageId, RiftError};
use rift_deploy::{DeployContext, GameInstall, GameSource, GameState, Journal, RefStatus};
use rift_nexus::NexusClient;

use crate::account::{AccountWord, StandingsCache};
use crate::conflicts::{self, PakHashCache};
use crate::downloads::Downloads;
use crate::dto::{ConflictView, RecoveryNote, ScanProgressEvent, ScanState};
use crate::events::{self, EventSink};
use crate::family::Families;
use crate::fresh::Fresh;
use crate::keys::KeyStore;
use crate::library::{Shelf, SourcePool};
use crate::migrate;
use crate::nexus::NexusStore;
use crate::persist::{self, DataDirs, Loaded, SavedFile, StoredSettings};
use crate::play::{
    GameCadence, GameLauncher, GameRun, GameWatch, GameWord, SystemGameWatch, SystemLauncher,
};
use crate::protocol::{self, ProtocolRegistry};
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

/// Env override for the mods folder (the WSL bench, `npm run tauri dev`).
pub const MODS_DIR_ENV: &str = "RIFTSTONE_MODS_DIR";

/// The mods folder (RS-5): `override_dir` when it is set and not empty
/// (`RIFTSTONE_MODS_DIR`), else `mods` beside `exe` (the installed
/// `riftstone.exe`). Never canonicalized: the folder may not exist yet, and
/// the player reads the path as Windows spells it.
pub fn mods_dir_from(exe: &Path, override_dir: Option<OsString>) -> PathBuf {
    match override_dir.filter(|dir| !dir.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => exe
            .parent()
            .map_or_else(|| PathBuf::from("mods"), |dir| dir.join("mods")),
    }
}

/// How the spine finds the game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Discovery {
    /// The app: `RIFTSTONE_GAME_ROOT`, then a hand-picked folder, then Steam
    /// (rift-deploy).
    System,
    /// Tests and harnesses: exactly this folder (found as `env`); nothing is
    /// searched, no environment variable is read.
    Fixed { game_root: Option<PathBuf> },
}

/// Everything the spine needs to know before it opens.
#[derive(Clone)]
pub struct SpineConfig {
    pub dirs: DataDirs,
    pub discovery: Discovery,
    /// The one library: the mods folder ([`mods_dir_from`]).
    pub library_dir: PathBuf,
    /// Announced to Nexus on every request.
    pub app_version: String,
    /// Point the Nexus client elsewhere (GraphQL URL, v1 base) — tests only.
    pub nexus_endpoints: Option<(String, String)>,
    /// Arm the mods-folder watcher (off in tests that do not exercise it).
    pub watch_library: bool,
    /// How Play starts the game and the mods folder is shown.
    pub launcher: Arc<dyn GameLauncher>,
    /// Whether Dragon's Dogma 2 runs.
    pub game_watch: Arc<dyn GameWatch>,
    /// How often the game watcher asks.
    pub game_cadence: GameCadence,
    /// v0.3: where nxm:// links are registered (the Windows registry; an
    /// unsupported one elsewhere; a memory one in the tests).
    pub protocols: Arc<dyn ProtocolRegistry>,
    /// v0.3: the stone's own `riftstone.exe`: what the nxm handler registers
    /// and recognises as its own.
    pub exe: PathBuf,
}

impl std::fmt::Debug for SpineConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpineConfig")
            .field("dirs", &self.dirs)
            .field("discovery", &self.discovery)
            .field("library_dir", &self.library_dir)
            .field("app_version", &self.app_version)
            .field("nexus_endpoints", &self.nexus_endpoints)
            .field("watch_library", &self.watch_library)
            .field("game_cadence", &self.game_cadence)
            .field("exe", &self.exe)
            .finish_non_exhaustive()
    }
}

impl SpineConfig {
    /// The app's own configuration: discovery from the system, the mods
    /// folder resolved at launch, the desktop's launcher, process list and
    /// registry, the running exe, the crate's version.
    pub fn system(dirs: DataDirs, library_dir: PathBuf) -> SpineConfig {
        SpineConfig {
            dirs,
            discovery: Discovery::System,
            library_dir,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            nexus_endpoints: None,
            watch_library: true,
            launcher: Arc::new(SystemLauncher),
            game_watch: Arc::new(SystemGameWatch),
            game_cadence: GameCadence::default(),
            protocols: protocol::system_registry(),
            exe: std::env::current_exe().unwrap_or_else(|_| PathBuf::from("riftstone.exe")),
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

/// The game and its journal. Cheap to clone (the journal is an `Arc`):
/// [`AppState::world`] hands out copies so nobody reads the disk under the lock.
#[derive(Debug, Clone)]
pub struct World {
    pub game: Option<GameInstall>,
    pub journal: JournalSlot,
}

/// What the game folder looks like right now, derived from disk. Built once
/// per change and shared ([`Fresh`]): searches and option details take the
/// one built since the last change; the window's re-reads (`get_overview`,
/// `get_installed`) take one built a moment ago at most.
#[derive(Debug, Default)]
pub struct Snapshot {
    pub ref_status: Option<RefStatus>,
    /// The game watcher's word when the snapshot was built.
    pub running: bool,
    pub game_state: Option<GameState>,
    pub conflicts: Vec<ConflictView>,
    /// Option ids taking part in any conflict.
    pub conflicted: std::collections::HashSet<String>,
}

/// One deploy operation's view of the world, owned.
pub struct Scene {
    pub game: GameInstall,
    pub ref_status: RefStatus,
    pub running: bool,
}

impl Scene {
    pub fn ctx<'a>(&'a self, state_dir: &'a Path) -> DeployContext<'a> {
        DeployContext {
            game: &self.game,
            state_dir,
            ref_status: &self.ref_status,
            game_running: self.running,
        }
    }
}

/// One of the player's files that could not be read at launch, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldFile {
    /// `settings` or `nexus`.
    pub what: &'static str,
    pub path: PathBuf,
    pub why: String,
}

pub struct AppState {
    pub(crate) config: SpineConfig,
    pub(crate) events: Arc<dyn EventSink>,
    pub(crate) keys: Box<dyn KeyStore>,
    pub(crate) settings: RwLock<StoredSettings>,
    settings_file: SavedFile,
    nexus_file: SavedFile,
    /// Why the mods folder was not there and could not be made at launch.
    mods_unmade: Option<String>,
    shelf: RwLock<Arc<Shelf>>,
    catalog_cache: Mutex<Arc<CatalogCache>>,
    pub(crate) last_scan_ms: Mutex<Option<u64>>,
    scan: Mutex<Option<ScanProgress>>,
    /// Signalled whenever a scan ends.
    scan_idle: Condvar,
    scan_again: AtomicBool,
    pub(crate) world: RwLock<World>,
    pub(crate) deploy_lock: Mutex<()>,
    /// Installs under way, waiting for the deploy lock or holding it: Play
    /// waits for them.
    installing: AtomicUsize,
    /// A migration that found the deploy lock taken; the change holding it
    /// runs it when it is done.
    migration_due: AtomicBool,
    /// Installs found again in the mods folder this session (`relinked`).
    relinked: AtomicUsize,
    /// v0.3: what the bytes said of each relink the scans read for
    /// ([`AppState::confirm_relinks`]), for the session. Held only for a
    /// lookup or an update, never across a read.
    relink_verdicts: Mutex<HashMap<migrate::Confirmation, bool>>,
    /// A scan is reading for the relink pass right now (one at a time: they
    /// share the scratch folder).
    relink_reading: AtomicBool,
    /// How many relink confirmations read a package this session.
    relink_reads: AtomicUsize,
    pub(crate) recovered: Mutex<Option<RecoveryNote>>,
    snapshot: Fresh<Snapshot>,
    pub(crate) paks: PakHashCache,
    pub(crate) sources: SourcePool,
    pub(crate) nexus: Mutex<NexusStore>,
    pub(crate) nexus_client: RwLock<NexusClient>,
    pub(crate) nexus_check: Mutex<()>,
    pub(crate) nexus_checking: AtomicBool,
    pub(crate) has_api_key: AtomicBool,
    /// v0.3: what the spine believes about the stored key's account.
    pub(crate) account: Mutex<AccountWord>,
    /// v0.3: the key's extras as last heard, for the session.
    pub(crate) standings: Mutex<Option<StandingsCache>>,
    /// v0.3: this session's downloads and their worker.
    pub(crate) downloads: Downloads,
    /// v0.3: nxm links that arrived before the stone could check them and
    /// say what became of them (startup step 9).
    link_door: Mutex<LinkDoor>,
    watcher: Mutex<Option<LibraryWatcher>>,
    game: Arc<GameWord>,
    game_watching: AtomicBool,
    /// v0.4: the Muster's slots, its parsed parties and the game's names.
    pub(crate) muster: crate::muster::MusterState,
}

impl AppState {
    // -- startup ------------------------------------------------------------

    /// Startup steps 1–3, on the caller's thread and fast: settings, the
    /// catalog cache, the Nexus cache, the game, its journal (a schema-1
    /// journal is lifted by rift-deploy as it loads), the mods folder — made
    /// when missing, the stone's own leftover partial downloads removed — and
    /// the recovery of a change a crash interrupted. Everything slow happens
    /// in [`AppState::wake`]. `roots.json` (v0.2) is never read, written or
    /// deleted.
    ///
    /// A file of the player's that exists but cannot be read starts that part
    /// from its defaults and is held: never written over this session.
    pub fn open(
        config: SpineConfig,
        events: Arc<dyn EventSink>,
        keys: Box<dyn KeyStore>,
    ) -> Arc<AppState> {
        let dirs = &config.dirs;
        // A relink check a quit cut short leaves its parts behind.
        clear_relink_scratch(&dirs.relink_scratch());
        let settings_loaded: Loaded<StoredSettings> = persist::load_json(&dirs.settings_file());
        let settings_file = SavedFile::new(dirs.settings_file(), settings_loaded.unreadable());
        let settings = settings_loaded.found().unwrap_or_default();
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

        let mods_unmade = make_mods_folder(&config.library_dir);
        let shelf = if mods_unmade.is_some() {
            Shelf::empty()
        } else {
            remove_partial_downloads(&config.library_dir);
            Shelf::from_cache(&config.library_dir, &cache)
        };
        // The labels nexus.json kept: the window filters by them at once.
        let shelf = {
            let labels = nexus.package_labels(shelf.packages());
            shelf.with_labels(labels)
        };
        let running = config.game_watch.running();
        let now = persist::now_ms();

        let state = Arc::new(AppState {
            events,
            keys,
            settings: RwLock::new(settings),
            settings_file,
            nexus_file,
            mods_unmade,
            shelf: RwLock::new(Arc::new(shelf)),
            catalog_cache: Mutex::new(Arc::new(cache)),
            last_scan_ms: Mutex::new(last_scan_ms),
            scan: Mutex::new(None),
            scan_idle: Condvar::new(),
            scan_again: AtomicBool::new(false),
            world: RwLock::new(World { game, journal }),
            deploy_lock: Mutex::new(()),
            installing: AtomicUsize::new(0),
            migration_due: AtomicBool::new(false),
            relinked: AtomicUsize::new(0),
            relink_verdicts: Mutex::new(HashMap::new()),
            relink_reading: AtomicBool::new(false),
            relink_reads: AtomicUsize::new(0),
            recovered: Mutex::new(None),
            snapshot: Fresh::default(),
            paks: PakHashCache::default(),
            sources: SourcePool::default(),
            nexus: Mutex::new(nexus),
            nexus_client: RwLock::new(client),
            nexus_check: Mutex::new(()),
            nexus_checking: AtomicBool::new(false),
            has_api_key: AtomicBool::new(has_api_key),
            account: Mutex::new(AccountWord::at_launch(has_api_key)),
            standings: Mutex::new(None),
            downloads: Downloads::new(now),
            link_door: Mutex::new(LinkDoor::default()),
            watcher: Mutex::new(None),
            game: Arc::new(GameWord::new(running, now)),
            game_watching: AtomicBool::new(false),
            muster: crate::muster::MusterState::default(),
            config,
        });
        state.recover_journal();
        state
    }

    /// Startup steps 4–9, in the background. The stored key's validation (no
    /// quota spent) starts first, on a thread of its own: it needs no
    /// library, so a long first scan never delays it (an nxm link's grant
    /// expires). Then the game watcher, the mods-folder watcher, the scan
    /// (and the migration after it), and the Nexus check when the last one
    /// is stale. The check is anonymous: it never waits on the key, its
    /// validation or its v1 quota.
    pub fn wake(self: &Arc<Self>) {
        self.wake_account();
        let me = Arc::clone(self);
        let spawned = std::thread::Builder::new()
            .name("riftstone-wake".into())
            .spawn(move || {
                me.watch_game();
                me.arm_watcher();
                crate::muster::wake(&me);
                // When a scan already runs (the window's, the watcher's), that
                // one is the startup scan: the auto-check waits for it, so it
                // judges the library the scan found.
                if !me.scan_blocking() {
                    me.wait_until_scanned();
                }
                crate::commands::nexus::auto_check(&me);
            });
        if let Err(err) = spawned {
            log::error!("could not start the background wake-up: {err}");
        }
    }

    /// Startup steps 7 and 9, on a thread of their own so nothing else waits
    /// on Nexus: validate the stored key, then open the link door's first
    /// half (the held nxm links go once the window listens too).
    fn wake_account(self: &Arc<Self>) {
        let me = Arc::clone(self);
        let account = move || {
            crate::account::validate_at_wake(&me);
            crate::nxm::launch_validated(&me);
        };
        let spawned = std::thread::Builder::new()
            .name("riftstone-account".into())
            .spawn(account);
        if let Err(err) = spawned {
            log::warn!("could not validate the key on a thread of its own ({err}); the held nxm links are checked without it");
            crate::nxm::launch_validated(self);
        }
    }

    // -- the nxm link door (v0.3, startup step 9) ------------------------------

    /// The nxm links of the stone's own command line (a cold start for the
    /// site's Mod Manager Download button): held at the door like every
    /// link that arrives early ([`AppState::hold_link`]). Call before
    /// [`AppState::wake`].
    pub fn queue_launch_links(&self, links: Vec<String>) {
        lock(&self.link_door).held.extend(links);
    }

    /// The links the door holds, taken (none are handled for them).
    pub fn take_launch_links(&self) -> Vec<String> {
        std::mem::take(&mut lock(&self.link_door).held)
    }

    /// Hold a link that arrived (a second launch's) while the door is shut:
    /// `true` when it is held, `false` when the door is open and the caller
    /// handles it now.
    pub fn hold_link(&self, link: &str) -> bool {
        let mut door = lock(&self.link_door);
        if door.open() {
            return false;
        }
        door.held.push(link.to_string());
        true
    }

    /// The launch's key validation is done (or could not run): the held
    /// links to handle now, when the window listens already.
    pub fn launch_validated(&self) -> Vec<String> {
        let mut door = lock(&self.link_door);
        door.validated = true;
        door.take_when_open()
    }

    /// The window read its first frame (`get_overview`, which
    /// `src/composables/boot.ts` asks only once it listens to every event):
    /// the held links to handle now, when the launch validation is done too.
    pub fn window_listens(&self) -> Vec<String> {
        let mut door = lock(&self.link_door);
        if door.listening {
            return Vec::new();
        }
        door.listening = true;
        door.take_when_open()
    }

    // -- the journal ----------------------------------------------------------

    /// One turn at the game folder: the deploy lock, held while the [`Turn`]
    /// lives. One writer in the game folder at a time.
    pub fn turn(&self) -> Turn<'_> {
        Turn {
            state: self,
            guard: Some(lock(&self.deploy_lock)),
        }
    }

    /// [`AppState::turn`], unless a change holds the game folder right now.
    pub fn try_turn(&self) -> Option<Turn<'_>> {
        let guard = match self.deploy_lock.try_lock() {
            Ok(guard) => guard,
            Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
            Err(TryLockError::WouldBlock) => return None,
        };
        Some(Turn {
            state: self,
            guard: Some(guard),
        })
    }

    /// Roll back an operation a crash left in the journal (startup step 2).
    pub fn recover_journal(&self) {
        let _turn = self.turn();
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
    /// its journal, recover it, remember the choice, and find its installs
    /// in the mods folder again.
    pub fn switch_game(&self, game: GameInstall) {
        {
            let _turn = self.turn();
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
            self.migrate_guarded();
        }
        // This journal's relinks that its bytes must confirm: read with the
        // game folder let go, then the migration once more with the verdicts.
        self.confirm_relinks();
        self.migrate();
    }

    // -- settings ------------------------------------------------------------

    /// Persist the settings as they stand now (never under the settings lock).
    pub fn save_settings(&self) {
        self.settings_file
            .save(true, || read(&self.settings).clone());
    }

    /// Persist the Nexus store as it stands now (never under its lock).
    pub fn save_nexus(&self) {
        self.nexus_file.save(true, || lock(&self.nexus).clone());
    }

    /// [`AppState::save_nexus`], reporting a failed write instead of logging it.
    pub fn try_save_nexus(&self) -> Result<(), RiftError> {
        self.nexus_file.try_save(true, || lock(&self.nexus).clone())
    }

    /// The player's files that could not be read at launch: each starts from
    /// its defaults and is not written this session.
    pub fn held_files(&self) -> Vec<HeldFile> {
        [
            ("settings", &self.settings_file),
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

    // -- the mods folder -------------------------------------------------------

    /// The one library.
    pub fn library_dir(&self) -> &Path {
        &self.config.library_dir
    }

    /// Why the mods folder was not there and could not be made at launch.
    pub fn mods_unmade(&self) -> Option<&str> {
        self.mods_unmade.as_deref()
    }

    /// (Re-)arm the mods-folder watcher, replacing the one there was. A
    /// folder that is not there is not watched: Scan arms it once the folder
    /// is there again, and `open_mods_folder` once it makes it.
    pub fn arm_watcher(self: &Arc<Self>) {
        if !self.config.watch_library {
            return;
        }
        let own = [
            self.config.dirs.data.clone(),
            self.config.dirs.cache.clone(),
        ];
        let weak = Arc::downgrade(self);
        let armed = watch::arm(&self.config.library_dir, &own, watch::DEBOUNCE, move || {
            if let Some(state) = weak.upgrade() {
                state.request_scan();
            }
        });
        *lock(&self.watcher) = armed;
    }

    /// Is the mods-folder watcher armed?
    pub fn watching_library(&self) -> bool {
        lock(&self.watcher).is_some()
    }

    /// Should the mods folder be watched, and nothing watches it (it was
    /// missing at the wake, a scan found it gone, or the platform watcher
    /// would not start)?
    pub fn library_unwatched(&self) -> bool {
        self.config.watch_library && !self.watching_library()
    }

    /// A scan found the mods folder gone: let its watcher go. A folder made
    /// again is a new folder, which that watcher never sees, so the next
    /// Scan or Open mods folder arms one afresh ([`AppState::library_unwatched`]).
    /// Decided under the watcher's lock, so a watcher armed on a folder made
    /// again meanwhile (it is made before it is armed) is never the one let go.
    fn unwatch_if_gone(&self) {
        let gone = {
            let mut watcher = lock(&self.watcher);
            if watcher.is_some() && !self.config.library_dir.is_dir() {
                watcher.take()
            } else {
                None
            }
        };
        if gone.is_some() {
            log::info!(
                "the mods folder {} is gone, so it is no longer watched; Scan or Open mods folder watches it again once it is back",
                self.config.library_dir.display()
            );
        }
        drop(gone);
    }

    // -- the library -----------------------------------------------------------

    pub fn shelf(&self) -> Arc<Shelf> {
        Arc::clone(&read(&self.shelf))
    }

    fn catalog_cache_snapshot(&self) -> Arc<CatalogCache> {
        Arc::clone(&lock(&self.catalog_cache))
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
        // v0.5: a link or a check can tie versions of one file or untie them, and the
        // snapshot's conflicts read the families: the next one is built afresh.
        self.invalidate_snapshot();
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

    /// One pass: a fresh catalog of the mods folder (cache-backed), swapped
    /// in whole; the cache persisted; then the migration.
    fn scan_once(&self) {
        let library = self.config.library_dir.clone();
        let cache = self.catalog_cache_snapshot();
        let events = &self.events;
        let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
            Catalog::scan(&library, &cache, &mut |progress: ScanProgress| {
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
        self.unwatch_if_gone();
        match outcome {
            Ok((catalog, fresh)) => {
                self.shelve(Shelf::scanned(catalog));
                // Written only when it changed: the watcher rescans after every
                // change in the mods folder, and most leave the library as it was.
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
                self.confirm_relinks();
                self.migrate();
            }
            Err(_) => log::error!(
                "the library scan stopped unexpectedly; the library stays as it was before it"
            ),
        }
    }

    // -- the migration (v0.3) ---------------------------------------------------

    /// After every scan: give the journal and `nexus.json` the v0.3 ids of
    /// what they knew under older ones (docs/WINDOW-CONTRACT.md § v0.3 Legacy
    /// data). It needs the deploy lock; when a change holds it, the migration
    /// runs as soon as that change lets go of the lock ([`Turn`]).
    pub fn migrate(&self) {
        // Marked before the lock is tried: a turn that ends between the try
        // and the mark still sees it. A scan that asks again while this pass
        // runs gets one more pass.
        self.migration_due.store(true, Ordering::SeqCst);
        while self.migration_due.load(Ordering::SeqCst) {
            let guard = match self.deploy_lock.try_lock() {
                Ok(guard) => guard,
                Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
                Err(TryLockError::WouldBlock) => return,
            };
            self.migrate_guarded();
            drop(guard);
        }
    }

    /// The migration with the deploy lock held; one that stops unexpectedly
    /// is logged and leaves the ids as they were (the next scan tries again).
    fn migrate_guarded(&self) {
        self.migration_due.store(false, Ordering::SeqCst);
        if panic::catch_unwind(AssertUnwindSafe(|| self.migrate_locked())).is_err() {
            log::error!("finding moved installs stopped unexpectedly; their ids stay as they were until the next scan");
        }
    }

    /// (1) Every folder package's v0.2 id becomes its v0.3 id, exactly; (2)
    /// installs whose package is not in the mods folder are matched to a
    /// library option (rift-deploy's relink pass), each one whose install
    /// recorded fingerprints only once its bytes confirmed it (the verdicts
    /// [`AppState::confirm_relinks`] read; one with none yet waits, and
    /// nothing is read here). One journal save, one `nexus.json` save (a
    /// link record follows its package), and the window hears what changed.
    /// Never while a change is pending.
    fn migrate_locked(&self) {
        let shelf = self.shelf();
        let mut moves = migrate::folder_moves(shelf.packages());
        let mut journal_changed = false;
        let world = self.world();
        if let (Some(_), Some(journal)) = (&world.game, world.journal.ready()) {
            if journal.pending.is_none() {
                let known = lock(&self.relink_verdicts).clone();
                let plan =
                    migrate::plan_rekey(journal, &moves, shelf.packages(), &|confirmation| {
                        known.get(confirmation).copied()
                    });
                if !plan.map.is_empty() {
                    match self.deploy_unlocked(|ctx, journal| {
                        rift_deploy::rekey_with_files(ctx, journal, &plan.map, &plan.files)
                    }) {
                        Ok(rekeyed) => {
                            if rekeyed > 0 {
                                log::info!(
                                    "{rekeyed} install(s) found again in the mods folder after a move"
                                );
                                self.relinked.fetch_add(rekeyed, Ordering::SeqCst);
                                journal_changed = true;
                            }
                            for (from, to) in plan.package_moves {
                                moves.entry(from).or_insert(to);
                            }
                        }
                        Err(err) => log::warn!(
                            "installs whose mods moved keep their old ids for now: {err}"
                        ),
                    }
                }
            }
        }
        let nexus_changed = lock(&self.nexus).move_records(&moves);
        drop(shelf);
        if nexus_changed {
            self.save_nexus();
            self.relabel();
            self.events.changed(events::UPDATES_CHANGED);
        }
        if journal_changed {
            self.events.changed(events::GAME_CHANGED);
        }
    }

    /// Installs found again in the mods folder this session.
    pub fn relinked_count(&self) -> usize {
        self.relinked.load(Ordering::SeqCst)
    }

    // -- the relink's bytes (v0.3) ---------------------------------------------

    /// Before the migration, with no lock held: read what the relink pass
    /// needs confirmed. The sizes propose; for every proposal whose install
    /// recorded fingerprints and that has no verdict yet, the candidate's
    /// package is opened as an install opens it (`rift_archive::open`), the
    /// entries the check names are unpacked into the scratch folder
    /// ([`DataDirs::relink_scratch`]) and fingerprinted with the engine's own
    /// fingerprint (rift-deploy's [`rift_deploy::relink::read_fingerprints`]),
    /// and the verdict is kept for the session. The migration applies only
    /// what the bytes confirmed; it never reads.
    ///
    /// Bounded: a scan unpacks at most [`migrate::RELINK_READ_BUDGET`] bytes
    /// (as the packages list them). What does not fit waits for the next
    /// scan; a check that needs more than the whole budget on its own is
    /// never read, and its install keeps its id. A package that cannot be
    /// read confirms nothing. One reader at a time: a second one finds the
    /// first one's verdicts at its next scan. While a scan runs, its progress
    /// names each package read (`checking <file> against the installed files
    /// (i of n)`): the scan has not ended until the reading has.
    pub fn confirm_relinks(&self) {
        if panic::catch_unwind(AssertUnwindSafe(|| self.confirm_relinks_now())).is_err() {
            log::error!("reading what the relink pass asked stopped unexpectedly; those installs keep their ids until the next scan");
        }
    }

    fn confirm_relinks_now(&self) {
        let shelf = self.shelf();
        let world = self.world();
        let (Some(_), Some(journal)) = (&world.game, world.journal.ready()) else {
            return;
        };
        if journal.pending.is_some() {
            return;
        }
        let moves = migrate::folder_moves(shelf.packages());
        let known = lock(&self.relink_verdicts).clone();
        let plan = migrate::plan_rekey(journal, &moves, shelf.packages(), &|confirmation| {
            known.get(confirmation).copied()
        });
        let mut verdicts: HashMap<migrate::Confirmation, bool> = HashMap::new();
        let reading = if plan.unconfirmed.is_empty() {
            None
        } else {
            ScratchReader::begin(&self.relink_reading)
        };
        if let Some(_reader) = reading {
            let scratch = self.config.dirs.relink_scratch();
            let (read, too_large) =
                migrate::within_budget(&plan.unconfirmed, migrate::RELINK_READ_BUDGET);
            for confirmation in too_large {
                log::warn!(
                    "{} keeps its id: confirming {} would unpack {} bytes of {}, more than a scan reads",
                    confirmation.check.install(),
                    confirmation.check.option(),
                    confirmation.check.bytes(),
                    confirmation.package.display()
                );
                verdicts.insert(confirmation.clone(), false);
            }
            let reads = read
                .iter()
                .filter(|confirmation| confirmation.reads())
                .count();
            let mut reading = 0;
            for confirmation in read {
                if confirmation.reads() {
                    reading += 1;
                    self.tell_relink_read(confirmation, reading, reads);
                }
                verdicts.insert(
                    confirmation.clone(),
                    self.read_confirmation(confirmation, &scratch),
                );
            }
            clear_relink_scratch(&scratch);
        }
        // The verdicts the journal still asks for, and the new ones.
        let asked: HashSet<&migrate::Confirmation> = plan.asked.iter().collect();
        let mut memo = lock(&self.relink_verdicts);
        memo.retain(|confirmation, _| asked.contains(confirmation));
        memo.extend(verdicts);
    }

    /// While a scan runs, its progress names the confirmation about to be
    /// read (`reading` of the `reads` this pass reads; the scan's counts
    /// stand), and the window hears it as it hears any package: the
    /// unpacking can take a while, and the scan has not ended. When no scan
    /// runs (a game switch's reading), no progress is touched.
    fn tell_relink_read(&self, confirmation: &migrate::Confirmation, reading: usize, reads: usize) {
        let told = {
            let mut scan = lock(&self.scan);
            let Some(progress) = scan.as_mut() else {
                return;
            };
            progress.current = format!(
                "checking {} against the installed files ({reading} of {reads})",
                rift_catalog::display_file_name(&confirmation.package)
            );
            ScanProgressEvent {
                done: progress.done,
                total: progress.total,
                current: progress.current.clone(),
            }
        };
        self.events.emit(events::SCAN_PROGRESS, &told);
    }

    /// Read one confirmation's entries: do they carry the fingerprints the
    /// install recorded? `false` when the package cannot be opened or read,
    /// or the parts cannot be written (logged).
    fn read_confirmation(&self, confirmation: &migrate::Confirmation, scratch: &Path) -> bool {
        let check = &confirmation.check;
        if !confirmation.reads() {
            return check.confirms(&HashMap::new());
        }
        self.relink_reads.fetch_add(1, Ordering::SeqCst);
        let read = || -> Result<HashMap<String, String>, RiftError> {
            clear_relink_scratch(scratch);
            fs::create_dir_all(scratch).map_err(|err| {
                RiftError::io(format!("could not make {}", scratch.display()), err)
            })?;
            let mut source = rift_archive::open(&confirmation.package)?;
            rift_deploy::relink::read_fingerprints(check, source.as_mut(), scratch)
        };
        match panic::catch_unwind(AssertUnwindSafe(read)) {
            Ok(Ok(prints)) => {
                let same = check.confirms(&prints);
                if !same {
                    log::info!(
                        "{} keeps its id: {} holds other bytes than the ones it installed",
                        check.install(),
                        confirmation.package.display()
                    );
                }
                same
            }
            Ok(Err(err)) => {
                log::warn!(
                    "{} keeps its id: {} could not be read to confirm the match ({err})",
                    check.install(),
                    confirmation.package.display()
                );
                false
            }
            Err(_) => {
                log::error!(
                    "reading {} to confirm a relink stopped unexpectedly",
                    confirmation.package.display()
                );
                false
            }
        }
    }

    /// How many relink confirmations read a package this session.
    pub fn relink_checks_read(&self) -> usize {
        self.relink_reads.load(Ordering::SeqCst)
    }

    // -- the game, believed ---------------------------------------------------

    /// Start the game watcher (startup step 6); once per spine.
    pub fn watch_game(self: &Arc<Self>) {
        if self.game_watching.swap(true, Ordering::SeqCst) {
            return;
        }
        let weak = Arc::downgrade(self);
        let started = crate::play::spawn_watcher(
            Arc::clone(&self.game),
            self.config.game_cadence,
            move || match weak.upgrade() {
                Some(state) => {
                    state.sample_game();
                    true
                }
                None => false,
            },
        );
        if !started {
            self.game_watching.store(false, Ordering::SeqCst);
        }
    }

    /// Ask the watch once. A start or a stop changes the spine's word on the
    /// game, reads the game folder again and tells the window
    /// (`rift://game-changed`); `true` then.
    pub fn sample_game(&self) -> bool {
        self.apply_sample(self.config.game_watch.running())
    }

    fn apply_sample(&self, running: bool) -> bool {
        let flipped = self.game.sample(running, persist::now_ms());
        if flipped {
            if running {
                log::info!("Dragon's Dogma 2 started");
            } else {
                log::info!("Dragon's Dogma 2 stopped");
            }
            self.invalidate_snapshot();
            self.events.changed(events::GAME_CHANGED);
        }
        flipped
    }

    /// Ask the watch now (a change to the game folder, Play): whether the
    /// game runs, with any start or stop told as the watcher would.
    pub fn observe_game(&self) -> bool {
        let running = self.config.game_watch.running();
        self.apply_sample(running);
        running
    }

    /// The game as the spine believes it now (`GameSummary.running`,
    /// `startedMs`).
    pub fn game_run(&self) -> GameRun {
        self.game.run()
    }

    /// Play was pressed: the watcher samples faster for a while.
    pub fn game_launched(&self) {
        self.game.launched(Instant::now());
    }

    /// Is a change to the game folder being made right now (the deploy lock
    /// is held, or an install waits for it)?
    ///
    /// The lock is probed through a [`Turn`], never a bare guard: a scan's
    /// migration that meets the probe's moment is put off, and the turn runs
    /// it as it ends, exactly as a change's turn would. (A bare guard let it
    /// wait for the next change or scan, and a v0.2 install could meet a new
    /// install of its option under the v0.3 id in between.)
    pub fn deploy_busy(&self) -> bool {
        self.installs_under_way() || self.try_turn().is_none()
    }

    /// Is an install under way, waiting for the deploy lock or holding it?
    /// (What "busy" means to a caller holding the lock itself.)
    pub fn installs_under_way(&self) -> bool {
        self.installing.load(Ordering::SeqCst) > 0
    }

    /// Mark an install under way for as long as the guard lives.
    pub fn begin_install(&self) -> Installing<'_> {
        self.installing.fetch_add(1, Ordering::SeqCst);
        Installing(&self.installing)
    }

    // -- the game folder, derived ---------------------------------------------

    /// The game and its journal as they stand — an owned copy, so the caller
    /// reads the disk with no lock held.
    /// v0.4: read exactly these save folders (tests and benches), nothing discovered.
    pub fn fix_save_folders(&self, folders: Vec<rift_muster::saves::SaveFolder>) {
        self.muster.fix_folders(folders);
    }

    pub fn world(&self) -> World {
        read(&self.world).clone()
    }

    /// Something the snapshot is made of changed (an install, a scan, a
    /// switch, the game starting): the next ask reads the game folder again.
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
    /// did not make itself (another tool, REFramework copied in) appear too.
    pub fn refresh_snapshot(&self) -> Arc<Snapshot> {
        self.snapshot.get_recent(|| self.compute_snapshot())
    }

    /// How many times the game folder has been read for a snapshot.
    pub fn snapshot_builds(&self) -> u64 {
        self.snapshot.builds()
    }

    /// The snapshot reads the Nexus store (v0.5, for the families), so nothing asks for one
    /// while holding the store's lock.
    fn compute_snapshot(&self) -> Snapshot {
        let world = self.world();
        let running = self.game.run().running;
        let mut snapshot = Snapshot {
            running,
            ..Snapshot::default()
        };
        if let Some(game) = &world.game {
            let ref_status = rift_deploy::read_status(&game.root);
            if let Some(journal) = world.journal.ready() {
                let ctx = DeployContext {
                    game,
                    state_dir: self.config.dirs.state_dir(),
                    ref_status: &ref_status,
                    game_running: running,
                };
                let game_state = rift_deploy::read_game_state(&ctx, journal);
                let layers = conflicts::game_layers(&game.root, journal, &game_state, &self.paks);
                // v0.5: versions of one file stacked in your world (installed before v0.5
                // replaced them, or before a link proved them one file) override each other
                // by design. That is an older version still installed, which the family's own
                // mark tells (WINDOW-CONTRACT § v0.5): no conflict.
                let families = Families::of_shelf(&self.shelf(), &lock(&self.nexus));
                snapshot.conflicts = conflicts::detect(&layers)
                    .into_iter()
                    .filter(|conflict| {
                        let packages: Vec<PackageId> = conflict
                            .layers
                            .iter()
                            .map(|layer| OptionId(layer.option_id.clone()).package())
                            .collect();
                        !families.one_family(&packages)
                    })
                    .collect();
                snapshot.conflicted = conflicts::conflicted_ids(&snapshot.conflicts);
                snapshot.game_state = Some(game_state);
            }
            snapshot.ref_status = Some(ref_status);
        }
        snapshot
    }

    // -- deploying ---------------------------------------------------------------

    fn scene(&self, world: &World) -> Result<Scene, RiftError> {
        let game = world.game.clone().ok_or(RiftError::GameNotFound)?;
        Ok(Scene {
            ref_status: rift_deploy::read_status(&game.root),
            running: self.observe_game(),
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
        let scene = self.scene(&world)?;
        Ok(op(&scene.ctx(self.config.dirs.state_dir()), &journal))
    }

    /// Run one journal-changing operation: one at a time, against a copy of
    /// the journal that replaces the original when the engine is done (the
    /// engine saved its intent and its commit to disk itself, success or not).
    /// A migration a scan put off while this change held the lock runs right
    /// after it ([`Turn`]).
    pub fn deploy<R>(
        &self,
        op: impl FnOnce(&DeployContext, &mut Journal) -> Result<R, RiftError>,
    ) -> Result<R, RiftError> {
        let _turn = self.turn();
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
        let scene = self.scene(&world)?;
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

/// The door nxm links pass (startup step 9). A link that arrives before the
/// launch's key validation is done would be checked against no account (a
/// link made for another one would pass), and one whose outcome is told
/// before the window listens is never heard (no refusal said, no move to THE
/// FAR WORLD). So every link waits at the door until both have happened:
/// the cold start's own, and a second launch's that came early.
///
/// No `Debug`: a held link carries the site's grant, a secret.
#[derive(Default)]
struct LinkDoor {
    /// Waiting, in the order they arrived.
    held: Vec<String>,
    /// The launch's key validation has run (whatever it found).
    validated: bool,
    /// The window has read its first frame: it hears `rift://nxm-received`.
    listening: bool,
}

impl LinkDoor {
    fn open(&self) -> bool {
        self.validated && self.listening
    }

    /// Everything held, when the door is open; nothing otherwise.
    fn take_when_open(&mut self) -> Vec<String> {
        if self.open() {
            std::mem::take(&mut self.held)
        } else {
            Vec::new()
        }
    }
}

/// A turn at the game folder ([`AppState::turn`]). When it ends, the lock is
/// let go first; then a migration a scan put off meanwhile runs
/// ([`AppState::migrate`]) — unless the turn ends in a panic, which the next
/// scan's migration outlives.
pub struct Turn<'a> {
    state: &'a AppState,
    guard: Option<MutexGuard<'a, ()>>,
}

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        drop(self.guard.take());
        if !std::thread::panicking() && self.state.migration_due.load(Ordering::SeqCst) {
            self.state.migrate();
        }
    }
}

/// An install under way ([`AppState::begin_install`]); dropping it ends it.
pub struct Installing<'a>(&'a AtomicUsize);

impl Drop for Installing<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

/// The relink scratch folder's one reader ([`AppState::confirm_relinks`]);
/// dropping it, an unwinding panic too, lets the next one read.
struct ScratchReader<'a>(&'a AtomicBool);

impl ScratchReader<'_> {
    /// The reader's turn, unless another reader holds it.
    fn begin(flag: &AtomicBool) -> Option<ScratchReader<'_>> {
        (!flag.swap(true, Ordering::SeqCst)).then_some(ScratchReader(flag))
    }
}

impl Drop for ScratchReader<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Find the game: the fixed folder (tests), or `RIFTSTONE_GAME_ROOT`, then a
/// hand-picked folder, then Steam.
fn find_game(discovery: &Discovery, manual: Option<&Path>) -> Option<GameInstall> {
    match discovery {
        Discovery::Fixed { game_root } => game_root
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

/// Make the mods folder when it is missing (startup step 3): `Some(why)`
/// when it is not there and could not be made. Never a panic: the stone opens
/// with an empty library and the `mods` check says what happened.
fn make_mods_folder(folder: &Path) -> Option<String> {
    match fs::create_dir_all(folder) {
        Ok(()) if folder.is_dir() => None,
        Ok(()) => Some("something that is not a folder has its name".into()),
        Err(err) => {
            log::error!(
                "the mods folder {} could not be made: {err}",
                folder.display()
            );
            Some(err.to_string())
        }
    }
}

/// The name the stone gives a download while it is under way
/// (`.riftstone-<id>.part`).
pub fn is_partial_download(name: &str) -> bool {
    name.len() > ".riftstone-.part".len()
        && name.starts_with(".riftstone-")
        && name.ends_with(".part")
}

/// Empty the relink's scratch folder ([`DataDirs::relink_scratch`]), the
/// folder itself included. A link at its name goes as a link, never
/// followed. A folder that is not there is fine.
fn clear_relink_scratch(scratch: &Path) {
    match fs::remove_dir_all(scratch) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => log::warn!("could not empty {}: {err}", scratch.display()),
    }
}

/// Remove the stone's own partial downloads a quit cut short: files directly
/// in the mods folder named `.riftstone-<id>.part`, and nothing else (RS-2).
/// A link or a folder of that name is left alone.
fn remove_partial_downloads(folder: &Path) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if !is_partial_download(&name.to_string_lossy()) {
            continue;
        }
        if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
            continue;
        }
        match fs::remove_file(entry.path()) {
            Ok(()) => log::info!(
                "removed a download a quit cut short ({})",
                name.to_string_lossy()
            ),
            Err(err) => log::warn!(
                "could not remove the partial download {}: {err}",
                entry.path().display()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mods_folder_sits_beside_the_exe_unless_the_bench_moves_it() {
        let exe = Path::new("D:/Vernworth/The Riftstone/riftstone.exe");
        assert_eq!(
            mods_dir_from(exe, None),
            PathBuf::from("D:/Vernworth/The Riftstone/mods")
        );
        assert_eq!(
            mods_dir_from(exe, Some(OsString::new())),
            PathBuf::from("D:/Vernworth/The Riftstone/mods"),
            "an empty override is no override"
        );
        assert_eq!(
            mods_dir_from(exe, Some(OsString::from("/bench/mods"))),
            PathBuf::from("/bench/mods")
        );
        assert_eq!(
            mods_dir_from(Path::new("riftstone.exe"), None),
            PathBuf::from("mods"),
            "an exe with no folder: beside it, where the stone runs"
        );
    }

    #[test]
    fn only_the_stones_own_partial_names_count_as_partial_downloads() {
        assert!(is_partial_download(".riftstone-3f2a.part"));
        assert!(!is_partial_download(".riftstone-.part"));
        assert!(!is_partial_download("riftstone-3f2a.part"));
        assert!(!is_partial_download(".riftstone-3f2a.part.zip"));
        assert!(!is_partial_download("Lantern Glow Recolor.zip.part"));
    }

    #[test]
    fn a_missing_mods_folder_is_made_and_one_that_cannot_be_says_why() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("The Riftstone").join("mods");
        assert_eq!(make_mods_folder(&mods), None);
        assert!(mods.is_dir());
        assert_eq!(make_mods_folder(&mods), None, "an existing folder is fine");
        let blocked = dir.path().join("not a folder");
        fs::write(&blocked, b"a file").unwrap();
        let why = make_mods_folder(&blocked.join("mods")).expect("cannot be made");
        assert!(!why.is_empty());
    }

    #[test]
    fn leftover_partial_downloads_go_and_nothing_else_does() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path();
        fs::write(mods.join(".riftstone-a1.part"), b"half").unwrap();
        fs::write(mods.join("Lantern Glow Recolor.zip"), b"PK").unwrap();
        fs::write(mods.join("Vernworth Cloaks.zip.part"), b"a browser's").unwrap();
        fs::create_dir_all(mods.join(".riftstone-b2.part")).unwrap();
        fs::create_dir_all(mods.join("Pawn Banter Quieter")).unwrap();
        fs::write(
            mods.join("Pawn Banter Quieter").join(".riftstone-c3.part"),
            b"not directly in the mods folder",
        )
        .unwrap();
        remove_partial_downloads(mods);
        let mut left: Vec<String> = fs::read_dir(mods)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(
            left,
            vec![
                ".riftstone-b2.part",
                "Lantern Glow Recolor.zip",
                "Pawn Banter Quieter",
                "Vernworth Cloaks.zip.part"
            ]
        );
        assert!(mods
            .join("Pawn Banter Quieter")
            .join(".riftstone-c3.part")
            .is_file());
    }
}
