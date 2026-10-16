//! Nexus Mods: the update rows, the check, MD5 identification, links, the
//! optional API key and its quota.
//!
//! Every network call here runs on a blocking thread (the commands' blocking
//! pool or the wake-up thread): rift-nexus's client is `reqwest::blocking`,
//! which must never be created, used or dropped on the async runtime.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};

use rift_core::{LinkSource, NexusLink, PackageFormat, PackageId, RiftError};
use rift_nexus::{ModFilesReport, NexusMod};

use crate::commands::{blocking, not_found, Spine};
use crate::dto::{
    self, Md5Result, NexusQuota, NexusSummary, UpdateRow, UpdateStateView, UpdatesPhase,
    UpdatesProgressEvent, UpdatesView,
};
use crate::events;
use crate::library::Shelf;
use crate::nexus::{auto_check_due, judge, nexus_error, quota_view, NexusStore};
use crate::persist;
use crate::state::{lock, read, write, AppState};

/// One row per package that has options or a link.
fn rows(store: &NexusStore, shelf: &Shelf) -> Vec<UpdateRow> {
    shelf
        .packages()
        .iter()
        .filter(|p| !p.options.is_empty() || store.link_for(p).is_some())
        .map(|p| store.row(p))
        .collect()
}

pub fn summary(state: &AppState, shelf: &Shelf) -> NexusSummary {
    let store = lock(&state.nexus);
    let rows = rows(&store, shelf);
    NexusSummary {
        last_check_ms: store.last_check_ms,
        checking: state.nexus_checking.load(Ordering::SeqCst),
        linked: rows.iter().filter(|r| r.link.is_some()).count(),
        unlinked: rows.iter().filter(|r| r.link.is_none()).count(),
        updates: rows
            .iter()
            .filter(|r| matches!(r.state, UpdateStateView::UpdateAvailable { .. }))
            .count(),
        has_api_key: state.has_api_key.load(Ordering::SeqCst),
    }
}

pub fn get_updates_impl(state: &AppState) -> UpdatesView {
    let shelf = state.shelf();
    let store = lock(&state.nexus);
    UpdatesView {
        last_check_ms: store.last_check_ms,
        rows: rows(&store, &shelf),
    }
}

/// Clears the "checking" flag however a check ends.
struct Checking<'a>(&'a AtomicBool);

impl Drop for Checking<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Ask Nexus (anonymously) whether a newer file exists for every linked
/// package: one batched mod lookup, the mods' file lists in batches of 20,
/// each package pinned to its exact file and judged within its file group.
/// Every call asks Nexus — a link made a moment ago, or a row that could not
/// be checked last time, is judged now; calls take turns, one check at a time.
pub fn check_updates_impl(state: &AppState) -> Result<UpdatesView, RiftError> {
    let _one_at_a_time = lock(&state.nexus_check);
    state.nexus_checking.store(true, Ordering::SeqCst);
    let checking = Checking(&state.nexus_checking);
    let result = run_check(state);
    drop(checking);
    state.events.changed(events::UPDATES_CHANGED);
    result.map(|()| get_updates_impl(state))
}

fn run_check(state: &AppState) -> Result<(), RiftError> {
    let shelf = state.shelf();
    let targets: Vec<(&rift_core::Package, NexusLink)> = {
        let store = lock(&state.nexus);
        shelf
            .packages()
            .iter()
            .filter_map(|p| store.link_for(p).map(|link| (p, link)))
            .collect()
    };
    let mut mod_ids: Vec<u64> = targets.iter().map(|(_, link)| link.mod_id).collect();
    mod_ids.sort_unstable();
    mod_ids.dedup();
    let total = mod_ids.len();
    let progress = |done: usize| {
        state.events.emit(
            events::UPDATES_PROGRESS,
            &UpdatesProgressEvent {
                phase: UpdatesPhase::Checking,
                done,
                total,
            },
        );
    };
    progress(0);

    let client = read(&state.nexus_client).clone();
    let mods: HashMap<u64, NexusMod> = if mod_ids.is_empty() {
        HashMap::new()
    } else {
        client
            .mods(&mod_ids)
            .map_err(nexus_error)?
            .into_iter()
            .map(|m| (m.mod_id, m))
            .collect()
    };
    let mut report = ModFilesReport::default();
    let mut done = 0;
    for chunk in mod_ids.chunks(rift_nexus::MOD_FILES_PER_REQUEST) {
        let part = client.mod_files_report(chunk).map_err(nexus_error)?;
        report.files.extend(part.files);
        report.refused.extend(part.refused);
        done += chunk.len();
        progress(done);
    }

    let now = persist::now_ms();
    {
        let mut store = lock(&state.nexus);
        for (package, link) in &targets {
            let outcome = judge(link, &mods, &report);
            store.record_check(package, outcome, now);
        }
        store.last_check_ms = Some(now);
    }
    state.save_nexus();
    Ok(())
}

/// The launch check (startup step 6): only when the player wants it, the
/// last check is older than 12 hours, and something is linked at all.
pub fn auto_check(state: &AppState) {
    if !read(&state.settings).auto_check_updates {
        return;
    }
    if !auto_check_due(lock(&state.nexus).last_check_ms, persist::now_ms()) {
        return;
    }
    let linked = {
        let shelf = state.shelf();
        let store = lock(&state.nexus);
        shelf.packages().iter().any(|p| store.link_for(p).is_some())
    };
    if linked {
        if let Err(err) = check_updates_impl(state) {
            log::warn!("the launch update check did not finish: {err}");
        }
    }
}

/// Hash each package's archive and look the hashes up in one batch. A match
/// links the package to its exact Nexus file (source `md5`).
pub fn identify_packages_impl(
    state: &AppState,
    package_ids: &[String],
) -> Result<Vec<Md5Result>, RiftError> {
    let shelf = state.shelf();
    let total = package_ids.len();
    let mut hashed: Vec<Option<String>> = Vec::with_capacity(total);
    for (n, id) in package_ids.iter().enumerate() {
        let package = shelf.package(&PackageId(id.clone()));
        let md5 = package
            .filter(|p| p.format != PackageFormat::Folder && p.path.is_file())
            .and_then(|p| match rift_nexus::md5_file(&p.path, &mut |_| {}) {
                Ok(md5) => Some(md5),
                Err(err) => {
                    log::warn!("could not hash {}: {err}", p.path.display());
                    None
                }
            });
        hashed.push(md5);
        state.events.emit(
            events::UPDATES_PROGRESS,
            &UpdatesProgressEvent {
                phase: UpdatesPhase::Hashing,
                done: n + 1,
                total,
            },
        );
    }
    let asked: Vec<String> = hashed.iter().flatten().cloned().collect();
    let linking = |done: usize| {
        state.events.emit(
            events::UPDATES_PROGRESS,
            &UpdatesProgressEvent {
                phase: UpdatesPhase::Linking,
                done,
                total: asked.len(),
            },
        );
    };
    let hits = if asked.is_empty() {
        Vec::new()
    } else {
        linking(0);
        let client = read(&state.nexus_client).clone();
        let hits = client.files_by_md5(&asked).map_err(nexus_error)?;
        linking(asked.len());
        hits
    };

    let mut results = Vec::with_capacity(total);
    let mut matched_any = false;
    let mut store = lock(&state.nexus);
    for (id, md5) in package_ids.iter().zip(&hashed) {
        let package = shelf.package(&PackageId(id.clone()));
        let hit = md5
            .as_ref()
            .and_then(|md5| hits.iter().find(|hit| &hit.md5 == md5));
        match (package, hit) {
            (Some(package), Some(hit)) => {
                let link = hit.to_link();
                store.set_link(package, link.clone());
                matched_any = true;
                results.push(Md5Result {
                    package_id: id.clone(),
                    matched: true,
                    link: Some(dto::NexusLink::from(&link)),
                });
            }
            _ => results.push(Md5Result {
                package_id: id.clone(),
                matched: false,
                link: None,
            }),
        }
    }
    drop(store);
    if matched_any {
        state.save_nexus();
        state.events.changed(events::UPDATES_CHANGED);
    }
    Ok(results)
}

/// Link a package by a pasted nexusmods.com/dragonsdogma2 URL or a bare mod id.
pub fn link_package_impl(
    state: &AppState,
    package_id: &str,
    url_or_id: &str,
) -> Result<UpdateRow, RiftError> {
    let shelf = state.shelf();
    let package = shelf
        .package(&PackageId(package_id.to_string()))
        .ok_or_else(|| not_found(package_id))?;
    let (mod_id, file_id) =
        rift_nexus::parse_mod_url(url_or_id).ok_or_else(|| RiftError::Nexus {
            detail: "it is neither a nexusmods.com/dragonsdogma2 mod page nor a mod id".into(),
        })?;
    let row = {
        let mut store = lock(&state.nexus);
        store.set_link(
            package,
            NexusLink {
                mod_id,
                file_id,
                group_id: None,
                sqid: None,
                version: None,
                uploaded_ms: None,
                source: LinkSource::Manual,
            },
        );
        store.row(package)
    };
    state.save_nexus();
    state.events.changed(events::UPDATES_CHANGED);
    Ok(row)
}

pub fn unlink_package_impl(state: &AppState, package_id: &str) -> Result<(), RiftError> {
    let shelf = state.shelf();
    let package = shelf
        .package(&PackageId(package_id.to_string()))
        .ok_or_else(|| not_found(package_id))?;
    lock(&state.nexus).unlink(package);
    state.save_nexus();
    state.events.changed(events::UPDATES_CHANGED);
    Ok(())
}

/// Set (validated against v1 `users/validate.json` first) or clear the
/// personal API key. The key goes to the credential store and the client;
/// nowhere else.
pub fn set_nexus_key_impl(state: &AppState, key: Option<&str>) -> Result<NexusSummary, RiftError> {
    let key = key.map(str::trim).filter(|k| !k.is_empty());
    let base = read(&state.nexus_client).clone();
    match key {
        Some(key) => {
            let keyed = base.with_api_key(Some(key.to_string()));
            keyed.validate_key().map_err(nexus_error)?;
            state.keys.store(key)?;
            *write(&state.nexus_client) = keyed;
            state.has_api_key.store(true, Ordering::SeqCst);
        }
        None => {
            state.keys.forget()?;
            *write(&state.nexus_client) = base.with_api_key(None);
            state.has_api_key.store(false, Ordering::SeqCst);
        }
    }
    Ok(summary(state, &state.shelf()))
}

/// The v1 budget from the last `X-RL-*` headers; `None` without a key or
/// before any v1 call (anonymous v2 carries no quota headers).
pub fn get_nexus_quota_impl(state: &AppState) -> Option<NexusQuota> {
    if !state.has_api_key.load(Ordering::SeqCst) {
        return None;
    }
    read(&state.nexus_client)
        .last_quota()
        .as_ref()
        .and_then(quota_view)
}

// -- wrappers ----------------------------------------------------------------

#[tauri::command]
pub async fn get_updates(state: Spine<'_>) -> Result<UpdatesView, RiftError> {
    blocking(&state, |state| Ok(get_updates_impl(state))).await
}

#[tauri::command]
pub async fn check_updates(state: Spine<'_>) -> Result<UpdatesView, RiftError> {
    blocking(&state, |state| check_updates_impl(state)).await
}

#[tauri::command]
pub async fn identify_packages(
    state: Spine<'_>,
    package_ids: Vec<String>,
) -> Result<Vec<Md5Result>, RiftError> {
    blocking(&state, move |state| {
        identify_packages_impl(state, &package_ids)
    })
    .await
}

#[tauri::command]
pub async fn link_package(
    state: Spine<'_>,
    package_id: String,
    url_or_id: String,
) -> Result<UpdateRow, RiftError> {
    blocking(&state, move |state| {
        link_package_impl(state, &package_id, &url_or_id)
    })
    .await
}

#[tauri::command]
pub async fn unlink_package(state: Spine<'_>, package_id: String) -> Result<(), RiftError> {
    blocking(&state, move |state| unlink_package_impl(state, &package_id)).await
}

#[tauri::command]
pub async fn set_nexus_key(
    state: Spine<'_>,
    key: Option<String>,
) -> Result<NexusSummary, RiftError> {
    blocking(&state, move |state| {
        set_nexus_key_impl(state, key.as_deref())
    })
    .await
}

#[tauri::command]
pub async fn get_nexus_quota(state: Spine<'_>) -> Result<Option<NexusQuota>, RiftError> {
    blocking(&state, |state| Ok(get_nexus_quota_impl(state))).await
}
