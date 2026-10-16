//! The spine end to end, with no Tauri runtime: first launch over a fake game,
//! a fake Fluffy and a fake Downloads folder → scan → search → option and
//! package details → thumbnail → plan → install → the installed view (with a
//! pak-vs-loose conflict read from the paks' own tables of contents) →
//! uninstall → Fluffy adoption → a relaunch that shows the cached library at
//! once. Vanilla files and the library are checked untouched at the end.
//!
//! The only test in this binary, because it points `RIFTSTONE_DATA_HOME` at
//! its tempdir — process-wide state no other test may race.

mod support;

use std::fs;

use riftstone_lib::commands::{deploy, library, nexus, overview};
use riftstone_lib::dto::{
    CheckLevel, ConflictKey, GameSource, InstallOrigin, JournalOrigin, PlanWarning, RemovalAction,
    ScanState, SearchFilters, SearchQuery, Settings, ThumbnailSize, TreeNodeKind, WriteAction,
};
use riftstone_lib::events::{GAME_CHANGED, INSTALL_PROGRESS, LIBRARY_CHANGED, SCAN_PROGRESS};
use riftstone_lib::persist::{DataDirs, DATA_HOME_ENV};
use riftstone_lib::state::AppState;
use support::{Bench, BANTER_LUA, ERMINE_MESH, WOOL_MESH};

fn query(text: &str) -> SearchQuery {
    SearchQuery {
        text: text.into(),
        filters: SearchFilters::default(),
        limit: 0,
    }
}

fn option_named(state: &AppState, name: &str) -> String {
    library::search_impl(
        state,
        &SearchQuery {
            text: String::new(),
            filters: SearchFilters {
                include_menu_only: true,
                ..SearchFilters::default()
            },
            limit: 1_000,
        },
    )
    .into_iter()
    .find(|hit| hit.name == name)
    .unwrap_or_else(|| panic!("{name} is in the library"))
    .option_id
}

#[test]
fn the_spine_scans_searches_installs_and_uninstalls_over_a_fake_world() {
    let bench = Bench::new();
    std::env::set_var(DATA_HOME_ENV, bench.data_home());
    let dirs = DataDirs::from_env().expect("RIFTSTONE_DATA_HOME selects the data dirs");
    assert_eq!(dirs, DataDirs::under(&bench.data_home()));
    let vanilla = Bench::snapshot(&bench.game)
        .into_iter()
        .filter(|(path, _)| !path.starts_with("natives/"))
        .collect::<Vec<_>>();
    let library_before = Bench::snapshot(&bench.mods);

    // -- first launch: Fluffy found, default roots made, the scan ----------
    let (state, events) = bench.open(bench.config(dirs.clone()));
    let roots = state.roots_snapshot();
    let ids: Vec<&str> = roots.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["fluffy", "downloads"],
        "the first launch makes both default roots"
    );
    assert!(
        roots[1].dd2_shaped_only,
        "Downloads lists DD2-shaped archives only"
    );
    assert!(dirs.roots_file().is_file(), "roots.json is written at once");

    let before = overview::get_overview_impl(&state).unwrap();
    assert_eq!(before.library.last_scan_ms, None, "nothing scanned yet");
    assert!(state.scan_blocking());
    assert!(
        !events.named(SCAN_PROGRESS).is_empty(),
        "the scan reports progress"
    );
    assert_eq!(events.named(LIBRARY_CHANGED).len(), 1);
    assert!(
        dirs.catalog_cache_file().is_file(),
        "the catalog cache is persisted"
    );

    let home = overview::get_overview_impl(&state).unwrap();
    let game = home.game.as_ref().expect("the fake game is found");
    assert_eq!(game.source, GameSource::Env);
    assert!(!game.running);
    assert_eq!(home.scan, ScanState::Idle);
    assert!(home.library.last_scan_ms.is_some());
    assert_eq!(
        home.library.packages, 3,
        "two Fluffy mods + one Nexus download; the photos are no mod"
    );
    assert_eq!(home.library.options, 5);
    assert_eq!(home.library.installed_options, 0);
    let fluffy = home.fluffy.as_ref().expect("Fluffy is found");
    assert_eq!((fluffy.installed_count, fluffy.unadopted_count), (1, 1));
    assert!(home
        .checks
        .iter()
        .any(|c| c.id == "game" && c.level == CheckLevel::Ok));
    assert!(
        home.checks.iter().any(|c| c.id == "fluffy"),
        "rift-deploy's preflight speaks through the overview, under the window's ids: {:?}",
        home.checks.iter().map(|c| &c.id).collect::<Vec<_>>()
    );
    assert!(home.recovered.is_none());
    assert!(!home.nexus.has_api_key);
    assert!(overview::get_troubled_packages_impl(&state).is_empty());

    // -- search -------------------------------------------------------------
    let hits = library::search_impl(&state, &query("ermine"));
    let ermine_hit = hits.first().expect("a hit for ermine");
    assert_eq!(ermine_hit.name, "Ermine Cape");
    assert_eq!(
        ermine_hit.trail,
        vec!["Vernworth Cloaks Collection", "Cloaks"]
    );
    assert!(ermine_hit.has_screenshot);
    assert_eq!(ermine_hit.author.as_deref(), Some("Vern"));
    assert_eq!(
        ermine_hit.description_snippet.as_deref(),
        Some("A white cape with black tips, for the cold roads north of Vernworth."),
        "whitespace collapsed"
    );
    assert!(ermine_hit.installed.is_none());
    assert!(ermine_hit.update.is_none(), "not linked to Nexus");
    let by_file = library::search_impl(&state, &query("ermine_cape.mesh"));
    assert_eq!(by_file.len(), 1);
    assert!(
        !by_file[0].file_matches.is_empty(),
        "a file path inside the archive matched"
    );

    let fluffy_hits = library::search_impl(
        &state,
        &SearchQuery {
            text: String::new(),
            filters: SearchFilters {
                installed: Some(true),
                ..SearchFilters::default()
            },
            limit: 0,
        },
    );
    assert_eq!(
        fluffy_hits.len(),
        1,
        "only Fluffy's install counts as installed so far"
    );
    assert_eq!(fluffy_hits[0].name, "Wool Hood");
    assert_eq!(
        fluffy_hits[0].installed.as_ref().map(|b| b.origin),
        Some(InstallOrigin::FluffyUnadopted)
    );

    let ermine = option_named(&state, "Ermine Cape");
    let lantern = option_named(&state, "Lantern Glow Recolor");
    let cloaks = option_named(&state, "Cloaks");

    // -- details ------------------------------------------------------------
    let detail = library::get_option_impl(&state, &ermine).unwrap();
    assert_eq!(detail.parent_name.as_deref(), Some("Cloaks"));
    assert!(detail.installed.is_none());
    assert!(!detail.fluffy_installed);
    assert!(detail.conflicts.is_empty());
    assert!(detail.update.is_none());
    assert_eq!(detail.option.kinds.loose, 1);
    let menu = library::get_option_impl(&state, &cloaks).unwrap();
    assert!(menu.option.menu_only);
    assert_eq!(
        menu.children.len(),
        2,
        "Ermine Cape and Wool Hood hang under Cloaks"
    );
    assert!(library::get_option_impl(&state, "nope#nothing").is_err());

    let package = library::get_package_impl(&state, &detail.package.id).unwrap();
    assert_eq!(package.package.option_count, 3);
    assert_eq!(package.tree.len(), 1);
    assert_eq!(package.tree[0].kind, TreeNodeKind::Option);
    assert_eq!(package.tree[0].children.len(), 2);
    assert!(package
        .entries
        .iter()
        .any(|e| e.path.ends_with("README.txt")));

    // -- thumbnails ---------------------------------------------------------
    let card = library::get_thumbnail_impl(&state, &ermine, ThumbnailSize::Card)
        .unwrap()
        .expect("Ermine Cape has a screenshot");
    assert!(card.starts_with("data:image/jpeg;base64,"));
    let cached: Vec<_> = fs::read_dir(dirs.thumbs_dir()).unwrap().collect();
    assert_eq!(cached.len(), 1, "the card is cached on disk");
    let again = library::get_thumbnail_impl(&state, &ermine, ThumbnailSize::Card).unwrap();
    assert_eq!(
        again.as_deref(),
        Some(card.as_str()),
        "the cache answers the second time"
    );
    assert!(
        library::get_thumbnail_impl(&state, &lantern, ThumbnailSize::Full)
            .unwrap()
            .is_none()
    );

    // -- plan + install the loose cape ------------------------------------
    let plan = deploy::plan_install_impl(&state, &ermine).unwrap();
    assert_eq!(plan.refused, None);
    assert_eq!(plan.writes.len(), 1);
    assert_eq!(plan.writes[0].game_path, ERMINE_MESH);
    assert_eq!(plan.writes[0].action, WriteAction::Create);
    assert!(plan.warnings.contains(&PlanWarning::ReframeworkMissing));
    assert!(
        !bench.game.join(ERMINE_MESH).exists(),
        "planning writes nothing"
    );

    events.clear();
    let landed = deploy::install_impl(&state, &ermine).unwrap();
    assert_eq!(landed.files_written, 1);
    assert!(!landed.restart_needed);
    assert_eq!(
        fs::read(bench.game.join(ERMINE_MESH)).unwrap(),
        b"ermine cape mesh bytes"
    );
    let progress = events.named(INSTALL_PROGRESS);
    assert!(!progress.is_empty());
    assert_eq!(progress.last().unwrap()["filesDone"], 1);
    assert_eq!(progress.last().unwrap()["optionId"], ermine.as_str());
    assert_eq!(events.named(GAME_CHANGED).len(), 1);

    let hit = &library::search_impl(&state, &query("ermine"))[0];
    assert_eq!(
        hit.installed.as_ref().map(|b| b.origin),
        Some(InstallOrigin::Riftstone)
    );
    assert!(hit.installed.as_ref().unwrap().installed_at.is_some());

    // -- the pak would lose to the loose cape: seen before it is installed --
    let preview = library::get_option_impl(&state, &lantern).unwrap();
    assert_eq!(
        preview.conflicts.len(),
        1,
        "the pak carries the cape's hash"
    );
    let conflict = &preview.conflicts[0];
    assert!(matches!(conflict.key, ConflictKey::Hash { .. }));
    assert_eq!(
        conflict.winner.option_id, ermine,
        "a loose file beats every pak"
    );
    assert_eq!(conflict.layers.last().unwrap().option_id, ermine);

    let pak = deploy::install_impl(&state, &lantern).unwrap();
    assert_eq!(pak.files_written, 1);
    assert!(bench
        .game
        .join("pak_mods/zrift-0000-LanternGlow.pak")
        .is_file());

    let world = deploy::get_installed_impl(&state).unwrap();
    assert_eq!(world.installs.len(), 2);
    assert!(world
        .installs
        .iter()
        .all(|i| i.origin == JournalOrigin::Riftstone && !i.package_missing));
    assert_eq!(world.pak_order, vec![lantern.clone()]);
    let pak_row = world
        .installs
        .iter()
        .find(|i| i.option_id == lantern)
        .unwrap();
    assert_eq!(pak_row.paks.len(), 1);
    assert_eq!(pak_row.paks[0].file_name, "zrift-0000-LanternGlow.pak");
    assert_eq!(
        world.conflicts.len(),
        1,
        "the pak's table of contents was read from pak_mods/"
    );
    assert_eq!(world.conflicts[0].winner.name, "Ermine Cape");
    assert_eq!(world.fluffy_unadopted.len(), 1);
    assert_eq!(world.fluffy_unadopted[0].name, "Wool Hood");
    assert_eq!(
        world.fluffy_unadopted[0].matched_option_id.as_deref(),
        Some(option_named(&state, "Wool Hood").as_str()),
        "Fluffy's ModName finds its library option"
    );
    assert!(world.strays.is_empty());
    assert!(world.missing.is_empty());
    assert_eq!(world.pak_count_warning, 100);
    let lantern_hit = &library::search_impl(&state, &query("lantern"))[0];
    assert!(lantern_hit.conflicted);
    assert_eq!(
        overview::get_overview_impl(&state)
            .unwrap()
            .library
            .conflicts,
        1
    );

    // -- uninstall the cape --------------------------------------------------
    let back = deploy::plan_uninstall_impl(&state, &ermine).unwrap();
    assert_eq!(back.refused, None);
    assert_eq!(back.removals.len(), 1);
    assert_eq!(back.removals[0].action, RemovalAction::Delete);
    let gone = deploy::uninstall_impl(&state, &ermine).unwrap();
    assert_eq!(gone.files_removed, 1);
    assert!(!bench.game.join(ERMINE_MESH).exists());
    let world = deploy::get_installed_impl(&state).unwrap();
    assert_eq!(world.installs.len(), 1);
    assert!(world.conflicts.is_empty(), "no loose cape, no conflict");
    assert!(library::search_impl(&state, &query("ermine"))[0]
        .installed
        .is_none());

    // -- adopt Fluffy's install ----------------------------------------------
    let adopted = deploy::adopt_fluffy_impl(&state).unwrap();
    assert_eq!(adopted.adopted, 1);
    assert!(adopted.unmatched.is_empty());
    let world = deploy::get_installed_impl(&state).unwrap();
    assert!(world.fluffy_unadopted.is_empty());
    let wool = world
        .installs
        .iter()
        .find(|i| i.name == "Wool Hood")
        .expect("adopted");
    assert_eq!(wool.origin, JournalOrigin::Fluffy);
    let wool_hit = &library::search_impl(&state, &query("wool"))[0];
    assert_eq!(
        wool_hit.installed.as_ref().map(|b| b.origin),
        Some(InstallOrigin::Fluffy)
    );
    assert_eq!(
        fs::read(bench.game.join(WOOL_MESH)).unwrap(),
        b"wool hood as Fluffy left it"
    );

    // -- settings, roots -----------------------------------------------------
    let settings = Settings {
        veil_thumbnails: true,
        auto_check_updates: false,
    };
    assert_eq!(overview::set_settings_impl(&state, settings), settings);
    assert_eq!(overview::get_settings_impl(&state), settings);
    assert!(
        nexus::get_nexus_quota_impl(&state).is_none(),
        "no key, no quota"
    );

    // -- vanilla and the library are untouched --------------------------------
    let vanilla_after = Bench::snapshot(&bench.game)
        .into_iter()
        .filter(|(path, _)| !path.starts_with("natives/") && !path.starts_with("pak_mods/"))
        .collect::<Vec<_>>();
    assert_eq!(
        vanilla_after, vanilla,
        "DD2.exe, re_chunk_000.pak*, dlc/ untouched"
    );
    assert_eq!(
        Bench::snapshot(&bench.mods),
        library_before,
        "the library is read-only (RS-2)"
    );
    assert!(!bench.game.join(BANTER_LUA).exists());

    // -- relaunch: the cached library shows before any scan ------------------
    drop(state);
    let (again, again_events) = bench.open(bench.config(dirs.clone()));
    let home = overview::get_overview_impl(&again).unwrap();
    assert_eq!(
        home.library.packages, 3,
        "the catalog cache answers at once"
    );
    assert!(home.library.last_scan_ms.is_some());
    assert_eq!(
        home.library.installed_options, 2,
        "the journal kept the pak and the adoption"
    );
    assert_eq!(
        overview::get_settings_impl(&again),
        settings,
        "settings.json kept the choice"
    );
    assert_eq!(library::search_impl(&again, &query("ermine")).len(), 1);
    assert_eq!(
        again.roots_snapshot().len(),
        2,
        "no second set of default roots"
    );

    library::remove_root_impl(&again, "downloads").unwrap();
    assert_eq!(again.roots_snapshot().len(), 1);
    assert_eq!(again_events.named(LIBRARY_CHANGED).len(), 1);
    assert_eq!(
        overview::get_overview_impl(&again)
            .unwrap()
            .library
            .packages,
        2
    );
    let added = library::add_root_impl(&again, &bench.downloads.to_string_lossy()).unwrap();
    assert_eq!(
        added.id, "downloads",
        "Downloads comes back as its built-in root"
    );
    assert!(added.dd2_shaped_only);
    let doubled = library::add_root_impl(&again, &bench.downloads.to_string_lossy()).unwrap();
    assert_eq!(
        doubled.id, "downloads",
        "adding a root twice returns the one root"
    );
    assert!(library::add_root_impl(&again, "/no/such/folder/anywhere").is_err());

    std::env::remove_var(DATA_HOME_ENV);
}
