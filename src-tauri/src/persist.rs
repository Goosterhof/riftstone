//! What the spine remembers between launches, and where.
//!
//! The app data dir holds the JSON files the spine owns — `settings.json`,
//! `catalog-cache.json`, `nexus.json`, and (v0.3, only while the player has
//! the nxm handler on) `nxm-handler.json` — beside the files rift-deploy owns
//! there (`journal-<hex>.json`, one per game folder, and the `backups/`
//! store). Thumbnails live in `thumbs/` under the app cache dir. v0.2's
//! `roots.json` may still sit there: v0.3 never reads, writes or deletes it.
//!
//! `RIFTSTONE_DATA_HOME` replaces both dirs (data = the folder itself, cache =
//! `<folder>/cache`): the tests and the WSL bench never touch the real app
//! data.
//!
//! Every write is a temp file beside the target, flushed to disk, renamed over
//! it — a crash leaves the old file or the new one, never half of either. A
//! file that cannot be parsed is set aside as `<name>.damaged` (never deleted)
//! and the spine starts that part fresh — except `nxm-handler.json`, which
//! stays exactly where it is ([`inspect_json`]): the nxm handler refuses to
//! give the links back rather than guess. A file that exists but cannot be
//! READ (permissions, another program's lock, a disk error) is a different
//! case: it may still hold the player's own choices, so that part starts from
//! defaults in memory and the file is HELD — never written over for the rest
//! of the session ([`SavedFile`]); the overview says so.
//!
//! The player's files (`settings.json`, `nexus.json`) are saved
//! through [`SavedFile::save`], one save at a time, each writing the newest
//! state — so no lock on that state is ever held across a disk write.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use rift_core::RiftError;

use crate::dto::{Settings, VeilMode};

/// Env override for the app data + cache dirs (tests, the WSL bench).
pub const DATA_HOME_ENV: &str = "RIFTSTONE_DATA_HOME";

/// The app's identifier (`tauri.conf.json`): the name of its folders under
/// `%APPDATA%` and `%LOCALAPPDATA%`.
pub const APP_IDENTIFIER: &str = "nl.zmuuzn.riftstone";

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

    /// The folders the installed stone uses on Windows, found without Tauri
    /// (the uninstaller's `--give-back-nxm` runs no app): `%APPDATA%` and
    /// `%LOCALAPPDATA%`, each with [`APP_IDENTIFIER`] — where Tauri's
    /// `app_data_dir` and `app_cache_dir` put them. `None` elsewhere, or
    /// when Windows names no such folder.
    pub fn installed() -> Option<DataDirs> {
        if !cfg!(windows) {
            return None;
        }
        DataDirs::installed_under(
            std::env::var_os("APPDATA"),
            std::env::var_os("LOCALAPPDATA"),
        )
    }

    /// [`DataDirs::installed`] from the two folders Windows names.
    pub fn installed_under(
        roaming: Option<std::ffi::OsString>,
        local: Option<std::ffi::OsString>,
    ) -> Option<DataDirs> {
        let roaming = roaming.filter(|dir| !dir.is_empty())?;
        let data = PathBuf::from(roaming).join(APP_IDENTIFIER);
        let cache = local.filter(|dir| !dir.is_empty()).map_or_else(
            || data.join("cache"),
            |dir| PathBuf::from(dir).join(APP_IDENTIFIER),
        );
        Some(DataDirs { data, cache })
    }

    /// Where the journal and its `backups/` live (rift-deploy's `state_dir`).
    pub fn state_dir(&self) -> &Path {
        &self.data
    }

    pub fn settings_file(&self) -> PathBuf {
        self.data.join("settings.json")
    }

    pub fn catalog_cache_file(&self) -> PathBuf {
        self.data.join("catalog-cache.json")
    }

    pub fn nexus_file(&self) -> PathBuf {
        self.data.join("nexus.json")
    }

    /// v0.3: the nxm handler's memory — the command the stone replaced in
    /// the registry when the player turned its handler on.
    pub fn nxm_handler_file(&self) -> PathBuf {
        self.data.join("nxm-handler.json")
    }

    pub fn thumbs_dir(&self) -> PathBuf {
        self.cache.join("thumbs")
    }

    /// v0.3: where the relink's content check unpacks the entries it
    /// fingerprints, one scan at a time. Emptied after every scan and at
    /// every launch; nothing else is ever written there.
    pub fn relink_scratch(&self) -> PathBuf {
        self.cache.join("relink-check")
    }
}

/// `settings.json`: the window's [`Settings`] plus what only the spine needs.
///
/// v0.2 replaced the `veilThumbnails` switch with the three-way `veil`. A
/// v0.1 file is read as it stands: with no `veil` key, its `veilThumbnails`
/// decides (`true` → `all`, `false` → `off`). The file is written with `veil`
/// only, so the next save drops the old key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", from = "SettingsFile")]
pub struct StoredSettings {
    pub veil: VeilMode,
    pub auto_check_updates: bool,
    /// The game folder the player picked by hand (`set_game_root`), tried
    /// before Steam discovery on every launch.
    pub manual_game_root: Option<PathBuf>,
    /// v0.5: THE MUSTER's chosen character (a hash of private names): never logged.
    pub muster_character: Option<String>,
}

/// `settings.json` as any version wrote it.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsFile {
    /// v0.2's key. A value this build does not know (a later version's mode)
    /// reads as absent rather than failing the whole file: the file also
    /// holds the game folder the player picked.
    #[serde(default, deserialize_with = "known_veil")]
    veil: Option<VeilMode>,
    /// v0.1's key, read only when `veil` is absent.
    #[serde(default)]
    veil_thumbnails: Option<bool>,
    #[serde(default = "yes")]
    auto_check_updates: bool,
    #[serde(default)]
    manual_game_root: Option<PathBuf>,
    /// v0.5. Anything but a character key reads as absent rather than failing the whole file.
    #[serde(default, deserialize_with = "known_character")]
    muster_character: Option<String>,
}

fn known_character<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(character_key(value.as_str()))
}

/// A Muster character key as `MusterSlot.character` spells it (16 lowercase hex digits);
/// anything else is no choice at all.
fn character_key(text: Option<&str>) -> Option<String> {
    text.filter(|key| {
        key.len() == 16 && key.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    })
    .map(str::to_owned)
}

fn known_veil<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<VeilMode>, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).ok())
}

impl From<SettingsFile> for StoredSettings {
    fn from(file: SettingsFile) -> Self {
        let veil = file.veil.unwrap_or(match file.veil_thumbnails {
            Some(true) => VeilMode::All,
            Some(false) | None => VeilMode::Off,
        });
        StoredSettings {
            veil,
            auto_check_updates: file.auto_check_updates,
            manual_game_root: file.manual_game_root,
            muster_character: file.muster_character,
        }
    }
}

fn yes() -> bool {
    true
}

impl Default for StoredSettings {
    fn default() -> Self {
        let window = Settings::default();
        StoredSettings {
            veil: window.veil,
            auto_check_updates: window.auto_check_updates,
            manual_game_root: None,
            muster_character: None,
        }
    }
}

impl StoredSettings {
    /// The part the window reads and writes.
    pub fn window(&self) -> Settings {
        Settings {
            veil: self.veil,
            auto_check_updates: self.auto_check_updates,
            muster_character: self.muster_character.clone(),
        }
    }

    pub fn apply(&mut self, window: Settings) {
        self.veil = window.veil;
        self.auto_check_updates = window.auto_check_updates;
        self.muster_character = character_key(window.muster_character.as_deref());
    }
}

/// What reading one of the spine's files found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Loaded<T> {
    /// There is no such file.
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

/// The file's bytes; `Err(Loaded::Absent)` or `Err(Loaded::Unreadable)` when
/// there are none to read.
fn read_bytes<T>(path: &Path) -> Result<Vec<u8>, Loaded<T>> {
    match fs::read(path) {
        Ok(bytes) => Ok(bytes),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Err(Loaded::Absent),
        Err(err) => {
            log::warn!(
                "could not read {}: {err}; that part starts from its defaults and the file is left as it is",
                path.display()
            );
            Err(Loaded::Unreadable(err.to_string()))
        }
    }
}

/// Read a JSON file, telling apart a file that is not there, one that cannot
/// be read, and one that cannot be parsed (set aside as `<name>.damaged` first,
/// so nothing the player had is lost silently).
pub fn load_json<T: DeserializeOwned>(path: &Path) -> Loaded<T> {
    let bytes = match read_bytes(path) {
        Ok(bytes) => bytes,
        Err(loaded) => return loaded,
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

/// [`load_json`] for a file that must stay exactly where it is: one that
/// cannot be parsed is [`Loaded::Damaged`] and is NOT set aside. For a
/// record the stone refuses to act on rather than lose (the nxm handler's
/// memory of what it replaced in the registry, [`DataDirs::nxm_handler_file`]).
pub fn inspect_json<T: DeserializeOwned>(path: &Path) -> Loaded<T> {
    let bytes = match read_bytes(path) {
        Ok(bytes) => bytes,
        Err(loaded) => return loaded,
    };
    match serde_json::from_slice(&bytes) {
        Ok(value) => Loaded::Found(value),
        Err(err) => {
            log::warn!(
                "{} is damaged ({err}); it is left where it is",
                path.display()
            );
            Loaded::Damaged
        }
    }
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
            dirs.catalog_cache_file(),
            PathBuf::from("/bench/riftstone/catalog-cache.json")
        );
        assert_eq!(
            dirs.nexus_file(),
            PathBuf::from("/bench/riftstone/nexus.json")
        );
        assert_eq!(
            dirs.nxm_handler_file(),
            PathBuf::from("/bench/riftstone/nxm-handler.json")
        );
        assert_eq!(dirs.state_dir(), Path::new("/bench/riftstone"));
    }

    #[test]
    fn the_installed_folders_are_the_ones_tauri_names() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(
            conf["identifier"], APP_IDENTIFIER,
            "the folders follow the app's identifier"
        );
        let dirs = DataDirs::installed_under(
            Some(r"C:\Users\Arisen\AppData\Roaming".into()),
            Some(r"C:\Users\Arisen\AppData\Local".into()),
        )
        .unwrap();
        assert_eq!(
            dirs.data,
            Path::new(r"C:\Users\Arisen\AppData\Roaming").join(APP_IDENTIFIER)
        );
        assert_eq!(
            dirs.cache,
            Path::new(r"C:\Users\Arisen\AppData\Local").join(APP_IDENTIFIER)
        );
        assert_eq!(DataDirs::installed_under(None, Some("x".into())), None);
        assert_eq!(DataDirs::installed_under(Some("".into()), None), None);
    }

    #[test]
    fn writes_are_atomic_and_leave_no_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("settings.json");
        let settings = StoredSettings {
            veil: VeilMode::All,
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
        let blocked = dir.path().join("settings.json");
        fs::create_dir_all(&blocked).unwrap();
        let loaded = load_json::<StoredSettings>(&blocked);
        assert!(
            matches!(&loaded, Loaded::Unreadable(why) if !why.is_empty()),
            "{loaded:?}"
        );
        assert!(blocked.is_dir(), "an unreadable file is never moved aside");
        assert!(!dir.path().join("settings.json.damaged").exists());
        let torn = dir.path().join("nexus.json");
        fs::write(&torn, b"{ torn").unwrap();
        assert_eq!(load_json::<StoredSettings>(&torn), Loaded::Damaged);
        let good = dir.path().join("good-settings.json");
        write_json(&good, &StoredSettings::default(), true).unwrap();
        assert_eq!(
            load_json::<StoredSettings>(&good),
            Loaded::Found(StoredSettings::default())
        );
    }

    #[test]
    fn an_inspected_file_is_told_apart_and_never_set_aside() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            inspect_json::<StoredSettings>(&dir.path().join("missing.json")),
            Loaded::Absent
        );
        let torn = dir.path().join("nxm-handler.json");
        fs::write(&torn, b"{ torn").unwrap();
        assert_eq!(inspect_json::<StoredSettings>(&torn), Loaded::Damaged);
        assert_eq!(fs::read(&torn).unwrap(), b"{ torn", "left where it is");
        assert!(!dir.path().join("nxm-handler.json.damaged").exists());
        let blocked = dir.path().join("blocked.json");
        fs::create_dir_all(&blocked).unwrap();
        assert!(matches!(
            inspect_json::<StoredSettings>(&blocked),
            Loaded::Unreadable(why) if !why.is_empty()
        ));
        let good = dir.path().join("good.json");
        write_json(&good, &StoredSettings::default(), true).unwrap();
        assert_eq!(
            inspect_json::<StoredSettings>(&good),
            Loaded::Found(StoredSettings::default())
        );
    }

    #[test]
    fn a_held_file_is_never_written_and_saves_write_the_newest_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nexus.json");
        fs::write(&path, b"the player's own links").unwrap();
        let held = SavedFile::new(path.clone(), Some("permission denied".into()));
        assert_eq!(held.held(), Some("permission denied"));
        let mut asked = false;
        held.save(true, || {
            asked = true;
            StoredSettings::default()
        });
        assert!(!asked, "a held file does not even ask for the value");
        assert_eq!(fs::read(&path).unwrap(), b"the player's own links");

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
        assert_eq!(settings.veil, VeilMode::Off);
        assert_eq!(settings.muster_character, None, "no character picked yet");
        let mut stored = StoredSettings::default();
        stored.apply(Settings {
            veil: VeilMode::Adult,
            auto_check_updates: false,
            muster_character: Some("9f3c0a7e5b1d2c44".into()),
        });
        assert_eq!(
            stored.window(),
            Settings {
                veil: VeilMode::Adult,
                auto_check_updates: false,
                muster_character: Some("9f3c0a7e5b1d2c44".into()),
            }
        );
    }

    #[test]
    fn the_muster_character_is_kept_only_as_a_character_key() {
        for (text, kept) in [
            (serde_json::json!("9f3c0a7e5b1d2c44"), true),
            (serde_json::json!("9F3C0A7E5B1D2C44"), false),
            (serde_json::json!("9f3c0a7e"), false),
            (serde_json::json!("Odalric and Bramble"), false),
            (serde_json::json!(42), false),
            (serde_json::Value::Null, false),
        ] {
            let read: StoredSettings =
                serde_json::from_value(serde_json::json!({ "musterCharacter": text })).unwrap();
            assert_eq!(read.muster_character.is_some(), kept, "{text}");
        }
        let mut stored = StoredSettings::default();
        stored.apply(Settings {
            muster_character: Some("../../settings".into()),
            ..Settings::default()
        });
        assert_eq!(stored.muster_character, None);
    }

    #[test]
    fn a_v01_settings_file_keeps_its_veil_and_the_next_save_writes_veil_only() {
        // v0.1 → v0.2: the old switch decides while `veil` is absent.
        let veiled: StoredSettings = serde_json::from_value(serde_json::json!({
            "veilThumbnails": true,
            "autoCheckUpdates": false,
            "manualGameRoot": "D:\\Games\\Dragons Dogma 2"
        }))
        .unwrap();
        assert_eq!(veiled.veil, VeilMode::All, "veilThumbnails true → all");
        assert!(!veiled.auto_check_updates);
        assert_eq!(
            veiled.manual_game_root.as_deref(),
            Some(Path::new("D:\\Games\\Dragons Dogma 2"))
        );
        let bare: StoredSettings =
            serde_json::from_value(serde_json::json!({ "veilThumbnails": false })).unwrap();
        assert_eq!(bare.veil, VeilMode::Off, "veilThumbnails false → off");

        // v0.2 → disk: `veil` alone, the old key gone.
        let written = serde_json::to_value(&veiled).unwrap();
        assert_eq!(written["veil"], "all");
        assert!(
            written.get("veilThumbnails").is_none(),
            "the old key is never written again: {written}"
        );
        let mut keys: Vec<&str> = written
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "autoCheckUpdates",
                "manualGameRoot",
                "musterCharacter",
                "veil"
            ]
        );

        // A file with both keys was written by v0.2 and edited by hand (or by
        // v0.1 after v0.2): `veil` wins.
        let both: StoredSettings = serde_json::from_value(serde_json::json!({
            "veil": "adult",
            "veilThumbnails": true
        }))
        .unwrap();
        assert_eq!(both.veil, VeilMode::Adult);
    }

    #[test]
    fn a_v01_settings_file_on_disk_migrates_through_a_save() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            br#"{ "veilThumbnails": true, "autoCheckUpdates": true }"#,
        )
        .unwrap();
        let Loaded::Found(stored) = load_json::<StoredSettings>(&path) else {
            panic!("a v0.1 settings.json still loads");
        };
        assert_eq!(stored.window().veil, VeilMode::All);
        let file = SavedFile::new(path.clone(), None);
        file.save(true, || stored.clone());
        let on_disk: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(on_disk["veil"], "all");
        assert!(on_disk.get("veilThumbnails").is_none(), "{on_disk}");
        assert_eq!(read_json::<StoredSettings>(&path), Some(stored));
    }

    #[test]
    fn a_veil_this_build_does_not_know_falls_back_instead_of_losing_the_file() {
        // A later version's mode, or a hand edit: the game folder survives.
        let later: StoredSettings = serde_json::from_value(serde_json::json!({
            "veil": "blurAll",
            "veilThumbnails": true,
            "manualGameRoot": "D:\\Games\\Dragons Dogma 2"
        }))
        .unwrap();
        assert_eq!(later.veil, VeilMode::All, "the old key still speaks");
        assert!(later.manual_game_root.is_some());
        let odd: StoredSettings = serde_json::from_value(serde_json::json!({ "veil": 3 })).unwrap();
        assert_eq!(odd.veil, VeilMode::Off);
        let null: StoredSettings =
            serde_json::from_value(serde_json::json!({ "veil": null })).unwrap();
        assert_eq!(null.veil, VeilMode::Off);
    }
}
