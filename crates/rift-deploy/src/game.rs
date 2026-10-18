//! Finding Dragon's Dogma 2 — discovery, not configuration.
//!
//! Order: `RIFTSTONE_GAME_ROOT` (the bench and a manual override) → on
//! Windows the registry (`HKCU\Software\Valve\Steam\SteamPath`, then
//! `HKLM\SOFTWARE\WOW6432Node\Valve\Steam\InstallPath`) → `RIFTSTONE_STEAM_ROOT`
//! (the WSL bench: `/mnt/c/Program Files (x86)/Steam`) → Steam's default
//! folders on Windows. For each Steam root: every library in
//! `steamapps/libraryfolders.vdf` (those whose `apps` list app 2054970
//! first), its `steamapps/appmanifest_2054970.acf` `installdir`, then
//! `<library>/steamapps/common/<installdir>`. A valid root holds `DD2.exe`
//! (any case).

use std::fs;
use std::path::{Path, PathBuf};

use rift_core::{dd2, lower_key, RiftError};
use serde::{Deserialize, Serialize};

use crate::fsops::child_ci;
pub use crate::vdf;

/// Steam's app id for Dragon's Dogma 2.
pub const DD2_STEAM_APP_ID: &str = dd2::STEAM_APP_ID;

/// The game's executable — a folder holding it is a game root.
pub const GAME_EXE: &str = "DD2.exe";

/// Env override: the game folder itself.
pub const ENV_GAME_ROOT: &str = "RIFTSTONE_GAME_ROOT";

/// Env override: a Steam install folder whose libraries are searched.
pub const ENV_STEAM_ROOT: &str = "RIFTSTONE_STEAM_ROOT";

/// One Dragon's Dogma 2 install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameInstall {
    pub root: PathBuf,
    /// Steam's build id from the app manifest, when one sits beside the game.
    pub build_id: Option<String>,
    pub source: GameSource,
}

/// How the install was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GameSource {
    Steam,
    Env,
    Manual,
}

/// Find the game the way a player would expect: the env override, then
/// every Steam root this machine knows about.
pub fn discover_game() -> Option<GameInstall> {
    if let Some(root) = env_path(ENV_GAME_ROOT) {
        if has_game_exe(&root) {
            return Some(GameInstall {
                build_id: read_build_id(&root),
                root,
                source: GameSource::Env,
            });
        }
        log::warn!(
            "{ENV_GAME_ROOT} points at {}, which holds no {GAME_EXE}; searching Steam instead",
            root.display()
        );
    }
    steam_roots()
        .iter()
        .find_map(|steam| discover_game_from(Some(steam)))
}

/// The testable core of discovery: look through one Steam install's
/// libraries. `None` in, `None` out.
pub fn discover_game_from(steam_root: Option<&Path>) -> Option<GameInstall> {
    let steam = steam_root?;
    for library in candidate_libraries(steam) {
        let Some(steamapps) = child_ci(&library, "steamapps") else {
            continue;
        };
        let manifest_name = format!("appmanifest_{DD2_STEAM_APP_ID}.acf");
        let Some(manifest) = child_ci(&steamapps, &manifest_name).and_then(|p| read_vdf(&p)) else {
            continue;
        };
        let state = manifest.get("AppState");
        let Some(install_dir) = state.and_then(|s| s.get_str("installdir")) else {
            continue;
        };
        let Some(common) = child_ci(&steamapps, "common") else {
            continue;
        };
        let root = child_ci(&common, install_dir).unwrap_or_else(|| common.join(install_dir));
        if has_game_exe(&root) {
            return Some(GameInstall {
                build_id: state
                    .and_then(|s| s.get_str("buildid"))
                    .filter(|b| !b.is_empty())
                    .map(str::to_string),
                root,
                source: GameSource::Steam,
            });
        }
    }
    None
}

/// A folder the player picked by hand. It must hold `DD2.exe`.
pub fn validate_game_root(path: &Path) -> Result<GameInstall, RiftError> {
    if !path.is_dir() {
        return Err(RiftError::unreadable(
            path,
            "the folder does not exist or is not a folder",
        ));
    }
    if !has_game_exe(path) {
        return Err(RiftError::GameNotFound);
    }
    Ok(GameInstall {
        root: path.to_path_buf(),
        build_id: read_build_id(path),
        source: GameSource::Manual,
    })
}

/// Steam's build id for the install at `game_root`, from the library's
/// `appmanifest_2054970.acf` (`<library>/steamapps/common/<game>` → two
/// folders up).
pub fn read_build_id(game_root: &Path) -> Option<String> {
    let steamapps = game_root.parent()?.parent()?;
    let manifest = child_ci(steamapps, &format!("appmanifest_{DD2_STEAM_APP_ID}.acf"))?;
    read_vdf(&manifest)?
        .get("AppState")?
        .get_str("buildid")
        .filter(|b| !b.is_empty())
        .map(str::to_string)
}

/// Does `root` hold the game's executable (any case)?
pub(crate) fn has_game_exe(root: &Path) -> bool {
    child_ci(root, GAME_EXE)
        .and_then(|p| fs::metadata(p).ok())
        .is_some_and(|m| m.is_file())
}

/// Libraries to search, those that list the app first. The Steam root itself
/// is always a library (on the WSL bench its own entry in the vdf carries a
/// Windows path that has to be translated first).
fn candidate_libraries(steam: &Path) -> Vec<PathBuf> {
    let mut with_app: Vec<PathBuf> = Vec::new();
    let mut others: Vec<PathBuf> = vec![steam.to_path_buf()];
    let vdf_path = child_ci(steam, "steamapps").and_then(|s| child_ci(&s, "libraryfolders.vdf"));
    if let Some(doc) = vdf_path.and_then(|p| read_vdf(&p)) {
        if let Some(folders) = doc.get("libraryfolders") {
            for (key, value) in folders.entries() {
                match value {
                    vdf::Vdf::Obj(_) => {
                        let Some(path) = value.get_str("path") else {
                            continue;
                        };
                        let holds_app = value
                            .get("apps")
                            .and_then(|apps| apps.get(DD2_STEAM_APP_ID))
                            .is_some();
                        let library = localize_library_path(path);
                        if holds_app {
                            with_app.push(library);
                        } else {
                            others.push(library);
                        }
                    }
                    // The pre-2021 format: `"1" "D:\\SteamLibrary"`.
                    vdf::Vdf::Str(path) if key.chars().all(|c| c.is_ascii_digit()) => {
                        others.push(localize_library_path(path));
                    }
                    vdf::Vdf::Str(_) => {}
                }
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    with_app
        .into_iter()
        .chain(others)
        .filter(|p| seen.insert(lower_key(&p.to_string_lossy().replace('\\', "/"))))
        .collect()
}

/// A library path as written by Steam on Windows. Off Windows (the WSL
/// bench), `C:\Steam` that does not exist is tried as `/mnt/c/Steam`.
fn localize_library_path(raw: &str) -> PathBuf {
    let direct = PathBuf::from(raw);
    if cfg!(windows) || direct.exists() {
        return direct;
    }
    windows_to_wsl(raw).filter(|p| p.exists()).unwrap_or(direct)
}

/// `C:\Program Files (x86)\Steam` → `/mnt/c/Program Files (x86)/Steam`.
pub(crate) fn windows_to_wsl(raw: &str) -> Option<PathBuf> {
    let mut chars = raw.chars();
    let drive = chars.next().filter(char::is_ascii_alphabetic)?;
    if chars.next() != Some(':') {
        return None;
    }
    let rest: String = chars.collect::<String>().replace('\\', "/");
    let rest = rest.trim_start_matches('/');
    Some(PathBuf::from(format!(
        "/mnt/{}/{rest}",
        drive.to_ascii_lowercase()
    )))
}

fn read_vdf(path: &Path) -> Option<vdf::Vdf> {
    let bytes = fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    match vdf::parse(&text) {
        Ok(doc) => Some(doc),
        Err(err) => {
            log::warn!("could not read {}: {err}", path.display());
            None
        }
    }
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Every Steam install folder this machine knows about, in the order discovery asks them:
/// the registry (Windows), `RIFTSTONE_STEAM_ROOT`, then Steam's default folders (Windows).
/// v0.4: the Muster looks for the saves under these (`userdata`), where Play finds the game.
pub fn steam_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    #[cfg(windows)]
    roots.extend(registry::steam_roots());
    if let Some(root) = env_path(ENV_STEAM_ROOT) {
        roots.push(root);
    }
    #[cfg(windows)]
    {
        roots.push(PathBuf::from(r"C:\Program Files (x86)\Steam"));
        roots.push(PathBuf::from(r"C:\Program Files\Steam"));
    }
    let mut seen = std::collections::HashSet::new();
    roots
        .into_iter()
        .filter(|p| seen.insert(lower_key(&p.to_string_lossy().replace('\\', "/"))))
        .collect()
}

#[cfg(windows)]
mod registry {
    use std::path::PathBuf;

    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    /// Steam's own record of where it lives. `SteamPath` is written with
    /// forward slashes (`c:/program files (x86)/steam`).
    pub(super) fn steam_roots() -> Vec<PathBuf> {
        let mut roots = Vec::new();
        if let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Software\Valve\Steam") {
            if let Ok(path) = key.get_value::<String, _>("SteamPath") {
                roots.push(PathBuf::from(path.replace('/', "\\")));
            }
        }
        if let Ok(key) =
            RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SOFTWARE\WOW6432Node\Valve\Steam")
        {
            if let Ok(path) = key.get_value::<String, _>("InstallPath") {
                roots.push(PathBuf::from(path));
            }
        }
        roots
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn manifest(install_dir: &str, build: &str) -> String {
        format!(
            "\"AppState\"\n{{\n\t\"appid\"\t\t\"2054970\"\n\t\"installdir\"\t\t\"{install_dir}\"\n\t\"buildid\"\t\t\"{build}\"\n}}\n"
        )
    }

    #[test]
    fn finds_the_game_in_a_second_library_that_lists_the_app() {
        let dir = tempfile::tempdir().unwrap();
        let steam = dir.path().join("Steam");
        let library = dir.path().join("SteamLibrary");
        let vdf = format!(
            "\"libraryfolders\"\n{{\n\t\"0\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t\t\"apps\"\n\t\t{{\n\t\t\t\"228980\"\t\t\"1\"\n\t\t}}\n\t}}\n\t\"1\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t\t\"apps\"\n\t\t{{\n\t\t\t\"2054970\"\t\t\"2\"\n\t\t}}\n\t}}\n}}\n",
            steam.display().to_string().replace('\\', "\\\\"),
            library.display().to_string().replace('\\', "\\\\"),
        );
        write(&steam.join("steamapps/libraryfolders.vdf"), &vdf);
        write(
            &library.join("steamapps/appmanifest_2054970.acf"),
            &manifest("Dragons Dogma 2", "25290080"),
        );
        write(
            &library.join("steamapps/common/Dragons Dogma 2/DD2.exe"),
            "",
        );

        let found = discover_game_from(Some(&steam)).expect("the game should be found");
        assert_eq!(found.root, library.join("steamapps/common/Dragons Dogma 2"));
        assert_eq!(found.build_id.as_deref(), Some("25290080"));
        assert_eq!(found.source, GameSource::Steam);
        assert_eq!(read_build_id(&found.root).as_deref(), Some("25290080"));
    }

    #[test]
    fn the_steam_root_is_a_library_even_when_its_vdf_path_is_foreign() {
        let dir = tempfile::tempdir().unwrap();
        let steam = dir.path().join("Steam");
        write(
            &steam.join("SteamApps/libraryfolders.vdf"),
            "\"libraryfolders\" { \"0\" { \"path\" \"Q:\\\\Nowhere\\\\Steam\" \"apps\" { \"2054970\" \"1\" } } }",
        );
        write(
            &steam.join("SteamApps/appmanifest_2054970.acf"),
            &manifest("Dragons Dogma 2", "1"),
        );
        // Older installs spell the folder SteamApps; the exe may be upper case.
        write(&steam.join("SteamApps/common/Dragons Dogma 2/dd2.EXE"), "");
        let found = discover_game_from(Some(&steam)).expect("found through the steam root");
        assert!(found.root.ends_with("Dragons Dogma 2"));
    }

    #[test]
    fn a_library_without_the_exe_is_not_a_game() {
        let dir = tempfile::tempdir().unwrap();
        let steam = dir.path().join("Steam");
        write(
            &steam.join("steamapps/appmanifest_2054970.acf"),
            &manifest("Dragons Dogma 2", "1"),
        );
        fs::create_dir_all(steam.join("steamapps/common/Dragons Dogma 2")).unwrap();
        assert!(discover_game_from(Some(&steam)).is_none());
        assert!(discover_game_from(None).is_none());
    }

    #[test]
    fn old_format_library_entries_are_read() {
        let dir = tempfile::tempdir().unwrap();
        let steam = dir.path().join("Steam");
        let library = dir.path().join("Old Library");
        write(
            &steam.join("steamapps/libraryfolders.vdf"),
            &format!(
                "\"LibraryFolders\" {{ \"TimeNextStatsReport\" \"1\" \"1\" \"{}\" }}",
                library.display().to_string().replace('\\', "\\\\")
            ),
        );
        write(
            &library.join("steamapps/appmanifest_2054970.acf"),
            &manifest("DD2", "7"),
        );
        write(&library.join("steamapps/common/DD2/DD2.exe"), "");
        let found = discover_game_from(Some(&steam)).unwrap();
        assert_eq!(found.build_id.as_deref(), Some("7"));
    }

    #[test]
    fn a_manual_pick_needs_the_exe() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            validate_game_root(dir.path()).unwrap_err(),
            RiftError::GameNotFound
        );
        assert!(matches!(
            validate_game_root(&dir.path().join("missing")),
            Err(RiftError::Unreadable { .. })
        ));
        fs::write(dir.path().join("DD2.exe"), b"").unwrap();
        let picked = validate_game_root(dir.path()).unwrap();
        assert_eq!(picked.source, GameSource::Manual);
        assert_eq!(picked.build_id, None);
    }

    #[test]
    fn windows_paths_translate_for_the_wsl_bench() {
        assert_eq!(
            windows_to_wsl(r"C:\Program Files (x86)\Steam"),
            Some(PathBuf::from("/mnt/c/Program Files (x86)/Steam"))
        );
        assert_eq!(
            windows_to_wsl("D:/SteamLibrary"),
            Some(PathBuf::from("/mnt/d/SteamLibrary"))
        );
        assert_eq!(windows_to_wsl("/home/x"), None);
    }
}
