//! Downloads into the mods folder (v0.3): `get_downloads`, `start_download`,
//! `cancel_download`, `clear_downloads`. The work itself is
//! [`crate::downloads`]; these are its doors.

use rift_core::RiftError;

use crate::commands::{blocking, Spine};
use crate::downloads;
use crate::dto::DownloadView;
use crate::state::AppState;

/// This session's downloads, newest first.
pub fn get_downloads_impl(state: &AppState) -> Vec<DownloadView> {
    state.downloads.views()
}

#[tauri::command]
pub async fn get_downloads(state: Spine<'_>) -> Result<Vec<DownloadView>, RiftError> {
    blocking(&state, |state| Ok(get_downloads_impl(state))).await
}

#[tauri::command]
pub async fn start_download(
    state: Spine<'_>,
    mod_id: u64,
    file_id: u64,
    for_package_id: Option<String>,
) -> Result<DownloadView, RiftError> {
    blocking(&state, move |state| {
        downloads::start(state, mod_id, file_id, for_package_id)
    })
    .await
}

#[tauri::command]
pub async fn cancel_download(state: Spine<'_>, id: String) -> Result<(), RiftError> {
    blocking(&state, move |state| downloads::cancel(state, &id)).await
}

#[tauri::command]
pub async fn clear_downloads(state: Spine<'_>) -> Result<(), RiftError> {
    blocking(&state, |state| {
        downloads::clear(state);
        Ok(())
    })
    .await
}
