//! The six events the spine sends the window (docs/WINDOW-CONTRACT.md § Events),
//! and the seam they travel through.
//!
//! Command bodies never touch Tauri: they emit through an [`EventSink`]. The
//! app hands them [`TauriEvents`]; the tests hand them [`RecordedEvents`] and
//! read back what would have reached the window.

use std::sync::Mutex;

use serde::Serialize;
use serde_json::Value;

/// After each package of a scan — `ScanProgressEvent`.
pub const SCAN_PROGRESS: &str = "rift://scan-progress";
/// A scan finished (manual, startup, or the library watcher) — `{}`.
pub const LIBRARY_CHANGED: &str = "rift://library-changed";
/// During extraction — `InstallProgressEvent`.
pub const INSTALL_PROGRESS: &str = "rift://install-progress";
/// After install / uninstall / reorder / adopt / switch / game root change,
/// and after a parked file is put back or discarded (v0.2) — `{}`. A refused
/// command changed nothing and sends nothing.
pub const GAME_CHANGED: &str = "rift://game-changed";
/// During `check_updates` / `identify_packages` — `UpdatesProgressEvent`.
pub const UPDATES_PROGRESS: &str = "rift://updates-progress";
/// After an update check or a link change — `{}`.
pub const UPDATES_CHANGED: &str = "rift://updates-changed";

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
