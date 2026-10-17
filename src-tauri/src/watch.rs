//! The mods-folder watcher (startup step 5): the one library folder watched
//! recursively (a folder package's insides count too); a burst of changes
//! settles for [`DEBOUNCE`] (1.5 s), then the library is rescanned in the
//! background and the window hears `rift://library-changed`.
//!
//! What does not count as a change: reads (the scan's own, a thumbnail's),
//! metadata-only touches, files still being written (`.crdownload`, `.part`,
//! `.partial`, `.tmp`, `.download` — a browser's, or the stone's own
//! `.riftstone-<id>.part` while a download runs: the scan skips those anyway,
//! and rescanning on every chunk would be noise; the finished file's rename
//! counts), and anything under the Riftstone's own data and cache folders,
//! which a bench's `RIFTSTONE_DATA_HOME` may put inside the mods folder:
//! every scan writes its cache there, so it would rescan itself forever.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use notify::event::ModifyKind;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use rift_core::lower_key;

/// Quiet time after the last change before a rescan starts.
pub const DEBOUNCE: Duration = Duration::from_millis(1500);

/// Suffixes of files a browser is still writing.
const IN_FLIGHT: &[&str] = &[".crdownload", ".part", ".partial", ".tmp", ".download"];

/// A running watcher. Dropping it stops the watching and its thread.
pub struct LibraryWatcher {
    _watcher: RecommendedWatcher,
}

/// Watch `folder`; call `on_change` once per settled burst of changes.
/// Changes under `ignore` never count. `None` when the folder is not there or
/// the platform watcher could not start (the library then changes only on a
/// manual rescan).
pub fn arm(
    folder: &Path,
    ignore: &[PathBuf],
    quiet: Duration,
    on_change: impl Fn() + Send + 'static,
) -> Option<LibraryWatcher> {
    if !folder.is_dir() {
        log::warn!(
            "the mods folder {} is not there, so it is not watched; rescan by hand once it is",
            folder.display()
        );
        return None;
    }
    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
    let mut watcher = match notify::recommended_watcher(tx) {
        Ok(watcher) => watcher,
        Err(err) => {
            log::warn!(
                "the library watcher could not start ({err}); rescan by hand to see new mods"
            );
            return None;
        }
    };
    if let Err(err) = watcher.watch(folder, RecursiveMode::Recursive) {
        log::warn!(
            "could not watch {} ({err}); rescan by hand to see its changes",
            folder.display()
        );
        return None;
    }
    let ignore = ignore.to_vec();
    let spawned = std::thread::Builder::new()
        .name("riftstone-watch".into())
        .spawn(move || settle(&rx, &ignore, quiet, &on_change));
    if let Err(err) = spawned {
        log::warn!("the library watcher could not start its thread: {err}");
        return None;
    }
    Some(LibraryWatcher { _watcher: watcher })
}

/// Wait for a change, then for `quiet` without one, then call `on_change`.
/// Ends when the watcher (the only sender) is dropped.
fn settle(
    rx: &Receiver<notify::Result<Event>>,
    ignore: &[PathBuf],
    quiet: Duration,
    on_change: &dyn Fn(),
) {
    loop {
        match rx.recv() {
            Ok(event) if counts(&event, ignore) => {}
            Ok(_) => continue,
            Err(_) => return,
        }
        let mut deadline = Instant::now() + quiet;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(left) {
                Ok(event) if counts(&event, ignore) => deadline = Instant::now() + quiet,
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
        on_change();
    }
}

/// Does this event change what the library holds?
fn counts(event: &notify::Result<Event>, ignore: &[PathBuf]) -> bool {
    let Ok(event) = event else {
        // The watcher lost track (an overflow): rescanning is the safe answer.
        return true;
    };
    let kind_counts = match event.kind {
        EventKind::Access(_) => false,
        EventKind::Modify(ModifyKind::Metadata(_)) => false,
        EventKind::Any
        | EventKind::Create(_)
        | EventKind::Modify(_)
        | EventKind::Remove(_)
        | EventKind::Other => true,
    };
    kind_counts
        && (event.paths.is_empty()
            || event
                .paths
                .iter()
                .any(|path| !in_flight(path) && !ignore.iter().any(|dir| within(path, dir))))
}

/// `path` is `dir` or lies inside it, compared the way Windows compares paths:
/// case-blind, either slash.
pub fn within(path: &Path, dir: &Path) -> bool {
    let key = |p: &Path| {
        let text = p.to_string_lossy().replace('\\', "/");
        lower_key(text.trim_end_matches('/'))
    };
    let (path, dir) = (key(path), key(dir));
    path == dir || path.starts_with(&format!("{dir}/"))
}

fn in_flight(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    IN_FLIGHT.iter().any(|suffix| name.ends_with(suffix))
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{AccessKind, CreateKind, MetadataKind};
    use std::path::PathBuf;

    fn event(kind: EventKind, path: &str) -> notify::Result<Event> {
        Ok(Event::new(kind).add_path(PathBuf::from(path)))
    }

    #[test]
    fn reads_touches_and_downloads_in_flight_do_not_count() {
        assert!(counts(
            &event(
                EventKind::Create(CreateKind::File),
                "/mods/Vernworth Cloaks.zip"
            ),
            &[]
        ));
        assert!(counts(
            &event(
                EventKind::Remove(notify::event::RemoveKind::File),
                "/mods/a.zip"
            ),
            &[]
        ));
        assert!(!counts(
            &event(EventKind::Access(AccessKind::Any), "/mods/a.zip"),
            &[]
        ));
        assert!(!counts(
            &event(
                EventKind::Modify(ModifyKind::Metadata(MetadataKind::AccessTime)),
                "/mods/a.zip"
            ),
            &[]
        ));
        assert!(!counts(
            &event(EventKind::Create(CreateKind::File), "/dl/a.zip.crdownload"),
            &[]
        ));
        assert!(!counts(
            &event(EventKind::Create(CreateKind::File), "/dl/a.zip.PART"),
            &[]
        ));
        assert!(
            !counts(
                &event(
                    EventKind::Modify(ModifyKind::Any),
                    "/mods/.riftstone-a1.part"
                ),
                &[]
            ),
            "the stone's own download in flight is no change yet"
        );
        assert!(counts(&Err(notify::Error::generic("overflow")), &[]));
    }

    #[test]
    fn the_riftstones_own_folders_never_count() {
        let own = [PathBuf::from(
            "C:/Users/Arisen/AppData/Roaming/nl.zmuuzn.riftstone",
        )];
        let cache = r"c:\users\arisen\appdata\roaming\NL.ZMUUZN.RIFTSTONE\catalog-cache.json";
        assert!(!counts(
            &event(EventKind::Modify(ModifyKind::Any), cache),
            &own
        ));
        assert!(counts(
            &event(
                EventKind::Create(CreateKind::File),
                "D:/Vernworth/The Riftstone/mods/Lantern Glow.zip"
            ),
            &own
        ));
        assert!(
            counts(
                &Ok(Event::new(EventKind::Create(CreateKind::File))
                    .add_path(PathBuf::from(cache))
                    .add_path(PathBuf::from("D:/Vernworth/The Riftstone/mods/b.zip"))),
                &own
            ),
            "one real change among the stone's own writes still counts"
        );
        assert!(!within(
            Path::new("C:/Users/Arisen/AppData/Roaming/nl.zmuuzn.riftstone-old/x"),
            &own[0]
        ));
        assert!(within(&own[0], &own[0]));
    }

    #[test]
    fn a_mods_folder_holding_the_data_folder_does_not_rescan_itself() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("riftstone-data");
        std::fs::create_dir_all(&data).unwrap();
        let (tx, rx) = mpsc::channel();
        let watcher = arm(
            dir.path(),
            std::slice::from_ref(&data),
            Duration::from_millis(200),
            move || {
                let _ = tx.send(());
            },
        )
        .expect("the platform watcher starts");
        for n in 0..3 {
            std::fs::write(data.join(format!("catalog-cache.json.tmp-{n}")), b"{}").unwrap();
            std::fs::write(data.join("catalog-cache.json"), format!("{{\"n\":{n}}}")).unwrap();
        }
        assert!(
            rx.recv_timeout(Duration::from_millis(1200)).is_err(),
            "the stone's own cache writes are not a library change"
        );
        std::fs::write(dir.path().join("Lantern Glow.zip"), b"PK").unwrap();
        rx.recv_timeout(Duration::from_secs(10))
            .expect("a real download still asks for a rescan");
        drop(watcher);
    }

    #[test]
    fn a_burst_of_changes_settles_into_one_rescan() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, rx) = mpsc::channel();
        let watcher = arm(dir.path(), &[], Duration::from_millis(300), move || {
            let _ = tx.send(());
        })
        .expect("the platform watcher starts");
        for n in 0..5 {
            std::fs::write(dir.path().join(format!("Lantern Glow {n}.zip")), b"PK").unwrap();
        }
        rx.recv_timeout(Duration::from_secs(10))
            .expect("a settled burst asks for one rescan");
        assert!(
            rx.recv_timeout(Duration::from_millis(900)).is_err(),
            "five writes in a burst are one rescan, not five"
        );
        drop(watcher);
    }

    #[test]
    fn a_missing_folder_is_not_watched() {
        let dir = tempfile::tempdir().unwrap();
        assert!(arm(&dir.path().join("mods"), &[], DEBOUNCE, || {}).is_none());
    }
}
