//! The nxm handler (v0.3, docs/WINDOW-CONTRACT.md § v0.3 The nxm handler):
//! the switch that makes the stone answer the site's Mod Manager Download
//! button, and what the stone does with a link that arrives.
//!
//! **The switch** is off by default and the player's own act. On: the
//! command the user's registry key held (if any) is remembered in
//! `nxm-handler.json` in the app data folder FIRST, then
//! `HKCU\Software\Classes\nxm` is written for this `riftstone.exe`. Off: when
//! the user's key still points at this exe, the remembered command goes back
//! (or the key is removed when there was none, and the machine's handler, if
//! any, answers again); a key another program took back is left alone. The
//! installer never registers anything.
//!
//! **An arriving link** is untrusted input. A second launch hands it over
//! through `tauri-plugin-single-instance` (the app's first plugin), a cold
//! start through the stone's own command line, handled once the spine is
//! awake. It is accepted only when it reads as a Dragon's Dogma 2 file link
//! with the site's grant (`key` + `expires`), the grant has not expired, the
//! link was made for the validated account (when both ids are known) and a
//! key is stored that Nexus has not refused. It then resumes the waiting
//! download of the same mod and file, else starts one
//! ([`downloads::take_link`]). `nxm://premium` validates the account again.
//! Collections and other games are refused with a sentence. The window hears
//! `rift://nxm-received` every time.
//!
//! The link's `key` is a secret: it is never logged, never put in an error
//! and never sent to the window; only the mod and file ids are.

use std::fs;
use std::io;
use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use rift_core::{lower_key, NexusReason, RiftError};
use rift_nexus::{NexusError, NxmMessage, NxmRefusal};

use crate::account;
use crate::downloads;
use crate::dto::{NxmHandlerState, NxmHandlerView, NxmReceivedEvent};
use crate::events;
use crate::persist;
use crate::protocol::{command_for, points_at, program_name, NOT_ON_THIS_SYSTEM};
use crate::state::AppState;

/// The refusal of a link made for another account than the stored key's.
pub const NOT_YOURS: &str = "That Mod Manager Download link was made for another Nexus Mods account than the one whose API key the Riftstone holds. Sign in on the site with that key's account, then press Mod Manager Download again.";

/// `nxm-handler.json`: the command the user's registry key held before the
/// stone answered nxm:// links.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HandlerMemory {
    #[serde(default)]
    schema: u32,
    /// `None`: the user's key held no handler (the machine's, if any,
    /// answered).
    #[serde(default)]
    previous: Option<String>,
}

impl HandlerMemory {
    const SCHEMA: u32 = 1;
}

fn remembered(state: &AppState) -> Option<HandlerMemory> {
    persist::read_json(&state.config.dirs.nxm_handler_file())
}

fn forget(state: &AppState) {
    let file = state.config.dirs.nxm_handler_file();
    match fs::remove_file(&file) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => log::warn!(
            "the nxm handler's memory {} could not be removed: {err}",
            file.display()
        ),
    }
}

// ---------------------------------------------------------------------------
// The switch: get_nxm_handler / set_nxm_handler
// ---------------------------------------------------------------------------

/// Is `command` a handler the links may go back to? Not this exe, and not a
/// stone of the same name elsewhere (an older install the player moved away
/// from): giving the links back to that would hand them to a dead path.
fn worth_giving_back(command: &str, exe: &Path) -> bool {
    let same_name = match (program_name(command), exe.file_name()) {
        (Some(program), Some(own)) => lower_key(&program) == lower_key(&own.to_string_lossy()),
        _ => false,
    };
    !points_at(command, exe) && !same_name
}

/// Who answers nxm:// links on this PC now (`get_nxm_handler`).
pub fn handler_view(state: &AppState) -> Result<NxmHandlerView, RiftError> {
    let registry = &state.config.protocols;
    if !registry.supported() {
        return Ok(NxmHandlerView {
            state: NxmHandlerState::Unsupported,
            other: None,
            gives_back_to: None,
        });
    }
    let exe = &state.config.exe;
    Ok(match registry.effective_command()? {
        None => NxmHandlerView {
            state: NxmHandlerState::None,
            other: None,
            gives_back_to: None,
        },
        Some(command) if points_at(&command, exe) => {
            let previous = remembered(state)
                .and_then(|memory| memory.previous)
                .filter(|previous| worth_giving_back(previous, exe));
            let back = match previous {
                Some(previous) => Some(previous),
                None => registry
                    .machine_command()?
                    .filter(|machine| worth_giving_back(machine, exe)),
            };
            NxmHandlerView {
                state: NxmHandlerState::Ours,
                other: None,
                gives_back_to: back.as_deref().and_then(program_name),
            }
        }
        Some(command) => NxmHandlerView {
            state: NxmHandlerState::Other,
            other: program_name(&command),
            gives_back_to: None,
        },
    })
}

/// Turn the stone's nxm handler on or off (`set_nxm_handler`). Refused off
/// Windows.
pub fn set_handler(state: &AppState, on: bool) -> Result<NxmHandlerView, RiftError> {
    let registry = &state.config.protocols;
    if !registry.supported() {
        return Err(RiftError::refused(NOT_ON_THIS_SYSTEM));
    }
    let exe = &state.config.exe;
    let user = registry.user_command()?;
    let ours = user
        .as_deref()
        .is_some_and(|command| points_at(command, exe));
    if on && !ours {
        // Remembered first: the registry changes only once the stone can
        // give the links back.
        let memory = HandlerMemory {
            schema: HandlerMemory::SCHEMA,
            previous: user.filter(|previous| worth_giving_back(previous, exe)),
        };
        persist::write_json(&state.config.dirs.nxm_handler_file(), &memory, true)?;
        registry.write_user(&command_for(exe))?;
        log::info!("the Riftstone answers nxm:// links now; the handler it replaced is remembered");
    } else if !on {
        if ours {
            let previous = remembered(state)
                .and_then(|memory| memory.previous)
                .filter(|previous| worth_giving_back(previous, exe));
            match previous {
                Some(previous) => registry.write_user(&previous)?,
                None => registry.remove_user()?,
            }
            log::info!("the Riftstone gave nxm:// links back");
        }
        forget(state);
    }
    handler_view(state)
}

// ---------------------------------------------------------------------------
// An arriving link
// ---------------------------------------------------------------------------

/// The `nxm://` links among a launch's arguments (the first is the exe).
pub fn nxm_arguments(args: &[String]) -> Vec<String> {
    args.iter()
        .map(|arg| {
            arg.trim()
                .trim_matches(|c: char| matches!(c, '"' | '\''))
                .trim()
        })
        .filter(|arg| {
            arg.get(..6)
                .is_some_and(|scheme| scheme.eq_ignore_ascii_case("nxm://"))
        })
        .map(str::to_string)
        .collect()
}

/// Take one arriving link, tell the window (`rift://nxm-received`), and
/// return what it was told. Never waits on the network: the single-instance
/// callback runs on the window's own thread.
pub fn handle_nxm(state: &Arc<AppState>, text: &str) -> NxmReceivedEvent {
    let received = receive(state, text);
    match (&received.refusal, received.accepted) {
        (_, true) => log::info!(
            "a Mod Manager Download link reached the stone (mod {}, file {})",
            received.mod_id.unwrap_or_default(),
            received.file_id.unwrap_or_default()
        ),
        (Some(refusal), false) => log::info!("an nxm link was refused: {refusal}"),
        (None, false) => {}
    }
    state.events.emit(events::NXM_RECEIVED, &received);
    received
}

fn receive(state: &Arc<AppState>, text: &str) -> NxmReceivedEvent {
    let refused = |ids: (Option<u64>, Option<u64>), refusal: RiftError| NxmReceivedEvent {
        mod_id: ids.0,
        file_id: ids.1,
        accepted: false,
        download_id: None,
        refusal: Some(refusal),
    };
    let link = match rift_nexus::parse_nxm(text) {
        Err(refusal) => return refused((None, None), RiftError::refused(refusal.to_string())),
        Ok(NxmMessage::Premium) => {
            revalidate(state);
            return NxmReceivedEvent {
                mod_id: None,
                file_id: None,
                accepted: false,
                download_id: None,
                refusal: None,
            };
        }
        Ok(NxmMessage::File(link)) => link,
    };
    let ids = (Some(link.mod_id), Some(link.file_id));
    if !state.has_api_key.load(Ordering::SeqCst) {
        return refused(ids, NexusError::NoApiKey.into());
    }
    if account::key_rejected(state) {
        return refused(ids, NexusError::KeyRejected.into());
    }
    let Some(grant) = &link.grant else {
        // The site's button always sends its grant; a link without one is
        // not the button's.
        return refused(ids, RiftError::refused(NxmRefusal::Unreadable.to_string()));
    };
    if grant.expired(persist::now_ms() / 1000) {
        return refused(ids, NexusError::LinkExpired.into());
    }
    if !link.made_for(account::account_user_id(state)) {
        return refused(
            ids,
            RiftError::nexus_because(NexusReason::LinkNotYours, NOT_YOURS),
        );
    }
    let download_id = downloads::take_link(state, link);
    NxmReceivedEvent {
        mod_id: ids.0,
        file_id: ids.1,
        accepted: true,
        download_id: Some(download_id),
        refusal: None,
    }
}

/// `nxm://premium`: the account changed on the site; validate it again, off
/// the window's thread.
fn revalidate(state: &Arc<AppState>) {
    if !state.has_api_key.load(Ordering::SeqCst) {
        return;
    }
    let me = Arc::clone(state);
    let spawned = std::thread::Builder::new()
        .name("riftstone-account".into())
        .spawn(move || {
            if let Err(err) = account::validate(&me) {
                log::warn!("the account could not be validated again: {err}");
            }
        });
    if let Err(err) = spawned {
        log::warn!("could not validate the account again: {err}");
    }
}

/// The links of the stone's own command line (a cold start for a Mod Manager
/// Download link), handled once the spine is awake (startup step 9).
pub fn handle_launch_links(state: &Arc<AppState>) {
    for link in state.take_launch_links() {
        handle_nxm(state, &link);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_nxm_arguments_are_taken_from_a_launch() {
        let args: Vec<String> = [
            r"D:\Vernworth\The Riftstone\riftstone.exe",
            "--flag",
            "\"nxm://dragonsdogma2/mods/2207/files/9001?key=k&expires=1\"",
            "  NXM://premium ",
            "https://www.nexusmods.com/dragonsdogma2/mods/2207",
            "nxm:",
        ]
        .iter()
        .map(|arg| (*arg).to_string())
        .collect();
        assert_eq!(
            nxm_arguments(&args),
            vec![
                "nxm://dragonsdogma2/mods/2207/files/9001?key=k&expires=1".to_string(),
                "NXM://premium".to_string()
            ]
        );
    }
}
