//! Fluffy Mod Manager is a neighbour, not a victim (RS-5): this module only
//! ever READS its files.
//!
//! Fluffy keeps its Dragon's Dogma 2 state in
//! `<fluffy>/Games/DragonsDogma2/installed.ini`:
//!
//! ```ini
//! GameInstallService=Steam
//! GameInstallPath=C:\Program Files (x86)\Steam\steamapps\common\Dragons Dogma 2
//! [Vernworth Cloaks]
//! ModName=Vernworth Cloaks Collection
//! installtype=normal
//! file=natives/stm/appsystem/cloak/mesh/cloak_wb_f.pfb.18
//! file=pak_mods\x0000-Vernworth Cloaks.pak
//! ```
//!
//! A section is one installed mod (its folder name in the archive); `ModName`
//! is its display name; one `file=` per deployed file, relative to the game
//! root, written with either slash.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use rift_core::{lower_key, normalize_entry_path, RiftError};
use serde::{Deserialize, Serialize};

use crate::fsops::child_ci;

/// Env override: Fluffy's folder (the one holding `Modmanager.exe`).
pub const ENV_FLUFFY_ROOT: &str = "RIFTSTONE_FLUFFY_ROOT";

/// The whole discovery search visits at most this many folders.
pub const SEARCH_CAP: usize = 4_000;

/// Folders the search never walks into (system and profile trees).
const SKIPPED: &[&str] = &[
    "windows",
    "programdata",
    "$recycle.bin",
    "system volume information",
    "users",
];

/// Where Fluffy lives and keeps its DD2 state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FluffyInstall {
    /// The folder holding `Modmanager.exe`.
    pub root: PathBuf,
    /// `<root>/Games/DragonsDogma2`.
    pub game_dir: PathBuf,
    /// `<game_dir>/Mods` — Fluffy's library of archives and folders.
    pub mods_dir: PathBuf,
    /// `<game_dir>/installed.ini`, when Fluffy has installed anything.
    pub installed_ini: Option<PathBuf>,
}

/// Fluffy's record of what it installed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FluffyLedger {
    /// `GameInstallPath=` — the game folder Fluffy deploys into.
    pub game_install_path: Option<String>,
    pub mods: Vec<FluffyMod>,
}

/// One mod Fluffy installed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FluffyMod {
    /// The section header — the mod's folder name in its archive.
    pub section: String,
    /// `ModName=`, else the section.
    pub name: String,
    /// Deployed files, normalized, `/` separators, relative to the game root.
    pub files: Vec<String>,
}

impl FluffyLedger {
    /// The mods whose sections are not in `adopted` (lower-cased sections).
    pub fn unadopted<'a>(
        &'a self,
        adopted: &'a std::collections::HashSet<String>,
    ) -> impl Iterator<Item = &'a FluffyMod> + 'a {
        self.mods
            .iter()
            .filter(move |m| !adopted.contains(&lower_key(&m.section)))
    }
}

/// Find Fluffy: `RIFTSTONE_FLUFFY_ROOT`, then a bounded search — every drive
/// root's children and grandchildren, and the user's Downloads, Desktop,
/// Documents and Games folders three levels deep — for a folder holding
/// `Modmanager.exe` and `Games/DragonsDogma2/`. At most [`SEARCH_CAP`]
/// folders are visited.
pub fn discover_fluffy() -> Option<FluffyInstall> {
    if let Some(root) = std::env::var_os(ENV_FLUFFY_ROOT).filter(|v| !v.is_empty()) {
        let root = PathBuf::from(root);
        if let Some(found) = check_root(&root) {
            return Some(found);
        }
        log::warn!(
            "{ENV_FLUFFY_ROOT} points at {}, which is not a Fluffy Mod Manager folder with Dragon's Dogma 2 state",
            root.display()
        );
    }
    let mut searches: Vec<(PathBuf, usize)> = drive_roots().into_iter().map(|d| (d, 2)).collect();
    searches.extend(user_folders().into_iter().map(|d| (d, 3)));
    search(&searches, SEARCH_CAP)
}

/// The testable core of discovery: search these folders (each one itself,
/// then three levels below it) for Fluffy.
pub fn discover_fluffy_in(candidates: &[PathBuf]) -> Option<FluffyInstall> {
    let searches: Vec<(PathBuf, usize)> = candidates.iter().map(|c| (c.clone(), 3)).collect();
    search(&searches, SEARCH_CAP)
}

fn search(roots: &[(PathBuf, usize)], cap: usize) -> Option<FluffyInstall> {
    let mut visited = 0usize;
    for (root, depth) in roots {
        let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::from([(root.clone(), 0)]);
        while let Some((dir, level)) = queue.pop_front() {
            if visited >= cap {
                log::warn!("the Fluffy search stopped after {cap} folders");
                return None;
            }
            visited += 1;
            if let Some(found) = check_root(&dir) {
                return Some(found);
            }
            if level >= *depth {
                continue;
            }
            for child in subfolders(&dir) {
                queue.push_back((child, level + 1));
            }
        }
    }
    None
}

/// Real (non-link) subfolders of `dir`, sorted, minus the skipped trees.
fn subfolders(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut folders: Vec<PathBuf> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|entry| {
            let name = lower_key(&entry.file_name().to_string_lossy());
            !SKIPPED.contains(&name.as_str()) && !name.starts_with("program files")
        })
        .map(|entry| entry.path())
        .collect();
    folders.sort();
    folders
}

/// Is `dir` a Fluffy folder with Dragon's Dogma 2 state?
fn check_root(dir: &Path) -> Option<FluffyInstall> {
    let exe = child_ci(dir, "Modmanager.exe")?;
    if !exe.is_file() {
        return None;
    }
    let game_dir = child_ci(&child_ci(dir, "Games")?, "DragonsDogma2")?;
    if !game_dir.is_dir() {
        return None;
    }
    let mods_dir = child_ci(&game_dir, "Mods").unwrap_or_else(|| game_dir.join("Mods"));
    let installed_ini = child_ci(&game_dir, "installed.ini").filter(|p| p.is_file());
    Some(FluffyInstall {
        root: dir.to_path_buf(),
        game_dir,
        mods_dir,
        installed_ini,
    })
}

fn drive_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for letter in b'c'..=b'z' {
        let letter = char::from(letter);
        let root = if cfg!(windows) {
            PathBuf::from(format!("{}:\\", letter.to_ascii_uppercase()))
        } else {
            // The WSL bench mounts Windows drives at /mnt/<letter>.
            PathBuf::from(format!("/mnt/{letter}"))
        };
        if root.is_dir() {
            roots.push(root);
        }
    }
    roots
}

fn user_folders() -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = [
        dirs::download_dir(),
        dirs::desktop_dir(),
        dirs::document_dir(),
    ]
    .into_iter()
    .flatten()
    .collect();
    if let Some(home) = dirs::home_dir() {
        folders.push(home.join("Games"));
    }
    folders.retain(|f| f.is_dir());
    folders
}

/// Read Fluffy's `installed.ini`.
pub fn read_ledger(installed_ini: &Path) -> Result<FluffyLedger, RiftError> {
    let bytes = fs::read(installed_ini).map_err(|err| RiftError::unreadable(installed_ini, err))?;
    Ok(parse_ledger(&bytes))
}

/// Parse `installed.ini` bytes (UTF-8 with or without BOM, UTF-16 with a
/// BOM, else Windows-1252). Keys are case-insensitive; a repeated section
/// gathers its files into one mod; one file listed twice (in any spelling —
/// Windows treats `natives/STM` and `natives/stm` as one place) is kept once;
/// `file=` paths that would climb out of the game folder are dropped.
pub fn parse_ledger(bytes: &[u8]) -> FluffyLedger {
    let text = decode(bytes);
    let mut ledger = FluffyLedger::default();
    let mut by_section: HashMap<String, usize> = HashMap::new();
    let mut named: Vec<bool> = Vec::new();
    let mut listed: Vec<HashSet<String>> = Vec::new();
    let mut current: Option<usize> = None;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') && line.len() >= 2 {
            let section = line[1..line.len() - 1].trim();
            if section.is_empty() {
                current = None;
                continue;
            }
            let index = *by_section.entry(lower_key(section)).or_insert_with(|| {
                ledger.mods.push(FluffyMod {
                    section: section.to_string(),
                    name: section.to_string(),
                    files: Vec::new(),
                });
                named.push(false);
                listed.push(HashSet::new());
                ledger.mods.len() - 1
            });
            current = Some(index);
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        match (current, key.as_str()) {
            (None, "gameinstallpath") if !value.is_empty() => {
                ledger.game_install_path = Some(value.to_string());
            }
            (Some(index), "modname") if !value.is_empty() && !named[index] => {
                ledger.mods[index].name = value.to_string();
                named[index] = true;
            }
            (Some(index), "file") => match normalize_entry_path(value) {
                Ok(path) if !path.is_empty() => {
                    if listed[index].insert(lower_key(&path)) {
                        ledger.mods[index].files.push(path);
                    }
                }
                Ok(_) => {}
                Err(_) => log::warn!(
                    "Fluffy's installed.ini lists an unsafe path {value:?} under [{}]; ignored",
                    ledger.mods[index].section
                ),
            },
            _ => {}
        }
    }
    ledger
}

fn decode(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return encoding_rs::UTF_16LE
            .decode_without_bom_handling(rest)
            .0
            .into_owned();
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return encoding_rs::UTF_16BE
            .decode_without_bom_handling(rest)
            .0
            .into_owned();
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => encoding_rs::WINDOWS_1252
            .decode_without_bom_handling(bytes)
            .0
            .into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEDGER: &str = "GameInstallService=Steam\r\nGameInstallPath=C:\\Program Files (x86)\\Steam\\steamapps\\common\\Dragons Dogma 2\r\n[RE Framework]\r\nModID_v1=581683767515617555\r\nShortModID_v1=16313623759508256203\r\nModName=RE Framework\r\ninstalltype=normal\r\nfile=dinput8.dll\r\n[Lantern Glow-v2]\r\nModName=Lantern Glow Recolor\r\ninstalltype=normal\r\nfile=pak_mods\\x0000-Lantern Glow-v2.pak\r\n[Cloaks314-Ermine]\r\nModName=Ermine Cape\r\nfile=natives/stm/appsystem/CloakWb/mesh/cloak_Ermine_wb_f.pfb.18\r\nfile=reframework\\data\\GransysWardrobe\\database\\cloak\\Female\\Cloak_314.json\r\n[Menu Only]\r\nModName=Vernworth Cloaks Collection\r\ninstalltype=normal\r\n";

    #[test]
    fn reads_the_documented_shape_with_both_slashes() {
        let ledger = parse_ledger(LEDGER.as_bytes());
        assert_eq!(
            ledger.game_install_path.as_deref(),
            Some(r"C:\Program Files (x86)\Steam\steamapps\common\Dragons Dogma 2")
        );
        assert_eq!(ledger.mods.len(), 4);
        assert_eq!(ledger.mods[0].section, "RE Framework");
        assert_eq!(ledger.mods[0].files, ["dinput8.dll"]);
        assert_eq!(ledger.mods[1].name, "Lantern Glow Recolor");
        assert_eq!(ledger.mods[1].files, ["pak_mods/x0000-Lantern Glow-v2.pak"]);
        assert_eq!(
            ledger.mods[2].files,
            [
                "natives/stm/appsystem/CloakWb/mesh/cloak_Ermine_wb_f.pfb.18",
                "reframework/data/GransysWardrobe/database/cloak/Female/Cloak_314.json"
            ]
        );
        // A section with no files is still a mod Fluffy considers installed.
        assert_eq!(ledger.mods[3].name, "Vernworth Cloaks Collection");
        assert!(ledger.mods[3].files.is_empty());
    }

    #[test]
    fn reads_every_encoding_fluffy_might_write() {
        let mut utf8_bom = vec![0xEF, 0xBB, 0xBF];
        utf8_bom.extend_from_slice(b"[Caf\xC3\xA9 Mod]\r\nfile=dinput8.dll\r\n");
        assert_eq!(parse_ledger(&utf8_bom).mods[0].section, "Café Mod");

        let mut utf16 = vec![0xFF, 0xFE];
        for unit in "[Café Mod]\r\nModName=Café\r\nfile=a.dll\r\n".encode_utf16() {
            utf16.extend_from_slice(&unit.to_le_bytes());
        }
        let ledger = parse_ledger(&utf16);
        assert_eq!(ledger.mods[0].name, "Café");
        assert_eq!(ledger.mods[0].files, ["a.dll"]);

        let latin = b"[Caf\xE9 Mod]\r\nfile=b.dll\r\n";
        assert_eq!(parse_ledger(latin).mods[0].section, "Café Mod");
    }

    #[test]
    fn repeated_sections_merge_and_unsafe_files_are_dropped() {
        let text = "[A]\nfile=natives/stm/a.mesh\nFILE=..\\..\\evil.dll\n[a]\nfile=natives/stm/b.mesh\nfile=C:/Windows/evil.dll\n";
        let ledger = parse_ledger(text.as_bytes());
        assert_eq!(ledger.mods.len(), 1);
        assert_eq!(
            ledger.mods[0].files,
            ["natives/stm/a.mesh", "natives/stm/b.mesh"]
        );
    }

    fn fluffy_at(base: &Path, rel: &str) -> PathBuf {
        let root = base.join(rel);
        fs::create_dir_all(root.join("Games/DragonsDogma2/Mods")).unwrap();
        fs::write(root.join("Modmanager.exe"), b"MZ").unwrap();
        fs::write(root.join("Games/DragonsDogma2/installed.ini"), LEDGER).unwrap();
        root
    }

    #[test]
    fn discovery_finds_fluffy_a_few_folders_down() {
        let dir = tempfile::tempdir().unwrap();
        let root = fluffy_at(dir.path(), "Modding/Fluffy");
        let found = discover_fluffy_in(&[dir.path().to_path_buf()]).expect("found");
        assert_eq!(found.root, root);
        assert_eq!(found.mods_dir, root.join("Games/DragonsDogma2/Mods"));
        assert_eq!(
            found.installed_ini,
            Some(root.join("Games/DragonsDogma2/installed.ini"))
        );
        let ledger = read_ledger(found.installed_ini.as_ref().unwrap()).unwrap();
        assert_eq!(ledger.mods.len(), 4);
    }

    #[test]
    fn discovery_needs_the_exe_and_the_dd2_state_and_skips_system_trees() {
        let dir = tempfile::tempdir().unwrap();
        // An exe without DD2 state is some other Fluffy.
        let decoy = dir.path().join("Decoy");
        fs::create_dir_all(decoy.join("Games/MonsterHunterWilds")).unwrap();
        fs::write(decoy.join("Modmanager.exe"), b"MZ").unwrap();
        // A real one under a skipped tree is never visited.
        fluffy_at(dir.path(), "Program Files/Fluffy");
        fluffy_at(dir.path(), "Users/someone");
        assert!(discover_fluffy_in(&[dir.path().to_path_buf()]).is_none());
        // Case does not matter.
        let upper = dir.path().join("Tools/FMM");
        fs::create_dir_all(upper.join("GAMES/dragonsdogma2")).unwrap();
        fs::write(upper.join("MODMANAGER.EXE"), b"MZ").unwrap();
        let found = discover_fluffy_in(&[dir.path().to_path_buf()]).expect("found");
        assert_eq!(found.root, upper);
        assert_eq!(found.installed_ini, None);
    }

    #[test]
    fn discovery_stops_at_its_depth_and_its_cap() {
        let dir = tempfile::tempdir().unwrap();
        fluffy_at(dir.path(), "a/b/c/d/deep");
        assert!(discover_fluffy_in(&[dir.path().to_path_buf()]).is_none());
        let wide = tempfile::tempdir().unwrap();
        for i in 0..20 {
            fs::create_dir_all(wide.path().join(format!("{i:02}"))).unwrap();
        }
        fluffy_at(wide.path(), "19/fluffy");
        assert!(search(&[(wide.path().to_path_buf(), 3)], 10).is_none());
        assert!(search(&[(wide.path().to_path_buf(), 3)], 100).is_some());
    }
}
