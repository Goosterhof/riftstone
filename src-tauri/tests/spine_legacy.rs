//! The investor's v0.2 data keeps working (WINDOW-CONTRACT § v0.3 Legacy
//! data): a schema-1 journal with an adopted install loads, lists as adopted
//! and uninstalls; after a scan, a folder package's v0.2 id becomes its v0.3
//! id in the journal and in `nexus.json` (exactly), and an install whose
//! package moved — the whole mods folder moved, or an adopted stand-in — is
//! found again by its files (the relink pass), in one journal save.
//!
//! The schema-1 journals here are written by hand with INVENTED legacy kinds:
//! the lift recognises only the kinds that stay.

mod support;

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use rift_core::{OptionId, PackageId, RiftError};
use riftstone_lib::commands::{deploy, library, nexus, overview};
use riftstone_lib::dto::{CheckLevel, InstallOrigin, RemovalAction, SearchFilters, SearchQuery};
use riftstone_lib::events::{RecordedEvents, GAME_CHANGED, LIBRARY_CHANGED, UPDATES_CHANGED};
use riftstone_lib::persist::DataDirs;
use riftstone_lib::state::AppState;
use serde_json::{json, Value};
use support::{option_named, Bench, WOOL_MESH};

const OLD_HOOD: &str = "natives/stm/appsystem/hood/old_hood.mesh.260421070";
const BANNER: &str = "natives/stm/banner/bakbattahl_banner.tex.251211553";
const BANNERS: &str = "Bakbattahl Banners";

/// A schema-1 install the journal took over from files already in the game,
/// under an invented origin kind and a stand-in id.
fn adopted_install(seq: u64, option_id: &str, name: &str, root: &str, files: Value) -> Value {
    json!({
        "seq": seq,
        "optionId": option_id,
        "name": name,
        "packageFile": "",
        "packagePath": "",
        "packageSize": 0,
        "optionRoot": root,
        "version": null,
        "installedAt": "2026-10-01T08:00:00.000Z",
        "gameBuild": null,
        "origin": { "kind": "takenOverByHand", "section": name },
        "files": files,
    })
}

fn schema_1(game_root: &Path, installs: Vec<Value>) -> Value {
    json!({
        "schema": 1,
        "gameRoot": game_root,
        "installs": installs,
        "pakOrder": [],
        "pending": null,
        "nextSeq": 10,
        "cleanup": [],
        "parked": [],
    })
}

fn loose(game_path: &str, size: u64) -> Value {
    json!({ "gamePath": game_path, "kind": "loose", "size": size, "entry": null,
            "below": { "kind": "nothing" } })
}

fn wait_for(events: &RecordedEvents, event: &str, count: usize) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while events.named(event).len() < count {
        assert!(Instant::now() < deadline, "{event} ×{count} never arrived");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_schema_1_journal_with_an_adopted_install_loads_lists_as_adopted_and_uninstalls() {
    let bench = Bench::new();
    let dirs = DataDirs::under(&bench.data_home());
    // Another tool's file in the game folder, which the journal took over.
    let hood = bench.game.join(OLD_HOOD);
    fs::create_dir_all(hood.parent().unwrap()).unwrap();
    fs::write(&hood, b"an old hood").unwrap();
    bench.seed_journal(
        &dirs,
        &schema_1(
            &bench.game,
            vec![adopted_install(
                1,
                "handmade#Old Hood",
                "Old Hood",
                "",
                json!([loose(OLD_HOOD, 11)]),
            )],
        ),
    );
    let journal_file = rift_deploy::journal_path(dirs.state_dir(), &bench.game);
    let as_written = fs::read(&journal_file).unwrap();

    let (state, events) = bench.open(bench.config(dirs.clone()));
    assert!(state.scan_blocking());
    let home = overview::get_overview_impl(&state).unwrap();
    assert!(
        !home.checks.iter().any(|c| c.id == "journal"),
        "the old journal loads"
    );
    assert_eq!(home.library.installed_options, 1);
    assert!(
        !home.checks.iter().any(|c| c.id == "relinked"),
        "its files fit no option in the mods folder"
    );
    assert_eq!(
        fs::read(&journal_file).unwrap(),
        as_written,
        "loading, scanning and finding nothing to re-key save nothing"
    );
    assert!(events.named(GAME_CHANGED).is_empty());

    let world = deploy::get_installed_impl(&state).unwrap();
    assert_eq!(world.installs.len(), 1);
    let row = &world.installs[0];
    assert_eq!(row.option_id, "adopted#Old Hood", "the lifted stand-in");
    assert_eq!(row.origin, InstallOrigin::Adopted);
    assert!(row.package_missing);
    assert_eq!(row.name, "Old Hood");
    assert_eq!(row.package_name, "Old Hood");
    assert!(matches!(
        library::get_option_impl(&state, &row.option_id),
        Err(RiftError::EntryNotFound { .. })
    ));

    let plan = deploy::plan_uninstall_impl(&state, &row.option_id).unwrap();
    assert_eq!(plan.refused, None);
    assert_eq!(plan.removals.len(), 1);
    assert_eq!(plan.removals[0].action, RemovalAction::Delete);
    let gone = deploy::uninstall_impl(&state, &row.option_id).unwrap();
    assert_eq!(gone.files_removed, 1);
    assert!(!hood.exists(), "the journal knew the file");
    assert!(deploy::get_installed_impl(&state)
        .unwrap()
        .installs
        .is_empty());
    assert_eq!(
        bench.journal_on_disk(&dirs)["schema"],
        2,
        "saved as schema 2 at its first change"
    );
}

#[test]
fn an_adopted_stand_in_whose_files_fit_one_option_is_found_again() {
    let bench = Bench::new();
    let dirs = DataDirs::under(&bench.data_home());
    let wool = bench.game.join(WOOL_MESH);
    fs::create_dir_all(wool.parent().unwrap()).unwrap();
    fs::write(&wool, b"wool hood mesh bytes").unwrap();
    let root = "Vernworth Cloaks Collection/Cloaks/Wool Hood";
    bench.seed_journal(
        &dirs,
        &schema_1(
            &bench.game,
            vec![adopted_install(
                1,
                "handmade#Wool Hood",
                "Wool Hood",
                root,
                json!([loose(WOOL_MESH, 20)]),
            )],
        ),
    );

    let (state, events) = bench.open(bench.config(dirs.clone()));
    assert!(state.scan_blocking());
    let wool_hood = option_named(&state, "Wool Hood");
    let world = deploy::get_installed_impl(&state).unwrap();
    assert_eq!(
        world.installs[0].option_id, wool_hood,
        "matched by its files"
    );
    assert_eq!(world.installs[0].origin, InstallOrigin::Adopted);
    assert!(!world.installs[0].package_missing);
    let hit = &library::search_impl(
        &state,
        &SearchQuery {
            text: "wool".into(),
            filters: SearchFilters::default(),
            limit: 0,
        },
    )[0];
    assert_eq!(
        hit.installed.as_ref().map(|badge| badge.origin),
        Some(InstallOrigin::Adopted)
    );
    assert_eq!(events.named(GAME_CHANGED).len(), 1, "the relink is told");
    let on_disk = bench.journal_on_disk(&dirs);
    assert_eq!(on_disk["schema"], 2);
    assert_eq!(on_disk["installs"][0]["optionId"], wool_hood.as_str());
    assert_eq!(
        on_disk["installs"][0]["packageFile"],
        "Vernworth Cloaks Collection.zip"
    );
    let check = overview::get_overview_impl(&state)
        .unwrap()
        .checks
        .into_iter()
        .find(|c| c.id == "relinked")
        .expect("the session says what it found again");
    assert_eq!(check.level, CheckLevel::Info);
    assert!(check.title.starts_with("1 install"), "{}", check.title);

    // A second scan finds nothing more to do.
    assert!(state.scan_blocking());
    assert_eq!(events.named(GAME_CHANGED).len(), 1);
}

/// A folder package in the mods folder, installed under v0.3 and linked by
/// hand; then everything is put back the way v0.2 would have written it: the
/// folder's id is `legacy_folder(<path the folder had>)` in the journal and
/// in `nexus.json`.
fn staged_v02_folder(bench: &Bench, dirs: &DataDirs, v02_path: &Path) -> (String, PackageId) {
    let folder = bench.mods.join(BANNERS);
    fs::create_dir_all(folder.join("natives/stm/banner")).unwrap();
    fs::write(folder.join("modinfo.ini"), b"name=Bakbattahl Banners\r\n").unwrap();
    fs::write(folder.join(BANNER), b"banner").unwrap();
    let (state, _) = bench.open(bench.config(dirs.clone()));
    assert!(state.scan_blocking());
    let banners = option_named(&state, "Bakbattahl Banners");
    deploy::install_impl(&state, &banners).unwrap();
    let package = OptionId(banners.clone()).package();
    assert_eq!(package, PackageId::for_folder(BANNERS));
    nexus::link_package_impl(&state, &package.0, "2207").unwrap();
    drop(state);

    let legacy = PackageId::legacy_folder(&std::path::absolute(v02_path).unwrap());
    for file in [
        rift_deploy::journal_path(dirs.state_dir(), &bench.game),
        dirs.nexus_file(),
    ] {
        let text = fs::read_to_string(&file).unwrap();
        assert!(text.contains(&package.0), "{}", file.display());
        fs::write(&file, text.replace(&package.0, &legacy.0)).unwrap();
    }
    let mut journal = bench.journal_on_disk(dirs);
    journal["schema"] = json!(1);
    bench.seed_journal(dirs, &journal);
    (banners, legacy)
}

#[test]
fn a_folders_v02_id_becomes_its_v03_id_in_the_journal_and_in_nexus_json() {
    let bench = Bench::new();
    let dirs = DataDirs::under(&bench.data_home());
    let (banners, legacy) = staged_v02_folder(&bench, &dirs, &bench.mods.join(BANNERS));

    let (state, events) = bench.open(bench.config(dirs.clone()));
    let before = deploy::get_installed_impl(&state).unwrap();
    assert!(
        before.installs[0].package_missing,
        "before the scan the journal still speaks v0.2"
    );
    assert!(state.scan_blocking());

    let world = deploy::get_installed_impl(&state).unwrap();
    assert_eq!(world.installs[0].option_id, banners);
    assert!(!world.installs[0].package_missing);
    let journal = bench.journal_on_disk(&dirs);
    assert_eq!(journal["schema"], 2);
    assert!(!journal.to_string().contains(&legacy.0), "{journal}");

    let store: Value = serde_json::from_slice(&fs::read(dirs.nexus_file()).unwrap()).unwrap();
    let package = PackageId::for_folder(BANNERS);
    assert_eq!(store["packages"][&package.0]["link"]["modId"], 2207);
    assert!(store["packages"].get(&legacy.0).is_none());
    let row = nexus::get_updates_impl(&state)
        .rows
        .into_iter()
        .find(|row| row.package_id == package.0)
        .unwrap();
    assert_eq!(
        row.link.unwrap().mod_id,
        2207,
        "the link followed its package"
    );

    assert_eq!(events.named(GAME_CHANGED).len(), 1);
    assert_eq!(events.named(UPDATES_CHANGED).len(), 1);
    let order: Vec<String> = events
        .all()
        .into_iter()
        .map(|(name, _)| name)
        .filter(|name| name == GAME_CHANGED || name == LIBRARY_CHANGED)
        .collect();
    assert_eq!(
        order,
        vec![GAME_CHANGED.to_string(), LIBRARY_CHANGED.to_string()],
        "the re-key lands before the window hears the scan ended"
    );
    assert!(overview::get_overview_impl(&state)
        .unwrap()
        .checks
        .iter()
        .any(|c| c.id == "relinked"));

    // Uninstalling under the new id works off the journal as before.
    let gone = deploy::uninstall_impl(&state, &banners).unwrap();
    assert_eq!(gone.files_removed, 1);
    assert!(!bench.game.join(BANNER).exists());
}

#[test]
fn a_moved_mods_folder_is_found_again_by_its_files() {
    let mut bench = Bench::new();
    let dirs = DataDirs::under(&bench.data_home());
    let old_mods = bench.mods.clone();
    let (_, legacy) = staged_v02_folder(&bench, &dirs, &old_mods.join(BANNERS));
    // The player moves the whole mods folder (and the stone with it).
    let new_home = bench.dir.path().join("Another Drive").join("The Riftstone");
    fs::create_dir_all(&new_home).unwrap();
    fs::rename(&old_mods, new_home.join("mods")).unwrap();
    bench.mods = new_home.join("mods");

    let (state, events) = bench.open(bench.config(dirs.clone()));
    assert!(state.scan_blocking());
    let banners = option_named(&state, "Bakbattahl Banners");
    assert_eq!(
        OptionId(banners.clone()).package(),
        PackageId::for_folder(BANNERS)
    );
    let world = deploy::get_installed_impl(&state).unwrap();
    assert_eq!(world.installs.len(), 1);
    assert_eq!(
        world.installs[0].option_id, banners,
        "relinked by its files"
    );
    assert!(!world.installs[0].package_missing);
    let store: Value = serde_json::from_slice(&fs::read(dirs.nexus_file()).unwrap()).unwrap();
    assert_eq!(
        store["packages"][&PackageId::for_folder(BANNERS).0]["link"]["modId"],
        2207,
        "the moved package's link record followed it"
    );
    assert!(store["packages"].get(&legacy.0).is_none());
    assert_eq!(events.named(GAME_CHANGED).len(), 1);
    assert!(!bench.journal_on_disk(&dirs).to_string().contains(&legacy.0));
}

#[test]
fn a_migration_that_meets_a_change_runs_when_the_change_lets_go() {
    let bench = Bench::new();
    let dirs = DataDirs::under(&bench.data_home());
    let (banners, legacy) = staged_v02_folder(&bench, &dirs, &bench.mods.join(BANNERS));
    let (state, events) = bench.open(bench.config(dirs.clone()));

    // A change holds the game folder while the scan ends.
    let turn = state.turn();
    let scan = {
        let state = std::sync::Arc::clone(&state);
        std::thread::spawn(move || state.scan_blocking())
    };
    wait_for(&events, LIBRARY_CHANGED, 1);
    assert!(scan.join().unwrap());
    assert!(
        bench.journal_on_disk(&dirs).to_string().contains(&legacy.0),
        "the migration waited for the change"
    );
    assert!(events.named(GAME_CHANGED).is_empty());
    drop(turn);
    assert_eq!(
        deploy::get_installed_impl(&state).unwrap().installs[0].option_id,
        banners,
        "and ran when it let go"
    );
    assert!(!bench.journal_on_disk(&dirs).to_string().contains(&legacy.0));
    assert_eq!(events.named(GAME_CHANGED).len(), 1);
    assert_relinked(&state, 1);
}

fn assert_relinked(state: &AppState, count: usize) {
    let check = overview::get_overview_impl(state)
        .unwrap()
        .checks
        .into_iter()
        .find(|c| c.id == "relinked")
        .expect("a relinked check");
    assert!(
        check.title.starts_with(&format!("{count} install")),
        "{}",
        check.title
    );
}
