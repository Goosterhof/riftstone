//! The startup sequence: a crash's leftover intent rolled back at launch and
//! reported in `Overview.recovered`; a damaged journal refusing every change
//! (and never reset); the one mods folder — made at launch when missing, its
//! leftover partial downloads removed, a folder that cannot be made or read
//! said plainly, v0.2's `roots.json` never touched; the background wake-up —
//! the scan, the mods-folder watcher catching a new mod, the game watcher,
//! the Nexus auto-check that fails quietly offline.

mod support;

use std::time::{Duration, Instant};

use rift_core::RiftError;
use rift_deploy::{Intent, IntentAction, IntentStep, Journal};
use riftstone_lib::commands::{deploy, library, overview};
use riftstone_lib::dto::{CheckAction, CheckLevel, RecoveryAction, SearchFilters, SearchQuery};
use riftstone_lib::events::{RecordedEvents, LIBRARY_CHANGED, SCAN_PROGRESS, UPDATES_CHANGED};
use riftstone_lib::keys::MemoryKeyStore;
use riftstone_lib::persist::DataDirs;
use riftstone_lib::play::Launched;
use riftstone_lib::state::AppState;
use std::sync::Arc;
use support::{write_zip, Bench, GatedEvents, ERMINE_MESH};

fn ermine(state: &AppState) -> String {
    library::search_impl(
        state,
        &SearchQuery {
            text: "ermine".into(),
            filters: SearchFilters::default(),
            limit: 0,
        },
    )[0]
    .option_id
    .clone()
}

/// Wait until `events` has heard `event` `count` times.
fn wait_for(events: &RecordedEvents, event: &str, count: usize) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while events.named(event).len() < count {
        assert!(Instant::now() < deadline, "{event} ×{count} never arrived");
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn an_interrupted_change_is_rolled_back_at_launch_and_reported() {
    let bench = Bench::new();
    let dirs = DataDirs::under(&bench.data_home());
    let (state, _) = bench.open(bench.config(dirs.clone()));
    assert!(state.scan_blocking());
    let id = ermine(&state);
    deploy::install_impl(&state, &id).unwrap();
    drop(state);

    // The state a crash leaves: an uninstall's intent saved, no byte moved.
    let mut journal = Journal::load(dirs.state_dir(), &bench.game).unwrap();
    let seq = journal.next_seq;
    journal.pending = Some(Intent {
        action: IntentAction::Uninstall,
        seq,
        option_id: rift_core::OptionId(id.clone()),
        touched: vec![ERMINE_MESH.into()],
        started_at: "2026-10-16T10:00:00Z".into(),
        option_name: "Ermine Cape".into(),
        steps: vec![IntentStep::Remove {
            game_path: ERMINE_MESH.into(),
            trash: format!("{ERMINE_MESH}.riftstone-trash-{seq}"),
        }],
        phase: 0,
        dirs: Vec::new(),
    });
    journal.next_seq = seq + 1;
    journal.save(dirs.state_dir()).unwrap();

    let (relaunched, _) = bench.open(bench.config(dirs.clone()));
    let home = overview::get_overview_impl(&relaunched).unwrap();
    let note = home
        .recovered
        .expect("the launch reports what it rolled back");
    assert_eq!(note.option_name, "Ermine Cape");
    assert_eq!(note.action, RecoveryAction::Uninstall);
    assert!(
        bench.game.join(ERMINE_MESH).is_file(),
        "the cape is whole again"
    );
    assert!(Journal::load(dirs.state_dir(), &bench.game)
        .unwrap()
        .pending
        .is_none());
    assert!(!home.checks.iter().any(|c| c.id == "interrupted"));
    assert_eq!(home.library.installed_options, 1, "still installed");
}

#[test]
fn a_damaged_journal_refuses_every_change_and_is_never_reset() {
    let bench = Bench::new();
    let dirs = DataDirs::under(&bench.data_home());
    std::fs::create_dir_all(&dirs.data).unwrap();
    let journal_file = rift_deploy::journal_path(dirs.state_dir(), &bench.game);
    std::fs::write(&journal_file, b"{ the journal, torn").unwrap();

    let (state, _) = bench.open(bench.config(dirs.clone()));
    assert!(state.scan_blocking());
    let home = overview::get_overview_impl(&state).unwrap();
    let check = home
        .checks
        .iter()
        .find(|c| c.id == "journal")
        .expect("the damaged journal is the first thing the home frame says");
    assert_eq!(check.level, CheckLevel::Error);
    assert_eq!(home.checks[0].level, CheckLevel::Error, "errors come first");

    let id = ermine(&state);
    let plan = deploy::plan_install_impl(&state, &id).unwrap();
    assert!(plan.refused.unwrap().contains("journal"));
    assert!(matches!(
        deploy::install_impl(&state, &id),
        Err(RiftError::Journal { .. })
    ));
    assert!(matches!(
        deploy::get_installed_impl(&state),
        Err(RiftError::Journal { .. })
    ));
    assert!(
        !bench.game.join(ERMINE_MESH).exists(),
        "nothing was written"
    );
    assert_eq!(
        std::fs::read(&journal_file).unwrap(),
        b"{ the journal, torn",
        "the only record of what was installed is left exactly as it was"
    );
}

#[test]
fn without_a_game_plans_are_refused_and_the_overview_asks_for_the_folder() {
    let bench = Bench::new();
    let mut config = bench.config(DataDirs::under(&bench.data_home()));
    config.discovery = riftstone_lib::state::Discovery::Fixed { game_root: None };
    let (state, _) = bench.open(config);
    assert!(state.scan_blocking());
    let home = overview::get_overview_impl(&state).unwrap();
    assert!(home.game.is_none());
    let check = home.checks.iter().find(|c| c.id == "game").unwrap();
    assert_eq!(check.level, CheckLevel::Error);
    assert_eq!(
        serde_json::to_value(check.action.as_ref().unwrap()).unwrap()["kind"],
        "pickGameFolder"
    );
    let id = ermine(&state);
    assert!(deploy::plan_install_impl(&state, &id)
        .unwrap()
        .refused
        .is_some());
    assert!(matches!(
        deploy::get_installed_impl(&state),
        Err(RiftError::GameNotFound)
    ));

    // Pointing at a folder without DD2.exe is refused; the bench's game is taken.
    assert!(deploy::set_game_root_impl(&state, &bench.mods.to_string_lossy()).is_err());
    let game = deploy::set_game_root_impl(&state, &bench.game.to_string_lossy()).unwrap();
    assert_eq!(game.source, riftstone_lib::dto::GameSource::Manual);
    assert!(deploy::plan_install_impl(&state, &id)
        .unwrap()
        .refused
        .is_none());
    assert!(overview::get_overview_impl(&state).unwrap().game.is_some());
}

/// The bench opened once, scanned, closed: the next open's library (from the
/// cache) holds the Nexus download, linked by name.
fn launched_before(bench: &Bench) -> DataDirs {
    let dirs = DataDirs::under(&bench.data_home());
    let (state, _) = bench.open(bench.config(dirs.clone()));
    assert!(state.scan_blocking());
    dirs
}

#[test]
fn the_launch_update_check_waits_for_a_scan_already_running() {
    let bench = Bench::new();
    let dirs = launched_before(&bench);
    // A scan that stops at its first progress report until the test says go.
    let gated = Arc::new(GatedEvents::closed_on(SCAN_PROGRESS));
    let state = AppState::open(
        bench.config(dirs),
        gated.clone(),
        Box::new(MemoryKeyStore::default()),
    );
    // The window (or the watcher) started a scan before the wake-up got to it.
    assert!(state.start_scan());
    wait_for(&gated.recorded, SCAN_PROGRESS, 1);
    state.wake();
    std::thread::sleep(Duration::from_millis(800));
    let early = gated.recorded.named(UPDATES_CHANGED).len();
    gated.open();
    wait_for(&gated.recorded, LIBRARY_CHANGED, 1);
    wait_for(&gated.recorded, UPDATES_CHANGED, 1);

    assert_eq!(
        early, 0,
        "the launch check ran while the first scan was still reading the library"
    );
    let order: Vec<String> = gated
        .recorded
        .all()
        .into_iter()
        .map(|(name, _)| name)
        .filter(|name| name == LIBRARY_CHANGED || name == UPDATES_CHANGED)
        .collect();
    assert_eq!(
        order[0], LIBRARY_CHANGED,
        "the check follows the scan: {order:?}"
    );
}

#[test]
fn the_wake_up_scans_watches_the_game_and_the_watcher_sees_a_new_mod() {
    let bench = Bench::new();
    let mut config = bench.config(DataDirs::under(&bench.data_home()));
    config.watch_library = true;
    let events = Arc::new(RecordedEvents::default());
    let state = AppState::open(config, events.clone(), Box::new(MemoryKeyStore::default()));
    // Startup steps 1–3 are done; the window could ask already.
    assert_eq!(
        overview::get_overview_impl(&state)
            .unwrap()
            .library
            .packages,
        0,
        "no scan and no cache yet"
    );
    let asked_at_open = bench.game_watch.asked();

    state.wake();
    wait_for(&events, LIBRARY_CHANGED, 1);
    let home = overview::get_overview_impl(&state).unwrap();
    assert_eq!(home.library.packages, 3);
    let deadline = Instant::now() + Duration::from_secs(20);
    while bench.game_watch.asked() < asked_at_open + 3 {
        assert!(Instant::now() < deadline, "the game watcher samples");
        std::thread::sleep(Duration::from_millis(10));
    }
    // The download carries a Nexus file name, the last check is "never":
    // the auto-check runs — against nothing — and fails quietly.
    wait_for(&events, UPDATES_CHANGED, 1);
    let home = overview::get_overview_impl(&state).unwrap();
    assert!(!home.nexus.checking);
    assert_eq!(home.nexus.last_check_ms, None, "a failed check is no check");

    write_zip(
        &bench.mods.join("Bakbattahl Banners.zip"),
        &[("Banners/natives/stm/banner/banner.tex.251211553", b"banner")],
    );
    wait_for(&events, LIBRARY_CHANGED, 2);
    assert_eq!(
        overview::get_overview_impl(&state)
            .unwrap()
            .library
            .packages,
        4
    );
}

/// The intent a crash would leave for the cape's uninstall: saved, no byte moved.
fn uninstall_intent(journal: &Journal, id: &str) -> Intent {
    let seq = journal.next_seq;
    Intent {
        action: IntentAction::Uninstall,
        seq,
        option_id: rift_core::OptionId(id.to_string()),
        touched: vec![ERMINE_MESH.into()],
        started_at: "2026-10-16T10:00:00Z".into(),
        option_name: "Ermine Cape".into(),
        steps: vec![IntentStep::Remove {
            game_path: ERMINE_MESH.into(),
            trash: format!("{ERMINE_MESH}.riftstone-trash-{seq}"),
        }],
        phase: 0,
        dirs: Vec::new(),
    }
}

#[test]
fn a_change_that_stops_mid_way_keeps_its_intent_and_refuses_the_next_change() {
    let bench = Bench::new();
    let dirs = DataDirs::under(&bench.data_home());
    let (state, _) = bench.open(bench.config(dirs.clone()));
    assert!(state.scan_blocking());
    let id = ermine(&state);
    deploy::install_impl(&state, &id).unwrap();

    // An engine that saves its intent and then panics, the way a decoder can
    // on a damaged archive in the middle of an extract.
    let state_dir = dirs.state_dir().to_path_buf();
    let stopped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        state.deploy(|_, journal| -> Result<(), RiftError> {
            journal.pending = Some(uninstall_intent(journal, &id));
            journal.next_seq += 1;
            journal.save(&state_dir).unwrap();
            panic!("a decoder gave up mid-archive");
        })
    }));
    assert!(
        stopped.is_err(),
        "the panic still reaches the command layer"
    );

    // The world now holds the journal on disk, intent and all.
    let refused = deploy::uninstall_impl(&state, &id);
    assert!(
        matches!(refused, Err(RiftError::Refused { .. })),
        "the next change waits for the rollback: {refused:?}"
    );
    assert!(
        Journal::load(dirs.state_dir(), &bench.game)
            .unwrap()
            .pending
            .is_some(),
        "nothing wrote over the intent the rollback needs"
    );
    let home = overview::get_overview_impl(&state).unwrap();
    assert!(home.checks.iter().any(|c| c.id == "interrupted"));
    drop(state);

    let (relaunched, _) = bench.open(bench.config(dirs.clone()));
    let home = overview::get_overview_impl(&relaunched).unwrap();
    assert_eq!(
        home.recovered.map(|note| note.option_name).as_deref(),
        Some("Ermine Cape"),
        "the next launch rolls it back"
    );
    assert!(bench.game.join(ERMINE_MESH).is_file());
    assert!(!home.checks.iter().any(|c| c.id == "interrupted"));
}

#[test]
fn a_scan_that_changes_nothing_leaves_the_cache_file_alone() {
    let bench = Bench::new();
    let dirs = DataDirs::under(&bench.data_home());
    let (state, _) = bench.open(bench.config(dirs.clone()));
    assert!(state.scan_blocking());
    let cache = dirs.catalog_cache_file();
    let written = std::fs::metadata(&cache).unwrap().modified().unwrap();
    std::thread::sleep(Duration::from_millis(30));
    assert!(state.scan_blocking());
    assert_eq!(
        std::fs::metadata(&cache).unwrap().modified().unwrap(),
        written,
        "the same library is not written again"
    );
    write_zip(
        &bench.mods.join("Lantern Glow Recolor Extra.zip"),
        &[(
            "Lantern Glow/natives/stm/lantern/glow.tex.251211553",
            b"tex",
        )],
    );
    assert!(state.scan_blocking());
    assert_ne!(
        std::fs::metadata(&cache).unwrap().modified().unwrap(),
        written,
        "a changed library is"
    );
}

#[test]
fn a_missing_mods_folder_is_made_at_launch_and_said_empty() {
    let bench = Bench::new();
    let mut config = bench.config(DataDirs::under(&bench.data_home()));
    config.library_dir = bench.dir.path().join("Fresh Install").join("mods");
    let (state, _) = bench.open(config.clone());
    assert!(config.library_dir.is_dir(), "made at launch");
    assert!(state.scan_blocking());
    let home = overview::get_overview_impl(&state).unwrap();
    assert!(home.mods.exists);
    assert_eq!(home.mods.path, config.library_dir.to_string_lossy());
    assert_eq!(home.library.packages, 0);
    let check = home
        .checks
        .iter()
        .find(|c| c.id == "mods")
        .expect("an empty mods folder is said");
    assert_eq!(check.level, CheckLevel::Info);
    assert_eq!(check.title, "The mods folder is empty");
    assert_eq!(check.action, Some(CheckAction::OpenModsFolder));
    assert!(check
        .detail
        .contains(&*config.library_dir.to_string_lossy()));
    let view = library::get_library_impl(&state);
    assert!(view.mods.exists && view.packages.is_empty());
}

#[test]
fn leftover_partial_downloads_go_at_launch_and_nothing_else_is_touched() {
    let bench = Bench::new();
    std::fs::write(bench.mods.join(".riftstone-7c1e.part"), b"half a download").unwrap();
    std::fs::write(
        bench.mods.join("Pawn Banter Quieter.zip.part"),
        b"a browser's",
    )
    .unwrap();
    std::fs::write(bench.mods.join("desktop.ini"), b"[.ShellClassInfo]").unwrap();
    let before: Vec<_> = Bench::snapshot(&bench.mods)
        .into_iter()
        .filter(|(path, _)| path != ".riftstone-7c1e.part")
        .collect();
    let (state, _) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    assert!(state.scan_blocking());
    assert!(!bench.mods.join(".riftstone-7c1e.part").exists());
    assert_eq!(
        Bench::snapshot(&bench.mods).into_iter().collect::<Vec<_>>(),
        before,
        "only the stone's own partial download went"
    );
    assert_eq!(
        overview::get_overview_impl(&state)
            .unwrap()
            .library
            .packages,
        3,
        "a download in flight and desktop.ini are no packages"
    );
}

#[test]
fn a_mods_folder_that_cannot_be_made_is_an_error_never_a_crash() {
    let bench = Bench::new();
    let blocked = bench.dir.path().join("not a folder");
    std::fs::write(&blocked, b"a file where the install folder should be").unwrap();
    let mut config = bench.config(DataDirs::under(&bench.data_home()));
    config.library_dir = blocked.join("mods");
    let (state, _) = bench.open(config.clone());
    assert!(state.scan_blocking());
    let home = overview::get_overview_impl(&state).unwrap();
    assert!(!home.mods.exists);
    assert_eq!(home.library.packages, 0, "an empty shelf");
    let check = home.checks.iter().find(|c| c.id == "mods").unwrap();
    assert_eq!(check.level, CheckLevel::Error);
    assert_eq!(
        check.action, None,
        "nothing the window can press would help"
    );
    assert!(
        check
            .detail
            .contains(&*config.library_dir.to_string_lossy()),
        "{}",
        check.detail
    );
    let view = library::get_library_impl(&state);
    assert!(!view.mods.exists && view.packages.is_empty());
    assert!(
        library::open_mods_folder_impl(&state).is_err(),
        "still cannot be made"
    );
    assert!(bench.launcher.seen().is_empty(), "nothing was shown");
}

#[cfg(unix)]
#[test]
fn a_mods_folder_that_cannot_be_listed_warns() {
    use std::os::unix::fs::PermissionsExt;

    let bench = Bench::new();
    let (state, _) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    std::fs::set_permissions(&bench.mods, std::fs::Permissions::from_mode(0o000)).unwrap();
    let listable = std::fs::read_dir(&bench.mods).is_ok();
    let home = overview::get_overview_impl(&state).unwrap();
    std::fs::set_permissions(&bench.mods, std::fs::Permissions::from_mode(0o755)).unwrap();
    if listable {
        // Running as root: permissions do not stop a listing on this bench.
        return;
    }
    let check = home.checks.iter().find(|c| c.id == "mods").unwrap();
    assert_eq!(check.level, CheckLevel::Warn);
    assert!(home.mods.exists);
}

#[test]
fn open_mods_folder_makes_a_missing_folder_and_shows_it() {
    let bench = Bench::new();
    let mut config = bench.config(DataDirs::under(&bench.data_home()));
    config.library_dir = bench.dir.path().join("Second Install").join("mods");
    let (state, _) = bench.open(config.clone());
    std::fs::remove_dir(&config.library_dir).unwrap();
    library::open_mods_folder_impl(&state).unwrap();
    assert!(config.library_dir.is_dir(), "made again first");
    assert_eq!(
        bench.launcher.seen(),
        vec![Launched::Folder(config.library_dir.clone())]
    );
    library::open_mods_folder_impl(&state).unwrap();
    assert_eq!(bench.launcher.seen().len(), 2);
}

#[test]
fn a_mods_folder_deleted_after_launch_is_made_again_watched_and_read() {
    let bench = Bench::new();
    let mut config = bench.config(DataDirs::under(&bench.data_home()));
    config.watch_library = true;
    let events = Arc::new(RecordedEvents::default());
    let state = AppState::open(config, events.clone(), Box::new(MemoryKeyStore::default()));
    state.wake();
    wait_for(&events, LIBRARY_CHANGED, 1);
    assert!(state.watching_library());

    // The player deletes the whole mods folder while the stone runs; the
    // watcher's rescan finds nothing there.
    std::fs::remove_dir_all(&bench.mods).unwrap();
    wait_for(&events, LIBRARY_CHANGED, 2);
    state.wait_until_scanned();
    let home = overview::get_overview_impl(&state).unwrap();
    assert_eq!(home.library.packages, 0);
    assert!(!home.mods.exists, "not there at the moment of the read");
    let check = home.checks.iter().find(|c| c.id == "mods").unwrap();
    assert_eq!(
        (check.level, check.action.clone()),
        (CheckLevel::Warn, Some(CheckAction::OpenModsFolder)),
        "{check:?}"
    );
    assert!(
        check.detail.contains("Open mods folder") && !check.detail.contains("could not be made"),
        "the way back, not a folder that cannot be made: {}",
        check.detail
    );
    assert!(check.detail.contains(&*bench.mods.to_string_lossy()));

    // Open mods folder makes it again, watches the new folder and reads it.
    library::open_mods_folder_impl(&state).unwrap();
    assert!(bench.mods.is_dir());
    state.wait_until_scanned();
    write_zip(
        &bench.mods.join("Lantern Glow Recolor.zip"),
        &[(
            "Lantern Glow/natives/stm/effect/lantern/lantern_glow.tex.251211553",
            b"glow",
        )],
    );
    let deadline = Instant::now() + Duration::from_secs(20);
    while overview::get_overview_impl(&state)
        .unwrap()
        .library
        .packages
        != 1
    {
        assert!(
            Instant::now() < deadline,
            "the watcher on the folder made again sees a new mod"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    let home = overview::get_overview_impl(&state).unwrap();
    assert!(home.mods.exists);
    assert!(
        home.checks.iter().all(|c| c.id != "mods"),
        "it holds a package"
    );
}

/// Wait until the overview counts `count` packages; panics with `what`.
fn wait_for_packages(state: &AppState, count: usize, what: &str) {
    support::wait_for(Duration::from_secs(20), what, || {
        overview::get_overview_impl(state).unwrap().library.packages == count
    });
}

#[test]
fn a_mods_folder_made_by_hand_after_launch_is_watched_from_the_next_scan() {
    let bench = Bench::new();
    let blocked = bench.dir.path().join("not a folder");
    std::fs::write(&blocked, b"a file where the install folder should be").unwrap();
    let mut config = bench.config(DataDirs::under(&bench.data_home()));
    config.library_dir = blocked.join("mods");
    config.watch_library = true;
    let events = Arc::new(RecordedEvents::default());
    let state = AppState::open(
        config.clone(),
        events.clone(),
        Box::new(MemoryKeyStore::default()),
    );
    state.wake();
    wait_for(&events, LIBRARY_CHANGED, 1);
    assert!(!overview::get_overview_impl(&state).unwrap().mods.exists);
    assert!(state.library_unwatched(), "nothing to watch at launch");

    // The player clears the way, makes the folder by hand and presses Scan.
    std::fs::remove_file(&blocked).unwrap();
    std::fs::create_dir_all(&config.library_dir).unwrap();
    overview::rescan_impl(&state);
    wait_for(&events, LIBRARY_CHANGED, 2);
    state.wait_until_scanned();
    let home = overview::get_overview_impl(&state).unwrap();
    assert!(home.mods.exists);
    assert_eq!(home.library.packages, 0);
    assert!(state.watching_library(), "Scan armed the watcher first");

    // A mod dropped in afterwards is read without another press.
    write_zip(
        &config.library_dir.join("Lantern Glow Recolor.zip"),
        &[(
            "Lantern Glow/natives/stm/effect/lantern/lantern_glow.tex.251211553",
            b"glow",
        )],
    );
    wait_for_packages(&state, 1, "the watcher Scan armed sees a new mod");
}

#[test]
fn a_mods_folder_put_back_by_hand_is_watched_and_read_again_by_open_mods_folder() {
    let bench = Bench::new();
    let mut config = bench.config(DataDirs::under(&bench.data_home()));
    config.watch_library = true;
    let events = Arc::new(RecordedEvents::default());
    let state = AppState::open(config, events.clone(), Box::new(MemoryKeyStore::default()));
    state.wake();
    wait_for(&events, LIBRARY_CHANGED, 1);
    assert!(state.watching_library());

    // Deleted while the stone runs: the scan that finds it gone lets the
    // watcher of the folder that went go.
    std::fs::remove_dir_all(&bench.mods).unwrap();
    wait_for(&events, LIBRARY_CHANGED, 2);
    state.wait_until_scanned();
    assert_eq!(
        overview::get_overview_impl(&state)
            .unwrap()
            .library
            .packages,
        0
    );
    support::wait_for(
        Duration::from_secs(20),
        "the scan that found the folder gone lets its watcher go",
        || state.library_unwatched(),
    );

    // The player makes it again by hand, a mod in it, and presses Open mods
    // folder: the new folder is watched and read.
    std::fs::create_dir_all(&bench.mods).unwrap();
    write_zip(
        &bench.mods.join("Lantern Glow Recolor.zip"),
        &[(
            "Lantern Glow/natives/stm/effect/lantern/lantern_glow.tex.251211553",
            b"glow",
        )],
    );
    let heard = events.named(LIBRARY_CHANGED).len();
    library::open_mods_folder_impl(&state).unwrap();
    wait_for(&events, LIBRARY_CHANGED, heard + 1);
    wait_for_packages(&state, 1, "Open mods folder reads the folder put back");
    assert!(state.watching_library());

    // A later mod is seen without another press.
    write_zip(
        &bench.mods.join("Bakbattahl Banners.zip"),
        &[("Banners/natives/stm/banner/banner.tex.251211553", b"banner")],
    );
    wait_for_packages(
        &state,
        2,
        "the watcher on the folder put back sees a new mod",
    );
}

#[test]
fn v02s_roots_file_is_never_read_written_or_deleted() {
    let bench = Bench::new();
    let dirs = DataDirs::under(&bench.data_home());
    // v0.2 kept the player's library folders here; one of them holds a mod.
    let elsewhere = bench.dir.path().join("Mods I Keep");
    std::fs::create_dir_all(&elsewhere).unwrap();
    write_zip(
        &elsewhere.join("Bakbattahl Banners.zip"),
        &[("Banners/natives/stm/banner/banner.tex.251211553", b"banner")],
    );
    let roots = serde_json::to_vec_pretty(&serde_json::json!({
        "schema": 1,
        "roots": [{ "id": "custom-1", "path": elsewhere.to_string_lossy(),
                    "label": "Mods I Keep", "kind": "custom", "dd2ShapedOnly": false }]
    }))
    .unwrap();
    std::fs::create_dir_all(&dirs.data).unwrap();
    let roots_file = dirs.data.join("roots.json");
    std::fs::write(&roots_file, &roots).unwrap();

    let (state, _) = bench.open(bench.config(dirs.clone()));
    assert!(state.scan_blocking());
    let home = overview::get_overview_impl(&state).unwrap();
    assert_eq!(home.library.packages, 3, "the mods folder only");
    assert!(library::search_impl(
        &state,
        &SearchQuery {
            text: "banner".into(),
            filters: SearchFilters::default(),
            limit: 0,
        }
    )
    .is_empty());
    drop(state);
    let (again, _) = bench.open(bench.config(dirs.clone()));
    assert!(again.scan_blocking());
    drop(again);
    assert_eq!(
        std::fs::read(&roots_file).unwrap(),
        roots,
        "left exactly as v0.2 wrote it"
    );
    assert!(!dirs.data.join("roots.json.damaged").exists());
}
