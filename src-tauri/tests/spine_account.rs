//! The key's account and its extras (v0.3) against a loopback stand-in for
//! Nexus: the stored key is validated at the wake on a thread of its own,
//! without waiting for the first scan (no quota spent), and its account
//! shows; a new key keeps the account
//! it validated; a refused key reads `rejected` and Nexus out of reach reads
//! `unchecked`; the update check never waits on a spent v1 quota; tracked
//! mods and endorsements round-trip through the session cache; browsing and
//! a mod's files say what the library holds. Every name, id and key here is
//! invented.

mod support;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use riftstone_lib::commands::browse::{browse_nexus_impl, get_mod_files_impl};
use riftstone_lib::commands::nexus;
use riftstone_lib::commands::standings::{
    get_nexus_standings_impl, set_endorsement_impl, set_tracked_impl,
};
use riftstone_lib::dto::{
    EndorseStatus, NexusBrowseQuery, NexusBrowseSort, NexusFileCategory, NexusKeyStanding,
};
use riftstone_lib::events::{LIBRARY_CHANGED, SCAN_PROGRESS, UPDATES_CHANGED};
use riftstone_lib::keys::MemoryKeyStore;
use riftstone_lib::persist::DataDirs;
use riftstone_lib::state::AppState;
use support::{wait_for, Answer, Bench, GatedEvents, Seen, StandIn, BANTER_UPLOADED_S};

const KEY: &str = "invented-personal-key-0042";
const EMAIL: &str = "arisen@vernworth.invalid";

/// How the stand-in's validate.json answers.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Validate {
    Premium,
    Free,
    Rejected,
    /// Free, with the v1 quota spent until an hour from now.
    Spent,
}

fn status(code: u16) -> Answer {
    Answer {
        status: code,
        headers: Vec::new(),
        body: json!({}),
    }
}

/// The mods the stand-in's v2 lookup knows.
fn known_mod(mod_id: u64) -> Option<Value> {
    let (name, version, adult) = match mod_id {
        1234 => ("Pawn Banter Plus", "1", false),
        2207 => ("Lantern Glow Recolor", "2.2", false),
        4242 => ("Vernworth Cloaks Collection", "3", true),
        _ => return None,
    };
    Some(json!({ "modId": mod_id, "name": name, "version": version,
                 "updatedAt": "2026-10-15T09:00:00Z", "status": "published",
                 "category": "Visuals", "tags": [], "adultContent": adult }))
}

fn nexus_api(mode: Arc<Mutex<Validate>>) -> impl Fn(&Seen) -> Answer + Send + 'static {
    move |request| {
        let path = request.path.split('?').next().unwrap_or_default();
        if path.ends_with("/users/validate.json") {
            let mode = *mode.lock().unwrap();
            if mode == Validate::Rejected {
                return status(401);
            }
            let mut answer = Answer::json(json!({
                "user_id": 4242, "key": KEY, "name": "Arisen of Vernworth", "email": EMAIL,
                "is_premium": mode == Validate::Premium, "is_supporter": mode == Validate::Premium
            }));
            let (left, reset) = if mode == Validate::Spent {
                let reset = chrono::Utc::now() + chrono::Duration::hours(1);
                ("0", reset.to_rfc3339())
            } else {
                ("499", "2026-10-16T13:00:00+00:00".to_string())
            };
            answer.headers = vec![
                ("X-RL-Daily-Limit".into(), "20000".into()),
                ("X-RL-Daily-Remaining".into(), left.into()),
                ("X-RL-Hourly-Limit".into(), "500".into()),
                ("X-RL-Hourly-Remaining".into(), left.into()),
                ("X-RL-Hourly-Reset".into(), reset),
            ];
            return answer;
        }
        if path.ends_with("/user/tracked_mods.json") {
            return match request.method.as_str() {
                "GET" => Answer::json(json!([
                    { "mod_id": 2207, "domain_name": "dragonsdogma2" },
                    { "mod_id": 1234, "domain_name": "dragonsdogma2" },
                    { "mod_id": 77, "domain_name": "skyrimspecialedition" }
                ])),
                "POST" => Answer {
                    status: 201,
                    headers: Vec::new(),
                    body: json!({ "message": "Tracked" }),
                },
                _ => Answer::json(json!({ "message": "Untracked" })),
            };
        }
        if path.ends_with("/user/endorsements.json") {
            return Answer::json(json!([
                { "mod_id": 1234, "domain_name": "dragonsdogma2", "date": 1_791_000_000, "version": "1", "status": "Endorsed" },
                { "mod_id": 2207, "domain_name": "dragonsdogma2", "date": 1_791_000_000, "version": "2.2", "status": "Undecided" }
            ]));
        }
        if path.ends_with("/mods/2207/endorse.json") {
            return Answer {
                status: 403,
                headers: Vec::new(),
                body: json!({ "message": "TOO_SOON_AFTER_DOWNLOAD" }),
            };
        }
        if path.ends_with("/abstain.json") || path.ends_with("/endorse.json") {
            return Answer::json(
                json!({ "message": "Updated to: Abstained", "status": "Abstained" }),
            );
        }
        if request.body.contains("legacyModsByDomain") {
            let asked: Value = serde_json::from_str(&request.body).unwrap();
            let nodes: Vec<Value> = asked["variables"]["ids"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|id| known_mod(id["modId"].as_u64().unwrap()))
                .collect();
            return Answer::json(json!({ "data": { "legacyModsByDomain": {
                "totalCount": nodes.len(), "nodes": nodes
            }}}));
        }
        if request.body.contains("RiftstoneModFiles") {
            return Answer::json(json!({ "data": {
                "m1234": [{ "fileId": 501, "groupId": 9, "sqid": "Ab3dE5fG7", "name": "Pawn Banter",
                            "version": "1", "category": "MAIN", "date": BANTER_UPLOADED_S,
                            "primary": 1, "sizeInBytes": "100" }]
            }}));
        }
        if request.body.contains("RiftstoneModFileDetails") {
            if request.body.contains("modId: 1234") {
                return Answer::json(json!({ "data": { "modFiles": [
                    { "fileId": 501, "groupId": 9, "sqid": "Ab3dE5fG7", "name": "Pawn Banter",
                      "version": "1", "category": "ARCHIVED", "date": BANTER_UPLOADED_S, "primary": 0,
                      "sizeInBytes": "100", "description": "  The first   quieter pawns. ",
                      "changelogText": ["First release.", "  "] },
                    { "fileId": 502, "groupId": 9, "sqid": "Zz9yX8wV7", "name": "Pawn Banter",
                      "version": "2", "category": "MAIN", "date": BANTER_UPLOADED_S + 86_400, "primary": 1,
                      "sizeInBytes": "120", "description": "Quieter pawns, now in camp too.",
                      "changelogText": ["Camp banter is quieter.", "Fixed a long pause."] },
                    { "fileId": 503, "groupId": 10, "sqid": "Qq1wE2rT3", "name": "Pawn Banter - Silent Camp",
                      "version": "1", "category": "OPTIONAL", "date": BANTER_UPLOADED_S + 3_600, "primary": 0,
                      "sizeInBytes": null, "description": null, "changelogText": [] }
                ]}}));
            }
            return Answer::json(json!({ "data": { "modFiles": [] } }));
        }
        if request.body.contains("RiftstoneBrowseMods") {
            return Answer::json(json!({ "data": { "mods": { "totalCount": 41, "nodes": [
                { "modId": 1234, "name": "Pawn Banter Plus", "version": "2", "author": "Vern",
                  "uploader": { "name": "Vern" }, "category": "Audio", "modCategory": { "name": "Audio" },
                  "adultContent": false, "downloads": 900, "endorsements": 40,
                  "summary": "Pawns talk less.", "updatedAt": "2026-10-15T09:00:00Z",
                  "game": { "domainName": "dragonsdogma2" } },
                { "modId": 5150, "name": "Vernworth Cloaks Collection", "version": "3", "author": null,
                  "uploader": { "name": "Weaver of Melve" }, "category": "Armour", "modCategory": null,
                  "adultContent": false, "downloads": 120, "endorsements": 9,
                  "summary": "Cloaks for the cold roads.", "updatedAt": "2026-10-01T09:00:00Z",
                  "game": { "domainName": "dragonsdogma2" } }
            ] } } }));
        }
        status(404)
    }
}

fn stand_in(mode: Validate) -> (StandIn, Arc<Mutex<Validate>>) {
    let mode = Arc::new(Mutex::new(mode));
    (StandIn::start(nexus_api(Arc::clone(&mode))), mode)
}

fn count(api: &StandIn, what: &str) -> usize {
    api.seen()
        .iter()
        .filter(|request| request.path.contains(what))
        .count()
}

fn summary(state: &AppState) -> riftstone_lib::dto::NexusSummary {
    nexus::summary(state, &state.shelf())
}

#[test]
fn the_stored_key_is_validated_at_the_wake_without_waiting_for_the_scan() {
    let (api, _) = stand_in(Validate::Premium);
    let bench = Bench::new();
    // The first scan stops at its first package until the test lets it go.
    let events = Arc::new(GatedEvents::closed_on(SCAN_PROGRESS));
    let state = AppState::open(
        bench.config_at(&api),
        events.clone(),
        Box::new(MemoryKeyStore::holding(KEY)),
    );
    let before = summary(&state);
    assert!(before.has_api_key);
    assert_eq!(before.key, NexusKeyStanding::Unchecked, "not validated yet");
    assert_eq!(before.account, None);
    assert_eq!(count(&api, "validate.json"), 0, "nothing is asked at open");

    state.wake();
    wait_for(
        Duration::from_secs(20),
        "the account thread validates the key while the scan is held",
        || summary(&state).key == NexusKeyStanding::Connected,
    );
    assert!(
        events.recorded.named(LIBRARY_CHANGED).is_empty(),
        "the validation never waited for the library"
    );
    let after = summary(&state);
    let account = after.account.clone().expect("connected: the account shows");
    assert_eq!(account.name, "Arisen of Vernworth");
    assert!(account.premium && account.supporter);
    assert!(account.checked_ms > 0);
    let wire = serde_json::to_string(&after).unwrap();
    assert!(!wire.contains(KEY) && !wire.contains(EMAIL), "{wire}");
    assert!(
        !events.recorded.named(UPDATES_CHANGED).is_empty(),
        "the window heard the account"
    );
    // validate.json spends no quota, and still reports it.
    let quota = nexus::get_nexus_quota_impl(&state).expect("the launch validation shows the quota");
    assert_eq!(quota.hourly_remaining, 499);

    events.open();
    wait_for(Duration::from_secs(20), "the first scan ends", || {
        !events.recorded.named(LIBRARY_CHANGED).is_empty()
    });
}

#[test]
fn a_new_key_keeps_the_account_it_validated_and_a_cleared_key_forgets_it() {
    let (api, _) = stand_in(Validate::Free);
    let bench = Bench::new();
    let (state, events) = bench.open(bench.config_at(&api));
    assert_eq!(summary(&state).key, NexusKeyStanding::None);

    let keyed = nexus::set_nexus_key_impl(&state, Some(KEY)).unwrap();
    assert_eq!(keyed.key, NexusKeyStanding::Connected);
    let account = keyed.account.expect("the account it validated");
    assert_eq!(account.name, "Arisen of Vernworth");
    assert!(!account.premium);
    assert_eq!(count(&api, "validate.json"), 1, "validated once, kept");
    assert_eq!(summary(&state).key, NexusKeyStanding::Connected);
    assert_eq!(events.named(UPDATES_CHANGED).len(), 1);

    let cleared = nexus::set_nexus_key_impl(&state, None).unwrap();
    assert_eq!(cleared.key, NexusKeyStanding::None);
    assert_eq!(cleared.account, None);
    assert!(!cleared.has_api_key);
}

#[test]
fn a_refused_key_reads_rejected_and_nexus_out_of_reach_reads_unchecked() {
    let (api, mode) = stand_in(Validate::Rejected);
    let bench = Bench::new();
    let (state, _) = bench.open_keyed(bench.config_at(&api), KEY);
    let refused = nexus::refresh_nexus_account_impl(&state).expect("a refused key is no error");
    assert_eq!(refused.key, NexusKeyStanding::Rejected);
    assert_eq!(refused.account, None);
    assert!(
        refused.has_api_key,
        "the key stays stored: the player replaces it"
    );

    *mode.lock().unwrap() = Validate::Premium;
    let back = nexus::refresh_nexus_account_impl(&state).unwrap();
    assert_eq!(back.key, NexusKeyStanding::Connected);

    // Nothing listens: offline, an error with its reason, and unchecked.
    let bench = Bench::new();
    let (offline, _) = bench.open_keyed(bench.config(DataDirs::under(&bench.data_home())), KEY);
    let err = nexus::refresh_nexus_account_impl(&offline).unwrap_err();
    let wire = serde_json::to_value(&err).unwrap();
    assert_eq!(wire["kind"], "nexus");
    assert_eq!(wire["detail"]["reason"], "offline", "{wire}");
    assert_eq!(summary(&offline).key, NexusKeyStanding::Unchecked);
}

#[test]
fn the_update_check_never_waits_on_a_spent_v1_quota() {
    let (api, _) = stand_in(Validate::Spent);
    let bench = Bench::new();
    let (state, _) = bench.open_keyed(bench.config_at(&api), KEY);
    assert!(state.scan_blocking());
    nexus::refresh_nexus_account_impl(&state).unwrap();
    let quota = nexus::get_nexus_quota_impl(&state).unwrap();
    assert_eq!((quota.hourly_remaining, quota.daily_remaining), (0, 0));

    // Anonymous v2: the check runs whatever the v1 budget says.
    let checked = nexus::check_updates_impl(&state).expect("the check is anonymous");
    assert!(checked.last_check_ms.is_some());
    // A metered call is refused at home, without a request.
    let err = get_nexus_standings_impl(&state, true).unwrap_err();
    assert_eq!(
        serde_json::to_value(&err).unwrap()["detail"]["reason"],
        "rateLimited"
    );
    assert_eq!(count(&api, "tracked_mods.json"), 0);
}

fn tracked_ids(standings: &riftstone_lib::dto::NexusStandings) -> Vec<u64> {
    standings.tracked.iter().map(|card| card.mod_id).collect()
}

#[test]
fn tracked_mods_and_endorsements_round_trip_through_the_session_cache() {
    let (api, _) = stand_in(Validate::Free);
    let bench = Bench::new();
    let (state, events) = bench.open_keyed(bench.config_at(&api), KEY);
    assert!(state.scan_blocking());

    let first = get_nexus_standings_impl(&state, false).unwrap();
    assert_eq!(
        tracked_ids(&first),
        vec![2207, 1234],
        "this game's, in Nexus's order"
    );
    let banter = first
        .tracked
        .iter()
        .find(|card| card.mod_id == 1234)
        .unwrap();
    assert_eq!(banter.name, "Pawn Banter Plus");
    assert_eq!(banter.linked_packages, 1, "the Nexus download links to it");
    assert_eq!(
        banter.page_url,
        "https://www.nexusmods.com/dragonsdogma2/mods/1234"
    );
    let endorsements: Vec<(u64, EndorseStatus)> = first
        .endorsements
        .iter()
        .map(|e| (e.mod_id, e.status))
        .collect();
    assert_eq!(
        endorsements,
        vec![(1234, EndorseStatus::Endorsed)],
        "undecided is absent"
    );
    assert!(first.checked_ms > 0);
    let wire = serde_json::to_value(&first).unwrap();
    assert_eq!(
        wire["endorsements"][0],
        json!({ "modId": 1234, "status": "endorsed" })
    );

    // Cached for the session: no second round of v1 calls.
    let asked = count(&api, "tracked_mods.json");
    let again = get_nexus_standings_impl(&state, false).unwrap();
    assert_eq!(again, first);
    assert_eq!(count(&api, "tracked_mods.json"), asked);

    // Untrack, then track another: patched in place.
    let after = set_tracked_impl(&state, 2207, false).unwrap();
    assert_eq!(tracked_ids(&after), vec![1234]);
    let untrack = api
        .seen()
        .into_iter()
        .find(|r| r.method == "DELETE")
        .expect("untracked on Nexus");
    assert_eq!(
        untrack.path,
        "/v1/user/tracked_mods.json?domain_name=dragonsdogma2"
    );
    assert_eq!(untrack.body, "mod_id=2207");
    assert_eq!(untrack.header("apikey"), Some(KEY));
    let after = set_tracked_impl(&state, 4242, true).unwrap();
    assert_eq!(tracked_ids(&after), vec![1234, 4242]);
    let cloaks = &after.tracked[1];
    assert_eq!(
        (cloaks.name.as_str(), cloaks.adult),
        ("Vernworth Cloaks Collection", true)
    );
    assert!(api
        .seen()
        .iter()
        .any(|r| r.method == "POST" && r.body == "mod_id=4242"));

    // Abstain, naming the version the library holds (the download's "1").
    let after = set_endorsement_impl(&state, 1234, EndorseStatus::Abstained).unwrap();
    assert_eq!(after.endorsements[0].status, EndorseStatus::Abstained);
    let abstain = api
        .seen()
        .into_iter()
        .find(|r| r.path.ends_with("/mods/1234/abstain.json"))
        .expect("abstained on Nexus");
    assert_eq!(abstain.body, "Version=1");

    // Nexus's refusal comes back with its reason; the standings stand.
    let err = set_endorsement_impl(&state, 2207, EndorseStatus::Endorsed).unwrap_err();
    assert_eq!(
        serde_json::to_value(&err).unwrap()["detail"]["reason"],
        "endorseTooSoon"
    );
    let endorse = api
        .seen()
        .into_iter()
        .find(|r| r.path.ends_with("/mods/2207/endorse.json"))
        .unwrap();
    assert_eq!(
        endorse.body, "Version=2.2",
        "no package holds it: the mod's version"
    );
    assert_eq!(
        get_nexus_standings_impl(&state, false)
            .unwrap()
            .endorsements
            .len(),
        1
    );
    assert!(!events.named(UPDATES_CHANGED).is_empty());

    // Refresh asks again.
    get_nexus_standings_impl(&state, true).unwrap();
    assert_eq!(
        count(&api, "tracked_mods.json") - asked,
        3,
        "post, delete, and the refresh's get"
    );

    // Without a key: noKey, and nothing is asked.
    let bench = Bench::new();
    let (keyless, _) = bench.open(bench.config_at(&api));
    for err in [
        get_nexus_standings_impl(&keyless, false).unwrap_err(),
        set_tracked_impl(&keyless, 1234, true).unwrap_err(),
        set_endorsement_impl(&keyless, 1234, EndorseStatus::Endorsed).unwrap_err(),
    ] {
        assert_eq!(
            serde_json::to_value(&err).unwrap()["detail"]["reason"],
            "noKey"
        );
    }
}

#[test]
fn browsing_and_a_mods_files_say_what_the_library_holds() {
    let (api, _) = stand_in(Validate::Free);
    let bench = Bench::new();
    let (state, _) = bench.open(bench.config_at(&api));
    assert!(state.scan_blocking());

    let page = browse_nexus_impl(
        &state,
        &NexusBrowseQuery {
            text: "  pawn   banter ".into(),
            sort: NexusBrowseSort::Updated,
            offset: 20,
        },
    )
    .unwrap();
    assert_eq!((page.total, page.offset), (41, 20));
    let cards: Vec<(u64, usize)> = page
        .mods
        .iter()
        .map(|m| (m.mod_id, m.linked_packages))
        .collect();
    assert_eq!(cards, vec![(1234, 1), (5150, 0)]);
    assert_eq!(page.mods[1].author.as_deref(), Some("Weaver of Melve"));
    let asked = api
        .seen()
        .into_iter()
        .find(|r| r.body.contains("RiftstoneBrowseMods"))
        .unwrap();
    let body: Value = serde_json::from_str(&asked.body).unwrap();
    assert_eq!(
        body["variables"]["text"], "pawn banter",
        "only what was typed, tidied"
    );
    let query = body["query"].as_str().unwrap();
    assert!(
        query.contains("updatedAt: { direction: DESC }") && query.contains("offset: 20"),
        "{query}"
    );
    assert_eq!(asked.header("apikey"), None, "browsing is anonymous");

    // Pin the Nexus download to its first file, then read the mod's files.
    let banter = state
        .shelf()
        .packages()
        .iter()
        .find(|p| p.display_name == "Pawn Banter")
        .unwrap()
        .id
        .0
        .clone();
    nexus::link_package_impl(
        &state,
        &banter,
        "https://www.nexusmods.com/dragonsdogma2/mods/1234?tab=files&file_id=501",
    )
    .unwrap();
    let files = get_mod_files_impl(&state, 1234).unwrap();
    assert_eq!(files.mod_name.as_deref(), Some("Pawn Banter Plus"));
    assert_eq!(
        files.page_url,
        "https://www.nexusmods.com/dragonsdogma2/mods/1234"
    );
    assert!(!files.adult);
    let order: Vec<(u64, NexusFileCategory, bool, bool)> = files
        .files
        .iter()
        .map(|f| (f.file_id, f.category, f.newest, f.in_library))
        .collect();
    assert_eq!(
        order,
        vec![
            (502, NexusFileCategory::Main, true, false),
            (503, NexusFileCategory::Optional, true, false),
            (501, NexusFileCategory::Archived, false, true),
        ],
        "live categories first; the newest of each group; the pinned file"
    );
    let main = &files.files[0];
    assert_eq!(
        main.changelog,
        vec!["Camp banter is quieter.", "Fixed a long pause."]
    );
    assert_eq!(main.size, Some(120));
    assert_eq!(main.uploaded_ms, (BANTER_UPLOADED_S + 86_400) * 1000);
    let archived = &files.files[2];
    assert_eq!(
        archived.description.as_deref(),
        Some("The first quieter pawns.")
    );
    assert_eq!(archived.changelog, vec!["First release."]);
    assert_eq!(files.files[1].size, None);
    for request in api.seen() {
        assert_eq!(
            request.header("apikey"),
            None,
            "no key was stored: {request:?}"
        );
    }

    // A mod Nexus knows nothing of.
    let err = get_mod_files_impl(&state, 31337).unwrap_err();
    assert_eq!(
        serde_json::to_value(&err).unwrap()["detail"]["reason"],
        "fileGone"
    );
}
