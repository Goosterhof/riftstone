//! The Riftstone's failure vocabulary.
//!
//! Every error crosses the Tauri bridge as `{ "kind": "...", "detail": {...} }`.
//! The backend states facts; the frontend gives them the Error Manifesto's
//! voice (what happened, that we understand what you tried, what to do next).

use serde::Serialize;

#[derive(Debug, Clone, thiserror::Error, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
pub enum RiftError {
    /// A file or folder could not be read (permissions, vanished, locked).
    #[error("could not read {path}: {detail}")]
    Unreadable { path: String, detail: String },

    /// The bytes are not a zip, 7z, rar or folder the Riftstone can open.
    #[error("{path} is not an archive the Riftstone can open")]
    UnsupportedFormat { path: String },

    /// The archive is password-protected.
    #[error("{path} is password-protected")]
    Encrypted { path: String },

    /// The archive opened but its structure is broken.
    #[error("{path} is damaged: {detail}")]
    CorruptArchive { path: String, detail: String },

    /// An entry path tries to climb out of its folder (`..`, absolute paths,
    /// drive letters). Never extracted — the zip-slip guard.
    #[error("refused unsafe entry path {entry}")]
    UnsafeEntryPath { entry: String },

    /// The entry asked for is not in the package.
    #[error("{entry} is not in the package")]
    EntryNotFound { entry: String },

    /// No Dragon's Dogma 2 install could be found.
    #[error("Dragon's Dogma 2 is not installed where the Riftstone looked")]
    GameNotFound,

    /// A deploy guard refused an operation (vanilla protection, game running,
    /// a path outside the game folder). `reason` is a full sentence.
    #[error("refused: {reason}")]
    Refused { reason: String },

    /// The deployment journal could not be read or written.
    #[error("the journal failed: {detail}")]
    Journal { detail: String },

    /// Nexus Mods could not be reached, refused the key, or answered oddly.
    #[error("Nexus Mods: {detail}")]
    Nexus { detail: String },

    /// Any other filesystem failure while writing.
    #[error("{detail}")]
    Io { detail: String },
}

impl RiftError {
    pub fn unreadable(path: impl AsRef<std::path::Path>, err: impl std::fmt::Display) -> Self {
        RiftError::Unreadable {
            path: path.as_ref().display().to_string(),
            detail: err.to_string(),
        }
    }

    pub fn corrupt(path: impl AsRef<std::path::Path>, err: impl std::fmt::Display) -> Self {
        RiftError::CorruptArchive {
            path: path.as_ref().display().to_string(),
            detail: err.to_string(),
        }
    }

    pub fn io(context: impl std::fmt::Display, err: impl std::fmt::Display) -> Self {
        RiftError::Io {
            detail: format!("{context}: {err}"),
        }
    }

    pub fn refused(reason: impl Into<String>) -> Self {
        RiftError::Refused {
            reason: reason.into(),
        }
    }
}

pub type RiftResult<T> = Result<T, RiftError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_cross_the_bridge_as_kind_and_detail() {
        let err = RiftError::EntryNotFound {
            entry: "natives/stm/a.mesh".into(),
        };
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["kind"], "entryNotFound");
        assert_eq!(json["detail"]["entry"], "natives/stm/a.mesh");
    }

    #[test]
    fn unit_variant_serialises_with_kind_only() {
        let json = serde_json::to_value(RiftError::GameNotFound).unwrap();
        assert_eq!(json["kind"], "gameNotFound");
    }
}
