//! One build at a time of a value read from disk — the game folder's
//! [`crate::state::Snapshot`] — shared by everyone who asks while it is being
//! built.
//!
//! - A change the spine makes itself (an install, a scan, a switch) calls
//!   [`Fresh::invalidate`]: every later ask wants a value built after it.
//! - [`Fresh::get`] returns the newest value built since the last
//!   invalidation. When there is none, it waits for a build already under way
//!   that will do, or builds one itself — so the burst of calls the window
//!   makes after one event (a search, the overview, the open option) reads the
//!   game folder once, not once each, and a search between two changes reads
//!   nothing at all.
//! - [`Fresh::get_recent`] also wants a build that began at most [`RECENT`]
//!   before the ask: the window's re-reads (`get_overview`, `get_installed`)
//!   see what changed on disk without the spine knowing (Fluffy installing,
//!   the game starting, REFramework copied in by hand).
//!
//! The build runs with no lock held. A build that panics wakes its waiters
//! (the next ask builds again) and then passes the panic on to its caller.

use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// How old a build may be and still answer a re-read.
pub const RECENT: Duration = Duration::from_millis(250);

/// Which change a build follows, and when it began.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Stamp {
    generation: u64,
    started: Instant,
}

struct Slot<T> {
    /// Bumped by every invalidation.
    generation: u64,
    /// The newest finished build.
    current: Option<(Stamp, Arc<T>)>,
    /// The newest build under way.
    building: Option<Stamp>,
    /// Builds started so far.
    builds: u64,
}

pub struct Fresh<T> {
    slot: Mutex<Slot<T>>,
    ready: Condvar,
}

impl<T> Default for Fresh<T> {
    fn default() -> Self {
        Fresh {
            slot: Mutex::new(Slot {
                generation: 0,
                current: None,
                building: None,
                builds: 0,
            }),
            ready: Condvar::new(),
        }
    }
}

impl<T> Fresh<T> {
    fn lock(&self) -> MutexGuard<'_, Slot<T>> {
        self.slot.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Something changed: later asks want a value built after this.
    pub fn invalidate(&self) {
        self.lock().generation += 1;
    }

    /// The newest value built since the last invalidation.
    pub fn get(&self, build: impl FnOnce() -> T) -> Arc<T> {
        self.ask(None, build)
    }

    /// [`Fresh::get`], built at most [`RECENT`] ago.
    pub fn get_recent(&self, build: impl FnOnce() -> T) -> Arc<T> {
        let asked = Instant::now();
        self.ask(Some(asked.checked_sub(RECENT).unwrap_or(asked)), build)
    }

    /// How many builds have started (the tests count them).
    pub fn builds(&self) -> u64 {
        self.lock().builds
    }

    fn ask(&self, not_before: Option<Instant>, build: impl FnOnce() -> T) -> Arc<T> {
        let mut slot = self.lock();
        let wanted = slot.generation;
        let fits = |stamp: &Stamp| {
            stamp.generation >= wanted && not_before.is_none_or(|at| stamp.started >= at)
        };
        loop {
            if let Some((stamp, value)) = &slot.current {
                if fits(stamp) {
                    return Arc::clone(value);
                }
            }
            match slot.building {
                Some(stamp) if fits(&stamp) => {
                    slot = self
                        .ready
                        .wait(slot)
                        .unwrap_or_else(PoisonError::into_inner);
                }
                _ => break,
            }
        }

        let stamp = Stamp {
            generation: slot.generation,
            started: Instant::now(),
        };
        slot.building = Some(stamp);
        slot.builds += 1;
        drop(slot);
        let outcome = panic::catch_unwind(AssertUnwindSafe(build));

        let mut slot = self.lock();
        if slot.building == Some(stamp) {
            slot.building = None;
        }
        let outcome = outcome.map(|value| {
            let value = Arc::new(value);
            if slot
                .current
                .as_ref()
                .is_none_or(|(newest, _)| stamp > *newest)
            {
                slot.current = Some((stamp, Arc::clone(&value)));
            }
            value
        });
        drop(slot);
        self.ready.notify_all();
        match outcome {
            Ok(value) => value,
            Err(panic) => panic::resume_unwind(panic),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::mpsc;

    #[test]
    fn asks_between_changes_build_nothing_and_a_change_builds_once() {
        let fresh: Fresh<u32> = Fresh::default();
        assert_eq!(*fresh.get(|| 1), 1);
        assert_eq!(*fresh.get(|| unreachable!("nothing changed")), 1);
        fresh.invalidate();
        assert_eq!(*fresh.get(|| 2), 2);
        assert_eq!(fresh.builds(), 2);
    }

    #[test]
    fn a_burst_after_a_change_shares_one_build() {
        let fresh: Arc<Fresh<u32>> = Arc::new(Fresh::default());
        let built = Arc::new(AtomicU32::new(0));
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let first = {
            let fresh = Arc::clone(&fresh);
            let built = Arc::clone(&built);
            std::thread::spawn(move || {
                *fresh.get(move || {
                    started_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    built.fetch_add(1, Ordering::SeqCst) + 1
                })
            })
        };
        started_rx.recv().unwrap();
        // Seven more asks — four of them re-reads — while the build runs.
        let others: Vec<_> = (0..7)
            .map(|n| {
                let fresh = Arc::clone(&fresh);
                let built = Arc::clone(&built);
                std::thread::spawn(move || {
                    let build = move || built.fetch_add(1, Ordering::SeqCst) + 1;
                    if n % 2 == 0 {
                        *fresh.get_recent(build)
                    } else {
                        *fresh.get(build)
                    }
                })
            })
            .collect();
        std::thread::sleep(Duration::from_millis(50));
        release_tx.send(()).unwrap();
        assert_eq!(first.join().unwrap(), 1);
        for other in others {
            assert_eq!(other.join().unwrap(), 1, "every ask got the shared build");
        }
        assert_eq!(built.load(Ordering::SeqCst), 1);
        assert_eq!(fresh.builds(), 1);
    }

    #[test]
    fn a_re_read_builds_again_once_the_last_build_is_old() {
        let fresh: Fresh<u32> = Fresh::default();
        fresh.get(|| 1);
        assert_eq!(*fresh.get_recent(|| unreachable!("still recent")), 1);
        std::thread::sleep(RECENT + Duration::from_millis(20));
        assert_eq!(*fresh.get_recent(|| 2), 2, "the disk is read again");
        assert_eq!(
            *fresh.get(|| unreachable!("nothing changed")),
            2,
            "a plain ask takes the re-read's value"
        );
    }

    #[test]
    fn a_build_that_began_before_a_change_never_answers_after_it() {
        let fresh: Arc<Fresh<u32>> = Arc::new(Fresh::default());
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let stale = {
            let fresh = Arc::clone(&fresh);
            std::thread::spawn(move || {
                *fresh.get(move || {
                    started_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    1
                })
            })
        };
        started_rx.recv().unwrap();
        fresh.invalidate();
        // Asked after the change: the build under way read the old state.
        assert_eq!(*fresh.get(|| 2), 2);
        release_tx.send(()).unwrap();
        assert_eq!(stale.join().unwrap(), 1);
        assert_eq!(
            *fresh.get(|| unreachable!("the newer build stands")),
            2,
            "a slower old build does not replace a newer one"
        );
    }

    #[test]
    fn a_build_that_panics_wakes_its_waiters() {
        let fresh: Arc<Fresh<u32>> = Arc::new(Fresh::default());
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let doomed = {
            let fresh = Arc::clone(&fresh);
            std::thread::spawn(move || {
                fresh.get(move || -> u32 {
                    started_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    panic!("the game folder vanished mid-read");
                });
            })
        };
        started_rx.recv().unwrap();
        let waiter = {
            let fresh = Arc::clone(&fresh);
            std::thread::spawn(move || *fresh.get(|| 7))
        };
        std::thread::sleep(Duration::from_millis(50));
        release_tx.send(()).unwrap();
        assert!(doomed.join().is_err(), "the panic reaches its own caller");
        assert_eq!(waiter.join().unwrap(), 7, "the waiter built again");
    }
}
