//! The spine's Nexus side against a stand-in on a loopback port: the
//! filename link of a 2026 download is pinned by its sqid and judged within
//! its file group; a pasted URL links by hand; an MD5 lookup links exactly;
//! the API key is validated, stored in the (in-memory) credential store, and
//! its quota surfaces; unlinking silences the catalog's guess. v0.2: the
//! same check brings each mod's category, tags and adult flag, and they reach
//! the library's search filters, its facets and its option details; and Find
//! on Nexus searches by a package's name for what MD5 cannot identify (an
//! unpacked folder, a repack), so a pick can link it and a check label it.

mod support;

use serde_json::{json, Value};

use riftstone_lib::commands::{library, nexus};
use riftstone_lib::dto::{
    LabelCount, LinkSource, MatchedField, NexusLabels, SearchFilters, SearchQuery, SearchResult,
    UpdateBadge, UpdateStateView, UpdatesPhase,
};
use riftstone_lib::events::{UPDATES_CHANGED, UPDATES_PROGRESS};
use riftstone_lib::persist::DataDirs;
use riftstone_lib::state::AppState;
use support::{Answer, Bench, Seen, StandIn, BANTER_UPLOADED_S};

fn file(
    file_id: u64,
    sqid: &str,
    version: &str,
    category: &str,
    date: u64,
    primary: u8,
) -> serde_json::Value {
    json!({
        "fileId": file_id, "groupId": 9, "sqid": sqid, "name": "Pawn Banter",
        "version": version, "category": category, "date": date, "primary": primary,
        "sizeInBytes": "100"
    })
}

#[test]
fn every_check_asks_nexus_so_a_fresh_link_is_judged_at_once() {
    let stand_in = StandIn::start(move |request| {
        if request.body.contains("legacyModsByDomain") {
            return Answer::json(json!({ "data": { "legacyModsByDomain": {
                "totalCount": 2,
                "nodes": [
                    { "modId": 1234, "name": "Pawn Banter Plus", "version": "1",
                      "updatedAt": "2026-10-15T09:00:00Z", "status": "published" },
                    { "modId": 4242, "name": "Lantern Glow Recolor", "version": "2.1",
                      "updatedAt": "2026-10-15T09:00:00Z", "status": "published" }
                ]
            }}}));
        }
        if request.body.contains("modFiles") {
            return Answer::json(json!({ "data": {
                "m1234": [file(501, "Ab3dE5fG7", "1", "MAIN", BANTER_UPLOADED_S, 1)],
                "m4242": [file(901, "vW1xY3zA5", "2.1", "MAIN", 1_789_000_000, 1)]
            }}));
        }
        Answer {
            status: 404,
            headers: Vec::new(),
            body: json!({}),
        }
    });
    let bench = Bench::new();
    let mut config = bench.config(DataDirs::under(&bench.data_home()));
    config.nexus_endpoints = Some((stand_in.graphql_url(), stand_in.v1_base()));
    let (state, events) = bench.open(config);
    assert!(state.scan_blocking());

    nexus::check_updates_impl(&state).unwrap();
    let lantern_id = state
        .shelf()
        .packages()
        .iter()
        .find(|p| p.file_name == "Lantern Glow Recolor.zip")
        .unwrap()
        .id
        .0
        .clone();
    // Linked a moment after the check; the player presses Check again at once.
    nexus::link_package_impl(&state, &lantern_id, "4242").unwrap();
    events.clear();
    let again = nexus::check_updates_impl(&state).unwrap();

    let lantern = again
        .rows
        .iter()
        .find(|r| r.package_id == lantern_id)
        .unwrap();
    assert_ne!(
        lantern.state,
        UpdateStateView::NotChecked,
        "the package linked since the last check is judged by this one"
    );
    assert!(lantern.checked_ms.is_some());
    assert!(
        !events.named(UPDATES_PROGRESS).is_empty(),
        "the check reports its progress"
    );
    assert_eq!(events.named(UPDATES_CHANGED).len(), 1, "and its end");
    let asked = stand_in
        .seen()
        .iter()
        .filter(|r| r.body.contains("legacyModsByDomain"))
        .count();
    assert_eq!(asked, 2, "both presses asked Nexus");
}

#[test]
fn update_checks_pin_judge_link_identify_and_key_against_a_stand_in() {
    let bench = Bench::new();
    let lantern_zip = bench.mods.join("Lantern Glow Recolor.zip");
    let lantern_md5 = rift_nexus::md5_file(&lantern_zip, &mut |_| {}).unwrap();
    let md5 = lantern_md5.clone();
    let stand_in = StandIn::start(move |request| {
        if request.path.ends_with("/users/validate.json") {
            let mut answer = Answer::json(json!({
                "user_id": 1, "name": "Arisen", "is_premium": false, "is_supporter": false
            }));
            answer.headers = vec![
                ("X-RL-Daily-Limit".into(), "20000".into()),
                ("X-RL-Daily-Remaining".into(), "19990".into()),
                ("X-RL-Hourly-Limit".into(), "500".into()),
                ("X-RL-Hourly-Remaining".into(), "499".into()),
                (
                    "X-RL-Hourly-Reset".into(),
                    "2026-10-16T13:00:00+00:00".into(),
                ),
            ];
            return answer;
        }
        if request.body.contains("legacyModsByDomain") {
            return Answer::json(json!({ "data": { "legacyModsByDomain": {
                "totalCount": 1,
                "nodes": [{ "modId": 1234, "name": "Pawn Banter Plus", "version": "1",
                            "updatedAt": "2026-10-15T09:00:00Z", "status": "published" }]
            }}}));
        }
        if request.body.contains("modFiles") {
            return Answer::json(json!({ "data": { "m1234": [
                file(501, "Ab3dE5fG7", "1", "ARCHIVED", BANTER_UPLOADED_S, 0),
                file(502, "Zz9yX8wV7", "2", "MAIN", BANTER_UPLOADED_S + 86_400, 1),
            ]}}));
        }
        if request.body.contains("fileHashes") {
            return Answer::json(json!({ "data": { "fileHashes": [{
                "md5": md5, "fileName": "Lantern Glow Recolor-4242-2-1-1789000000.zip",
                "fileSize": "1048576", "gameId": 6234, "modFileId": 901,
                "createdAt": "2026-09-09T12:26:40Z",
                "modFile": { "modId": 4242, "fileId": 901, "groupId": 88001, "sqid": "vW1xY3zA5",
                             "name": "Lantern Glow Recolor", "version": "2.1", "category": "MAIN",
                             "date": 1789000000, "primary": 1, "sizeInBytes": "1048576" }
            }]}}));
        }
        Answer {
            status: 404,
            headers: Vec::new(),
            body: json!({}),
        }
    });

    let mut config = bench.config(DataDirs::under(&bench.data_home()));
    config.nexus_endpoints = Some((stand_in.graphql_url(), stand_in.v1_base()));
    let (state, events) = bench.open(config);
    assert!(state.scan_blocking());

    // -- before any check: the download's file name links it -----------------
    let view = nexus::get_updates_impl(&state);
    assert_eq!(view.last_check_ms, None);
    let banter = view
        .rows
        .iter()
        .find(|r| r.package_name == "Pawn Banter")
        .expect("the Nexus download has a row");
    assert_eq!(banter.state, UpdateStateView::NotChecked);
    let link = banter.link.as_ref().unwrap();
    assert_eq!((link.mod_id, link.source), (1234, LinkSource::Filename));
    assert_eq!(link.sqid.as_deref(), Some("Ab3dE5fG7"));
    assert_eq!(
        banter.page_url.as_deref(),
        Some("https://www.nexusmods.com/dragonsdogma2/mods/1234")
    );
    assert!(view
        .rows
        .iter()
        .filter(|r| r.package_name != "Pawn Banter")
        .all(|r| r.state == UpdateStateView::Unlinked));

    // -- the check: pinned by sqid, the newer MAIN file in its group wins ----
    let checked = nexus::check_updates_impl(&state).unwrap();
    assert!(checked.last_check_ms.is_some());
    let banter = checked
        .rows
        .iter()
        .find(|r| r.package_name == "Pawn Banter")
        .unwrap();
    match &banter.state {
        UpdateStateView::UpdateAvailable { newer } => {
            assert_eq!(newer.file_id, 502);
            assert_eq!(newer.version.as_deref(), Some("2"));
        }
        other => panic!("expected an update, got {other:?}"),
    }
    assert_eq!(
        banter.link.as_ref().unwrap().file_id,
        Some(501),
        "pinned to the exact file"
    );
    assert_eq!(banter.link.as_ref().unwrap().source, LinkSource::Filename);
    assert_eq!(banter.mod_name.as_deref(), Some("Pawn Banter Plus"));
    assert!(banter.checked_ms.is_some());
    let phases: Vec<_> = events
        .named(UPDATES_PROGRESS)
        .iter()
        .map(|p| p["phase"].as_str().unwrap().to_string())
        .collect();
    assert!(phases.iter().all(|p| p == "checking") && !phases.is_empty());
    assert_eq!(events.named(UPDATES_CHANGED).len(), 1);
    assert!(stand_in
        .seen()
        .iter()
        .all(|r| r.header("Application-Name") == Some("Riftstone")));

    let summary = nexus::summary(&state, &state.shelf());
    assert_eq!(
        (summary.linked, summary.updates, summary.checking),
        (1, 1, false)
    );
    let hit = &library::search_impl(
        &state,
        &SearchQuery {
            text: "banter".into(),
            filters: SearchFilters::default(),
            limit: 0,
        },
    )[0];
    assert_eq!(hit.update, Some(UpdateBadge::Available));

    // -- a pasted URL links by hand; garbage is refused ----------------------
    let lantern_id = state
        .shelf()
        .packages()
        .iter()
        .find(|p| p.file_name == "Lantern Glow Recolor.zip")
        .unwrap()
        .id
        .0
        .clone();
    let row = nexus::link_package_impl(
        &state,
        &lantern_id,
        "https://www.nexusmods.com/dragonsdogma2/mods/4242?tab=files&file_id=901",
    )
    .unwrap();
    let link = row.link.unwrap();
    assert_eq!(
        (link.mod_id, link.file_id, link.source),
        (4242, Some(901), LinkSource::Manual)
    );
    assert_eq!(row.state, UpdateStateView::NotChecked);
    assert!(matches!(
        nexus::link_package_impl(&state, &lantern_id, "https://example.com/not-a-mod"),
        Err(rift_core::RiftError::Nexus { .. })
    ));

    // -- an MD5 lookup links exactly ------------------------------------------
    events.clear();
    let found = nexus::identify_packages_impl(&state, std::slice::from_ref(&lantern_id)).unwrap();
    assert_eq!(found.len(), 1);
    assert!(found[0].matched);
    let link = found[0].link.as_ref().unwrap();
    assert_eq!(
        (link.mod_id, link.file_id, link.source),
        (4242, Some(901), LinkSource::Md5)
    );
    let phases: Vec<serde_json::Value> = events
        .named(UPDATES_PROGRESS)
        .iter()
        .map(|p| p["phase"].clone())
        .collect();
    assert_eq!(
        phases,
        vec![
            json!(UpdatesPhase::Hashing),
            json!(UpdatesPhase::Linking),
            json!(UpdatesPhase::Linking)
        ],
        "hash each archive, then link them in one lookup"
    );
    assert!(stand_in
        .seen()
        .iter()
        .any(|r| r.body.contains(&lantern_md5)));

    // -- unlinking silences the file-name guess for good ----------------------
    let banter_id = state
        .shelf()
        .packages()
        .iter()
        .find(|p| p.display_name == "Pawn Banter")
        .unwrap()
        .id
        .0
        .clone();
    nexus::unlink_package_impl(&state, &banter_id).unwrap();
    let rows = nexus::get_updates_impl(&state).rows;
    let banter = rows.iter().find(|r| r.package_id == banter_id).unwrap();
    assert_eq!(banter.state, UpdateStateView::Unlinked);
    assert!(banter.link.is_none());

    // -- the API key: validated, stored, quota shown; cleared again -----------
    assert!(nexus::get_nexus_quota_impl(&state).is_none());
    let keyed = nexus::set_nexus_key_impl(&state, Some("  invented-personal-key  ")).unwrap();
    assert!(keyed.has_api_key);
    let validate = stand_in
        .seen()
        .into_iter()
        .find(|r| r.path.ends_with("/users/validate.json"))
        .expect("the key was validated");
    assert_eq!(
        validate.header("apikey"),
        Some("invented-personal-key"),
        "trimmed"
    );
    let quota = nexus::get_nexus_quota_impl(&state).expect("the v1 answer carried a quota");
    assert_eq!((quota.hourly_remaining, quota.hourly_limit), (499, 500));
    assert_eq!((quota.daily_remaining, quota.daily_limit), (19_990, 20_000));
    assert!(quota.hourly_reset_ms.is_some());
    let cleared = nexus::set_nexus_key_impl(&state, None).unwrap();
    assert!(!cleared.has_api_key);
    assert!(nexus::get_nexus_quota_impl(&state).is_none());

    // -- the store survives a relaunch ------------------------------------------
    drop(state);
    let mut config = bench.config(DataDirs::under(&bench.data_home()));
    config.nexus_endpoints = Some((stand_in.graphql_url(), stand_in.v1_base()));
    let (again, _) = bench.open(config);
    let rows = nexus::get_updates_impl(&again).rows;
    assert!(rows
        .iter()
        .find(|r| r.package_id == banter_id)
        .unwrap()
        .link
        .is_none());
    let lantern = rows.iter().find(|r| r.package_id == lantern_id).unwrap();
    assert_eq!(lantern.link.as_ref().unwrap().source, LinkSource::Md5);
}

/// A search with no text and these label filters (menu entries left out, as
/// the window's default filters leave them).
fn by_labels(state: &AppState, categories: &[&str], tags: &[&str]) -> Vec<SearchResult> {
    library::search_impl(
        state,
        &SearchQuery {
            text: String::new(),
            filters: SearchFilters {
                categories: categories.iter().map(|c| (*c).to_string()).collect(),
                tags: tags.iter().map(|t| (*t).to_string()).collect(),
                ..SearchFilters::default()
            },
            limit: 0,
        },
    )
}

fn names(hits: &[SearchResult]) -> Vec<&str> {
    let mut names: Vec<&str> = hits.iter().map(|hit| hit.name.as_str()).collect();
    names.sort_unstable();
    names
}

fn count(name: &str, options: usize) -> LabelCount {
    LabelCount {
        name: name.into(),
        options,
    }
}

fn package_id(state: &AppState, file_name: &str) -> String {
    state
        .shelf()
        .packages()
        .iter()
        .find(|p| p.file_name == file_name)
        .unwrap_or_else(|| panic!("{file_name} is in the library"))
        .id
        .0
        .clone()
}

#[test]
fn a_check_labels_the_library_and_search_filters_by_the_labels() {
    let bench = Bench::new();
    let cloaks_md5 = rift_nexus::md5_file(
        &bench.mods.join("Vernworth Cloaks Collection.zip"),
        &mut |_| {},
    )
    .unwrap();
    let md5 = cloaks_md5.clone();
    let stand_in = StandIn::start(move |request| {
        if request.body.contains("legacyModsByDomain") {
            return Answer::json(json!({ "data": { "legacyModsByDomain": {
                "totalCount": 3,
                "nodes": [
                    { "modId": 1234, "name": "Pawn Banter Plus", "version": "1",
                      "updatedAt": "2026-10-15T09:00:00Z", "status": "published",
                      "category": "Utilities", "tags": [{ "name": "Utilities for Players" }],
                      "adultContent": false },
                    { "modId": 4242, "name": "Lantern Glow Recolor", "version": "2.1",
                      "updatedAt": "2026-10-15T09:00:00Z", "status": "published",
                      "category": "Visuals", "tags": [], "adultContent": null },
                    { "modId": 5150, "name": "Vernworth Cloaks Collection", "version": "2",
                      "updatedAt": "2026-10-15T09:00:00Z", "status": "published",
                      "category": "Armour",
                      "tags": [{ "name": "Cloaks" }, { "name": "Lore-Friendly" }],
                      "adultContent": true }
                ]
            }}}));
        }
        if request.body.contains("modFiles") {
            return Answer::json(json!({ "data": {
                "m1234": [file(501, "Ab3dE5fG7", "1", "MAIN", BANTER_UPLOADED_S, 1)],
                "m4242": [file(901, "vW1xY3zA5", "2.1", "MAIN", 1_789_000_000, 1)],
                "m5150": [file(777, "Rk4vLmN2Q", "2", "MAIN", 1_789_500_000, 1)]
            }}));
        }
        if request.body.contains("fileHashes") {
            return Answer::json(json!({ "data": { "fileHashes": [{
                "md5": md5, "fileName": "Vernworth Cloaks Collection-5150-2-1789500000.zip",
                "fileSize": "2048", "gameId": 6234, "modFileId": 777,
                "createdAt": "2026-09-15T08:00:00Z",
                "modFile": { "modId": 5150, "fileId": 777, "groupId": 31, "sqid": "Rk4vLmN2Q",
                             "name": "Vernworth Cloaks Collection", "version": "2",
                             "category": "MAIN", "date": 1789500000, "primary": 1,
                             "sizeInBytes": "2048" }
            }]}}));
        }
        Answer {
            status: 404,
            headers: Vec::new(),
            body: json!({}),
        }
    });
    let bench_config = || {
        let mut config = bench.config(DataDirs::under(&bench.data_home()));
        config.nexus_endpoints = Some((stand_in.graphql_url(), stand_in.v1_base()));
        config
    };
    let (state, _) = bench.open(bench_config());
    assert!(state.scan_blocking());

    // -- before any check: nothing is labeled, a label filter finds nothing --
    let facets = nexus::get_labels_impl(&state);
    assert!(facets.categories.is_empty() && facets.tags.is_empty());
    assert_eq!(
        facets.unlabeled_packages, 3,
        "the cloaks, the lantern, the banter"
    );
    assert!(by_labels(&state, &["Armour"], &[]).is_empty());

    // Three ways to be linked: the banter by its Nexus file name already; the
    // lantern by hand; the cloaks by an MD5 match.
    let lantern = package_id(&state, "Lantern Glow Recolor.zip");
    let cloaks = package_id(&state, "Vernworth Cloaks Collection.zip");
    nexus::link_package_impl(&state, &lantern, "4242").unwrap();
    let found = nexus::identify_packages_impl(&state, std::slice::from_ref(&cloaks)).unwrap();
    assert!(found[0].matched);
    assert_eq!(found[0].link.as_ref().unwrap().source, LinkSource::Md5);
    assert_eq!(
        nexus::get_labels_impl(&state).unlabeled_packages,
        3,
        "a link alone brings no labels: no check has heard of the mods yet"
    );

    // -- the check: one mod lookup brings the labels ----------------------------
    nexus::check_updates_impl(&state).unwrap();
    let lookups: Vec<_> = stand_in
        .seen()
        .into_iter()
        .filter(|r| r.body.contains("legacyModsByDomain"))
        .collect();
    assert_eq!(lookups.len(), 1, "no extra request for the labels");
    for field in ["category", "tags", "adultContent"] {
        assert!(
            lookups[0].body.contains(field),
            "the lookup asks for {field}"
        );
    }

    let facets = nexus::get_labels_impl(&state);
    assert_eq!(
        facets.categories,
        vec![count("Armour", 2), count("Utilities", 1), count("Visuals", 1)],
        "installable options per category (the cloaks' menu entry does not count), most first, then by name"
    );
    assert_eq!(
        facets.tags,
        vec![
            count("Cloaks", 2),
            count("Lore-Friendly", 2),
            count("Utilities for Players", 1)
        ]
    );
    assert_eq!(facets.unlabeled_packages, 0);

    // -- search: categories ANY-of, tags ALL-of, both case-insensitive ---------
    let armour = by_labels(&state, &["armour"], &[]);
    assert_eq!(names(&armour), vec!["Ermine Cape", "Wool Hood"]);
    for hit in &armour {
        assert_eq!(hit.nexus_category.as_deref(), Some("Armour"));
        assert_eq!(hit.nexus_tags, vec!["Cloaks", "Lore-Friendly"]);
        assert!(hit.adult, "Nexus flags the cloaks as adult");
    }
    assert_eq!(
        names(&by_labels(&state, &["Armour", "Visuals"], &[])),
        vec!["Ermine Cape", "Lantern Glow Recolor", "Wool Hood"],
        "two categories widen"
    );
    assert_eq!(
        by_labels(&state, &[], &["Cloaks", "lore-friendly"]).len(),
        2,
        "every tag asked for is carried"
    );
    assert!(
        by_labels(&state, &[], &["Cloaks", "Utilities for Players"]).is_empty(),
        "two tags narrow"
    );
    let banter = library::search_impl(
        &state,
        &SearchQuery {
            text: "utilities".into(),
            filters: SearchFilters::default(),
            limit: 0,
        },
    );
    assert_eq!(names(&banter), vec!["Pawn Banter"]);
    assert!(banter[0].matched.contains(&MatchedField::NexusLabel));
    assert!(!banter[0].adult);

    // -- the option detail shows the same labels -------------------------------
    let lantern_option = by_labels(&state, &["Visuals"], &[])[0].option_id.clone();
    let detail = library::get_option_impl(&state, &lantern_option).unwrap();
    assert_eq!(
        detail.labels,
        Some(NexusLabels {
            category: Some("Visuals".into()),
            tags: vec![],
            adult: false,
        }),
        "a null adultContent is not adult"
    );

    // -- an unlinked package loses its labels at once ---------------------------
    let banter_id = package_id(&state, support::BANTER_ZIP);
    nexus::unlink_package_impl(&state, &banter_id).unwrap();
    assert!(by_labels(&state, &["Utilities"], &[]).is_empty());
    let after_unlink = nexus::get_labels_impl(&state);
    assert_eq!(
        after_unlink.categories,
        vec![count("Armour", 2), count("Visuals", 1)]
    );
    assert_eq!(after_unlink.unlabeled_packages, 1);

    // -- a relaunch filters at once, before any check: nexus.json kept them -----
    drop(state);
    let (again, _) = bench.open(bench_config());
    assert_eq!(nexus::get_labels_impl(&again), after_unlink);
    assert_eq!(
        names(&by_labels(&again, &["ARMOUR"], &[])),
        vec!["Ermine Cape", "Wool Hood"]
    );
    // …and a rescan keeps them: a new catalog is labeled before it is shown.
    assert!(again.scan_blocking());
    assert_eq!(nexus::get_labels_impl(&again), after_unlink);
    assert_eq!(by_labels(&again, &[], &["Cloaks"]).len(), 2);
    let checks = stand_in
        .seen()
        .iter()
        .filter(|r| r.body.contains("legacyModsByDomain"))
        .count();
    assert_eq!(checks, 1, "the relaunch and the rescan asked Nexus nothing");
}

/// Labels are kept like verdicts: a check that runs while a linked package
/// is out of the mods folder (moved away for a while, or the first scan of a
/// launch still running with no catalog cache) does not ask about its mod,
/// and must not forget what an earlier check heard of it.
#[test]
fn labels_outlive_a_check_made_while_their_package_was_out_of_the_library() {
    let stand_in = StandIn::start(|request| {
        if request.body.contains("legacyModsByDomain") {
            return Answer::json(json!({ "data": { "legacyModsByDomain": {
                "totalCount": 1,
                "nodes": [{ "modId": 1234, "name": "Pawn Banter Plus", "version": "1",
                            "updatedAt": "2026-10-15T09:00:00Z", "status": "published",
                            "category": "Utilities", "tags": [{ "name": "Utilities for Players" }],
                            "adultContent": false }]
            }}}));
        }
        if request.body.contains("modFiles") {
            return Answer::json(json!({ "data": {
                "m1234": [file(501, "Ab3dE5fG7", "1", "MAIN", BANTER_UPLOADED_S, 1)]
            }}));
        }
        Answer {
            status: 404,
            headers: Vec::new(),
            body: json!({}),
        }
    });
    let bench = Bench::new();
    let bench_config = || {
        let mut config = bench.config(DataDirs::under(&bench.data_home()));
        config.nexus_endpoints = Some((stand_in.graphql_url(), stand_in.v1_base()));
        config
    };
    let (state, _) = bench.open(bench_config());
    assert!(state.scan_blocking());
    let banter_id = package_id(&state, support::BANTER_ZIP);
    let verdict = |state: &AppState| {
        nexus::get_updates_impl(state)
            .rows
            .into_iter()
            .find(|row| row.package_id == banter_id)
            .map(|row| row.state)
    };

    // The banter is linked by its Nexus file name; one check labels it.
    nexus::check_updates_impl(&state).unwrap();
    let labeled = nexus::get_labels_impl(&state);
    assert_eq!(labeled.categories, vec![count("Utilities", 1)]);
    assert_eq!(labeled.tags, vec![count("Utilities for Players", 1)]);
    assert_eq!(labeled.unlabeled_packages, 2, "the cloaks and the lantern");
    assert_eq!(verdict(&state), Some(UpdateStateView::Current));

    // It goes out of the mods folder, and a check runs meanwhile: the shelf
    // holds no linked package, so this check asks Nexus about nothing.
    let away = bench.dir.path().join(support::BANTER_ZIP);
    std::fs::rename(bench.mods.join(support::BANTER_ZIP), &away).unwrap();
    assert!(state.scan_blocking());
    nexus::check_updates_impl(&state).unwrap();
    // It comes back and is scanned again.
    std::fs::rename(&away, bench.mods.join(support::BANTER_ZIP)).unwrap();
    assert!(state.scan_blocking());
    assert_eq!(
        nexus::get_labels_impl(&state),
        labeled,
        "the labels are the banter's as long as its link stands"
    );
    assert_eq!(
        names(&by_labels(&state, &["Utilities"], &[])),
        vec!["Pawn Banter"]
    );
    assert_eq!(verdict(&state), Some(UpdateStateView::Current));

    // nexus.json kept them for the next launch…
    drop(state);
    let (again, _) = bench.open(bench_config());
    assert_eq!(nexus::get_labels_impl(&again), labeled);

    // …and a check on an empty shelf (no catalog cache, the launch's first
    // scan not done yet) forgets nothing either.
    drop(again);
    std::fs::remove_file(DataDirs::under(&bench.data_home()).catalog_cache_file()).unwrap();
    let (cold, _) = bench.open(bench_config());
    assert!(cold.shelf().packages().is_empty());
    nexus::check_updates_impl(&cold).unwrap();
    assert!(cold.scan_blocking());
    assert_eq!(nexus::get_labels_impl(&cold), labeled);
    assert_eq!(verdict(&cold), Some(UpdateStateView::Current));
}

// ---------------------------------------------------------------------------
// Find on Nexus (v0.2): by name, for what MD5 cannot identify
// ---------------------------------------------------------------------------

/// The unpacked folder of the Find tests: no archive to hash, no Nexus file
/// name, one option bundled under the collection it came from.
const UNPACKED: &str = "VernworthCloaks_Unpacked_v3";
const TRIM_MESH: &str = "natives/stm/appsystem/cloak/ermine_trim.mesh.260421070";

fn unpack_folder(bench: &Bench, name: &str, modinfo: &str) {
    let folder = bench.mods.join(name);
    let mesh = folder.join(TRIM_MESH);
    std::fs::create_dir_all(mesh.parent().unwrap()).unwrap();
    std::fs::write(folder.join("modinfo.ini"), modinfo).unwrap();
    std::fs::write(mesh, b"ermine trim mesh bytes").unwrap();
}

fn unpack_the_cloaks(bench: &Bench) {
    unpack_folder(
        bench,
        UNPACKED,
        "name=Ermine Trim\r\nNameAsBundle=Vernworth Cloaks Collection\r\nversion=3\r\n",
    );
}

/// What a find request asked: its terms and its `nameStemmed` op. `None`
/// for any other request.
fn find_asked(request: &Seen) -> Option<(String, String)> {
    if !request.body.contains("RiftstoneFindMods") {
        return None;
    }
    let body: Value = serde_json::from_str(&request.body).unwrap();
    let query = body["query"].as_str().unwrap();
    for word in ["Vernworth", "Lantern", "Pawn"] {
        assert!(
            !query.contains(word),
            "the terms are never query text: {query}"
        );
    }
    let op = if query.contains("op: WILDCARD") {
        "WILDCARD"
    } else {
        "MATCHES"
    };
    let terms = body["variables"]["terms"].as_str().unwrap().to_string();
    Some((terms, op.to_string()))
}

/// Every find the stand-in heard, in order.
fn finds(stand_in: &StandIn) -> Vec<(String, String)> {
    stand_in.seen().iter().filter_map(find_asked).collect()
}

fn asked(terms: &str, op: &str) -> (String, String) {
    (terms.to_string(), op.to_string())
}

/// A find answer holding these mods.
fn found(mods: &[(u64, &str)]) -> Answer {
    let nodes: Vec<Value> = mods
        .iter()
        .map(|(id, name)| {
            json!({ "modId": id, "name": name, "version": "2", "author": "Vern",
                    "uploader": { "name": "VernUploads" }, "category": "Armour",
                    "modCategory": { "name": "Armour" }, "adultContent": false,
                    "downloads": 640, "endorsements": 31,
                    "summary": "Cloaks for the cold roads   north of Vernworth.",
                    "updatedAt": "2026-10-14T14:23:50Z", "game": { "domainName": "dragonsdogma2" } })
        })
        .collect();
    Answer::json(json!({ "data": { "mods": { "totalCount": nodes.len(), "nodes": nodes } } }))
}

fn not_there() -> Answer {
    Answer {
        status: 404,
        headers: Vec::new(),
        body: json!({}),
    }
}

fn candidate_ids(findings: &riftstone_lib::dto::NexusFindings) -> Vec<u64> {
    findings.candidates.iter().map(|c| c.mod_id).collect()
}

#[test]
fn find_on_nexus_by_name_then_a_pick_links_and_a_check_labels_an_unpacked_folder() {
    let stand_in = StandIn::start(|request| {
        if let Some((terms, op)) = find_asked(request) {
            return match (terms.as_str(), op.as_str()) {
                ("Vernworth Cloaks Collection", "WILDCARD") => found(&[
                    (5150, "Vernworth Cloaks Collection"),
                    (6006, "Vernworth Cloaks Collection - Hoods"),
                ]),
                _ => found(&[]),
            };
        }
        if request.body.contains("legacyModsByDomain") {
            return Answer::json(json!({ "data": { "legacyModsByDomain": {
                "totalCount": 2,
                "nodes": [
                    { "modId": 1234, "name": "Pawn Banter Plus", "version": "1",
                      "updatedAt": "2026-10-15T09:00:00Z", "status": "published",
                      "category": "Utilities", "tags": [{ "name": "Utilities for Players" }],
                      "adultContent": false },
                    { "modId": 5150, "name": "Vernworth Cloaks Collection", "version": "2",
                      "updatedAt": "2026-10-15T09:00:00Z", "status": "published",
                      "category": "Armour",
                      "tags": [{ "name": "Cloaks" }, { "name": "Lore-Friendly" }],
                      "adultContent": false }
                ]
            }}}));
        }
        if request.body.contains("modFiles") {
            return Answer::json(json!({ "data": {
                "m1234": [file(501, "Ab3dE5fG7", "1", "MAIN", BANTER_UPLOADED_S, 1)],
                "m5150": [file(777, "Rk4vLmN2Q", "2", "MAIN", 1_789_500_000, 1)]
            }}));
        }
        not_there()
    });
    let bench = Bench::new();
    unpack_the_cloaks(&bench);
    let mut config = bench.config(DataDirs::under(&bench.data_home()));
    config.nexus_endpoints = Some((stand_in.graphql_url(), stand_in.v1_base()));
    let (state, events) = bench.open(config);
    assert!(state.scan_blocking());
    let folder = package_id(&state, UNPACKED);

    // What MD5 cannot identify: a folder has no archive to hash.
    let md5 = nexus::identify_packages_impl(&state, std::slice::from_ref(&folder)).unwrap();
    assert!(!md5[0].matched);
    assert!(stand_in.seen().is_empty(), "nothing to hash, nothing asked");

    let rows_before = nexus::get_updates_impl(&state);
    let labels_before = nexus::get_labels_impl(&state);
    assert_eq!(labels_before.unlabeled_packages, 4);
    events.clear();

    // -- Find: the folder's own name finds nothing, its bundle does ----------
    let findings = nexus::find_on_nexus_impl(&state, &folder).unwrap();
    assert_eq!(
        finds(&stand_in),
        vec![
            asked("Vernworth Cloaks Unpacked", "MATCHES"),
            asked("Vernworth Cloaks Unpacked", "WILDCARD"),
            asked("Vernworth Cloaks Collection", "MATCHES"),
            asked("Vernworth Cloaks Collection", "WILDCARD"),
        ],
        "the name, then the first option's bundle: four POSTs at most"
    );
    assert_eq!(findings.package_id, folder);
    assert_eq!(
        findings.terms, "Vernworth Cloaks Collection",
        "the terms answered last"
    );
    assert_eq!(candidate_ids(&findings), vec![5150, 6006], "Nexus's order");
    let cloaks = &findings.candidates[0];
    assert_eq!(cloaks.name, "Vernworth Cloaks Collection");
    assert_eq!(
        cloaks.page_url,
        "https://www.nexusmods.com/dragonsdogma2/mods/5150"
    );
    assert_eq!(cloaks.linked_packages, 0, "nothing links to it yet");
    assert_eq!(
        (cloaks.author.as_deref(), cloaks.category.as_deref()),
        (Some("Vern"), Some("Armour"))
    );
    assert_eq!(
        cloaks.summary.as_deref(),
        Some("Cloaks for the cold roads north of Vernworth.")
    );
    assert_eq!(cloaks.updated_ms, Some(1_791_987_830_000));

    // -- a find links nothing and tells the window nothing -------------------
    assert_eq!(nexus::get_updates_impl(&state), rows_before);
    assert_eq!(nexus::get_labels_impl(&state), labels_before);
    assert!(events.named(UPDATES_CHANGED).is_empty());
    assert!(
        stand_in.seen().iter().all(|r| find_asked(r).is_some()),
        "a find sends finds only"
    );

    // -- the player picks the cloaks: the window links by mod id and checks --
    let row = nexus::link_package_impl(&state, &folder, &cloaks.mod_id.to_string()).unwrap();
    let link = row.link.unwrap();
    assert_eq!((link.mod_id, link.source), (5150, LinkSource::Manual));
    nexus::check_updates_impl(&state).unwrap();

    // The labels arrive in search and in get_labels.
    assert_eq!(
        names(&by_labels(&state, &["armour"], &[])),
        vec!["Ermine Trim"]
    );
    let trim = by_labels(&state, &[], &["Cloaks", "Lore-Friendly"]);
    assert_eq!(names(&trim), vec!["Ermine Trim"]);
    assert_eq!(trim[0].nexus_category.as_deref(), Some("Armour"));
    assert!(!trim[0].adult);
    let facets = nexus::get_labels_impl(&state);
    assert!(
        facets.categories.contains(&count("Armour", 1)),
        "{facets:?}"
    );
    assert!(facets.tags.contains(&count("Cloaks", 1)), "{facets:?}");
    assert_eq!(
        facets.unlabeled_packages, 2,
        "the folder and the banter are labeled; the two zips are not"
    );
    // The check judged the new link. A link by mod id carries no file
    // identity (no sqid, file id, upload time or version), so the check
    // knows the mod and its labels, and cannot tell which file this is.
    let verdict = nexus::get_updates_impl(&state)
        .rows
        .into_iter()
        .find(|r| r.package_id == folder)
        .unwrap();
    assert!(verdict.checked_ms.is_some());
    assert_eq!(
        verdict.mod_name.as_deref(),
        Some("Vernworth Cloaks Collection")
    );
    assert_eq!(verdict.state, UpdateStateView::Unpinned);

    // -- a candidate counts the packages linked to its mod right now ---------
    let zip = package_id(&state, "Vernworth Cloaks Collection.zip");
    nexus::link_package_impl(
        &state,
        &zip,
        "https://www.nexusmods.com/dragonsdogma2/mods/5150",
    )
    .unwrap();
    let again = nexus::find_on_nexus_impl(&state, &folder).unwrap();
    assert_eq!(
        again
            .candidates
            .iter()
            .map(|c| (c.mod_id, c.linked_packages))
            .collect::<Vec<_>>(),
        vec![(5150, 2), (6006, 0)]
    );
}

#[test]
fn a_repack_found_by_its_name_at_once_asks_nothing_more() {
    let stand_in = StandIn::start(|request| {
        if let Some((terms, op)) = find_asked(request) {
            return if (terms.as_str(), op.as_str()) == ("Lantern Glow Recolor", "MATCHES") {
                // More than a find keeps: Nexus's first five stand.
                found(&[
                    (2207, "Lantern Glow Recolor"),
                    (4242, "Lantern Glow - Ember Red"),
                    (11, "Lantern Glow 11"),
                    (12, "Lantern Glow 12"),
                    (13, "Lantern Glow 13"),
                    (14, "Lantern Glow 14"),
                    (15, "Lantern Glow 15"),
                ])
            } else {
                found(&[])
            };
        }
        if request.body.contains("fileHashes") {
            // A repack: its bytes are new, Nexus knows no file with its MD5.
            return Answer::json(json!({ "data": { "fileHashes": [] } }));
        }
        not_there()
    });
    let bench = Bench::new();
    let mut config = bench.config(DataDirs::under(&bench.data_home()));
    config.nexus_endpoints = Some((stand_in.graphql_url(), stand_in.v1_base()));
    let (state, _) = bench.open(config);
    assert!(state.scan_blocking());
    let lantern = package_id(&state, "Lantern Glow Recolor.zip");
    let md5 = nexus::identify_packages_impl(&state, std::slice::from_ref(&lantern)).unwrap();
    assert!(!md5[0].matched, "MD5 cannot identify a repack");

    let findings = nexus::find_on_nexus_impl(&state, &lantern).unwrap();
    assert_eq!(findings.terms, "Lantern Glow Recolor");
    assert_eq!(
        finds(&stand_in),
        vec![asked("Lantern Glow Recolor", "MATCHES")],
        "a hit on the first ask is the only POST"
    );
    assert_eq!(
        candidate_ids(&findings),
        vec![2207, 4242, 11, 12, 13],
        "at most five, in Nexus's order"
    );
}

#[test]
fn a_folder_named_like_nothing_asks_by_its_first_options_names() {
    // `DD2_v1.2` holds nothing to search for, so the find asks with the
    // folder's modinfo: its bundle first, then the option's own name.
    let stand_in = StandIn::start(|request| {
        if let Some((terms, op)) = find_asked(request) {
            return if (terms.as_str(), op.as_str()) == ("Ember Red", "MATCHES") {
                found(&[(4242, "Lantern Glow - Ember Red")])
            } else {
                found(&[])
            };
        }
        not_there()
    });
    let bench = Bench::new();
    unpack_folder(
        &bench,
        "DD2_v1.2",
        "name=Ember Red\r\nNameAsBundle=Lantern Glow Recolor\r\n",
    );
    let mut config = bench.config(DataDirs::under(&bench.data_home()));
    config.nexus_endpoints = Some((stand_in.graphql_url(), stand_in.v1_base()));
    let (state, events) = bench.open(config);
    assert!(state.scan_blocking());
    let folder = package_id(&state, "DD2_v1.2");
    events.clear();

    let findings = nexus::find_on_nexus_impl(&state, &folder).unwrap();
    assert_eq!(
        finds(&stand_in),
        vec![
            asked("Lantern Glow Recolor", "MATCHES"),
            asked("Lantern Glow Recolor", "WILDCARD"),
            asked("Ember Red", "MATCHES"),
        ],
        "the bundle, then the option's name; never the folder's own name"
    );
    assert_eq!(findings.terms, "Ember Red", "the terms answered last");
    assert_eq!(candidate_ids(&findings), vec![4242]);
    assert!(events.named(UPDATES_CHANGED).is_empty(), "a find only asks");
}

#[test]
fn a_find_says_when_it_could_not_search_or_could_not_hear() {
    let stand_in = StandIn::start(|request| {
        if let Some((terms, _)) = find_asked(request) {
            return match terms.as_str() {
                "Lantern Glow Recolor" => Answer::json(json!({
                    "data": { "mods": { "totalCount": 0, "nodes": [] } },
                    "errors": [{ "message": "Search backend unavailable" }]
                })),
                "Pawn Banter" => Answer {
                    status: 429,
                    headers: vec![("Retry-After".into(), "30".into())],
                    body: json!({}),
                },
                _ => found(&[]),
            };
        }
        not_there()
    });
    let bench = Bench::new();
    unpack_the_cloaks(&bench);
    // A folder whose name holds nothing to search for, nor its option's: a
    // modinfo without a `name=` names the option after the folder.
    unpack_folder(&bench, "DD2_v1.2", "version=1.2\r\n");
    let mut config = bench.config(DataDirs::under(&bench.data_home()));
    config.nexus_endpoints = Some((stand_in.graphql_url(), stand_in.v1_base()));
    let (state, _) = bench.open(config);
    assert!(state.scan_blocking());

    // -- nothing searchable: an answer, and not a word sent -------------------
    let silent = package_id(&state, "DD2_v1.2");
    let nothing = nexus::find_on_nexus_impl(&state, &silent).unwrap();
    assert_eq!(nothing.package_id, silent);
    assert_eq!(nothing.terms, "");
    assert!(nothing.candidates.is_empty());
    assert!(stand_in.seen().is_empty(), "nothing was sent");

    // -- an unknown package --------------------------------------------------
    assert_eq!(
        nexus::find_on_nexus_impl(&state, "0000000000000000").unwrap_err(),
        rift_core::RiftError::EntryNotFound {
            entry: "0000000000000000".into()
        }
    );
    assert!(stand_in.seen().is_empty());

    // -- both finds come up empty: the answer names the terms sent last ------
    let folder = package_id(&state, UNPACKED);
    let empty = nexus::find_on_nexus_impl(&state, &folder).unwrap();
    assert_eq!(empty.terms, "Vernworth Cloaks Collection");
    assert!(empty.candidates.is_empty());
    assert_eq!(finds(&stand_in).len(), 4);

    // -- a search that failed is an error, never "nothing found" -------------
    let lantern = package_id(&state, "Lantern Glow Recolor.zip");
    match nexus::find_on_nexus_impl(&state, &lantern) {
        Err(rift_core::RiftError::Nexus { detail, .. }) => {
            assert!(detail.contains("Search backend unavailable"), "{detail}")
        }
        other => panic!("expected a Nexus error, got {other:?}"),
    }
    assert_eq!(finds(&stand_in).len(), 5, "no retry after a refusal");

    // -- slowed down: the rate limit speaks, and the next find waits ---------
    let banter = package_id(&state, support::BANTER_ZIP);
    match nexus::find_on_nexus_impl(&state, &banter) {
        Err(rift_core::RiftError::Nexus { reason, .. }) => {
            assert_eq!(reason, Some(rift_core::NexusReason::RateLimited))
        }
        other => panic!("expected the rate limit, got {other:?}"),
    }
    assert!(matches!(
        nexus::find_on_nexus_impl(&state, &folder),
        Err(rift_core::RiftError::Nexus { .. })
    ));
    assert_eq!(
        finds(&stand_in).len(),
        6,
        "the back-off held the last find home"
    );
}
