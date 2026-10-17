//! Play (v0.3) through the bench's recording launcher, and the game watcher:
//! the route Play takes and its warnings, a start that the watcher then sees
//! (one `rift://game-changed` per start or stop, never per sample), and every
//! refusal — the game already runs, a change holds the game folder, an
//! install is under way, no game at all. Nothing is ever really started.

mod support;

use std::time::{Duration, Instant};

use rift_core::RiftError;
use rift_deploy::{GameInstall, GameSource, LaunchRoute as Route};
use riftstone_lib::commands::{deploy, library, overview, play};
use riftstone_lib::dto::{LaunchRoute, LaunchWarning, PlanWarning, SearchFilters, SearchQuery};
use riftstone_lib::events::{RecordedEvents, GAME_CHANGED};
use riftstone_lib::persist::DataDirs;
use riftstone_lib::play::Launched;
use riftstone_lib::state::Discovery;
use support::{option_named, Bench};

fn wait_for(events: &RecordedEvents, event: &str, count: usize) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while events.named(event).len() < count {
        assert!(Instant::now() < deadline, "{event} ×{count} never arrived");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Long enough for the bench's watcher (every 40 ms) to sample many times.
fn several_samples() {
    std::thread::sleep(Duration::from_millis(400));
}

#[test]
fn play_starts_dd2_exe_and_the_watcher_tells_the_window_once_it_runs() {
    let bench = Bench::new();
    let (state, events) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    assert!(state.scan_blocking());
    state.watch_game();

    let plan = play::plan_launch_impl(&state);
    assert_eq!(
        plan.launch,
        LaunchRoute::Exe,
        "found by hand or env: DD2.exe"
    );
    assert!(plan.refused.is_none(), "{plan:?}");
    assert!(plan.warnings.is_empty(), "nothing installed: {plan:?}");

    // A pak installed and no REFramework: a warning, never a blocker.
    deploy::install_impl(&state, &option_named(&state, "Lantern Glow Recolor")).unwrap();
    let plan = play::plan_launch_impl(&state);
    assert_eq!(
        plan.warnings,
        vec![LaunchWarning::ReframeworkMissing { paks: 1 }]
    );
    assert!(plan.refused.is_none());

    events.clear();
    let report = play::launch_game_impl(&state).unwrap();
    assert_eq!(report.launch, LaunchRoute::Exe);
    assert!(report.launched_ms > 0);
    assert_eq!(
        bench.launcher.seen(),
        vec![Launched::Game(Route::Exe {
            exe: bench.game.join("DD2.exe"),
            cwd: bench.game.clone(),
        })],
        "DD2.exe with the game folder as its working directory"
    );
    // The bench's game is up now; the watcher sees it and says so once.
    wait_for(&events, GAME_CHANGED, 1);
    several_samples();
    assert_eq!(events.named(GAME_CHANGED).len(), 1, "one start, one event");
    let home = overview::get_overview_impl(&state).unwrap();
    let game = home.game.unwrap();
    assert!(game.running);
    assert!(game.started_ms.is_some_and(|ms| ms >= report.launched_ms));
    assert!(
        home.checks.iter().any(|c| c.id == "gameRunning"),
        "the game folder was read again with the game running"
    );

    // Running: Play is refused in one sentence, and nothing starts twice.
    let refused = play::plan_launch_impl(&state).refused.expect("refused");
    assert!(refused.contains("already running"), "{refused}");
    assert!(matches!(
        play::launch_game_impl(&state),
        Err(RiftError::Refused { reason }) if reason == refused
    ));
    assert_eq!(bench.launcher.seen().len(), 1);

    // The game stops: one more event, and Play is free again.
    bench.game_watch.set(false);
    wait_for(&events, GAME_CHANGED, 2);
    several_samples();
    assert_eq!(events.named(GAME_CHANGED).len(), 2, "one stop, one event");
    let game = overview::get_overview_impl(&state).unwrap().game.unwrap();
    assert!(!game.running);
    assert_eq!(game.started_ms, None);
    assert!(play::plan_launch_impl(&state).refused.is_none());
}

#[test]
fn play_waits_while_a_change_holds_the_game_folder_or_an_install_is_under_way() {
    let bench = Bench::new();
    let (state, _) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    assert!(state.scan_blocking());

    {
        let _turn = state.turn();
        let refused = play::plan_launch_impl(&state)
            .refused
            .expect("a change holds the game folder");
        assert!(refused.contains("right now"), "{refused}");
        // Play asks from another thread, the way the window would.
        let asked = {
            let state = std::sync::Arc::clone(&state);
            std::thread::spawn(move || play::launch_game_impl(&state))
                .join()
                .unwrap()
        };
        assert!(matches!(
            asked,
            Err(RiftError::Refused { reason }) if reason == refused
        ));
    }
    {
        let _installing = state.begin_install();
        assert!(play::plan_launch_impl(&state).refused.is_some());
        assert!(play::launch_game_impl(&state).is_err());
    }
    assert!(bench.launcher.seen().is_empty(), "nothing was started");
    assert!(play::plan_launch_impl(&state).refused.is_none());
    play::launch_game_impl(&state).unwrap();
    assert_eq!(bench.launcher.seen().len(), 1);
}

#[test]
fn a_game_found_through_steam_starts_through_steam() {
    let bench = Bench::new();
    let (state, _) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    let game = rift_deploy::validate_game_root(&bench.game).unwrap();
    state.switch_game(GameInstall {
        source: GameSource::Steam,
        ..game
    });
    let home = overview::get_overview_impl(&state).unwrap();
    assert_eq!(home.game.unwrap().launch, LaunchRoute::Steam);
    let plan = play::plan_launch_impl(&state);
    assert_eq!(plan.launch, LaunchRoute::Steam);
    let report = play::launch_game_impl(&state).unwrap();
    assert_eq!(report.launch, LaunchRoute::Steam);
    assert_eq!(
        bench.launcher.seen(),
        vec![Launched::Game(Route::Steam {
            url: "steam://rungameid/2054970".into()
        })]
    );
}

#[test]
fn without_a_game_play_is_refused_in_a_sentence() {
    let bench = Bench::new();
    let mut config = bench.config(DataDirs::under(&bench.data_home()));
    config.discovery = Discovery::Fixed { game_root: None };
    let (state, _) = bench.open(config);
    let plan = play::plan_launch_impl(&state);
    assert_eq!(
        plan.refused.as_deref(),
        Some(rift_deploy::launch::REFUSED_NO_GAME)
    );
    assert!(plan.warnings.is_empty());
    assert!(matches!(
        play::launch_game_impl(&state),
        Err(RiftError::Refused { .. })
    ));
    // A game folder that lost its DD2.exe is refused by rift-deploy's rule.
    state.switch_game(rift_deploy::validate_game_root(&bench.game).unwrap());
    std::fs::remove_file(bench.game.join("DD2.exe")).unwrap();
    let refused = play::plan_launch_impl(&state).refused.unwrap();
    assert!(refused.contains("DD2.exe"), "{refused}");
    assert!(bench.launcher.seen().is_empty());
}

#[test]
fn the_watcher_only_reads_the_game_folder_again_on_a_start_or_a_stop() {
    let bench = Bench::new();
    let (state, events) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    assert!(state.scan_blocking());
    let search = || {
        library::search_impl(
            &state,
            &SearchQuery {
                text: "ermine".into(),
                filters: SearchFilters::default(),
                limit: 0,
            },
        )
    };
    search();
    let built = state.snapshot_builds();
    state.watch_game();
    let asked = bench.game_watch.asked();
    several_samples();
    assert!(bench.game_watch.asked() > asked + 3, "the watcher samples");
    search();
    assert_eq!(
        state.snapshot_builds(),
        built,
        "samples that change nothing read nothing"
    );
    assert!(events.named(GAME_CHANGED).is_empty());

    bench.game_watch.set(true);
    wait_for(&events, GAME_CHANGED, 1);
    search();
    search();
    assert_eq!(
        state.snapshot_builds(),
        built + 1,
        "a start reads the game folder once"
    );
}

#[test]
fn a_change_to_the_game_folder_asks_the_process_list_afresh() {
    let bench = Bench::new();
    let (state, events) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    assert!(state.scan_blocking());
    // No watcher runs: the game starts unseen…
    bench.game_watch.set(true);
    // …and the plan of a change still knows, and says so once.
    let plan =
        deploy::plan_install_impl(&state, &option_named(&state, "Lantern Glow Recolor")).unwrap();
    assert!(
        plan.warnings.contains(&PlanWarning::GameRunning),
        "{plan:?}"
    );
    assert_eq!(events.named(GAME_CHANGED).len(), 1);
    assert!(
        overview::get_overview_impl(&state)
            .unwrap()
            .game
            .unwrap()
            .running
    );
}
