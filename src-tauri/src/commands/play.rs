//! Play (v0.3): `plan_launch` and `launch_game`.
//!
//! rift-deploy decides the route, the warnings and the refusal
//! ([`rift_deploy::plan_launch`]); the spine supplies what it needs to know
//! now — the game watcher's word on whether the game runs (asked afresh), and
//! whether a change holds the game folder — and starts the game through its
//! [`crate::play::GameLauncher`]. The window hears `rift://game-changed`
//! when the game watcher sees `DD2.exe` start, not when Play is pressed.

use rift_core::RiftError;
use rift_deploy::{GameInstall, Journal, LaunchPlan};

use crate::commands::{blocking, Spine};
use crate::dto::{LaunchPlanView, LaunchReport, LaunchRoute};
use crate::persist;
use crate::state::AppState;

/// Play's plan for the game in use, or `None` when no game was found.
/// `busy`: a change holds the game folder right now.
fn plan(state: &AppState, busy: bool) -> Option<(GameInstall, LaunchPlan)> {
    let running = state.observe_game();
    let snapshot = state.refresh_snapshot();
    let world = state.world();
    let game = world.game.clone()?;
    let ref_status = snapshot
        .ref_status
        .clone()
        .unwrap_or_else(|| rift_deploy::read_status(&game.root));
    let plan = match (world.journal.ready(), snapshot.game_state.as_ref()) {
        (Some(journal), Some(game_state)) => {
            rift_deploy::plan_launch(&game, &ref_status, journal, game_state, running, busy)
        }
        // A journal that cannot be read does not keep the game from
        // starting: Play plans as if nothing were installed.
        _ => {
            let empty = Journal::new(&game.root);
            let ctx = rift_deploy::DeployContext {
                game: &game,
                state_dir: &state.config.dirs.data,
                ref_status: &ref_status,
                game_running: running,
                actor: &rift_deploy::PLAYER,
            };
            let game_state = rift_deploy::read_game_state(&ctx, &empty);
            rift_deploy::plan_launch(&game, &ref_status, &empty, &game_state, running, busy)
        }
    };
    Some((game, plan))
}

/// The plan when there is no game to plan with.
fn no_game() -> LaunchPlanView {
    LaunchPlanView {
        launch: LaunchRoute::Exe,
        warnings: Vec::new(),
        refused: Some(rift_deploy::launch::REFUSED_NO_GAME.to_string()),
    }
}

/// What Play would do now: its route, its warnings (never blockers), or the
/// sentence that refuses it. Pure: nothing is started or written.
pub fn plan_launch_impl(state: &AppState) -> LaunchPlanView {
    match plan(state, state.deploy_busy()) {
        Some((_, plan)) => LaunchPlanView::from(&plan),
        None => no_game(),
    }
}

/// Start the game: refused exactly when [`plan_launch_impl`] refuses. Play
/// takes the game folder's turn while it asks Steam or Windows, so no change
/// starts in the same moment; then the game watcher samples faster for a
/// while, and its `rift://game-changed` tells the window the game is up.
pub fn launch_game_impl(state: &AppState) -> Result<LaunchReport, RiftError> {
    let Some(turn) = state.try_turn() else {
        let refused = plan(state, true)
            .and_then(|(_, plan)| plan.refused)
            .unwrap_or_else(|| rift_deploy::launch::REFUSED_NO_GAME.to_string());
        return Err(RiftError::refused(refused));
    };
    let Some((_, plan)) = plan(state, state.installs_under_way()) else {
        return Err(RiftError::refused(rift_deploy::launch::REFUSED_NO_GAME));
    };
    if let Some(refused) = plan.refused {
        return Err(RiftError::refused(refused));
    }
    let launched_ms = persist::now_ms();
    state.config.launcher.launch(&plan.route)?;
    drop(turn);
    state.game_launched();
    log::info!("Play: asked {} to start the game", plan.route.kind());
    Ok(LaunchReport {
        launch: LaunchRoute::from(&plan.route),
        launched_ms,
    })
}

// -- wrappers ----------------------------------------------------------------

#[tauri::command]
pub async fn plan_launch(state: Spine<'_>) -> Result<LaunchPlanView, RiftError> {
    blocking(&state, |state| Ok(plan_launch_impl(state))).await
}

#[tauri::command]
pub async fn launch_game(state: Spine<'_>) -> Result<LaunchReport, RiftError> {
    blocking(&state, |state| launch_game_impl(state)).await
}
