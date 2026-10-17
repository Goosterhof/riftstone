//! The nxm handler switch (v0.3): `get_nxm_handler`, `set_nxm_handler`. The
//! work itself is [`crate::nxm`]; these are its doors.

use rift_core::RiftError;

use crate::commands::{blocking, Spine};
use crate::dto::NxmHandlerView;
use crate::nxm;

#[tauri::command]
pub async fn get_nxm_handler(state: Spine<'_>) -> Result<NxmHandlerView, RiftError> {
    blocking(&state, |state| nxm::handler_view(state)).await
}

#[tauri::command]
pub async fn set_nxm_handler(state: Spine<'_>, on: bool) -> Result<NxmHandlerView, RiftError> {
    blocking(&state, move |state| nxm::set_handler(state, on)).await
}
