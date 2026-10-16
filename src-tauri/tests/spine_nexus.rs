//! The spine's Nexus side against a stand-in on a loopback port: the
//! filename link of a 2026 download is pinned by its sqid and judged within
//! its file group; a pasted URL links by hand; an MD5 lookup links exactly;
//! the API key is validated, stored in the (in-memory) credential store, and
//! its quota surfaces; unlinking silences the catalog's guess.

mod support;

use serde_json::json;

use riftstone_lib::commands::{library, nexus};
use riftstone_lib::dto::{
    LinkSource, SearchFilters, SearchQuery, UpdateBadge, UpdateStateView, UpdatesPhase,
};
use riftstone_lib::events::{UPDATES_CHANGED, UPDATES_PROGRESS};
use riftstone_lib::persist::DataDirs;
use support::{Answer, Bench, StandIn, BANTER_UPLOADED_S};

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
