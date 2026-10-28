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

    /// v0.4: a save slot would not open. `reason` names the case for the window; `detail` is
    /// a sentence that names the slot by its file name, never by its path (the path holds the
    /// Steam account number).
    #[error("{detail}")]
    Save { detail: String, reason: SaveReason },

    /// v0.7: THE WORKSHOP refused something about the pawn (WINDOW-CONTRACT
    /// § v0.7, Errors). Produced only by a build with the `pawn` feature.
    #[error("{detail}")]
    Pawn { detail: String, reason: PawnReason },

    /// v0.7: a build could not be brought through into the library (RS-2's
    /// second exception). Produced only by a build with the `pawn` feature.
    #[error("{detail}")]
    Bring { detail: String, reason: BringReason },

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

/// Why THE WORKSHOP refused (v0.7), for the window to voice by code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PawnReason {
    /// The fence cannot be built here; the sentence names the missing piece
    /// and the command that installs it.
    NoFence,
    /// The fence's last probe on this ground let something through.
    Breached,
    AlreadyHere,
    NotHere,
    /// The probe cannot run while a pawn is summoned.
    PawnHere,
    NothingHeld,
    /// A Play press for a run or request that is not the one waiting now.
    StaleHeld,
    /// The run's ledger cannot be written, which stops every change in it.
    LedgerDown,
    /// A read-only input overlaps a protected root; the sentence names the root.
    InputRefused,
    /// A Put back id that is not that run's own still-installed install.
    NotThisRun,
}

/// Why a build could not be brought through (v0.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BringReason {
    NameTaken,
    BytesChanged,
    NoReceipt,
    ReceiptMismatch,
    NotInDist,
}

impl RiftError {
    pub fn pawn(reason: PawnReason, detail: impl Into<String>) -> Self {
        RiftError::Pawn {
            detail: detail.into(),
            reason,
        }
    }

    pub fn bring(reason: BringReason, detail: impl Into<String>) -> Self {
        RiftError::Bring {
            detail: detail.into(),
            reason,
        }
    }
}

/// Why a save slot would not open (v0.4), so the window can voice it without reading the
/// sentence. Serialized camelCase into `RiftError::Save`'s detail, and into a slot's
/// `problem` in the Muster's list (which the DTO tests read back, hence `Deserialize`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SaveReason {
    /// The slot opens with no Steam account the stone found: another account's save.
    OtherAccount,
    /// The slot's signature disagreed twice while its file changed: the game is writing it.
    BeingWritten,
    /// The container or its stream is broken.
    Damaged,
    /// The file opens but holds no party.
    NoParty,
    /// The slot left the folder since the list was read.
    Gone,
}

impl RiftError {
    /// A save slot that would not open, for the window to voice by `reason`.
    pub fn save(reason: SaveReason, detail: impl Into<String>) -> Self {
        RiftError::Save {
            detail: detail.into(),
            reason,
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
    #[test]
    fn the_workshops_two_kinds_cross_the_bridge_with_their_reason() {
        let json = serde_json::to_value(RiftError::pawn(
            PawnReason::NoFence,
            "socat is missing in WSL.",
        ))
        .unwrap();
        assert_eq!(json["kind"], "pawn");
        assert_eq!(json["detail"]["reason"], "noFence");
        assert_eq!(json["detail"]["detail"], "socat is missing in WSL.");
        let json =
            serde_json::to_value(RiftError::bring(BringReason::BytesChanged, "changed")).unwrap();
        assert_eq!(json["kind"], "bring");
        assert_eq!(json["detail"]["reason"], "bytesChanged");
    }

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
