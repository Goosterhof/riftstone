//! Reading Nexus without a key (v0.3): one mod's files with their patch
//! notes (`get_mod_files`) and Dragon's Dogma 2's mods page by page
//! (`browse_nexus`). Both anonymous (v2): no key, no quota. Browsing sends
//! only what the player typed; nothing from the library leaves the machine.

use std::cmp::Reverse;
use std::collections::HashSet;

use rift_core::{LinkSource, NexusLink, RiftError};
use rift_nexus::{NexusError, NexusFile, UpdateState};

use crate::commands::standings::linked_counts;
use crate::commands::{blocking, Spine};
use crate::dto::{
    NexusBrowsePage, NexusBrowseQuery, NexusCandidate, NexusFileCategory, NexusFileEntry,
    NexusModFiles,
};
use crate::nexus::nexus_error;
use crate::state::{lock, read, AppState};

/// Where a file category sorts: main, update, optional, miscellaneous, then
/// the retired ones, and a category this build does not know last.
fn category_rank(category: NexusFileCategory) -> u8 {
    match category {
        NexusFileCategory::Main => 0,
        NexusFileCategory::Update => 1,
        NexusFileCategory::Optional => 2,
        NexusFileCategory::Miscellaneous => 3,
        NexusFileCategory::OldVersion => 4,
        NexusFileCategory::Removed => 5,
        NexusFileCategory::Archived => 6,
        NexusFileCategory::Unknown => 7,
    }
}

/// Is `file` the newest of its group that is not retired, by rift-nexus's
/// own update rules (a link pinned to it reads `current`)?
pub fn is_newest(file: &NexusFile, files: &[NexusFile]) -> bool {
    if file.category.is_retired() {
        return false;
    }
    let pinned = NexusLink {
        mod_id: 0,
        file_id: Some(file.file_id),
        group_id: file.group_id,
        sqid: file.sqid.clone(),
        version: file.version.clone(),
        uploaded_ms: Some(file.uploaded_ms),
        source: LinkSource::Manual,
    };
    matches!(
        rift_nexus::update_state(&pinned, None, files),
        UpdateState::Current
    )
}

/// The files the library's packages are pinned to, for one mod.
fn pinned_files(state: &AppState, mod_id: u64) -> HashSet<u64> {
    let shelf = state.shelf();
    let store = lock(&state.nexus);
    shelf
        .packages()
        .iter()
        .filter_map(|package| store.link_for(package))
        .filter(|link| link.mod_id == mod_id)
        .filter_map(|link| link.file_id)
        .collect()
}

/// `get_mod_files`: every file of one mod (v2 `modFiles` with each file's
/// description and patch notes), which one the library is pinned to, and
/// the newest of each group; the mod's name and adult flag from the same
/// anonymous lookup the update check makes. A mod Nexus knows nothing of is
/// `fileGone`.
pub fn get_mod_files_impl(state: &AppState, mod_id: u64) -> Result<NexusModFiles, RiftError> {
    let client = read(&state.nexus_client).clone();
    let details = client.mod_file_details(mod_id).map_err(nexus_error)?;
    let found = client
        .mods(&[mod_id])
        .map_err(nexus_error)?
        .into_iter()
        .find(|found| found.mod_id == mod_id);
    if details.is_empty() && found.is_none() {
        return Err(NexusError::FileGone.into());
    }
    let pinned = pinned_files(state, mod_id);
    let files: Vec<NexusFile> = details.iter().map(|detail| detail.file.clone()).collect();
    let mut entries: Vec<NexusFileEntry> = details
        .into_iter()
        .map(|detail| {
            let newest = is_newest(&detail.file, &files);
            let file = detail.file;
            NexusFileEntry {
                file_id: file.file_id,
                name: file.name,
                version: file.version,
                category: file.category.into(),
                size: file.size_bytes,
                uploaded_ms: file.uploaded_ms,
                description: detail.description,
                changelog: detail.changelog,
                in_library: pinned.contains(&file.file_id),
                newest,
            }
        })
        .collect();
    entries.sort_by_key(|entry| {
        (
            category_rank(entry.category),
            Reverse(entry.uploaded_ms),
            Reverse(entry.file_id),
        )
    });
    Ok(NexusModFiles {
        mod_id,
        mod_name: found.as_ref().map(|found| found.name.clone()),
        page_url: rift_nexus::mod_page_url(mod_id),
        adult: found.as_ref().is_some_and(|found| found.adult),
        files: entries,
    })
}

/// `browse_nexus`: one page of Dragon's Dogma 2's mods (20), by the words
/// the player typed and an order; each card says how many packages of the
/// library are linked to its mod.
pub fn browse_nexus_impl(
    state: &AppState,
    query: &NexusBrowseQuery,
) -> Result<NexusBrowsePage, RiftError> {
    let client = read(&state.nexus_client).clone();
    let page = client
        .browse(&rift_nexus::BrowseQuery {
            text: query.text.clone(),
            sort: query.sort.into(),
            offset: query.offset,
        })
        .map_err(nexus_error)?;
    let linked = linked_counts(state);
    Ok(NexusBrowsePage {
        total: page.total,
        offset: page.offset,
        mods: page
            .mods
            .iter()
            .map(|found| NexusCandidate::of(found, linked.get(&found.mod_id).copied().unwrap_or(0)))
            .collect(),
    })
}

// -- wrappers ----------------------------------------------------------------

#[tauri::command]
pub async fn get_mod_files(state: Spine<'_>, mod_id: u64) -> Result<NexusModFiles, RiftError> {
    blocking(&state, move |state| get_mod_files_impl(state, mod_id)).await
}

#[tauri::command]
pub async fn browse_nexus(
    state: Spine<'_>,
    query: NexusBrowseQuery,
) -> Result<NexusBrowsePage, RiftError> {
    blocking(&state, move |state| browse_nexus_impl(state, &query)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use rift_nexus::FileCategory;

    fn file(file_id: u64, group: u64, category: FileCategory, uploaded_ms: u64) -> NexusFile {
        NexusFile {
            file_id,
            group_id: Some(group),
            sqid: Some(format!("sq{file_id}")),
            name: "Lantern Glow Recolor".into(),
            version: Some(format!("{file_id}")),
            category,
            uploaded_ms,
            primary: false,
            size_bytes: Some(1024),
        }
    }

    #[test]
    fn the_newest_live_file_of_each_group_is_what_an_update_fetches() {
        let files = vec![
            file(1, 7, FileCategory::Archived, 10),
            file(2, 7, FileCategory::Main, 20),
            file(3, 7, FileCategory::Main, 30),
            file(4, 8, FileCategory::Optional, 15),
            file(5, 8, FileCategory::OldVersion, 40),
        ];
        let newest: Vec<u64> = files
            .iter()
            .filter(|f| is_newest(f, &files))
            .map(|f| f.file_id)
            .collect();
        assert_eq!(
            newest,
            vec![3, 4],
            "per group, never a retired file, even the latest upload"
        );
    }

    #[test]
    fn categories_sort_live_first_and_unknown_last() {
        let mut categories = vec![
            NexusFileCategory::Unknown,
            NexusFileCategory::Archived,
            NexusFileCategory::Miscellaneous,
            NexusFileCategory::Main,
            NexusFileCategory::OldVersion,
            NexusFileCategory::Optional,
            NexusFileCategory::Removed,
            NexusFileCategory::Update,
        ];
        categories.sort_by_key(|c| category_rank(*c));
        assert_eq!(
            categories,
            vec![
                NexusFileCategory::Main,
                NexusFileCategory::Update,
                NexusFileCategory::Optional,
                NexusFileCategory::Miscellaneous,
                NexusFileCategory::OldVersion,
                NexusFileCategory::Removed,
                NexusFileCategory::Archived,
                NexusFileCategory::Unknown,
            ]
        );
    }
}
