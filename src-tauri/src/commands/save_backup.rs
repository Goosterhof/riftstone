//! On-demand save backup: never changes a source save or writes decrypted data.
use std::path::Path;

use rift_core::RiftError;

use crate::commands::{blocking, Spine};
use crate::save_backup::SaveBackupReport;
use crate::state::AppState;

pub fn backup_saves_impl(
    state: &AppState,
    destination: &str,
) -> Result<SaveBackupReport, RiftError> {
    // Play uses the same turn: the stone cannot launch DD2 during its backup.
    let _turn = state.turn();
    let folders = crate::muster::backup_folders(state);
    let mut protected = vec![
        state.config.dirs.data.clone(),
        state.config.dirs.cache.clone(),
        state.library_dir().to_path_buf(),
    ];
    if let Some(older) = crate::family::older_folder(state.library_dir()) {
        protected.push(older);
    }
    if let Some(game) = state.world().game {
        protected.push(game.root);
    }
    if let Some(install) = state.config.exe.parent() {
        protected.push(install.to_path_buf());
    }
    crate::save_backup::create(&folders, Path::new(destination), &protected, || {
        state.observe_game()
    })
}

#[tauri::command]
pub async fn backup_saves(
    state: Spine<'_>,
    destination: String,
) -> Result<SaveBackupReport, RiftError> {
    blocking(&state, move |s| backup_saves_impl(s, &destination)).await
}
