//! v0.3 against the loopback stand-in: what the personal API key does
//! (the account, download links with and without the site's grant, tracking,
//! endorsing) and what needs no key (a mod's files with their patch notes,
//! browsing, the download check).
//!
//! The answers are hand-written in the shapes researched on 2026-10-17
//! (`download_link.json` → `[{ "URI", "name", "short_name" }]`, its 403 / 410
//! / 400 / 404 refusals, `tracked_mods.json` and `endorsements.json` lists,
//! the `NOT_DOWNLOADED_MOD` / `TOO_SOON_AFTER_DOWNLOAD` / `IS_OWN_MOD`
//! refusal codes, `modFiles` with `description` and `changelogText`, a
//! `mods` page with `totalCount`). Names, ids, keys and links are invented.
//! Nothing here touches the network.

mod support;

use rift_core::NexusReason;
use rift_nexus::{
    parse_nxm, BrowseQuery, BrowseSort, DownloadCheck, EndorseRefusal, EndorseStatus, FileCategory,
    NexusClient, NexusError, NxmGrant, NxmMessage,
};
use serde_json::{json, Value};
use support::{Recorded, Reply, TestServer};

const V1_VALIDATE: &str = include_str!("fixtures/v1_validate.json");
const V1_FILES: &str = include_str!("fixtures/v1_files.json");
const FILE_HASHES: &str = include_str!("fixtures/file_hashes.json");

/// The invented personal API key every keyed client here holds.
const API_KEY: &str = "invented-personal-key";
/// The invented key of a site link's grant.
const GRANT_KEY: &str = "Invented-Grant_Key.0123";

fn keyed(server: &TestServer) -> NexusClient {
    server.client().with_api_key(Some(API_KEY.into()))
}

fn grant() -> NxmGrant {
    NxmGrant {
        key: GRANT_KEY.into(),
        expires_s: 1_791_987_830,
    }
}

/// Every v1 request here must carry the key, and only v1 requests may.
fn assert_key_only_on_v1(requests: &[Recorded]) {
    for request in requests {
        let on_v1 = request.path.starts_with("/v1/");
        assert_eq!(
            request.header("apikey"),
            on_v1.then_some(API_KEY),
            "{} {}",
            request.method,
            request.route()
        );
    }
}

/// A `download_link.json` answer naming `count` mirrors on this server.
fn mirrors_on(server_url: impl Fn(&str) -> String, count: usize) -> Value {
    let names = ["Nexus CDN", "Amsterdam", "Chicago"];
    Value::Array(
        names
            .iter()
            .take(count)
            .enumerate()
            .map(|(index, name)| {
                json!({ "name": name, "short_name": name,
                        "URI": server_url(&format!("/cdn/{index}/lantern.zip?md5=invented-signature&expires=1")) })
            })
            .collect(),
    )
}

// ---------------------------------------------------------------------------
// The account
// ---------------------------------------------------------------------------

#[test]
fn validate_key_reads_the_account_its_id_and_its_standing() {
    let server = TestServer::start(|_| Reply::fixture(V1_VALIDATE));
    let account = keyed(&server).validate_key().unwrap();
    assert_eq!(account.name, "LanternKeeper");
    assert_eq!(account.user_id, Some(1_234_567));
    assert!(account.is_premium, "the current spelling wins");
    assert!(!account.is_supporter);
    let shown = format!("{account:?}");
    assert!(
        !shown.contains("invented-key-the-server-echoes-back")
            && !shown.contains("example.invalid"),
        "never the echoed key, never the e-mail: {shown}"
    );

    let supporter = TestServer::start(|_| {
        Reply::json(json!({ "user_id": 4242, "name": "Arisen of Vernworth",
                            "is_premium?": false, "is_supporter?": true }))
    });
    let account = keyed(&supporter).validate_key().unwrap();
    assert_eq!(
        (account.user_id, account.is_premium, account.is_supporter),
        (Some(4242), false, true)
    );
}

// ---------------------------------------------------------------------------
// Download links
// ---------------------------------------------------------------------------

#[test]
fn download_links_without_a_grant_ask_as_a_premium_account() {
    let cdn = TestServer::start(|_| Reply::status(500));
    let answer = mirrors_on(|path| cdn.url(path), 2);
    let server = TestServer::start(move |_| Reply::json(answer.clone()));
    let mirrors = keyed(&server).download_links(2207, 9001, None).unwrap();
    assert_eq!(
        mirrors.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
        vec!["Nexus CDN", "Amsterdam"],
        "Nexus's preferred mirror first"
    );
    assert!(mirrors[0].uri.starts_with(&cdn.url("/cdn/0/lantern.zip")));
    let request = &server.requests()[0];
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.path, "/v1/games/dragonsdogma2/mods/2207/files/9001/download_link.json",
        "no query without a grant"
    );
    assert_key_only_on_v1(&server.requests());
    assert_eq!(cdn.request_count(), 0, "asking for links downloads nothing");
}

#[test]
fn download_links_with_a_grant_carry_its_key_and_expiry() {
    let server = TestServer::start(|_| {
        Reply::json(json!([{ "name": "Nexus CDN", "short_name": "Nexus CDN",
                             "URI": "https://cdn.example.invalid/2207/9001/lantern.zip?md5=sig" }]))
    });
    let mirrors = keyed(&server)
        .download_links(2207, 9001, Some(&grant()))
        .unwrap();
    assert_eq!(mirrors.len(), 1);
    let request = &server.requests()[0];
    assert_eq!(
        request.route(),
        "/v1/games/dragonsdogma2/mods/2207/files/9001/download_link.json"
    );
    assert_eq!(
        request.query_pairs(),
        vec![
            ("key".to_string(), GRANT_KEY.to_string()),
            ("expires".to_string(), "1791987830".to_string())
        ]
    );
    assert_key_only_on_v1(&server.requests());

    // A grant read from a site link goes out as it came in, even a key with
    // characters a query must escape.
    let Ok(NxmMessage::File(link)) = parse_nxm(
        "nxm://dragonsdogma2/mods/2207/files/9001?key=a%2Bb/c&expires=1791987830&user_id=4242",
    ) else {
        panic!("the invented link parses");
    };
    keyed(&server)
        .download_links(link.mod_id, link.file_id, link.grant.as_ref())
        .unwrap();
    assert_eq!(
        server.requests()[1].query_pairs()[0],
        ("key".to_string(), "a+b/c".to_string())
    );
}

#[test]
fn each_download_link_refusal_has_its_reason_and_never_shows_the_grant() {
    let cases = [
        (400, NexusError::LinkRefused, Some(NexusReason::LinkRefused)),
        (401, NexusError::KeyRejected, Some(NexusReason::KeyRejected)),
        (
            403,
            NexusError::PremiumRequired,
            Some(NexusReason::PremiumRequired),
        ),
        (404, NexusError::FileGone, Some(NexusReason::FileGone)),
        (410, NexusError::LinkExpired, Some(NexusReason::LinkExpired)),
        (500, NexusError::Http(500), None),
    ];
    for (status, expected, reason) in cases {
        let server = TestServer::start(move |_| {
            Reply::raw(status, r#"{"code":0,"message":"invented refusal"}"#)
        });
        let err = keyed(&server)
            .download_links(2207, 9001, Some(&grant()))
            .unwrap_err();
        assert_eq!(err, expected, "HTTP {status}");
        assert_eq!(err.reason(), reason, "HTTP {status}");
        for shown in [err.to_string(), format!("{err:?}")] {
            assert!(
                !shown.contains(GRANT_KEY) && !shown.contains(API_KEY),
                "{shown}"
            );
        }
    }
    // A link answer with no https mirror is no answer to download from.
    let plain = TestServer::start(|_| {
        Reply::json(
            json!([{ "name": "Old", "short_name": "Old", "URI": "http://cdn.example.invalid/a.zip" }]),
        )
    });
    match keyed(&plain).download_links(2207, 9001, None) {
        Err(NexusError::BadResponse(reason)) => {
            assert!(!reason.contains("example.invalid"), "{reason}")
        }
        other => panic!("expected BadResponse, got {other:?}"),
    }
}

#[test]
fn a_spent_quota_refuses_key_calls_locally() {
    let reset = (chrono::Utc::now() + chrono::TimeDelta::hours(1)).to_rfc3339();
    let server = TestServer::start(move |request| match request.route() {
        "/v1/users/validate.json" => Reply::fixture(V1_VALIDATE)
            .header("X-RL-Daily-Remaining", "0")
            .header("X-RL-Hourly-Remaining", "0")
            .header("X-RL-Hourly-Reset", &reset),
        other => panic!("nothing metered may leave the machine, yet {other} did"),
    });
    let client = keyed(&server);
    client.validate_key().unwrap();
    let spent: Vec<Result<(), NexusError>> = vec![
        client.download_links(2207, 9001, None).map(|_| ()),
        client
            .download_links(2207, 9001, Some(&grant()))
            .map(|_| ()),
        client.tracked_mods().map(|_| ()),
        client.track(2207),
        client.untrack(2207),
        client.endorsements().map(|_| ()),
        client.endorse(2207, "2.1"),
        client.abstain(2207, "2.1"),
    ];
    for result in spent {
        match result {
            Err(NexusError::RateLimited {
                retry_after_s: Some(wait),
            }) => assert!((3500..=3600).contains(&wait), "{wait}"),
            other => panic!("expected the quota to hold, got {other:?}"),
        }
    }
    assert_eq!(
        server.request_count(),
        1,
        "only the free validation went out"
    );
    // validate.json spends no quota, so it still goes.
    client.validate_key().unwrap();
    assert_eq!(server.request_count(), 2);
}

#[test]
fn key_calls_without_a_key_send_nothing() {
    let server = TestServer::start(|_| Reply::status(500));
    let client = server.client();
    assert_eq!(
        client.download_links(2207, 9001, None).unwrap_err(),
        NexusError::NoApiKey
    );
    assert_eq!(client.tracked_mods().unwrap_err(), NexusError::NoApiKey);
    assert_eq!(client.track(2207).unwrap_err(), NexusError::NoApiKey);
    assert_eq!(client.endorse(2207, "1").unwrap_err(), NexusError::NoApiKey);
    assert_eq!(
        NexusError::NoApiKey.reason(),
        Some(NexusReason::NoKey),
        "the window voices it as noKey"
    );
    assert_eq!(server.request_count(), 0);
}

// ---------------------------------------------------------------------------
// Tracking
// ---------------------------------------------------------------------------

#[test]
fn tracked_mods_are_this_games_each_once() {
    let server = TestServer::start(|_| {
        Reply::json(json!([
            { "mod_id": 2207, "domain_name": "dragonsdogma2" },
            { "mod_id": 4242, "domain_name": "skyrimspecialedition" },
            { "mod_id": 5150, "domain_name": "dragonsdogma2" },
            { "mod_id": 2207, "domain_name": "dragonsdogma2" }
        ]))
    });
    assert_eq!(keyed(&server).tracked_mods().unwrap(), vec![2207, 5150]);
    let request = &server.requests()[0];
    assert_eq!(
        (request.method.as_str(), request.path.as_str()),
        ("GET", "/v1/user/tracked_mods.json")
    );
    assert_key_only_on_v1(&server.requests());
}

#[test]
fn track_and_untrack_send_the_domain_and_the_mod_as_a_form() {
    let server = TestServer::start(|request| match request.method.as_str() {
        "POST" if request.form() == [("mod_id".to_string(), "2207".to_string())] => {
            Reply::raw(201, r#"{"message":"User 1 is now Tracking Mod: 2207"}"#)
        }
        "POST" => Reply::raw(200, r#"{"message":"User 1 is already Tracking Mod: 5150"}"#),
        "DELETE" if request.form() == [("mod_id".to_string(), "2207".to_string())] => {
            Reply::raw(200, r#"{"message":"User 1 is no longer tracking 2207"}"#)
        }
        _ => Reply::raw(
            404,
            r#"{"message":"User is not tracking mod. Unable to untrack."}"#,
        ),
    });
    let client = keyed(&server);
    client.track(2207).expect("201: now tracked");
    client.track(5150).expect("200: already tracked counts");
    client.untrack(2207).expect("200: untracked");
    client
        .untrack(6006)
        .expect("404: it was not tracked, so it is untracked either way");

    let requests = server.requests();
    assert_eq!(
        requests
            .iter()
            .map(|r| r.method.as_str())
            .collect::<Vec<_>>(),
        vec!["POST", "POST", "DELETE", "DELETE"]
    );
    for request in &requests {
        assert_eq!(request.route(), "/v1/user/tracked_mods.json");
        assert_eq!(
            request.query_pairs(),
            vec![("domain_name".to_string(), "dragonsdogma2".to_string())]
        );
        assert_eq!(
            request.header("Content-Type"),
            Some("application/x-www-form-urlencoded")
        );
        assert_eq!(request.form().len(), 1, "only the mod id");
    }
    assert_key_only_on_v1(&requests);

    let refused = TestServer::start(|request| match request.method.as_str() {
        "POST" => Reply::raw(404, r#"{"message":"No Mod Found"}"#),
        _ => Reply::raw(401, r#"{"message":"Please provide a valid API Key"}"#),
    });
    assert_eq!(keyed(&refused).track(1).unwrap_err(), NexusError::FileGone);
    assert_eq!(
        keyed(&refused).untrack(1).unwrap_err(),
        NexusError::KeyRejected
    );
}

// ---------------------------------------------------------------------------
// Endorsing
// ---------------------------------------------------------------------------

#[test]
fn endorse_and_abstain_send_the_version_the_player_holds() {
    let server = TestServer::start(|request| {
        let status = if request.route().ends_with("/endorse.json") {
            "Endorsed"
        } else {
            "Abstained"
        };
        Reply::json(json!({ "message": format!("Updated to: {status}"), "status": status }))
    });
    let client = keyed(&server);
    client.endorse(2207, " 2.1 beta ").unwrap();
    client.abstain(5150, "8").unwrap();
    let requests = server.requests();
    assert_eq!(
        requests
            .iter()
            .map(|r| (r.method.as_str(), r.route()))
            .collect::<Vec<_>>(),
        vec![
            ("POST", "/v1/games/dragonsdogma2/mods/2207/endorse.json"),
            ("POST", "/v1/games/dragonsdogma2/mods/5150/abstain.json")
        ]
    );
    assert_eq!(
        requests[0].form(),
        vec![("Version".to_string(), "2.1 beta".to_string())],
        "trimmed, and encoded as a form"
    );
    assert_eq!(
        requests[1].form(),
        vec![("Version".to_string(), "8".to_string())]
    );
    assert_key_only_on_v1(&requests);
}

#[test]
fn endorse_refusals_come_back_with_their_reasons() {
    let cases = [
        (
            "NOT_DOWNLOADED_MOD",
            EndorseRefusal::NotDownloaded,
            NexusReason::EndorseNotDownloaded,
        ),
        (
            "TOO_SOON_AFTER_DOWNLOAD",
            EndorseRefusal::TooSoon,
            NexusReason::EndorseTooSoon,
        ),
        (
            "IS_OWN_MOD",
            EndorseRefusal::OwnMod,
            NexusReason::EndorseOwnMod,
        ),
    ];
    for (code, refusal, reason) in cases {
        for status in [403, 400, 200] {
            let server = TestServer::start(move |_| {
                Reply::raw(
                    status,
                    &format!(r#"{{"code":{status},"message":"{code}"}}"#),
                )
            });
            let err = keyed(&server).endorse(2207, "2.1").unwrap_err();
            assert_eq!(
                err,
                NexusError::EndorseRefused(refusal),
                "{code} with {status}"
            );
            assert_eq!(err.reason(), Some(reason));
            let sentence = err.to_string();
            assert!(
                !sentence.contains("403") && !sentence.to_lowercase().contains("reject"),
                "a refusal is not a rejected key: {sentence}"
            );
            assert_eq!(
                keyed(&server).abstain(2207, "2.1").unwrap_err(),
                NexusError::EndorseRefused(refusal)
            );
        }
    }
    let other = TestServer::start(|request| {
        if request.route().contains("/2207/") {
            Reply::raw(404, r#"{"message":"No Mod Found"}"#)
        } else {
            Reply::raw(401, r#"{"message":"Please provide a valid API Key"}"#)
        }
    });
    assert_eq!(
        keyed(&other).endorse(2207, "1").unwrap_err(),
        NexusError::FileGone
    );
    assert_eq!(
        keyed(&other).endorse(5150, "1").unwrap_err(),
        NexusError::KeyRejected
    );
}

#[test]
fn endorsements_list_this_games_decisions() {
    let server = TestServer::start(|_| {
        Reply::json(json!([
            { "mod_id": 2207, "domain_name": "dragonsdogma2", "date": 1_700_000_000, "version": "2.1", "status": "Endorsed" },
            { "mod_id": 5150, "domain_name": "dragonsdogma2", "date": 1_700_000_000, "version": "8", "status": "Abstained" },
            { "mod_id": 6006, "domain_name": "dragonsdogma2", "date": 1_700_000_000, "version": "1", "status": "Undecided" },
            { "mod_id": 4242, "domain_name": "skyrimspecialedition", "date": 1_700_000_000, "version": "1", "status": "Endorsed" }
        ]))
    });
    assert_eq!(
        keyed(&server).endorsements().unwrap(),
        vec![
            (2207, EndorseStatus::Endorsed),
            (5150, EndorseStatus::Abstained)
        ]
    );
    let request = &server.requests()[0];
    assert_eq!(
        (request.method.as_str(), request.path.as_str()),
        ("GET", "/v1/user/endorsements.json")
    );
}

// ---------------------------------------------------------------------------
// files.json names each archive
// ---------------------------------------------------------------------------

#[test]
fn files_v1_names_each_files_archive() {
    let server = TestServer::start(|_| Reply::fixture(V1_FILES));
    let list = keyed(&server).files_v1(5150).unwrap();
    assert_eq!(
        list.file_name(802),
        Some("Vernworth Cloaks Collection-5150-8-1791987810.zip")
    );
    assert_eq!(
        list.file_name(801),
        Some("Vernworth Cloaks Collection-5150-7-1790767476.zip")
    );
    assert_eq!(list.file_name(803), None);
    assert_eq!(list.successor_of(801), Some(802), "the chain still reads");
}

// ---------------------------------------------------------------------------
// A mod's files with their patch notes (v2, anonymous)
// ---------------------------------------------------------------------------

#[test]
fn mod_file_details_bring_descriptions_and_patch_notes_without_the_key() {
    let server = TestServer::start(|_| {
        Reply::json(json!({ "data": { "modFiles": [
            { "fileId": 9002, "groupId": 31, "sqid": "Rk4vLmN2Q", "name": "Lantern Glow Recolor",
              "version": "2.1", "category": "MAIN", "date": 1_791_987_810, "primary": 1,
              "sizeInBytes": "30703184", "description": "Warmer lanterns\nfor Vernworth.",
              "changelogText": ["Ember tint for the gate lanterns.", "Fixed a seam on the guild sign."] },
            { "fileId": 9001, "groupId": 31, "sqid": "aB3dE5fG7", "name": "Lantern Glow Recolor",
              "version": "2.0", "category": "OLD_VERSION", "date": 1_790_767_476, "primary": 0,
              "sizeInBytes": null, "description": "", "changelogText": [] }
        ] } }))
    });
    let details = keyed(&server).mod_file_details(2207).unwrap();
    assert_eq!(
        details.iter().map(|d| d.file.file_id).collect::<Vec<_>>(),
        vec![9001, 9002],
        "oldest upload first"
    );
    let main = &details[1];
    assert_eq!(main.file.category, FileCategory::Main);
    assert_eq!(main.file.size_bytes, Some(30_703_184));
    assert_eq!(main.file.group_id, Some(31));
    assert_eq!(main.file.uploaded_ms, 1_791_987_810_000);
    assert!(main.file.primary);
    assert_eq!(
        main.description.as_deref(),
        Some("Warmer lanterns for Vernworth.")
    );
    assert_eq!(
        main.changelog,
        vec![
            "Ember tint for the gate lanterns.",
            "Fixed a seam on the guild sign."
        ]
    );
    assert_eq!(details[0].description, None);
    assert!(details[0].changelog.is_empty());

    let request = &server.requests()[0];
    assert_eq!(
        (request.method.as_str(), request.path.as_str()),
        ("POST", "/v2/graphql")
    );
    let query = request.query();
    assert!(
        query.contains("modFiles(modId: 2207, gameId: 6234)"),
        "{query}"
    );
    assert!(query.contains("description changelogText"), "{query}");
    assert_eq!(request.header("apikey"), None, "v2 is anonymous");
}

#[test]
fn mod_file_details_never_read_a_failed_list_as_no_files() {
    let failed = TestServer::start(|_| {
        Reply::fixture(
            r#"{"data":{"modFiles":null},"errors":[{"message":"Mod not available","path":["modFiles"]}]}"#,
        )
    });
    assert_eq!(
        failed.client().mod_file_details(2207),
        Err(NexusError::BadResponse("Mod not available".into()))
    );
    let partial = TestServer::start(|_| {
        Reply::fixture(
            r#"{"data":{"modFiles":[{"fileId":1,"name":"A","changelogText":null}]},"errors":[{"message":"changelogText could not be resolved","path":["modFiles",0,"changelogText"]}]}"#,
        )
    });
    assert_eq!(
        partial.client().mod_file_details(2207),
        Err(NexusError::BadResponse(
            "changelogText could not be resolved".into()
        )),
        "notes that failed never read as no notes"
    );
    let unknown = TestServer::start(|_| Reply::json(json!({ "data": { "modFiles": [] } })));
    assert!(unknown.client().mod_file_details(7777).unwrap().is_empty());
    let silent = TestServer::start(|_| Reply::status(500));
    assert!(silent.client().mod_file_details(0).unwrap().is_empty());
    assert!(silent
        .client()
        .mod_file_details(1 << 40)
        .unwrap()
        .is_empty());
    assert_eq!(
        silent.request_count(),
        0,
        "ids Nexus cannot hold send nothing"
    );
}

// ---------------------------------------------------------------------------
// Browsing (v2, anonymous)
// ---------------------------------------------------------------------------

/// A browse answer: `count` mods from `first`, of `total` in all.
fn browse_page(first: u64, count: u64, total: u64) -> Reply {
    let nodes: Vec<Value> = (first..first + count)
        .map(|id| {
            json!({ "modId": id, "name": format!("Vernworth Cloak {id}"), "version": "1",
                    "author": "Vern", "category": "Armour", "adultContent": false,
                    "downloads": 100, "endorsements": 10, "summary": "A cloak.",
                    "updatedAt": "2026-10-14T14:23:50Z", "game": { "domainName": "dragonsdogma2" } })
        })
        .collect();
    Reply::json(json!({ "data": { "mods": { "totalCount": total, "nodes": nodes } } }))
}

#[test]
fn browse_sends_the_text_only_as_a_variable_and_pages_by_twenty() {
    let server = TestServer::start(|request| {
        let offset = request
            .query()
            .split("offset: ")
            .nth(1)
            .and_then(|rest| rest.split(',').next())
            .and_then(|offset| offset.trim().parse::<u64>().ok())
            .expect("an offset in the query");
        browse_page(1000 + offset, 20.min(45 - offset), 45)
    });
    let client = keyed(&server);
    let mut pages = Vec::new();
    for offset in [0, 20, 40] {
        pages.push(
            client
                .browse(&BrowseQuery {
                    text: "  vernworth   cloak ".into(),
                    sort: BrowseSort::Relevance,
                    offset,
                })
                .unwrap(),
        );
    }
    assert_eq!(
        pages
            .iter()
            .map(|p| (p.total, p.offset, p.mods.len()))
            .collect::<Vec<_>>(),
        vec![(45, 0, 20), (45, 20, 20), (45, 40, 5)],
        "the page size is however many came"
    );
    assert_eq!(pages[1].mods[0].mod_id, 1020);
    assert_eq!(pages[0].mods[0].name, "Vernworth Cloak 1000");

    for request in server.requests() {
        assert_eq!(request.header("apikey"), None, "browsing is anonymous");
        assert_eq!(
            request.json()["variables"],
            json!({ "text": "vernworth cloak" })
        );
        let query = request.query();
        assert!(
            !query.contains("vernworth"),
            "the text is no query text: {query}"
        );
        assert!(
            query.contains("nameStemmed: [{ value: $text, op: MATCHES }]"),
            "{query}"
        );
        assert!(
            query.contains("sort: [{ relevance: { direction: DESC } }]"),
            "{query}"
        );
        assert!(query.contains("count: 20)"), "{query}");
        assert!(!query.contains("OR"), "never a top-level OR: {query}");
    }
}

#[test]
fn browse_without_text_declares_no_variable_and_sorts_as_asked() {
    let server = TestServer::start(|_| browse_page(1, 20, 4000));
    let client = server.client();
    for (sort, key) in [
        (BrowseSort::Endorsements, "endorsements"),
        (BrowseSort::Downloads, "downloads"),
        (BrowseSort::Updated, "updatedAt"),
        (BrowseSort::Relevance, "endorsements"),
    ] {
        let page = client
            .browse(&BrowseQuery {
                text: "   ".into(),
                sort,
                offset: 60,
            })
            .unwrap();
        assert_eq!((page.total, page.offset, page.mods.len()), (4000, 60, 20));
        let request = server.requests().pop().unwrap();
        assert!(request.json().get("variables").is_none(), "{sort:?}");
        let query = request.query();
        assert!(
            !query.contains("$text") && !query.contains("nameStemmed"),
            "{query}"
        );
        assert!(
            query.contains(&format!(
                "sort: [{{ {key}: {{ direction: DESC }} }}], offset: 60, count: 20)"
            )),
            "{sort:?}: {query}"
        );
        assert!(
            query.contains(r#"gameDomainName: [{ value: "dragonsdogma2", op: EQUALS }]"#),
            "{query}"
        );
    }
}

#[test]
fn a_browse_that_failed_is_an_error_never_an_empty_page() {
    let server = TestServer::start(|_| {
        Reply::fixture(
            r#"{"data":{"mods":{"totalCount":0,"nodes":[]}},"errors":[{"message":"Search backend unavailable"}]}"#,
        )
    });
    assert_eq!(
        server.client().browse(&BrowseQuery::default()),
        Err(NexusError::BadResponse("Search backend unavailable".into()))
    );
    let limited = TestServer::start(|_| Reply::status(429).header("Retry-After", "9"));
    let client = limited.client();
    assert_eq!(
        client.browse(&BrowseQuery::default()),
        Err(NexusError::RateLimited {
            retry_after_s: Some(9)
        })
    );
    assert!(matches!(
        client.mod_file_details(2207),
        Err(NexusError::RateLimited { .. })
    ));
    assert_eq!(
        limited.request_count(),
        1,
        "the back-off covers every v2 call"
    );
}

// ---------------------------------------------------------------------------
// The download check (v2, anonymous)
// ---------------------------------------------------------------------------

#[test]
fn confirm_download_names_the_file_or_refuses_it() {
    // The fixture knows 9e107d9d… as mod 4242's file 901 (and another game's
    // upload), and nothing at all for any other hash.
    let server = TestServer::start(|request| {
        let asked = request.json()["variables"]["md5s"][0]
            .as_str()
            .unwrap_or_default()
            .to_string();
        if asked == "9e107d9d372bb6826bd81d3542a419d6" {
            Reply::fixture(FILE_HASHES)
        } else {
            Reply::json(json!({ "data": { "fileHashes": [] } }))
        }
    });
    let client = keyed(&server);
    assert_eq!(
        client.confirm_download("9E107D9D372BB6826BD81D3542A419D6", 4242, 901),
        Ok(DownloadCheck::Confirmed)
    );
    assert_eq!(
        client.confirm_download("9e107d9d372bb6826bd81d3542a419d6", 4242, 902),
        Err(NexusError::NotTheFile),
        "the bytes are another file of the same mod"
    );
    assert_eq!(
        client.confirm_download("9e107d9d372bb6826bd81d3542a419d6", 2207, 901),
        Err(NexusError::NotTheFile)
    );
    assert_eq!(
        NexusError::NotTheFile.reason(),
        Some(NexusReason::NotTheFile)
    );
    assert_eq!(
        client.confirm_download("d41d8cd98f00b204e9800998ecf8427e", 4242, 901),
        Ok(DownloadCheck::Unknown),
        "a hash Nexus has not seen yet: the size alone vouches"
    );
    let before = server.request_count();
    assert!(matches!(
        client.confirm_download("not-a-hash", 4242, 901),
        Err(NexusError::BadResponse(_))
    ));
    assert_eq!(
        server.request_count(),
        before,
        "a broken hash sends nothing"
    );
    for request in server.requests() {
        assert_eq!(request.header("apikey"), None, "the check is anonymous");
    }
}
