//! What the spine remembers between launches, and where.
//!
//! The app data dir holds four JSON files the spine owns — `settings.json`,
//! `roots.json`, `catalog-cache.json`, `nexus.json` — beside the files
//! rift-deploy owns there (`journal-<hex>.json`, one per game folder, and the
//! `backups/` store). Thumbnails live in `thumbs/` under the app cache dir.
//!
//! `RIFTSTONE_DATA_HOME` replaces both dirs (data = the folder itself, cache =
//! `<folder>/cache`): the tests and the WSL bench never touch the real app
//! data.
//!
//! Every write is a temp file beside the target, flushed to disk, renamed over
//! it — a crash leaves the old file or the new one, never half of either. A
//! file that cannot be parsed is set aside as `<name>.damaged` (never deleted)
//! and the spine starts that part fresh. A file that exists but cannot be
//! READ (permissions, another program's lock, a disk error) is a different
//! case: it may still hold the player's own choices, so that part starts from
//! defaults in memory and the file is HELD — never written over for the rest
//! of the session ([`SavedFile`]); the overview says so.
//!
//! The player's files (`settings.json`, `roots.json`, `nexus.json`) are saved
//! through [`SavedFile::save`], one save at a time, each writing the newest
//! state — so no lock on that state is ever held across a disk write.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use rift_core::{LibraryRoot, RiftError};

use crate::dto::Settings;

/// Env override for the app data + cache dirs (tests, the WSL bench).
pub const DATA_HOME_ENV: &str = "RIFTSTONE_DATA_HOME";

/// The two folders the spine writes into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataDirs {
    /// The app data dir: the spine's JSON files, rift-deploy's journal and backups.
    pub data: PathBuf,
    /// The app cache dir: `thumbs/`.
    pub cache: PathBuf,
}

impl DataDirs {
    /// Both dirs under one folder — what `RIFTSTONE_DATA_HOME` selects.
    pub fn under(home: &Path) -> DataDirs {
        DataDirs {
            data: home.to_path_buf(),
            cache: home.join("cache"),
        }
    }

    /// `RIFTSTONE_DATA_HOME`, when set and not empty.
    pub fn from_env() -> Option<DataDirs> {
        std::env::var_os(DATA_HOME_ENV)
            .filter(|value| !value.is_empty())
            .map(|value| DataDirs::under(Path::new(&value)))
    }

    /// Where the journal and its `backups/` live (rift-deploy's `state_dir`).
    pub fn state_dir(&self) -> &Path {
        &self.data
    }

    pub fn settings_file(&self) -> PathBuf {
        self.data.join("settings.json")
    }

    pub fn roots_file(&self) -> PathBuf {
        self.data.join("roots.json")
    }

    pub fn catalog_cache_file(&self) -> PathBuf {
        self.data.join("catalog-cache.json")
    }

    pub fn nexus_file(&self) -> PathBuf {
        self.data.join("nexus.json")
    }

    pub fn thumbs_dir(&self) -> PathBuf {
        self.cache.join("thumbs")
    }
}

/// `settings.json`: the window's [`Settings`] plus what only the spine needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredSettings {
    #[serde(default)]
    pub veil_thumbnails: bool,
    #[serde(default = "yes")]
    pub auto_check_updates: bool,
    /// The game folder the player picked by hand (`set_game_root`), tried
    /// before Steam discovery on every launch.
    #[serde(default)]
    pub manual_game_root: Option<PathBuf>,
}

fn yes() -> bool {
    true
}

impl Default for StoredSettings {
    fn default() -> Self {
        let window = Settings::default();
        StoredSettings {
            veil_thumbnails: window.veil_thumbnails,
            auto_check_updates: window.auto_check_updates,
            manual_game_root: None,
        }
    }
}

impl StoredSettings {
    /// The part the window reads and writes.
    pub fn window(&self) -> Settings {
        Settings {
            veil_thumbnails: self.veil_thumbnails,
            auto_check_updates: self.auto_check_updates,
        }
    }

    pub fn apply(&mut self, window: Settings) {
        self.veil_thumbnails = window.veil_thumbnails;
        self.auto_check_updates = window.auto_check_updates;
    }
}

/// `roots.json`. Its absence is what makes a launch the first one (the
/// default roots are created then, and only then).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredRoots {
    pub schema: u32,
    pub roots: Vec<LibraryRoot>,
}

impl StoredRoots {
    pub const SCHEMA: u32 = 1;

    pub fn new(roots: Vec<LibraryRoot>) -> StoredRoots {
        StoredRoots {
            schema: StoredRoots::SCHEMA,
            roots,
        }
    }
}

/// What reading one of the spine's files found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Loaded<T> {
    /// There is no such file. (A missing `roots.json` is what makes a launch
    /// the first one.)
    Absent,
    Found(T),
    /// It could not be parsed. It was set aside as `<name>.damaged` (never
    /// deleted); that part starts fresh and may be saved again.
    Damaged,
    /// It exists, but could not be read — the reason, in words. That part
    /// starts from defaults in memory, and the file must not be written over.
    Unreadable(String),
}

impl<T> Loaded<T> {
    /// The value, when the file was read.
    pub fn found(self) -> Option<T> {
        match self {
            Loaded::Found(value) => Some(value),
            _ => None,
        }
    }

    /// Why the file must be held, when it could not be read.
    pub fn unreadable(&self) -> Option<String> {
        match self {
            Loaded::Unreadable(why) => Some(why.clone()),
            _ => None,
        }
    }
}

/// Read a JSON file, telling apart a file that is not there, one that cannot
/// be read, and one that cannot be parsed (set aside as `<name>.damaged` first,
/// so nothing the player had is lost silently).
pub fn load_json<T: DeserializeOwned>(path: &Path) -> Loaded<T> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Loaded::Absent,
        Err(err) => {
            log::warn!(
                "could not read {}: {err}; that part starts from its defaults and the file is left as it is",
                path.display()
            );
            return Loaded::Unreadable(err.to_string());
        }
    };
    match serde_json::from_slice(&bytes) {
        Ok(value) => Loaded::Found(value),
        Err(err) => {
            let aside = sibling(path, ".damaged");
            log::warn!(
                "{} is damaged ({err}); it was set aside as {} and that part starts fresh",
                path.display(),
                aside.display()
            );
            if let Err(err) = fs::rename(path, &aside) {
                log::warn!("could not set {} aside: {err}", path.display());
            }
            Loaded::Damaged
        }
    }
}

/// [`load_json`] for files that are only a cache: anything but a readable,
/// parseable file is `None`.
pub fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    load_json(path).found()
}

/// One of the player's files, saved one save at a time.
///
/// [`SavedFile::save`] takes the file's turn first and only then asks for the
/// value to write, so whichever save runs last writes the newest state — the
/// caller never holds a lock on that state across the disk write. A file that
/// could not be read at launch is held: every save is skipped.
#[derive(Debug)]
pub struct SavedFile {
    path: PathBuf,
    held: Option<String>,
    turn: Mutex<()>,
}

impl SavedFile {
    /// `held`: why the file is not written this session (it exists and could
    /// not be read at launch), or `None`.
    pub fn new(path: PathBuf, held: Option<String>) -> SavedFile {
        SavedFile {
            path,
            held,
            turn: Mutex::new(()),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Why the file is held, when it is.
    pub fn held(&self) -> Option<&str> {
        self.held.as_deref()
    }

    /// Write what `latest` returns, atomically. `latest` runs inside this
    /// file's turn: it must take its own locks briefly and never wait on a
    /// save itself. Failures are logged — the state in memory stands.
    pub fn save<T: Serialize>(&self, pretty: bool, latest: impl FnOnce() -> T) {
        let _turn = self.turn.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(why) = &self.held {
            log::warn!(
                "{} was not saved: it could not be read at launch ({why}), so it is left untouched until the next launch",
                self.path.display()
            );
            return;
        }
        let value = latest();
        if let Err(err) = write_json(&self.path, &value, pretty) {
            log::warn!("{err}");
        }
    }
}

/// Write `value` as JSON, atomically.
pub fn write_json<T: Serialize>(path: &Path, value: &T, pretty: bool) -> Result<(), RiftError> {
    let bytes = if pretty {
        serde_json::to_vec_pretty(value)
    } else {
        serde_json::to_vec(value)
    }
    .map_err(|err| RiftError::io(format!("could not encode {}", path.display()), err))?;
    write_atomic(path, &bytes)
}

/// Write bytes atomically: a temp file beside `path`, flushed, renamed over it.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), RiftError> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)
            .map_err(|err| RiftError::io(format!("could not create {}", dir.display()), err))?;
    }
    let temp = sibling(
        path,
        &format!(
            ".tmp-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ),
    );
    let written = fs::File::create(&temp)
        .and_then(|mut file| {
            file.write_all(bytes)?;
            file.sync_all()
        })
        .and_then(|()| fs::rename(&temp, path));
    if let Err(err) = written {
        let _ = fs::remove_file(&temp);
        return Err(RiftError::io(
            format!("could not write {}", path.display()),
            err,
        ));
    }
    Ok(())
}

/// `path` with `suffix` appended to its file name.
fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(suffix);
    path.with_file_name(name)
}

/// The file's last-modified time, ms since the epoch.
pub fn modified_ms(path: &Path) -> Option<u64> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    let since = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
    u64::try_from(since.as_millis()).ok()
}

/// Now, ms since the epoch.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_home_puts_both_dirs_under_one_folder() {
        let dirs = DataDirs::under(Path::new("/bench/riftstone"));
        assert_eq!(dirs.data, PathBuf::from("/bench/riftstone"));
        assert_eq!(dirs.cache, PathBuf::from("/bench/riftstone/cache"));
        assert_eq!(
            dirs.thumbs_dir(),
            PathBuf::from("/bench/riftstone/cache/thumbs")
        );
        assert_eq!(
            dirs.settings_file(),
            PathBuf::from("/bench/riftstone/settings.json")
        );
        assert_eq!(
            dirs.roots_file(),
            PathBuf::from("/bench/riftstone/roots.json")
        );
        assert_eq!(
            dirs.catalog_cache_file(),
            PathBuf::from("/bench/riftstone/catalog-cache.json")
        );
        assert_eq!(
            dirs.nexus_file(),
            PathBuf::from("/bench/riftstone/nexus.json")
        );
        assert_eq!(dirs.state_dir(), Path::new("/bench/riftstone"));
    }

    #[test]
    fn writes_are_atomic_and_leave_no_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("settings.json");
        let settings = StoredSettings {
            veil_thumbnails: true,
            ..StoredSettings::default()
        };
        write_json(&path, &settings, true).unwrap();
        write_json(&path, &settings, true).unwrap();
        let back: StoredSettings = read_json(&path).unwrap();
        assert_eq!(back, settings);
        let names: Vec<String> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["settings.json".to_string()]);
    }

    #[test]
    fn a_damaged_file_is_set_aside_never_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nexus.json");
        fs::write(&path, b"{ not json").unwrap();
        assert!(read_json::<StoredSettings>(&path).is_none());
        assert!(!path.exists());
        assert_eq!(
            fs::read(dir.path().join("nexus.json.damaged")).unwrap(),
            b"{ not json"
        );
        assert!(read_json::<StoredSettings>(&dir.path().join("missing.json")).is_none());
    }

    #[test]
    fn absent_unreadable_and_damaged_files_are_told_apart() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            load_json::<StoredSettings>(&dir.path().join("missing.json")),
            Loaded::Absent
        );
        // A folder where the file should be: there, but not readable as a file.
        let blocked = dir.path().join("roots.json");
        fs::create_dir_all(&blocked).unwrap();
        let loaded = load_json::<StoredRoots>(&blocked);
        assert!(
            matches!(&loaded, Loaded::Unreadable(why) if !why.is_empty()),
            "{loaded:?}"
        );
        assert!(blocked.is_dir(), "an unreadable file is never moved aside");
        assert!(!dir.path().join("roots.json.damaged").exists());
        let torn = dir.path().join("nexus.json");
        fs::write(&torn, b"{ torn").unwrap();
        assert_eq!(load_json::<StoredSettings>(&torn), Loaded::Damaged);
        let good = dir.path().join("settings.json");
        write_json(&good, &StoredSettings::default(), true).unwrap();
        assert_eq!(
            load_json::<StoredSettings>(&good),
            Loaded::Found(StoredSettings::default())
        );
    }

    #[test]
    fn a_held_file_is_never_written_and_saves_write_the_newest_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("roots.json");
        fs::write(&path, b"the player's own roots").unwrap();
        let held = SavedFile::new(path.clone(), Some("permission denied".into()));
        assert_eq!(held.held(), Some("permission denied"));
        let mut asked = false;
        held.save(true, || {
            asked = true;
            StoredRoots::new(Vec::new())
        });
        assert!(!asked, "a held file does not even ask for the value");
        assert_eq!(fs::read(&path).unwrap(), b"the player's own roots");

        // Two threads change the state and save; whatever order the saves run
        // in, the file ends with the newest state.
        let state = std::sync::Arc::new(Mutex::new(0u32));
        let file = std::sync::Arc::new(SavedFile::new(dir.path().join("count.json"), None));
        let workers: Vec<_> = (0..8)
            .map(|_| {
                let state = std::sync::Arc::clone(&state);
                let file = std::sync::Arc::clone(&file);
                std::thread::spawn(move || {
                    *state.lock().unwrap() += 1;
                    file.save(false, || *state.lock().unwrap());
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
        assert_eq!(read_json::<u32>(file.path()), Some(8));
    }

    #[test]
    fn settings_default_and_tolerate_older_files() {
        let settings: StoredSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings, StoredSettings::default());
        assert!(settings.auto_check_updates, "auto-check defaults to on");
        assert!(!settings.veil_thumbnails);
        let mut stored = StoredSettings::default();
        stored.apply(Settings {
            veil_thumbnails: true,
            auto_check_updates: false,
        });
        assert_eq!(
            stored.window(),
            Settings {
                veil_thumbnails: true,
                auto_check_updates: false
            }
        );
    }
}
