//! The `nxm://` protocol in the Windows registry (v0.3): the seam the nxm
//! handler switch writes through ([`crate::nxm`]).
//!
//! Windows looks an `nxm://` link up under `HKEY_CLASSES_ROOT\nxm`, a merged
//! view: the user's own `HKEY_CURRENT_USER\Software\Classes\nxm` when there
//! is one, else the machine's `HKEY_LOCAL_MACHINE\Software\Classes\nxm`. It
//! runs that key's `shell\open\command`; where the user's key holds that
//! command key, the user's key answers, values and all. The stone reads all
//! three and only ever writes the user's key, and only when the player turns
//! its handler on or off (or the uninstaller asks it to give the links back,
//! [`crate::nxm::give_back`]). The installer registers nothing, and
//! `tauri.conf.json` declares no URL scheme.
//!
//! **What the claim writes, and what off puts back.** The claim writes three
//! values under the user's key: its default (`URL:NXM Protocol`), `URL
//! Protocol` (`""`) and the default of `shell\open\command` (the stone's
//! command), making the keys it needs. Before it does, the stone reads the
//! key as the claim will change it ([`UserKey`]) and remembers that. Off puts
//! exactly that back ([`ProtocolRegistry::restore_user`]): the whole key goes
//! only when the claim made it; otherwise each of the three values goes back
//! as it was (or goes, where it was not set), the keys the claim made go
//! while nothing else is in them, and everything the claim never wrote
//! (another program's `DefaultIcon`, its other verbs, a `DelegateExecute`)
//! stays.
//!
//! - `SystemRegistry` (Windows only, `winreg`; built only there): the real
//!   registry.
//! - [`UnsupportedRegistry`]: every other system (the Linux bench). It reads
//!   nothing and refuses every write.
//! - [`MemoryRegistry`]: the tests' registry, the user's key and the
//!   machine's command in memory, merged the way Windows merges them.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};

use rift_core::{lower_key, RiftError};

/// The sentence a switch on a system without the Windows registry answers.
pub const NOT_ON_THIS_SYSTEM: &str = "Mod Manager Download links reach a program through the Windows registry, and this system has none, so the Riftstone cannot answer them here. Download from the Riftstone itself with a premium account, or use the Riftstone on Windows.";

/// The default value the claim gives the user's key.
pub const PROTOCOL_NAME: &str = "URL:NXM Protocol";

/// The keys under the user's key that lead to the command, outermost first:
/// [`UserKey::command_depth`] counts how many of them are there.
pub const COMMAND_PATH: [&str; 3] = ["shell", r"shell\open", r"shell\open\command"];

/// The user's `HKCU\Software\Classes\nxm` as far as the stone's claim writes
/// it: read before the claim, remembered in `nxm-handler.json`, put back by
/// off ([`ProtocolRegistry::restore_user`]).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserKey {
    /// The key was there.
    #[serde(default)]
    pub exists: bool,
    /// Its default value; `None`: not set.
    #[serde(default)]
    pub default: Option<String>,
    /// Its `URL Protocol` value; `None`: not set.
    #[serde(default)]
    pub url_protocol: Option<String>,
    /// How many of [`COMMAND_PATH`] were there: 0 none, 3 the command key.
    #[serde(default)]
    pub command_depth: u8,
    /// The command key's default value as it was, a blank one kept (a
    /// `DelegateExecute` handler leaves it blank); `None`: not set, or no
    /// command key.
    #[serde(default)]
    pub command: Option<String>,
}

impl UserKey {
    /// The key as the claim leaves it, starting `command`.
    pub fn claimed(command: &str) -> UserKey {
        UserKey {
            exists: true,
            default: Some(PROTOCOL_NAME.to_string()),
            url_protocol: Some(String::new()),
            command_depth: 3,
            command: Some(command.to_string()),
        }
    }

    /// The command it starts, when it holds one: a blank value starts
    /// nothing the stone can name.
    pub fn handler(&self) -> Option<&str> {
        if !self.exists || self.command_depth < 3 {
            return None;
        }
        self.command
            .as_deref()
            .filter(|command| !command.trim().is_empty())
    }
}

/// Where nxm:// links go.
pub trait ProtocolRegistry: Send + Sync {
    /// Is there a registry to read and write (Windows)?
    fn supported(&self) -> bool;
    /// `HKCU\Software\Classes\nxm\shell\open\command`: the user's own handler
    /// (a blank value reads as none).
    fn user_command(&self) -> Result<Option<String>, RiftError>;
    /// `HKLM\Software\Classes\nxm\shell\open\command`: the machine's handler,
    /// what Windows falls back on without a user key.
    fn machine_command(&self) -> Result<Option<String>, RiftError>;
    /// `HKCR\nxm\shell\open\command`: what Windows runs now.
    fn effective_command(&self) -> Result<Option<String>, RiftError>;
    /// The user's key as the claim would change it: read before the claim,
    /// so off can put it back.
    fn user_key(&self) -> Result<UserKey, RiftError>;
    /// Claim the user's key: its default `URL:NXM Protocol`, `URL Protocol`
    /// = `""`, and `shell\open\command` = `command`.
    fn write_user(&self, command: &str) -> Result<(), RiftError>;
    /// Put the user's key back as `before` remembers it (the module docs say
    /// how): removed whole when it was not there; otherwise the claim's three
    /// values as they were and the keys the claim made gone while empty,
    /// everything else untouched. A key that is gone meanwhile stays gone.
    fn restore_user(&self, before: &UserKey) -> Result<(), RiftError>;
    /// Remove the user's key and everything under it (an absent key is fine).
    fn remove_user(&self) -> Result<(), RiftError>;
}

/// The registry of the system the stone runs on.
pub fn system_registry() -> Arc<dyn ProtocolRegistry> {
    #[cfg(windows)]
    let registry: Arc<dyn ProtocolRegistry> = Arc::new(win::SystemRegistry);
    #[cfg(not(windows))]
    let registry: Arc<dyn ProtocolRegistry> = Arc::new(UnsupportedRegistry);
    registry
}

/// A system without the Windows registry.
pub struct UnsupportedRegistry;

impl ProtocolRegistry for UnsupportedRegistry {
    fn supported(&self) -> bool {
        false
    }

    fn user_command(&self) -> Result<Option<String>, RiftError> {
        Ok(None)
    }

    fn machine_command(&self) -> Result<Option<String>, RiftError> {
        Ok(None)
    }

    fn effective_command(&self) -> Result<Option<String>, RiftError> {
        Ok(None)
    }

    fn user_key(&self) -> Result<UserKey, RiftError> {
        Ok(UserKey::default())
    }

    fn write_user(&self, _command: &str) -> Result<(), RiftError> {
        Err(RiftError::refused(NOT_ON_THIS_SYSTEM))
    }

    fn restore_user(&self, _before: &UserKey) -> Result<(), RiftError> {
        Err(RiftError::refused(NOT_ON_THIS_SYSTEM))
    }

    fn remove_user(&self) -> Result<(), RiftError> {
        Err(RiftError::refused(NOT_ON_THIS_SYSTEM))
    }
}

fn hold<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The tests' registry: the user's key (what the claim writes, and beside it
/// what it never writes) and the machine's command, merged as Windows merges
/// them. It counts the stone's writes.
#[derive(Default)]
pub struct MemoryRegistry {
    user: Mutex<UserKey>,
    /// Under the user's key, what the claim never writes, by its path below
    /// the key (`DefaultIcon`, `shell\open\command\DelegateExecute`):
    /// another program's.
    extras: Mutex<BTreeMap<String, String>>,
    machine: Mutex<Option<String>>,
    writes: AtomicUsize,
}

impl MemoryRegistry {
    /// A registry holding these commands (a user command as a program that
    /// claims the key writes it).
    pub fn holding(user: Option<&str>, machine: Option<&str>) -> MemoryRegistry {
        let registry = MemoryRegistry {
            machine: Mutex::new(machine.map(str::to_string)),
            ..MemoryRegistry::default()
        };
        registry.set_user(user);
        registry
    }

    /// The user's command now (a blank one reads as none).
    pub fn user(&self) -> Option<String> {
        hold(&self.user).handler().map(str::to_string)
    }

    /// The user's key now, as far as the claim writes it.
    pub fn user_key_now(&self) -> UserKey {
        hold(&self.user).clone()
    }

    /// Another program writes the user's key the way a claim does (another
    /// manager taking the links back at its start, say), or removes it whole.
    pub fn set_user(&self, command: Option<&str>) {
        match command {
            Some(command) => *hold(&self.user) = UserKey::claimed(command),
            None => {
                *hold(&self.user) = UserKey::default();
                hold(&self.extras).clear();
            }
        }
    }

    /// The user's key exactly as a test sets the scene.
    pub fn set_user_key(&self, key: UserKey) {
        *hold(&self.user) = key;
    }

    /// Another program's value or key under the user's key, by its path below
    /// the key.
    pub fn add_extra(&self, path: &str, value: &str) {
        hold(&self.extras).insert(path.to_string(), value.to_string());
    }

    /// What the claim never writes under the user's key, as it stands.
    pub fn extras(&self) -> BTreeMap<String, String> {
        hold(&self.extras).clone()
    }

    /// How many times the stone wrote, restored or removed the user's key.
    pub fn writes(&self) -> usize {
        self.writes.load(Ordering::SeqCst)
    }
}

impl ProtocolRegistry for MemoryRegistry {
    fn supported(&self) -> bool {
        true
    }

    fn user_command(&self) -> Result<Option<String>, RiftError> {
        Ok(self.user())
    }

    fn machine_command(&self) -> Result<Option<String>, RiftError> {
        Ok(hold(&self.machine).clone())
    }

    fn effective_command(&self) -> Result<Option<String>, RiftError> {
        let user = self.user_key_now();
        if user.exists && user.command_depth >= 3 {
            // The user's command key answers, even with a blank value.
            return Ok(user.handler().map(str::to_string));
        }
        self.machine_command()
    }

    fn user_key(&self) -> Result<UserKey, RiftError> {
        Ok(self.user_key_now())
    }

    fn write_user(&self, command: &str) -> Result<(), RiftError> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        *hold(&self.user) = UserKey::claimed(command);
        Ok(())
    }

    fn restore_user(&self, before: &UserKey) -> Result<(), RiftError> {
        if !before.exists {
            return self.remove_user();
        }
        self.writes.fetch_add(1, Ordering::SeqCst);
        let mut user = hold(&self.user);
        if !user.exists {
            return Ok(());
        }
        user.default.clone_from(&before.default);
        user.url_protocol.clone_from(&before.url_protocol);
        if user.command_depth >= 3 {
            user.command.clone_from(&before.command);
        }
        let extras = hold(&self.extras);
        user.command_depth = user.command_depth.min(3);
        while user.command_depth > before.command_depth {
            let path = COMMAND_PATH[usize::from(user.command_depth) - 1];
            let inside = format!("{path}\\");
            let holds_value = user.command_depth >= 3 && user.command.is_some();
            if holds_value || extras.keys().any(|key| key.starts_with(&inside)) {
                break;
            }
            user.command_depth -= 1;
        }
        if user.command_depth < 3 {
            user.command = None;
        }
        Ok(())
    }

    fn remove_user(&self) -> Result<(), RiftError> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        *hold(&self.user) = UserKey::default();
        hold(&self.extras).clear();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Commands: what the registry holds, read
// ---------------------------------------------------------------------------

/// The command the stone registers for itself: `"<riftstone.exe>" "%1"`.
pub fn command_for(exe: &Path) -> String {
    format!("\"{}\" \"%1\"", exe.display())
}

/// The program a command starts: the quoted first part, else everything up
/// to the first `.exe`, else the first word.
pub fn command_program(command: &str) -> Option<&str> {
    let command = command.trim();
    let program = if let Some(rest) = command.strip_prefix('"') {
        rest.split('"').next().unwrap_or_default()
    } else {
        match command.to_ascii_lowercase().find(".exe") {
            Some(at) => &command[..at + 4],
            None => command.split_whitespace().next().unwrap_or_default(),
        }
    };
    let program = program.trim();
    (!program.is_empty()).then_some(program)
}

/// The program's file name alone (`OtherManager.exe`): what the window may show
/// of another program, never its folder.
pub fn program_name(command: &str) -> Option<String> {
    let program = command_program(command)?;
    let name = program.rsplit(['\\', '/']).next().unwrap_or(program).trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// Does `command` start this `exe`? Compared the way Windows compares
/// paths: case-blind, either slash.
pub fn points_at(command: &str, exe: &Path) -> bool {
    let key = |text: &str| lower_key(&text.trim().replace('/', "\\"));
    command_program(command).is_some_and(|program| key(program) == key(&exe.to_string_lossy()))
}

// ---------------------------------------------------------------------------
// The Windows registry
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod win {
    use std::io;

    use winreg::enums::{
        HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE,
    };
    use winreg::RegKey;

    use rift_core::RiftError;

    use super::{ProtocolRegistry, UserKey, COMMAND_PATH, PROTOCOL_NAME};

    /// The user's key, under `HKEY_CURRENT_USER`.
    const USER_KEY: &str = r"Software\Classes\nxm";
    /// Its command, under `HKEY_CURRENT_USER` and `HKEY_LOCAL_MACHINE`.
    const CLASSES_COMMAND: &str = r"Software\Classes\nxm\shell\open\command";
    /// The command as Windows sees it, under `HKEY_CLASSES_ROOT`.
    const ROOT_COMMAND: &str = r"nxm\shell\open\command";
    /// The command, under the user's key.
    const COMMAND: &str = r"shell\open\command";

    /// The Windows registry, through `winreg`.
    pub struct SystemRegistry;

    /// A key's default value; `None` when the key or the value is absent
    /// or blank.
    fn default_value(root: &RegKey, path: &str) -> Result<Option<String>, RiftError> {
        let key = match root.open_subkey_with_flags(path, KEY_READ) {
            Ok(key) => key,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(read_error(&err)),
        };
        match key.get_value::<String, _>("") {
            Ok(value) => Ok(Some(value).filter(|value| !value.trim().is_empty())),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(read_error(&err)),
        }
    }

    /// A value as it is, a blank one kept; `None` when it is not set.
    fn raw_value(key: &RegKey, name: &str) -> Result<Option<String>, RiftError> {
        match key.get_value::<String, _>(name) {
            Ok(value) => Ok(Some(value)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(read_error(&err)),
        }
    }

    /// Set `name` to `value`, or remove it where `value` is `None` (a value
    /// that is not there is fine).
    fn put_value(key: &RegKey, name: &str, value: Option<&str>) -> Result<(), RiftError> {
        match value {
            Some(value) => key.set_value(name, &value).map_err(|err| write_error(&err)),
            None => match key.delete_value(name) {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(err) => Err(write_error(&err)),
            },
        }
    }

    /// Remove `path` below `key` while nothing is in it (no value, no key);
    /// `true` when it is gone (or never was), `false` when it holds something.
    fn remove_if_empty(key: &RegKey, path: &str) -> Result<bool, RiftError> {
        let info = match key.open_subkey_with_flags(path, KEY_READ) {
            Ok(sub) => sub.query_info().map_err(|err| read_error(&err))?,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(true),
            Err(err) => return Err(read_error(&err)),
        };
        if info.sub_keys > 0 || info.values > 0 {
            return Ok(false);
        }
        match key.delete_subkey(path) {
            Ok(()) => Ok(true),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(true),
            Err(err) => Err(write_error(&err)),
        }
    }

    fn read_error(err: &io::Error) -> RiftError {
        RiftError::Io {
            detail: format!(
                "Windows did not let the Riftstone read which program answers Mod Manager Download links ({err}). Try again; nothing was changed."
            ),
        }
    }

    fn write_error(err: &io::Error) -> RiftError {
        RiftError::Io {
            detail: format!(
                "Windows did not let the Riftstone change which program answers Mod Manager Download links ({err}). Try again."
            ),
        }
    }

    impl ProtocolRegistry for SystemRegistry {
        fn supported(&self) -> bool {
            true
        }

        fn user_command(&self) -> Result<Option<String>, RiftError> {
            default_value(&RegKey::predef(HKEY_CURRENT_USER), CLASSES_COMMAND)
        }

        fn machine_command(&self) -> Result<Option<String>, RiftError> {
            default_value(&RegKey::predef(HKEY_LOCAL_MACHINE), CLASSES_COMMAND)
        }

        fn effective_command(&self) -> Result<Option<String>, RiftError> {
            default_value(&RegKey::predef(HKEY_CLASSES_ROOT), ROOT_COMMAND)
        }

        fn user_key(&self) -> Result<UserKey, RiftError> {
            let key = match RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey_with_flags(USER_KEY, KEY_READ)
            {
                Ok(key) => key,
                Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(UserKey::default()),
                Err(err) => return Err(read_error(&err)),
            };
            let mut found = UserKey {
                exists: true,
                default: raw_value(&key, "")?,
                url_protocol: raw_value(&key, "URL Protocol")?,
                ..UserKey::default()
            };
            for path in COMMAND_PATH {
                match key.open_subkey_with_flags(path, KEY_READ) {
                    Ok(sub) => {
                        found.command_depth += 1;
                        if usize::from(found.command_depth) == COMMAND_PATH.len() {
                            found.command = raw_value(&sub, "")?;
                        }
                    }
                    Err(err) if err.kind() == io::ErrorKind::NotFound => break,
                    Err(err) => return Err(read_error(&err)),
                }
            }
            Ok(found)
        }

        fn write_user(&self, command: &str) -> Result<(), RiftError> {
            let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
                .create_subkey(USER_KEY)
                .map_err(|err| write_error(&err))?;
            key.set_value("", &PROTOCOL_NAME)
                .map_err(|err| write_error(&err))?;
            key.set_value("URL Protocol", &"")
                .map_err(|err| write_error(&err))?;
            let (open, _) = key
                .create_subkey(COMMAND)
                .map_err(|err| write_error(&err))?;
            open.set_value("", &command)
                .map_err(|err| write_error(&err))
        }

        fn restore_user(&self, before: &UserKey) -> Result<(), RiftError> {
            if !before.exists {
                return self.remove_user();
            }
            let key = match RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey_with_flags(USER_KEY, KEY_READ | KEY_WRITE)
            {
                Ok(key) => key,
                // Gone meanwhile: nothing of the claim is left to take back.
                Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(err) => return Err(write_error(&err)),
            };
            put_value(&key, "", before.default.as_deref())?;
            put_value(&key, "URL Protocol", before.url_protocol.as_deref())?;
            match key.open_subkey_with_flags(COMMAND, KEY_READ | KEY_WRITE) {
                Ok(command) => put_value(&command, "", before.command.as_deref())?,
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => return Err(write_error(&err)),
            }
            // The keys the claim made, innermost first, while nothing else
            // is in them.
            for depth in (usize::from(before.command_depth)..COMMAND_PATH.len()).rev() {
                if !remove_if_empty(&key, COMMAND_PATH[depth])? {
                    break;
                }
            }
            Ok(())
        }

        fn remove_user(&self) -> Result<(), RiftError> {
            match RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(USER_KEY) {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
                Err(err) => Err(write_error(&err)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const OTHER: &str = r#""C:\Tools\Other Manager\OtherManager.exe" -d "%1""#;

    #[test]
    fn the_stone_registers_itself_quoted_with_the_link_as_its_argument() {
        let exe = PathBuf::from(r"D:\Vernworth\The Riftstone\riftstone.exe");
        let command = command_for(&exe);
        assert_eq!(
            command,
            r#""D:\Vernworth\The Riftstone\riftstone.exe" "%1""#
        );
        assert!(points_at(&command, &exe));
        assert!(
            points_at(r#""d:/vernworth/the riftstone/RIFTSTONE.EXE" "%1""#, &exe),
            "case-blind, either slash"
        );
        assert!(!points_at(OTHER, &exe));
        assert_eq!(program_name(&command).as_deref(), Some("riftstone.exe"));
    }

    #[test]
    fn another_programs_command_is_named_by_its_file_alone() {
        assert_eq!(program_name(OTHER).as_deref(), Some("OtherManager.exe"));
        assert_eq!(
            command_program(OTHER),
            Some(r"C:\Tools\Other Manager\OtherManager.exe")
        );
        // Unquoted, with spaces in its folder: up to the .exe.
        assert_eq!(
            program_name(r"C:\Program Files\Other Manager\OtherManager.EXE %1").as_deref(),
            Some("OtherManager.EXE")
        );
        assert_eq!(
            program_name("handler %1").as_deref(),
            Some("handler"),
            "no .exe: the first word"
        );
        assert_eq!(program_name("   "), None);
        assert_eq!(program_name(r#""" "%1""#), None);
    }

    #[test]
    fn the_memory_registry_merges_like_windows() {
        let registry = MemoryRegistry::holding(None, Some(OTHER));
        assert_eq!(
            registry.effective_command().unwrap().as_deref(),
            Some(OTHER)
        );
        registry.write_user("mine").unwrap();
        assert_eq!(
            registry.effective_command().unwrap().as_deref(),
            Some("mine"),
            "the user's key wins"
        );
        registry.remove_user().unwrap();
        assert_eq!(
            registry.effective_command().unwrap().as_deref(),
            Some(OTHER)
        );
        assert_eq!(registry.writes(), 2);
    }

    #[test]
    fn an_unsupported_registry_reads_nothing_and_writes_nothing() {
        let registry = UnsupportedRegistry;
        assert!(!registry.supported());
        assert_eq!(registry.effective_command().unwrap(), None);
        assert!(matches!(
            registry.write_user("x"),
            Err(RiftError::Refused { .. })
        ));
        assert!(matches!(
            registry.remove_user(),
            Err(RiftError::Refused { .. })
        ));
    }
}
