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
    AdoptResultView, GameSummary, InstallPlanView, InstallResultView, InstalledView, LibraryRoot,
    Md5Result, NexusSummary, OptionDetail, Overview, PackageDetail, PackageSummary, SearchResult,
    Settings, UninstallPlanView, UninstallResultView, UpdateRow, UpdatesView,
};
use riftstone_lib::events::LIBRARY_CHANGED;
use riftstone_lib::persist::DataDirs;
use support::Bench;

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
        json!({ "settings": { "veilThumbnails": true, "autoCheckUpdates": false } }),
    );
    assert!(settings.veil_thumbnails && !settings.auto_check_updates);
    let troubled: Vec<PackageSummary> = window.call("get_troubled_packages", json!({}));
    assert!(troubled.is_empty());

    // -- the library -------------------------------------------------------------
    let hits: Vec<SearchResult> = window.call(
        "search",
        json!({ "query": { "text": "ermine", "filters": { "installed": null, "rootIds": [], "includeMenuOnly": false }, "limit": 0 } }),
    );
    let ermine = hits[0].option_id.clone();
    let package_id = hits[0].package_id.clone();
    let lantern = window
        .call::<Vec<SearchResult>>(
            "search",
            json!({ "query": { "text": "lantern", "filters": { "installed": null, "rootIds": ["fluffy"], "includeMenuOnly": false }, "limit": 5 } }),
        )[0]
        .option_id
        .clone();
    let detail: OptionDetail = window.call("get_option", json!({ "optionId": ermine }));
    assert_eq!(detail.option.name, "Ermine Cape");
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
}
