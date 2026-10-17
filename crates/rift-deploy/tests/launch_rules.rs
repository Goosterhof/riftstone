//! Play (WINDOW-CONTRACT § v0.3 Play): the route `plan_launch` picks, each
//! warning (never a blocker), each refusal (one sentence), and `start_exe`
//! starting a program in the game folder.

mod common;

use std::fs;

use common::{Bench, TestMod};
use rift_core::RiftError;
use rift_deploy::{
    launch_route, plan_launch, read_game_state, start_exe, GameInstall, GameSource, Intent,
    IntentAction, Journal, LaunchPlan, LaunchRoute, LaunchWarning, RefStatus,
};

/// Play's plan for the bench as it stands, with the game found by `source`.
fn play(
    bench: &Bench,
    journal: &Journal,
    source: GameSource,
    running: bool,
    busy: bool,
) -> LaunchPlan {
    let game = GameInstall {
        source,
        ..bench.game.clone()
    };
    let state = read_game_state(&bench.ctx(), journal);
    plan_launch(&game, &bench.reframework, journal, &state, running, busy)
}

/// A bench with one pak install and one install of two loose files.
fn modded() -> (Bench, Journal) {
    let bench = Bench::new();
    let mut journal = bench.journal();
    TestMod::new("Lantern Glow Recolor")
        .pak("glow.pak", b"KPKA glow")
        .install(&bench, &mut journal);
    TestMod::new("Ermine Cape")
        .loose("natives/stm/cape.mesh", b"cape")
        .loose("natives/stm/cape_f.mesh", b"cape f")
        .install(&bench, &mut journal);
    (bench, journal)
}

#[test]
fn play_goes_through_steam_when_steam_found_the_game() {
    let bench = Bench::new();
    let journal = bench.journal();
    let plan = play(&bench, &journal, GameSource::Steam, false, false);
    assert_eq!(
        plan.route,
        LaunchRoute::Steam {
            url: "steam://rungameid/2054970".into()
        }
    );
    assert_eq!(plan.route.kind(), "steam");
    let steam = GameInstall {
        source: GameSource::Steam,
        ..bench.game.clone()
    };
    assert_eq!(launch_route(&steam), plan.route);
    assert_eq!(plan.refused, None);
    assert!(plan.warnings.is_empty(), "{:?}", plan.warnings);
}

#[test]
fn play_starts_dd2_exe_as_spelled_on_disk_in_the_game_folder_otherwise() {
    let bench = Bench::new();
    let journal = bench.journal();
    for source in [GameSource::Manual, GameSource::Env] {
        let plan = play(&bench, &journal, source, false, false);
        assert_eq!(
            plan.route,
            LaunchRoute::Exe {
                exe: bench.root().join("DD2.exe"),
                cwd: bench.root().to_path_buf(),
            },
            "{source:?}"
        );
        assert_eq!(plan.route.kind(), "exe");
        assert_eq!(plan.refused, None);
        let game = GameInstall {
            source,
            ..bench.game.clone()
        };
        assert_eq!(launch_route(&game), plan.route, "one rule for the route");
    }
    // The executable's own spelling, whatever case it is in.
    fs::rename(bench.root().join("DD2.exe"), bench.root().join("dd2.EXE")).unwrap();
    let plan = play(&bench, &journal, GameSource::Manual, false, false);
    assert_eq!(
        plan.route,
        LaunchRoute::Exe {
            exe: bench.root().join("dd2.EXE"),
            cwd: bench.root().to_path_buf(),
        }
    );
    assert_eq!(plan.refused, None);
}

#[test]
fn each_warning_speaks_for_what_is_installed_and_never_blocks() {
    let (mut bench, journal) = modded();
    assert!(
        play(&bench, &journal, GameSource::Steam, false, false)
            .warnings
            .is_empty(),
        "REFramework installed with both switches on: nothing to warn about"
    );

    bench.reframework = RefStatus::default();
    let missing = play(&bench, &journal, GameSource::Steam, false, false);
    assert_eq!(
        missing.warnings,
        [
            LaunchWarning::ReframeworkMissing { paks: 1 },
            LaunchWarning::LooseLoaderOff { files: 2 },
        ]
    );
    assert_eq!(missing.refused, None, "warnings never block");

    bench.reframework = RefStatus {
        installed: true,
        pak_directory: Some(false),
        loose_file_loader: Some(true),
        ..RefStatus::default()
    };
    assert_eq!(
        play(&bench, &journal, GameSource::Steam, false, false).warnings,
        [LaunchWarning::PakFolderOff { paks: 1 }]
    );

    bench.reframework = RefStatus {
        installed: true,
        pak_directory: None,
        loose_file_loader: Some(false),
        ..RefStatus::default()
    };
    assert_eq!(
        play(&bench, &journal, GameSource::Steam, false, false).warnings,
        [LaunchWarning::LooseLoaderOff { files: 2 }],
        "a pak folder switch the config leaves out is on"
    );

    // Steam updated the game since the newest install.
    let (mut bench, journal) = modded();
    bench.game.build_id = Some("25300000".into());
    assert_eq!(
        play(&bench, &journal, GameSource::Steam, false, false).warnings,
        [LaunchWarning::BuildChanged {
            recorded: "25290080".into(),
            now: "25300000".into(),
        }]
    );
}

#[test]
fn nothing_installed_means_no_mod_warnings_even_without_reframework() {
    let mut bench = Bench::new();
    bench.reframework = RefStatus::default();
    let journal = bench.journal();
    let plan = play(&bench, &journal, GameSource::Steam, false, false);
    assert!(plan.warnings.is_empty(), "{:?}", plan.warnings);
    assert_eq!(plan.refused, None);
}

#[test]
fn each_refusal_is_one_sentence_and_the_warnings_still_stand() {
    let (mut bench, mut journal) = modded();
    bench.reframework = RefStatus::default();
    // One sentence: it ends in a period and holds no other sentence end.
    let sentence = |plan: &LaunchPlan| {
        let text = plan.refused.clone().expect("refused");
        assert!(text.ends_with('.'), "{text}");
        assert!(
            !text[..text.len() - 1].contains(". "),
            "one sentence: {text}"
        );
        text
    };

    let running = play(&bench, &journal, GameSource::Steam, true, false);
    assert!(
        sentence(&running).contains("already running"),
        "{running:?}"
    );
    assert_eq!(running.warnings.len(), 2, "the warnings stand");

    let busy = play(&bench, &journal, GameSource::Steam, false, true);
    assert!(sentence(&busy).contains("changing the game folder right now"));
    let both = play(&bench, &journal, GameSource::Steam, true, true);
    assert_eq!(
        both.refused, running.refused,
        "a running game is the first thing to say"
    );

    journal.pending = Some(Intent {
        action: IntentAction::Install,
        seq: journal.next_seq,
        option_id: rift_core::OptionId("0000000000000c01#Cape".into()),
        touched: Vec::new(),
        started_at: "2026-10-16T12:00:00.000Z".into(),
        option_name: "Ermine Cape".into(),
        steps: Vec::new(),
        phase: 0,
        dirs: Vec::new(),
    });
    let cut_short = play(&bench, &journal, GameSource::Steam, false, false);
    assert!(sentence(&cut_short).contains("cut short"), "{cut_short:?}");
    let busy_and_pending = play(&bench, &journal, GameSource::Steam, false, true);
    assert_eq!(
        busy_and_pending.refused, busy.refused,
        "a change being made is not a change cut short"
    );

    // No game to start: the folder lost its executable.
    journal.pending = None;
    fs::remove_file(bench.root().join("DD2.exe")).unwrap();
    for source in [GameSource::Steam, GameSource::Manual] {
        let gone = play(&bench, &journal, source, false, false);
        let text = sentence(&gone);
        assert!(text.contains("DD2.exe"), "{text}");
        assert!(text.contains(&bench.root().display().to_string()), "{text}");
    }
    let no_game = rift_deploy::launch::REFUSED_NO_GAME;
    assert!(no_game.ends_with('.') && !no_game[..no_game.len() - 1].contains(". "));
}

#[test]
fn planning_play_writes_nothing() {
    let (bench, journal) = modded();
    let before = bench.snapshot();
    let journal_before = bench.journal();
    let _ = play(&bench, &journal, GameSource::Manual, false, false);
    let _ = play(&bench, &journal, GameSource::Steam, true, true);
    assert_eq!(bench.snapshot(), before);
    assert_eq!(bench.journal(), journal_before);
}

#[test]
fn start_exe_refuses_what_is_not_there() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("DD2.exe");
    let err = start_exe(&missing, dir.path()).unwrap_err();
    assert!(matches!(err, RiftError::Refused { .. }), "{err}");
    fs::write(&missing, b"").unwrap();
    let err = start_exe(&missing, &dir.path().join("gone")).unwrap_err();
    assert!(matches!(err, RiftError::Refused { .. }), "{err}");
    let err = start_exe(dir.path(), dir.path()).unwrap_err();
    assert!(
        matches!(err, RiftError::Refused { .. }),
        "a folder is not a program: {err}"
    );
}

/// The exe route runs the program with the game folder as its working
/// directory and never waits for it.
#[cfg(unix)]
#[test]
fn start_exe_runs_the_program_in_the_game_folder() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let game = dir.path().join("Dragons Dogma 2");
    fs::create_dir_all(&game).unwrap();
    let exe = game.join("DD2.exe");
    fs::write(&exe, "#!/bin/sh\necho started > started.txt\n").unwrap();
    fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).unwrap();

    start_exe(&exe, &game).unwrap();
    let marker = game.join("started.txt");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !written(&marker) && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(written(&marker), "the program ran in the game folder");
}

#[cfg(unix)]
fn written(marker: &std::path::Path) -> bool {
    fs::read_to_string(marker).is_ok_and(|text| text.trim() == "started")
}
