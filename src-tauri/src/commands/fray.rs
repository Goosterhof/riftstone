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
use crate::state::{read, write};

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

/// Open the party screen, or bring it to its monitor when it is open (v0.6).
///
/// With a second monitor, THE FRAY takes one whole: the War Tent's Second Screen, as proven on
/// the investor's desk (`war-tent-scientist/src-tauri/src/commands/overview.rs`). That is a
/// borderless window sized to exactly that monitor's bounds, NOT true fullscreen, and in physical
/// units throughout, so a mixed-DPI desk does not misplace it by the scale ratio. Which monitor:
/// the one it took last by name, else by place, else the last one ([`own_monitor`]). With one
/// monitor it opens as a plain window, since a window filling the only screen would cover the
/// game.
///
/// It never takes focus, so the game keeps the keyboard (FQ.8). That decides the order: tao
/// 0.37 force-activates a window BUILT fullscreen, and `focused(false)` only covers a window's
/// first show (the flag is cleared once it is made), so a window built hidden and shown later
/// takes focus. It is therefore built visible, already on its monitor, and then fitted, and
/// tao's moves and resizes pass `SWP_NOACTIVATE`. Moved to another monitor (Win+Shift+Arrow),
/// it fills that one and remembers it.
#[tauri::command]
pub async fn open_fray<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: Spine<'_>,
) -> Result<(), RiftError> {
    let monitors = app.available_monitors().map_err(window_error)?;
    let names: Vec<Option<String>> = monitors.iter().map(|m| m.name().cloned()).collect();
    let target = {
        let settings = read(&state.settings);
        own_monitor(
            &names,
            settings.fray_display_name.as_deref(),
            settings.fray_display,
        )
    }
    .and_then(|index| monitors.get(index).map(|m| (index, m.clone())));

    if let Some(window) = app.get_webview_window(FRAY_WINDOW) {
        if let Some((index, monitor)) = &target {
            remember(&state, *index, monitor.name().cloned());
            fill_monitor(&window, monitor)?;
        }
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
    let mut builder = tauri::WebviewWindowBuilder::from_config(&app, &config)
        .map_err(window_error)?
        .focused(false);
    if let Some((_, monitor)) = &target {
        // Born on its monitor, in that monitor's logical units: the fit below corrects any
        // rounding, but the window never shows on the game's screen first.
        let scale = monitor.scale_factor();
        builder = builder
            .decorations(false)
            .shadow(false)
            .position(
                f64::from(monitor.position().x) / scale,
                f64::from(monitor.position().y) / scale,
            )
            .inner_size(
                f64::from(monitor.size().width) / scale,
                f64::from(monitor.size().height) / scale,
            );
    }
    let window = builder.build().map_err(window_error)?;
    if let Some((index, monitor)) = &target {
        remember(&state, *index, monitor.name().cloned());
        fill_monitor(&window, monitor)?;
        follow_moves(&window, Arc::clone(state.inner()));
    }
    Ok(())
}

/// Which attached monitor THE FRAY takes, `None` for a plain window: with one monitor there is
/// no spare, and filling the only screen would cover the game. The War Tent's order (WR-0831):
/// the remembered NAME wherever it now sits, since monitor order is not stable across a replug;
/// then the remembered place while it is still in range; then the LAST monitor, on a desk with
/// a spare far more often the spare than the first. A stale memory is never an error.
pub fn own_monitor(
    monitors: &[Option<String>],
    remembered_name: Option<&str>,
    remembered_index: Option<usize>,
) -> Option<usize> {
    if monitors.len() < 2 {
        return None;
    }
    if let Some(name) = remembered_name {
        if let Some(at) = monitors.iter().position(|m| m.as_deref() == Some(name)) {
            return Some(at);
        }
    }
    if let Some(index) = remembered_index.filter(|index| *index < monitors.len()) {
        return Some(index);
    }
    Some(monitors.len() - 1)
}

/// Size and place the window to exactly cover one monitor, in physical units.
fn fill_monitor<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
    monitor: &tauri::Monitor,
) -> Result<(), RiftError> {
    window
        .set_position(tauri::PhysicalPosition::new(
            monitor.position().x,
            monitor.position().y,
        ))
        .map_err(window_error)?;
    window
        .set_size(tauri::PhysicalSize::new(
            monitor.size().width,
            monitor.size().height,
        ))
        .map_err(window_error)
}

/// Remember the monitor THE FRAY stands on: the name to find it again, the place for a monitor
/// the OS did not name.
fn remember(state: &AppState, index: usize, name: Option<String>) {
    {
        let mut settings = write(&state.settings);
        if settings.fray_display == Some(index) && settings.fray_display_name == name {
            return;
        }
        settings.fray_display = Some(index);
        settings.fray_display_name = name;
    }
    state.save_settings();
}

/// Moved onto another monitor, THE FRAY fills that one and remembers it. Its own fit moves it
/// too, onto the monitor it already remembers, so the fit cannot loop.
fn follow_moves<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>, state: Arc<AppState>) {
    let moved = window.clone();
    window.on_window_event(move |event| {
        if !matches!(event, tauri::WindowEvent::Moved(_)) {
            return;
        }
        let (Ok(Some(monitor)), Ok(monitors)) =
            (moved.current_monitor(), moved.available_monitors())
        else {
            return;
        };
        let name = monitor.name().cloned();
        let remembered = read(&state.settings).fray_display_name.clone();
        if name.is_none() || name == remembered {
            return;
        }
        let index = monitors
            .iter()
            .position(|m| m.name() == name.as_ref())
            .unwrap_or_default();
        remember(&state, index, name);
        if let Err(err) = fill_monitor(&moved, &monitor) {
            log::warn!("the party screen could not fill its new monitor: {err}");
        }
    });
}

fn window_error(err: tauri::Error) -> RiftError {
    log::warn!("the party screen would not open: {err}");
    RiftError::Io { detail: "The party screen would not open. Try again; if it keeps failing, restart the Riftstone.".into() }
}

#[cfg(test)]
mod tests {
    use super::own_monitor;

    fn named(names: &[&str]) -> Vec<Option<String>> {
        names.iter().map(|n| Some((*n).to_owned())).collect()
    }

    #[test]
    fn should_stay_a_plain_window_on_one_monitor() {
        assert_eq!(own_monitor(&named(&["one"]), Some("one"), Some(0)), None);
        assert_eq!(own_monitor(&[], None, None), None);
    }

    #[test]
    fn should_find_the_remembered_monitor_by_name_wherever_it_now_sits() {
        let desk = named(&["left", "spare", "right"]);
        assert_eq!(
            own_monitor(&desk, Some("spare"), Some(2)),
            Some(1),
            "the name beats the place"
        );
    }

    #[test]
    fn should_fall_back_to_the_place_then_the_last_monitor() {
        let desk = named(&["left", "right"]);
        assert_eq!(own_monitor(&desk, Some("unplugged"), Some(0)), Some(0));
        assert_eq!(
            own_monitor(&desk, Some("unplugged"), Some(5)),
            Some(1),
            "a stale place is no error"
        );
        assert_eq!(own_monitor(&desk, None, None), Some(1));
        let unnamed = vec![None, None, None];
        assert_eq!(own_monitor(&unnamed, None, Some(1)), Some(1));
    }
}
