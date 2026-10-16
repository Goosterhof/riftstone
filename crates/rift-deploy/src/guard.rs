//! The target guard: every path the deploy engine writes, moves or deletes
//! in the game folder passes through here first.
//!
//! The only writable places are `natives/**`, `reframework/**`,
//! `pak_mods/*.pak` (flat, lowercase `.pak`) and files directly in the game
//! root. On top of that, refused outright whatever a mod says:
//! [`rift_core::dd2::NEVER_WRITE`] (`DD2.exe`, `re_chunk_000.pak` and every
//! `re_chunk_000.pak.*`, `dlc/`, `_storage_/`, `steam_api64.dll`,
//! `steam_appid.txt`, `shader.cache2`, `config.ini`, `config_default.ini`),
//! any `.exe` or `.pak` directly in the root, names Windows would bend into
//! another file (trailing dots or spaces, device names, short 8.3 aliases),
//! and names carrying the engine's own temporary marker (`.riftstone-`).
//!
//! The name rules are only as good as the path they judge, so the disk check
//! makes the name the truth: every folder that already exists on the way from
//! the game root to the target must be a real folder — not a link, not a
//! junction, not a file — and must resolve to exactly the folders it names.
//! A link inside the game folder could otherwise lead `natives/x/` onto
//! `re_chunk_000.pak` while the name looked harmless; the flat layout mods
//! use never needs one.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use rift_core::dd2::{NEVER_WRITE, PAK_MODS_DIR};
use rift_core::{lower_key, normalize_entry_path, RiftError};

use crate::fsops::{is_link, join_rel};
use crate::journal::DeployKind;
use crate::paks::PAK_EXTENSION;

/// The marker in every temporary name the engine writes beside a game file
/// (`.riftstone-part`, `.riftstone-trash-<n>`, `.riftstone-reorder-<n>`,
/// `.riftstone-backup`). A mod may not use it: those names are the engine's.
pub const RESERVED_MARKER: &str = ".riftstone-";

const DEVICE_NAMES: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Names a root FILE may not take: they are the game's (or REFramework's)
/// folders.
const FOLDER_NAMES: &[&str] = &["natives", "reframework", PAK_MODS_DIR, "dlc", "_storage_"];

const WINDOWS_ILLEGAL: &[char] = &['<', '>', '"', '|', '?', '*'];

/// Check a game path against the guard's rules. Returns it normalized
/// (`/` separators, original case). Purely by name — no disk access.
pub fn check_game_path(game_path: &str) -> Result<String, RiftError> {
    let normalized = normalize_entry_path(game_path).map_err(|_| {
        RiftError::refused(format!(
            "The path \"{game_path}\" tries to leave the game folder (a \"..\", a drive letter or a \":\"), so the Riftstone refused to write it. The archive is unsafe; do not install it."
        ))
    })?;
    if normalized.is_empty() {
        return Err(RiftError::refused(
            "An empty path cannot be written into the game folder.",
        ));
    }
    let segments: Vec<&str> = normalized.split('/').collect();
    for segment in &segments {
        if segment.ends_with('.') || segment.ends_with(' ') {
            return Err(RiftError::refused(format!(
                "\"{normalized}\" has a name ending in a dot or a space, which Windows silently drops, so it could land on a different file. The Riftstone refused it; the mod needs repacking."
            )));
        }
        if segment.contains(WINDOWS_ILLEGAL) {
            return Err(RiftError::refused(format!(
                "\"{normalized}\" contains a character Windows does not allow in file names (< > \" | ? *). The Riftstone refused it; the mod needs repacking."
            )));
        }
        let stem = lower_key(segment.split('.').next().unwrap_or(segment));
        if DEVICE_NAMES.contains(&stem.trim_end()) {
            return Err(RiftError::refused(format!(
                "\"{normalized}\" uses the reserved Windows device name \"{segment}\", which cannot be a file. The Riftstone refused it; the mod needs repacking."
            )));
        }
        if lower_key(segment).contains(RESERVED_MARKER) {
            return Err(RiftError::refused(format!(
                "\"{normalized}\" carries \"{RESERVED_MARKER}\" in its name, which the Riftstone keeps for its own temporary files. The Riftstone refused it; the mod needs repacking."
            )));
        }
    }
    let lower = lower_key(&normalized);
    for rule in NEVER_WRITE {
        let hit = match rule.strip_suffix('/') {
            Some(folder) => lower == folder || lower.starts_with(rule),
            None => lower == *rule || lower.starts_with(&format!("{rule}.")),
        };
        if hit {
            return Err(RiftError::refused(format!(
                "\"{normalized}\" belongs to Dragon's Dogma 2 itself (the game, its data paks, its DLC, Steam's files or REFramework's cache). The Riftstone never writes there; mods live in natives/, reframework/ and pak_mods/."
            )));
        }
    }
    let first = lower_key(segments[0]);
    if segments.len() == 1 {
        return check_root_file(&normalized, &lower);
    }
    match first.as_str() {
        "natives" | "reframework" => Ok(normalized),
        PAK_MODS_DIR => {
            if segments.len() == 2 && segments[1].ends_with(PAK_EXTENSION) && segments[1].len() > PAK_EXTENSION.len() {
                Ok(normalized)
            } else {
                Err(RiftError::refused(format!(
                    "\"{normalized}\" is not a .pak directly inside pak_mods/. REFramework loads pak_mods/ in file-name order and walks into subfolders, so the Riftstone keeps it flat and writes only lowercase .pak files there."
                )))
            }
        }
        _ => Err(RiftError::refused(format!(
            "\"{normalized}\" is outside the places mods live (natives/, reframework/, pak_mods/ and files directly in the game folder), so the Riftstone refused to write it."
        ))),
    }
}

fn check_root_file(normalized: &str, lower: &str) -> Result<String, RiftError> {
    if lower.ends_with(".exe") {
        return Err(RiftError::refused(format!(
            "\"{normalized}\" is a program in the game folder. The Riftstone never writes .exe files there."
        )));
    }
    if lower.ends_with(PAK_EXTENSION) {
        return Err(RiftError::refused(format!(
            "\"{normalized}\" is a pak aimed at the game folder itself. Paks load from pak_mods/ only, and the Riftstone never puts one in the game root."
        )));
    }
    if normalized.contains('~') {
        return Err(RiftError::refused(format!(
            "\"{normalized}\" looks like a Windows short (8.3) name, which can stand for a different file in the game folder. The Riftstone refused it."
        )));
    }
    if FOLDER_NAMES.contains(&lower) {
        return Err(RiftError::refused(format!(
            "\"{normalized}\" would be a file named like one of the game's folders. The Riftstone refused it."
        )));
    }
    Ok(normalized.to_string())
}

/// Does the payload's declared kind agree with where the path points?
pub fn check_kind(kind: DeployKind, normalized: &str) -> Result<(), RiftError> {
    let mut segments = normalized.split('/');
    let first = lower_key(segments.next().unwrap_or(""));
    let deeper = segments.next().is_some();
    let ok = match kind {
        DeployKind::Loose => first == "natives" && deeper,
        DeployKind::Reframework => first == "reframework" && deeper,
        DeployKind::Pak => first == PAK_MODS_DIR && deeper,
        DeployKind::Root => !deeper,
    };
    if ok {
        Ok(())
    } else {
        let place = match kind {
            DeployKind::Loose => "natives/",
            DeployKind::Reframework => "reframework/",
            DeployKind::Pak => "pak_mods/",
            DeployKind::Root => "the game folder itself",
        };
        Err(RiftError::refused(format!(
            "The mod marks \"{normalized}\" as a {} file, but that kind belongs in {place}. The Riftstone refused it.",
            kind_word(kind)
        )))
    }
}

fn kind_word(kind: DeployKind) -> &'static str {
    match kind {
        DeployKind::Loose => "loose",
        DeployKind::Reframework => "REFramework",
        DeployKind::Pak => "pak",
        DeployKind::Root => "game-folder",
    }
}

/// The name checks plus the disk: every folder on the way that already
/// exists must be a real folder (no link, no junction, no file in its place)
/// that resolves to exactly the folders the path names — so the name rules
/// above judged the place the bytes really land — and what already sits at
/// the target must be a plain file answering to this very name. Returns the
/// absolute path.
pub fn guard_target(game_root: &Path, game_path: &str) -> Result<PathBuf, RiftError> {
    let normalized = check_game_path(game_path)?;
    let target = join_rel(game_root, &normalized);
    let canonical_root = game_root.canonicalize().map_err(|err| {
        RiftError::refused(format!(
            "The game folder {} could not be resolved ({err}), so the Riftstone cannot check where files would land.",
            game_root.display()
        ))
    })?;
    let segments: Vec<&str> = normalized.split('/').collect();
    let folders = &segments[..segments.len() - 1];
    let mut dir = game_root.to_path_buf();
    let mut existing = 0usize;
    for segment in folders {
        dir.push(segment);
        match fs::symlink_metadata(&dir) {
            Ok(meta) if is_link(&meta) => {
                return Err(RiftError::refused(format!(
                    "\"{normalized}\" would be written through {}, which is a link or junction. The Riftstone never writes through links inside the game folder — they can lead onto the game's own files. Replace it with a real folder, then try again.",
                    dir.display()
                )));
            }
            Ok(meta) if !meta.is_dir() => {
                return Err(RiftError::refused(format!(
                    "A file sits where the folder {} should be, so \"{normalized}\" cannot be written there. Move that file out of the game folder, then try again.",
                    dir.display()
                )));
            }
            Ok(_) => existing += 1,
            Err(err) if err.kind() == io::ErrorKind::NotFound => break,
            Err(err) => {
                return Err(RiftError::refused(format!(
                    "{} could not be checked ({err}), so the Riftstone cannot tell where \"{normalized}\" would land.",
                    dir.display()
                )));
            }
        }
    }
    if existing > 0 {
        let deepest = join_rel(game_root, &folders[..existing].join("/"));
        let resolved = deepest.canonicalize().map_err(|err| {
            RiftError::refused(format!(
                "{} could not be resolved ({err}), so the Riftstone cannot check where \"{normalized}\" would land.",
                deepest.display()
            ))
        })?;
        let lands: Option<Vec<String>> = resolved.strip_prefix(&canonical_root).ok().map(|rel| {
            rel.components()
                .map(|c| lower_key(&c.as_os_str().to_string_lossy()))
                .collect()
        });
        let named: Vec<String> = folders[..existing].iter().map(|s| lower_key(s)).collect();
        if lands.as_ref() != Some(&named) {
            return Err(RiftError::refused(format!(
                "\"{normalized}\" would land in {}, not in the folders its name gives, so the Riftstone refused to write it.",
                resolved.display()
            )));
        }
    }
    if let Ok(meta) = fs::symlink_metadata(&target) {
        if is_link(&meta) {
            return Err(RiftError::refused(format!(
                "\"{normalized}\" is a link to somewhere else, so the Riftstone will not write through it."
            )));
        }
        if meta.is_dir() {
            return Err(RiftError::refused(format!(
                "A folder already sits where the file \"{normalized}\" should go, so the Riftstone cannot place it."
            )));
        }
        // Windows answers an 8.3 short name or a case variant with the real
        // file; the real name must be the one asked for.
        if let Ok(real) = target.canonicalize() {
            let real_name = real
                .file_name()
                .map(|n| lower_key(&n.to_string_lossy()))
                .unwrap_or_default();
            let asked = lower_key(rift_core::path::file_name(&normalized));
            if real_name != asked {
                return Err(RiftError::refused(format!(
                    "\"{normalized}\" opens a file with a different name on disk, so the Riftstone refused to write through it."
                )));
            }
        }
    }
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refused(path: &str) -> bool {
        matches!(check_game_path(path), Err(RiftError::Refused { .. }))
    }

    #[test]
    fn the_places_mods_live_are_allowed() {
        for ok in [
            "natives/stm/a.mesh",
            "NATIVES/STM/a.mesh",
            r"natives\stm\a.mesh",
            "reframework/autorun/lantern_glow.lua",
            "reframework/plugins/x.dll",
            "pak_mods/zrift-0000-Lantern Glow.pak",
            "dinput8.dll",
            "re2_fw_config.txt",
            "lantern_glow.ini",
        ] {
            assert!(check_game_path(ok).is_ok(), "{ok} should be allowed");
        }
        assert_eq!(
            check_game_path(r"natives\STM\a.mesh").unwrap(),
            "natives/STM/a.mesh"
        );
    }

    #[test]
    fn vanilla_is_refused_outright() {
        for bad in [
            "DD2.exe",
            "dd2.EXE",
            "re_chunk_000.pak",
            "RE_CHUNK_000.PAK",
            "re_chunk_000.pak.patch_016.pak",
            "re_chunk_000.pak.sub_000.pak.patch_001.pak",
            "dlc/re_dlc_stm_2581000.pak",
            "DLC/x.pak",
            "dlc",
            "_storage_/AkConvolutionReverb.dll",
            "steam_api64.dll",
            "steam_appid.txt",
            "shader.cache2",
            "config.ini",
            "Config_Default.ini",
        ] {
            assert!(refused(bad), "{bad} must be refused");
        }
    }

    #[test]
    fn everything_outside_the_mod_places_is_refused() {
        for bad in [
            "../outside.dll",
            "natives/../../outside.dll",
            "C:/Windows/evil.dll",
            "file.txt:stream",
            "",
            "./",
            "CrashReport.exe",
            "other.exe",
            "mod.pak",
            "pak_mods/sub/x.pak",
            "pak_mods/x.PAK",
            "pak_mods/readme.txt",
            "pak_mods/.pak",
            "bin/x.dll",
            "D3D12/x.dll",
            "natives",
            "pak_mods",
            "DD2.exe.",
            "dinput8.dll ",
            "natives /stm/a.mesh",
            "natives/stm/con.mesh",
            "NUL",
            "natives/stm/a?.mesh",
            "STEAM_~1.DLL",
            "natives/stm/a.mesh.riftstone-part",
            "natives/stm/a.mesh.RIFTSTONE-TRASH-3",
            "pak_mods/zrift-0000-a.pak.riftstone-reorder-0",
            "re2_fw_config.txt.riftstone-backup",
        ] {
            assert!(refused(bad), "{bad:?} must be refused");
        }
    }

    #[test]
    fn kinds_must_point_where_they_belong() {
        assert!(check_kind(DeployKind::Loose, "natives/stm/a.mesh").is_ok());
        assert!(check_kind(DeployKind::Loose, "reframework/autorun/a.lua").is_err());
        assert!(check_kind(DeployKind::Reframework, "reframework/autorun/a.lua").is_ok());
        assert!(check_kind(DeployKind::Root, "dinput8.dll").is_ok());
        assert!(check_kind(DeployKind::Root, "natives/a.dll").is_err());
        assert!(check_kind(DeployKind::Pak, "pak_mods/zrift-0000-a.pak").is_ok());
        assert!(check_kind(DeployKind::Pak, "natives/a.pak").is_err());
    }

    #[test]
    fn the_disk_check_keeps_writes_inside_the_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("game");
        fs::create_dir_all(root.join("natives/stm")).unwrap();
        let target = guard_target(&root, "natives/stm/a.mesh").unwrap();
        assert_eq!(target, root.join("natives/stm/a.mesh"));
        // A folder where a file should go.
        fs::create_dir_all(root.join("natives/stm/b.mesh")).unwrap();
        assert!(matches!(
            guard_target(&root, "natives/stm/b.mesh"),
            Err(RiftError::Refused { .. })
        ));
        // A file where a folder should be.
        fs::write(root.join("natives/stm/c"), b"a file").unwrap();
        assert!(matches!(
            guard_target(&root, "natives/stm/c/d.mesh"),
            Err(RiftError::Refused { .. })
        ));
        // Folders that do not exist yet are fine.
        assert!(guard_target(&root, "natives/stm/new/deeper/e.mesh").is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn a_link_that_stays_inside_the_game_folder_is_refused_too() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("game");
        fs::create_dir_all(root.join("natives")).unwrap();
        fs::write(root.join("re_chunk_000.pak"), b"KPKA").unwrap();
        // natives/vault -> the game root: lexically a loose file, really the
        // game's own data pak.
        std::os::unix::fs::symlink(&root, root.join("natives/vault")).unwrap();
        let err = guard_target(&root, "natives/vault/re_chunk_000.pak").unwrap_err();
        assert!(err.to_string().contains("link"), "{err}");
        // A deeper path through it is refused as well.
        assert!(guard_target(&root, "natives/vault/dlc/x.pak").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_link_out_of_the_game_folder_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("game");
        let outside = dir.path().join("outside");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(outside.join("stm")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("natives")).unwrap();
        let err = guard_target(&root, "natives/stm/a.mesh").unwrap_err();
        assert!(matches!(err, RiftError::Refused { .. }), "{err}");
        // A file that is itself a link is refused too.
        fs::create_dir_all(root.join("reframework/autorun")).unwrap();
        fs::write(outside.join("real.lua"), b"--").unwrap();
        std::os::unix::fs::symlink(
            outside.join("real.lua"),
            root.join("reframework/autorun/x.lua"),
        )
        .unwrap();
        assert!(guard_target(&root, "reframework/autorun/x.lua").is_err());
    }
}
