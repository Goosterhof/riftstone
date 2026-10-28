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
    let errands = StoneErrands::new(Arc::clone(&state));
    let runs = RunsFile::new(dirs.data.join(RunsFile::NAME));
    let broker = Broker::new(RUN, &errands, &runs, mailbox::now);
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
    let errands = StoneErrands::new(Arc::clone(&state));
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
    let errands = StoneErrands::new(Arc::clone(&state));
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
    let errands = StoneErrands::new(Arc::clone(&state));
    let runs = RunsFile::new(dirs.data.join(RunsFile::NAME));
    let broker = Broker::new(RUN, &errands, &runs, mailbox::now);
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
    let errands = StoneErrands::new(Arc::clone(&state));
    // A folder where the file should be: it cannot be read, and is never
    // written over.
    let path = dirs.data.join(RunsFile::NAME);
    fs::create_dir_all(&path).unwrap();
    let runs = RunsFile::new(path.clone());
    let broker = Broker::new(RUN, &errands, &runs, mailbox::now);
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
    let errands = StoneErrands::new(Arc::clone(&state));
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
