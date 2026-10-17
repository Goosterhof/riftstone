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
    /// `reason` (v0.3) names the case for the window; `None` for an odd answer.
    #[error("Nexus Mods: {detail}")]
    Nexus {
        detail: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<NexusReason>,
    },

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

/// Why Nexus said no (v0.3), so the window can voice it without reading the
/// sentence. Serialized camelCase into `RiftError::Nexus`'s detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NexusReason {
    Offline,
    RateLimited,
    NoKey,
    KeyRejected,
    /// `download_link.json` without the site's link: premium accounts only.
    PremiumRequired,
    /// The site's Mod Manager Download link has expired (HTTP 410).
    LinkExpired,
    /// The link was made for another account than the stored key's.
    LinkNotYours,
    /// Nexus refused the link's key and expiry for this file (HTTP 400).
    LinkRefused,
    /// The mod or file is gone from Nexus (HTTP 404).
    FileGone,
    /// The bytes that arrived are not the size Nexus stated.
    SizeMismatch,
    /// The bytes that arrived hash to another file than the one asked for.
    NotTheFile,
    /// Endorsing a mod this account never downloaded from Nexus.
    EndorseNotDownloaded,
    /// Endorsing within 15 minutes of the download.
    EndorseTooSoon,
    /// Endorsing one's own mod.
    EndorseOwnMod,
}

impl RiftError {
    /// A Nexus failure with no particular reason (an odd answer).
    pub fn nexus(detail: impl Into<String>) -> Self {
        RiftError::Nexus {
            detail: detail.into(),
            reason: None,
        }
    }

    /// A Nexus failure the window can voice by its reason.
    pub fn nexus_because(reason: NexusReason, detail: impl Into<String>) -> Self {
        RiftError::Nexus {
            detail: detail.into(),
            reason: Some(reason),
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
    fn a_nexus_failure_carries_its_reason_only_when_it_has_one() {
        let json = serde_json::to_value(RiftError::nexus_because(
            NexusReason::PremiumRequired,
            "only premium accounts may",
        ))
        .unwrap();
        assert_eq!(json["kind"], "nexus");
        assert_eq!(json["detail"]["reason"], "premiumRequired");
        assert_eq!(json["detail"]["detail"], "only premium accounts may");
        let plain = serde_json::to_value(RiftError::nexus("an odd answer")).unwrap();
        assert!(plain["detail"].get("reason").is_none());
    }

    #[test]
    fn unit_variant_serialises_with_kind_only() {
        let json = serde_json::to_value(RiftError::GameNotFound).unwrap();
        assert_eq!(json["kind"], "gameNotFound");
    }
}
