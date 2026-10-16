//! Nexus Mods: the update rows, the check, MD5 identification, Find on Nexus
//! by name (v0.2), links, the optional API key and its quota, and the labels
//! (v0.2) the window filters by.
//!
//! Every network call here runs on a blocking thread (the commands' blocking
//! pool or the wake-up thread): rift-nexus's client is `reqwest::blocking`,
//! which must never be created, used or dropped on the async runtime.
//!
//! Whatever changes which mod a package is linked to, or what a check heard
//! about a mod, relabels the library before `rift://updates-changed` goes out,
//! so the window's refetch of `get_labels` and its searches see the new labels.
//! A find changes neither: it only asks.

use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};

use rift_core::{LinkSource, NexusLink, Package, PackageFormat, PackageId, RiftError};
use rift_nexus::{ModFilesReport, NexusMod};

use crate::commands::{blocking, not_found, Spine};
use crate::dto::{
    self, LabelFacets, Md5Result, NexusCandidate, NexusFindings, NexusQuota, NexusSummary,
    UpdateRow, UpdateStateView, UpdatesPhase, UpdatesProgressEvent, UpdatesView,
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
/// The mod lookup brings each mod's labels too (v0.2): the library is
/// relabeled before the window hears the check ended.
pub fn check_updates_impl(state: &AppState) -> Result<UpdatesView, RiftError> {
    let _one_at_a_time = lock(&state.nexus_check);
    state.nexus_checking.store(true, Ordering::SeqCst);
    let checking = Checking(&state.nexus_checking);
    let result = run_check(state);
    drop(checking);
    if result.is_ok() {
        state.relabel();
    }
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
        store.record_labels(&mod_ids, &mods);
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
    // Let go of the shelf before relabeling it (see `AppState::relabel`).
    drop(shelf);
    if matched_any {
        state.save_nexus();
        state.relabel();
        state.events.changed(events::UPDATES_CHANGED);
    }
    Ok(results)
}

/// Search Nexus for where a package came from, by its NAME (v0.2): for what
/// MD5 cannot identify, an unpacked folder (nothing to hash) or a repack (new
/// bytes). The words come from the package's display name
/// ([`rift_nexus::search_terms`]), or, when it holds nothing searchable, from
/// its first option's bundle, else that option's name; when they find
/// nothing, one more find with the next of those that makes a different
/// search ([`find_plan`]). Each find is a MATCHES search and, on no hit, one
/// WILDCARD retry, so a call sends at most four POSTs; `terms` is the set
/// answered last. When none of the names holds anything searchable, nothing
/// is sent (`terms: ""`).
///
/// Nothing is linked and nothing is announced: the player picks a candidate,
/// the window links it with `link_package` and checks, and the labels and
/// verdict follow. Each candidate says how many packages of the library are
/// linked to its mod at the moment of the answer.
pub fn find_on_nexus_impl(state: &AppState, package_id: &str) -> Result<NexusFindings, RiftError> {
    let (first, fallback) = {
        let shelf = state.shelf();
        let package = shelf
            .package(&PackageId(package_id.to_string()))
            .ok_or_else(|| not_found(package_id))?;
        find_plan(package)
    };
    let mut findings = NexusFindings {
        package_id: package_id.to_string(),
        terms: first,
        candidates: Vec::new(),
    };
    if findings.terms.is_empty() {
        return Ok(findings);
    }
    let client = read(&state.nexus_client).clone();
    let mut found = client.find_mods(&findings.terms).map_err(nexus_error)?;
    if found.is_empty() {
        if let Some(fallback) = fallback {
            found = client.find_mods(&fallback).map_err(nexus_error)?;
            findings.terms = fallback;
        }
    }
    // Who is linked to what, now that the answer is in.
    let shelf = state.shelf();
    let store = lock(&state.nexus);
    let linked_to = |mod_id: u64| {
        shelf
            .packages()
            .iter()
            .filter(|p| store.link_for(p).is_some_and(|link| link.mod_id == mod_id))
            .count()
    };
    findings.candidates = found
        .iter()
        .map(|candidate| NexusCandidate::of(candidate, linked_to(candidate.mod_id)))
        .collect();
    Ok(findings)
}

/// The words a find sends for `package`, and those of the second find for
/// when the first finds nothing: the first two DIFFERENT searches among the
/// package's display name, its first option's bundle and that option's name,
/// in that order, each read by [`rift_nexus::search_terms`] and skipped when
/// it holds nothing searchable. So a folder named like `DD2_v1.2` asks by its
/// first option, whose name is the folder's modinfo name.
/// `("", None)` when none of the three holds anything searchable: then nothing
/// is sent at all.
pub fn find_plan(package: &Package) -> (String, Option<String>) {
    let option = package.options.first();
    let names = [
        Some(package.display_name.as_str()),
        option.and_then(|option| option.bundle.as_deref()),
        option.map(|option| option.name.as_str()),
    ];
    let mut searches: Vec<String> = Vec::with_capacity(2);
    for terms in names.into_iter().flatten().map(rift_nexus::search_terms) {
        if searches.len() < 2
            && !terms.is_empty()
            && !searches.iter().any(|asked| same_search(asked, &terms))
        {
            searches.push(terms);
        }
    }
    let mut searches = searches.into_iter();
    (searches.next().unwrap_or_default(), searches.next())
}

/// Do two sets of terms make the same search? MATCHES and WILDCARD both want
/// every term, in any order, and Nexus ignores case: the same words are the
/// same search.
fn same_search(a: &str, b: &str) -> bool {
    let words = |terms: &str| {
        terms
            .split_whitespace()
            .map(str::to_lowercase)
            .collect::<BTreeSet<String>>()
    };
    words(a) == words(b)
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
    drop(shelf);
    state.save_nexus();
    // A mod an earlier check heard of brings its labels at once.
    state.relabel();
    state.events.changed(events::UPDATES_CHANGED);
    Ok(row)
}

pub fn unlink_package_impl(state: &AppState, package_id: &str) -> Result<(), RiftError> {
    let shelf = state.shelf();
    let package = shelf
        .package(&PackageId(package_id.to_string()))
        .ok_or_else(|| not_found(package_id))?;
    lock(&state.nexus).unlink(package);
    drop(shelf);
    state.save_nexus();
    state.relabel();
    state.events.changed(events::UPDATES_CHANGED);
    Ok(())
}

/// The Nexus categories and tags the library's linked packages carry, each
/// with the installable options under it (count descending, then name), and
/// how many packages a label filter hides (v0.2). No network: the labels are
/// those the latest check heard, kept in `nexus.json`.
pub fn get_labels_impl(state: &AppState) -> LabelFacets {
    LabelFacets::from(&state.shelf().label_facets())
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
pub async fn find_on_nexus(
    state: Spine<'_>,
    package_id: String,
) -> Result<NexusFindings, RiftError> {
    blocking(&state, move |state| find_on_nexus_impl(state, &package_id)).await
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

#[tauri::command]
pub async fn get_labels(state: Spine<'_>) -> Result<LabelFacets, RiftError> {
    blocking(&state, |state| Ok(get_labels_impl(state))).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use rift_core::{ModOption, OptionId, PayloadKinds};
    use std::path::PathBuf;

    fn option(name: &str, bundle: Option<&str>) -> ModOption {
        ModOption {
            id: OptionId(format!("00000000000000aa#{name}")),
            package_id: PackageId("00000000000000aa".into()),
            root: name.into(),
            name: name.into(),
            info: None,
            screenshot: None,
            payload: vec![],
            skipped: vec![],
            kinds: PayloadKinds::default(),
            parent: None,
            addon_for: None,
            bundle: bundle.map(str::to_string),
            menu_only: false,
            trail: vec![],
        }
    }

    fn package(display_name: &str, options: Vec<ModOption>) -> Package {
        Package {
            id: PackageId("00000000000000aa".into()),
            root_id: "fluffy".into(),
            path: PathBuf::from(format!("/mods/{display_name}")),
            file_name: display_name.into(),
            display_name: display_name.into(),
            format: PackageFormat::Folder,
            size: 1,
            modified_ms: 1,
            entry_count: 1,
            options,
            nexus: None,
            problems: vec![],
        }
    }

    #[test]
    fn a_find_asks_by_the_display_name_first() {
        let (first, fallback) = find_plan(&package("LanternGlowRecolor_v2.1", vec![]));
        assert_eq!(first, "Lantern Glow Recolor");
        assert_eq!(fallback, None, "no option, no second find");
    }

    #[test]
    fn the_first_options_bundle_comes_before_its_name() {
        let cloaks = package(
            "Vernworth Wardrobe",
            vec![
                option("Ermine Cape", Some("Vernworth Cloaks Collection")),
                option("Wool Hood", None),
            ],
        );
        assert_eq!(
            find_plan(&cloaks),
            (
                "Vernworth Wardrobe".to_string(),
                Some("Vernworth Cloaks Collection".to_string())
            )
        );
        // No bundle: the option's own name.
        let cape = package("Vernworth Wardrobe", vec![option("Ermine Cape", None)]);
        assert_eq!(find_plan(&cape).1.as_deref(), Some("Ermine Cape"));
    }

    #[test]
    fn a_second_find_must_make_a_different_search() {
        // The same words, in another case or order, or with the noise a
        // folder name carries: Nexus would answer the same, so no second find.
        for (display, option_name) in [
            ("Lantern Glow Recolor", "Lantern Glow Recolor"),
            ("Lantern Glow Recolor", "lantern glow recolor"),
            ("Lantern Glow Recolor", "Recolor Lantern Glow"),
            ("Lantern_Glow_Recolor_v2", "Lantern Glow Recolor (DD2)"),
        ] {
            let lantern = package(display, vec![option(option_name, None)]);
            assert_eq!(find_plan(&lantern).1, None, "{display} / {option_name}");
        }
        // A bundle that makes the same search gives way to a name that does not.
        let lantern = package(
            "Lantern Glow Recolor",
            vec![option("Ember Red", Some("Lantern Glow Recolor"))],
        );
        assert_eq!(find_plan(&lantern).1.as_deref(), Some("Ember Red"));
        // A bundle with nothing searchable gives way too.
        let lantern = package(
            "Lantern Glow Recolor",
            vec![option("Ember Red", Some("v2"))],
        );
        assert_eq!(find_plan(&lantern).1.as_deref(), Some("Ember Red"));
        // Fewer words are a different (wider) search.
        let wider = package("Vernworth Cloaks Collection", vec![option("Cloaks", None)]);
        assert_eq!(find_plan(&wider).1.as_deref(), Some("Cloaks"));
    }

    #[test]
    fn a_name_with_nothing_searchable_asks_by_its_first_option_instead() {
        // A folder called `DD2_v1.2`: its modinfo name is the only clue.
        let folder = package("DD2_v1.2", vec![option("Lantern Glow Recolor", None)]);
        assert_eq!(
            find_plan(&folder),
            ("Lantern Glow Recolor".to_string(), None)
        );
        // The bundle first, then the option's own name as the second find.
        let bundled = package(
            "DD2 v1.2",
            vec![option("Ermine Cape", Some("Vernworth Cloaks"))],
        );
        assert_eq!(
            find_plan(&bundled),
            (
                "Vernworth Cloaks".to_string(),
                Some("Ermine Cape".to_string())
            )
        );
        // A bundle with nothing searchable gives way to the name.
        let quiet_bundle = package("1.0", vec![option("Ermine Cape", Some("v2"))]);
        assert_eq!(find_plan(&quiet_bundle), ("Ermine Cape".to_string(), None));
        // A name that makes the bundle's search again is no second find.
        let same = package(
            "DD2 v1.2",
            vec![option("ermine cape", Some("Ermine Cape (DD2)"))],
        );
        assert_eq!(find_plan(&same), ("Ermine Cape".to_string(), None));
    }

    #[test]
    fn nothing_searchable_anywhere_plans_no_find_at_all() {
        // An option without a modinfo name is named after its package.
        let silent = package("DD2 v1.2", vec![option("DD2 v1.2", None)]);
        assert_eq!(find_plan(&silent), (String::new(), None));
        assert_eq!(find_plan(&package("1.0", vec![])), (String::new(), None));
        let silent_bundle = package("DD2 v1.2", vec![option("v3", Some("FMM"))]);
        assert_eq!(find_plan(&silent_bundle), (String::new(), None));
    }

    #[test]
    fn same_search_ignores_order_case_and_repeats() {
        assert!(same_search("Lantern Glow", "glow LANTERN"));
        assert!(same_search("Lantern Glow Glow", "Lantern Glow"));
        assert!(!same_search("Lantern Glow", "Lantern Glow Recolor"));
        assert!(!same_search("Lantern Glow", "Lantern"));
    }
}
