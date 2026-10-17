//! The nxm handler switch (v0.3) on the memory registry: the stone claims the
//! nxm:// links only when the player turns it on, remembers the handler it
//! replaced (in `nxm-handler.json`, before the registry changes), gives the
//! links back on off, and leaves alone a key another program took back. Off
//! Windows the switch is refused. The program names here are invented.

mod support;

use std::sync::Arc;

use riftstone_lib::dto::NxmHandlerState;
use riftstone_lib::nxm::{handler_view, set_handler};
use riftstone_lib::persist::DataDirs;
use riftstone_lib::protocol::{command_for, MemoryRegistry, UnsupportedRegistry};
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
