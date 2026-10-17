//! Play (v0.3): how the stone starts Dragon's Dogma 2.
//!
//! [`plan_launch`] is pure (it reads the game folder's listing, writes
//! nothing): the route Play takes, what may keep mods from loading
//! (warnings, never blockers) and, in one sentence, why Play cannot start now.
//! The spine opens the Steam route itself, through the shell; [`start_exe`]
//! runs the exe route.
//!
//! - **Route.** Steam when the game was found through Steam: Steam starts if
//!   it has to and applies the player's launch options, and REFramework's
//!   `dinput8.dll` loads from the game folder either way. Otherwise `DD2.exe`
//!   as it is spelled on disk, with the game folder as its working directory.
//! - **Refused** (in this order): the game folder holds no `DD2.exe`; the game
//!   already runs; a change is being made right now (an install, uninstall,
//!   reorder, switch or restore holds the game folder); a change was cut short
//!   (the journal holds a pending intent).
//! - **Warnings**: paks installed and REFramework missing; REFramework's pak
//!   folder switch off while paks are installed; its loose file loader off
//!   while loose files are installed; Steam updated the game since the newest
//!   install.

use std::fs;
use std::path::{Path, PathBuf};

use rift_core::error::RiftResult;
use rift_core::{lower_key, RiftError};
use serde::Serialize;

use crate::game::{GameInstall, GameSource, DD2_STEAM_APP_ID, GAME_EXE};
use crate::journal::{DeployKind, Journal};
use crate::reframework::RefStatus;
use crate::state::GameState;

/// The sentence for Play when no game was found at all, so there is no
/// [`GameInstall`] to plan with (the spine says it; [`plan_launch`] cannot).
pub const REFUSED_NO_GAME: &str =
    "Dragon's Dogma 2 was not found, so pick the game folder before you play.";

/// How Play goes, decided before it starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchPlan {
    pub route: LaunchRoute,
    /// What may keep mods from loading. Never blockers: Play starts anyway.
    /// They stand whether or not Play is refused right now.
    pub warnings: Vec<LaunchWarning>,
    /// One sentence when Play cannot start now (what stops it, and what to
    /// do); `None` when it can.
    pub refused: Option<String>,
}

/// The way Play starts the game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LaunchRoute {
    /// Open `url` (`steam://rungameid/2054970`) through the shell.
    Steam { url: String },
    /// Start `exe` with `cwd`, the game folder, as its working directory
    /// ([`start_exe`]).
    Exe { exe: PathBuf, cwd: PathBuf },
}

impl LaunchRoute {
    /// The route's name on the wire (`LaunchRoute` in `src/types/rift.ts`):
    /// `"steam"` or `"exe"`.
    pub fn kind(&self) -> &'static str {
        match self {
            LaunchRoute::Steam { .. } => "steam",
            LaunchRoute::Exe { .. } => "exe",
        }
    }
}

/// What may keep mods from loading once the game runs. The wire shape of
/// `LaunchWarning` in `src/types/rift.ts`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LaunchWarning {
    /// Installs put `paks` paks in `pak_mods/`, and REFramework's
    /// `dinput8.dll` is not in the game folder: they will not load.
    ReframeworkMissing { paks: usize },
    /// REFramework's pak folder switch is off: the `paks` installed paks will
    /// not load.
    PakFolderOff { paks: usize },
    /// REFramework's loose file loader is not on: the `files` installed loose
    /// files will not load.
    LooseLoaderOff { files: usize },
    /// Steam updated the game since the newest install recorded its build.
    BuildChanged { recorded: String, now: String },
}

/// Plan Play for `game`. `running` is the game watcher's word that DD2.exe
/// runs; `busy` that a change to the game folder is being made right now (the
/// deploy lock is held, or an install is under way). Pure: nothing is written.
pub fn plan_launch(
    game: &GameInstall,
    ref_status: &RefStatus,
    journal: &Journal,
    game_state: &GameState,
    running: bool,
    busy: bool,
) -> LaunchPlan {
    let exe = game_exe(game);
    let route = route_with(game, exe.as_deref());
    let refused = if exe.is_none() {
        Some(format!(
            "The game folder ({}) holds no {GAME_EXE} any more, so pick the game folder again or let Steam repair the game.",
            game.root.display()
        ))
    } else if running {
        Some(
            "Dragon's Dogma 2 is already running; switch to its window, or close it before you start it again."
                .to_string(),
        )
    } else if busy {
        Some(
            "The Riftstone is changing the game folder right now (an install, uninstall, reorder, switch or restore), so try Play again once that is done."
                .to_string(),
        )
    } else if journal.pending.is_some() {
        Some(
            "A change to the game folder was cut short, so restart the Riftstone to put the folder back the way it was, then play."
                .to_string(),
        )
    } else {
        None
    };
    LaunchPlan {
        route,
        warnings: launch_warnings(ref_status, journal, game_state),
        refused,
    }
}

/// The route Play takes for `game`, on its own (the window's
/// `GameSummary.launch` is its [`LaunchRoute::kind`]): Steam when Steam found
/// the game, else `DD2.exe` as it is spelled on disk (`<root>/DD2.exe` when
/// the folder holds none — [`plan_launch`] refuses then). Reads the game
/// folder's listing; writes nothing.
pub fn launch_route(game: &GameInstall) -> LaunchRoute {
    route_with(game, game_exe(game).as_deref())
}

/// The game's executable as it is spelled on disk, when it is a file. The
/// folder is read rather than asked: Windows answers `DD2.exe` for any
/// spelling, and Play is rare enough that one listing costs nothing.
fn game_exe(game: &GameInstall) -> Option<PathBuf> {
    let wanted = lower_key(GAME_EXE);
    fs::read_dir(&game.root)
        .ok()?
        .flatten()
        .find(|entry| lower_key(&entry.file_name().to_string_lossy()) == wanted)
        .map(|entry| entry.path())
        .filter(|path| fs::metadata(path).is_ok_and(|meta| meta.is_file()))
}

fn route_with(game: &GameInstall, exe: Option<&Path>) -> LaunchRoute {
    if game.source == GameSource::Steam {
        LaunchRoute::Steam {
            url: format!("steam://rungameid/{DD2_STEAM_APP_ID}"),
        }
    } else {
        LaunchRoute::Exe {
            exe: exe.map_or_else(|| game.root.join(GAME_EXE), Path::to_path_buf),
            cwd: game.root.clone(),
        }
    }
}

/// The warnings of the module docs. Counts come from the journal: the files
/// the installs (adopted ones too) put in the game folder.
fn launch_warnings(
    ref_status: &RefStatus,
    journal: &Journal,
    game_state: &GameState,
) -> Vec<LaunchWarning> {
    let count = |kind: DeployKind| {
        journal
            .installs
            .iter()
            .flat_map(|install| &install.files)
            .filter(|file| file.kind == kind)
            .count()
    };
    let (paks, loose) = (count(DeployKind::Pak), count(DeployKind::Loose));
    let mut warnings = Vec::new();
    if paks > 0 && !ref_status.installed {
        warnings.push(LaunchWarning::ReframeworkMissing { paks });
    }
    if paks > 0 && ref_status.pak_directory == Some(false) {
        warnings.push(LaunchWarning::PakFolderOff { paks });
    }
    if loose > 0 && ref_status.loose_file_loader != Some(true) {
        warnings.push(LaunchWarning::LooseLoaderOff { files: loose });
    }
    if let Some(change) = &game_state.build_changed {
        warnings.push(LaunchWarning::BuildChanged {
            recorded: change.recorded.clone(),
            now: change.now.clone(),
        });
    }
    warnings
}

/// Start `exe` with `cwd` as its working directory, detached: the stone never
/// waits for it, hands it none of its own handles, and gives it no console of
/// its own (DD2.exe is a windowed program). Refused when `exe` is not a file
/// or `cwd` is not a folder.
pub fn start_exe(exe: &Path, cwd: &Path) -> RiftResult<()> {
    if !fs::metadata(exe).is_ok_and(|meta| meta.is_file()) {
        return Err(RiftError::refused(format!(
            "{} is not there any more, so the game could not be started; look at the game folder on the preflight page.",
            exe.display()
        )));
    }
    if !cwd.is_dir() {
        return Err(RiftError::refused(format!(
            "The game folder {} is not there any more, so the game could not be started; pick it again on the preflight page.",
            cwd.display()
        )));
    }
    spawn::detached(exe, cwd)
        .map_err(|err| RiftError::io(format!("could not start {}", exe.display()), err))
}

#[cfg(windows)]
mod spawn {
    use std::ffi::{OsStr, OsString};
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use std::ptr;

    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        CreateProcessW, CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS, PROCESS_INFORMATION,
        STARTUPINFOW,
    };

    /// `text` as a NUL-terminated UTF-16 string. One with a NUL of its own is
    /// refused: Windows would read a shorter path.
    fn wide(text: &OsStr) -> io::Result<Vec<u16>> {
        let mut units: Vec<u16> = text.encode_wide().collect();
        if units.contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the path holds a NUL character",
            ));
        }
        units.push(0);
        Ok(units)
    }

    /// Start `exe` in `cwd` with CreateProcessW: no handle of the stone's is
    /// inherited (`bInheritHandles` is false — std's `Command` always passes
    /// true), detached from any console and from the stone's Ctrl+C group,
    /// never waited on. Both handles the call returns are closed at once.
    pub(super) fn detached(exe: &Path, cwd: &Path) -> io::Result<()> {
        let application = wide(exe.as_os_str())?;
        // argv[0] only, quoted: a Windows path cannot hold a quote.
        let mut quoted = OsString::from("\"");
        quoted.push(exe.as_os_str());
        quoted.push("\"");
        let mut command_line = wide(&quoted)?;
        let directory = wide(cwd.as_os_str())?;
        let startup = STARTUPINFOW {
            cb: std::mem::size_of::<STARTUPINFOW>() as u32,
            ..STARTUPINFOW::default()
        };
        let mut process = PROCESS_INFORMATION::default();
        // SAFETY: the three strings are live NUL-terminated buffers owned by
        // this frame for the whole call, the command line mutable as
        // CreateProcessW requires; `startup` is zeroed and sized; `process`
        // is written by the call and read only after it succeeded.
        let created = unsafe {
            CreateProcessW(
                application.as_ptr(),
                command_line.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                0,
                DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP,
                ptr::null(),
                directory.as_ptr(),
                &startup,
                &mut process,
            )
        };
        if created == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: both handles came from the successful call above and are
        // closed exactly once; the game runs on without them.
        unsafe {
            CloseHandle(process.hThread);
            CloseHandle(process.hProcess);
        }
        Ok(())
    }
}

#[cfg(not(windows))]
mod spawn {
    use std::io;
    use std::path::Path;
    use std::process::{Command, Stdio};

    /// Start `exe` in `cwd` without waiting for it (the Linux bench; the
    /// stone ships on Windows). A thread reaps it when it exits.
    pub(super) fn detached(exe: &Path, cwd: &Path) -> io::Result<()> {
        let mut child = Command::new(exe)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        std::thread::Builder::new()
            .name("riftstone-launched".into())
            .spawn(move || {
                let _ = child.wait();
            })
            .map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_name_themselves_the_way_the_wire_does() {
        let steam = LaunchRoute::Steam {
            url: "steam://rungameid/2054970".into(),
        };
        let exe = LaunchRoute::Exe {
            exe: PathBuf::from("/g/DD2.exe"),
            cwd: PathBuf::from("/g"),
        };
        assert_eq!((steam.kind(), exe.kind()), ("steam", "exe"));
        let wire = serde_json::to_value(LaunchWarning::LooseLoaderOff { files: 3 }).unwrap();
        assert_eq!(
            wire,
            serde_json::json!({ "kind": "looseLoaderOff", "files": 3 })
        );
        let wire = serde_json::to_value(LaunchWarning::BuildChanged {
            recorded: "1".into(),
            now: "2".into(),
        })
        .unwrap();
        assert_eq!(
            wire,
            serde_json::json!({ "kind": "buildChanged", "recorded": "1", "now": "2" })
        );
    }
}
