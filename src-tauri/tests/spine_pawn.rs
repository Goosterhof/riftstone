//! Tamsin's tools against the real spine (v0.7, BUILD-SPEC § v0.7): the
//! broker over [`StoneErrands`] and `pawn-runs.json`, on a fake game and an
//! invented library. Only with the `pawn` feature.
#![cfg(feature = "pawn")]

mod support;

use std::fs;
use std::sync::Arc;

use rift_deploy::Actor;
use riftstone_lib::commands::deploy;
use riftstone_lib::pawn::errands::{RunsFile, StoneErrands};
use riftstone_lib::pawn::mailbox::{
    self, plan_token, Broker, Errands, Outcome, Removed, LEDGER_DOWN, STALE_PLAN,
};
use riftstone_lib::persist::DataDirs;
use riftstone_lib::state::AppState;
use serde_json::Value;
use support::{option_named, Bench};

const RUN: &str = "0b5e2c6e-4a8d-4d3e-9f4b-2a1c7d9e8f00";
const ID: &str = "11111111-2222-4333-8444-555555555555";

fn pawn() -> Actor {
    Actor::Pawn { run_id: RUN.into() }
}

fn opened() -> (Bench, DataDirs, Arc<AppState>) {
    let bench = Bench::new();
    let dirs = DataDirs::under(&bench.data_home());
    let (state, _events) = bench.open(bench.config(dirs.clone()));
    assert!(state.scan_blocking());
    (bench, dirs, state)
}

fn request(id: &str, tool: &str, args: Value) -> (String, Vec<u8>) {
    (
        format!("{id}.json"),
        serde_json::to_vec(&serde_json::json!({ "id": id, "tool": tool, "args": args })).unwrap(),
    )
}

/// Is the option installed, as the spine holds the journal now?
fn installed(state: &AppState, option_id: &str) -> bool {
    state
        .journal_installs()
        .contains_key(&rift_core::OptionId(option_id.to_string()))
}

fn actor_on_disk(bench: &Bench, dirs: &DataDirs, option_id: &str) -> Option<Value> {
    bench.journal_on_disk(dirs)["installs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|install| install["optionId"] == option_id)
        .map(|install| install["actor"].clone())
}

#[test]
fn tamsins_install_goes_through_the_journal_as_hers_and_lands_in_her_ledger() {
    let (bench, dirs, state) = opened();
    let errands = Arc::new(StoneErrands::new(Arc::clone(&state)));
    let runs = Arc::new(RunsFile::new(dirs.data.join(RunsFile::NAME)));
    let broker = Broker::new(RUN, errands.clone(), runs.clone(), mailbox::now);
    let lantern = option_named(&state, "Lantern Glow Recolor");

    let (name, bytes) = request(
        ID,
        "plan_install",
        serde_json::json!({ "optionId": lantern }),
    );
    let plan = broker.handle(&name, &bytes).unwrap();
    assert_eq!(plan.answer.outcome, Outcome::Done, "{:?}", plan.answer);
    let token = plan.answer.data["planToken"].as_str().unwrap().to_string();

    let install_id = "22222222-2222-4333-8444-555555555555";
    let (name, bytes) = request(
        install_id,
        "install",
        serde_json::json!({ "optionId": lantern, "planToken": token }),
    );
    let done = broker.handle(&name, &bytes).unwrap();
    assert_eq!(done.answer.outcome, Outcome::Done, "{:?}", done.answer);
    assert!(done
        .answer
        .text
        .starts_with("installed Lantern Glow Recolor"));
    assert_eq!(
        actor_on_disk(&bench, &dirs, &lantern),
        Some(serde_json::json!({ "kind": "pawn", "runId": RUN })),
        "the journal on disk names her run"
    );
    let kept = runs.runs();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].ops[0].tool, "install");
    assert_eq!(kept[0].ops[0].outcome, Outcome::Done);
}

#[test]
fn a_players_install_between_her_plan_and_her_install_makes_the_plan_stale() {
    let (_bench, _dirs, state) = opened();
    let errands = Arc::new(StoneErrands::new(Arc::clone(&state)));
    let ermine = option_named(&state, "Ermine Cape");
    let planned = errands.plan_install(&ermine).unwrap();
    deploy::install_impl(&state, &option_named(&state, "Lantern Glow Recolor")).unwrap();
    assert_eq!(
        errands.install(&ermine, &pawn(), &planned),
        Err(STALE_PLAN.into())
    );
    assert!(!installed(&state, &ermine), "nothing installed");
}

#[test]
fn her_plan_is_checked_inside_the_turn_that_commits() {
    // The Heretic's WB1, on the real lock: the library changes while the
    // install waits for its turn; the install still refuses.
    let (_bench, _dirs, state) = opened();
    let errands = Arc::new(StoneErrands::new(Arc::clone(&state)));
    let ermine = option_named(&state, "Ermine Cape");
    let planned = errands.plan_install(&ermine).unwrap();
    let turn = state.turn();
    let outcome = std::thread::scope(|scope| {
        let waiting = scope.spawn(|| errands.install(&ermine, &pawn(), &planned));
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(!waiting.is_finished(), "the install waits for the turn");
        assert!(
            state.scan_blocking(),
            "the library is scanned again meanwhile"
        );
        drop(turn);
        waiting.join().unwrap()
    });
    assert_eq!(outcome, Err(STALE_PLAN.into()));
    assert!(!installed(&state, &ermine));
}

#[test]
fn her_uninstall_records_whose_install_it_removed() {
    let (_bench, dirs, state) = opened();
    let errands = Arc::new(StoneErrands::new(Arc::clone(&state)));
    let runs = Arc::new(RunsFile::new(dirs.data.join(RunsFile::NAME)));
    let broker = Broker::new(RUN, errands.clone(), runs.clone(), mailbox::now);
    let lantern = option_named(&state, "Lantern Glow Recolor");
    deploy::install_impl(&state, &lantern).unwrap();
    let (name, bytes) = request(ID, "uninstall", serde_json::json!({ "installId": lantern }));
    let done = broker.handle(&name, &bytes).unwrap();
    assert_eq!(done.answer.outcome, Outcome::Done, "{:?}", done.answer);
    let op = &runs.runs()[0].ops[0];
    assert_eq!(op.removed.as_ref().map(|r| &r.actor), Some(&Actor::Player));
    assert!(matches!(op.removed, Some(Removed { seq, .. }) if seq >= 1));
    assert_eq!(done.answer.data["name"], "Lantern Glow Recolor");
}

#[test]
fn a_ledger_file_that_cannot_be_read_stops_her_before_anything_moves() {
    let (_bench, dirs, state) = opened();
    let errands = Arc::new(StoneErrands::new(Arc::clone(&state)));
    // A folder where the file should be: it cannot be read, and is never
    // written over.
    let path = dirs.data.join(RunsFile::NAME);
    fs::create_dir_all(&path).unwrap();
    let runs = Arc::new(RunsFile::new(path.clone()));
    let broker = Broker::new(RUN, errands.clone(), runs.clone(), mailbox::now);
    let lantern = option_named(&state, "Lantern Glow Recolor");
    let token = plan_token(&errands.plan_install(&lantern).unwrap(), RUN);
    let (name, bytes) = request(
        ID,
        "install",
        serde_json::json!({ "optionId": lantern, "planToken": token }),
    );
    let refused = broker.handle(&name, &bytes).unwrap();
    assert_eq!(refused.answer.text, LEDGER_DOWN);
    assert!(!installed(&state, &lantern), "nothing installed");
    assert!(path.is_dir(), "never written over");
}

#[test]
fn she_reads_the_tail_of_the_reframework_log_and_nothing_else() {
    let (bench, _dirs, state) = opened();
    let errands = Arc::new(StoneErrands::new(Arc::clone(&state)));
    let log = bench.game.join("re2_framework_log.txt");
    fs::write(&log, "one\r\ntwo\r\nthree\r\n").unwrap();
    let answer = errands.read_reframework_log(2).unwrap();
    assert_eq!(
        answer,
        serde_json::json!({ "count": 2, "lines": ["two", "three"] })
    );
    assert_eq!(fs::read_to_string(&log).unwrap(), "one\r\ntwo\r\nthree\r\n");
    fs::remove_file(&log).unwrap();
    assert!(errands.read_reframework_log(5).is_err());
}

#[test]
fn no_scan_publishes_between_her_check_and_her_commit() {
    // The Heretic's WA1: a scan never takes the deploy turn, so the library
    // could be republished after the check passed and before the install
    // committed. A checked install now holds the library from its check to
    // its commit; the scan publishes after.
    let (_bench, _dirs, state) = opened();
    let lantern = option_named(&state, "Lantern Glow Recolor");
    let checked = std::sync::Barrier::new(2);
    let at_check = std::sync::atomic::AtomicU64::new(u64::MAX);
    let during = std::sync::atomic::AtomicU64::new(u64::MAX);
    let check = |_: &riftstone_lib::dto::InstallPlanView, _: &rift_deploy::Journal| {
        at_check.store(
            state.library_generation(),
            std::sync::atomic::Ordering::SeqCst,
        );
        checked.wait();
        // The scan runs now; give it every chance to publish.
        std::thread::sleep(std::time::Duration::from_millis(300));
        during.store(
            state.library_generation(),
            std::sync::atomic::Ordering::SeqCst,
        );
        Ok(())
    };
    std::thread::scope(|scope| {
        let install =
            scope.spawn(|| deploy::install_checked(&state, &lantern, &pawn(), Some(&check)));
        checked.wait();
        let scan = scope.spawn(|| state.scan_blocking());
        install.join().unwrap().unwrap();
        assert!(
            scan.join().unwrap(),
            "the scan still finishes, after the commit"
        );
    });
    let at_check = at_check.into_inner();
    assert_eq!(
        during.into_inner(),
        at_check,
        "the library stayed as checked until the install committed"
    );
    assert!(
        state.library_generation() > at_check,
        "and was published after"
    );
}

mod desk {
    use super::*;
    use rift_core::{PawnReason, RiftError};
    use riftstone_lib::events::EventSink;
    use riftstone_lib::pawn::commands::{get_workshop_impl, plan_put_back_impl};
    use riftstone_lib::pawn::desk::{workshop_dir, Desk};
    use riftstone_lib::pawn::persona::Voice;
    use sha2::{Digest, Sha256};

    fn reason(err: RiftError) -> PawnReason {
        match err {
            RiftError::Pawn { reason, .. } => reason,
            other => panic!("not a pawn refusal: {other:?}"),
        }
    }

    #[test]
    fn the_workshop_view_reads_the_folder_its_builds_and_the_pawn() {
        let (_bench, dirs, state) = opened();
        let desk = Desk::new(&dirs.data);
        let view = get_workshop_impl(&state, &desk).unwrap();
        assert!(!view.folder.made, "not made until the first summon");
        assert!(view.artifacts.is_empty());
        assert_eq!(view.pawn.state, "absent");
        assert_eq!(view.fence.chip, "unprobed");
        assert_eq!(view.voice, Voice::Sparing);

        // A build with its receipt, then one with none.
        let workshop = workshop_dir(state.library_dir()).unwrap();
        fs::create_dir_all(workshop.join("dist")).unwrap();
        let bytes = b"invented build";
        let sha: String = Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        fs::write(workshop.join("dist/Stature-0.2.0.zip"), bytes).unwrap();
        fs::write(
            workshop.join("dist/Stature-0.2.0.receipt.json"),
            serde_json::to_vec(&serde_json::json!({ "artifact": "Stature-0.2.0.zip",
                "sha256": sha, "builtAt": "2026-10-28T12:00:00Z" }))
            .unwrap(),
        )
        .unwrap();
        fs::write(workshop.join("dist/Bare.zip"), b"bare").unwrap();
        let view = get_workshop_impl(&state, &desk).unwrap();
        assert!(view.folder.made);
        let names: Vec<&str> = view.artifacts.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["Bare.zip", "Stature-0.2.0.zip"]);
        assert_eq!(view.artifacts[0].receipt_problem, Some("missing"));
        assert_eq!(view.artifacts[1].sha256.as_deref(), Some(sha.as_str()));
        assert!(!view.artifacts[1].in_library);

        // Brought through, it is in the library.
        let plan = riftstone_lib::pawn::bring::plan_bring_through(
            &workshop,
            state.library_dir(),
            "Stature-0.2.0.zip",
        )
        .unwrap();
        riftstone_lib::pawn::bring::bring_through(
            &workshop,
            state.library_dir(),
            "Stature-0.2.0.zip",
            &plan.sha256,
            "t1",
        )
        .unwrap();
        let view = get_workshop_impl(&state, &desk).unwrap();
        assert!(view.artifacts[1].in_library);
    }

    #[test]
    fn a_summon_the_fence_cannot_hold_is_refused_and_nothing_runs() {
        // A distro that does not exist: on Windows `wsl.exe -d` fails the
        // survey; on the Linux bench (which runs /bin/sh itself) the temp
        // mods folder is no Windows drive and the fence refuses its plan.
        // noFence either way. (The first native run of this test, with no
        // distro named, summoned Tamsin for real through wsl.exe.)
        let (_bench, dirs, state) = opened();
        fs::create_dir_all(&dirs.data).unwrap();
        fs::write(
            dirs.data.join("pawn.json"),
            br#"{ "distro": "no-such-distro-riftstone" }"#,
        )
        .unwrap();
        let desk = Desk::new(&dirs.data);
        let events: Arc<dyn EventSink> = Arc::new(riftstone_lib::events::RecordedEvents::default());
        let refused = desk.summon(&state, events, 80, 24).unwrap_err();
        assert_eq!(reason(refused), PawnReason::NoFence);
        assert_eq!(desk.pawn_state().state, "absent");
        assert_eq!(
            reason(desk.write("r", "x").unwrap_err()),
            PawnReason::NotHere
        );
        assert_eq!(
            reason(
                desk.depart(&riftstone_lib::events::RecordedEvents::default())
                    .unwrap_err()
            ),
            PawnReason::NotHere
        );
        assert_eq!(
            reason(desk.answer_play(&state, RUN, ID, true).unwrap_err()),
            PawnReason::StaleHeld
        );
    }

    #[test]
    fn put_back_never_removes_a_reinstall_made_after_its_plan() {
        // The Heretic's WD1: the uninstall re-checks, in its own turn, that
        // the install is still hers at the seq the press was planned on.
        let (_bench, _dirs, state) = opened();
        let errands = StoneErrands::new(Arc::clone(&state));
        let lantern = option_named(&state, "Lantern Glow Recolor");
        let planned = errands.plan_install(&lantern).unwrap();
        errands.install(&lantern, &pawn(), &planned).unwrap();
        let seq_then = state
            .world()
            .journal
            .ready()
            .unwrap()
            .installs
            .iter()
            .find(|i| i.option_id.0 == lantern)
            .unwrap()
            .seq;
        // The player takes it over: uninstalled and installed again.
        deploy::uninstall_impl(&state, &lantern).unwrap();
        deploy::install_impl(&state, &lantern).unwrap();
        let check = |journal: &rift_deploy::Journal| {
            if riftstone_lib::pawn::put_back::still_hers(RUN, journal, &lantern, seq_then) {
                Ok(())
            } else {
                Err(RiftError::pawn(PawnReason::NotThisRun, "changed"))
            }
        };
        let refused = deploy::uninstall_checked(&state, &lantern, Some(&check)).unwrap_err();
        assert_eq!(reason(refused), PawnReason::NotThisRun);
        assert!(installed(&state, &lantern), "the player's reinstall stays");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_linked_dist_lists_nothing_of_what_it_points_at() {
        // The Heretic's WD4: the listing keeps bring-through's boundary.
        let (_bench, dirs, state) = opened();
        let desk = Desk::new(&dirs.data);
        let workshop = workshop_dir(state.library_dir()).unwrap();
        fs::create_dir_all(&workshop).unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("Canary.zip"), b"outside canary").unwrap();
        std::os::unix::fs::symlink(outside.path(), workshop.join("dist")).unwrap();
        let view = get_workshop_impl(&state, &desk).unwrap();
        assert!(view.artifacts.is_empty(), "{:?}", view.artifacts);
    }

    /// Invented raw saves for one account, and an existing destination.
    fn saves(bench: &Bench, state: &AppState) -> std::path::PathBuf {
        let source = bench.dir.path().join("saves");
        let destination = bench.dir.path().join("Save Backups");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&destination).unwrap();
        fs::write(source.join("data00-1.bin"), b"invented system data").unwrap();
        state.fix_save_folders(vec![rift_muster::saves::SaveFolder {
            account_id: 17,
            path: source,
        }]);
        destination
    }

    fn backup_roots(state: &AppState) -> Result<usize, RiftError> {
        Ok(riftstone_lib::pawn::desk::protected_roots(state)?
            .iter()
            .filter(|root| root.name == "a save backup folder")
            .count())
    }

    #[test]
    fn a_backup_that_cannot_be_registered_is_not_made() {
        // The Heretic's WD3, first control: the inventory cannot be written.
        let (bench, dirs, state) = opened();
        let destination = saves(&bench, &state);
        fs::create_dir_all(dirs.data.join("save-backup-destinations.json")).unwrap();
        let refused = riftstone_lib::commands::save_backup::backup_saves_impl(
            &state,
            destination.to_str().unwrap(),
        );
        assert!(refused.is_err(), "no backup without its registration");
        assert_eq!(
            fs::read_dir(&destination).unwrap().count(),
            0,
            "nothing was published"
        );
    }

    #[test]
    fn a_damaged_inventory_refuses_instead_of_forgetting() {
        // The Heretic's WD3, second control: registered coverage is never
        // silently lost to a damaged file.
        let (bench, dirs, state) = opened();
        let destination = saves(&bench, &state);
        riftstone_lib::commands::save_backup::backup_saves_impl(
            &state,
            destination.to_str().unwrap(),
        )
        .unwrap();
        assert_eq!(
            backup_roots(&state).unwrap(),
            1,
            "registered before it was written"
        );
        let inventory = dirs.data.join("save-backup-destinations.json");
        fs::write(&inventory, b"{ not a list").unwrap();
        let refused = backup_roots(&state).unwrap_err();
        let detail = format!("{refused:?}");
        assert!(detail.contains("save-backup-destinations.json"), "{detail}");
        let private = dirs.data.to_string_lossy().into_owned();
        assert!(
            !detail.contains(&private),
            "no private path in the sentence: {detail}"
        );
        assert_eq!(reason(refused), PawnReason::NoFence);
        let desk = Desk::new(&dirs.data);
        assert!(desk
            .set_inputs(&state, vec!["C:\\Elsewhere".into()])
            .is_err());
        let second = bench.dir.path().join("Second Backups");
        fs::create_dir_all(&second).unwrap();
        let refused = riftstone_lib::commands::save_backup::backup_saves_impl(
            &state,
            second.to_str().unwrap(),
        )
        .unwrap_err();
        assert!(!format!("{refused:?}").contains(&private), "{refused:?}");
        assert_eq!(
            fs::read(&inventory).unwrap(),
            b"{ not a list",
            "left exactly as it was"
        );
        assert_eq!(fs::read_dir(&second).unwrap().count(), 0);
    }

    #[test]
    fn put_back_names_only_a_run_in_her_ledger() {
        let (_bench, dirs, state) = opened();
        let desk = Desk::new(&dirs.data);
        assert_eq!(
            reason(plan_put_back_impl(&state, &desk, RUN).unwrap_err()),
            PawnReason::NotThisRun
        );
        assert_eq!(
            reason(
                riftstone_lib::pawn::commands::put_back_impl(&state, RUN, &["aa#Glow".into()])
                    .unwrap_err()
            ),
            PawnReason::NotThisRun
        );
    }

    #[test]
    fn an_input_over_a_protected_root_is_refused_by_name() {
        let (_bench, dirs, state) = opened();
        let desk = Desk::new(&dirs.data);
        let refused = desk
            .set_inputs(
                &state,
                vec![state.library_dir().to_string_lossy().into_owned()],
            )
            .unwrap_err();
        assert_eq!(reason(refused), PawnReason::InputRefused);
        assert!(desk.settings().inputs.is_empty(), "nothing saved");
    }
}

/// A bench aid for the Windows host, by hand
/// (`RIFT_BENCH_NATIVE_SUMMON=1 cargo test -p riftstone --features pawn
/// --test spine_pawn bench_ -- --ignored --nocapture`): the desk summons
/// Tamsin for real through wsl.exe in a ConPTY, exactly as the app will, on
/// a bench whose mods folder is under Windows' temp. It prints her
/// terminal's first seconds, departs, and asks WSL whether any of the
/// fence's processes outlived the departure.
#[cfg(windows)]
#[test]
#[ignore = "a bench aid: summons the real fence through wsl.exe"]
fn bench_summon_natively_through_wsl() {
    use base64::Engine;
    use riftstone_lib::events::EventSink;
    use riftstone_lib::pawn::desk::{Desk, PAWN_OUTPUT, PAWN_STATE};
    assert!(std::env::var_os("RIFT_BENCH_NATIVE_SUMMON").is_some());
    let bench = Bench::new();
    let dirs = DataDirs::under(&bench.data_home());
    let (state, events) = bench.open(bench.config(dirs.clone()));
    assert!(state.scan_blocking());
    let desk = Desk::new(&dirs.data);
    let sink: Arc<dyn EventSink> = events.clone();
    let summoned = desk.summon(&state, sink, 100, 30).expect("summoned");
    println!("BENCH summoned: {summoned:?}");
    // ConPTY asks its host for the cursor position (`ESC[6n`) and holds
    // every later byte until it is answered; in the room, xterm.js answers
    // through pawn_write. The bench answers as xterm.js would.
    let asked = std::time::Instant::now();
    while std::time::Instant::now().duration_since(asked) < std::time::Duration::from_secs(5) {
        let queried = events.named(PAWN_OUTPUT).iter().any(|event| {
            let data = event["data"].as_str().unwrap_or_default();
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(data)
                .unwrap_or_default();
            bytes.windows(4).any(|w| w == b"\x1b[6n")
        });
        if queried {
            desk.write(
                summoned.run_id.as_deref().unwrap_or_default(),
                "\u{1b}[1;1R",
            )
            .expect("answered the cursor query");
            println!("BENCH answered ConPTY's cursor query");
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let wait: u64 = std::env::var("RIFT_BENCH_NATIVE_WAIT")
        .ok()
        .and_then(|w| w.parse().ok())
        .unwrap_or(10);
    std::thread::sleep(std::time::Duration::from_secs(wait));
    let here = desk.pawn_state();
    let mut shown = Vec::new();
    for event in events.named(PAWN_OUTPUT) {
        let data = event["data"].as_str().unwrap_or_default();
        shown.extend(
            base64::engine::general_purpose::STANDARD
                .decode(data)
                .unwrap_or_default(),
        );
    }
    let began = std::time::Instant::now();
    let departed = desk.depart(events.as_ref()).expect("departed");
    let took = began.elapsed();
    std::thread::sleep(std::time::Duration::from_millis(800));
    let left = std::process::Command::new("wsl.exe")
        .args([
            "-e",
            "/bin/sh",
            "-c",
            "pgrep -f riftstone/pawn/relay | wc -l",
        ])
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .unwrap_or_else(|err| format!("could not ask: {err}"));
    let text: String = String::from_utf8_lossy(&shown)
        .chars()
        .filter(|c| !c.is_control() || *c == '\n')
        .collect();
    println!("BENCH state after the wait: {} {:?}", here.state, here.host);
    println!(
        "BENCH output events: {}, bytes: {}",
        events.named(PAWN_OUTPUT).len(),
        shown.len()
    );
    println!(
        "BENCH words seen: welcome={} started={}",
        text.contains("Welcome"),
        text.contains("started")
    );
    println!(
        "BENCH raw head: {:?}",
        String::from_utf8_lossy(&shown[..shown.len().min(600)])
    );
    println!(
        "BENCH departed: {} in {:?}; state events: {}",
        departed.state,
        took,
        events.named(PAWN_STATE).len()
    );
    println!("BENCH fence processes left in WSL (minus the asking shell): {left}");
}

/// A bench aid, by hand, on either host: run the fence's real probe
/// (`RIFT_BENCH_PROBE_ROOT` = a folder on a Windows drive; on the Linux bench
/// `/mnt/c/…`, on Windows `C:\…`), with the mods folder and the app data
/// under it, as the stone's are. Prints each step and the chip.
#[test]
#[ignore = "a bench aid: runs the real probe through the real fence"]
fn bench_run_the_real_probe() {
    use riftstone_lib::pawn::desk::Desk;
    let root = std::path::PathBuf::from(
        std::env::var("RIFT_BENCH_PROBE_ROOT").expect("RIFT_BENCH_PROBE_ROOT"),
    );
    let _ = fs::remove_dir_all(&root);
    let bench = Bench::new();
    let dirs = DataDirs::under(&root.join("data"));
    let mut config = bench.config(dirs.clone());
    config.library_dir = root.join("mods");
    fs::create_dir_all(&config.library_dir).unwrap();
    let (state, _events) = bench.open(config);
    let desk = Desk::new(&dirs.data);
    let view = desk.run_probe(&state).expect("the probe ran");
    for step in &view.steps {
        println!(
            "BENCH step {:>2} {:?} {:?} — {}",
            step.step, step.expected, step.verdict, step.observed
        );
    }
    println!(
        "BENCH chip={} changed={:?} ground={:?}",
        view.chip,
        view.changed,
        view.ground
            .as_ref()
            .map(|g| (&g.distro, &g.claude_version, &g.kernel))
    );
    let record = desk.record().unwrap();
    println!(
        "BENCH control={} holds={} breached={}",
        record.control,
        record.holds(),
        record.breached()
    );
}
