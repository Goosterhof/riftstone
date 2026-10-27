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
    let plain = (config.width, config.height);
    let monitors = app.available_monitors().map_err(window_error)?;
    let names: Vec<Option<String>> = monitors.iter().map(|m| m.name().cloned()).collect();
    let target = {
        let settings = read(&state.settings);
        own_monitor(
            &names,
            settings.fray_display_name.as_deref(),
            settings.fray_display,
        )
    };
    if let Some(index) = target {
        remember(&state, index, names[index].clone());
    }
    let target_monitor = target.and_then(|index| monitors.get(index));

    // Open already: the press settles its mode for the monitors there are NOW (PR #4 review: a
    // window opened plain on one monitor fills a second one plugged in since, and one left
    // borderless when a monitor went away turns plain rather than cover the game).
    if let Some(window) = app.get_webview_window(FRAY_WINDOW) {
        fit(&window, target_monitor, plain)?;
        return window.show().map_err(window_error);
    }
    let mut builder = tauri::WebviewWindowBuilder::from_config(&app, &config)
        .map_err(window_error)?
        .focused(false);
    if let Some(index) = target {
        builder = builder.decorations(false).shadow(false);
        // Born on its monitor, so it never shows on the game's screen first. tao places a
        // logical birth point on the first monitor that holds it in THAT monitor's scale, so on
        // a mixed-DPI desk the target's own origin can land on another one: the point is
        // chosen by tao's rule ([`birth_point`]). The fit below then sets the exact bounds.
        let screens: Vec<Screen> = monitors.iter().map(Screen::of).collect();
        if let Some((x, y)) = birth_point(&screens, index) {
            builder = builder.position(x, y);
        }
    }
    let window = builder.build().map_err(window_error)?;
    if target_monitor.is_some() {
        fit(&window, target_monitor, plain)?;
    }
    follow_moves(&window, Arc::clone(state.inner()), plain);
    Ok(())
}

/// One monitor as tao's placement reads it: its physical origin and size, and its scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Screen {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub scale: f64,
}

impl Screen {
    fn of(monitor: &tauri::Monitor) -> Screen {
        Screen {
            x: monitor.position().x,
            y: monitor.position().y,
            width: i32::try_from(monitor.size().width).unwrap_or(i32::MAX),
            height: i32::try_from(monitor.size().height).unwrap_or(i32::MAX),
            scale: monitor.scale_factor(),
        }
    }

    fn holds(&self, x: i32, y: i32) -> bool {
        self.x <= x && x <= self.x + self.width && self.y <= y && y <= self.y + self.height
    }
}

/// The monitor tao 0.37 creates a window on for a logical birth point, read from its Windows
/// code: the first monitor, in the OS's order, that holds the point converted with that
/// monitor's own scale (no frame allowance: the window has no shadow).
pub fn born_on(screens: &[Screen], x: f64, y: f64) -> Option<usize> {
    screens.iter().position(|s| {
        // As tao's `LogicalPosition::to_physical::<i32>`: scaled, then rounded.
        let px = (x * s.scale).round() as i32;
        let py = (y * s.scale).round() as i32;
        s.holds(px, py)
    })
}

/// A logical birth point tao places on `target`: the target's own logical grid, searched
/// from its middle out, so a mixed-DPI desk cannot send the window to another monitor first.
/// `None` when no point of that grid lands there.
pub fn birth_point(screens: &[Screen], target: usize) -> Option<(f64, f64)> {
    let s = screens.get(target)?;
    const STEPS: i32 = 16;
    let mut offsets: Vec<i32> = (0..=STEPS).collect();
    offsets.sort_by_key(|i| (i - STEPS / 2).abs());
    for &i in &offsets {
        for &j in &offsets {
            let x = f64::from(s.x + s.width * i / STEPS) / s.scale;
            let y = f64::from(s.y + s.height * j / STEPS) / s.scale;
            if born_on(screens, x, y) == Some(target) {
                return Some((x, y));
            }
        }
    }
    None
}

/// Settle the window's mode: borderless over exactly `monitor`, or the plain window of
/// `plain` logical size, centered, when there is no monitor of its own. Style changes, moves
/// and resizes all pass `SWP_NOACTIVATE` in tao 0.37 for a window that is not fullscreen.
fn fit<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
    monitor: Option<&tauri::Monitor>,
    plain: (f64, f64),
) -> Result<(), RiftError> {
    let decorated = window.is_decorated().map_err(window_error)?;
    match monitor {
        Some(monitor) => {
            if decorated {
                window.set_decorations(false).map_err(window_error)?;
                window.set_shadow(false).map_err(window_error)?;
            }
            fill_monitor(window, monitor)
        }
        None if !decorated => {
            window.set_decorations(true).map_err(window_error)?;
            window.set_shadow(true).map_err(window_error)?;
            window
                .set_size(tauri::LogicalSize::new(plain.0, plain.1))
                .map_err(window_error)?;
            window.center().map_err(window_error)
        }
        None => Ok(()),
    }
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

/// While it owns a monitor, THE FRAY follows a move: onto another monitor, it fills that one
/// and remembers it; with one monitor left (a monitor unplugged, Windows moving the window onto
/// the game's screen), it turns plain rather than cover the game. Its own fit moves it too,
/// onto the monitor it already remembers, so the fit cannot loop. A plain window is left where
/// the player puts it.
fn follow_moves<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
    state: Arc<AppState>,
    plain: (f64, f64),
) {
    let moved = window.clone();
    window.on_window_event(move |event| {
        if !matches!(event, tauri::WindowEvent::Moved(_)) {
            return;
        }
        if moved.is_decorated().unwrap_or(true) {
            return;
        }
        let (Ok(Some(monitor)), Ok(monitors)) =
            (moved.current_monitor(), moved.available_monitors())
        else {
            return;
        };
        let fitted = if monitors.len() < 2 {
            fit(&moved, None, plain)
        } else {
            let name = monitor.name().cloned();
            if name.is_none() || name == read(&state.settings).fray_display_name {
                return;
            }
            let index = monitors
                .iter()
                .position(|m| m.name() == name.as_ref())
                .unwrap_or_default();
            remember(&state, index, name);
            fit(&moved, Some(&monitor), plain)
        };
        if let Err(err) = fitted {
            log::warn!("the party screen could not settle on its monitor: {err}");
        }
    });
}

fn window_error(err: tauri::Error) -> RiftError {
    log::warn!("the party screen would not open: {err}");
    RiftError::Io { detail: "The party screen would not open. Try again; if it keeps failing, restart the Riftstone.".into() }
}

#[cfg(test)]
mod tests {
    use super::{birth_point, born_on, own_monitor, Screen};

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

    /// The Heretic's desk on PR #4: a 100% primary and a 150% portrait monitor to its right.
    /// The side monitor's origin divided by its own scale (1280) is held by the primary first.
    #[test]
    fn should_be_born_on_the_target_on_a_mixed_dpi_desk() {
        let desk = [
            Screen {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
                scale: 1.0,
            },
            Screen {
                x: 1920,
                y: 0,
                width: 1080,
                height: 1920,
                scale: 1.5,
            },
        ];
        assert_eq!(
            born_on(&desk, 1920.0 / 1.5, 0.0),
            Some(0),
            "the naive point lands on the primary"
        );
        let (x, y) = birth_point(&desk, 1).expect("a point on the side monitor");
        assert_eq!(born_on(&desk, x, y), Some(1));
        let (x, y) = birth_point(&desk, 0).expect("a point on the primary");
        assert_eq!(born_on(&desk, x, y), Some(0));
    }

    #[test]
    fn should_find_a_birth_point_left_of_the_primary_and_at_equal_scales() {
        let desk = [
            Screen {
                x: 0,
                y: 0,
                width: 2560,
                height: 1440,
                scale: 1.25,
            },
            Screen {
                x: -1080,
                y: -400,
                width: 1080,
                height: 1920,
                scale: 1.0,
            },
        ];
        let (x, y) = birth_point(&desk, 1).expect("a point on the left monitor");
        assert_eq!(born_on(&desk, x, y), Some(1));
        let same = [
            Screen {
                x: 0,
                y: 0,
                width: 1920,
                height: 1080,
                scale: 1.0,
            },
            Screen {
                x: 1920,
                y: 0,
                width: 1920,
                height: 1080,
                scale: 1.0,
            },
        ];
        let (x, y) = birth_point(&same, 1).expect("a point on the right monitor");
        assert_eq!(born_on(&same, x, y), Some(1));
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
