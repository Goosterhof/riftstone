//! The nxm handler switch (v0.3) on the memory registry: the stone claims the
//! nxm:// links only when the player turns it on, remembers the user's key as
//! the claim found it (in `nxm-handler.json`, before the registry changes),
//! puts exactly that back on off — the whole key only when the claim made it,
//! never what another program kept in it — and leaves alone a key another
//! program took back. A memory that cannot be read refuses the off and keeps
//! everything as it is. Off Windows the switch is refused. The program names
//! here are invented.

mod support;

use std::sync::Arc;

use riftstone_lib::dto::NxmHandlerState;
use riftstone_lib::nxm::{
    give_back, give_back_as_uninstalled, handler_view, set_handler, GIVE_BACK_ARG,
};
use riftstone_lib::persist::DataDirs;
use riftstone_lib::protocol::{
    command_for, MemoryRegistry, UnsupportedRegistry, UserKey, PROTOCOL_NAME,
};
use support::Bench;

/// Another program's handler, as Windows would hold it.
const OTHER: &str = r#""C:\Tools\Other Manager\OtherManager.exe" -d "%1""#;

fn memory_file(bench: &Bench) -> std::path::PathBuf {
    DataDirs::under(&bench.data_home()).nxm_handler_file()
}

#[test]
fn the_stone_claims_the_links_remembers_the_handler_and_gives_them_back() {
    let bench = Bench::new();
    bench.registry.set_user(Some(OTHER));
    let (state, _) = bench.open(bench.config(DataDirs::under(&bench.data_home())));

    let before = handler_view(&state).unwrap();
    assert_eq!(before.state, NxmHandlerState::Other);
    assert_eq!(
        before.other.as_deref(),
        Some("OtherManager.exe"),
        "the file name, never its folder"
    );
    assert_eq!(before.gives_back_to, None);
    assert_eq!(bench.registry.writes(), 0, "looking writes nothing");

    let on = set_handler(&state, true).unwrap();
    assert_eq!(on.state, NxmHandlerState::Ours);
    assert_eq!(on.gives_back_to.as_deref(), Some("OtherManager.exe"));
    assert_eq!(
        bench.registry.user().as_deref(),
        Some(command_for(&bench.exe).as_str())
    );
    let memory: serde_json::Value =
        serde_json::from_slice(&std::fs::read(memory_file(&bench)).unwrap()).unwrap();
    assert_eq!(
        memory["previous"], OTHER,
        "remembered in the app data folder"
    );
    assert!(!memory_file(&bench).starts_with(&bench.mods));

    // Already on: nothing is written again.
    let writes = bench.registry.writes();
    assert_eq!(
        set_handler(&state, true).unwrap().state,
        NxmHandlerState::Ours
    );
    assert_eq!(bench.registry.writes(), writes);

    let off = set_handler(&state, false).unwrap();
    assert_eq!(off.state, NxmHandlerState::Other);
    assert_eq!(
        bench.registry.user().as_deref(),
        Some(OTHER),
        "given back as it was"
    );
    assert!(!memory_file(&bench).exists(), "the memory goes with it");
}

#[test]
fn with_no_handler_before_off_removes_the_key_again() {
    let bench = Bench::new();
    let (state, _) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    let none = handler_view(&state).unwrap();
    assert_eq!(
        (none.state, none.other, none.gives_back_to),
        (NxmHandlerState::None, None, None)
    );
    let on = set_handler(&state, true).unwrap();
    assert_eq!((on.state, on.gives_back_to), (NxmHandlerState::Ours, None));
    let off = set_handler(&state, false).unwrap();
    assert_eq!(off.state, NxmHandlerState::None);
    assert_eq!(bench.registry.user(), None);
    assert_eq!(
        bench.registry.user_key_now(),
        UserKey::default(),
        "the claim made the key, so the whole key goes"
    );
    // Off when already off: nothing to give back, nothing written.
    let writes = bench.registry.writes();
    assert_eq!(
        set_handler(&state, false).unwrap().state,
        NxmHandlerState::None
    );
    assert_eq!(bench.registry.writes(), writes);
}

#[test]
fn a_key_another_program_took_back_is_left_alone() {
    let bench = Bench::new();
    let (state, _) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    set_handler(&state, true).unwrap();
    // Another manager takes the links back at its start.
    bench.registry.set_user(Some(OTHER));
    let view = handler_view(&state).unwrap();
    assert_eq!(view.state, NxmHandlerState::Other);
    let writes = bench.registry.writes();
    let off = set_handler(&state, false).unwrap();
    assert_eq!(off.state, NxmHandlerState::Other);
    assert_eq!(
        bench.registry.writes(),
        writes,
        "not the stone's key any more"
    );
    assert_eq!(bench.registry.user().as_deref(), Some(OTHER));
    assert!(!memory_file(&bench).exists());
}

#[test]
fn the_machines_handler_answers_again_after_off() {
    let mut bench = Bench::new();
    bench.registry = Arc::new(MemoryRegistry::holding(None, Some(OTHER)));
    let (state, _) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    assert_eq!(handler_view(&state).unwrap().state, NxmHandlerState::Other);
    let on = set_handler(&state, true).unwrap();
    assert_eq!(on.state, NxmHandlerState::Ours);
    assert_eq!(
        on.gives_back_to.as_deref(),
        Some("OtherManager.exe"),
        "the machine's handler answers once the user's key goes"
    );
    let off = set_handler(&state, false).unwrap();
    assert_eq!(off.state, NxmHandlerState::Other);
    assert_eq!(
        bench.registry.user(),
        None,
        "the user's key is removed, not copied"
    );
}

#[test]
fn a_stone_installed_elsewhere_before_is_not_given_the_links_back() {
    // The player moved the stone: the old install's command points at a
    // riftstone.exe that is gone. Off must not hand the links to it.
    let bench = Bench::new();
    let old = r#""D:\Old Place\The Riftstone\riftstone.exe" "%1""#;
    bench.registry.set_user(Some(old));
    let (state, _) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    let before = handler_view(&state).unwrap();
    assert_eq!(before.state, NxmHandlerState::Other);
    assert_eq!(before.other.as_deref(), Some("riftstone.exe"));
    let on = set_handler(&state, true).unwrap();
    assert_eq!((on.state, on.gives_back_to), (NxmHandlerState::Ours, None));
    let off = set_handler(&state, false).unwrap();
    assert_eq!(off.state, NxmHandlerState::None);
    assert_eq!(bench.registry.user(), None, "the dead path is not restored");
    let key = bench.registry.user_key_now();
    assert!(
        key.exists && key.command_depth == 3 && key.command.is_none(),
        "the key was there before the claim: only the command goes: {key:?}"
    );
}

/// The user's key as another program left it: no command of its own (the
/// machine's handler answers), but its icon kept in it.
fn key_without_a_command() -> UserKey {
    UserKey {
        exists: true,
        default: Some(PROTOCOL_NAME.into()),
        url_protocol: Some(String::new()),
        command_depth: 0,
        command: None,
    }
}

#[test]
fn off_puts_back_a_key_another_program_kept_its_icon_in() {
    let mut bench = Bench::new();
    bench.registry = Arc::new(MemoryRegistry::holding(None, Some(OTHER)));
    bench.registry.set_user_key(key_without_a_command());
    bench
        .registry
        .add_extra("DefaultIcon", r"C:\Tools\Other Manager\nxm.ico");
    let (state, _) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    assert_eq!(
        handler_view(&state).unwrap().state,
        NxmHandlerState::Other,
        "the machine's handler answers"
    );

    let on = set_handler(&state, true).unwrap();
    assert_eq!(on.state, NxmHandlerState::Ours);
    assert_eq!(on.gives_back_to.as_deref(), Some("OtherManager.exe"));
    let off = set_handler(&state, false).unwrap();
    assert_eq!(
        off.state,
        NxmHandlerState::Other,
        "the machine's handler answers again"
    );
    assert_eq!(
        bench.registry.user_key_now(),
        key_without_a_command(),
        "the key as it was: the keys the claim made are gone"
    );
    assert_eq!(
        bench
            .registry
            .extras()
            .get("DefaultIcon")
            .map(String::as_str),
        Some(r"C:\Tools\Other Manager\nxm.ico"),
        "what the claim never wrote stays"
    );
}

#[test]
fn off_puts_back_a_blank_command_and_leaves_its_delegate_execute() {
    let bench = Bench::new();
    let delegated = UserKey {
        command_depth: 3,
        command: Some(String::new()),
        ..key_without_a_command()
    };
    bench.registry.set_user_key(delegated.clone());
    bench.registry.add_extra(
        r"shell\open\command\DelegateExecute",
        "{00000000-0000-0000-0000-000000002207}",
    );
    let (state, _) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    assert_eq!(
        handler_view(&state).unwrap().state,
        NxmHandlerState::None,
        "a blank command names no program"
    );
    assert_eq!(
        set_handler(&state, true).unwrap().state,
        NxmHandlerState::Ours
    );
    set_handler(&state, false).unwrap();
    assert_eq!(
        bench.registry.user_key_now(),
        delegated,
        "the blank command is back, its key kept"
    );
    assert_eq!(
        bench.registry.extras().len(),
        1,
        "the DelegateExecute stays"
    );
}

#[test]
fn a_memory_that_cannot_be_parsed_refuses_off_and_keeps_everything() {
    let bench = Bench::new();
    bench.registry.set_user(Some(OTHER));
    let (state, _) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    set_handler(&state, true).unwrap();
    let ours = command_for(&bench.exe);
    let file = memory_file(&bench);
    std::fs::write(&file, b"{ \"schema\": 2, \"previous\": ").unwrap();
    let torn = std::fs::read(&file).unwrap();
    let aside = file.with_file_name("nxm-handler.json.damaged");

    // Looking reads the memory and leaves it exactly where it is.
    let view = handler_view(&state).unwrap();
    assert_eq!(view.state, NxmHandlerState::Ours);
    assert_eq!(
        view.gives_back_to, None,
        "the stone cannot tell what off gives back to"
    );
    assert_eq!(std::fs::read(&file).unwrap(), torn);
    assert!(!aside.exists(), "never set aside");

    let writes = bench.registry.writes();
    let refused = set_handler(&state, false).unwrap_err();
    let wire = serde_json::to_value(&refused).unwrap();
    assert_eq!(wire["kind"], "refused", "{wire}");
    let sentence = wire["detail"]["reason"].as_str().unwrap_or_default();
    assert!(
        sentence.contains("nxm-handler.json") && sentence.ends_with('.'),
        "the refusal names the file: {sentence}"
    );
    assert_eq!(bench.registry.writes(), writes, "nothing was changed");
    assert_eq!(bench.registry.user().as_deref(), Some(ours.as_str()));
    assert_eq!(
        std::fs::read(&file).unwrap(),
        torn,
        "the memory is never forgotten unread"
    );
    assert!(!aside.exists());

    // The way out the sentence names: the file goes, then off leaves the
    // links to Windows.
    std::fs::remove_file(&file).unwrap();
    let off = set_handler(&state, false).unwrap();
    assert_eq!(off.state, NxmHandlerState::None);
}

#[test]
fn a_memory_that_cannot_be_read_refuses_off_and_keeps_everything() {
    let bench = Bench::new();
    bench.registry.set_user(Some(OTHER));
    let (state, _) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    set_handler(&state, true).unwrap();
    let file = memory_file(&bench);
    // A folder where the file should be: there, and not readable as a file.
    std::fs::remove_file(&file).unwrap();
    std::fs::create_dir_all(&file).unwrap();
    let writes = bench.registry.writes();
    assert!(matches!(
        set_handler(&state, false),
        Err(rift_core::RiftError::Refused { .. })
    ));
    assert_eq!(bench.registry.writes(), writes);
    assert_eq!(
        bench.registry.user().as_deref(),
        Some(command_for(&bench.exe).as_str())
    );
    assert!(file.is_dir(), "left exactly as it was");
}

#[test]
fn a_schema_1_memory_still_gives_the_links_back() {
    // An earlier v0.3 build remembered only the command it replaced.
    let bench = Bench::new();
    bench.registry.set_user(Some(&command_for(&bench.exe)));
    let (state, _) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    let file = memory_file(&bench);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(
        &file,
        serde_json::to_vec(&serde_json::json!({ "schema": 1, "previous": OTHER })).unwrap(),
    )
    .unwrap();
    assert_eq!(
        handler_view(&state).unwrap().gives_back_to.as_deref(),
        Some("OtherManager.exe")
    );
    let off = set_handler(&state, false).unwrap();
    assert_eq!(off.state, NxmHandlerState::Other);
    assert_eq!(bench.registry.user().as_deref(), Some(OTHER));
    assert!(!file.exists(), "forgotten once given back");
}

#[test]
fn off_windows_the_switch_is_refused() {
    let bench = Bench::new();
    let mut config = bench.config(DataDirs::under(&bench.data_home()));
    config.protocols = Arc::new(UnsupportedRegistry);
    let (state, _) = bench.open(config);
    let view = handler_view(&state).unwrap();
    assert_eq!(
        (view.state, view.other, view.gives_back_to),
        (NxmHandlerState::Unsupported, None, None)
    );
    for on in [true, false] {
        assert!(matches!(
            set_handler(&state, on),
            Err(rift_core::RiftError::Refused { .. })
        ));
    }
    assert!(!memory_file(&bench).exists());
}

#[test]
fn the_uninstaller_gives_the_links_back_as_the_switch_would() {
    let bench = Bench::new();
    bench.registry.set_user(Some(OTHER));
    let (state, _) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    set_handler(&state, true).unwrap();
    drop(state);

    // `riftstone.exe --give-back-nxm`: the registry, the exe and the memory,
    // no spine.
    let file = memory_file(&bench);
    assert!(give_back(&*bench.registry, &bench.exe, &file).unwrap());
    assert_eq!(bench.registry.user().as_deref(), Some(OTHER));
    assert!(!file.exists());
    let writes = bench.registry.writes();
    assert!(
        !give_back(&*bench.registry, &bench.exe, &file).unwrap(),
        "nothing of the stone's is left: nothing to do"
    );
    assert_eq!(bench.registry.writes(), writes);
}

#[test]
fn an_uninstall_never_leaves_the_links_to_the_exe_it_removes() {
    let bench = Bench::new();
    bench.registry.set_user(Some(OTHER));
    let (state, _) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    set_handler(&state, true).unwrap();
    drop(state);
    // The memory cannot be parsed: the switch would refuse; the uninstaller
    // cannot ask, and the exe is about to go.
    let file = memory_file(&bench);
    std::fs::write(&file, b"{ torn").unwrap();
    assert!(give_back_as_uninstalled(
        &*bench.registry,
        &bench.exe,
        &file
    ));
    assert_eq!(
        bench.registry.user_key_now(),
        UserKey::default(),
        "the key that starts the removed exe goes whole"
    );
    assert_eq!(
        std::fs::read(&file).unwrap(),
        b"{ torn",
        "the memory is left as it is"
    );
    // Not the stone's key: nothing to do, nothing written.
    bench.registry.set_user(Some(OTHER));
    let writes = bench.registry.writes();
    assert!(give_back_as_uninstalled(
        &*bench.registry,
        &bench.exe,
        &file
    ));
    assert_eq!(bench.registry.writes(), writes);
    assert_eq!(bench.registry.user().as_deref(), Some(OTHER));
}

#[test]
fn the_uninstall_hook_asks_the_stone_to_give_the_links_back() {
    let spine = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let conf: serde_json::Value =
        serde_json::from_slice(&std::fs::read(spine.join("tauri.conf.json")).unwrap()).unwrap();
    let hooks = conf["bundle"]["windows"]["nsis"]["installerHooks"]
        .as_str()
        .expect("tauri.conf.json names the installer hooks");
    let hooks = std::fs::read_to_string(spine.join(hooks)).expect("the hooks file is there");
    let body = &hooks[hooks
        .find("!macro NSIS_HOOK_PREUNINSTALL")
        .expect("a pre-uninstall hook")..];
    let body = &body[..body.find("!macroend").expect("the hook ends")];
    assert!(
        body.contains(&format!(
            r#"ExecWait '"$INSTDIR\${{MAINBINARYNAME}}.exe" {GIVE_BACK_ARG}'"#
        )),
        "the hook runs the installed exe with the spine's own argument: {body}"
    );
    assert!(
        body.contains("${If} $UpdateMode <> 1"),
        "an updater's /UPDATE keeps the links: {body}"
    );
}
