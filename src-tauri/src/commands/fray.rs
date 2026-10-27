//! THE FRAY (#00015 §19): `fray_frame`, `open_fray`.
//!
//! The `fray` window pulls one frame every 250 ms (push travels by webview eval, which WebView2
//! starves). Read-only: the desk reads the eye's own folder, and nothing here writes.

use std::sync::Arc;
use std::time::Instant;

use rift_core::RiftError;
use rift_fray::Frame;
use tauri::Manager;

use crate::commands::{off_runtime, Spine};
use crate::fray::{FrayDesk, GameView};
use crate::state::AppState;

/// The window's label in tauri.conf.json.
pub const FRAY_WINDOW: &str = "fray";

pub fn fray_frame_impl(state: &AppState, desk: &FrayDesk) -> Frame {
    let run = state.game_run();
    let game = GameView {
        root: state.world().game.map(|g| g.root),
        running: run.running,
        started_ms: run.started_ms,
    };
    desk.frame(&game, Instant::now(), || crate::muster::codex(state).ok())
}

#[tauri::command]
pub async fn fray_frame(
    state: Spine<'_>,
    desk: tauri::State<'_, Arc<FrayDesk>>,
) -> Result<Frame, RiftError> {
    let state = Arc::clone(state.inner());
    let desk = Arc::clone(desk.inner());
    off_runtime(move || Ok(fray_frame_impl(&state, &desk))).await
}

/// Open the party screen, or bring it forward when it is open. It is made without focus, so the
/// game keeps the keyboard (FQ.8); the player drags it to the second monitor once, and Windows
/// remembers nothing for it yet (a later step).
#[tauri::command]
pub async fn open_fray<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Result<(), RiftError> {
    if let Some(window) = app.get_webview_window(FRAY_WINDOW) {
        return window.show().map_err(window_error);
    }
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == FRAY_WINDOW)
        .cloned()
        .ok_or_else(|| RiftError::Io {
            detail: "This build of the Riftstone carries no party screen.".into(),
        })?;
    tauri::WebviewWindowBuilder::from_config(&app, &config)
        .map_err(window_error)?
        .focused(false)
        .build()
        .map(|_| ())
        .map_err(window_error)
}

fn window_error(err: tauri::Error) -> RiftError {
    log::warn!("the party screen would not open: {err}");
    RiftError::Io { detail: "The party screen would not open. Try again; if it keeps failing, restart the Riftstone.".into() }
}
