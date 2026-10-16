//! The client against a local stand-in for Nexus Mods.
//!
//! The canned answers in `fixtures/` are hand-written from the shapes confirmed
//! against the live API on 2026-10-16 (schema introspection plus real
//! answers): `updatedAt` RFC 3339, `sizeInBytes`/`fileSize` as JSON strings,
//! `primary` as 0/1, `version` possibly `""`, `modFile` possibly null, unknown
//! mods omitted from `legacyModsByDomain` and answered `[]` by `modFiles`, the
//! `MD5_INVALID` error shape, and (v0.2) each mod's `category`, `tags { name }`
//! (often `[]`) and nullable `adultContent`. `mods_page_without_labels.json` is
//! the v0.1 answer shape, kept to prove an answer without labels still reads.
//! Names, ids, categories and tags are invented. Nothing here touches the
//! network.

mod support;

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use rift_core::{LinkSource, NexusLink};
use rift_nexus::{
    pin_link, update_state, FileCategory, NexusClient, NexusError, UpdateState, GAME_ID,
};
use serde_json::{json, Value};
use support::{dead_address, Recorded, Reply, TestServer};

const MODS_PAGE: &str = include_str!("fixtures/mods_page.json");
const MODS_PAGE_WITHOUT_LABELS: &str = include_str!("fixtures/mods_page_without_labels.json");
const MOD_FILES: &str = include_str!("fixtures/mod_files_aliased.json");
const FILE_HASHES: &str = include_str!("fixtures/file_hashes.json");
const GRAPHQL_REFUSAL: &str = include_str!("fixtures/graphql_refusal.json");
const GRAPHQL_INVALID_MD5: &str = include_str!("fixtures/graphql_invalid_md5.json");
const V1_VALIDATE: &str = include_str!("fixtures/v1_validate.json");
const V1_FILES: &str = include_str!("fixtures/v1_files.json");

/// The mod ids an aliased `modFiles` query asks for, in query order.
fn aliased_ids(query: &str) -> Vec<u64> {
    query
        .split("modFiles(modId: ")
        .skip(1)
        .filter_map(|rest| rest.split(',').next()?.trim().parse().ok())
        .collect()
}

/// The invented category the echo stand-in files a mod under.
fn echo_category(mod_id: u64) -> &'static str {
    ["Armour", "Visuals", "Utilities"][(mod_id % 3) as usize]
}

/// Answer `legacyModsByDomain` with one labelled node per requested id,
/// reversed — Nexus does not keep request order either. Every third mod
/// carries no tags; even ids are flagged adult.
fn echo_mods(request: &Recorded) -> Reply {
    let body = request.json();
    let nodes: Vec<Value> = body["variables"]["ids"]
        .as_array()
        .expect("ids variable")
        .iter()
        .rev()
        .map(|id| {
            let mod_id = id["modId"].as_u64().unwrap();
            let tags = if mod_id % 3 == 0 {
                json!([])
            } else {
                json!([{ "name": format!("Invented Tag {mod_id}") }])
            };
            json!({ "modId": mod_id, "name": format!("Invented Mod {mod_id}"), "version": "1",
                    "updatedAt": "2026-10-14T14:23:50Z", "status": "published",
                    "category": echo_category(mod_id), "tags": tags,
                    "adultContent": mod_id % 2 == 0 })
        })
        .collect();
    Reply::json(
        json!({ "data": { "legacyModsByDomain": { "totalCount": nodes.len(), "nodes": nodes } } }),
    )
}

/// Answer an aliased `modFiles` query with one file per mod.
fn echo_mod_files(request: &Recorded) -> Reply {
    let mut data = serde_json::Map::new();
    for id in aliased_ids(&request.query()) {
        data.insert(
            format!("m{id}"),
            json!([{ "fileId": id * 10, "groupId": id, "sqid": format!("sq{id}"),
                     "name": format!("Invented File {id}"), "version": "1", "category": "MAIN",
                     "date": 1_790_000_000u64, "primary": 1, "sizeInBytes": "100" }]),
        );
    }
    Reply::json(json!({ "data": data }))
}

#[test]
fn every_request_names_the_riftstone_and_v2_never_sees_the_key() {
    let server = TestServer::start(|_| Reply::fixture(MODS_PAGE));
    let client = server
        .client()
        .with_api_key(Some("invented-personal-key".into()));
    client.mods(&[4242]).unwrap();
    let request = &server.requests()[0];
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/v2/graphql");
    assert_eq!(request.header("Application-Name"), Some("Riftstone"));
    assert_eq!(request.header("Application-Version"), Some("0.1.0"));
    let agent = request.header("User-Agent").unwrap();
    assert!(
        agent.starts_with("Riftstone/0.1.0 (Dragon's Dogma 2 mod manager; "),
        "{agent}"
    );
    assert!(request
        .header("Content-Type")
        .is_some_and(|value| value.starts_with("application/json")));
    assert_eq!(request.header("apikey"), None, "v2 is anonymous");
}

#[test]
fn four_hundred_fifty_mods_take_three_posts() {
    let server = TestServer::start(echo_mods);
    let mut ids: Vec<u64> = (1..=450).collect();
    ids.extend([7, 0, 1 << 40]); // a duplicate and two ids Nexus cannot hold
    let mods = server.client().mods(&ids).unwrap();

    let sizes: Vec<usize> = server
        .requests()
        .iter()
        .map(|request| request.json()["variables"]["ids"].as_array().unwrap().len())
        .collect();
    assert_eq!(sizes, vec![200, 200, 50]);
    for request in server.requests() {
        let body = request.json();
        assert_eq!(body["variables"]["count"], 200);
        assert!(body["variables"]["ids"]
            .as_array()
            .unwrap()
            .iter()
            .all(|id| id["gameDomain"] == "dragonsdogma2"));
    }
    assert_eq!(mods.len(), 450);
    assert!(mods
        .iter()
        .zip(1..=450u64)
        .all(|(m, id)| m.mod_id == id && m.name == format!("Invented Mod {id}")));
    // Every mod came back labelled, and the labels added no POST: still three.
    for m in &mods {
        assert_eq!(m.category.as_deref(), Some(echo_category(m.mod_id)));
        let expected: Vec<String> = if m.mod_id % 3 == 0 {
            vec![]
        } else {
            vec![format!("Invented Tag {}", m.mod_id)]
        };
        assert_eq!(m.tags, expected, "mod {}", m.mod_id);
        assert_eq!(m.adult, m.mod_id % 2 == 0, "mod {}", m.mod_id);
    }
    assert_eq!(server.request_count(), 3, "labels cost no request");
}

#[test]
fn labels_come_with_the_mods_in_the_same_post() {
    let server = TestServer::start(|_| Reply::fixture(MODS_PAGE));
    let mods = server.client().mods(&[5150, 4242, 6006]).unwrap();
    assert_eq!(server.request_count(), 1, "one POST for mods and labels");
    let query = server.requests()[0].query();
    assert!(
        query.contains("status category tags { name } adultContent"),
        "{query}"
    );

    // Category, tags and the adult flag, in Nexus's spelling and order.
    let cloaks = &mods[0];
    assert_eq!(cloaks.mod_id, 5150);
    assert_eq!(cloaks.category.as_deref(), Some("Armour"));
    assert_eq!(cloaks.tags, ["Cloaks", "Lore-Friendly"]);
    assert!(cloaks.adult);

    // An empty tag list: a category and nothing else.
    let lantern = &mods[1];
    assert_eq!(lantern.mod_id, 4242);
    assert_eq!(lantern.category.as_deref(), Some("Visuals"));
    assert!(lantern.tags.is_empty());
    assert!(!lantern.adult);

    let banners = &mods[2];
    assert_eq!(banners.tags, ["Utilities for Players"]);
    assert!(!banners.adult, "adultContent null is not flagged");
    assert!(
        !banners.is_available(),
        "a hidden mod keeps its labels and its status"
    );
}

#[test]
fn an_answer_in_the_older_shape_still_parses_unlabeled() {
    // The v0.1 answer: no category, no tags, no adultContent anywhere.
    let server = TestServer::start(|_| Reply::fixture(MODS_PAGE_WITHOUT_LABELS));
    let mods = server.client().mods(&[4242, 6006, 5150, 7777]).unwrap();
    assert_eq!(
        mods.iter().map(|m| m.mod_id).collect::<Vec<_>>(),
        vec![4242, 6006, 5150],
        "the same mods as ever, in request order"
    );
    assert_eq!(mods[0].name, "Lantern Glow Recolor");
    assert_eq!(mods[0].version.as_deref(), Some("2.1"));
    assert!(!mods[1].is_available());
    assert_eq!(mods[2].updated_ms, Some(1_791_987_830_000));
    for m in &mods {
        assert_eq!(m.category, None, "mod {}", m.mod_id);
        assert!(m.tags.is_empty(), "mod {}", m.mod_id);
        assert!(!m.adult, "mod {}", m.mod_id);
    }
    assert_eq!(server.request_count(), 1);
}

#[test]
fn a_label_that_failed_refuses_the_page_so_adult_never_reads_as_false() {
    // `adultContent` is nullable: a resolver failure nulls it and names it in
    // `errors`. Read at face value, a flagged mod would come back unflagged and
    // the window's adult veil would leave its screenshots bare.
    let server = TestServer::start(|_| {
        Reply::fixture(
            r#"{"data":{"legacyModsByDomain":{"totalCount":1,"nodes":[{"modId":5150,"name":"Vernworth Cloaks Collection","version":"1","updatedAt":"2026-10-14T14:23:50Z","status":"published","category":"Armour","tags":[],"adultContent":null}]}},
                "errors":[{"message":"adultContent could not be resolved","path":["legacyModsByDomain","nodes",0,"adultContent"]}]}"#,
        )
    });
    assert_eq!(
        server.client().mods(&[5150]),
        Err(NexusError::BadResponse(
            "adultContent could not be resolved".into()
        ))
    );
    assert_eq!(server.request_count(), 1);
}

#[test]
fn unordered_answers_are_keyed_by_mod_id() {
    let server = TestServer::start(|_| Reply::fixture(MODS_PAGE));
    // The fixture answers 5150, 4242, 6006 — and nothing for 7777.
    let mods = server.client().mods(&[4242, 6006, 5150, 7777]).unwrap();
    let ids: Vec<u64> = mods.iter().map(|m| m.mod_id).collect();
    assert_eq!(
        ids,
        vec![4242, 6006, 5150],
        "request order, unknown left out"
    );
    assert_eq!(mods[0].name, "Lantern Glow Recolor");
    assert_eq!(mods[0].version.as_deref(), Some("2.1"));
    assert_eq!(mods[1].version, None, "an empty mod version is no version");
    assert!(!mods[1].is_available(), "status hidden");
    assert_eq!(mods[2].updated_ms, Some(1_791_987_830_000));
    assert!(mods[2].is_available());
}

#[test]
fn aliased_mod_files_parse_per_mod() {
    let server = TestServer::start(|_| Reply::fixture(MOD_FILES));
    let files = server.client().mod_files(&[5150, 4242, 7777]).unwrap();
    assert_eq!(server.request_count(), 1, "three mods, one POST");
    let query = server.requests()[0].query();
    assert!(
        query.contains("m5150: modFiles(modId: 5150, gameId: 6234)"),
        "{query}"
    );
    assert!(query.contains("sizeInBytes"));

    assert_eq!(
        files.keys().copied().collect::<Vec<_>>(),
        vec![4242, 5150, 7777]
    );
    assert!(files[&7777].is_empty(), "an unknown mod answers []");
    let cloaks = &files[&5150];
    assert_eq!(
        cloaks.iter().map(|f| f.file_id).collect::<Vec<_>>(),
        vec![801, 803, 802],
        "oldest upload first"
    );
    let main = &cloaks[2];
    assert_eq!(main.category, FileCategory::Main);
    assert!(main.primary, "primary: 1");
    assert_eq!(
        main.size_bytes,
        Some(30_703_184),
        "sizeInBytes arrives as a string"
    );
    assert_eq!(main.group_id, Some(77001));
    assert_eq!(main.sqid.as_deref(), Some("hJ8kL0mN2"));
    assert_eq!(main.uploaded_ms, 1_791_987_810_000);
    let dyes = &cloaks[1];
    assert_eq!((dyes.version.as_deref(), dyes.size_bytes), (None, None));
    assert_eq!(cloaks[0].category, FileCategory::Archived);
}

#[test]
fn mod_files_go_twenty_mods_per_post() {
    let server = TestServer::start(echo_mod_files);
    let ids: Vec<u64> = (100..145).collect();
    let files = server.client().mod_files(&ids).unwrap();
    let batches: Vec<usize> = server
        .requests()
        .iter()
        .map(|request| aliased_ids(&request.query()).len())
        .collect();
    assert_eq!(batches, vec![20, 20, 5]);
    assert_eq!(files.len(), 45);
    assert_eq!(files[&144][0].file_id, 1440);
}

/// Refuse every asked mod the way GraphQL does when `modFiles` (a non-null
/// list) fails: one error per alias, each naming it, and `data` nulled.
fn refuse_every_mod(request: &Recorded) -> Reply {
    let errors: Vec<Value> = aliased_ids(&request.query())
        .into_iter()
        .map(|id| {
            json!({ "message": "Mod not available", "path": [format!("m{id}")],
                    "extensions": { "code": "MOD_UNAVAILABLE" } })
        })
        .collect();
    Reply::json(json!({ "errors": errors, "data": null }))
}

#[test]
fn a_refused_mod_is_left_out_and_the_rest_asked_again_as_one_batch() {
    // GRAPHQL_REFUSAL names m666 and nulls `data`: one bad mod took the whole
    // answer down with it.
    let server = TestServer::start(|request| {
        if aliased_ids(&request.query()).contains(&666) {
            Reply::fixture(GRAPHQL_REFUSAL)
        } else {
            echo_mod_files(request)
        }
    });
    let report = server.client().mod_files_report(&[1, 666, 2]).unwrap();
    assert_eq!(
        report.files.keys().copied().collect::<Vec<_>>(),
        vec![1, 2],
        "666 is left out"
    );
    assert_eq!(
        report.refused.get(&666),
        Some(&NexusError::BadResponse(
            "Mod not available [MOD_UNAVAILABLE]".into()
        )),
        "and the reason is kept, not only logged"
    );
    let asked: Vec<Vec<u64>> = server
        .requests()
        .iter()
        .map(|request| aliased_ids(&request.query()))
        .collect();
    assert_eq!(
        asked,
        vec![vec![1, 666, 2], vec![1, 2]],
        "the innocent mods go again as ONE batch, and 666 is not asked twice"
    );
    let files = server.client().mod_files(&[1, 666, 2]).unwrap();
    assert_eq!(files.keys().copied().collect::<Vec<_>>(), vec![1, 2]);

    // Every mod refused: the plain call errs, the report says why per mod.
    let all_refused = TestServer::start(refuse_every_mod);
    match all_refused.client().mod_files(&[665, 666]) {
        Err(NexusError::BadResponse(reason)) => {
            assert_eq!(reason, "Mod not available [MOD_UNAVAILABLE]")
        }
        other => panic!("expected BadResponse, got {other:?}"),
    }
    assert_eq!(all_refused.request_count(), 1, "nothing left to ask again");
    let report = all_refused.client().mod_files_report(&[665, 666]).unwrap();
    assert!(report.files.is_empty());
    assert_eq!(
        report.refused.keys().copied().collect::<Vec<_>>(),
        vec![665, 666]
    );
}

#[test]
fn a_refusal_in_one_batch_leaves_the_other_batches_answered() {
    let server = TestServer::start(|request| {
        if aliased_ids(&request.query()).contains(&7) {
            Reply::json(json!({ "data": null, "errors": [
                { "message": "Mod not available", "path": ["m7"] } ] }))
        } else {
            echo_mod_files(request)
        }
    });
    let ids: Vec<u64> = (1..=25).collect();
    let report = server.client().mod_files_report(&ids).unwrap();
    assert_eq!(report.files.len(), 24);
    assert!(!report.files.contains_key(&7));
    assert_eq!(report.refused.len(), 1);
    // Batch one (1..=20) refuses only 7, batch two (21..=25) answers.
    let sizes: Vec<usize> = server
        .requests()
        .iter()
        .map(|request| aliased_ids(&request.query()).len())
        .collect();
    assert_eq!(sizes, vec![20, 19, 5]);
}

#[test]
fn md5_hits_are_filtered_to_dragons_dogma_2_and_ordered_best_first() {
    let server = TestServer::start(|_| Reply::fixture(FILE_HASHES));
    let asked = vec![
        "E4D909C290D0FB1CA068FFADDF22CBD0".to_string(),
        " 9e107d9d372bb6826bd81d3542a419d6 ".to_string(),
        "not-a-hash".to_string(),
        "9e107d9d372bb6826bd81d3542a419d6".to_string(),
    ];
    let hits = server.client().files_by_md5(&asked).unwrap();

    let sent = server.requests()[0].json();
    assert_eq!(
        sent["variables"]["md5s"],
        json!([
            "e4d909c290d0fb1ca068ffaddf22cbd0",
            "9e107d9d372bb6826bd81d3542a419d6"
        ]),
        "normalized, deduplicated, the invalid one never sent"
    );
    assert!(sent["query"]
        .as_str()
        .unwrap()
        .contains("fileHashes(md5s: $md5s)"));

    let found: Vec<(u64, u64)> = hits.iter().map(|h| (h.mod_id, h.file_id)).collect();
    assert_eq!(
        found,
        vec![(5150, 802), (5150, 790), (4242, 901)],
        "asked order; the live file before its removed twin; other games and orphans dropped"
    );
    let best = &hits[0];
    assert_eq!(best.md5, "e4d909c290d0fb1ca068ffaddf22cbd0");
    assert_eq!(best.group_id, Some(77001));
    assert_eq!(best.uploaded_ms, Some(1_791_987_810_000));
    assert_eq!(best.size_bytes, Some(30_703_184));
    assert_eq!(best.category, FileCategory::Main);
    assert!(best.file_name.ends_with("hJ8kL0mN2.zip"));
    let link = hits[2].to_link();
    assert_eq!(
        (link.mod_id, link.file_id, link.source),
        (4242, Some(901), LinkSource::Md5)
    );
    assert!(hits.iter().all(|h| h.mod_id != 3), "game {GAME_ID} only");
}

#[test]
fn hashes_that_are_not_md5s_send_nothing() {
    let server = TestServer::start(|_| Reply::fixture(GRAPHQL_INVALID_MD5));
    let hits = server
        .client()
        .files_by_md5(&["not-a-hash".to_string(), String::new()])
        .unwrap();
    assert!(hits.is_empty());
    assert_eq!(server.request_count(), 0);
}

#[test]
fn too_many_requests_is_rate_limited_and_the_client_backs_off() {
    let server = TestServer::start(|request| {
        if request.path.starts_with("/v1/") {
            Reply::fixture(V1_VALIDATE)
        } else {
            Reply::status(429).header("Retry-After", "7")
        }
    });
    let client = server.client().with_api_key(Some("invented-key".into()));
    assert_eq!(
        client.mods(&[1]).unwrap_err(),
        NexusError::RateLimited {
            retry_after_s: Some(7)
        }
    );
    // Within the window nothing more reaches Nexus's v2 endpoint.
    match client.mod_files(&[1]) {
        Err(NexusError::RateLimited {
            retry_after_s: Some(wait),
        }) => assert!((1..=7).contains(&wait), "{wait}"),
        other => panic!("expected the back-off, got {other:?}"),
    }
    match client
        .clone()
        .files_by_md5(&["d41d8cd98f00b204e9800998ecf8427e".into()])
    {
        Err(NexusError::RateLimited { .. }) => {}
        other => panic!("clones share the back-off, got {other:?}"),
    }
    assert_eq!(server.request_count(), 1);
    // v1 keeps its own limits.
    assert_eq!(client.validate_key().unwrap().name, "LanternKeeper");
    assert_eq!(server.request_count(), 2);
}

#[test]
fn a_429_without_retry_after_still_backs_off() {
    let server = TestServer::start(|_| Reply::status(429));
    let client = server.client();
    assert_eq!(
        client.mods(&[1]).unwrap_err(),
        NexusError::RateLimited {
            retry_after_s: None
        }
    );
    match client.mods(&[1]) {
        Err(NexusError::RateLimited {
            retry_after_s: Some(wait),
        }) => assert!((1..=30).contains(&wait), "{wait}"),
        other => panic!("expected the default back-off, got {other:?}"),
    }
    assert_eq!(server.request_count(), 1);
}

#[test]
fn http_failures_and_graphql_errors_are_named() {
    let server = TestServer::start(|_| Reply::status(500));
    assert_eq!(
        server.client().mods(&[1]).unwrap_err(),
        NexusError::Http(500)
    );

    let server = TestServer::start(|_| Reply::fixture(GRAPHQL_INVALID_MD5).with_status(400));
    match server.client().mods(&[1]) {
        Err(NexusError::BadResponse(reason)) => {
            assert!(
                reason.starts_with("HTTP 400:") && reason.contains("MD5_INVALID"),
                "{reason}"
            )
        }
        other => panic!("expected BadResponse, got {other:?}"),
    }

    let server = TestServer::start(|_| Reply::fixture(GRAPHQL_REFUSAL));
    match server.client().mods(&[1]) {
        Err(NexusError::BadResponse(reason)) => {
            assert!(reason.contains("Mod not available"), "{reason}")
        }
        other => panic!("expected BadResponse, got {other:?}"),
    }

    let server = TestServer::start(|_| Reply::raw(200, "<html>maintenance</html>"));
    assert!(matches!(
        server.client().mod_files(&[1]),
        Err(NexusError::BadResponse(_))
    ));
}

#[test]
fn an_unreachable_nexus_is_offline() {
    let dead = dead_address();
    let client = NexusClient::new("0.1.0")
        .with_endpoints(&format!("{dead}/v2/graphql"), &format!("{dead}/v1"))
        .with_api_key(Some("invented-key".into()));
    match client.mods(&[1]) {
        Err(NexusError::Offline(reason)) => {
            assert!(reason.starts_with("could not connect"), "{reason}")
        }
        other => panic!("expected Offline, got {other:?}"),
    }
    assert!(matches!(client.validate_key(), Err(NexusError::Offline(_))));
}

#[test]
fn a_nexus_that_never_answers_times_out_as_offline() {
    let server =
        TestServer::start(|_| Reply::fixture(MODS_PAGE).after(Duration::from_millis(1500)));
    let client = server.client().with_timeout(Duration::from_millis(300));
    assert_eq!(
        client.mods(&[4242]).unwrap_err(),
        NexusError::Offline("no answer arrived in time".into())
    );
}

#[test]
fn validate_key_reads_the_account_and_keeps_the_quota() {
    let server = TestServer::start(|request| match request.path.as_str() {
        "/v1/users/validate.json" if request.header("apikey") == Some("invented-key") => {
            Reply::fixture(V1_VALIDATE)
                .header("X-RL-Daily-Limit", "20000")
                .header("X-RL-Daily-Remaining", "19990")
                .header("X-RL-Hourly-Limit", "500")
                .header("X-RL-Hourly-Remaining", "500")
                .header("X-RL-Hourly-Reset", "2026-10-16T13:00:00+00:00")
        }
        _ => Reply::raw(401, r#"{"message":"Please provide a valid API Key"}"#),
    });

    let account = server
        .client()
        .with_api_key(Some(" invented-key ".into()))
        .validate_key()
        .unwrap();
    assert_eq!(account.name, "LanternKeeper");
    assert!(account.is_premium);
    let request = &server.requests()[0];
    assert_eq!(request.method, "GET");
    assert_eq!(request.header("apikey"), Some("invented-key"));
    assert_eq!(request.header("Application-Name"), Some("Riftstone"));

    let client = server.client().with_api_key(Some("wrong-key".into()));
    assert_eq!(client.validate_key().unwrap_err(), NexusError::KeyRejected);
    assert_eq!(client.last_quota(), None, "a 401 carried no quota headers");

    let fresh = server.client().with_api_key(Some("invented-key".into()));
    fresh.validate_key().unwrap();
    let quota = fresh.last_quota().unwrap();
    assert_eq!(
        (quota.daily_remaining, quota.hourly_limit),
        (Some(19_990), Some(500))
    );
    assert!(!quota.is_exhausted());

    let before = server.request_count();
    assert_eq!(
        server.client().validate_key().unwrap_err(),
        NexusError::NoApiKey
    );
    assert_eq!(
        server
            .client()
            .with_api_key(Some("two words".into()))
            .validate_key()
            .unwrap_err(),
        NexusError::KeyRejected,
        "a key with a space inside can never be valid"
    );
    assert_eq!(server.request_count(), before, "neither reached Nexus");
}

#[test]
fn files_v1_reads_the_replacement_chain_and_honours_a_spent_quota() {
    let reset = (chrono::Utc::now() + chrono::TimeDelta::hours(1)).to_rfc3339();
    let server = TestServer::start(move |request| match request.path.as_str() {
        "/v1/games/dragonsdogma2/mods/5150/files.json" => Reply::fixture(V1_FILES)
            .header("X-RL-Daily-Remaining", "0")
            .header("X-RL-Hourly-Remaining", "0")
            .header("X-RL-Hourly-Reset", &reset),
        "/v1/users/validate.json" => Reply::fixture(V1_VALIDATE),
        other => panic!("unexpected request to {other}"),
    });
    let client = server.client().with_api_key(Some("invented-key".into()));

    let list = client.files_v1(5150).unwrap();
    assert_eq!(
        list.files.iter().map(|f| f.file_id).collect::<Vec<_>>(),
        vec![801, 802]
    );
    assert_eq!(list.files[0].category, FileCategory::Archived);
    assert_eq!(list.files[1].size_bytes, Some(30_703_184));
    assert!(list.files[1].primary);
    assert_eq!(list.successor_of(801), Some(802));

    // Both buckets are spent: the next metered call never leaves the machine.
    match client.files_v1(5150) {
        Err(NexusError::RateLimited {
            retry_after_s: Some(wait),
        }) => assert!((3500..=3600).contains(&wait), "{wait}"),
        other => panic!("expected the quota to hold, got {other:?}"),
    }
    assert_eq!(server.request_count(), 1);
    // validate.json is free of the hourly limit.
    client.validate_key().unwrap();
    assert_eq!(server.request_count(), 2);
}

#[test]
fn empty_questions_send_nothing() {
    let server = TestServer::start(|_| Reply::status(500));
    let client = server.client();
    assert!(client.mods(&[]).unwrap().is_empty());
    assert!(client.mods(&[0]).unwrap().is_empty());
    assert!(client.mod_files(&[]).unwrap().is_empty());
    assert!(client.files_by_md5(&[]).unwrap().is_empty());
    assert_eq!(server.request_count(), 0);
}

#[test]
fn an_update_check_end_to_end() {
    // What the window's check does: look the mod up, list its files, pin the
    // package from its 2026 download name, decide.
    let server = TestServer::start(|request| {
        if request.query().contains("legacyModsByDomain") {
            Reply::fixture(MODS_PAGE)
        } else {
            Reply::fixture(MOD_FILES)
        }
    });
    let client = server.client();
    let mods = client.mods(&[5150]).unwrap();
    let files = client.mod_files(&[5150]).unwrap();
    assert_eq!(mods.len(), 1, "only the mod asked about");
    assert_eq!(files.len(), 1);
    // The check's mods lookup brought the labels along (asserted below: the
    // whole check is still two POSTs).
    assert_eq!(mods[0].category.as_deref(), Some("Armour"));
    assert_eq!(mods[0].tags, ["Cloaks", "Lore-Friendly"]);

    // `Vernworth Cloaks Collection 5150 7 2026-09-30T10-44Z aB3dE5fG7.zip`
    let local = NexusLink {
        mod_id: 5150,
        file_id: None,
        group_id: None,
        sqid: Some("aB3dE5fG7".into()),
        version: Some("7".into()),
        uploaded_ms: Some(1_790_767_440_000),
        source: LinkSource::Filename,
    };
    let pinned = pin_link(&local, &files[&5150]).unwrap();
    assert_eq!((pinned.file_id, pinned.group_id), (Some(801), Some(77001)));
    match update_state(&pinned, mods.first(), &files[&5150]) {
        UpdateState::UpdateAvailable { newer } => {
            assert_eq!((newer.file_id, newer.version.as_deref()), (802, Some("8")))
        }
        other => panic!("expected v8 to be offered, got {other:?}"),
    }
    assert_eq!(server.request_count(), 2);
}

// ---------------------------------------------------------------------------
// Answers that carry GraphQL errors: a lookup that failed never reads as empty
// ---------------------------------------------------------------------------

#[test]
fn an_md5_lookup_that_failed_is_an_error_not_no_match() {
    // `fileHashes` is a nullable list: a resolver error nulls it, and a null
    // list must not read as "no Nexus file has this MD5".
    let server = TestServer::start(|_| {
        Reply::fixture(
            r#"{"data":{"fileHashes":null},"errors":[{"message":"Internal server error","path":["fileHashes"]}]}"#,
        )
    });
    let result = server
        .client()
        .files_by_md5(&["d41d8cd98f00b204e9800998ecf8427e".to_string()]);
    assert_eq!(
        result,
        Err(NexusError::BadResponse("Internal server error".into()))
    );
    assert_eq!(server.request_count(), 1);
}

#[test]
fn an_md5_error_inside_another_games_upload_does_not_sink_the_lookup() {
    let with_error_in_hit = |index: u64| {
        let mut answer: Value = serde_json::from_str(FILE_HASHES).unwrap();
        answer["errors"] = json!([{ "message": "That mod is hidden",
                                    "path": ["fileHashes", index, "modFile"] }]);
        answer
    };
    let lantern = vec!["9e107d9d372bb6826bd81d3542a419d6".to_string()];

    // Hit 1 is another game's upload (game 110): Dragon's Dogma 2's hit stands.
    let other_game = with_error_in_hit(1);
    let server = TestServer::start(move |_| Reply::json(other_game.clone()));
    let hits = server.client().files_by_md5(&lantern).unwrap();
    assert_eq!(
        hits.iter()
            .map(|h| (h.mod_id, h.file_id))
            .collect::<Vec<_>>(),
        vec![(4242, 901)]
    );

    // Hit 0 is Dragon's Dogma 2's own: the same error sinks the lookup.
    let own_game = with_error_in_hit(0);
    let server = TestServer::start(move |_| Reply::json(own_game.clone()));
    assert_eq!(
        server.client().files_by_md5(&lantern),
        Err(NexusError::BadResponse("That mod is hidden".into()))
    );
}

#[test]
fn a_mods_page_that_came_with_an_error_is_refused_whole() {
    // An empty page plus an error: the mods are not unknown, the search failed.
    let server = TestServer::start(|_| {
        Reply::fixture(
            r#"{"data":{"legacyModsByDomain":{"totalCount":0,"nodes":[]}},"errors":[{"message":"Search backend unavailable"}]}"#,
        )
    });
    let ids: Vec<u64> = (1..=450).collect();
    assert_eq!(
        server.client().mods(&ids),
        Err(NexusError::BadResponse("Search backend unavailable".into()))
    );
    assert_eq!(
        server.request_count(),
        1,
        "the first refused batch ends the call"
    );

    // A node whose `status` failed would read as status "" (an unavailable mod).
    let server = TestServer::start(|_| {
        Reply::fixture(
            r#"{"data":{"legacyModsByDomain":{"totalCount":1,"nodes":[{"modId":5,"name":"Lantern Glow Recolor","version":"1","updatedAt":null,"status":null}]}},
                "errors":[{"message":"status could not be resolved","path":["legacyModsByDomain","nodes",0,"status"]}]}"#,
        )
    });
    assert_eq!(
        server.client().mods(&[5]),
        Err(NexusError::BadResponse(
            "status could not be resolved".into()
        ))
    );
}

#[test]
fn mod_files_errors_that_name_no_mod_fail_the_batch_at_once() {
    for body in [
        // Every alias null, the error naming none of them.
        r#"{"data":{"m5":null,"m6":null},"errors":[{"message":"Search backend unavailable"}]}"#,
        // A whole-request failure: no data, no path.
        r#"{"data":null,"errors":[{"message":"Search backend unavailable"}]}"#,
    ] {
        let server = TestServer::start(move |_| Reply::fixture(body));
        assert_eq!(
            server.client().mod_files(&[5, 6]),
            Err(NexusError::BadResponse("Search backend unavailable".into())),
            "{body}"
        );
        assert_eq!(server.request_count(), 1, "{body}");
    }
}

#[test]
fn every_mod_refused_by_name_is_an_error_not_an_empty_map() {
    let server = TestServer::start(|_| {
        Reply::fixture(
            r#"{"data":{"m5":null,"m6":null},"errors":[
                {"message":"Mod not available","path":["m5"]},{"message":"Mod not available","path":["m6"]}]}"#,
        )
    });
    assert_eq!(
        server.client().mod_files(&[5, 6]),
        Err(NexusError::BadResponse("Mod not available".into()))
    );
    let report = server.client().mod_files_report(&[5, 6]).unwrap();
    assert!(report.files.is_empty());
    assert_eq!(
        report.refused.keys().copied().collect::<Vec<_>>(),
        vec![5, 6]
    );
    assert_eq!(
        server.request_count(),
        2,
        "one POST per call, nothing asked again"
    );
}

#[test]
fn an_error_inside_one_mods_files_leaves_that_mod_unchecked() {
    let server = TestServer::start(|_| {
        Reply::fixture(
            r#"{"data":{"m5":[{"fileId":50,"groupId":null,"sqid":"a","name":"A","version":"1","category":"MAIN","date":1790000000,"primary":1,"sizeInBytes":null}],
                        "m6":[{"fileId":60,"groupId":6,"sqid":"b","name":"B","version":"1","category":"MAIN","date":1790000000,"primary":1,"sizeInBytes":null}]},
                "errors":[{"message":"groupId could not be resolved","path":["m5",0,"groupId"]}]}"#,
        )
    });
    let report = server.client().mod_files_report(&[5, 6]).unwrap();
    assert_eq!(
        report.files.keys().copied().collect::<Vec<_>>(),
        vec![6],
        "mod 5's list arrived with an error: not trusted"
    );
    assert_eq!(
        report.refused.get(&5),
        Some(&NexusError::BadResponse(
            "groupId could not be resolved".into()
        ))
    );
    assert_eq!(
        server.request_count(),
        1,
        "mod 6 was answered in the same POST"
    );
}

#[test]
fn a_maintenance_page_or_a_whole_request_error_costs_one_post() {
    let ids: Vec<u64> = (1..=45).collect();
    let html = TestServer::start(|_| Reply::raw(200, "<html>maintenance</html>"));
    match html.client().mod_files(&ids) {
        Err(NexusError::BadResponse(reason)) => {
            assert!(reason.contains("not GraphQL JSON"), "{reason}")
        }
        other => panic!("expected BadResponse, got {other:?}"),
    }
    assert_eq!(
        html.request_count(),
        1,
        "not 21: a page that is not GraphQL names no mod to leave out"
    );

    let complex = TestServer::start(|_| {
        Reply::fixture(
            r#"{"data":null,"errors":[{"message":"Query is too complex","extensions":{"code":"QUERY_TOO_COMPLEX"}}]}"#,
        )
    });
    assert_eq!(
        complex.client().mod_files(&ids),
        Err(NexusError::BadResponse(
            "Query is too complex [QUERY_TOO_COMPLEX]".into()
        ))
    );
    assert_eq!(complex.request_count(), 1);
}

// ---------------------------------------------------------------------------
// Redirects and plain http: the key never travels where it should not
// ---------------------------------------------------------------------------

#[test]
fn a_redirect_is_never_followed_so_the_key_never_reaches_another_host() {
    let elsewhere = TestServer::start(|request| {
        if request.method == "POST" {
            Reply::fixture(MODS_PAGE)
        } else {
            Reply::fixture(r#"{"name":"Elsewhere","is_premium":false}"#)
        }
    });
    let v1_target = format!("{}/users/validate.json", elsewhere.v1_base());
    let graphql_target = elsewhere.graphql_url();
    let server = TestServer::start(move |request| {
        if request.path.starts_with("/v1/") {
            Reply::status(302).header("Location", &v1_target)
        } else {
            Reply::status(307).header("Location", &graphql_target)
        }
    });
    let client = server
        .client()
        .with_api_key(Some("invented-secret-key".into()));
    assert_eq!(client.validate_key(), Err(NexusError::Http(302)));
    assert_eq!(client.mods(&[4242]), Err(NexusError::Http(307)));
    assert_eq!(server.request_count(), 2);
    assert_eq!(
        elsewhere.request_count(),
        0,
        "the redirect target never heard from the Riftstone, key or no key"
    );
}

#[test]
fn a_remote_endpoint_must_speak_https() {
    // 192.0.2.1 is TEST-NET-1 (RFC 5737): reserved for documentation and never
    // routed. The client refuses plain http to a host that is not loopback
    // before it opens any connection.
    let client = NexusClient::new("0.1.0")
        .with_endpoints("http://192.0.2.1/v2/graphql", "http://192.0.2.1/v1")
        .with_api_key(Some("invented-secret-key".into()))
        .with_timeout(Duration::from_millis(500));
    for result in [
        client.mods(&[1]).map(|_| ()),
        client.validate_key().map(|_| ()),
    ] {
        match result {
            Err(NexusError::Offline(reason)) => {
                assert!(reason.contains("scheme"), "{reason}")
            }
            other => panic!("expected the plain-http refusal, got {other:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// The stand-in itself
// ---------------------------------------------------------------------------

#[test]
fn the_stand_in_catches_a_request_that_does_not_name_the_riftstone() {
    let server = TestServer::start(|_| Reply::fixture(MODS_PAGE));
    let send_raw = |head: &str| {
        let mut stream = TcpStream::connect(server.addr()).unwrap();
        stream
            .write_all(format!("{head}Content-Length: 2\r\n\r\n{{}}").as_bytes())
            .unwrap();
        let mut reply = Vec::new();
        stream.read_to_end(&mut reply).unwrap();
    };

    send_raw("POST /v2/graphql HTTP/1.1\r\nHost: 127.0.0.1\r\nUser-Agent: curl/8\r\n");
    let violations = server.take_violations();
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].contains("Application-Name") && violations[0].contains("Application-Version"),
        "{violations:?}"
    );

    send_raw(
        "POST /v2/graphql HTTP/1.1\r\nHost: 127.0.0.1\r\nApplication-Name: Riftstone\r\n\
         Application-Version: 0.1.0\r\nUser-Agent: curl/8\r\n",
    );
    let violations = server.take_violations();
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(violations[0].contains("User-Agent"), "{violations:?}");

    // The real client passes the same check.
    server.client().mods(&[4242]).unwrap();
    assert!(server.take_violations().is_empty());
}
