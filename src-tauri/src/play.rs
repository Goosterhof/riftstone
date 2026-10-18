//! Play (v0.3): the spine's two seams onto the desktop, and the game watcher.
//!
//! - [`GameLauncher`] is the spine's one hand on the desktop: it starts the
//!   game the way rift-deploy's plan says (Steam's `steam://rungameid/…` link
//!   through the shell, or `DD2.exe` with the game folder as its working
//!   directory) and shows the mods folder in the file manager. The app hands
//!   the spine [`SystemLauncher`]; the tests hand it a [`RecordingLauncher`]
//!   and read back what would have started, so no test ever opens a window
//!   or starts a program.
//! - [`GameWatch`] says whether `DD2.exe` runs. The app's [`SystemGameWatch`]
//!   reads the Windows process list (always `false` elsewhere); the tests
//!   flip a [`SwitchedGameWatch`].
//! - [`GameWord`] is what the spine believes about the game: running or not,
//!   and since when. The `riftstone-game` thread samples the watch every
//!   [`GameCadence::every`] (2 s), and every [`GameCadence::after_launch`]
//!   (1 s) for [`GameCadence::after_launch_for`] (120 s) after Play. Only a
//!   start or a stop (a flip) changes the word; the spine then reads the game
//!   folder again and the window hears `rift://game-changed` once.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use rift_core::RiftError;
use rift_deploy::LaunchRoute;

// ---------------------------------------------------------------------------
// The launcher
// ---------------------------------------------------------------------------

/// How the spine starts the game and shows a folder.
pub trait GameLauncher: Send + Sync {
    /// Start the game by `route`. It returns once Steam or Windows took the
    /// request; the game watcher sees the game itself start.
    fn launch(&self, route: &LaunchRoute) -> Result<(), RiftError>;
    /// Show `folder` in the file manager (`open_mods_folder`).
    fn show_folder(&self, folder: &Path) -> Result<(), RiftError>;
}

/// The desktop itself: the shell opens the Steam link and the folder,
/// rift-deploy starts `DD2.exe`.
pub struct SystemLauncher;

impl GameLauncher for SystemLauncher {
    fn launch(&self, route: &LaunchRoute) -> Result<(), RiftError> {
        match route {
            LaunchRoute::Steam { url } => {
                tauri_plugin_opener::open_url(url, None::<&str>).map_err(|err| {
                    log::warn!("the shell did not take the Steam link: {err}");
                    RiftError::Io {
                        detail: "Windows did not pass the start request on to Steam. Start Steam, then press Play again.".into(),
                    }
                })
            }
            LaunchRoute::Exe { exe, cwd } => rift_deploy::start_exe(exe, cwd),
        }
    }

    fn show_folder(&self, folder: &Path) -> Result<(), RiftError> {
        tauri_plugin_opener::open_path(folder, None::<&str>)
            .map_err(|err| RiftError::io(format!("could not show {}", folder.display()), err))
    }
}

/// What a [`RecordingLauncher`] was asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Launched {
    Game(LaunchRoute),
    Folder(PathBuf),
}

/// The tests' desktop: it remembers every request and starts nothing. Made
/// with [`RecordingLauncher::starting`], a launch also flips the tests' game
/// watch to running, the way the real game would appear in the process list.
#[derive(Default)]
pub struct RecordingLauncher {
    seen: Mutex<Vec<Launched>>,
    starts: Option<Arc<SwitchedGameWatch>>,
}

impl RecordingLauncher {
    /// A launcher whose launches make `watch` say the game runs.
    pub fn starting(watch: Arc<SwitchedGameWatch>) -> RecordingLauncher {
        RecordingLauncher {
            seen: Mutex::new(Vec::new()),
            starts: Some(watch),
        }
    }

    /// Every request so far, oldest first.
    pub fn seen(&self) -> Vec<Launched> {
        lock(&self.seen).clone()
    }
}

impl GameLauncher for RecordingLauncher {
    fn launch(&self, route: &LaunchRoute) -> Result<(), RiftError> {
        lock(&self.seen).push(Launched::Game(route.clone()));
        if let Some(watch) = &self.starts {
            watch.set(true);
        }
        Ok(())
    }

    fn show_folder(&self, folder: &Path) -> Result<(), RiftError> {
        lock(&self.seen).push(Launched::Folder(folder.to_path_buf()));
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// The watch
// ---------------------------------------------------------------------------

/// Whether Dragon's Dogma 2 runs right now.
pub trait GameWatch: Send + Sync {
    fn running(&self) -> bool;
}

/// `DD2.exe` in the Windows process list (rift-deploy; `false` elsewhere).
pub struct SystemGameWatch;

impl GameWatch for SystemGameWatch {
    fn running(&self) -> bool {
        rift_deploy::is_game_running()
    }
}

/// The tests' process list: the game runs when the test says so. It counts
/// how often it was asked.
#[derive(Default)]
pub struct SwitchedGameWatch {
    running: AtomicBool,
    asked: AtomicUsize,
}

impl SwitchedGameWatch {
    pub fn set(&self, running: bool) {
        self.running.store(running, Ordering::SeqCst);
    }

    /// How many times the spine asked.
    pub fn asked(&self) -> usize {
        self.asked.load(Ordering::SeqCst)
    }
}

impl GameWatch for SwitchedGameWatch {
    fn running(&self) -> bool {
        self.asked.fetch_add(1, Ordering::SeqCst);
        self.running.load(Ordering::SeqCst)
    }
}

// ---------------------------------------------------------------------------
// What the spine believes about the game
// ---------------------------------------------------------------------------

/// How often the game watcher samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameCadence {
    /// The usual wait between two samples (2 s).
    pub every: Duration,
    /// The wait right after Play (1 s)…
    pub after_launch: Duration,
    /// …for this long after it (120 s).
    pub after_launch_for: Duration,
}

impl Default for GameCadence {
    fn default() -> Self {
        GameCadence {
            every: Duration::from_secs(2),
            after_launch: Duration::from_secs(1),
            after_launch_for: Duration::from_secs(120),
        }
    }
}

impl GameCadence {
    /// The wait before the next sample, `since_launch` after the last Play
    /// (`None` when Play was not pressed this session).
    pub fn next_wait(&self, since_launch: Option<Duration>) -> Duration {
        match since_launch {
            Some(since) if since < self.after_launch_for => self.after_launch,
            _ => self.every,
        }
    }
}

/// The game as the spine last saw it (`GameSummary.running`, `startedMs`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GameRun {
    pub running: bool,
    /// When the spine first saw this run, ms since the epoch; `None` while
    /// the game does not run.
    pub started_ms: Option<u64>,
}

struct Word {
    run: GameRun,
    /// When Play was last pressed.
    launched: Option<Instant>,
}

/// What the spine believes about the game, shared with the watcher thread.
pub struct GameWord {
    word: Mutex<Word>,
    /// Rings when Play is pressed, so the watcher speeds up at once.
    rung: Condvar,
}

impl GameWord {
    /// The word at launch: what the first sample said.
    pub fn new(running: bool, now_ms: u64) -> GameWord {
        GameWord {
            word: Mutex::new(Word {
                run: GameRun {
                    running,
                    started_ms: running.then_some(now_ms),
                },
                launched: None,
            }),
            rung: Condvar::new(),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Word> {
        lock(&self.word)
    }

    pub fn run(&self) -> GameRun {
        self.lock().run
    }

    /// Take a sample. `true` when it flips the word (a start or a stop).
    pub fn sample(&self, running: bool, now_ms: u64) -> bool {
        let mut word = self.lock();
        if word.run.running == running {
            return false;
        }
        word.run = GameRun {
            running,
            started_ms: running.then_some(now_ms),
        };
        true
    }

    /// Play was pressed: sample faster for a while, starting now.
    pub fn launched(&self, at: Instant) {
        self.lock().launched = Some(at);
        self.rung.notify_all();
    }

    /// Wait until the next sample is due (or Play is pressed).
    pub fn wait(&self, cadence: GameCadence) {
        let word = self.lock();
        let since = word.launched.map(|at| at.elapsed());
        let wait = cadence.next_wait(since);
        let _ = self
            .rung
            .wait_timeout(word, wait)
            .unwrap_or_else(PoisonError::into_inner);
    }
}

/// Start the `riftstone-game` thread: wait, then `sample`, for as long as
/// `sample` answers `true` (it answers `false` once the spine is gone).
pub fn spawn_watcher(
    word: Arc<GameWord>,
    cadence: GameCadence,
    sample: impl Fn() -> bool + Send + 'static,
) -> bool {
    let spawned = std::thread::Builder::new()
        .name("riftstone-game".into())
        .spawn(move || loop {
            word.wait(cadence);
            if !sample() {
                return;
            }
        });
    match spawned {
        Ok(_) => true,
        Err(err) => {
            log::warn!("the game watcher could not start ({err}); the overview reads the game on each visit instead");
            false
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cadence_quickens_for_two_minutes_after_play() {
        let cadence = GameCadence::default();
        assert_eq!(cadence.next_wait(None), Duration::from_secs(2));
        assert_eq!(
            cadence.next_wait(Some(Duration::from_secs(5))),
            Duration::from_secs(1)
        );
        assert_eq!(
            cadence.next_wait(Some(Duration::from_secs(120))),
            Duration::from_secs(2),
            "after 120 s the usual pace returns"
        );
    }

    #[test]
    fn only_a_start_or_a_stop_changes_the_word() {
        let word = GameWord::new(false, 0);
        assert_eq!(word.run(), GameRun::default());
        assert!(!word.sample(false, 10), "still not running: no flip");
        assert!(word.sample(true, 20), "a start flips");
        assert_eq!(
            word.run(),
            GameRun {
                running: true,
                started_ms: Some(20)
            }
        );
        assert!(!word.sample(true, 30), "still running: no flip");
        assert_eq!(word.run().started_ms, Some(20), "first seen, not last seen");
        assert!(word.sample(false, 40), "a stop flips");
        assert_eq!(word.run(), GameRun::default());
        let running_at_launch = GameWord::new(true, 7);
        assert_eq!(running_at_launch.run().started_ms, Some(7));
    }

    #[test]
    fn play_wakes_a_waiting_watcher_at_once() {
        let word = Arc::new(GameWord::new(false, 0));
        let slow = GameCadence {
            every: Duration::from_secs(30),
            after_launch: Duration::from_millis(10),
            after_launch_for: Duration::from_secs(120),
        };
        let waiter = {
            let word = Arc::clone(&word);
            std::thread::spawn(move || {
                let started = Instant::now();
                word.wait(slow);
                started.elapsed()
            })
        };
        std::thread::sleep(Duration::from_millis(100));
        word.launched(Instant::now());
        let waited = waiter.join().unwrap();
        assert!(
            waited < Duration::from_secs(10),
            "the watcher woke for Play instead of sleeping 30 s: {waited:?}"
        );
    }

    #[test]
    fn the_recording_launcher_starts_nothing_and_remembers_everything() {
        let watch = Arc::new(SwitchedGameWatch::default());
        let launcher = RecordingLauncher::starting(Arc::clone(&watch));
        let route = LaunchRoute::Steam {
            url: "steam://rungameid/2054970".into(),
        };
        launcher.launch(&route).unwrap();
        launcher.show_folder(Path::new("/mods")).unwrap();
        assert_eq!(
            launcher.seen(),
            vec![
                Launched::Game(route),
                Launched::Folder(PathBuf::from("/mods"))
            ]
        );
        assert!(watch.running(), "the recorded launch started the game");
        assert_eq!(watch.asked(), 1);
    }
}
