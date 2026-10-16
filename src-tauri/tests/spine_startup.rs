//! The startup sequence: a crash's leftover intent rolled back at launch and
//! reported in `Overview.recovered`; a damaged journal refusing every change
//! (and never reset); the background wake-up — Fluffy found, default roots,
//! the scan, the library watcher catching a new download, the Nexus
//! auto-check that fails quietly offline.

mod support;

use std::time::{Duration, Instant};

use rift_core::RiftError;
use rift_deploy::{Intent, IntentAction, IntentStep, Journal};
use riftstone_lib::commands::{deploy, library, overview};
use riftstone_lib::dto::{CheckLevel, RecoveryAction, SearchFilters, SearchQuery};
use riftstone_lib::events::{RecordedEvents, LIBRARY_CHANGED, SCAN_PROGRESS, UPDATES_CHANGED};
use riftstone_lib::keys::MemoryKeyStore;
use riftstone_lib::persist::DataDirs;
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
    config.discovery = riftstone_lib::state::Discovery::Fixed {
        game_root: None,
        fluffy_root: Some(bench.fluffy.clone()),
    };
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
    assert!(deploy::set_game_root_impl(&state, &bench.fluffy.to_string_lossy()).is_err());
    let game = deploy::set_game_root_impl(&state, &bench.game.to_string_lossy()).unwrap();
    assert_eq!(game.source, riftstone_lib::dto::GameSource::Manual);
    assert!(deploy::plan_install_impl(&state, &id)
        .unwrap()
        .refused
        .is_none());
    assert!(overview::get_overview_impl(&state).unwrap().game.is_some());
}

/// The bench opened once, scanned, closed: the next open is no first launch,
/// and its library (from the cache) holds the Nexus download, linked by name.
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
fn a_folder_added_before_fluffy_is_found_survives_the_first_launch() {
    let bench = Bench::new();
    let dirs = DataDirs::under(&bench.data_home());
    let state = AppState::open(
        bench.config(dirs.clone()),
        Arc::new(RecordedEvents::default()),
        Box::new(MemoryKeyStore::default()),
    );
    // The window is up before the wake-up thread has found Fluffy.
    let kept = bench.dir.path().join("Mods I Keep");
    std::fs::create_dir_all(&kept).unwrap();
    let added = library::add_root_impl(&state, &kept.to_string_lossy()).unwrap();
    state.find_fluffy();

    let ids: Vec<String> = state.roots_snapshot().into_iter().map(|r| r.id).collect();
    assert_eq!(
        ids,
        vec![
            "fluffy".to_string(),
            "downloads".to_string(),
            added.id.clone()
        ],
        "the default roots join the player's folder, never replace it"
    );
    let stored: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dirs.roots_file()).unwrap()).unwrap();
    let stored_ids: Vec<&str> = stored["roots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(stored_ids, vec!["fluffy", "downloads", added.id.as_str()]);
}

#[cfg(unix)]
#[test]
fn an_unreadable_roots_file_is_never_written_over() {
    use std::os::unix::fs::PermissionsExt;

    let bench = Bench::new();
    let dirs = DataDirs::under(&bench.data_home());
    {
        // The player's own folders, from an earlier launch.
        let (state, _) = bench.open(bench.config(dirs.clone()));
        let kept = bench.dir.path().join("Mods I Keep");
        std::fs::create_dir_all(&kept).unwrap();
        library::add_root_impl(&state, &kept.to_string_lossy()).unwrap();
    }
    let roots_file = dirs.roots_file();
    let before = std::fs::read(&roots_file).unwrap();
    std::fs::set_permissions(&roots_file, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read(&roots_file).is_ok() {
        // Running as root: permissions do not stop a read, so there is no
        // unreadable file to stage on this bench.
        std::fs::set_permissions(&roots_file, std::fs::Permissions::from_mode(0o644)).unwrap();
        return;
    }

    let (state, _) = bench.open(bench.config(dirs.clone()));
    let home = overview::get_overview_impl(&state).unwrap();
    std::fs::set_permissions(&roots_file, std::fs::Permissions::from_mode(0o644)).unwrap();

    assert_eq!(
        std::fs::read(&roots_file).unwrap(),
        before,
        "the file holding the player's folders is left exactly as it was"
    );
    assert!(
        state.roots_snapshot().is_empty(),
        "an unreadable roots.json is no first launch: no default roots"
    );
    let check = home
        .checks
        .iter()
        .find(|c| c.id == "rootsUnreadable")
        .expect("the overview says the folders could not be read");
    assert_eq!(check.level, CheckLevel::Warn);
    assert!(check.detail.contains("roots.json"), "{}", check.detail);
    assert!(
        !home.checks.iter().any(|c| c.id == "roots"),
        "no 'add your first folder' while the player's folders are only unreadable"
    );
}

#[test]
fn a_damaged_roots_file_is_set_aside_and_the_defaults_come_back() {
    let bench = Bench::new();
    let dirs = DataDirs::under(&bench.data_home());
    std::fs::create_dir_all(&dirs.data).unwrap();
    std::fs::write(dirs.roots_file(), b"{ the roots, torn").unwrap();
    let (state, _) = bench.open(bench.config(dirs.clone()));
    let ids: Vec<String> = state.roots_snapshot().into_iter().map(|r| r.id).collect();
    assert_eq!(ids, vec!["fluffy".to_string(), "downloads".to_string()]);
    assert_eq!(
        std::fs::read(dirs.data.join("roots.json.damaged")).unwrap(),
        b"{ the roots, torn",
        "the damaged file is kept aside, never deleted"
    );
    assert!(state.held_files().is_empty(), "a damaged file is not held");
}

#[test]
fn the_wake_up_finds_fluffy_scans_and_the_watcher_sees_a_new_download() {
    let bench = Bench::new();
    let mut config = bench.config(DataDirs::under(&bench.data_home()));
    config.watch_roots = true;
    let events = Arc::new(RecordedEvents::default());
    let state = AppState::open(config, events.clone(), Box::new(MemoryKeyStore::default()));
    // Startup steps 1–3 are done; the window could ask already.
    assert!(overview::get_overview_impl(&state)
        .unwrap()
        .fluffy
        .is_none());

    state.wake();
    wait_for(&events, LIBRARY_CHANGED, 1);
    let home = overview::get_overview_impl(&state).unwrap();
    assert!(home.fluffy.is_some());
    assert_eq!(home.roots.len(), 2);
    assert_eq!(home.library.packages, 3);
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
fn a_root_holding_the_stones_own_data_or_the_game_is_refused() {
    let bench = Bench::new();
    let home = bench.dir.path().join("Mods and data");
    let dirs = DataDirs::under(&home.join("riftstone-data"));
    let (state, _) = bench.open(bench.config(dirs));
    std::fs::create_dir_all(home.join("riftstone-data")).unwrap();
    for refused in [
        home.clone(),
        home.join("riftstone-data"),
        bench.game.clone(),
        bench.game.join("pak_mods"),
        bench.game.parent().unwrap().to_path_buf(),
    ] {
        std::fs::create_dir_all(&refused).unwrap();
        let answer = library::add_root_impl(&state, &refused.to_string_lossy());
        assert!(
            matches!(answer, Err(RiftError::Refused { .. })),
            "{} must not become a library root: {answer:?}",
            refused.display()
        );
    }
    let fine = bench.dir.path().join("Lantern Mods");
    std::fs::create_dir_all(&fine).unwrap();
    assert!(library::add_root_impl(&state, &fine.to_string_lossy()).is_ok());
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
        &bench.downloads.join("Lantern Glow Recolor.zip"),
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
