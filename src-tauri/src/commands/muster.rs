//! The Muster (v0.4): `get_muster_slots`, `get_muster`, `open_saves_folder`.
//!
//! Read-only: [`crate::muster`] reads the save slots into memory and names their parties from the
//! game's own text; nothing here writes, and nothing of a save leaves the machine.

use rift_core::RiftError;

use crate::commands::{blocking, Spine};
use crate::dto::{MusterSlotsView, MusterView};
use crate::state::AppState;

/// Every save slot that holds a party, newest first.
pub fn get_muster_slots_impl(state: &AppState) -> MusterSlotsView {
    crate::muster::slots_view(state)
}

/// The whole party of one slot; a slot that will not open is a `save` error.
pub fn get_muster_impl(state: &AppState, slot_id: &str) -> Result<MusterView, RiftError> {
    crate::muster::view(state, slot_id)
}

/// The slot's save folder in the file manager.
pub fn open_saves_folder_impl(state: &AppState, slot_id: &str) -> Result<(), RiftError> {
    crate::muster::open_folder(state, slot_id)
}

#[tauri::command]
pub async fn get_muster_slots(state: Spine<'_>) -> Result<MusterSlotsView, RiftError> {
    blocking(&state, |s| Ok(get_muster_slots_impl(s))).await
}

#[tauri::command]
pub async fn get_muster(state: Spine<'_>, slot_id: String) -> Result<MusterView, RiftError> {
    blocking(&state, move |s| get_muster_impl(s, &slot_id)).await
}

#[tauri::command]
pub async fn open_saves_folder(state: Spine<'_>, slot_id: String) -> Result<(), RiftError> {
    blocking(&state, move |s| open_saves_folder_impl(s, &slot_id)).await
}
