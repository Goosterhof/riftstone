//! REFramework's own files, read — never written, except the one explicit
//! switch action ([`set_switch`]).
//!
//! The shapes, measured on the investor's install (REFramework v1.5.9.1+514,
//! October 2026):
//!
//! - `dinput8.dll` in the game root is REFramework itself.
//! - `re2_fw_config.txt` holds `Key=value` lines (CRLF):
//!   `LooseFileLoader_Enabled=true`, `IntegrityCheckBypass_LoadPakDirectory=true`.
//! - `re2_framework_log.txt` is an spdlog file rewritten at every launch
//!   (`[2026-10-14 21:34:14.972] [REFramework] [info] Tag: v1.5.9.1`). Its
//!   `[IntegrityCheckBypass]` lines say which `pak_mods/` paks were cached
//!   and injected, and into which patch slot.
//! - `reframework_faulty_files.txt` lists files the FaultyFileDetector caught;
//!   its log-shaped lines are headers, the other lines are paths.

use std::fs;
use std::path::Path;

use rift_core::RiftError;
use serde::{Deserialize, Serialize};

use crate::fsops::{self, child_ci, with_suffix, PART_SUFFIX};

/// REFramework itself.
pub const REFRAMEWORK_DLL: &str = "dinput8.dll";
/// REFramework's config, rewritten by REFramework when the game closes.
pub const CONFIG_FILE: &str = "re2_fw_config.txt";
/// REFramework's log of the current (or last) launch.
pub const LOG_FILE: &str = "re2_framework_log.txt";
/// The FaultyFileDetector's list.
pub const FAULTY_FILES: &str = "reframework_faulty_files.txt";
/// Where [`set_switch`] keeps the config as it was before the Riftstone
/// first changed it (written once, never overwritten by later flips).
pub const CONFIG_BACKUP_SUFFIX: &str = ".riftstone-backup";

/// What REFramework's files say about it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefStatus {
    /// `dinput8.dll` is present.
    pub installed: bool,
    /// `v1.5.9.1+514` — the log's `Tag:` plus `+` and `Commits past tag:`.
    pub version: Option<String>,
    /// The log's `Build date:` (REFramework writes `DD.MM.YYYY`).
    pub build_date: Option<String>,
    /// `LooseFileLoader_Enabled` — loose `natives/` files load only when true.
    /// REFramework's default when the key is absent: off.
    pub loose_file_loader: Option<bool>,
    /// `IntegrityCheckBypass_LoadPakDirectory` — `pak_mods/` loads unless
    /// false. REFramework's default when the key is absent: on.
    pub pak_directory: Option<bool>,
    /// Paths the FaultyFileDetector listed.
    pub faulty_files: Vec<String>,
}

impl RefStatus {
    /// Will `pak_mods/` load? (Absent key = REFramework's default, on.)
    pub fn paks_load(&self) -> bool {
        self.installed && self.pak_directory != Some(false)
    }

    /// Will loose `natives/` files load? (Absent key = off.)
    pub fn loose_files_load(&self) -> bool {
        self.installed && self.loose_file_loader == Some(true)
    }
}

/// The two switches the Riftstone may flip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RefSwitch {
    LooseFileLoader,
    PakDirectory,
}

impl RefSwitch {
    /// The key in `re2_fw_config.txt`.
    pub fn config_key(self) -> &'static str {
        match self {
            RefSwitch::LooseFileLoader => "LooseFileLoader_Enabled",
            RefSwitch::PakDirectory => "IntegrityCheckBypass_LoadPakDirectory",
        }
    }
}

/// Read REFramework's state from the game root. Missing files are simply
/// unknowns — this never fails.
pub fn read_status(game_root: &Path) -> RefStatus {
    let installed = child_ci(game_root, REFRAMEWORK_DLL).is_some_and(|p| p.is_file());
    let (version, build_date) = read_text(game_root, LOG_FILE)
        .map(|log| parse_version(&log))
        .unwrap_or((None, None));
    let config = read_text(game_root, CONFIG_FILE).unwrap_or_default();
    RefStatus {
        installed,
        version,
        build_date,
        loose_file_loader: config_bool(&config, RefSwitch::LooseFileLoader.config_key()),
        pak_directory: config_bool(&config, RefSwitch::PakDirectory.config_key()),
        faulty_files: read_text(game_root, FAULTY_FILES)
            .map(|text| parse_faulty_files(&text))
            .unwrap_or_default(),
    }
}

/// `Tag:` + `Commits past tag:` and `Build date:` from the log's first lines.
pub fn parse_version(log: &str) -> (Option<String>, Option<String>) {
    let mut tag = None;
    let mut commits = None;
    let mut build_date = None;
    for line in log.lines() {
        let message = log_message(line);
        if tag.is_none() {
            if let Some(v) = message.strip_prefix("Tag:") {
                tag = Some(v.trim().to_string()).filter(|v| !v.is_empty());
            }
        }
        if commits.is_none() {
            if let Some(v) = message.strip_prefix("Commits past tag:") {
                commits = Some(v.trim().to_string()).filter(|v| !v.is_empty());
            }
        }
        if build_date.is_none() {
            if let Some(v) = message.strip_prefix("Build date:") {
                build_date = Some(v.trim().to_string()).filter(|v| !v.is_empty());
            }
        }
        if tag.is_some() && commits.is_some() && build_date.is_some() {
            break;
        }
    }
    let version = tag.map(|tag| match commits.as_deref() {
        Some(n) if n != "0" => format!("{tag}+{n}"),
        _ => tag,
    });
    (version, build_date)
}

/// The value of `key` in a `Key=value` config (the last occurrence wins).
pub fn config_value(config: &str, key: &str) -> Option<String> {
    config
        .lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(k, _)| k.trim() == key)
        .map(|(_, v)| v.trim().to_string())
        .next_back()
}

fn config_bool(config: &str, key: &str) -> Option<bool> {
    match config_value(config, key)?.to_ascii_lowercase().as_str() {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

/// The FaultyFileDetector's paths: every non-empty line that is not an
/// spdlog line.
pub fn parse_faulty_files(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !is_log_line(line))
        .map(str::to_string)
        .collect()
}

/// Flip one switch in `re2_fw_config.txt`. Refused while the game runs —
/// REFramework rewrites its config on exit, so the change would be lost.
/// Before the first change the config is kept beside it as
/// `re2_fw_config.txt.riftstone-backup` (later flips never overwrite it, so
/// it always holds the player's own config); every other line (and the
/// file's line endings) stays byte for byte, and the new file replaces the
/// old one atomically. A missing config is created with the one line.
///
/// The config goes through the target guard (a config that is a link, or a
/// link on the way, is refused), and both temporary names are written only
/// as NEW files: a link or a file already wearing one of them is never
/// written through or over.
pub fn set_switch(
    game_root: &Path,
    key: RefSwitch,
    on: bool,
    game_running: bool,
) -> Result<(), RiftError> {
    if game_running {
        return Err(RiftError::refused(
            "Dragon's Dogma 2 is running, and REFramework rewrites re2_fw_config.txt when the game closes, so the change would be lost. Close the game, then flip the switch.",
        ));
    }
    let name = child_ci(game_root, CONFIG_FILE)
        .and_then(|found| found.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| CONFIG_FILE.to_string());
    let path = crate::guard::guard_target(game_root, &name)?;
    let existing = match fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => return Err(RiftError::unreadable(&path, err)),
    };
    let updated = rewrite_config(
        existing.as_deref().unwrap_or_default(),
        key.config_key(),
        on,
    );
    if existing.as_deref() == Some(updated.as_slice()) {
        return Ok(());
    }
    let part = with_suffix(&path, PART_SUFFIX);
    let part_taken = || {
        RiftError::refused(format!(
            "{} already sits in the game folder. The Riftstone writes the new config under that name first and never writes through or over a file it did not put there; delete or move it, then flip the switch again.",
            part.display()
        ))
    };
    if fsops::exists(&part) {
        return Err(part_taken());
    }
    if let Some(old) = &existing {
        let backup = with_suffix(&path, CONFIG_BACKUP_SUFFIX);
        match fs::symlink_metadata(&backup) {
            // The config as it was before the Riftstone first changed it.
            Ok(meta) if meta.is_file() && !fsops::is_link(&meta) => {}
            Ok(_) => {
                return Err(RiftError::refused(format!(
                    "{} is a link or a folder, and the Riftstone keeps the config's backup under that name. Remove it, then flip the switch again.",
                    backup.display()
                )));
            }
            Err(_) => fsops::write_new(&backup, old).map_err(|err| {
                RiftError::io(
                    format!("could not back up {} before changing it", path.display()),
                    err,
                )
            })?,
        }
    }
    if let Err(err) = fsops::write_new(&part, &updated) {
        return Err(if err.kind() == std::io::ErrorKind::AlreadyExists {
            part_taken()
        } else {
            RiftError::io(format!("could not write {}", path.display()), err)
        });
    }
    if let Err(err) = fs::rename(&part, &path) {
        let _ = fs::remove_file(&part);
        return Err(RiftError::io(
            format!("could not write {}", path.display()),
            err,
        ));
    }
    Ok(())
}

/// Replace (or append) `key=true|false`, byte-preserving everything else.
fn rewrite_config(original: &[u8], key: &str, on: bool) -> Vec<u8> {
    let value: &[u8] = if on { b"true" } else { b"false" };
    let newline: &[u8] = if original.is_empty() || original.windows(2).any(|w| w == b"\r\n") {
        b"\r\n"
    } else {
        b"\n"
    };
    let mut out = Vec::with_capacity(original.len() + key.len() + 8);
    let mut found = false;
    for line in original.split_inclusive(|&b| b == b'\n') {
        let body_end = line
            .iter()
            .rposition(|&b| b != b'\n' && b != b'\r')
            .map_or(0, |i| i + 1);
        let (body, ending) = line.split_at(body_end);
        let is_key = body
            .iter()
            .position(|&b| b == b'=')
            .is_some_and(|eq| body[..eq].trim_ascii() == key.as_bytes());
        if is_key {
            found = true;
            out.extend_from_slice(key.as_bytes());
            out.push(b'=');
            out.extend_from_slice(value);
            out.extend_from_slice(ending);
        } else {
            out.extend_from_slice(line);
        }
    }
    if !found {
        if !out.is_empty() && !out.ends_with(b"\n") {
            out.extend_from_slice(newline);
        }
        out.extend_from_slice(key.as_bytes());
        out.push(b'=');
        out.extend_from_slice(value);
        out.extend_from_slice(newline);
    }
    out
}

/// What REFramework's log says about `pak_mods/` at the last launch.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PakLoadReport {
    /// The first line's timestamp — when the game last started (local time,
    /// `2026-10-14 21:34:14.970`).
    pub log_started: Option<String>,
    /// File names REFramework found in `pak_mods/` ("Cached custom pak").
    pub cached: Vec<String>,
    /// Paks it injected, in injection order.
    pub injected: Vec<InjectedPak>,
    /// REFramework's "NONE were injected" line, verbatim, when present.
    pub error: Option<String>,
}

/// One pak REFramework injected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InjectedPak {
    pub file_name: String,
    /// The engine-side name it answers to (`re_chunk_000.pak.sub_000.pak.patch_001.pak`).
    pub native_name: String,
    pub slot: u32,
}

/// Read and parse `re2_framework_log.txt`; `None` when there is no log.
pub fn read_pak_load_report(game_root: &Path) -> Option<PakLoadReport> {
    read_text(game_root, LOG_FILE).map(|log| parse_pak_load_report(&log))
}

/// Parse REFramework's log (testable core of [`read_pak_load_report`]).
pub fn parse_pak_load_report(log: &str) -> PakLoadReport {
    const CACHED: &str = "[IntegrityCheckBypass]: Cached custom pak with name: ";
    const INJECTING: &str = "[IntegrityCheckBypass]: injecting ";
    let mut report = PakLoadReport {
        log_started: log.lines().find_map(log_timestamp),
        ..PakLoadReport::default()
    };
    for line in log.lines() {
        let line = line.trim_end();
        if let Some(at) = line.find(CACHED) {
            let rest = &line[at + CACHED.len()..];
            let name = match rest.split_once(" at path: ") {
                Some((name, _)) if !name.trim().is_empty() => name.trim().to_string(),
                Some((_, path)) => windows_file_name(path).to_string(),
                None => rest.trim().to_string(),
            };
            if !name.is_empty() {
                report.cached.push(name);
            }
        } else if let Some(at) = line.find(INJECTING) {
            let rest = &line[at + INJECTING.len()..];
            let Some((path, after)) = rest.split_once(" as native-form '") else {
                continue;
            };
            let Some((native, tail)) = after.split_once('\'') else {
                continue;
            };
            let slot = tail
                .split_once("slot=")
                .and_then(|(_, s)| {
                    let digits: String = s.chars().take_while(char::is_ascii_digit).collect();
                    digits.parse().ok()
                })
                .unwrap_or(0);
            report.injected.push(InjectedPak {
                file_name: windows_file_name(path).to_string(),
                native_name: native.to_string(),
                slot,
            });
        }
        if line.contains("NONE were injected") && report.error.is_none() {
            report.error = Some(log_message(line).to_string());
        }
    }
    report
}

/// The last segment of a path written with either separator.
fn windows_file_name(path: &str) -> &str {
    path.trim().rsplit(['\\', '/']).next().unwrap_or(path)
}

/// `[2026-10-14 21:34:14.970] [REFramework] [info] Tag: v1.5.9.1` → `Tag: v1.5.9.1`.
/// Strips up to three leading `[…]` groups (time, logger, level).
fn log_message(line: &str) -> &str {
    let mut rest = line.trim();
    for _ in 0..3 {
        if !rest.starts_with('[') {
            break;
        }
        match rest.find(']') {
            Some(end) => rest = rest[end + 1..].trim_start(),
            None => break,
        }
    }
    rest
}

/// The timestamp of an spdlog line, when it starts with one.
fn log_timestamp(line: &str) -> Option<String> {
    let rest = line.trim_start().strip_prefix('[')?;
    let (stamp, _) = rest.split_once(']')?;
    let looks_dated = stamp.len() >= 10
        && stamp.as_bytes()[..4].iter().all(u8::is_ascii_digit)
        && stamp.as_bytes()[4] == b'-';
    looks_dated.then(|| stamp.to_string())
}

fn is_log_line(line: &str) -> bool {
    log_timestamp(line).is_some()
}

fn read_text(game_root: &Path, name: &str) -> Option<String> {
    let path = child_ci(game_root, name)?;
    let bytes = fs::read(path).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "[2026-10-14 21:34:14.970] [REFramework] [info] Previous log preserved as C:\\Games\\DD2\\re2_framework_log.prev.txt\r\n\
[2026-10-14 21:34:14.972] [REFramework] [info] Commit hash: d052e8fd3fd41d0a1a33394fcf6e6c19be39b1ee\r\n\
[2026-10-14 21:34:14.972] [REFramework] [info] Tag: v1.5.9.1\r\n\
[2026-10-14 21:34:14.972] [REFramework] [info] Commits past tag: 514\r\n\
[2026-10-14 21:34:14.972] [REFramework] [info] Build date: 11.10.2026\r\n\
[2026-10-14 21:34:19.897] [REFramework] [info] [IntegrityCheckBypass]: Cached custom pak with name: x0000-Lantern Glow Recolor.pak at path: C:\\Games\\DD2\\pak_mods\\x0000-Lantern Glow Recolor.pak\r\n\
[2026-10-14 21:34:19.897] [REFramework] [info] [IntegrityCheckBypass]: Cached custom pak with name: zrift-0000-Vernworth Cloaks.pak at path: C:\\Games\\DD2\\pak_mods\\zrift-0000-Vernworth Cloaks.pak\r\n\
[2026-10-14 21:34:29.301] [REFramework] [info] [IntegrityCheckBypass]: injecting C:\\Games\\DD2\\pak_mods\\x0000-Lantern Glow Recolor.pak as native-form 're_chunk_000.pak.sub_000.pak.patch_001.pak' (a6=1, slot=103, auto-assigned)\r\n\
[2026-10-14 21:34:29.319] [REFramework] [info] [IntegrityCheckBypass]: redirecting open of 're_chunk_000.pak.sub_000.pak.patch_001.pak' -> 'C:\\Games\\DD2\\pak_mods\\x0000-Lantern Glow Recolor.pak'\r\n\
[2026-10-14 21:34:29.327] [REFramework] [info] [IntegrityCheckBypass]: injecting C:\\Games\\DD2\\pak_mods\\zrift-0000-Vernworth Cloaks.pak as native-form 're_chunk_000.pak.sub_000.pak.patch_002.pak' (a6=2, slot=104, auto-assigned)\r\n";

    fn game_with(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (name, body) in files {
            fs::write(dir.path().join(name), body).unwrap();
        }
        dir
    }

    #[test]
    fn status_reads_the_dll_the_log_the_config_and_the_faulty_list() {
        let dir = game_with(&[
            ("dinput8.dll", "MZ"),
            ("re2_framework_log.txt", LOG),
            (
                "re2_fw_config.txt",
                "Camera_Enabled=false\r\nIntegrityCheckBypass_LoadPakDirectory=true\r\nLooseFileLoader_Enabled=false\r\n",
            ),
            (
                "reframework_faulty_files.txt",
                "[2026-10-14 21:34:18.781] [FaultyFileDetector] [info] FaultyFileDetector constructed\r\nnatives/stm/a/broken.mesh.260421070\r\n\r\nnatives/stm/b/broken.tex.251211553\r\n",
            ),
        ]);
        let status = read_status(dir.path());
        assert!(status.installed);
        assert_eq!(status.version.as_deref(), Some("v1.5.9.1+514"));
        assert_eq!(status.build_date.as_deref(), Some("11.10.2026"));
        assert_eq!(status.pak_directory, Some(true));
        assert_eq!(status.loose_file_loader, Some(false));
        assert_eq!(
            status.faulty_files,
            [
                "natives/stm/a/broken.mesh.260421070",
                "natives/stm/b/broken.tex.251211553"
            ]
        );
        assert!(status.paks_load());
        assert!(!status.loose_files_load());
    }

    #[test]
    fn a_bare_folder_has_no_reframework_and_defaults_apply() {
        let dir = game_with(&[]);
        let status = read_status(dir.path());
        assert_eq!(status, RefStatus::default());
        let with_dll = game_with(&[("DINPUT8.DLL", "MZ")]);
        let status = read_status(with_dll.path());
        assert!(status.installed, "the dll is found in any case");
        assert!(status.paks_load(), "an absent key means pak_mods loads");
        assert!(
            !status.loose_files_load(),
            "an absent key means loose files do not"
        );
    }

    #[test]
    fn a_tag_without_commits_is_the_version() {
        let (version, date) = parse_version(
            "[t] [REFramework] [info] Tag: v1.5.9\n[t] [x] [info] Commits past tag: 0\n",
        );
        assert_eq!(version.as_deref(), Some("v1.5.9"));
        assert_eq!(date, None);
    }

    #[test]
    fn the_pak_load_report_reads_cached_injected_and_the_start() {
        let report = parse_pak_load_report(LOG);
        assert_eq!(
            report.log_started.as_deref(),
            Some("2026-10-14 21:34:14.970")
        );
        assert_eq!(
            report.cached,
            [
                "x0000-Lantern Glow Recolor.pak",
                "zrift-0000-Vernworth Cloaks.pak"
            ]
        );
        assert_eq!(report.injected.len(), 2);
        assert_eq!(
            report.injected[0].file_name,
            "x0000-Lantern Glow Recolor.pak"
        );
        assert_eq!(
            report.injected[0].native_name,
            "re_chunk_000.pak.sub_000.pak.patch_001.pak"
        );
        assert_eq!(report.injected[0].slot, 103);
        assert_eq!(report.injected[1].slot, 104);
        assert_eq!(report.error, None);
    }

    #[test]
    fn the_pak_load_report_keeps_the_none_injected_line() {
        let log = "[2026-10-14 21:34:14.970] [REFramework] [info] REFramework entry\n[2026-10-14 21:34:29.333] [REFramework] [error] [IntegrityCheckBypass]: Found 2 custom paks but NONE were injected!\n";
        let report = parse_pak_load_report(log);
        assert_eq!(
            report.error.as_deref(),
            Some("[IntegrityCheckBypass]: Found 2 custom paks but NONE were injected!")
        );
        assert!(report.injected.is_empty());
        let dir = game_with(&[]);
        assert!(read_pak_load_report(dir.path()).is_none());
    }

    #[test]
    fn set_switch_is_refused_while_the_game_runs() {
        let dir = game_with(&[("re2_fw_config.txt", "LooseFileLoader_Enabled=false\r\n")]);
        let err = set_switch(dir.path(), RefSwitch::LooseFileLoader, true, true).unwrap_err();
        assert!(matches!(err, RiftError::Refused { .. }));
        assert_eq!(
            fs::read_to_string(dir.path().join("re2_fw_config.txt")).unwrap(),
            "LooseFileLoader_Enabled=false\r\n"
        );
    }

    #[test]
    fn set_switch_changes_one_line_backs_up_and_keeps_the_rest() {
        let original = "Camera_FOV=81.000000\r\nLooseFileLoader_Enabled=false\r\nLooseFileLoader_LogLooseFiles=false\r\n";
        let dir = game_with(&[("re2_fw_config.txt", original)]);
        set_switch(dir.path(), RefSwitch::LooseFileLoader, true, false).unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("re2_fw_config.txt")).unwrap(),
            "Camera_FOV=81.000000\r\nLooseFileLoader_Enabled=true\r\nLooseFileLoader_LogLooseFiles=false\r\n"
        );
        assert_eq!(
            fs::read_to_string(dir.path().join("re2_fw_config.txt.riftstone-backup")).unwrap(),
            original
        );
        // The other switch is appended with the file's own line ending.
        set_switch(dir.path(), RefSwitch::PakDirectory, false, false).unwrap();
        let text = fs::read_to_string(dir.path().join("re2_fw_config.txt")).unwrap();
        assert!(
            text.ends_with("IntegrityCheckBypass_LoadPakDirectory=false\r\n"),
            "{text}"
        );
        assert_eq!(read_status(dir.path()).pak_directory, Some(false));
        // No leftovers.
        assert!(!dir.path().join("re2_fw_config.txt.riftstone-part").exists());
    }

    #[test]
    fn set_switch_creates_a_missing_config() {
        let dir = game_with(&[]);
        set_switch(dir.path(), RefSwitch::PakDirectory, true, false).unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("re2_fw_config.txt")).unwrap(),
            "IntegrityCheckBypass_LoadPakDirectory=true\r\n"
        );
        assert!(!dir
            .path()
            .join("re2_fw_config.txt.riftstone-backup")
            .exists());
    }

    #[test]
    fn set_switch_keeps_the_config_as_it_was_before_the_first_change() {
        let original =
            "LooseFileLoader_Enabled=false\r\nIntegrityCheckBypass_LoadPakDirectory=true\r\n";
        let dir = game_with(&[("re2_fw_config.txt", original)]);
        set_switch(dir.path(), RefSwitch::LooseFileLoader, true, false).unwrap();
        set_switch(dir.path(), RefSwitch::PakDirectory, false, false).unwrap();
        set_switch(dir.path(), RefSwitch::LooseFileLoader, false, false).unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("re2_fw_config.txt.riftstone-backup")).unwrap(),
            original,
            "later flips never overwrite the player's own config"
        );
    }

    #[cfg(unix)]
    #[test]
    fn set_switch_never_writes_through_a_link() {
        let config = "LooseFileLoader_Enabled=false\r\n";
        let dir = game_with(&[("re2_fw_config.txt", config)]);
        let outside = tempfile::tempdir().unwrap();
        let victim = outside.path().join("victim.txt");
        fs::write(&victim, "the victim").unwrap();
        for name in [
            "re2_fw_config.txt.riftstone-backup",
            "re2_fw_config.txt.riftstone-part",
        ] {
            let link = dir.path().join(name);
            std::os::unix::fs::symlink(&victim, &link).unwrap();
            let result = set_switch(dir.path(), RefSwitch::LooseFileLoader, true, false);
            assert!(
                matches!(result, Err(RiftError::Refused { .. })),
                "{name}: {result:?}"
            );
            assert_eq!(fs::read_to_string(&victim).unwrap(), "the victim", "{name}");
            assert_eq!(
                fs::read_to_string(dir.path().join("re2_fw_config.txt")).unwrap(),
                config,
                "{name}"
            );
            fs::remove_file(&link).unwrap();
            let _ = fs::remove_file(dir.path().join("re2_fw_config.txt.riftstone-backup"));
        }
        // A config that is itself a link is not written through either.
        fs::remove_file(dir.path().join("re2_fw_config.txt")).unwrap();
        std::os::unix::fs::symlink(&victim, dir.path().join("re2_fw_config.txt")).unwrap();
        assert!(set_switch(dir.path(), RefSwitch::PakDirectory, false, false).is_err());
        assert_eq!(fs::read_to_string(&victim).unwrap(), "the victim");
    }

    #[test]
    fn rewriting_preserves_lf_files_and_unterminated_last_lines() {
        let out = rewrite_config(
            b"A=1\nLooseFileLoader_Enabled = false",
            "LooseFileLoader_Enabled",
            true,
        );
        assert_eq!(out, b"A=1\nLooseFileLoader_Enabled=true");
        let out = rewrite_config(b"A=1", "B", false);
        assert_eq!(out, b"A=1\nB=false\n");
    }
}
