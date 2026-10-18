//! Where Steam keeps Dragon's Dogma 2's saves, and which files in a save folder are slots.
//!
//! `<Steam>\userdata\<account id>\2054970\remote\win64_save\`: one folder per Steam account that
//! played the game. The account id is the folder's name, and it is also the save's key. The bench
//! may point `RIFTSTONE_SAVE_DIR` at one folder, with the account taken from the folder three
//! levels up, or from `RIFTSTONE_SAVE_ACCOUNT`.
//!
//! A slot file is `data` + three characters + an optional `Slot` + `.bin` whose three characters
//! are a number (`data009Slot.bin` is slot 9, `data000.bin` slot 0). The game's system data,
//! `data00-1.bin`, and the `SS1_*` files are not slots.
//!
//! A save folder's path holds the Steam account number, which is private: no `Debug` here prints
//! it, and the spine never logs it.

use std::fs;
use std::path::{Path, PathBuf};

/// Steam's app id for Dragon's Dogma 2.
pub const APP_ID: &str = "2054970";
/// The bench's save folder.
pub const ENV_SAVE_DIR: &str = "RIFTSTONE_SAVE_DIR";
/// The bench's save folder's account, when its path does not say.
pub const ENV_SAVE_ACCOUNT: &str = "RIFTSTONE_SAVE_ACCOUNT";

/// One Steam account's Dragon's Dogma 2 save folder.
#[derive(Clone, PartialEq, Eq)]
pub struct SaveFolder {
    pub account_id: u32,
    pub path: PathBuf,
}

impl std::fmt::Debug for SaveFolder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SaveFolder(‹private›)")
    }
}

/// One slot file in a save folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotFile {
    /// The file's own name (`data009Slot.bin`): what sentences and logs may name.
    pub file_name: String,
    /// The name without `.bin`: the window's handle for the slot.
    pub stem: String,
    pub slot: i64,
}

/// `dir`'s child called `name`, compared the way Windows compares names.
fn child_ci(dir: &Path, name: &str) -> Option<PathBuf> {
    let exact = dir.join(name);
    if exact.exists() {
        return Some(exact);
    }
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(name)
        })
        .map(|entry| entry.path())
}

fn save_folder_under(account_dir: &Path) -> Option<PathBuf> {
    let app = child_ci(account_dir, APP_ID)?;
    let remote = child_ci(&app, "remote")?;
    child_ci(&remote, "win64_save").filter(|p| p.is_dir())
}

/// Every account's save folder under these Steam folders, by account id.
pub fn discover(steam_roots: &[PathBuf]) -> Vec<SaveFolder> {
    let mut found: Vec<SaveFolder> = Vec::new();
    for root in steam_roots {
        let Some(userdata) = child_ci(root, "userdata") else {
            continue;
        };
        let Ok(entries) = fs::read_dir(&userdata) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(account_id) = entry.file_name().to_string_lossy().parse::<u32>() else {
                continue;
            };
            if account_id == 0 {
                continue;
            }
            if let Some(path) = save_folder_under(&entry.path()) {
                let key = path.to_string_lossy().replace('\\', "/").to_lowercase();
                if !found
                    .iter()
                    .any(|f| f.path.to_string_lossy().replace('\\', "/").to_lowercase() == key)
                {
                    found.push(SaveFolder { account_id, path });
                }
            }
        }
    }
    found.sort_by_key(|f| f.account_id);
    found
}

/// The bench's save folder, when `RIFTSTONE_SAVE_DIR` names one.
pub fn from_env() -> Option<SaveFolder> {
    let path = PathBuf::from(std::env::var_os(ENV_SAVE_DIR).filter(|v| !v.is_empty())?);
    let account_id = std::env::var(ENV_SAVE_ACCOUNT)
        .ok()
        .and_then(|a| a.trim().parse().ok())
        .or_else(|| path.ancestors().nth(3)?.file_name()?.to_str()?.parse().ok())?;
    Some(SaveFolder { account_id, path })
}

/// Parse a slot file's name: `data` + a three-character number + optional `Slot` + `.bin`.
pub fn slot_of(file_name: &str) -> Option<SlotFile> {
    let lower = file_name.to_ascii_lowercase();
    let stem = lower.strip_suffix(".bin")?;
    let digits = stem.strip_prefix("data")?;
    let digits = digits.strip_suffix("slot").unwrap_or(digits);
    if digits.len() != 3 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(SlotFile {
        file_name: file_name.to_string(),
        stem: file_name[..file_name.len() - 4].to_string(),
        slot: digits.parse().ok()?,
    })
}

/// The slot files in a save folder, by name.
pub fn slot_files(folder: &Path) -> Vec<SlotFile> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut slots: Vec<SlotFile> = entries
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|e| slot_of(&e.file_name().to_string_lossy()))
        .collect();
    slots.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    slots
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_names_are_read_and_system_files_are_not_slots() {
        assert_eq!(
            slot_of("data009Slot.bin").map(|s| (s.slot, s.stem)),
            Some((9, "data009Slot".into()))
        );
        assert_eq!(slot_of("data000.bin").map(|s| s.slot), Some(0));
        assert_eq!(slot_of("DATA001SLOT.BIN").map(|s| s.slot), Some(1));
        assert_eq!(slot_of("data00-1.bin"), None, "the game's system data");
        assert_eq!(slot_of("SS1_data000.bin"), None);
        assert_eq!(slot_of("data009Slot.bin.bak"), None);
        assert_eq!(slot_of("remotecache.vdf"), None);
    }

    #[test]
    fn every_accounts_folder_is_found_and_the_rest_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let steam = dir.path().join("Steam");
        for (account, has) in [
            ("4242001", true),
            ("17", true),
            ("0", true),
            ("ac", true),
            ("99", false),
        ] {
            let app = steam
                .join("userdata")
                .join(account)
                .join(APP_ID)
                .join("remote");
            if has {
                fs::create_dir_all(app.join("win64_save")).unwrap();
            } else {
                fs::create_dir_all(&app).unwrap();
            }
        }
        let found = discover(&[steam.clone(), steam]);
        assert_eq!(
            found.iter().map(|f| f.account_id).collect::<Vec<_>>(),
            [17, 4_242_001]
        );
        assert_eq!(format!("{:?}", found[0]), "SaveFolder(‹private›)");
    }

    #[test]
    fn a_folders_slots_come_by_name() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "data009Slot.bin",
            "data000.bin",
            "data00-1.bin",
            "SS1_data000.bin",
            "notes.txt",
        ] {
            fs::write(dir.path().join(name), b"x").unwrap();
        }
        fs::create_dir(dir.path().join("data001Slot.bin")).unwrap();
        let slots: Vec<i64> = slot_files(dir.path()).iter().map(|s| s.slot).collect();
        assert_eq!(slots, [0, 9]);
    }
}
