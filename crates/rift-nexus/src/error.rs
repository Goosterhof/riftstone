//! What can go wrong between the Riftstone and Nexus Mods.
//!
//! Every message is a plain sentence: what happened, then what to do. None of
//! them is fatal — the Riftstone works offline, so a Nexus failure only means
//! the update column stays as it was, or one download, endorsement or track
//! did not happen.
//!
//! Two rules hold for every sentence here:
//!
//! - No sentence says "401", "403" or "reject" unless the API key really was
//!   rejected ([`NexusError::KeyRejected`]). A premium-only refusal, an expired
//!   link or a CDN that would not serve is not the key's fault, and the player
//!   must never be sent off to replace a key that works.
//! - No sentence carries a secret: never the API key, never an nxm link's
//!   `key`, never a download link (the CDN URI signs the request). The errors
//!   are built from fixed words and from the innermost transport reason only.

use rift_core::NexusReason;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
pub enum NexusError {
    /// No answer at all: no connection, DNS failure, refused, timed out, or a
    /// download that broke off. The string is the technical reason.
    #[error(
        "Nexus Mods could not be reached ({0}). Everything else in the Riftstone works \
         offline; check the internet connection and try again."
    )]
    Offline(String),

    /// HTTP 429, or a quota/back-off window that has not passed yet.
    #[error("{}", rate_limited_sentence(*retry_after_s))]
    #[serde(rename_all = "camelCase")]
    RateLimited { retry_after_s: Option<u64> },

    /// v1 answered 401: the personal API key is wrong, revoked or expired.
    #[error(
        "Nexus Mods rejected the API key. Paste a fresh personal API key from your Nexus \
         Mods account (Site preferences, API Keys); update checks keep working without one."
    )]
    KeyRejected,

    /// A v1 call was asked for without an API key configured.
    #[error(
        "This needs a Nexus Mods personal API key and none is set. Update checks work \
         without one; add a key in Settings only for the extras."
    )]
    NoApiKey,

    /// The answer arrived but could not be used (GraphQL errors, unexpected
    /// JSON, a body far too large). The string is the technical reason.
    #[error(
        "Nexus Mods sent an answer the Riftstone could not use ({0}). Try again later; if it \
         keeps happening, the Nexus API may have changed and the Riftstone needs an update."
    )]
    BadResponse(String),

    /// Any other non-success HTTP status.
    #[error("{}", http_sentence(.0))]
    Http(u16),

    /// v0.3: `download_link.json` without the site's link (HTTP 403 there):
    /// Nexus serves direct downloads to premium accounts only.
    #[error(
        "Nexus Mods lets only premium accounts download straight from the Riftstone. Press \
         Mod Manager Download on the file's Nexus Mods page instead; the Riftstone takes the \
         download over from there."
    )]
    PremiumRequired,

    /// v0.3: the site's Mod Manager Download link has expired (HTTP 410), or
    /// the CDN would no longer serve the download link it signed.
    #[error(
        "The download link from Nexus Mods has expired. Press Mod Manager Download on the \
         file's Nexus Mods page again for a fresh one."
    )]
    LinkExpired,

    /// v0.3: Nexus did not accept the site link's key and expiry for this
    /// file and account (HTTP 400).
    #[error(
        "Nexus Mods did not accept the download link for this file and account. Press Mod \
         Manager Download on the file's Nexus Mods page again, signed in with the account \
         whose API key the Riftstone holds."
    )]
    LinkRefused,

    /// v0.3: the mod or the file is gone from Nexus (HTTP 404).
    #[error(
        "That mod or file is no longer on Nexus Mods: the author removed it, or Nexus took \
         it down. Look on the mod's page for what replaced it."
    )]
    FileGone,

    /// v0.3: the player stopped the download. Its partial file was removed.
    #[error("The download was cancelled. Nothing was left behind in the mods folder.")]
    Cancelled,

    /// v0.3: the bytes that arrived are not the size Nexus (or the server)
    /// stated. The partial file was removed.
    #[error(
        "The download brought {got} bytes where {expected} were stated, so it was not kept. \
         Try the download again."
    )]
    SizeMismatch { expected: u64, got: u64 },

    /// v0.3: the archive's MD5 names another Nexus file than the one asked
    /// for. The spine's verifying step decides this ([`crate::NexusClient::confirm_download`]).
    #[error(
        "The archive that arrived is not the file Nexus Mods named, so it was not kept. Try \
         the download again later."
    )]
    NotTheFile,

    /// v0.3: the download could not be written to disk: the destination
    /// already exists, the folder is missing or read-only, or the disk is
    /// full. The string is the technical reason.
    #[error(
        "The download could not be written to disk ({0}). Check that the folder exists, can \
         be written to and has room, then try again."
    )]
    Disk(String),

    /// v0.3: Nexus refused an endorsement or an abstention.
    #[error("{0}")]
    EndorseRefused(EndorseRefusal),
}

/// Why Nexus refused an endorsement (v1 `endorse.json` / `abstain.json`),
/// read from the refusal code Nexus answers with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EndorseRefusal {
    /// `NOT_DOWNLOADED_MOD`: this account never downloaded the mod from Nexus.
    #[error(
        "Nexus Mods takes an endorsement only from an account that downloaded the mod from \
         Nexus. Download it there (or from the Riftstone) first, then endorse it."
    )]
    NotDownloaded,
    /// `TOO_SOON_AFTER_DOWNLOAD`: Nexus wants some time (about 15 minutes)
    /// between a download and an endorsement.
    #[error(
        "Nexus Mods wants a little time between a download and an endorsement (about 15 \
         minutes). Try again later."
    )]
    TooSoon,
    /// `IS_OWN_MOD`: authors cannot endorse their own mods.
    #[error("Nexus Mods does not let authors endorse their own mods.")]
    OwnMod,
}

impl EndorseRefusal {
    /// The refusal a Nexus answer names, if any: `NOT_DOWNLOADED_MOD`,
    /// `TOO_SOON_AFTER_DOWNLOAD` or `IS_OWN_MOD`, found anywhere in the text,
    /// any case.
    pub fn from_answer(text: &str) -> Option<Self> {
        let upper = text.to_ascii_uppercase();
        [
            ("NOT_DOWNLOADED_MOD", EndorseRefusal::NotDownloaded),
            ("TOO_SOON_AFTER_DOWNLOAD", EndorseRefusal::TooSoon),
            ("IS_OWN_MOD", EndorseRefusal::OwnMod),
        ]
        .into_iter()
        .find(|(code, _)| upper.contains(code))
        .map(|(_, refusal)| refusal)
    }
}

impl NexusError {
    /// The reason the window voices this failure by (rift-core's
    /// [`NexusReason`]); `None` for an odd answer, an HTTP status with no
    /// meaning of its own, a cancel or a disk failure.
    pub fn reason(&self) -> Option<NexusReason> {
        Some(match self {
            NexusError::Offline(_) => NexusReason::Offline,
            NexusError::RateLimited { .. } => NexusReason::RateLimited,
            NexusError::KeyRejected => NexusReason::KeyRejected,
            NexusError::NoApiKey => NexusReason::NoKey,
            NexusError::PremiumRequired => NexusReason::PremiumRequired,
            NexusError::LinkExpired => NexusReason::LinkExpired,
            NexusError::LinkRefused => NexusReason::LinkRefused,
            NexusError::FileGone => NexusReason::FileGone,
            NexusError::SizeMismatch { .. } => NexusReason::SizeMismatch,
            NexusError::NotTheFile => NexusReason::NotTheFile,
            NexusError::EndorseRefused(EndorseRefusal::NotDownloaded) => {
                NexusReason::EndorseNotDownloaded
            }
            NexusError::EndorseRefused(EndorseRefusal::TooSoon) => NexusReason::EndorseTooSoon,
            NexusError::EndorseRefused(EndorseRefusal::OwnMod) => NexusReason::EndorseOwnMod,
            NexusError::BadResponse(_)
            | NexusError::Http(_)
            | NexusError::Cancelled
            | NexusError::Disk(_) => return None,
        })
    }
}

fn rate_limited_sentence(retry_after_s: Option<u64>) -> String {
    match retry_after_s {
        Some(0) | Some(1) => {
            "Nexus Mods asked the Riftstone to slow down. Try again in a moment.".to_string()
        }
        Some(seconds) if seconds < 120 => {
            format!("Nexus Mods asked the Riftstone to slow down. Try again in {seconds} seconds.")
        }
        Some(seconds) => format!(
            "Nexus Mods asked the Riftstone to slow down. Try again in about {} minutes.",
            seconds.div_ceil(60)
        ),
        None => {
            "Nexus Mods asked the Riftstone to slow down. Try again in a few minutes.".to_string()
        }
    }
}

/// An HTTP status in words. 401 and 403 are not spelled: they would read as
/// "your key was rejected", which only [`NexusError::KeyRejected`] may say.
fn http_sentence(code: &u16) -> String {
    match code {
        401 | 403 => "Nexus Mods would not serve this request. Try again in a few minutes; if \
                      it keeps happening, look at the account on the Nexus Mods site."
            .to_string(),
        code => format!(
            "Nexus Mods answered with HTTP status {code}. The site may be busy or down; try \
             again in a few minutes."
        ),
    }
}

/// Into the shared vocabulary: a Nexus failure the window can voice by its
/// reason ([`RiftError::nexus_because`](rift_core::RiftError::nexus_because)),
/// or by its sentence alone. A disk failure is a disk failure
/// ([`rift_core::RiftError::Io`]), not a Nexus one.
impl From<NexusError> for rift_core::RiftError {
    fn from(err: NexusError) -> Self {
        let detail = err.to_string();
        match (err.reason(), err) {
            (Some(reason), _) => rift_core::RiftError::nexus_because(reason, detail),
            (None, NexusError::Disk(_)) => rift_core::RiftError::Io { detail },
            (None, _) => rift_core::RiftError::nexus(detail),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rift_core::RiftError;

    /// Every variant once, with stand-in values.
    fn every_error() -> Vec<NexusError> {
        vec![
            NexusError::Offline("connection refused".into()),
            NexusError::RateLimited {
                retry_after_s: Some(30),
            },
            NexusError::KeyRejected,
            NexusError::NoApiKey,
            NexusError::BadResponse("odd".into()),
            NexusError::Http(401),
            NexusError::Http(403),
            NexusError::Http(503),
            NexusError::PremiumRequired,
            NexusError::LinkExpired,
            NexusError::LinkRefused,
            NexusError::FileGone,
            NexusError::Cancelled,
            NexusError::SizeMismatch {
                expected: 2048,
                got: 1024,
            },
            NexusError::NotTheFile,
            NexusError::Disk("No space left on device (os error 28)".into()),
            NexusError::EndorseRefused(EndorseRefusal::NotDownloaded),
            NexusError::EndorseRefused(EndorseRefusal::TooSoon),
            NexusError::EndorseRefused(EndorseRefusal::OwnMod),
        ]
    }

    #[test]
    fn messages_say_what_happened_and_what_to_do() {
        let offline = NexusError::Offline("connection refused".into()).to_string();
        assert!(offline.contains("connection refused") && offline.contains("try again"));
        assert_eq!(
            NexusError::RateLimited {
                retry_after_s: Some(30)
            }
            .to_string(),
            "Nexus Mods asked the Riftstone to slow down. Try again in 30 seconds."
        );
        assert!(NexusError::RateLimited {
            retry_after_s: Some(600)
        }
        .to_string()
        .contains("about 10 minutes"));
        assert!(NexusError::RateLimited {
            retry_after_s: None
        }
        .to_string()
        .contains("a few minutes"));
        assert!(NexusError::Http(503).to_string().contains("503"));
        assert!(NexusError::KeyRejected.to_string().contains("API key"));
        let mismatch = NexusError::SizeMismatch {
            expected: 2048,
            got: 1024,
        }
        .to_string();
        assert!(
            mismatch.contains("2048") && mismatch.contains("1024"),
            "{mismatch}"
        );
        assert!(NexusError::Disk("disk full".into())
            .to_string()
            .contains("disk full"));
        for err in every_error() {
            let sentence = err.to_string();
            assert!(sentence.ends_with('.'), "a full sentence: {sentence}");
            assert!(sentence.len() > 40, "says what to do: {sentence}");
        }
    }

    #[test]
    fn only_a_rejected_key_says_rejected_or_names_401_or_403() {
        for err in every_error() {
            let sentence = err.to_string();
            let speaks_of_rejection = sentence.contains("401")
                || sentence.contains("403")
                || sentence.to_lowercase().contains("reject");
            assert_eq!(
                speaks_of_rejection,
                err == NexusError::KeyRejected,
                "{err:?}: {sentence}"
            );
        }
    }

    #[test]
    fn crosses_into_the_shared_error_vocabulary() {
        let rift: RiftError = NexusError::Http(502).into();
        match rift {
            RiftError::Nexus { detail, reason } => {
                assert!(detail.contains("502"));
                assert_eq!(reason, None, "an HTTP status names no reason");
            }
            other => panic!("expected RiftError::Nexus, got {other:?}"),
        }
    }

    #[test]
    fn every_failure_carries_its_reason_into_the_shared_vocabulary() {
        let cases = [
            (
                NexusError::Offline("dns".into()),
                Some(NexusReason::Offline),
            ),
            (
                NexusError::RateLimited {
                    retry_after_s: None,
                },
                Some(NexusReason::RateLimited),
            ),
            (NexusError::NoApiKey, Some(NexusReason::NoKey)),
            (NexusError::KeyRejected, Some(NexusReason::KeyRejected)),
            (
                NexusError::PremiumRequired,
                Some(NexusReason::PremiumRequired),
            ),
            (NexusError::LinkExpired, Some(NexusReason::LinkExpired)),
            (NexusError::LinkRefused, Some(NexusReason::LinkRefused)),
            (NexusError::FileGone, Some(NexusReason::FileGone)),
            (
                NexusError::SizeMismatch {
                    expected: 2,
                    got: 1,
                },
                Some(NexusReason::SizeMismatch),
            ),
            (NexusError::NotTheFile, Some(NexusReason::NotTheFile)),
            (
                NexusError::EndorseRefused(EndorseRefusal::NotDownloaded),
                Some(NexusReason::EndorseNotDownloaded),
            ),
            (
                NexusError::EndorseRefused(EndorseRefusal::TooSoon),
                Some(NexusReason::EndorseTooSoon),
            ),
            (
                NexusError::EndorseRefused(EndorseRefusal::OwnMod),
                Some(NexusReason::EndorseOwnMod),
            ),
            (NexusError::BadResponse("odd".into()), None),
            (NexusError::Http(500), None),
            (NexusError::Cancelled, None),
        ];
        for (err, expected) in cases {
            let sentence = err.to_string();
            assert_eq!(err.reason(), expected, "{err:?}");
            match RiftError::from(err.clone()) {
                RiftError::Nexus { detail, reason } => {
                    assert_eq!(reason, expected, "{err:?}");
                    assert_eq!(detail, sentence, "the sentence travels as the detail");
                }
                other => panic!("{err:?} became {other:?}"),
            }
        }
        // A disk failure is an io failure, not a Nexus one.
        match RiftError::from(NexusError::Disk("disk full".into())) {
            RiftError::Io { detail } => assert!(detail.contains("disk full")),
            other => panic!("expected RiftError::Io, got {other:?}"),
        }
    }

    #[test]
    fn serialises_as_kind_and_detail() {
        let json = serde_json::to_value(NexusError::RateLimited {
            retry_after_s: Some(9),
        })
        .unwrap();
        assert_eq!(json["kind"], "rateLimited");
        assert_eq!(json["detail"]["retryAfterS"], 9);
        let json = serde_json::to_value(NexusError::Offline("dns".into())).unwrap();
        assert_eq!(json["kind"], "offline");
        assert_eq!(json["detail"], "dns");
        let json = serde_json::to_value(NexusError::KeyRejected).unwrap();
        assert_eq!(json["kind"], "keyRejected");
        let json = serde_json::to_value(NexusError::SizeMismatch {
            expected: 2,
            got: 1,
        })
        .unwrap();
        assert_eq!(json["kind"], "sizeMismatch");
        assert_eq!(
            json["detail"],
            serde_json::json!({ "expected": 2, "got": 1 })
        );
        let json =
            serde_json::to_value(NexusError::EndorseRefused(EndorseRefusal::TooSoon)).unwrap();
        assert_eq!(json["kind"], "endorseRefused");
        assert_eq!(json["detail"], "tooSoon");
        let json = serde_json::to_value(NexusError::PremiumRequired).unwrap();
        assert_eq!(json["kind"], "premiumRequired");
    }

    #[test]
    fn endorse_refusals_are_read_from_nexus_codes_anywhere_in_the_answer() {
        assert_eq!(
            EndorseRefusal::from_answer(r#"{"message":"NOT_DOWNLOADED_MOD"}"#),
            Some(EndorseRefusal::NotDownloaded)
        );
        assert_eq!(
            EndorseRefusal::from_answer(r#"{"code":403,"message":"too_soon_after_download"}"#),
            Some(EndorseRefusal::TooSoon)
        );
        assert_eq!(
            EndorseRefusal::from_answer("IS_OWN_MOD"),
            Some(EndorseRefusal::OwnMod)
        );
        assert_eq!(
            EndorseRefusal::from_answer(
                r#"{"message":"Updated to: Endorsed","status":"Endorsed"}"#
            ),
            None
        );
        assert_eq!(EndorseRefusal::from_answer(""), None);
    }
}
