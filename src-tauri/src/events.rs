//! The events the spine sends the window (docs/WINDOW-CONTRACT.md § Events),
//! and the seam they travel through.
//!
//! Command bodies never touch Tauri: they emit through an [`EventSink`]. The
//! app hands them [`TauriEvents`]; the tests hand them [`RecordedEvents`] and
//! read back what would have reached the window.

use std::sync::Mutex;

use serde::Serialize;
use serde_json::Value;

/// After each package of a scan, and (v0.3) before each relink confirmation
/// the scan reads — `ScanProgressEvent`.
pub const SCAN_PROGRESS: &str = "rift://scan-progress";
/// A scan finished (manual, startup, or the library watcher) — `{}`.
pub const LIBRARY_CHANGED: &str = "rift://library-changed";
/// During extraction — `InstallProgressEvent`.
pub const INSTALL_PROGRESS: &str = "rift://install-progress";
/// After install / uninstall / reorder / switch / game root change, after a
/// parked file is put back or discarded (v0.2), after installs whose mods
/// moved were found again (v0.3), and when the game watcher sees DD2.exe
/// start or stop (v0.3) — `{}`. A refused command changed nothing and sends
/// nothing.
pub const GAME_CHANGED: &str = "rift://game-changed";
/// During `check_updates` / `identify_packages` — `UpdatesProgressEvent`.
pub const UPDATES_PROGRESS: &str = "rift://updates-progress";
/// After an update check or a link change (a moved package's link record
/// following it, v0.3, too); v0.3: also after a download lands (its package
/// is linked by the download) and after the account's standing changes (its
/// key validated or refused, a mod tracked or endorsed) — `{}`.
pub const UPDATES_CHANGED: &str = "rift://updates-changed";
/// v0.3: a download row appeared, changed phase or was cleared — `{}`.
pub const DOWNLOADS_CHANGED: &str = "rift://downloads-changed";
/// v0.3: while a download moves bytes: at most one per 200 ms per download,
/// plus the last — `DownloadProgressEvent`.
pub const DOWNLOAD_PROGRESS: &str = "rift://download-progress";
/// v0.3: an nxm:// link reached the stone (cold start or a second launch) —
/// `NxmReceivedEvent`. The spine also brings the window forward.
pub const NXM_RECEIVED: &str = "rift://nxm-received";
/// v0.4: a slot file appeared, changed or went (the game saved), or the game's names were
/// rebuilt after a game update — `{}`.
pub const MUSTER_CHANGED: &str = "rift://muster-changed";

/// Every event the spine sends, as `src/types/rift.ts` `EVENTS` names them.
pub const ALL: &[&str] = &[
    SCAN_PROGRESS,
    LIBRARY_CHANGED,
    INSTALL_PROGRESS,
    GAME_CHANGED,
    UPDATES_PROGRESS,
    UPDATES_CHANGED,
    DOWNLOADS_CHANGED,
    DOWNLOAD_PROGRESS,
    NXM_RECEIVED,
    MUSTER_CHANGED,
];

/// Where events go.
pub trait EventSink: Send + Sync {
    fn emit_value(&self, event: &'static str, payload: Value);
}

impl dyn EventSink {
    /// Serialize `payload` and send it. A payload that cannot serialize is a
    /// bug in this crate; it is logged, never sent half-formed.
    pub fn emit<T: Serialize>(&self, event: &'static str, payload: &T) {
        match serde_json::to_value(payload) {
            Ok(value) => self.emit_value(event, value),
            Err(err) => log::error!("the {event} payload could not be encoded: {err}"),
        }
    }

    /// Send the `{}` payload of the "something changed" events.
    pub fn changed(&self, event: &'static str) {
        self.emit_value(event, Value::Object(serde_json::Map::new()));
    }
}

/// The real window.
pub struct TauriEvents(pub tauri::AppHandle);

impl EventSink for TauriEvents {
    fn emit_value(&self, event: &'static str, payload: Value) {
        use tauri::Emitter;
        if let Err(err) = self.0.emit(event, payload) {
            log::warn!("the window did not hear {event}: {err}");
        }
    }
}

/// Events nobody listens to.
pub struct NoEvents;

impl EventSink for NoEvents {
    fn emit_value(&self, _event: &'static str, _payload: Value) {}
}

/// Every event, kept in order — the tests' window.
#[derive(Default)]
pub struct RecordedEvents {
    seen: Mutex<Vec<(String, Value)>>,
}

impl RecordedEvents {
    /// Every event so far, oldest first.
    pub fn all(&self) -> Vec<(String, Value)> {
        self.seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// The payloads sent under `event`, oldest first.
    pub fn named(&self, event: &str) -> Vec<Value> {
        self.all()
            .into_iter()
            .filter(|(name, _)| name == event)
            .map(|(_, payload)| payload)
            .collect()
    }

    pub fn clear(&self) {
        self.seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }
}

impl EventSink for RecordedEvents {
    fn emit_value(&self, event: &'static str, payload: Value) {
        self.seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((event.to_string(), payload));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::ScanProgressEvent;
    use std::sync::Arc;

    #[test]
    fn event_names_match_the_window_contract() {
        // src/types/rift.ts EVENTS
        assert_eq!(SCAN_PROGRESS, "rift://scan-progress");
        assert_eq!(LIBRARY_CHANGED, "rift://library-changed");
        assert_eq!(INSTALL_PROGRESS, "rift://install-progress");
        assert_eq!(GAME_CHANGED, "rift://game-changed");
        assert_eq!(UPDATES_PROGRESS, "rift://updates-progress");
        assert_eq!(UPDATES_CHANGED, "rift://updates-changed");
        assert_eq!(DOWNLOADS_CHANGED, "rift://downloads-changed");
        assert_eq!(DOWNLOAD_PROGRESS, "rift://download-progress");
        assert_eq!(NXM_RECEIVED, "rift://nxm-received");
    }

    /// The window's wire, as it is built.
    const WIRE: &str = include_str!("../../src/types/rift.ts");

    #[test]
    fn the_window_listens_for_exactly_the_events_the_spine_sends() {
        let start = WIRE
            .find("export const EVENTS = {")
            .expect("rift.ts declares EVENTS");
        let block = &WIRE[start..];
        let block = &block[..block.find("} as const;").expect("EVENTS closes")];
        let mut window: Vec<&str> = block
            .split('"')
            .filter(|piece| piece.starts_with("rift://"))
            .collect();
        window.sort_unstable();
        let mut spine = ALL.to_vec();
        spine.sort_unstable();
        assert_eq!(spine, window, "one list of events on both sides");
    }

    #[test]
    fn payloads_serialize_and_changes_carry_an_empty_object() {
        let recorded = Arc::new(RecordedEvents::default());
        let sink: Arc<dyn EventSink> = recorded.clone();
        sink.emit(
            SCAN_PROGRESS,
            &ScanProgressEvent {
                done: 1,
                total: 2,
                current: "Vernworth Cloaks.zip".into(),
            },
        );
        sink.changed(LIBRARY_CHANGED);
        let all = recorded.all();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].1["current"], "Vernworth Cloaks.zip");
        assert_eq!(all[1], (LIBRARY_CHANGED.to_string(), serde_json::json!({})));
        assert_eq!(recorded.named(LIBRARY_CHANGED).len(), 1);
    }
}
