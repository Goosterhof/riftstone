//! Every command through Tauri's own IPC dispatch (the mock runtime — no
//! window, no webview), invoked with exactly the names and camelCase argument
//! keys `src/services/rift.ts` sends, answers decoded into the wire types. A
//! renamed command or argument fails here, not in the investor's window.

mod support;

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{mock_builder, mock_context, noop_assets, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::Manager;

use riftstone_lib::dto::{
    CheckAction, GameSummary, InstallPlanView, InstallResultView, InstalledView, LabelFacets,
    LaunchPlanView, LaunchReport, LaunchRoute, LibraryView, Md5Result, NexusBrowsePage,
    NexusFindings, NexusKeyStanding, NexusModFiles, NexusStandings, NexusSummary, NxmHandlerState,
    NxmHandlerView, OptionDetail, Overview, OwnerKind, PackageDetail, PackageSummary,
    ParkedRestoreReport, SearchResult, Settings, UninstallPlanView, UninstallResultView, UpdateRow,
    UpdatesView, VeilMode,
};
use riftstone_lib::events::{GAME_CHANGED, LIBRARY_CHANGED};
use riftstone_lib::persist::DataDirs;
use riftstone_lib::play::Launched;
use riftstone_lib::state::AppState;
use support::{Answer, Bench, StandIn, ERMINE_MESH};

/// The window's own origin: what Tauri serves the bundled frontend from, and
/// the only origin the default capability lets call app commands. Windows
/// (WebView2) serves it as `http://tauri.localhost`; a call from
/// `tauri://localhost` there is a remote caller and is refused.
const LOCAL_ORIGIN: &str = if cfg!(windows) {
    "http://tauri.localhost"
} else {
    "tauri://localhost"
};

struct Window {
    webview: tauri::WebviewWindow<tauri::test::MockRuntime>,
}

impl Window {
    /// `invoke(cmd, args)` as the window's service layer calls it.
    fn invoke(&self, cmd: &str, args: Value) -> Result<Value, Value> {
        tauri::test::get_ipc_response(
            &self.webview,
            InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: LOCAL_ORIGIN.parse().unwrap(),
                body: InvokeBody::Json(args),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .map(|body| body.deserialize::<Value>().unwrap())
    }

    /// Invoke and decode the answer into its wire type.
    fn call<T: DeserializeOwned>(&self, cmd: &str, args: Value) -> T {
        let answer = self
            .invoke(cmd, args)
            .unwrap_or_else(|err| panic!("{cmd} was refused: {err}"));
        serde_json::from_value(answer.clone()).unwrap_or_else(|err| {
            panic!("{cmd} answered a shape the window cannot read ({err}): {answer}")
        })
    }
}

/// The mock app with every command registered and the spine managed, and its
/// window. Keep the app alive as long as the window is used.
fn mock_window(state: &Arc<AppState>) -> (tauri::App<tauri::test::MockRuntime>, Window) {
    let app = riftstone_lib::with_commands(mock_builder())
        .build(mock_context(noop_assets()))
        .expect("the mock app builds");
    app.manage(Arc::clone(state));
    let window = Window {
        webview: tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap(),
    };
    (app, window)
}

#[test]
fn every_command_answers_the_window_by_its_invoke_name_and_argument_keys() {
    let bench = Bench::new();
    let (state, events) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    assert!(state.scan_blocking());

    let (_app, window) = mock_window(&state);

    // -- the home frame --------------------------------------------------------
    let home: Overview = window.call("get_overview", json!({}));
    assert_eq!(home.library.packages, 3);
    assert_eq!(home.mods.path, bench.mods.to_string_lossy());
    let settings: Settings = window.call("get_settings", json!({}));
    assert!(settings.auto_check_updates);
    let settings: Settings = window.call(
        "set_settings",
        json!({ "settings": { "veil": "adult", "autoCheckUpdates": false } }),
    );
    assert_eq!(settings.veil, VeilMode::Adult);
    assert!(!settings.auto_check_updates);
    let troubled: Vec<PackageSummary> = window.call("get_troubled_packages", json!({}));
    assert!(troubled.is_empty());

    // -- the library -------------------------------------------------------------
    let library: LibraryView = window.call("get_library", json!({}));
    assert_eq!(library.packages.len(), 3);
    assert!(library.mods.exists);
    assert_eq!(
        window.invoke("open_mods_folder", json!({})),
        Ok(Value::Null)
    );
    assert_eq!(
        bench.launcher.seen(),
        vec![Launched::Folder(bench.mods.clone())],
        "shown Rust-side, through the launcher"
    );
    let hits: Vec<SearchResult> = window.call(
        "search",
        json!({ "query": { "text": "ermine", "filters": { "installed": null, "includeMenuOnly": false, "categories": [], "tags": [] }, "limit": 0 } }),
    );
    let ermine = hits[0].option_id.clone();
    let package_id = hits[0].package_id.clone();
    assert_eq!(hits[0].nexus_category, None, "no check has run");
    assert!(hits[0].nexus_tags.is_empty() && !hits[0].adult);
    // A v0.2 query (a root filter) still reads; the filter is ignored.
    let lantern = window
        .call::<Vec<SearchResult>>(
            "search",
            json!({ "query": { "text": "lantern", "filters": { "installed": null, "rootIds": ["custom-1"], "includeMenuOnly": false, "categories": [], "tags": [] }, "limit": 5 } }),
        )[0]
        .option_id
        .clone();
    let by_label: Vec<SearchResult> = window.call(
        "search",
        json!({ "query": { "text": "", "filters": { "installed": null, "includeMenuOnly": false, "categories": ["Armour"], "tags": ["Cloaks"] }, "limit": 0 } }),
    );
    assert!(
        by_label.is_empty(),
        "a label filter hides every unlabeled package"
    );
    let detail: OptionDetail = window.call("get_option", json!({ "optionId": ermine }));
    assert_eq!(detail.option.name, "Ermine Cape");
    assert!(detail.labels.is_none());
    let package: PackageDetail = window.call("get_package", json!({ "packageId": package_id }));
    assert_eq!(package.package.option_count, 3);
    let thumb: Option<String> = window.call(
        "get_thumbnail",
        json!({ "optionId": ermine, "size": "card" }),
    );
    assert!(thumb.unwrap().starts_with("data:image/jpeg;base64,"));

    // -- errors cross as { kind, detail } ------------------------------------------
    let missing = window
        .invoke("get_option", json!({ "optionId": "nope#nothing" }))
        .unwrap_err();
    assert_eq!(
        missing,
        json!({ "kind": "entryNotFound", "detail": { "entry": "nope#nothing" } })
    );
    assert!(
        window
            .invoke("get_option", json!({ "option_id": ermine }))
            .is_err(),
        "the window's camelCase key is the only one that maps"
    );

    // -- your world ------------------------------------------------------------------
    let plan: InstallPlanView = window.call("plan_install", json!({ "optionId": ermine }));
    assert!(plan.refused.is_none());
    let landed: InstallResultView = window.call("install", json!({ "optionId": ermine }));
    assert_eq!(landed.files_written, 1);
    let _: InstallResultView = window.call("install", json!({ "optionId": lantern }));
    let world: InstalledView = window.call("get_installed", json!({}));
    assert_eq!(world.installs.len(), 2);
    assert_eq!(
        window.invoke("reorder_paks", json!({ "order": [lantern] })),
        Ok(Value::Null)
    );
    let back: UninstallPlanView = window.call("plan_uninstall", json!({ "optionId": ermine }));
    assert_eq!(back.removals.len(), 1);
    let gone: UninstallResultView = window.call("uninstall", json!({ "optionId": ermine }));
    assert_eq!(gone.files_removed, 1);
    assert_eq!(
        window.invoke(
            "set_reframework_switch",
            json!({ "switch": "pakDirectory", "on": true })
        ),
        Ok(Value::Null)
    );
    let home: Overview = window.call("get_overview", json!({}));
    assert_eq!(home.reframework.unwrap().pak_directory, Some(true));
    let game: GameSummary = window.call(
        "set_game_root",
        json!({ "path": bench.game.to_string_lossy() }),
    );
    assert_eq!(game.root, bench.game.to_string_lossy());
    assert_eq!(game.launch, LaunchRoute::Exe);
    let refused = window
        .invoke(
            "set_game_root",
            json!({ "path": bench.mods.to_string_lossy() }),
        )
        .unwrap_err();
    assert_eq!(refused["kind"], "gameNotFound");

    // -- Play (v0.3) ---------------------------------------------------------------------
    let plan: LaunchPlanView = window.call("plan_launch", json!({}));
    assert_eq!(plan.launch, LaunchRoute::Exe);
    assert!(plan.refused.is_none(), "{plan:?}");
    let report: LaunchReport = window.call("launch_game", json!({}));
    assert_eq!(report.launch, LaunchRoute::Exe);
    assert!(report.launched_ms > 0);
    assert!(
        matches!(bench.launcher.seen().last(), Some(Launched::Game(_))),
        "started through the launcher: {:?}",
        bench.launcher.seen()
    );
    // The bench's launch started the game: a second Play is refused in words.
    let again = window.invoke("launch_game", json!({})).unwrap_err();
    assert_eq!(again["kind"], "refused", "{again}");
    bench.game_watch.set(false);
    state.sample_game();

    // -- files kept aside (v0.2) --------------------------------------------------------
    // A file that was there before the cape; the cape backs it up; another
    // tool rewrites the cape's file; the uninstall keeps the rewrite in place
    // and parks the original.
    let cape_path = bench.game.join(ERMINE_MESH);
    std::fs::create_dir_all(cape_path.parent().unwrap()).unwrap();
    std::fs::write(&cape_path, b"the cape that was there before").unwrap();
    let _: InstallResultView = window.call("install", json!({ "optionId": ermine }));
    std::fs::write(&cape_path, b"another tool's rewrite").unwrap();
    let kept: UninstallResultView = window.call("uninstall", json!({ "optionId": ermine }));
    assert_eq!((kept.files_kept, kept.parked), (1, 1));
    let home: Overview = window.call("get_overview", json!({}));
    let review = home
        .checks
        .iter()
        .find(|c| c.id == "parkedBackups")
        .expect("the overview says a file is kept aside");
    assert_eq!(review.action, Some(CheckAction::ReviewParked));
    let world: InstalledView = window.call("get_installed", json!({}));
    assert_eq!(world.parked.len(), 1);
    let parked = &world.parked[0];
    assert_eq!(parked.game_path, ERMINE_MESH);
    assert_eq!(parked.owner_kind, OwnerKind::Unowned);
    assert!(parked.occupied, "the rewrite sits at its path");
    // Each act on a kept file changes the game folder or its record, so the
    // window hears rift://game-changed once; a refusal changed nothing and
    // stays silent.
    let game_changed = || events.named(GAME_CHANGED).len();
    let heard = game_changed();
    let back: ParkedRestoreReport = window.call("restore_parked", json!({ "store": parked.store }));
    assert!(back.restored);
    assert_eq!(
        std::fs::read(&cape_path).unwrap(),
        b"the cape that was there before"
    );
    assert_eq!(
        game_changed(),
        heard + 1,
        "restore_parked emits game-changed"
    );
    let swapped = back
        .swapped_out
        .expect("the rewrite was kept aside in turn");
    assert_eq!(
        window.invoke("discard_parked", json!({ "store": swapped })),
        Ok(Value::Null)
    );
    assert_eq!(
        game_changed(),
        heard + 2,
        "discard_parked emits game-changed"
    );
    let world: InstalledView = window.call("get_installed", json!({}));
    assert!(world.parked.is_empty());
    // The handles the window still holds are out of date now: refused, in
    // words that name the game path (never the store's inner path), and
    // without a sound.
    for (command, store, names) in [
        ("restore_parked", parked.store.as_str(), Some(ERMINE_MESH)),
        ("discard_parked", swapped.as_str(), Some(ERMINE_MESH)),
        ("restore_parked", "backups/999/nothing", None),
        ("discard_parked", "backups/999/nothing", None),
    ] {
        let stale = window
            .invoke(command, json!({ "store": store }))
            .unwrap_err();
        assert_eq!(stale["kind"], "refused", "{command} {store}: {stale}");
        let reason = stale["detail"]["reason"].as_str().unwrap_or_default();
        assert!(
            !reason.contains("backups/"),
            "{command} {store}: the refusal shows no store path: {reason}"
        );
        assert!(
            reason.contains("PARKED"),
            "{command} {store}: it says where to look again: {reason}"
        );
        if let Some(game_path) = names {
            assert!(
                reason.contains(game_path),
                "{command} {store}: it names the game path: {reason}"
            );
        }
    }
    assert_eq!(game_changed(), heard + 2, "a refusal emits nothing");

    // -- a rescan --------------------------------------------------------------------
    let deadline = Instant::now() + Duration::from_secs(20);
    let heard = events.named(LIBRARY_CHANGED).len();
    assert_eq!(window.invoke("rescan", json!({})), Ok(Value::Null));
    while events.named(LIBRARY_CHANGED).len() == heard {
        assert!(Instant::now() < deadline, "rescan ends in library-changed");
        std::thread::sleep(Duration::from_millis(25));
    }

    // -- Nexus (nothing listens at the configured address) -----------------------------
    let updates: UpdatesView = window.call("get_updates", json!({}));
    assert!(updates.last_check_ms.is_none());
    let offline = window.invoke("check_updates", json!({})).unwrap_err();
    assert_eq!(offline["kind"], "nexus", "{offline}");
    let unknown: Vec<Md5Result> = window.call(
        "identify_packages",
        json!({ "packageIds": ["0000000000000000"] }),
    );
    assert!(!unknown[0].matched);
    // v0.2 Find: the package's name goes out on this call, and nothing listens.
    let unreachable = window
        .invoke("find_on_nexus", json!({ "packageId": package_id }))
        .unwrap_err();
    assert_eq!(unreachable["kind"], "nexus", "{unreachable}");
    let row: UpdateRow = window.call(
        "link_package",
        json!({ "packageId": package_id, "urlOrId": "https://www.nexusmods.com/dragonsdogma2/mods/5150" }),
    );
    assert_eq!(row.link.unwrap().mod_id, 5150);
    assert_eq!(
        window.invoke("unlink_package", json!({ "packageId": package_id })),
        Ok(Value::Null)
    );
    let summary: NexusSummary = window.call("set_nexus_key", json!({ "key": null }));
    assert!(!summary.has_api_key);
    assert_eq!(summary.key, NexusKeyStanding::None);
    assert!(summary.account.is_none());
    assert_eq!(window.invoke("get_nexus_quota", json!({})), Ok(Value::Null));
    let labels: LabelFacets = window.call("get_labels", json!({}));
    assert!(labels.categories.is_empty() && labels.tags.is_empty());
    assert_eq!(
        labels.unlabeled_packages, 3,
        "no check has heard any labels: every package is unlabeled"
    );

    // -- the key's extras (v0.3): no key is stored, nothing listens ------------------------
    let account: NexusSummary = window.call("refresh_nexus_account", json!({}));
    assert_eq!(
        account.key,
        NexusKeyStanding::None,
        "no key: nothing to validate"
    );
    // Each answer is a RiftError of the spine's own (so the arguments mapped).
    let reason = |answer: Value| answer["detail"]["reason"].as_str().map(str::to_string);
    let offline = window
        .invoke("get_mod_files", json!({ "modId": 2207 }))
        .unwrap_err();
    assert_eq!(offline["kind"], "nexus", "{offline}");
    let offline = window
        .invoke(
            "browse_nexus",
            json!({ "query": { "text": "lantern", "sort": "updated", "offset": 20 } }),
        )
        .unwrap_err();
    assert_eq!(offline["kind"], "nexus", "{offline}");
    for (command, args) in [
        ("get_nexus_standings", json!({ "refresh": false })),
        ("set_tracked", json!({ "modId": 2207, "tracked": true })),
        (
            "set_endorsement",
            json!({ "modId": 2207, "status": "endorsed" }),
        ),
        (
            "start_download",
            json!({ "modId": 2207, "fileId": 9001, "forPackageId": null }),
        ),
    ] {
        let refused = window.invoke(command, args).unwrap_err();
        assert_eq!(refused["kind"], "nexus", "{command}: {refused}");
        assert_eq!(reason(refused), Some("noKey".to_string()), "{command}");
    }
    let rows: Vec<Value> = window.call("get_downloads", json!({}));
    assert!(rows.is_empty());
    let stale = window
        .invoke("cancel_download", json!({ "id": "no-such-download" }))
        .unwrap_err();
    assert_eq!(stale["kind"], "refused", "{stale}");
    assert_eq!(window.invoke("clear_downloads", json!({})), Ok(Value::Null));
    let handler: NxmHandlerView = window.call("get_nxm_handler", json!({}));
    assert_eq!(handler.state, NxmHandlerState::None);
    let on: NxmHandlerView = window.call("set_nxm_handler", json!({ "on": true }));
    assert_eq!(on.state, NxmHandlerState::Ours);
    let raw = window.invoke("get_nxm_handler", json!({})).unwrap();
    assert_eq!(keys(&raw), sorted(&["state", "other", "givesBackTo"]));
    let off: NxmHandlerView = window.call("set_nxm_handler", json!({ "on": false }));
    assert_eq!(off.state, NxmHandlerState::None);
    assert!(
        window
            .invoke("set_nxm_handler", json!({ "enabled": true }))
            .is_err(),
        "the window's key is the only one that maps"
    );
}

/// The keys of a JSON object, sorted.
fn keys(value: &Value) -> Vec<String> {
    let mut keys: Vec<String> = value
        .as_object()
        .unwrap_or_else(|| panic!("expected an object, got {value}"))
        .keys()
        .cloned()
        .collect();
    keys.sort();
    keys
}

fn sorted(expected: &[&str]) -> Vec<String> {
    let mut keys: Vec<String> = expected.iter().map(|k| (*k).to_string()).collect();
    keys.sort();
    keys
}

/// The key's extras (v0.3) through the window's invoke names and argument
/// keys, against a stand-in for Nexus: a key set from the window keeps its
/// account; the standings, a mod's files and a browse page decode into the
/// wire types; a free account's download waits, and its row carries exactly
/// the keys of `src/types/rift.ts`.
#[test]
fn the_keys_extras_answer_the_window_by_their_invoke_names_and_argument_keys() {
    let stand_in = StandIn::start(|request| {
        let path = request.path.split('?').next().unwrap_or_default();
        if path.ends_with("/users/validate.json") {
            return Answer::json(json!({
                "user_id": 4242, "name": "Arisen of Vernworth", "is_premium": false, "is_supporter": true
            }));
        }
        if path.ends_with("/user/tracked_mods.json") {
            return Answer::json(json!([{ "mod_id": 2207, "domain_name": "dragonsdogma2" }]));
        }
        if path.ends_with("/user/endorsements.json") {
            return Answer::json(json!([]));
        }
        if path.ends_with("/endorse.json") || path.ends_with("/abstain.json") {
            return Answer::json(json!({ "status": "Endorsed" }));
        }
        if request.body.contains("legacyModsByDomain") {
            return Answer::json(
                json!({ "data": { "legacyModsByDomain": { "totalCount": 1, "nodes": [
                { "modId": 2207, "name": "Lantern Glow Recolor", "version": "2.2",
                  "updatedAt": "2026-10-15T09:00:00Z", "status": "published",
                  "category": "Visuals", "tags": [], "adultContent": false }
            ] } } }),
            );
        }
        if request.body.contains("RiftstoneModFileDetails") {
            return Answer::json(json!({ "data": { "modFiles": [
                { "fileId": 9001, "groupId": 31, "sqid": "Rk4vLmN2Q", "name": "Lantern Glow Recolor",
                  "version": "2.2", "category": "MAIN", "date": 1_791_987_780, "primary": 1,
                  "sizeInBytes": "4096", "description": "Warmer amber.", "changelogText": ["Amber."] }
            ] } }));
        }
        if request.body.contains("RiftstoneBrowseMods") {
            return Answer::json(json!({ "data": { "mods": { "totalCount": 1, "nodes": [{
                "modId": 2207, "name": "Lantern Glow Recolor", "version": "2.2",
                "author": "Vern", "uploader": { "name": "Vern" }, "category": "Visuals",
                "modCategory": { "name": "Visuals" }, "adultContent": false, "downloads": 1520,
                "endorsements": 88, "summary": "Warmer lanterns.", "updatedAt": "2024-05-04T01:30:45Z",
                "game": { "domainName": "dragonsdogma2" }
            }] } } }));
        }
        Answer {
            status: 404,
            headers: Vec::new(),
            body: json!({}),
        }
    });
    let bench = Bench::new();
    let (state, _) = bench.open(bench.config_at(&stand_in));
    assert!(state.scan_blocking());
    let (_app, window) = mock_window(&state);

    let keyed: NexusSummary = window.call("set_nexus_key", json!({ "key": "invented-key-0042" }));
    assert_eq!(keyed.key, NexusKeyStanding::Connected);
    let account = keyed.account.expect("the account it validated");
    assert!(!account.premium && account.supporter);
    let refreshed: NexusSummary = window.call("refresh_nexus_account", json!({}));
    assert_eq!(refreshed.key, NexusKeyStanding::Connected);

    let standings: NexusStandings = window.call("get_nexus_standings", json!({ "refresh": true }));
    assert_eq!(standings.tracked.len(), 1);
    let raw = window
        .invoke("get_nexus_standings", json!({ "refresh": false }))
        .unwrap();
    assert_eq!(
        keys(&raw),
        sorted(&["tracked", "endorsements", "checkedMs"])
    );
    let after: NexusStandings =
        window.call("set_tracked", json!({ "modId": 2207, "tracked": false }));
    assert!(after.tracked.is_empty());
    let after: NexusStandings = window.call(
        "set_endorsement",
        json!({ "modId": 2207, "status": "endorsed" }),
    );
    assert_eq!(after.endorsements.len(), 1);

    let files: NexusModFiles = window.call("get_mod_files", json!({ "modId": 2207 }));
    assert_eq!(files.files.len(), 1);
    assert!(files.files[0].newest);
    let page: NexusBrowsePage = window.call(
        "browse_nexus",
        json!({ "query": { "text": "lantern", "sort": "relevance", "offset": 0 } }),
    );
    assert_eq!(page.mods[0].mod_id, 2207);

    let waiting = window
        .invoke(
            "start_download",
            json!({ "modId": 2207, "fileId": 9001, "forPackageId": null }),
        )
        .unwrap();
    assert_eq!(
        waiting["phase"], "waiting",
        "a free account waits: {waiting}"
    );
    assert_eq!(
        keys(&waiting),
        sorted(&[
            "id",
            "modId",
            "fileId",
            "modName",
            "fileName",
            "version",
            "forPackageId",
            "phase",
            "bytesDone",
            "bytesTotal",
            "landedAs",
            "packageId",
            "alreadyHad",
            "failure",
            "nexusPageUrl",
            "startedMs",
            "finishedMs",
        ])
    );
    let rows: Vec<Value> = window.call("get_downloads", json!({}));
    assert_eq!(rows, vec![waiting.clone()]);
    assert_eq!(
        window.invoke("cancel_download", json!({ "id": waiting["id"] })),
        Ok(Value::Null)
    );
    assert_eq!(window.invoke("clear_downloads", json!({})), Ok(Value::Null));
    let rows: Vec<Value> = window.call("get_downloads", json!({}));
    assert!(rows.is_empty(), "the cancelled row is cleared");
}

#[test]
fn find_on_nexus_answers_the_window_by_its_invoke_name_and_argument_key() {
    let stand_in = StandIn::start(|request| {
        if request.body.contains("RiftstoneFindMods") {
            return Answer::json(json!({ "data": { "mods": { "totalCount": 1, "nodes": [{
                "modId": 2207, "name": "Lantern Glow Recolor", "version": "2.1",
                "author": null, "uploader": { "name": "Vern" },
                "category": "Visuals", "modCategory": { "name": "Visuals" },
                "adultContent": false, "downloads": 1520, "endorsements": 88,
                "summary": "Warmer lanterns.", "updatedAt": "2024-05-04T01:30:45Z",
                "game": { "domainName": "dragonsdogma2" }
            }] } } }));
        }
        Answer {
            status: 500,
            headers: Vec::new(),
            body: json!({}),
        }
    });
    let bench = Bench::new();
    let mut config = bench.config(DataDirs::under(&bench.data_home()));
    config.nexus_endpoints = Some((stand_in.graphql_url(), stand_in.v1_base()));
    let (state, _) = bench.open(config);
    assert!(state.scan_blocking());
    let (_app, window) = mock_window(&state);
    let lantern = state
        .shelf()
        .packages()
        .iter()
        .find(|p| p.file_name == "Lantern Glow Recolor.zip")
        .unwrap()
        .id
        .0
        .clone();

    let findings: NexusFindings = window.call("find_on_nexus", json!({ "packageId": lantern }));
    assert_eq!(findings.package_id, lantern);
    assert_eq!(findings.terms, "Lantern Glow Recolor");
    assert_eq!(findings.candidates.len(), 1);
    let candidate = &findings.candidates[0];
    assert_eq!(
        (
            candidate.mod_id,
            candidate.author.as_deref(),
            candidate.linked_packages
        ),
        (2207, Some("Vern"), 0)
    );
    assert_eq!(
        candidate.page_url,
        "https://www.nexusmods.com/dragonsdogma2/mods/2207"
    );
    assert_eq!(candidate.updated_ms, Some(1_714_786_245_000));

    // Exactly the keys of src/types/rift.ts cross the bridge.
    let raw = window
        .invoke("find_on_nexus", json!({ "packageId": lantern }))
        .unwrap();
    assert_eq!(keys(&raw), sorted(&["packageId", "terms", "candidates"]));
    assert_eq!(
        keys(&raw["candidates"][0]),
        sorted(&[
            "modId",
            "name",
            "author",
            "version",
            "category",
            "adult",
            "downloads",
            "endorsements",
            "summary",
            "updatedMs",
            "pageUrl",
            "linkedPackages",
        ])
    );

    // The window's camelCase key is the only one that maps; an unknown
    // package is entryNotFound, and asks Nexus nothing.
    let asked = stand_in.seen().len();
    assert!(window
        .invoke("find_on_nexus", json!({ "package_id": lantern }))
        .is_err());
    assert_eq!(
        window
            .invoke("find_on_nexus", json!({ "packageId": "0000000000000000" }))
            .unwrap_err(),
        json!({ "kind": "entryNotFound", "detail": { "entry": "0000000000000000" } })
    );
    assert_eq!(stand_in.seen().len(), asked);
}

/// The window's service, as it is built.
const SERVICE: &str = include_str!("../../src/services/rift.ts");

/// Every invoke name `src/services/rift.ts` sends, in order of appearance.
fn service_invoke_names() -> Vec<&'static str> {
    SERVICE
        .split("invoke(\"")
        .skip(1)
        .filter_map(|rest| rest.split('"').next())
        .filter(|name| {
            !name.is_empty() && name.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
        })
        .collect()
}

#[test]
fn every_invoke_name_the_window_sends_is_registered() {
    let names = service_invoke_names();
    assert!(
        names.len() >= 44,
        "the service's invoke names were read: {names:?}"
    );
    let bench = Bench::new();
    let (state, _) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    let (_app, window) = mock_window(&state);
    let unknown = window.invoke("no_such_command", json!({})).unwrap_err();
    assert!(unknown.to_string().contains("not found"), "{unknown}");
    // Asked with no arguments, a registered command either answers or says
    // what it misses; only an unregistered one is "not found". (The bench's
    // launcher records, its Nexus is unreachable: nothing leaves the bench.)
    for name in names {
        if let Err(err) = window.invoke(name, json!({})) {
            assert!(
                !err.to_string()
                    .contains(&format!("Command {name} not found")),
                "{name} is invoked by the window and not registered: {err}"
            );
        }
    }
}
