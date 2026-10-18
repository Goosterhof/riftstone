//! Downloads into the mods folder (v0.3) against two loopback stand-ins: one
//! plays Nexus's API (v1 with the key, v2 anonymous), the other its download
//! servers (the CDN). A premium download lands under its Nexus name and links
//! itself; a free account's download waits for the site's Mod Manager
//! Download link and resumes on it; a cut-off body, a wrong size, a hash that
//! names another file and a cancel leave nothing in the mods folder; the same
//! bytes already there are not written twice; the CDN never sees the key;
//! and every nxm link the stone refuses says why. Every id, name and key here
//! is invented.

mod support;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use riftstone_lib::commands::downloads::get_downloads_impl;
use riftstone_lib::commands::nexus;
use riftstone_lib::downloads;
use riftstone_lib::dto::{DownloadPhase, DownloadView, LinkSource};
use riftstone_lib::events::{
    RecordedEvents, DOWNLOADS_CHANGED, DOWNLOAD_PROGRESS, NXM_RECEIVED, UPDATES_CHANGED,
};
use riftstone_lib::nxm;
use riftstone_lib::state::AppState;
use support::{wait_for, write_zip, Answer, Bench, Cdn, Delivery, Seen, StandIn};

/// The bench's personal API key (invented).
const KEY: &str = "invented-personal-key-0042";
/// The site's grant in a Mod Manager Download link (invented).
const GRANT: &str = "Invented-Grant_Key.0123";
/// What validate.json echoes besides the account; never read.
const EMAIL: &str = "arisen@vernworth.invalid";
const MOD: u64 = 2207;
const FILE: u64 = 9001;
const USER: u64 = 4242;
/// The archive's own name on Nexus.
const ARCHIVE: &str = "Lantern Glow Recolor-2207-2-2-1791987780.zip";
const UPLOADED_S: u64 = 1_791_987_780;
const CDN_PATH: &str = "/cdn/6234/2207/Lantern%20Glow%20Recolor-2207-2-2-1791987780.zip";

/// The update the stand-in's CDN serves: a real zip the scan reads.
fn lantern_update() -> Vec<u8> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("update.zip");
    write_zip(
        &path,
        &[
            (
                "Lantern Glow/modinfo.ini",
                b"name=Lantern Glow Recolor\r\nversion=2.2\r\n",
            ),
            (
                "Lantern Glow/natives/stm/effect/lantern/lantern_glow_amber.tex.251211553",
                b"amber lantern texture bytes",
            ),
        ],
    );
    std::fs::read(path).unwrap()
}

/// What the API stand-in says.
#[derive(Clone, Default)]
struct Scene {
    premium: bool,
    /// The size Nexus states for the file.
    size: u64,
    /// The download link Nexus hands out.
    link: String,
    /// The file the MD5 lookup names: `None` for no hit at all.
    hash_names: Option<(u64, u64)>,
    /// Where Nexus states the size.
    stated: Stated,
    /// The archive's own name in v1 `files.json`, else [`ARCHIVE`].
    archive: Option<&'static str>,
    /// The anonymous MD5 lookup answers 429 with an hour's Retry-After.
    lookup_refused: bool,
}

/// Where the API stand-in states the file's size.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Stated {
    /// v2 `modFiles` and v1 `files.json` both.
    #[default]
    Everywhere,
    /// v1 `files.json` only (v2's `sizeInBytes` is null).
    V1Only,
    /// Nowhere.
    Nowhere,
}

fn status(code: u16) -> Answer {
    Answer {
        status: code,
        headers: Vec::new(),
        body: json!({}),
    }
}

fn nexus_api(scene: Scene) -> impl Fn(&Seen) -> Answer + Send + 'static {
    move |request| {
        let path = request.path.split('?').next().unwrap_or_default();
        if path.ends_with("/users/validate.json") {
            return Answer::json(json!({
                "user_id": USER, "key": KEY, "name": "Arisen of Vernworth", "email": EMAIL,
                "is_premium": scene.premium, "is_supporter": false
            }));
        }
        if path.ends_with(&format!("/mods/{MOD}/files.json")) {
            let v1_size = (scene.stated != Stated::Nowhere).then_some(scene.size);
            return Answer::json(json!({
                "files": [{ "file_id": FILE, "file_name": scene.archive.unwrap_or(ARCHIVE),
                            "name": "Lantern Glow Recolor", "version": "2.2", "category_id": 1,
                            "uploaded_timestamp": UPLOADED_S, "size_in_bytes": v1_size }],
                "file_updates": []
            }));
        }
        if path.ends_with(&format!("/mods/{MOD}/files/{FILE}/download_link.json")) {
            let granted = request.path.contains(&format!("key={GRANT}"));
            if !scene.premium && !granted {
                return status(403);
            }
            return Answer::json(json!([
                { "name": "Nexus CDN", "short_name": "Nexus CDN", "URI": scene.link },
                { "name": "Elsewhere", "short_name": "Elsewhere", "URI": "http://cdn.example.invalid/x.zip" }
            ]));
        }
        if request.body.contains("RiftstoneModFileDetails") {
            let v2_size = (scene.stated == Stated::Everywhere).then(|| scene.size.to_string());
            return Answer::json(json!({ "data": { "modFiles": [{
                "fileId": FILE, "groupId": 31, "sqid": "Rk4vLmN2Q", "name": "Lantern Glow Recolor",
                "version": "2.2", "category": "MAIN", "date": UPLOADED_S, "primary": 1,
                "sizeInBytes": v2_size,
                "description": "Warmer amber for every hand lantern.",
                "changelogText": ["Amber for every lantern."]
            }]}}));
        }
        if request.body.contains("legacyModsByDomain") {
            return Answer::json(json!({ "data": { "legacyModsByDomain": {
                "totalCount": 1,
                "nodes": [{ "modId": MOD, "name": "Lantern Glow Recolor", "version": "2.2",
                            "updatedAt": "2026-10-14T14:23:00Z", "status": "published",
                            "category": "Visuals", "tags": [], "adultContent": false }]
            }}}));
        }
        if request.body.contains("fileHashes") {
            if scene.lookup_refused {
                return Answer {
                    status: 429,
                    headers: vec![("Retry-After".into(), "3600".into())],
                    body: json!({}),
                };
            }
            let asked: Value = serde_json::from_str(&request.body).unwrap();
            let md5 = asked["variables"]["md5s"][0].clone();
            let hits = match scene.hash_names {
                Some((mod_id, file_id)) => json!([{
                    "md5": md5, "fileName": ARCHIVE, "fileSize": scene.size.to_string(),
                    "gameId": 6234, "modFileId": file_id, "createdAt": "2026-10-14T14:23:00Z",
                    "modFile": { "modId": mod_id, "fileId": file_id, "groupId": 31, "sqid": "Rk4vLmN2Q",
                                 "name": "Lantern Glow Recolor", "version": "2.2", "category": "MAIN",
                                 "date": UPLOADED_S, "primary": 1, "sizeInBytes": scene.size.to_string() }
                }]),
                None => json!([]),
            };
            return Answer::json(json!({ "data": { "fileHashes": hits } }));
        }
        status(404)
    }
}

/// Open the bench with the key stored, its Nexus at `api`, the library
/// scanned, the account validated.
fn open(bench: &Bench, api: &StandIn) -> (Arc<AppState>, Arc<RecordedEvents>) {
    let (state, events) = bench.open_keyed(bench.config_at(api), KEY);
    assert!(state.scan_blocking());
    nexus::refresh_nexus_account_impl(&state).unwrap();
    (state, events)
}

/// The row once it is over (landed, failed or cancelled).
fn settled(state: &AppState, id: &str) -> DownloadView {
    let row = || {
        get_downloads_impl(state)
            .into_iter()
            .find(|row| row.id == id)
            .expect("the row is listed")
    };
    wait_for(Duration::from_secs(30), "the download settles", || {
        row().phase.is_over()
    });
    row()
}

/// The names in the mods folder that are not the bench's own three.
fn new_files(bench: &Bench) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(&bench.mods)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| {
            ![
                "Vernworth Cloaks Collection.zip",
                "Lantern Glow Recolor.zip",
                support::BANTER_ZIP,
            ]
            .contains(&name.as_str())
        })
        .collect();
    names.sort();
    names
}

/// Nothing a secret could hide in reached the window: the views, every
/// event payload.
fn assert_no_secret(state: &AppState, events: &RecordedEvents) {
    let views = serde_json::to_string(&get_downloads_impl(state)).unwrap();
    let heard = serde_json::to_string(&events.all()).unwrap();
    for secret in [KEY, GRANT, "SIGNED-BY-THE-CDN", EMAIL] {
        assert!(!views.contains(secret), "{secret} in the views: {views}");
        assert!(!heard.contains(secret), "{secret} in the events: {heard}");
    }
}

/// The CDN never sees the key, the grant or the stone's API identity
/// beyond its own name.
fn assert_cdn_saw_no_key(cdn: &Cdn) {
    for request in cdn.seen() {
        assert_eq!(request.header("apikey"), None, "{request:?}");
        assert!(
            !request.path.contains(KEY) && !request.path.contains(GRANT),
            "{}",
            request.path
        );
        assert_eq!(request.header("Application-Name"), Some("Riftstone"));
        assert_eq!(request.header("Referer"), None);
    }
}

#[test]
fn a_premium_download_lands_under_its_nexus_name_and_links_itself() {
    let update = lantern_update();
    let cdn = Cdn::start({
        let update = update.clone();
        move |_| Delivery::bytes(&update)
    });
    let api = StandIn::start(nexus_api(Scene {
        premium: true,
        size: update.len() as u64,
        link: cdn.link(CDN_PATH),
        hash_names: Some((MOD, FILE)),
        ..Scene::default()
    }));
    let bench = Bench::new();
    let (state, events) = open(&bench, &api);
    events.clear();

    let row = downloads::start(&state, MOD, FILE, None).unwrap();
    assert_eq!(row.phase, DownloadPhase::Queued, "premium: queued at once");
    assert_eq!(row.mod_name.as_deref(), Some("Lantern Glow Recolor"));
    assert_eq!(row.file_name.as_deref(), Some("Lantern Glow Recolor"));
    assert_eq!(row.version.as_deref(), Some("2.2"));
    assert_eq!(row.bytes_total, Some(update.len() as u64));
    assert_eq!(row.nexus_page_url, None);
    assert_eq!(
        downloads::start(&state, MOD, FILE, None).unwrap().id,
        row.id,
        "the same file on its way answers instead of a second download"
    );

    let landed = settled(&state, &row.id);
    assert_eq!(landed.phase, DownloadPhase::Landed, "{landed:?}");
    assert_eq!(landed.landed_as.as_deref(), Some(ARCHIVE));
    assert!(!landed.already_had);
    assert_eq!(landed.bytes_done, update.len() as u64);
    assert!(landed.finished_ms.is_some());
    let package_id = rift_core::PackageId::for_archive(ARCHIVE, update.len() as u64);
    assert_eq!(landed.package_id.as_deref(), Some(package_id.0.as_str()));
    assert_eq!(std::fs::read(bench.mods.join(ARCHIVE)).unwrap(), update);
    assert_eq!(
        new_files(&bench),
        vec![ARCHIVE.to_string()],
        "no partial file left"
    );

    // The library finds it, already linked to the exact file.
    wait_for(
        Duration::from_secs(20),
        "the scan finds the archive",
        || state.shelf().package(&package_id).is_some(),
    );
    let rows = nexus::get_updates_impl(&state).rows;
    let linked = rows
        .iter()
        .find(|row| row.package_id == package_id.0)
        .and_then(|row| row.link.clone())
        .expect("the download linked its package");
    assert_eq!(
        (
            linked.mod_id,
            linked.file_id,
            linked.group_id,
            linked.source
        ),
        (MOD, Some(FILE), Some(31), LinkSource::Download)
    );
    assert_eq!(linked.version.as_deref(), Some("2.2"));
    assert_eq!(linked.uploaded_ms, Some(UPLOADED_S * 1000));
    assert_eq!(linked.sqid.as_deref(), Some("Rk4vLmN2Q"));

    // The window heard each phase, the bytes, and the link.
    assert!(
        events.named(DOWNLOADS_CHANGED).len() >= 4,
        "queued, linking, downloading, verifying, landed"
    );
    let progress = events.named(DOWNLOAD_PROGRESS);
    let last = progress.last().expect("the bytes were told");
    assert_eq!(last["bytesDone"], json!(update.len()));
    assert_eq!(last["id"], json!(row.id));
    assert!(!events.named(UPDATES_CHANGED).is_empty());

    // The CDN was asked once, without the key; the API with it.
    assert_eq!(cdn.seen().len(), 1);
    assert_cdn_saw_no_key(&cdn);
    let link_asked = api
        .seen()
        .into_iter()
        .find(|request| request.path.contains("download_link.json"))
        .expect("the download link was asked for");
    assert_eq!(link_asked.header("apikey"), Some(KEY));
    assert!(
        !link_asked.path.contains("key="),
        "premium: no grant: {}",
        link_asked.path
    );
    assert_no_secret(&state, &events);
}

#[test]
fn a_free_download_waits_for_the_site_link_and_resumes_on_it() {
    let update = lantern_update();
    let cdn = Cdn::start({
        let update = update.clone();
        move |_| Delivery::bytes(&update)
    });
    let api = StandIn::start(nexus_api(Scene {
        premium: false,
        size: update.len() as u64,
        link: cdn.link(CDN_PATH),
        hash_names: None,
        ..Scene::default()
    }));
    let bench = Bench::new();
    let (state, events) = open(&bench, &api);

    let row = downloads::start(&state, MOD, FILE, Some("00000000000000aa".into())).unwrap();
    assert_eq!(
        row.phase,
        DownloadPhase::Waiting,
        "free: waits for the site's link"
    );
    assert_eq!(
        row.nexus_page_url.as_deref(),
        Some("https://www.nexusmods.com/dragonsdogma2/mods/2207?tab=files&file_id=9001&nmm=1")
    );
    assert_eq!(row.for_package_id.as_deref(), Some("00000000000000aa"));
    std::thread::sleep(Duration::from_millis(100));
    assert!(
        !api.seen()
            .iter()
            .any(|r| r.path.contains("download_link.json")),
        "nothing is asked before the link arrives"
    );
    assert_eq!(get_downloads_impl(&state)[0].phase, DownloadPhase::Waiting);

    let expires = riftstone_lib::persist::now_ms() / 1000 + 3600;
    let received = nxm::handle_nxm(
        &state,
        &format!("nxm://dragonsdogma2/mods/{MOD}/files/{FILE}?key={GRANT}&expires={expires}&user_id={USER}"),
    );
    assert!(received.accepted, "{received:?}");
    assert_eq!(
        received.download_id.as_deref(),
        Some(row.id.as_str()),
        "the waiting one resumed"
    );
    assert_eq!((received.mod_id, received.file_id), (Some(MOD), Some(FILE)));
    let heard = events.named(NXM_RECEIVED);
    assert_eq!(heard.len(), 1);
    assert_eq!(heard[0]["accepted"], json!(true));
    assert_eq!(heard[0]["downloadId"], json!(row.id));
    assert!(heard[0]["refusal"].is_null());

    let landed = settled(&state, &row.id);
    assert_eq!(landed.phase, DownloadPhase::Landed, "{landed:?}");
    assert_eq!(landed.landed_as.as_deref(), Some(ARCHIVE));
    assert_eq!(landed.nexus_page_url, None);
    assert_eq!(
        get_downloads_impl(&state).len(),
        1,
        "resumed, not started twice"
    );

    let asked = api
        .seen()
        .into_iter()
        .find(|request| request.path.contains("download_link.json"))
        .unwrap();
    assert!(
        asked.path.contains(&format!("key={GRANT}"))
            && asked.path.contains(&format!("expires={expires}")),
        "the grant rode to the API: {}",
        asked.path
    );
    assert_cdn_saw_no_key(&cdn);
    assert_no_secret(&state, &events);

    // A second link for the same file, now that it landed, starts anew.
    let again = nxm::handle_nxm(
        &state,
        &format!("nxm://dragonsdogma2/mods/{MOD}/files/{FILE}?key={GRANT}&expires={expires}"),
    );
    assert!(again.accepted);
    assert_ne!(again.download_id.as_deref(), Some(row.id.as_str()));
    let had = settled(&state, again.download_id.as_deref().unwrap());
    assert_eq!(had.phase, DownloadPhase::Landed);
    assert!(
        had.already_had,
        "the same bytes were there: nothing written twice"
    );
    assert_eq!(had.landed_as.as_deref(), Some(ARCHIVE));
    assert_eq!(new_files(&bench), vec![ARCHIVE.to_string()]);
}

/// Every file of a folder with its bytes ([`Bench::snapshot`]).
type Folder = BTreeMap<String, Vec<u8>>;

/// Start a premium download over `delivery` and return the settled row,
/// with the mods folder before and after.
fn premium_run(
    delivery: Delivery,
    stated: u64,
    hash_names: Option<(u64, u64)>,
) -> (DownloadView, Folder, Folder) {
    let cdn = Cdn::start(move |_| delivery.clone());
    let api = StandIn::start(nexus_api(Scene {
        premium: true,
        size: stated,
        link: cdn.link(CDN_PATH),
        hash_names,
        ..Scene::default()
    }));
    let bench = Bench::new();
    let (state, events) = open(&bench, &api);
    let before = Bench::snapshot(&bench.mods);
    let row = downloads::start(&state, MOD, FILE, None).unwrap();
    let row = settled(&state, &row.id);
    // The worker removes the partial file before it says so.
    let after = Bench::snapshot(&bench.mods);
    assert_cdn_saw_no_key(&cdn);
    assert_no_secret(&state, &events);
    (row, before, after)
}

fn reason(row: &DownloadView) -> Value {
    serde_json::to_value(row.failure.as_ref().expect("a failure says why")).unwrap()
}

/// A settled premium download in `scene` (its link pointed at a CDN serving
/// `delivery`): the row, the mods folder before and after, the spine and its
/// bench (which holds the folders).
fn premium_in(
    delivery: Delivery,
    scene: Scene,
) -> (DownloadView, Folder, Folder, Arc<AppState>, Bench) {
    let cdn = Cdn::start(move |_| delivery.clone());
    let api = StandIn::start(nexus_api(Scene {
        premium: true,
        link: cdn.link(CDN_PATH),
        ..scene
    }));
    let bench = Bench::new();
    let (state, events) = open(&bench, &api);
    let before = Bench::snapshot(&bench.mods);
    let row = downloads::start(&state, MOD, FILE, None).unwrap();
    let row = settled(&state, &row.id);
    let after = Bench::snapshot(&bench.mods);
    assert_cdn_saw_no_key(&cdn);
    assert_no_secret(&state, &events);
    (row, before, after, state, bench)
}

#[test]
fn a_whole_download_lands_on_its_size_when_the_hash_lookup_cannot_be_asked() {
    // Every stated size met; the anonymous MD5 lookup is cooling down for an
    // hour (a 429 the update check shares). The bytes are kept.
    let update = lantern_update();
    let (row, before, after, _, _bench) = premium_in(
        Delivery::bytes(&update),
        Scene {
            size: update.len() as u64,
            lookup_refused: true,
            ..Scene::default()
        },
    );
    assert_eq!(row.phase, DownloadPhase::Landed, "{row:?}");
    assert_eq!(after[ARCHIVE], update);
    assert_eq!(
        after.len(),
        before.len() + 1,
        "the archive, no partial file"
    );
}

#[test]
fn a_download_nobody_sized_lands_only_on_a_hash_nexus_knows() {
    let update = lantern_update();
    let nowhere = |hash_names, lookup_refused| Scene {
        size: update.len() as u64,
        stated: Stated::Nowhere,
        hash_names,
        lookup_refused,
        ..Scene::default()
    };
    // No size from v2, v1 or the server, and a hash Nexus does not know:
    // nothing vouches for the bytes.
    let (row, before, after, _, _bench) =
        premium_in(Delivery::unsized_bytes(&update), nowhere(None, false));
    assert_eq!(row.phase, DownloadPhase::Failed, "{row:?}");
    assert_eq!(reason(&row)["kind"], "nexus");
    assert_eq!(after, before, "nothing kept");

    // Nexus names the file for its hash: that vouches for it.
    let (row, _, after, _, _bench) = premium_in(
        Delivery::unsized_bytes(&update),
        nowhere(Some((MOD, FILE)), false),
    );
    assert_eq!(row.phase, DownloadPhase::Landed, "{row:?}");
    assert_eq!(after[ARCHIVE], update);

    // Nobody sized it and the lookup could not be asked: nothing vouches.
    let (row, before, after, _, _bench) =
        premium_in(Delivery::unsized_bytes(&update), nowhere(None, true));
    assert_eq!(row.phase, DownloadPhase::Failed, "{row:?}");
    assert_eq!(reason(&row)["detail"]["reason"], "rateLimited");
    assert_eq!(after, before);
}

#[test]
fn files_json_states_the_size_when_v2_does_not() {
    let update = lantern_update();
    let v1_only = |size: u64, hash_names| Scene {
        size,
        stated: Stated::V1Only,
        hash_names,
        ..Scene::default()
    };
    // v2 states no size, the server none either; v1's is not what arrives.
    let (row, before, after, _, _bench) = premium_in(
        Delivery::unsized_bytes(&update),
        v1_only(update.len() as u64 + 7, Some((MOD, FILE))),
    );
    assert_eq!(row.phase, DownloadPhase::Failed, "{row:?}");
    assert_eq!(reason(&row)["detail"]["reason"], "sizeMismatch");
    assert_eq!(after, before);

    // v1's size met: it vouches, so a hash Nexus does not know lands.
    let (row, _, after, _, _bench) = premium_in(
        Delivery::unsized_bytes(&update),
        v1_only(update.len() as u64, None),
    );
    assert_eq!(row.phase, DownloadPhase::Landed, "{row:?}");
    assert_eq!(row.bytes_total, Some(update.len() as u64));
    assert_eq!(after[ARCHIVE], update);
}

#[test]
fn a_file_that_is_no_archive_lands_without_a_package_or_a_link() {
    // A raw pak: the scan lists only archives and folders, so no package
    // will ever answer to a link made for it.
    const RAW: &str = "Lantern Glow Recolor.pak";
    let pak = b"KPKA a raw pak with no archive around it".to_vec();
    let (row, before, after, state, bench) = premium_in(
        Delivery::bytes(&pak),
        Scene {
            size: pak.len() as u64,
            archive: Some(RAW),
            hash_names: Some((MOD, FILE)),
            ..Scene::default()
        },
    );
    assert_eq!(row.phase, DownloadPhase::Landed, "{row:?}");
    assert_eq!(row.landed_as.as_deref(), Some(RAW), "under its own name");
    assert_eq!(after[RAW], pak);
    assert_eq!(after.len(), before.len() + 1);
    assert_eq!(
        row.package_id, None,
        "no package: nothing to show in the Rift"
    );
    let orphan = rift_core::PackageId::for_archive(RAW, pak.len() as u64);
    let store = std::fs::read_to_string(
        riftstone_lib::persist::DataDirs::under(&bench.data_home()).nexus_file(),
    )
    .unwrap_or_default();
    assert!(!store.contains(&orphan.0), "no link for it: {store}");
    assert!(nexus::get_updates_impl(&state)
        .rows
        .iter()
        .all(|row| row.package_id != orphan.0));
}

#[test]
fn a_cut_off_body_leaves_nothing_in_the_mods_folder() {
    let update = lantern_update();
    let half = update[..update.len() / 2].to_vec();
    let (row, before, after) = premium_run(
        Delivery {
            declared: Some(update.len()),
            ..Delivery::bytes(&half)
        },
        update.len() as u64,
        Some((MOD, FILE)),
    );
    assert_eq!(row.phase, DownloadPhase::Failed, "{row:?}");
    let failure = reason(&row);
    assert_eq!(failure["kind"], "nexus", "{failure}");
    assert_eq!(failure["detail"]["reason"], "offline", "{failure}");
    assert_eq!(after, before, "nothing left, nothing touched");
    assert_eq!(row.landed_as, None);
}

#[test]
fn bytes_of_another_size_or_another_file_are_never_kept() {
    let update = lantern_update();
    // Nexus states one size, the server sends another.
    let (row, before, after) = premium_run(
        Delivery::bytes(&update),
        update.len() as u64 + 7,
        Some((MOD, FILE)),
    );
    assert_eq!(row.phase, DownloadPhase::Failed);
    assert_eq!(reason(&row)["detail"]["reason"], "sizeMismatch");
    assert_eq!(after, before);

    // The right size, but the MD5 names another file of the mod.
    let (row, before, after) = premium_run(
        Delivery::bytes(&update),
        update.len() as u64,
        Some((MOD, FILE + 1)),
    );
    assert_eq!(row.phase, DownloadPhase::Failed);
    assert_eq!(reason(&row)["detail"]["reason"], "notTheFile");
    assert_eq!(after, before);

    // A hash Nexus does not know yet passes on the size alone.
    let (row, _, after) = premium_run(Delivery::bytes(&update), update.len() as u64, None);
    assert_eq!(row.phase, DownloadPhase::Landed, "{row:?}");
    assert_eq!(after[ARCHIVE], update);
}

#[test]
fn a_cancel_mid_download_leaves_nothing_behind() {
    // 64 pieces 60 ms apart: the whole body would take about 4 s to arrive.
    let slow = vec![7u8; 512 * 1024];
    let cdn = Cdn::start({
        let slow = slow.clone();
        move |_| Delivery {
            pace: Some((8 * 1024, Duration::from_millis(60))),
            ..Delivery::bytes(&slow)
        }
    });
    let api = StandIn::start(nexus_api(Scene {
        premium: true,
        size: slow.len() as u64,
        link: cdn.link(CDN_PATH),
        hash_names: Some((MOD, FILE)),
        ..Scene::default()
    }));
    let bench = Bench::new();
    let (state, events) = open(&bench, &api);
    let before = Bench::snapshot(&bench.mods);
    let row = downloads::start(&state, MOD, FILE, None).unwrap();
    let part = bench.mods.join(downloads::part_name(&row.id));
    wait_for(Duration::from_secs(20), "bytes arrive", || {
        get_downloads_impl(&state)[0].bytes_done > 0
    });
    assert!(
        part.is_file(),
        "the partial file is written under its hidden name"
    );

    download_cancel(&state, &row.id);
    let cancelled = get_downloads_impl(&state)[0].clone();
    assert_eq!(
        cancelled.phase,
        DownloadPhase::Cancelled,
        "cancelled at once"
    );
    // The download stops at its next read, long before the body could end.
    wait_for(
        Duration::from_millis(1500),
        "the partial file goes at once",
        || !part.exists(),
    );
    // Let the worker finish its run, then nothing is there.
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(Bench::snapshot(&bench.mods), before);
    assert_eq!(
        get_downloads_impl(&state)[0].phase,
        DownloadPhase::Cancelled
    );

    // Over is over: a second cancel is refused, and the list forgets it.
    let again = downloads::cancel(&state, &row.id).unwrap_err();
    assert!(
        matches!(again, rift_core::RiftError::Refused { .. }),
        "{again:?}"
    );
    assert!(matches!(
        downloads::cancel(&state, "no-such-download"),
        Err(rift_core::RiftError::Refused { .. })
    ));
    downloads::clear(&state);
    assert!(get_downloads_impl(&state).is_empty());
    assert_no_secret(&state, &events);
}

fn download_cancel(state: &AppState, id: &str) {
    downloads::cancel(state, id).unwrap();
}

#[test]
fn a_waiting_download_cancels_without_asking_anything() {
    let api = StandIn::start(nexus_api(Scene {
        premium: false,
        size: 10,
        link: "http://127.0.0.1:9/never".into(),
        hash_names: None,
        ..Scene::default()
    }));
    let bench = Bench::new();
    let (state, events) = open(&bench, &api);
    let row = downloads::start(&state, MOD, FILE, None).unwrap();
    assert_eq!(row.phase, DownloadPhase::Waiting);
    events.clear();
    downloads::cancel(&state, &row.id).unwrap();
    let row = get_downloads_impl(&state)[0].clone();
    assert_eq!(row.phase, DownloadPhase::Cancelled);
    assert_eq!(row.nexus_page_url, None);
    assert_eq!(events.named(DOWNLOADS_CHANGED).len(), 1);
    // A link for it now starts a new download instead of a cancelled one.
    let expires = riftstone_lib::persist::now_ms() / 1000 + 3600;
    let received = nxm::handle_nxm(
        &state,
        &format!("nxm://dragonsdogma2/mods/{MOD}/files/{FILE}?key={GRANT}&expires={expires}"),
    );
    assert!(received.accepted);
    assert_ne!(received.download_id.as_deref(), Some(row.id.as_str()));
}

#[test]
fn a_cold_start_link_is_handled_once_the_spine_is_awake() {
    let api = StandIn::start(nexus_api(Scene {
        premium: false,
        size: 10,
        link: "http://127.0.0.1:9/never".into(),
        hash_names: None,
        ..Scene::default()
    }));
    let bench = Bench::new();
    let (state, events) = bench.open_keyed(bench.config_at(&api), KEY);
    let expires = riftstone_lib::persist::now_ms() / 1000 + 3600;
    // Windows started the stone for a link another account made.
    let args: Vec<String> = vec![
        bench.exe.to_string_lossy().into_owned(),
        format!(
            "\"nxm://dragonsdogma2/mods/{MOD}/files/{FILE}?key={GRANT}&expires={expires}&user_id=5150\""
        ),
    ];
    state.queue_launch_links(nxm::nxm_arguments(&args));
    assert!(
        events.named(NXM_RECEIVED).is_empty(),
        "nothing before the wake"
    );
    state.wake();
    // The window reads its first frame: it listens now.
    nxm::window_listens(&state);
    wait_for(
        Duration::from_secs(20),
        "the launch link is handled",
        || !events.named(NXM_RECEIVED).is_empty(),
    );
    let heard = &events.named(NXM_RECEIVED)[0];
    // Refused as another account's: the account was validated first.
    assert_eq!(heard["accepted"], json!(false), "{heard}");
    assert_eq!(
        heard["refusal"]["detail"]["reason"], "linkNotYours",
        "{heard}"
    );
    assert!(state.take_launch_links().is_empty(), "handled once");
    assert_no_secret(&state, &events);
}

#[test]
fn every_nxm_refusal_says_why_and_starts_nothing() {
    let api = StandIn::start(nexus_api(Scene {
        premium: false,
        size: 10,
        link: "http://127.0.0.1:9/never".into(),
        hash_names: None,
        ..Scene::default()
    }));
    let now_s = riftstone_lib::persist::now_ms() / 1000;
    let file_link =
        |extra: &str| format!("nxm://dragonsdogma2/mods/{MOD}/files/{FILE}?key={GRANT}&{extra}");

    // No key stored: noKey.
    let bench = Bench::new();
    let (keyless, events) = bench.open(bench.config_at(&api));
    let refused = nxm::handle_nxm(&keyless, &file_link(&format!("expires={}", now_s + 600)));
    assert!(!refused.accepted);
    let refusal = serde_json::to_value(refused.refusal.unwrap()).unwrap();
    assert_eq!(refusal["detail"]["reason"], "noKey", "{refusal}");
    assert_eq!(events.named(NXM_RECEIVED).len(), 1);
    assert!(get_downloads_impl(&keyless).is_empty());

    // A key stored, the account validated (user 4242).
    let bench = Bench::new();
    let (state, events) = open(&bench, &api);
    let cases: Vec<(String, Option<u64>, &str, &str)> = vec![
        (
            file_link(&format!("expires={}", now_s.saturating_sub(10))),
            Some(MOD),
            "nexus",
            "linkExpired",
        ),
        (
            file_link(&format!("expires={}&user_id=5150", now_s + 600)),
            Some(MOD),
            "nexus",
            "linkNotYours",
        ),
        (
            format!(
                "nxm://skyrimspecialedition/mods/{MOD}/files/{FILE}?key={GRANT}&expires={}",
                now_s + 600
            ),
            None,
            "refused",
            "",
        ),
        (
            "nxm://dragonsdogma2/collections/vernworth-set/revisions/3".into(),
            None,
            "refused",
            "",
        ),
        (
            format!("nxm://dragonsdogma2/mods/{MOD}/files/{FILE}"),
            Some(MOD),
            "refused",
            "",
        ),
        (
            "nxm://dragonsdogma2/mods/0/files/1".into(),
            None,
            "refused",
            "",
        ),
    ];
    for (link, mod_id, kind, why) in &cases {
        let received = nxm::handle_nxm(&state, link);
        assert!(!received.accepted, "{link}");
        assert_eq!(received.mod_id, *mod_id, "{link}");
        assert_eq!(received.download_id, None);
        let refusal = serde_json::to_value(received.refusal.expect("a refusal says why")).unwrap();
        assert_eq!(refusal["kind"], *kind, "{link}: {refusal}");
        if !why.is_empty() {
            assert_eq!(refusal["detail"]["reason"], *why, "{link}: {refusal}");
        }
        let sentence = refusal["detail"]
            .get("detail")
            .or_else(|| refusal["detail"].get("reason"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        assert!(sentence.ends_with('.'), "{link}: a sentence: {sentence}");
    }
    assert!(
        get_downloads_impl(&state).is_empty(),
        "no refusal starts a download"
    );
    assert_eq!(events.named(NXM_RECEIVED).len(), cases.len());

    // A key Nexus refused: keyRejected at once, nothing queued to fail later.
    let rejecting = StandIn::start(|request| {
        if request.path.contains("validate.json") {
            return status(401);
        }
        status(404)
    });
    let bench = Bench::new();
    let (refused_key, _) = bench.open_keyed(bench.config_at(&rejecting), KEY);
    nexus::refresh_nexus_account_impl(&refused_key).unwrap();
    let received = nxm::handle_nxm(
        &refused_key,
        &file_link(&format!("expires={}", now_s + 600)),
    );
    let refusal = serde_json::to_value(received.refusal.unwrap()).unwrap();
    assert_eq!(refusal["detail"]["reason"], "keyRejected", "{refusal}");
    assert!(get_downloads_impl(&refused_key).is_empty());

    // nxm://premium validates the account again, off the caller's thread.
    let validations = || {
        api.seen()
            .iter()
            .filter(|r| r.path.contains("validate.json"))
            .count()
    };
    let before = validations();
    let premium = nxm::handle_nxm(&state, "nxm://premium");
    assert!(!premium.accepted && premium.refusal.is_none() && premium.mod_id.is_none());
    wait_for(
        Duration::from_secs(10),
        "the account is validated again",
        || validations() > before,
    );
    assert_no_secret(&state, &events);
}
