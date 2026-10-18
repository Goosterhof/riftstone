//! The investor's v0.2 data keeps working (WINDOW-CONTRACT § v0.3 Legacy
//! data): a schema-1 journal with an adopted install loads, lists as adopted
//! and uninstalls; after a scan, a folder package's v0.2 id becomes its v0.3
//! id in the journal and in `nexus.json` (exactly), and an install whose
//! package moved — the whole mods folder moved, or an adopted stand-in — is
//! found again by its files (the relink pass), in one journal save. Where
//! the stone fingerprinted what it installed, the candidate's entries are
//! read and their fingerprints (size, first and last MiB) must match: a
//! newer version that kept every size takes an install over only when every
//! recorded file also kept its first and last MiB.
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
use riftstone_lib::events::{
    RecordedEvents, GAME_CHANGED, LIBRARY_CHANGED, SCAN_PROGRESS, UPDATES_CHANGED,
};
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
    assert_eq!(
        state.relink_checks_read(),
        0,
        "no fingerprint recorded: its sizes are all there is, and nothing is read"
    );
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
    assert_eq!(
        state.relink_checks_read(),
        1,
        "the stone fingerprinted the banner: the moved folder's bytes were read and matched"
    );
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

#[test]
fn a_migration_that_meets_plays_probe_runs_when_the_probe_lets_go() {
    // Play's plan asks whether a change holds the game folder by trying its
    // lock for a moment. A scan's migration that meets that moment is put
    // off; the probe must run it as it lets go, as a change's turn does.
    // Hammered from four threads across a whole scan, three times over: the
    // migration meets a probe nearly every time.
    for trial in 1..=3 {
        let bench = Bench::new();
        let dirs = DataDirs::under(&bench.data_home());
        let (banners, legacy) = staged_v02_folder(&bench, &dirs, &bench.mods.join(BANNERS));
        let (state, events) = bench.open(bench.config(dirs.clone()));
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        // The probes and the scan start together, and each probe asks at
        // least once before it looks at `stop`. A busy runner once scheduled a
        // probe only after the scan had ended, so it asked nothing and the
        // Sentinel failed (11 of 40 runs on two loaded cores).
        let start = std::sync::Arc::new(std::sync::Barrier::new(5));
        let probes: Vec<_> = (0..4)
            .map(|_| {
                let state = std::sync::Arc::clone(&state);
                let stop = std::sync::Arc::clone(&stop);
                let start = std::sync::Arc::clone(&start);
                std::thread::spawn(move || {
                    start.wait();
                    let mut asked = 0u64;
                    loop {
                        let _ = state.deploy_busy();
                        asked += 1;
                        if stop.load(std::sync::atomic::Ordering::SeqCst) {
                            break asked;
                        }
                    }
                })
            })
            .collect();
        start.wait();
        assert!(state.scan_blocking());
        stop.store(true, std::sync::atomic::Ordering::SeqCst);
        for probe in probes {
            assert!(
                probe.join().unwrap() > 0,
                "trial {trial}: every probe asked once the scan began"
            );
        }
        assert_eq!(
            deploy::get_installed_impl(&state).unwrap().installs[0].option_id,
            banners,
            "trial {trial}: the put-off migration ran"
        );
        let journal = bench.journal_on_disk(&dirs);
        assert!(
            !journal.to_string().contains(&legacy.0),
            "trial {trial}: re-keyed on disk: {journal}"
        );
        assert_eq!(events.named(GAME_CHANGED).len(), 1, "trial {trial}");
        assert_relinked(&state, 1);
    }
}

/// The bench's Lantern Glow Recolor installed by the stone, the window's
/// events cleared: its option id and the archive it came from.
fn installed_lantern(
    bench: &Bench,
) -> (
    std::sync::Arc<AppState>,
    std::sync::Arc<RecordedEvents>,
    String,
) {
    let dirs = DataDirs::under(&bench.data_home());
    let (state, events) = bench.open(bench.config(dirs));
    assert!(state.scan_blocking());
    let lantern = option_named(&state, "Lantern Glow Recolor");
    deploy::install_impl(&state, &lantern).unwrap();
    events.clear();
    (state, events, lantern)
}

#[test]
fn an_install_whose_package_is_there_but_unreadable_keeps_its_id() {
    let bench = Bench::new();
    let (state, events, lantern) = installed_lantern(&bench);
    // A byte-identical copy under another name, and the original damaged in
    // place (zero-filled at its length: the same package id, no options).
    let original = bench.mods.join("Lantern Glow Recolor.zip");
    fs::copy(
        &original,
        bench.mods.join("Lantern Glow Recolor (copy).zip"),
    )
    .unwrap();
    let length = fs::metadata(&original).unwrap().len();
    fs::write(&original, vec![0u8; usize::try_from(length).unwrap()]).unwrap();
    assert!(state.scan_blocking());

    let world = deploy::get_installed_impl(&state).unwrap();
    assert_eq!(
        world.installs[0].option_id, lantern,
        "its package is still in the mods folder, unreadable this scan: it stays"
    );
    assert!(events.named(GAME_CHANGED).is_empty(), "nothing moved");
    assert!(!overview::get_overview_impl(&state)
        .unwrap()
        .checks
        .iter()
        .any(|c| c.id == "relinked"));
}

#[test]
fn a_newer_archive_with_other_files_never_takes_over_an_install() {
    let bench = Bench::new();
    let (state, events, lantern) = installed_lantern(&bench);
    // Version 1.1 of the same mod beside it (the same option, a pak of
    // another size: three entries where 1.0's has two), then 1.0 deleted.
    let newer = support::pak(&[
        rift_pak::path_hash("natives/stm/effect/lantern/lantern_glow_amber.tex.251211553"),
        rift_pak::path_hash("natives/stm/effect/lantern/lantern_glow_ember.tex.251211553"),
        rift_pak::path_hash("natives/stm/effect/lantern/lantern_glow_ash.tex.251211553"),
    ]);
    support::write_zip(
        &bench.mods.join("Lantern Glow Recolor 1.1.zip"),
        &[
            (
                "Lantern Glow/modinfo.ini",
                b"name=Lantern Glow Recolor\r\nversion=1.1\r\n",
            ),
            ("Lantern Glow/LanternGlow.pak", &newer),
        ],
    );
    fs::remove_file(bench.mods.join("Lantern Glow Recolor.zip")).unwrap();
    assert!(state.scan_blocking());

    let world = deploy::get_installed_impl(&state).unwrap();
    assert_eq!(world.installs[0].option_id, lantern, "keeps its id");
    assert!(world.installs[0].package_missing, "its own archive is gone");
    assert!(events.named(GAME_CHANGED).is_empty());
    assert!(!overview::get_overview_impl(&state)
        .unwrap()
        .checks
        .iter()
        .any(|c| c.id == "relinked"));
}

/// The recheck's probe: Lantern Glow Recolor 2.1 installed (a pak of two
/// entries, 112 bytes); 2.2 lands beside it, the same option and layout, a
/// pak of two OTHER entries at the very same 112 bytes; then 2.1 is deleted.
/// The sizes fit; the bytes do not.
#[test]
fn a_newer_archive_of_the_same_size_never_takes_over_an_install() {
    let bench = Bench::new();
    let (state, events, lantern) = installed_lantern(&bench);
    let older = support::pak(&[
        rift_pak::path_hash(support::ERMINE_MESH),
        rift_pak::path_hash("natives/stm/effect/lantern/lantern_glow.tex.251211553"),
    ]);
    let newer = support::pak(&[
        rift_pak::path_hash("natives/stm/effect/lantern/lantern_glow_amber.tex.251211553"),
        rift_pak::path_hash("natives/stm/effect/lantern/lantern_glow_ember.tex.251211553"),
    ]);
    assert_eq!(newer.len(), older.len(), "the same size");
    assert_ne!(newer, older, "other bytes");
    support::write_zip(
        &bench.mods.join("Lantern Glow Recolor 2.2.zip"),
        &[
            (
                "Lantern Glow/modinfo.ini",
                b"name=Lantern Glow Recolor\r\nversion=2.2\r\n",
            ),
            ("Lantern Glow/LanternGlow.pak", &newer),
        ],
    );
    fs::remove_file(bench.mods.join("Lantern Glow Recolor.zip")).unwrap();
    assert!(state.scan_blocking());

    let world = deploy::get_installed_impl(&state).unwrap();
    assert_eq!(world.installs.len(), 1);
    assert_eq!(world.installs[0].option_id, lantern, "keeps its id");
    assert!(world.installs[0].package_missing, "its own archive is gone");
    assert_eq!(world.installs[0].version.as_deref(), Some("2.1"));
    let in_game: Vec<Vec<u8>> = fs::read_dir(bench.game.join("pak_mods"))
        .unwrap()
        .map(|entry| fs::read(entry.unwrap().path()).unwrap())
        .collect();
    assert_eq!(in_game, vec![older], "the game folder holds 2.1's pak");
    assert!(events.named(GAME_CHANGED).is_empty(), "nothing moved");
    assert!(!overview::get_overview_impl(&state)
        .unwrap()
        .checks
        .iter()
        .any(|c| c.id == "relinked"));
    assert_eq!(state.relink_checks_read(), 1, "2.2's pak was read once");
    assert!(
        !DataDirs::under(&bench.data_home())
            .relink_scratch()
            .exists(),
        "the scratch folder went with the scan"
    );

    // The verdict holds for the session: the next scan reads nothing again.
    assert!(state.scan_blocking());
    assert_eq!(state.relink_checks_read(), 1);
    assert_eq!(
        deploy::get_installed_impl(&state).unwrap().installs[0].option_id,
        lantern
    );
    assert!(events.named(GAME_CHANGED).is_empty());
}

/// The bench's Vernworth Cloaks Collection, every entry as the bench writes
/// it, the Ermine Cape's mesh given.
fn cloaks_collection(path: &Path, ermine_mesh: &[u8]) {
    let screenshot = support::png(640, 360);
    support::write_zip(
        path,
        &[
            ("Vernworth Cloaks Collection/README.txt", b"Read me by the fire."),
            (
                "Vernworth Cloaks Collection/Cloaks/modinfo.ini",
                b"name=Cloaks\r\nDummyMod=True\r\n",
            ),
            (
                "Vernworth Cloaks Collection/Cloaks/Ermine Cape/modinfo.ini",
                b"name=Ermine Cape\r\naddonfor=Cloaks\r\nauthor=Vern\r\nversion=2\r\ndescription=A white cape   with black tips,\tfor the cold roads north of Vernworth.\r\nscreenshot=screenshot.png\r\n",
            ),
            (
                "Vernworth Cloaks Collection/Cloaks/Ermine Cape/screenshot.png",
                &screenshot,
            ),
            (
                &format!(
                    "Vernworth Cloaks Collection/Cloaks/Ermine Cape/{}",
                    support::ERMINE_MESH
                ),
                ermine_mesh,
            ),
            (
                "Vernworth Cloaks Collection/Cloaks/Wool Hood/modinfo.ini",
                b"name=Wool Hood\r\naddonfor=Cloaks\r\n",
            ),
            (
                &format!("Vernworth Cloaks Collection/Cloaks/Wool Hood/{WOOL_MESH}"),
                b"wool hood mesh bytes",
            ),
        ],
    );
}

/// The same probe with a loose file: the Ermine Cape installed from the
/// collection; version 2 of the collection lands with the same layout and
/// the cape's mesh at the very same 22 bytes, other ones; the first copy is
/// deleted.
#[test]
fn a_newer_collection_whose_mesh_kept_its_size_never_takes_over_an_install() {
    let bench = Bench::new();
    let dirs = DataDirs::under(&bench.data_home());
    let (state, events) = bench.open(bench.config(dirs));
    assert!(state.scan_blocking());
    let ermine = option_named(&state, "Ermine Cape");
    deploy::install_impl(&state, &ermine).unwrap();
    events.clear();
    let installed = fs::read(bench.game.join(support::ERMINE_MESH)).unwrap();
    let recut = b"ERMINE CAPE MESH BYTES";
    assert_eq!(recut.len(), installed.len(), "the same size");
    cloaks_collection(&bench.mods.join("Vernworth Cloaks Collection 2.zip"), recut);
    fs::remove_file(bench.mods.join("Vernworth Cloaks Collection.zip")).unwrap();
    assert!(state.scan_blocking());

    let world = deploy::get_installed_impl(&state).unwrap();
    assert_eq!(world.installs.len(), 1);
    assert_eq!(world.installs[0].option_id, ermine, "keeps its id");
    assert!(world.installs[0].package_missing);
    assert_eq!(
        fs::read(bench.game.join(support::ERMINE_MESH)).unwrap(),
        installed,
        "the game folder holds the first cut"
    );
    assert!(events.named(GAME_CHANGED).is_empty(), "nothing moved");
    assert!(!overview::get_overview_impl(&state)
        .unwrap()
        .checks
        .iter()
        .any(|c| c.id == "relinked"));
    assert_eq!(state.relink_checks_read(), 1);
}

#[test]
fn the_same_archive_under_another_name_is_found_again_by_its_bytes() {
    let bench = Bench::new();
    let (state, events, lantern) = installed_lantern(&bench);
    fs::rename(
        bench.mods.join("Lantern Glow Recolor.zip"),
        bench.mods.join("Lantern Glow Recolor (kept).zip"),
    )
    .unwrap();
    assert!(state.scan_blocking());

    let kept = option_named(&state, "Lantern Glow Recolor");
    assert_ne!(kept, lantern, "another file name, another package id");
    let world = deploy::get_installed_impl(&state).unwrap();
    assert_eq!(world.installs[0].option_id, kept, "relinked by its bytes");
    assert!(!world.installs[0].package_missing);
    assert_eq!(
        state.relink_checks_read(),
        1,
        "its pak was read and matched"
    );
    assert_eq!(events.named(GAME_CHANGED).len(), 1);
    assert_relinked(&state, 1);
    assert!(!DataDirs::under(&bench.data_home())
        .relink_scratch()
        .exists());
}

#[test]
fn a_relink_that_reads_bytes_names_its_check_in_the_scans_progress() {
    let bench = Bench::new();
    let (state, events, _) = installed_lantern(&bench);
    fs::rename(
        bench.mods.join("Lantern Glow Recolor.zip"),
        bench.mods.join("Lantern Glow Recolor (kept).zip"),
    )
    .unwrap();
    assert!(state.scan_blocking());
    assert_eq!(state.relink_checks_read(), 1);

    let heard = events.all();
    let checking = "checking Lantern Glow Recolor (kept).zip against the installed files (1 of 1)";
    let at = heard
        .iter()
        .position(|(name, payload)| name == SCAN_PROGRESS && payload["current"] == checking)
        .unwrap_or_else(|| panic!("no scan-progress names the check: {heard:?}"));
    let ended = heard
        .iter()
        .position(|(name, _)| name == LIBRARY_CHANGED)
        .expect("the scan ended");
    assert!(at < ended, "told while the scan runs: {heard:?}");
    let packages = heard[..at]
        .iter()
        .rev()
        .find(|(name, _)| name == SCAN_PROGRESS)
        .map(|(_, payload)| payload.clone())
        .expect("the scan's own progress first");
    let told = &heard[at].1;
    assert_eq!(
        (&told["done"], &told["total"]),
        (&packages["done"], &packages["total"]),
        "the scan's counts stand"
    );
    assert_ne!(packages["current"], checking);
}

#[test]
fn a_relink_read_while_a_change_holds_the_game_folder_lands_when_it_lets_go() {
    // The bytes are read before the migration asks for the deploy lock: a
    // change holding it never delays the read, and the change's end applies
    // the verdict without reading again.
    let bench = Bench::new();
    let (state, events, lantern) = installed_lantern(&bench);
    fs::rename(
        bench.mods.join("Lantern Glow Recolor.zip"),
        bench.mods.join("Lantern Glow Recolor (kept).zip"),
    )
    .unwrap();
    let turn = state.turn();
    let scan = {
        let state = std::sync::Arc::clone(&state);
        std::thread::spawn(move || state.scan_blocking())
    };
    wait_for(&events, LIBRARY_CHANGED, 1);
    assert!(scan.join().unwrap());
    assert_eq!(
        state.relink_checks_read(),
        1,
        "read with the lock held elsewhere"
    );
    assert!(
        events.named(GAME_CHANGED).is_empty(),
        "the migration waited"
    );
    drop(turn);
    let world = deploy::get_installed_impl(&state).unwrap();
    assert_ne!(world.installs[0].option_id, lantern);
    assert!(
        !world.installs[0].package_missing,
        "and landed when it let go"
    );
    assert_eq!(
        state.relink_checks_read(),
        1,
        "on the verdict, unread again"
    );
    assert_eq!(events.named(GAME_CHANGED).len(), 1);
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
    assert_eq!(
        check.detail,
        "Each was known by an id the mods folder no longer answers to (an id from before v0.3, a folder or archive renamed), and was matched to the one option there that installs the same files: each at its game path, the same size, and the same fingerprint (its size and its first and last MiB) wherever the Riftstone recorded one. So it shows, updates and uninstalls as before. Nothing in the game folder changed.",
        "the check names the fingerprint it matched, never every byte"
    );
}
