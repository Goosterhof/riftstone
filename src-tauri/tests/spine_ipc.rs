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
    AdoptResultView, CheckAction, GameSummary, InstallPlanView, InstallResultView, InstalledView,
    LabelFacets, LibraryRoot, Md5Result, NexusSummary, OptionDetail, Overview, OwnerKind,
    PackageDetail, PackageSummary, ParkedRestoreReport, SearchResult, Settings, UninstallPlanView,
    UninstallResultView, UpdateRow, UpdatesView, VeilMode,
};
use riftstone_lib::events::{GAME_CHANGED, LIBRARY_CHANGED};
use riftstone_lib::persist::DataDirs;
use support::{Bench, ERMINE_MESH};

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

#[test]
fn every_command_answers_the_window_by_its_invoke_name_and_argument_keys() {
    let bench = Bench::new();
    let (state, events) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    assert!(state.scan_blocking());

    let app = riftstone_lib::with_commands(mock_builder())
        .build(mock_context(noop_assets()))
        .expect("the mock app builds");
    app.manage(Arc::clone(&state));
    let window = Window {
        webview: tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap(),
    };

    // -- the home frame --------------------------------------------------------
    let home: Overview = window.call("get_overview", json!({}));
    assert_eq!(home.library.packages, 3);
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
    let hits: Vec<SearchResult> = window.call(
        "search",
        json!({ "query": { "text": "ermine", "filters": { "installed": null, "rootIds": [], "includeMenuOnly": false, "categories": [], "tags": [] }, "limit": 0 } }),
    );
    let ermine = hits[0].option_id.clone();
    let package_id = hits[0].package_id.clone();
    assert_eq!(hits[0].nexus_category, None, "no check has run");
    assert!(hits[0].nexus_tags.is_empty() && !hits[0].adult);
    let lantern = window
        .call::<Vec<SearchResult>>(
            "search",
            json!({ "query": { "text": "lantern", "filters": { "installed": null, "rootIds": ["fluffy"], "includeMenuOnly": false, "categories": [], "tags": [] }, "limit": 5 } }),
        )[0]
        .option_id
        .clone();
    let by_label: Vec<SearchResult> = window.call(
        "search",
        json!({ "query": { "text": "", "filters": { "installed": null, "rootIds": [], "includeMenuOnly": false, "categories": ["Armour"], "tags": ["Cloaks"] }, "limit": 0 } }),
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
    let adopted: AdoptResultView = window.call("adopt_fluffy", json!({}));
    assert_eq!(adopted.adopted, 1);
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
    let refused = window
        .invoke(
            "set_game_root",
            json!({ "path": bench.fluffy.to_string_lossy() }),
        )
        .unwrap_err();
    assert_eq!(refused["kind"], "gameNotFound");

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

    // -- roots -----------------------------------------------------------------------
    let extra = bench.dir.path().join("Mods I Keep");
    std::fs::create_dir_all(&extra).unwrap();
    let root: LibraryRoot = window.call("add_root", json!({ "path": extra.to_string_lossy() }));
    assert_eq!(root.label, "Mods I Keep");
    assert!(root.id.starts_with("custom-"));
    assert_eq!(
        window.invoke("remove_root", json!({ "rootId": root.id })),
        Ok(Value::Null)
    );
    // add_root started a background scan; let it land before the next one.
    let deadline = Instant::now() + Duration::from_secs(20);
    while !matches!(
        window.call::<Overview>("get_overview", json!({})).scan,
        riftstone_lib::dto::ScanState::Idle
    ) {
        assert!(Instant::now() < deadline, "the scan settles");
        std::thread::sleep(Duration::from_millis(25));
    }
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
    assert_eq!(window.invoke("get_nexus_quota", json!({})), Ok(Value::Null));
    let labels: LabelFacets = window.call("get_labels", json!({}));
    assert!(labels.categories.is_empty() && labels.tags.is_empty());
    assert_eq!(
        labels.unlabeled_packages, 3,
        "no check has heard any labels: every package is unlabeled"
    );
}
