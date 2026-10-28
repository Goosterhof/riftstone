//! THE WORKSHOP's commands (WINDOW-CONTRACT § v0.7, Commands), registered
//! only in a build with the `pawn` feature (RS-10). Each runs its work off
//! the async runtime, with the spine and the desk.

use std::path::Path;
use std::sync::Arc;

use rift_core::{PawnReason, RiftError};
use serde::Serialize;

use crate::commands::{deploy, off_runtime, Spine};
use crate::dto::{UninstallPlanView, UninstallResultView};
use crate::pawn::bring::{self, BringOutcome, BringPlan, Conflict};
use crate::pawn::desk::{workshop_dir, Desk, PawnStateView, WORKSHOP_CHANGED};
use crate::pawn::mailbox::RunLedger;
use crate::pawn::persona::Voice;
use crate::pawn::put_back::{self, PutBack};
use crate::state::AppState;

pub type DeskState<'a> = tauri::State<'a, Arc<Desk>>;

/// Run `work` off the runtime, with the spine and the desk.
async fn with_desk<T, F>(spine: &Spine<'_>, desk: &DeskState<'_>, work: F) -> Result<T, RiftError>
where
    T: Send + 'static,
    F: FnOnce(&Arc<AppState>, &Arc<Desk>) -> Result<T, RiftError> + Send + 'static,
{
    let spine = Arc::clone(spine.inner());
    let desk = Arc::clone(desk.inner());
    off_runtime(move || work(&spine, &desk)).await
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderView {
    pub made: bool,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactView {
    pub name: String,
    pub bytes: u64,
    pub sha256: Option<String>,
    pub receipt: Option<bring::Receipt>,
    /// `missing`, `unreadable` or `mismatch`.
    pub receipt_problem: Option<&'static str>,
    pub in_library: bool,
}

/// `FenceView`: the chip is `unprobed` until the probe stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FenceView {
    pub chip: &'static str,
    pub ground: Option<serde_json::Value>,
    pub proved_at: Option<String>,
    pub steps: Vec<serde_json::Value>,
    pub changed: Option<String>,
}

impl FenceView {
    pub fn unprobed() -> Self {
        Self {
            chip: "unprobed",
            ground: None,
            proved_at: None,
            steps: Vec::new(),
            changed: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkshopView {
    pub folder: FolderView,
    pub artifacts: Vec<ArtifactView>,
    pub pawn: PawnStateView,
    pub fence: FenceView,
    pub voice: Voice,
    pub inputs: Vec<String>,
}

/// The built zips in the workshop's `dist\`, each with the stone's own
/// digest and what its receipt says.
fn artifacts(workshop: &Path, mods: &Path) -> Vec<ArtifactView> {
    // Every name, size and digest through the pinned dist handle; the card's
    // plan reads the receipt the same way.
    bring::pinned_builds(workshop)
        .into_iter()
        .map(
            |(name, bytes, sha)| match bring::plan_bring_through(workshop, mods, &name) {
                Ok(plan) => ArtifactView {
                    name,
                    bytes: plan.bytes,
                    sha256: Some(plan.sha256),
                    receipt: Some(plan.receipt),
                    receipt_problem: None,
                    in_library: plan.conflict == Conflict::AlreadyHad,
                },
                Err(refusal) => ArtifactView {
                    name,
                    bytes,
                    sha256: Some(sha),
                    receipt: None,
                    receipt_problem: Some(match refusal.reason {
                        rift_core::BringReason::ReceiptMismatch => "mismatch",
                        rift_core::BringReason::NoReceipt => "missing",
                        _ => "unreadable",
                    }),
                    in_library: false,
                },
            },
        )
        .collect()
}

fn workshop_of(state: &AppState) -> Result<std::path::PathBuf, RiftError> {
    workshop_dir(state.library_dir()).ok_or_else(|| {
        RiftError::pawn(
            PawnReason::NoFence,
            "The mods folder has no folder around it for a workshop.",
        )
    })
}

pub fn get_workshop_impl(state: &AppState, desk: &Desk) -> Result<WorkshopView, RiftError> {
    let workshop = workshop_of(state)?;
    let settings = desk.settings();
    Ok(WorkshopView {
        folder: FolderView {
            made: workshop.is_dir(),
            path: workshop.to_string_lossy().into_owned(),
        },
        artifacts: artifacts(&workshop, state.library_dir()),
        pawn: desk.pawn_state(),
        fence: FenceView::unprobed(),
        voice: settings.voice,
        inputs: settings.inputs,
    })
}

fn ledger_of(desk: &Desk, run_id: &str) -> Result<RunLedger, RiftError> {
    desk.runs()
        .runs()
        .into_iter()
        .find(|run| run.run_id == run_id)
        .ok_or_else(|| {
            RiftError::pawn(
                PawnReason::NotThisRun,
                "That summoning is not in Tamsin's ledger.",
            )
        })
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PutBackPlanView {
    pub run_id: String,
    pub remove: Vec<UninstallPlanView>,
    pub review: Vec<put_back::ReviewItem>,
    pub sentence: String,
}

pub fn plan_put_back_impl(
    state: &AppState,
    desk: &Desk,
    run_id: &str,
) -> Result<PutBackPlanView, RiftError> {
    let ledger = ledger_of(desk, run_id)?;
    let shelf = state.shelf();
    let world = state.world();
    let journal = world
        .journal
        .ready()
        .ok_or_else(|| RiftError::refused("The install journal is not ready."))?;
    let plan: PutBack = put_back::plan_put_back(run_id, &ledger, journal, &|option_id| {
        shelf
            .option(&rift_core::OptionId(option_id.to_string()))
            .is_some()
    });
    let remove = plan
        .remove
        .iter()
        .map(|item| deploy::plan_uninstall_impl(state, &item.install_id))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PutBackPlanView {
        run_id: plan.run_id,
        remove,
        review: plan.review,
        sentence: plan.sentence,
    })
}

pub fn put_back_impl(
    state: &AppState,
    run_id: &str,
    install_ids: &[String],
) -> Result<Vec<UninstallResultView>, RiftError> {
    let world = state.world();
    let journal = world
        .journal
        .ready()
        .ok_or_else(|| RiftError::refused("The install journal is not ready."))?;
    let ordered = put_back::checked_put_back(run_id, journal, install_ids).map_err(|id| {
        RiftError::pawn(
            PawnReason::NotThisRun,
            format!("{id} is not an install Tamsin made in that summoning, so Put back leaves it."),
        )
    })?;
    ordered
        .iter()
        .map(|(id, seq)| {
            let check = |journal: &rift_deploy::Journal| {
                if put_back::still_hers(run_id, journal, id, *seq) {
                    Ok(())
                } else {
                    Err(RiftError::pawn(
                        PawnReason::NotThisRun,
                        format!("{id} changed since Put back was planned, so it is left as it is."),
                    ))
                }
            };
            deploy::uninstall_checked(state, id, Some(&check)).map(|(view, _)| view)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_workshop(
    spine: Spine<'_>,
    desk: DeskState<'_>,
) -> Result<WorkshopView, RiftError> {
    with_desk(&spine, &desk, |state, desk| get_workshop_impl(state, desk)).await
}

#[tauri::command]
pub async fn summon_pawn(
    spine: Spine<'_>,
    desk: DeskState<'_>,
    cols: u16,
    rows: u16,
) -> Result<PawnStateView, RiftError> {
    with_desk(&spine, &desk, move |state, desk| {
        desk.summon(state, Arc::clone(&state.events), cols, rows)
    })
    .await
}

#[tauri::command]
pub async fn pawn_write(
    spine: Spine<'_>,
    desk: DeskState<'_>,
    data: String,
) -> Result<(), RiftError> {
    with_desk(&spine, &desk, move |_, desk| desk.write(&data)).await
}

#[tauri::command]
pub async fn pawn_resize(
    spine: Spine<'_>,
    desk: DeskState<'_>,
    cols: u16,
    rows: u16,
) -> Result<(), RiftError> {
    with_desk(&spine, &desk, move |_, desk| {
        desk.resize(cols, rows);
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn depart_pawn(
    spine: Spine<'_>,
    desk: DeskState<'_>,
) -> Result<PawnStateView, RiftError> {
    with_desk(&spine, &desk, |state, desk| {
        desk.depart(state.events.as_ref())
    })
    .await
}

#[tauri::command]
pub async fn answer_pawn_play(
    spine: Spine<'_>,
    desk: DeskState<'_>,
    run_id: String,
    held_request_id: String,
    accept: bool,
) -> Result<crate::pawn::desk::PlayAnsweredView, RiftError> {
    with_desk(&spine, &desk, move |state, desk| {
        desk.answer_play(state, &run_id, &held_request_id, accept)
    })
    .await
}

#[tauri::command]
pub async fn get_fence() -> Result<FenceView, RiftError> {
    Ok(FenceView::unprobed())
}

#[tauri::command]
pub async fn plan_bring_through(
    spine: Spine<'_>,
    desk: DeskState<'_>,
    artifact: String,
) -> Result<BringPlan, RiftError> {
    with_desk(&spine, &desk, move |state, _| {
        let workshop = workshop_of(state)?;
        Ok(bring::plan_bring_through(
            &workshop,
            state.library_dir(),
            &artifact,
        )?)
    })
    .await
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BringThroughResult {
    pub outcome: BringOutcome,
}

#[tauri::command]
pub async fn bring_through(
    spine: Spine<'_>,
    desk: DeskState<'_>,
    artifact: String,
    sha256: String,
) -> Result<BringThroughResult, RiftError> {
    with_desk(&spine, &desk, move |state, _| {
        let workshop = workshop_of(state)?;
        let part = crate::pawn::desk::new_run_id();
        let outcome =
            bring::bring_through(&workshop, state.library_dir(), &artifact, &sha256, &part)?;
        state.request_scan();
        state.events.changed(WORKSHOP_CHANGED);
        Ok(BringThroughResult { outcome })
    })
    .await
}

#[tauri::command]
pub async fn plan_put_back(
    spine: Spine<'_>,
    desk: DeskState<'_>,
    run_id: String,
) -> Result<PutBackPlanView, RiftError> {
    with_desk(&spine, &desk, move |state, desk| {
        plan_put_back_impl(state, desk, &run_id)
    })
    .await
}

#[tauri::command]
pub async fn put_back(
    spine: Spine<'_>,
    desk: DeskState<'_>,
    run_id: String,
    install_ids: Vec<String>,
) -> Result<Vec<UninstallResultView>, RiftError> {
    with_desk(&spine, &desk, move |state, _| {
        put_back_impl(state, &run_id, &install_ids)
    })
    .await
}

#[tauri::command]
pub async fn open_workshop_folder(spine: Spine<'_>, desk: DeskState<'_>) -> Result<(), RiftError> {
    with_desk(&spine, &desk, |state, _| {
        let workshop = workshop_of(state)?;
        std::fs::create_dir_all(workshop.join("dist"))
            .map_err(|err| RiftError::io("could not make the workshop", err))?;
        state.config.launcher.show_folder(&workshop)
    })
    .await
}

#[tauri::command]
pub async fn set_pawn_voice(
    spine: Spine<'_>,
    desk: DeskState<'_>,
    voice: Voice,
) -> Result<(), RiftError> {
    with_desk(&spine, &desk, move |_, desk| desk.set_voice(voice)).await
}

#[tauri::command]
pub async fn set_pawn_inputs(
    spine: Spine<'_>,
    desk: DeskState<'_>,
    inputs: Vec<String>,
) -> Result<(), RiftError> {
    with_desk(&spine, &desk, move |state, desk| {
        desk.set_inputs(state, inputs)
    })
    .await
}

/// Every THE WORKSHOP command's invoke name, for RS-10's registration test.
pub const COMMANDS: [&str; 15] = [
    "get_workshop",
    "summon_pawn",
    "pawn_write",
    "pawn_resize",
    "depart_pawn",
    "answer_pawn_play",
    "get_fence",
    "plan_bring_through",
    "bring_through",
    "plan_put_back",
    "put_back",
    "open_workshop_folder",
    "set_pawn_voice",
    "set_pawn_inputs",
    "run_fence_probe",
];

#[tauri::command]
pub async fn run_fence_probe() -> Result<FenceView, RiftError> {
    Err(RiftError::pawn(
        PawnReason::NoFence,
        "The fence's probe is not built yet in this version of the stone.",
    ))
}
