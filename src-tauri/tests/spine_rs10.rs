//! RS-10, measured on the registration itself: a build without the `pawn`
//! feature registers none of WINDOW-CONTRACT § v0.7's commands, and a build
//! with it registers every one (the Heretic's Assay, answer 5: an absent
//! inscription in the window is one piece of evidence, not the proof).
//! Each name is invoked through Tauri's own IPC dispatch (the mock runtime);
//! an unregistered command answers "not found", a registered one anything
//! else (its own refusal, a missing argument).

mod support;

use std::sync::Arc;

use serde_json::{json, Value};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{mock_builder, mock_context, noop_assets, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::Manager;

use riftstone_lib::persist::DataDirs;
use support::Bench;

const LOCAL_ORIGIN: &str = if cfg!(windows) {
    "http://tauri.localhost"
} else {
    "tauri://localhost"
};

/// WINDOW-CONTRACT § v0.7's commands, by their invoke names: the oracle,
/// written here from the contract, never read from the code under test.
const WORKSHOP: [&str; 15] = [
    "get_workshop",
    "summon_pawn",
    "pawn_write",
    "pawn_resize",
    "depart_pawn",
    "answer_pawn_play",
    "get_fence",
    "run_fence_probe",
    "plan_bring_through",
    "bring_through",
    "plan_put_back",
    "put_back",
    "open_workshop_folder",
    "set_pawn_voice",
    "set_pawn_inputs",
];

fn not_found(answer: &Result<Value, Value>, cmd: &str) -> bool {
    match answer {
        Err(Value::String(text)) => text == &format!("Command {cmd} not found"),
        _ => false,
    }
}

#[test]
fn the_workshops_commands_are_registered_only_with_the_pawn() {
    let bench = Bench::new();
    let dirs = DataDirs::under(&bench.data_home());
    let (state, _events) = bench.open(bench.config(dirs.clone()));
    let app = riftstone_lib::with_commands(mock_builder())
        .build(mock_context(noop_assets()))
        .expect("the mock app builds");
    app.manage(Arc::clone(&state));
    #[cfg(feature = "pawn")]
    app.manage(riftstone_lib::pawn::desk::Desk::new(&dirs.data));
    let webview = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let invoke = |cmd: &str| {
        tauri::test::get_ipc_response(
            &webview,
            InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: LOCAL_ORIGIN.parse().unwrap(),
                body: InvokeBody::Json(json!({})),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .map(|body| body.deserialize::<Value>().unwrap())
    };
    // The control: a command no build has is "not found", so the probe can
    // tell registered from unregistered at all.
    assert!(not_found(&invoke("summon_a_dragon"), "summon_a_dragon"));
    // And a public command is registered in every build.
    assert!(!not_found(&invoke("get_settings"), "get_settings"));
    for cmd in WORKSHOP {
        let answer = invoke(cmd);
        if cfg!(feature = "pawn") {
            assert!(
                !not_found(&answer, cmd),
                "{cmd} is registered with the pawn: {answer:?}"
            );
        } else {
            assert!(
                not_found(&answer, cmd),
                "{cmd} must not exist without the pawn: {answer:?}"
            );
        }
    }
}
