//! The optional Nexus Mods personal API key (RS-6).
//!
//! It lives in the operating system's credential store — the Windows
//! Credential Manager on the investor's machine — under service
//! [`SERVICE`], account [`ACCOUNT`]. Never in a file, never in the webview.
//!
//! The key itself never reaches a DTO, a log line or an error: every failure
//! here is a fixed sentence chosen by the KIND of failure, so nothing the
//! store echoes back (a platform message, a blob) can carry it out.

use std::sync::Mutex;

use rift_core::RiftError;

/// The credential's service name (the app identifier).
pub const SERVICE: &str = "nl.zmuuzn.riftstone";
/// The credential's account name.
pub const ACCOUNT: &str = "nexus-api-key";

/// Where the key is kept.
pub trait KeyStore: Send + Sync {
    /// The stored key; `None` when there is none.
    fn read(&self) -> Result<Option<String>, RiftError>;
    /// Store (or replace) the key.
    fn store(&self, key: &str) -> Result<(), RiftError>;
    /// Remove the key; removing an absent key is fine.
    fn forget(&self) -> Result<(), RiftError>;
}

/// The operating system's credential store, through `keyring`.
pub struct SystemKeyStore;

impl SystemKeyStore {
    fn entry(action: &str) -> Result<keyring::Entry, RiftError> {
        keyring::Entry::new(SERVICE, ACCOUNT).map_err(|err| store_error(action, &err))
    }
}

impl KeyStore for SystemKeyStore {
    fn read(&self) -> Result<Option<String>, RiftError> {
        match Self::entry("read")?.get_password() {
            Ok(key) => Ok(Some(key).filter(|k| !k.trim().is_empty())),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(store_error("read", &err)),
        }
    }

    fn store(&self, key: &str) -> Result<(), RiftError> {
        Self::entry("store")?
            .set_password(key)
            .map_err(|err| store_error("store", &err))
    }

    fn forget(&self) -> Result<(), RiftError> {
        match Self::entry("remove")?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(store_error("remove", &err)),
        }
    }
}

/// A plain sentence for a credential-store failure. Deliberately ignores
/// everything the error carries.
fn store_error(action: &str, err: &keyring::Error) -> RiftError {
    let why = match err {
        keyring::Error::NoStorageAccess(_) => "the credential store is locked or refused access",
        keyring::Error::PlatformFailure(_) => "the credential store reported a failure",
        keyring::Error::NoDefaultStore => {
            "this system has no credential store the Riftstone can use"
        }
        keyring::Error::TooLong(..) => "the key is longer than the credential store accepts",
        keyring::Error::BadEncoding(_) | keyring::Error::BadDataFormat(..) => {
            "the stored key is damaged; set it again"
        }
        keyring::Error::NoEntry => "no key is stored",
        _ => "the credential store answered in a way the Riftstone does not understand",
    };
    RiftError::Io {
        detail: format!(
            "Could not {action} the Nexus Mods API key: {why}. Update checks keep working without one."
        ),
    }
}

/// A key store in memory — the tests' credential manager.
#[derive(Default)]
pub struct MemoryKeyStore {
    key: Mutex<Option<String>>,
}

impl MemoryKeyStore {
    pub fn holding(key: &str) -> MemoryKeyStore {
        MemoryKeyStore {
            key: Mutex::new(Some(key.to_string())),
        }
    }
}

impl KeyStore for MemoryKeyStore {
    fn read(&self) -> Result<Option<String>, RiftError> {
        Ok(self
            .key
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone())
    }

    fn store(&self, key: &str) -> Result<(), RiftError> {
        *self
            .key
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(key.to_string());
        Ok(())
    }

    fn forget(&self) -> Result<(), RiftError> {
        *self
            .key
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_credential_names_are_the_ruled_ones() {
        assert_eq!(SERVICE, "nl.zmuuzn.riftstone");
        assert_eq!(ACCOUNT, "nexus-api-key");
    }

    #[test]
    fn store_failures_never_echo_what_the_store_returned() {
        // A damaged blob is the one failure that carries the stored bytes.
        let secret = b"abcdef0123456789-very-secret".to_vec();
        let err = store_error("read", &keyring::Error::BadEncoding(secret));
        let text = format!("{err} {err:?} {}", serde_json::to_string(&err).unwrap());
        assert!(!text.contains("very-secret"), "{text}");
        assert!(text.contains("damaged"));
    }

    #[test]
    fn the_memory_store_behaves_like_a_credential_manager() {
        let store = MemoryKeyStore::default();
        assert_eq!(store.read().unwrap(), None);
        store.store("k").unwrap();
        assert_eq!(store.read().unwrap().as_deref(), Some("k"));
        store.forget().unwrap();
        store.forget().unwrap();
        assert_eq!(store.read().unwrap(), None);
    }
}
