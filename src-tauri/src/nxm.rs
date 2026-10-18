//! The nxm handler (v0.3, docs/WINDOW-CONTRACT.md § v0.3 The nxm handler):
//! the switch that makes the stone answer the site's Mod Manager Download
//! button, and what the stone does with a link that arrives.
//!
//! **The switch** is off by default and the player's own act. On: the user's
//! registry key as the claim will change it ([`UserKey`]: whether it was
//! there, its values, the command it held) is remembered in
//! `nxm-handler.json` in the app data folder FIRST, then
//! `HKCU\Software\Classes\nxm` is written for this `riftstone.exe`. Off: when
//! the user's key still points at this exe, it is put back exactly as the
//! claim found it ([`crate::protocol`] says how): removed whole only when the
//! claim made it (the machine's handler, if any, answers again), the command
//! it held back in place unless that starts a `riftstone.exe`, and nothing
//! the claim never wrote touched. A key another program took back is left
//! alone. A memory that cannot be read or parsed refuses the off and is left
//! exactly where it is: the stone never guesses what it replaced. The
//! installer registers nothing; its uninstaller runs `riftstone.exe
//! --give-back-nxm` ([`give_back_on_uninstall`]): the switch's off, except
//! that a key it cannot put back is removed whole, since the exe it starts
//! is about to go ([`give_back_as_uninstalled`]).
//!
//! **An arriving link** is untrusted input. A second launch hands it over
//! through `tauri-plugin-single-instance` (the app's first plugin), a cold
//! start through the stone's own command line. Until the launch's key
//! validation is done and the window has read its first frame, every link
//! waits at a door ([`second_launch`], startup step 9): checked against the
//! validated account, and told to a window that hears it. It is accepted
//! only when it reads as a Dragon's Dogma 2 file link
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
use crate::persist::{self, DataDirs, Loaded};
use crate::protocol::{
    self, command_for, points_at, program_name, ProtocolRegistry, UserKey, NOT_ON_THIS_SYSTEM,
};
use crate::state::AppState;

/// The refusal of a link made for another account than the stored key's.
pub const NOT_YOURS: &str = "That Mod Manager Download link was made for another Nexus Mods account than the one whose API key the Riftstone holds. Sign in on the site with that key's account, then press Mod Manager Download again.";

/// The uninstaller's argument (`src-tauri/windows/hooks.nsh`): give the
/// nxm:// links back and exit, with no window ([`give_back_on_uninstall`]).
pub const GIVE_BACK_ARG: &str = "--give-back-nxm";

/// `nxm-handler.json`: what the claim replaced in the user's registry key,
/// written before the registry changes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HandlerMemory {
    #[serde(default)]
    schema: u32,
    /// The command the user's key held, when it is one the links may go back
    /// to; `None`: none (the machine's handler, if any, answered).
    #[serde(default)]
    previous: Option<String>,
    /// Schema 2: the user's key as the claim found it, which off puts back.
    /// A schema-1 memory has none and gives the links back as schema 1 did:
    /// the previous command written back, else the key removed whole.
    #[serde(default)]
    before: Option<UserKey>,
}

impl HandlerMemory {
    const SCHEMA: u32 = 2;
}

/// What the stone remembers of the handler it replaced.
enum Memory {
    /// Nothing: the switch was never turned on here, or the file went.
    Nothing,
    Held(HandlerMemory),
    /// `nxm-handler.json` is there and cannot be read or parsed: why. Off is
    /// refused while it is, and the file stays exactly as it is.
    Unknown(String),
}

/// The memory, read where it lies: a damaged one is never set aside.
fn remembered(file: &Path) -> Memory {
    match persist::inspect_json::<HandlerMemory>(file) {
        Loaded::Absent => Memory::Nothing,
        Loaded::Found(memory) => Memory::Held(memory),
        Loaded::Damaged => Memory::Unknown("it is damaged".into()),
        Loaded::Unreadable(why) => Memory::Unknown(why),
    }
}

/// The memory is spent: the links went back, or the key is another
/// program's now. Only ever called on a memory that was read.
fn forget(file: &Path) {
    match fs::remove_file(file) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => log::warn!(
            "the nxm handler's memory {} could not be removed: {err}",
            file.display()
        ),
    }
}

/// Off, refused: the memory of what the claim replaced cannot be read.
fn unreadable_memory(file: &Path, why: &str) -> RiftError {
    RiftError::refused(format!(
        "The Riftstone cannot read which program it took Mod Manager Download links from ({}: {why}), so it changed nothing rather than guess. To give the links back, turn link handling on again in that program; or delete that file and turn this off again, and Windows' own handler, if there is one, answers them.",
        file.display()
    ))
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

/// The user's key as off puts it back: as the claim found it, except a
/// command that starts a `riftstone.exe` (a blank one, a `DelegateExecute`
/// handler's, goes back as it was).
fn as_given_back(before: &UserKey, exe: &Path) -> UserKey {
    UserKey {
        command: before
            .command
            .clone()
            .filter(|command| command.trim().is_empty() || worth_giving_back(command, exe)),
        ..before.clone()
    }
}

/// The program off would give the links back to, while the key is ours.
fn gives_back_to(
    registry: &dyn ProtocolRegistry,
    exe: &Path,
    memory: &Memory,
) -> Result<Option<String>, RiftError> {
    let back = match memory {
        // Off is refused then: it gives nothing back.
        Memory::Unknown(_) => return Ok(None),
        Memory::Held(HandlerMemory {
            before: Some(before),
            ..
        }) => {
            let restored = as_given_back(before, exe);
            if restored.exists && restored.command_depth >= 3 {
                // The user's command key stays: it answers, values and all.
                return Ok(restored.handler().map(str::to_string));
            }
            None
        }
        Memory::Held(memory) => memory
            .previous
            .clone()
            .filter(|previous| worth_giving_back(previous, exe)),
        Memory::Nothing => None,
    };
    match back {
        Some(previous) => Ok(Some(previous)),
        None => Ok(registry
            .machine_command()?
            .filter(|machine| worth_giving_back(machine, exe))),
    }
}

/// Who answers nxm:// links on this PC now (`get_nxm_handler`).
pub fn handler_view(state: &AppState) -> Result<NxmHandlerView, RiftError> {
    let registry = &*state.config.protocols;
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
            let memory = remembered(&state.config.dirs.nxm_handler_file());
            let back = gives_back_to(registry, exe, &memory)?;
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
/// Windows, and off is refused while the memory cannot be read.
pub fn set_handler(state: &AppState, on: bool) -> Result<NxmHandlerView, RiftError> {
    let registry = &*state.config.protocols;
    let exe = &state.config.exe;
    let file = state.config.dirs.nxm_handler_file();
    if on {
        claim(registry, exe, &file)?;
    } else {
        give_back(registry, exe, &file)?;
    }
    handler_view(state)
}

/// On: unless the user's key already starts this exe, remember the key as
/// the claim will change it, then claim it.
fn claim(registry: &dyn ProtocolRegistry, exe: &Path, file: &Path) -> Result<(), RiftError> {
    if !registry.supported() {
        return Err(RiftError::refused(NOT_ON_THIS_SYSTEM));
    }
    let user = registry.user_command()?;
    if user
        .as_deref()
        .is_some_and(|command| points_at(command, exe))
    {
        return Ok(());
    }
    // Remembered first: the registry changes only once the stone can put
    // it back. A memory left from an earlier claim is spent: that claim is
    // no longer in the registry.
    let before = registry.user_key()?;
    let memory = HandlerMemory {
        schema: HandlerMemory::SCHEMA,
        previous: user.filter(|previous| worth_giving_back(previous, exe)),
        before: Some(before),
    };
    persist::write_json(file, &memory, true)?;
    registry.write_user(&command_for(exe))?;
    log::info!("the Riftstone answers nxm:// links now; the key it changed is remembered");
    Ok(())
}

/// Give the nxm:// links back: the switch's off, and the uninstaller's
/// `--give-back-nxm`. When the user's key still starts `exe`, it goes back as
/// the claim found it; a key another program took back is left alone. The
/// memory is forgotten afterwards. Refused, with nothing changed and the
/// memory left as it is, while the memory cannot be read and the key is
/// ours. `true` when the stone held the links and gave them back.
pub fn give_back(
    registry: &dyn ProtocolRegistry,
    exe: &Path,
    memory_file: &Path,
) -> Result<bool, RiftError> {
    if !registry.supported() {
        return Err(RiftError::refused(NOT_ON_THIS_SYSTEM));
    }
    let ours = registry
        .user_command()?
        .as_deref()
        .is_some_and(|command| points_at(command, exe));
    let memory = match remembered(memory_file) {
        Memory::Unknown(why) if ours => return Err(unreadable_memory(memory_file, &why)),
        // Not the stone's key any more, and a memory that was not read is
        // never removed: nothing to do.
        Memory::Unknown(_) => return Ok(false),
        Memory::Nothing => None,
        Memory::Held(memory) => Some(memory),
    };
    if ours {
        match memory {
            Some(HandlerMemory {
                before: Some(before),
                ..
            }) => registry.restore_user(&as_given_back(&before, exe))?,
            // Schema 1, or no memory at all: what that build did.
            other => match other
                .and_then(|memory| memory.previous)
                .filter(|previous| worth_giving_back(previous, exe))
            {
                Some(previous) => registry.write_user(&previous)?,
                None => registry.remove_user()?,
            },
        }
        log::info!("the Riftstone gave nxm:// links back");
    }
    forget(memory_file);
    Ok(ours)
}

/// `riftstone.exe --give-back-nxm`, run by the uninstaller before it removes
/// a file (`src-tauri/windows/hooks.nsh`): [`give_back_as_uninstalled`] over
/// the system's registry, this exe and the app data folder the installed
/// stone uses (`RIFTSTONE_DATA_HOME` when set), with no window, no
/// single-instance hand-over and no scan. The exit code: 0 when nothing is
/// left pointing at this exe (or there is no registry here), 1 when
/// something is; the uninstaller goes on either way.
pub fn give_back_on_uninstall() -> i32 {
    let registry = protocol::system_registry();
    if !registry.supported() {
        return 0;
    }
    let Ok(exe) = std::env::current_exe() else {
        return 1;
    };
    let Some(dirs) = DataDirs::from_env().or_else(DataDirs::installed) else {
        return 1;
    };
    if give_back_as_uninstalled(&*registry, &exe, &dirs.nxm_handler_file()) {
        0
    } else {
        1
    }
}

/// The uninstaller's give-back: [`give_back`], and when that cannot finish
/// (the memory cannot be read, Windows refused the restore) while the user's
/// key still starts `exe`, the key is removed whole, as the stone did before
/// it remembered more. The switch's off refuses in that case so the player
/// can choose; the uninstaller can ask no one, and the exe is about to go, so
/// a key that starts it would only hand the links to a dead path. The memory
/// itself is left as it is. `true` when nothing is left pointing at `exe`.
pub fn give_back_as_uninstalled(
    registry: &dyn ProtocolRegistry,
    exe: &Path,
    memory_file: &Path,
) -> bool {
    if give_back(registry, exe, memory_file).is_ok() {
        return true;
    }
    let ours = registry
        .user_command()
        .ok()
        .flatten()
        .is_some_and(|command| points_at(&command, exe));
    !ours || registry.remove_user().is_ok()
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

/// A second launch handed its arguments over (`riftstone.exe "nxm://…"`,
/// through single-instance): each nxm link in them is handled now, or held
/// at the door until the launch's key validation is done and the window
/// listens ([`AppState::hold_link`], startup step 9).
pub fn second_launch(state: &Arc<AppState>, args: &[String]) {
    for link in nxm_arguments(args) {
        if !state.hold_link(&link) {
            handle_nxm(state, &link);
        }
    }
}

/// The launch's key validation is done (the account thread, startup step
/// 7): the links held for it are handled, when the window listens.
pub fn launch_validated(state: &Arc<AppState>) {
    for link in state.launch_validated() {
        handle_nxm(state, &link);
    }
}

/// The window read its first frame (`get_overview`): the links held for it
/// are handled, when the launch validation is done.
pub fn window_listens(state: &Arc<AppState>) {
    for link in state.window_listens() {
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
