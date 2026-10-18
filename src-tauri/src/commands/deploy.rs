//! Your world: plan, install, uninstall, the load order, REFramework's
//! switches, the game folder, and the files kept aside (v0.2).
//!
//! Every write goes through rift-deploy's engine under the write-ahead
//! journal; the spine only picks the option, opens the package, and tells
//! the window what changed (`rift://install-progress`, `rift://game-changed`).
//! v0.5: an install of one version of a file replaces the other versions'
//! installs of the same option in the same step, then puts the older versions
//! it left without installs away (WINDOW-CONTRACT § v0.5).

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rift_core::{ExtractProgress, OptionId, RiftError};
use rift_deploy::Journal;

use crate::commands::overview::game_summary;
use crate::commands::versions;
use crate::commands::{blocking, not_found, Spine};
use crate::dto::{
    GameSummary, InstallPlanView, InstallProgressEvent, InstallResultView, InstalledRow,
    InstalledView, MissingFile, PakLoadReport, PakView, ParkedFileView, ParkedRestoreReport,
    ReplacesView, StrayFile, SwitchKey, UninstallPlanView, UninstallResultView,
};
use crate::events;
use crate::family::{self, Families};
use crate::state::{lock, read, AppState, JournalSlot};

/// Install progress reaches the window at most this often (and always at the end).
const PROGRESS_EVERY: Duration = Duration::from_millis(40);

/// A plan the engine cannot make (no game, a damaged journal) is still a
/// plan: refused, with the sentence that says why.
pub(crate) fn refusal(err: &RiftError) -> Option<String> {
    match err {
        RiftError::GameNotFound => Some(
            "Dragon's Dogma 2 has not been found yet. Point the Riftstone at the game folder first."
                .into(),
        ),
        RiftError::Journal { detail } => Some(format!(
            "The install journal could not be read, so nothing can be installed or uninstalled until it can: {detail}"
        )),
        _ => None,
    }
}

pub fn plan_install_impl(state: &AppState, option_id: &str) -> Result<InstallPlanView, RiftError> {
    let shelf = state.shelf();
    let (package, option) = shelf
        .option(&OptionId(option_id.to_string()))
        .ok_or_else(|| not_found(option_id))?;
    let families = Families::of_shelf(&shelf, &lock(&state.nexus));
    let planned = state.plan(|ctx, journal| {
        let replacing = family::replaced(&families, &shelf, package, option, |id| {
            journal.is_installed(id)
        });
        let plan = rift_deploy::plan_install_replacing(ctx, journal, package, option, &replacing);
        let replaces: Vec<ReplacesView> = replacing
            .iter()
            .map(|id| versions::replaces_view(&families, ctx, journal, id).0)
            .collect();
        (plan, replaces)
    });
    match planned {
        Ok((plan, replaces)) => Ok(InstallPlanView {
            replaces,
            ..InstallPlanView::from(&plan)
        }),
        Err(err) => match refusal(&err) {
            Some(sentence) => Ok(InstallPlanView {
                option_id: option.id.0.clone(),
                name: option.name.clone(),
                writes: Vec::new(),
                total_bytes: 0,
                warnings: Vec::new(),
                replaces: Vec::new(),
                refused: Some(sentence),
            }),
            None => Err(err),
        },
    }
}

pub fn install_impl(
    state: &Arc<AppState>,
    option_id: &str,
) -> Result<InstallResultView, RiftError> {
    // Play waits while an install is under way, from here to its last byte.
    let _installing = state.begin_install();
    let shelf = state.shelf();
    let (package, option) = shelf
        .option(&OptionId(option_id.to_string()))
        .ok_or_else(|| not_found(option_id))?;
    let families = Families::of_shelf(&shelf, &lock(&state.nexus));
    let events = &state.events;
    let mut last_sent: Option<Instant> = None;
    let mut progress = |p: ExtractProgress| {
        let finished = p.files_done >= p.files_total;
        if !finished && last_sent.is_some_and(|at| at.elapsed() < PROGRESS_EVERY) {
            return;
        }
        last_sent = Some(Instant::now());
        events.emit(
            events::INSTALL_PROGRESS,
            &InstallProgressEvent {
                option_id: option.id.0.clone(),
                files_done: p.files_done,
                files_total: p.files_total,
                bytes_done: p.bytes_done,
                bytes_total: p.bytes_total,
            },
        );
    };
    let (report, put) = state.deploy(|ctx, journal| {
        let replacing = family::replaced(&families, &shelf, package, option, |id| {
            journal.is_installed(id)
        });
        let mut source = rift_archive::open(&package.path)?;
        let report = rift_deploy::install_replacing(
            ctx,
            journal,
            package,
            option,
            &replacing,
            source.as_mut(),
            &mut progress,
        )?;
        drop(source);
        // The step committed: the older versions it left without installs go,
        // still under the deploy lock and before the scan below.
        let journal: &Journal = journal;
        let put = versions::put_away_older(state, &shelf, families.older_than(&package.id), |id| {
            versions::has_install(journal, id)
        });
        Ok((report, put))
    })?;
    state.events.changed(events::GAME_CHANGED);
    if !put.moved.is_empty() {
        state.request_scan();
    }
    Ok(InstallResultView {
        put_away: put.moved,
        put_away_failed: put.failed,
        ..InstallResultView::from(&report)
    })
}

pub fn plan_uninstall_impl(
    state: &AppState,
    option_id: &str,
) -> Result<UninstallPlanView, RiftError> {
    let id = OptionId(option_id.to_string());
    match state.plan(|ctx, journal| rift_deploy::plan_uninstall(ctx, journal, &id)) {
        Ok(plan) => Ok((&plan).into()),
        Err(err) => match refusal(&err) {
            Some(sentence) => Ok(UninstallPlanView {
                option_id: option_id.to_string(),
                name: state
                    .shelf()
                    .option(&id)
                    .map_or_else(|| option_id.to_string(), |(_, o)| o.name.clone()),
                removals: Vec::new(),
                refused: Some(sentence),
            }),
            None => Err(err),
        },
    }
}

pub fn uninstall_impl(state: &AppState, option_id: &str) -> Result<UninstallResultView, RiftError> {
    let id = OptionId(option_id.to_string());
    let report = state.deploy(|ctx, journal| rift_deploy::uninstall(ctx, journal, &id))?;
    state.events.changed(events::GAME_CHANGED);
    Ok((&report).into())
}

/// What is in the game now: the installs (adopted ones too), the pak load
/// order, who overrides whom, strays, missing files, the files kept aside,
/// and what REFramework's log says it loaded. An install whose package is not
/// in the mods folder is still listed (`packageMissing`): the journal knows
/// every file, so it uninstalls as before.
pub fn get_installed_impl(state: &AppState) -> Result<InstalledView, RiftError> {
    let snapshot = state.refresh_snapshot();
    match &read(&state.world).journal {
        JournalSlot::Ready(_) => {}
        JournalSlot::Damaged(err) => return Err(err.clone()),
        JournalSlot::NoGame => return Err(RiftError::GameNotFound),
    }
    let game_state = snapshot
        .game_state
        .as_ref()
        .ok_or(RiftError::GameNotFound)?;
    let shelf = state.shelf();
    let installs = game_state
        .installs
        .iter()
        .map(|install| {
            let package = shelf.package(&install.option_id.package());
            InstalledRow {
                option_id: install.option_id.0.clone(),
                name: install.name.clone(),
                package_name: package.map_or_else(
                    || stem(&install.package_file).unwrap_or_else(|| install.name.clone()),
                    |p| p.display_name.clone(),
                ),
                package_missing: package.is_none(),
                version: install.version.clone(),
                installed_at: install.installed_at.clone(),
                game_build: install.game_build.clone(),
                origin: (&install.origin).into(),
                kinds: install.kinds.into(),
                file_count: install.file_count,
                paks: install.paks.iter().map(PakView::from).collect(),
            }
        })
        .collect();
    Ok(InstalledView {
        installs,
        pak_order: game_state.pak_order.iter().map(|id| id.0.clone()).collect(),
        conflicts: snapshot.conflicts.clone(),
        strays: game_state.strays.iter().map(StrayFile::from).collect(),
        missing: game_state.missing.iter().map(MissingFile::from).collect(),
        pak_load: game_state.pak_load.as_ref().map(PakLoadReport::from),
        pak_count_warning: game_state.pak_count_warning,
        parked: game_state.parked.iter().map(ParkedFileView::from).collect(),
    })
}

/// Put a file kept aside back in the game folder (`store` is its handle,
/// as `InstalledView.parked` lists it). Whatever sits at its path now is
/// kept aside in its turn (`swappedOut`), so the swap can be undone the same
/// way. Refused while an install owns the file there, and for a handle the
/// list no longer holds. A running game does not
/// stop it: the file loads at the next launch (`restartNeeded`), and one the
/// game holds open fails to move and is rolled back. One journaled change
/// under the deploy lock, rolled back on failure; `rift://game-changed` after.
pub fn restore_parked_impl(
    state: &AppState,
    store: &str,
) -> Result<ParkedRestoreReport, RiftError> {
    let report = state.deploy(|ctx, journal| rift_deploy::restore_parked(ctx, journal, store))?;
    state.events.changed(events::GAME_CHANGED);
    Ok((&report).into())
}

/// Delete a file kept aside from the backup store for good. The game folder
/// is not touched; `rift://game-changed` after (the record changed).
pub fn discard_parked_impl(state: &AppState, store: &str) -> Result<(), RiftError> {
    state.deploy(|ctx, journal| rift_deploy::discard_parked(ctx, journal, store))?;
    state.events.changed(events::GAME_CHANGED);
    Ok(())
}

/// The file name without its extension (`Lantern Glow.zip` → `Lantern Glow`).
fn stem(file_name: &str) -> Option<String> {
    let name = file_name.trim();
    if name.is_empty() {
        return None;
    }
    Some(
        Path::new(name)
            .file_stem()
            .map_or_else(|| name.to_string(), |s| s.to_string_lossy().into_owned()),
    )
}

pub fn reorder_paks_impl(state: &AppState, order: &[String]) -> Result<(), RiftError> {
    let ids: Vec<OptionId> = order.iter().map(|id| OptionId(id.clone())).collect();
    state.deploy(|ctx, journal| rift_deploy::reorder_paks(ctx, journal, &ids))?;
    state.events.changed(events::GAME_CHANGED);
    Ok(())
}

pub fn set_reframework_switch_impl(
    state: &AppState,
    switch: SwitchKey,
    on: bool,
) -> Result<(), RiftError> {
    let game = read(&state.world)
        .game
        .clone()
        .ok_or(RiftError::GameNotFound)?;
    {
        // One writer in the game folder at a time.
        let _turn = state.turn();
        rift_deploy::set_switch(&game.root, switch.into(), on, state.observe_game())?;
    }
    state.invalidate_snapshot();
    state.events.changed(events::GAME_CHANGED);
    Ok(())
}

/// Point the Riftstone at a game folder by hand. It must hold `DD2.exe`; its
/// journal is loaded (and an interrupted change in it rolled back).
pub fn set_game_root_impl(state: &AppState, path: &str) -> Result<GameSummary, RiftError> {
    let game = rift_deploy::validate_game_root(Path::new(path.trim()))?;
    state.switch_game(game.clone());
    state.events.changed(events::GAME_CHANGED);
    let snapshot = state.refresh_snapshot();
    Ok(game_summary(state, &game, &snapshot))
}

// -- wrappers ----------------------------------------------------------------

#[tauri::command]
pub async fn plan_install(
    state: Spine<'_>,
    option_id: String,
) -> Result<InstallPlanView, RiftError> {
    blocking(&state, move |state| plan_install_impl(state, &option_id)).await
}

#[tauri::command]
pub async fn install(state: Spine<'_>, option_id: String) -> Result<InstallResultView, RiftError> {
    blocking(&state, move |state| install_impl(state, &option_id)).await
}

#[tauri::command]
pub async fn plan_uninstall(
    state: Spine<'_>,
    option_id: String,
) -> Result<UninstallPlanView, RiftError> {
    blocking(&state, move |state| plan_uninstall_impl(state, &option_id)).await
}

#[tauri::command]
pub async fn uninstall(
    state: Spine<'_>,
    option_id: String,
) -> Result<UninstallResultView, RiftError> {
    blocking(&state, move |state| uninstall_impl(state, &option_id)).await
}

#[tauri::command]
pub async fn get_installed(state: Spine<'_>) -> Result<InstalledView, RiftError> {
    blocking(&state, |state| get_installed_impl(state)).await
}

#[tauri::command]
pub async fn reorder_paks(state: Spine<'_>, order: Vec<String>) -> Result<(), RiftError> {
    blocking(&state, move |state| reorder_paks_impl(state, &order)).await
}

#[tauri::command]
pub async fn set_reframework_switch(
    state: Spine<'_>,
    switch: SwitchKey,
    on: bool,
) -> Result<(), RiftError> {
    blocking(&state, move |state| {
        set_reframework_switch_impl(state, switch, on)
    })
    .await
}

#[tauri::command]
pub async fn set_game_root(state: Spine<'_>, path: String) -> Result<GameSummary, RiftError> {
    blocking(&state, move |state| set_game_root_impl(state, &path)).await
}

#[tauri::command]
pub async fn restore_parked(
    state: Spine<'_>,
    store: String,
) -> Result<ParkedRestoreReport, RiftError> {
    blocking(&state, move |state| restore_parked_impl(state, &store)).await
}

#[tauri::command]
pub async fn discard_parked(state: Spine<'_>, store: String) -> Result<(), RiftError> {
    blocking(&state, move |state| discard_parked_impl(state, &store)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_that_cannot_be_made_are_refused_with_a_sentence() {
        assert!(refusal(&RiftError::GameNotFound)
            .unwrap()
            .contains("game folder"));
        assert!(refusal(&RiftError::Journal {
            detail: "journal-x.json is damaged".into()
        })
        .unwrap()
        .contains("journal-x.json is damaged"));
        assert!(refusal(&RiftError::refused("no")).is_none());
    }

    #[test]
    fn package_names_fall_back_to_the_file_stem() {
        assert_eq!(
            stem("Lantern Glow Recolor.zip").as_deref(),
            Some("Lantern Glow Recolor")
        );
        assert_eq!(stem("  "), None);
    }
}
