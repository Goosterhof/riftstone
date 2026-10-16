//! What can go wrong between the Riftstone and Nexus Mods.
//!
//! Every message is a plain sentence: what happened, then what to do. None of
//! them is fatal — the Riftstone works offline, so a Nexus failure only means
//! the update column stays as it was.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
pub enum NexusError {
    /// No answer at all: no connection, DNS failure, refused, timed out. The
    /// string is the technical reason.
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
    #[error(
        "Nexus Mods answered with HTTP status {0}. The site may be busy or down; try again \
         in a few minutes."
    )]
    Http(u16),
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

impl From<NexusError> for rift_core::RiftError {
    fn from(err: NexusError) -> Self {
        rift_core::RiftError::Nexus {
            detail: err.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[test]
    fn crosses_into_the_shared_error_vocabulary() {
        let rift: rift_core::RiftError = NexusError::Http(502).into();
        match rift {
            rift_core::RiftError::Nexus { detail } => assert!(detail.contains("502")),
            other => panic!("expected RiftError::Nexus, got {other:?}"),
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
    }
}
