//! Versions of one file (v0.5, WINDOW-CONTRACT § v0.5): keep only the newest, put one away, the
//! older-versions folder, and bring one back. The put-away an install makes lives here too
//! ([`put_away_older`]); `deploy::install_impl` runs it under the deploy lock once its step
//! committed.
//!
//! The older-versions folder sits beside the mods folder ([`family::older_folder`]). A move
//! there or back is the one change the stone makes to an entry of the mods folder (RS-2 as
//! amended in v0.5): the whole archive or folder, by [`landing::move_aside`], never onto an
//! existing name. A version's `nexus.json` record stays under its package id while it is away;
//! when a move has to take another name (` (2)` …), the record follows it to the id that name
//! gives, so the version finds its family again when it comes back.
//!
//! Ruling 10 adds the player's word ([`join_versions_impl`], [`part_versions_impl`],
//! [`decline_version_question_impl`]): a version Nexus cannot pin (an unpacked folder, a link
//! by mod id) joins the file group another package of the same mod is pinned to, at the
//! player's yes. A join changes the link only: nothing moves.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;

use rift_core::{lower_key, OptionId, Package, PackageFormat, PackageId, RiftError};
use rift_deploy::{DeployContext, Journal};

use crate::commands::deploy::refusal;
use crate::commands::{blocking, not_found, Spine};
use crate::dto::{
    BringBackResult, KeepNewestPlanView, KeepNewestResultView, OlderShelfItem, OlderShelfView,
    PutAwayResultView, PutAwayView, ReplacesView,
};
use crate::events;
use crate::family::{self, Families};
use crate::landing;
use crate::library::Shelf;
use crate::state::{lock, read, AppState, JournalSlot};

/// How deep a folder version's size is counted (rift-archive's folder source lists no deeper).
const FOLDER_MAX_DEPTH: usize = 64;

const NO_ROOM: &str = "The mods folder sits at the root of a drive, so there is no folder beside it to keep older versions in.";

/// What a put-away moved, and the sentence for what would not move.
#[derive(Debug, Default)]
pub(crate) struct PutAway {
    pub moved: Vec<PutAwayView>,
    pub failed: Option<String>,
}

/// Does the journal hold an install of any option of the package?
pub(crate) fn has_install(journal: &Journal, package: &PackageId) -> bool {
    journal
        .installs
        .iter()
        .any(|install| install.option_id.package() == *package)
}

/// Move every package of `older` that has no install (`installed`) to the older-versions folder.
/// The caller holds the deploy lock, and calls this only once the step that left them without
/// installs committed. A version that will not move stays where it was and is named in
/// `failed`; the others still move.
pub(crate) fn put_away_older(
    state: &AppState,
    shelf: &Shelf,
    older: &[PackageId],
    installed: impl Fn(&PackageId) -> bool,
) -> PutAway {
    let mut out = PutAway::default();
    let going: Vec<&Package> = older
        .iter()
        .filter(|id| !installed(id))
        .filter_map(|id| shelf.package(id))
        .collect();
    if going.is_empty() {
        return out;
    }
    let Some(folder) = family::older_folder(state.library_dir()) else {
        out.failed = Some(NO_ROOM.to_string());
        return out;
    };
    // An archive the pool holds open could refuse to move on Windows.
    state.sources.clear();
    let mut stayed: Vec<(String, io::Error)> = Vec::new();
    for package in going {
        match move_package(state, package, &folder) {
            Ok(view) => out.moved.push(view),
            Err(err) => {
                log::warn!("an older version could not be put away: {err}");
                stayed.push((package.file_name.clone(), err));
            }
        }
    }
    out.failed = stayed_sentence(&stayed);
    out
}

/// `putAwayFailed`: the window shows it verbatim and adds where the version stays.
fn stayed_sentence(stayed: &[(String, io::Error)]) -> Option<String> {
    let ((first, why), rest) = stayed.split_first()?;
    Some(if rest.is_empty() {
        format!("\"{first}\" could not be moved to the older-versions folder: {why}.")
    } else {
        let names: Vec<String> = stayed
            .iter()
            .map(|(name, _)| format!("\"{name}\""))
            .collect();
        format!(
            "{} older versions could not be moved to the older-versions folder ({}): {why}.",
            stayed.len(),
            names.join(", ")
        )
    })
}

/// A move or a read the OS refused (context `olderVersions`): the detail is the OS's own words
/// only, since the window names the file and what the stone tried (WINDOW-CONTRACT § v0.5,
/// Errors), and a path never reaches it.
fn refused_by_os(err: &io::Error) -> RiftError {
    RiftError::Io {
        detail: err.to_string(),
    }
}

/// A package's id from what it is on disk (rift-core's rule): archives by name and size,
/// folders by name.
fn id_for(name: &str, is_folder: bool, size: u64) -> PackageId {
    if is_folder {
        PackageId::for_folder(name)
    } else {
        PackageId::for_archive(name, size)
    }
}

/// Move one package to the older-versions folder; its `nexus.json` record follows a new name.
fn move_package(state: &AppState, package: &Package, folder: &Path) -> io::Result<PutAwayView> {
    let landed_as = landing::move_aside(&package.path, folder)?;
    if landed_as != package.file_name {
        let is_folder = package.format == PackageFormat::Folder;
        follow_record(
            state,
            &package.id,
            &id_for(&landed_as, is_folder, package.size),
        );
    }
    Ok(PutAwayView {
        file_name: package.file_name.clone(),
        landed_as,
    })
}

/// The record of the package once known as `from` is now `to`'s (a move took another name).
fn follow_record(state: &AppState, from: &PackageId, to: &PackageId) {
    let moved = lock(&state.nexus).move_records(&BTreeMap::from([(from.clone(), to.clone())]));
    if moved {
        state.save_nexus();
    }
}

/// The installs a put-away must not run past: none when the journal cannot be read.
fn ready_journal(state: &AppState) -> Result<Arc<Journal>, RiftError> {
    match &read(&state.world).journal {
        JournalSlot::Ready(journal) => Ok(Arc::clone(journal)),
        JournalSlot::Damaged(err) => Err(err.clone()),
        JournalSlot::NoGame => Err(RiftError::GameNotFound),
    }
}

fn alone(package: &Package) -> String {
    format!(
        "\"{}\" is the only version of its file in the mods folder.",
        package.file_name
    )
}

/// An install a replace or "keep only the newest" uninstalls, as the window lists it, and the
/// sentence its own uninstall plan refuses with (a running game holding its paks).
pub(crate) fn replaces_view(
    families: &Families,
    ctx: &DeployContext,
    journal: &Journal,
    option_id: &OptionId,
) -> (ReplacesView, Option<String>) {
    let plan = rift_deploy::plan_uninstall(ctx, journal, option_id);
    let view = ReplacesView {
        option_id: option_id.0.clone(),
        name: journal
            .find(option_id)
            .map_or_else(|| plan.name.clone(), |install| install.name.clone()),
        version: families.version(&option_id.package()),
        removals: plan.removals.len(),
    };
    (view, plan.refused)
}

/// Keep only the newest: the version kept (the newest member with an install, else the lead)
/// and the members older than it.
fn keep_cut<'a>(members: &'a [PackageId], journal: &Journal) -> (&'a PackageId, &'a [PackageId]) {
    let kept = members
        .iter()
        .position(|id| has_install(journal, id))
        .unwrap_or(0);
    (&members[kept], &members[kept + 1..])
}

/// Every install of the `older` members, bottom layer first.
fn older_installs(older: &[PackageId], journal: &Journal) -> Vec<OptionId> {
    journal
        .installs
        .iter()
        .filter(|install| older.contains(&install.option_id.package()))
        .map(|install| install.option_id.clone())
        .collect()
}

fn nothing_older(shelf: &Shelf, kept: &PackageId) -> String {
    let name = shelf
        .package(kept)
        .map_or_else(|| kept.0.clone(), |p| p.file_name.clone());
    format!(
        "No version of this file is older than \"{name}\", the one kept, so there is nothing to put away."
    )
}

pub fn plan_keep_newest_impl(
    state: &AppState,
    package_id: &str,
) -> Result<KeepNewestPlanView, RiftError> {
    let shelf = state.shelf();
    let id = PackageId(package_id.to_string());
    let package = shelf.package(&id).ok_or_else(|| not_found(package_id))?;
    let families = Families::of_shelf(&shelf, &lock(&state.nexus));
    let mut view = KeepNewestPlanView {
        package_id: families.lead(&id).unwrap_or(&id).0.clone(),
        keeps: None,
        uninstalls: Vec::new(),
        puts_away: Vec::new(),
        refused: None,
    };
    let Some(members) = families.family(&id) else {
        view.refused = Some(alone(package));
        return Ok(view);
    };
    let planned = state.plan(|ctx, journal| {
        let (kept, older) = keep_cut(members, journal);
        let mut refused = None;
        let uninstalls: Vec<ReplacesView> = older_installs(older, journal)
            .iter()
            .map(|option_id| {
                let (row, no) = replaces_view(&families, ctx, journal, option_id);
                refused = refused.take().or(no);
                row
            })
            .collect();
        (kept.clone(), older.to_vec(), uninstalls, refused)
    });
    match planned {
        Ok((kept, older, uninstalls, refused)) => {
            view.keeps = shelf.package(&kept).map(|p| p.file_name.clone());
            if older.is_empty() {
                view.refused = Some(nothing_older(&shelf, &kept));
                return Ok(view);
            }
            view.puts_away = older
                .iter()
                .filter_map(|id| shelf.package(id))
                .map(|p| p.file_name.clone())
                .collect();
            view.uninstalls = uninstalls;
            view.refused = refused;
            Ok(view)
        }
        Err(err) => match refusal(&err) {
            Some(sentence) => {
                view.refused = Some(sentence);
                Ok(view)
            }
            None => Err(err),
        },
    }
}

/// Uninstall every install of the versions older than the newest installed one, each by its
/// own uninstall, then put those versions away; all under one turn of the deploy lock.
pub fn keep_newest_impl(
    state: &Arc<AppState>,
    package_id: &str,
) -> Result<KeepNewestResultView, RiftError> {
    let shelf = state.shelf();
    let id = PackageId(package_id.to_string());
    let package = shelf.package(&id).ok_or_else(|| not_found(package_id))?;
    let families = Families::of_shelf(&shelf, &lock(&state.nexus));
    let members = families
        .family(&id)
        .ok_or_else(|| RiftError::refused(alone(package)))?;
    let mut uninstalled = 0;
    let done = state.deploy(|ctx, journal| {
        let (kept, older) = keep_cut(members, journal);
        if older.is_empty() {
            return Err(RiftError::refused(nothing_older(&shelf, kept)));
        }
        for option_id in older_installs(older, journal) {
            rift_deploy::uninstall(ctx, journal, &option_id)?;
            uninstalled += 1;
        }
        let journal: &Journal = journal;
        Ok(put_away_older(state, &shelf, older, |id| {
            has_install(journal, id)
        }))
    });
    if uninstalled > 0 {
        state.events.changed(events::GAME_CHANGED);
    }
    let put = done?;
    if !put.moved.is_empty() {
        state.request_scan();
    }
    Ok(KeepNewestResultView {
        uninstalled,
        put_away: put.moved,
        put_away_failed: put.failed,
    })
}

/// Move one older version with no installs to the older-versions folder, at the player's press.
/// The family is read under the turn, so a join taken back or a scan that landed since the press
/// cannot let a version that is no longer an older one move.
pub fn put_away_impl(state: &Arc<AppState>, package_id: &str) -> Result<PutAwayView, RiftError> {
    let id = PackageId(package_id.to_string());
    let view = {
        // One change at a time: no install of it can start while it moves.
        let _turn = state.turn();
        let shelf = state.shelf();
        let package = shelf.package(&id).ok_or_else(|| not_found(package_id))?;
        let families = Families::of_shelf(&shelf, &lock(&state.nexus));
        if families.family(&id).is_none() {
            return Err(RiftError::refused(alone(package)));
        }
        if !families.is_older(&id) {
            return Err(RiftError::refused(format!(
                "\"{}\" is the newest version of its file. Only older versions are put away.",
                package.file_name
            )));
        }
        let folder =
            family::older_folder(state.library_dir()).ok_or_else(|| RiftError::refused(NO_ROOM))?;
        let journal = ready_journal(state)?;
        if has_install(&journal, &id) {
            return Err(RiftError::refused(format!(
                "\"{}\" still has an install in your world. Uninstall it first, or keep only the newest version.",
                package.file_name
            )));
        }
        state.sources.clear();
        move_package(state, package, &folder).map_err(|err| refused_by_os(&err))?
    };
    state.request_scan();
    Ok(view)
}

/// The put-away an install of `package_id` makes after it commits, made again (the landing's
/// `Try again`): every member older than it left without installs, the family read under one
/// turn. Like the install's own, it moves what it can and names what would not move, so a
/// landing never loses a move that worked to a later one that failed.
pub fn retry_put_away_impl(
    state: &Arc<AppState>,
    package_id: &str,
) -> Result<PutAwayResultView, RiftError> {
    let id = PackageId(package_id.to_string());
    let put = {
        let _turn = state.turn();
        let shelf = state.shelf();
        shelf.package(&id).ok_or_else(|| not_found(package_id))?;
        let journal = ready_journal(state)?;
        let families = Families::of_shelf(&shelf, &lock(&state.nexus));
        put_away_older(state, &shelf, families.older_than(&id), |older| {
            has_install(&journal, older)
        })
    };
    if !put.moved.is_empty() {
        state.request_scan();
    }
    Ok(PutAwayResultView {
        put_away: put.moved,
        put_away_failed: put.failed,
    })
}

/// A folder version's bytes on disk, as the scan counts a folder package's size.
fn folder_bytes(path: &Path, depth: usize) -> u64 {
    if depth > FOLDER_MAX_DEPTH {
        return 0;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => folder_bytes(&entry.path(), depth + 1),
            Ok(kind) if kind.is_file() => entry.metadata().map_or(0, |m| m.len()),
            _ => 0,
        })
        .sum()
}

/// What the older-versions folder holds: its archives and folders, as a scan would list them,
/// each with its kept Nexus record.
pub fn get_older_versions_impl(state: &AppState) -> Result<OlderShelfView, RiftError> {
    let none = OlderShelfView {
        exists: false,
        items: Vec::new(),
    };
    let Some(folder) = family::older_folder(state.library_dir()) else {
        return Ok(none);
    };
    if !folder.is_dir() {
        return Ok(none);
    }
    let shelf = state.shelf();
    let store = lock(&state.nexus);
    let items = rift_catalog::scan::list_library(&folder)
        .iter()
        .filter_map(|path| {
            let file_name = path.file_name()?.to_str()?.to_string();
            let meta = fs::metadata(path).ok()?;
            let is_folder = meta.is_dir();
            let on_disk = if is_folder {
                folder_bytes(path, 0)
            } else {
                meta.len()
            };
            let id = id_for(&file_name, is_folder, meta.len());
            let link = store.record_link(&id);
            let newer_in_mods = link.as_ref().is_some_and(|link| {
                family::newer_in_mods(
                    &shelf,
                    &store,
                    &file_name,
                    link,
                    store.newer_file_id(&id, link),
                )
            });
            Some(OlderShelfItem {
                mod_name: link.as_ref().and_then(|l| store.mod_name(&id, l)),
                version: link.as_ref().and_then(|l| l.version.clone()),
                file_name,
                size: on_disk,
                newer_in_mods,
            })
        })
        .collect();
    Ok(OlderShelfView {
        exists: true,
        items,
    })
}

/// Move an item of the older-versions folder back into the mods folder, by the rules a
/// download lands by; a scan follows.
pub fn bring_back_impl(
    state: &Arc<AppState>,
    file_name: &str,
) -> Result<BringBackResult, RiftError> {
    let wanted = lower_key(file_name.trim());
    let folder = family::older_folder(state.library_dir()).ok_or_else(|| not_found(file_name))?;
    // Only what the folder's own listing holds: never a path, never a file no scan would read.
    let path = rift_catalog::scan::list_library(&folder)
        .into_iter()
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| lower_key(name) == wanted)
        })
        .ok_or_else(|| not_found(file_name))?;
    let there = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();
    let meta = fs::metadata(&path).map_err(|err| refused_by_os(&err))?;
    let mods = state.library_dir().to_path_buf();
    let made = !mods.is_dir();
    let landed_as = {
        let _turn = state.turn();
        landing::move_aside(&path, &mods).map_err(|err| refused_by_os(&err))?
    };
    if landed_as != there {
        let is_folder = meta.is_dir();
        follow_record(
            state,
            &id_for(&there, is_folder, meta.len()),
            &id_for(&landed_as, is_folder, meta.len()),
        );
    }
    if made || state.library_unwatched() {
        state.arm_watcher();
    }
    state.request_scan();
    Ok(BringBackResult { landed_as })
}

/// Show the older-versions folder in Explorer, Rust-side, as the mods folder is shown. The
/// folder is made at the first put-away, so before it there is nothing to show (`refused`).
pub fn open_older_versions_folder_impl(state: &AppState) -> Result<(), RiftError> {
    let folder = family::older_folder(state.library_dir())
        .filter(|folder| folder.is_dir())
        .ok_or_else(|| {
            RiftError::refused(
                "The stone hasn't put anything away yet, so the older-versions folder is not there to show.",
            )
        })?;
    state.config.launcher.show_folder(&folder).map_err(|err| {
        log::warn!("{err}");
        RiftError::Io {
            detail: "the desktop would not open it".into(),
        }
    })
}

// -- wrappers ----------------------------------------------------------------

/// The package and its link, for the player's word: refused when it has no link.
fn linked<'a>(
    shelf: &'a Shelf,
    store: &crate::nexus::NexusStore,
    package_id: &str,
) -> Result<(&'a Package, rift_core::NexusLink), RiftError> {
    let package = shelf
        .package(&PackageId(package_id.to_string()))
        .ok_or_else(|| not_found(package_id))?;
    let link = store.link_for(package).ok_or_else(|| {
        RiftError::refused(format!(
            "\"{}\" is not linked to a Nexus mod. Link it first, then say which file it is a version of.",
            package.file_name
        ))
    })?;
    Ok((package, link))
}

/// `of`'s file group, for the player's word: refused when Nexus pinned it to none.
fn pinned_group(of: &Package, link: &rift_core::NexusLink) -> Result<u64, RiftError> {
    link.group_id.ok_or_else(|| {
        RiftError::refused(format!(
            "\"{}\" is not pinned to a file on Nexus, so there is no file to join a version to.",
            of.file_name
        ))
    })
}

/// v0.5 (ruling 10): the player's yes. `package_id` is a version of `of_package_id`'s file: its
/// link takes that file group, `source: "joined"`, and the family rule takes it from there.
pub fn join_versions_impl(
    state: &AppState,
    package_id: &str,
    of_package_id: &str,
) -> Result<(), RiftError> {
    let shelf = state.shelf();
    {
        let mut store = lock(&state.nexus);
        let (package, link) = linked(&shelf, &store, package_id)?;
        let (of, of_link) = linked(&shelf, &store, of_package_id)?;
        if package.id == of.id {
            return Err(RiftError::refused(format!(
                "\"{}\" cannot be a version of itself.",
                package.file_name
            )));
        }
        let group_id = pinned_group(of, &of_link)?;
        if link.mod_id != of_link.mod_id {
            return Err(RiftError::refused(format!(
                "\"{}\" and \"{}\" are linked to different Nexus mods, and versions of one file share their mod.",
                package.file_name, of.file_name
            )));
        }
        if link.group_id.is_some() {
            return Err(RiftError::refused(format!(
                "\"{}\" is already pinned to its own file on Nexus, which says whose version it is.",
                package.file_name
            )));
        }
        store.join(package, link, group_id);
    }
    drop(shelf);
    state.save_nexus();
    state.relabel();
    state.events.changed(events::UPDATES_CHANGED);
    Ok(())
}

/// v0.5 (ruling 10): takes a `joined` link back to the link the package had before, and
/// remembers the answer as a no for that file group.
pub fn part_versions_impl(state: &AppState, package_id: &str) -> Result<(), RiftError> {
    let shelf = state.shelf();
    {
        let package = shelf
            .package(&PackageId(package_id.to_string()))
            .ok_or_else(|| not_found(package_id))?;
        if !lock(&state.nexus).part(package) {
            return Err(RiftError::refused(format!(
                "\"{}\" was not joined to another file by your word, so there is nothing to take back.",
                package.file_name
            )));
        }
    }
    drop(shelf);
    state.save_nexus();
    state.relabel();
    state.events.changed(events::UPDATES_CHANGED);
    Ok(())
}

/// v0.5 (ruling 10): the player's no. The stone does not ask again whether `package_id` is a
/// version of `of_package_id`'s file group.
pub fn decline_version_question_impl(
    state: &AppState,
    package_id: &str,
    of_package_id: &str,
) -> Result<(), RiftError> {
    let shelf = state.shelf();
    {
        let mut store = lock(&state.nexus);
        let package = shelf
            .package(&PackageId(package_id.to_string()))
            .ok_or_else(|| not_found(package_id))?;
        let (of, of_link) = linked(&shelf, &store, of_package_id)?;
        let group_id = pinned_group(of, &of_link)?;
        store.decline(&package.id, group_id);
    }
    drop(shelf);
    state.save_nexus();
    state.events.changed(events::UPDATES_CHANGED);
    Ok(())
}

#[tauri::command]
pub async fn join_versions(
    state: Spine<'_>,
    package_id: String,
    of_package_id: String,
) -> Result<(), RiftError> {
    blocking(&state, move |state| {
        join_versions_impl(state, &package_id, &of_package_id)
    })
    .await
}

#[tauri::command]
pub async fn part_versions(state: Spine<'_>, package_id: String) -> Result<(), RiftError> {
    blocking(&state, move |state| part_versions_impl(state, &package_id)).await
}

#[tauri::command]
pub async fn decline_version_question(
    state: Spine<'_>,
    package_id: String,
    of_package_id: String,
) -> Result<(), RiftError> {
    blocking(&state, move |state| {
        decline_version_question_impl(state, &package_id, &of_package_id)
    })
    .await
}

#[tauri::command]
pub async fn plan_keep_newest(
    state: Spine<'_>,
    package_id: String,
) -> Result<KeepNewestPlanView, RiftError> {
    blocking(&state, move |state| {
        plan_keep_newest_impl(state, &package_id)
    })
    .await
}

#[tauri::command]
pub async fn keep_newest(
    state: Spine<'_>,
    package_id: String,
) -> Result<KeepNewestResultView, RiftError> {
    blocking(&state, move |state| keep_newest_impl(state, &package_id)).await
}

#[tauri::command]
pub async fn put_away(state: Spine<'_>, package_id: String) -> Result<PutAwayView, RiftError> {
    blocking(&state, move |state| put_away_impl(state, &package_id)).await
}

#[tauri::command]
pub async fn retry_put_away(
    state: Spine<'_>,
    package_id: String,
) -> Result<PutAwayResultView, RiftError> {
    blocking(&state, move |state| retry_put_away_impl(state, &package_id)).await
}

#[tauri::command]
pub async fn get_older_versions(state: Spine<'_>) -> Result<OlderShelfView, RiftError> {
    blocking(&state, |state| get_older_versions_impl(state)).await
}

#[tauri::command]
pub async fn open_older_versions_folder(state: Spine<'_>) -> Result<(), RiftError> {
    blocking(&state, |state| open_older_versions_folder_impl(state)).await
}

#[tauri::command]
pub async fn bring_back(state: Spine<'_>, file_name: String) -> Result<BringBackResult, RiftError> {
    blocking(&state, move |state| bring_back_impl(state, &file_name)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_that_will_not_move_is_named_and_the_sentence_counts_the_rest() {
        assert_eq!(stayed_sentence(&[]), None);
        let one = stayed_sentence(&[(
            "Lantern Glow 1.0.zip".into(),
            io::Error::from(io::ErrorKind::PermissionDenied),
        )])
        .unwrap();
        assert_eq!(
            one,
            "\"Lantern Glow 1.0.zip\" could not be moved to the older-versions folder: permission denied."
        );
        let two = stayed_sentence(&[
            (
                "Lantern Glow 1.0.zip".into(),
                io::Error::from(io::ErrorKind::PermissionDenied),
            ),
            (
                "Lantern Glow 1.1.zip".into(),
                io::Error::from(io::ErrorKind::PermissionDenied),
            ),
        ])
        .unwrap();
        assert!(two.starts_with("2 older versions"), "{two}");
        assert!(
            two.contains("\"Lantern Glow 1.0.zip\", \"Lantern Glow 1.1.zip\""),
            "{two}"
        );
    }

    #[test]
    fn ids_follow_rift_cores_rule() {
        assert_eq!(
            id_for("Lantern Glow 1.0.zip", false, 31),
            PackageId::for_archive("Lantern Glow 1.0.zip", 31)
        );
        assert_eq!(
            id_for("Ermine Cape v2", true, 4096),
            PackageId::for_folder("Ermine Cape v2")
        );
    }
}
