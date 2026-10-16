//! The commands of docs/WINDOW-CONTRACT.md.
//!
//! Each command is two layers: a plain `*_impl(&AppState, …)` function that
//! does the work (the tests call these directly, with no Tauri runtime), and
//! a thin `#[tauri::command] async fn` wrapper with the exact invoke name and
//! argument names `src/services/rift.ts` uses (Tauri maps the window's
//! camelCase keys onto these snake_case parameters).
//!
//! Every wrapper hands its work to [`blocking`] — even the quick ones — so
//! nothing runs on the async runtime or the main thread (a search may read
//! the game folder once after a change; the Nexus client is
//! `reqwest::blocking`), and a panic in the work still answers the window
//! with a `RiftError` instead of leaving its invoke hanging.

use std::sync::Arc;

use rift_core::RiftError;

use crate::state::AppState;

pub mod deploy;
pub mod library;
pub mod nexus;
pub mod overview;

/// The managed state as the wrappers receive it.
pub type Spine<'a> = tauri::State<'a, Arc<AppState>>;

/// Run `work` on Tauri's blocking pool, with the spine.
pub async fn blocking<T, F>(state: &Spine<'_>, work: F) -> Result<T, RiftError>
where
    T: Send + 'static,
    F: FnOnce(&Arc<AppState>) -> Result<T, RiftError> + Send + 'static,
{
    let state = Arc::clone(state.inner());
    off_runtime(move || work(&state)).await
}

/// Run `work` on Tauri's blocking pool. A panic inside it becomes a
/// `RiftError::Io` with a fixed sentence (the panic's own text is logged,
/// never sent: it could carry anything).
pub async fn off_runtime<T, F>(work: F) -> Result<T, RiftError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, RiftError> + Send + 'static,
{
    match tauri::async_runtime::spawn_blocking(work).await {
        Ok(result) => result,
        Err(err) => {
            log::error!("a command stopped unexpectedly: {err}");
            Err(RiftError::Io {
                detail: "The Riftstone stopped that task before it finished. Try again; if it keeps happening, restart the Riftstone.".into(),
            })
        }
    }
}

/// An id the window sent that the library no longer holds.
pub fn not_found(id: &str) -> RiftError {
    RiftError::EntryNotFound {
        entry: id.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panicking_command_still_answers_with_a_rift_error() {
        let answer: Result<u32, RiftError> =
            tauri::async_runtime::block_on(off_runtime(|| panic!("secret-ish panic text")));
        let err = answer.unwrap_err();
        assert!(matches!(err, RiftError::Io { .. }), "{err:?}");
        let wire = serde_json::to_value(&err).unwrap();
        assert_eq!(wire["kind"], "io");
        assert!(
            !wire.to_string().contains("secret-ish"),
            "the panic's text stays in the log: {wire}"
        );
        let fine: Result<u32, RiftError> = tauri::async_runtime::block_on(off_runtime(|| Ok(7)));
        assert_eq!(fine.unwrap(), 7);
    }
}
