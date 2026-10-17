//! The `nxm://` protocol in the Windows registry (v0.3): the seam the nxm
//! handler switch writes through ([`crate::nxm`]).
//!
//! Windows looks an `nxm://` link up under `HKEY_CLASSES_ROOT\nxm`, a merged
//! view: the user's own `HKEY_CURRENT_USER\Software\Classes\nxm` when there
//! is one, else the machine's `HKEY_LOCAL_MACHINE\Software\Classes\nxm`. It
//! runs that key's `shell\open\command`. The stone reads all three and only
//! ever writes the user's key, and only when the player turns its handler on.
//! The installer never registers it, and `tauri.conf.json` declares no URL
//! scheme.
//!
//! - `SystemRegistry` (Windows only, `winreg`; built only there): the real
//!   registry.
//! - [`UnsupportedRegistry`]: every other system (the Linux bench). It reads
//!   nothing and refuses every write.
//! - [`MemoryRegistry`]: the tests' registry, a user key and a machine key in
//!   memory, merged the way Windows merges them.

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use rift_core::{lower_key, RiftError};

/// The sentence a switch on a system without the Windows registry answers.
pub const NOT_ON_THIS_SYSTEM: &str = "Mod Manager Download links reach a program through the Windows registry, and this system has none, so the Riftstone cannot answer them here. Download from the Riftstone itself with a premium account, or use the Riftstone on Windows.";

/// Where nxm:// links go.
pub trait ProtocolRegistry: Send + Sync {
    /// Is there a registry to read and write (Windows)?
    fn supported(&self) -> bool;
    /// `HKCU\Software\Classes\nxm\shell\open\command`: the user's own handler.
    fn user_command(&self) -> Result<Option<String>, RiftError>;
    /// `HKLM\Software\Classes\nxm\shell\open\command`: the machine's handler,
    /// what Windows falls back on without a user key.
    fn machine_command(&self) -> Result<Option<String>, RiftError>;
    /// `HKCR\nxm\shell\open\command`: what Windows runs now.
    fn effective_command(&self) -> Result<Option<String>, RiftError>;
    /// Write the user's key: its default `URL:NXM Protocol`, `URL Protocol`
    /// = `""`, and `shell\open\command` = `command`.
    fn write_user(&self, command: &str) -> Result<(), RiftError>;
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

    fn write_user(&self, _command: &str) -> Result<(), RiftError> {
        Err(RiftError::refused(NOT_ON_THIS_SYSTEM))
    }

    fn remove_user(&self) -> Result<(), RiftError> {
        Err(RiftError::refused(NOT_ON_THIS_SYSTEM))
    }
}

/// The tests' registry: a user key and a machine key, merged as Windows
/// merges them (the user's wins). It counts its writes.
#[derive(Default)]
pub struct MemoryRegistry {
    user: Mutex<Option<String>>,
    machine: Mutex<Option<String>>,
    writes: AtomicUsize,
}

impl MemoryRegistry {
    /// A registry holding these commands.
    pub fn holding(user: Option<&str>, machine: Option<&str>) -> MemoryRegistry {
        MemoryRegistry {
            user: Mutex::new(user.map(str::to_string)),
            machine: Mutex::new(machine.map(str::to_string)),
            writes: AtomicUsize::new(0),
        }
    }

    /// The user's command now.
    pub fn user(&self) -> Option<String> {
        self.user
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Another program writes the user's key (another manager taking the
    /// links back at its start, say).
    pub fn set_user(&self, command: Option<&str>) {
        *self.user.lock().unwrap_or_else(PoisonError::into_inner) = command.map(str::to_string);
    }

    /// How many times the stone wrote or removed the user's key.
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
        Ok(self
            .machine
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone())
    }

    fn effective_command(&self) -> Result<Option<String>, RiftError> {
        Ok(self.user().or(self.machine_command()?))
    }

    fn write_user(&self, command: &str) -> Result<(), RiftError> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        self.set_user(Some(command));
        Ok(())
    }

    fn remove_user(&self) -> Result<(), RiftError> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        self.set_user(None);
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

    use winreg::enums::{HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::RegKey;

    use rift_core::RiftError;

    use super::ProtocolRegistry;

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

        fn write_user(&self, command: &str) -> Result<(), RiftError> {
            let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
                .create_subkey(USER_KEY)
                .map_err(|err| write_error(&err))?;
            key.set_value("", &"URL:NXM Protocol")
                .map_err(|err| write_error(&err))?;
            key.set_value("URL Protocol", &"")
                .map_err(|err| write_error(&err))?;
            let (open, _) = key
                .create_subkey(COMMAND)
                .map_err(|err| write_error(&err))?;
            open.set_value("", &command)
                .map_err(|err| write_error(&err))
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
