//! Bench: READ-ONLY against a real Dragon's Dogma 2 folder and a real Fluffy
//! Mod Manager folder.
//!
//! ```sh
//! RIFT_BENCH_GAME="/mnt/c/Program Files (x86)/Steam/steamapps/common/Dragons Dogma 2" \
//! RIFT_BENCH_FLUFFY=/mnt/c/<fluffy> \
//! cargo test -p rift-deploy --test bench_real -- --ignored --nocapture
//! ```
//!
//! Discovery, REFramework's status, Fluffy's ledger, REFramework's pak load
//! report and the game state, against a journal in a tempdir. No install, no
//! uninstall, no switch — ever. It prints counts, never mod names.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rift_deploy::{
    discover_fluffy_in, discover_game_from, preflight, read_game_state, read_ledger,
    read_pak_load_report, read_status, validate_game_root, DeployContext, Journal, PakLoadState,
    PakOwner,
};

#[test]
#[ignore = "bench: reads the real game folder (RIFT_BENCH_GAME) and Fluffy (RIFT_BENCH_FLUFFY), read-only"]
fn bench_real_game_state_read_only() {
    let Some(game_root) = std::env::var_os("RIFT_BENCH_GAME").map(PathBuf::from) else {
        eprintln!("set RIFT_BENCH_GAME to a Dragon's Dogma 2 folder to run this bench");
        return;
    };
    let started = Instant::now();

    // <library>/steamapps/common/<game>: three folders up is the library —
    // the Steam root itself on a default install.
    let steam_root = game_root
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent);
    let discovered = discover_game_from(steam_root);
    println!(
        "discover_game_from: found = {}, build id = {:?}, same root = {}",
        discovered.is_some(),
        discovered.as_ref().and_then(|g| g.build_id.clone()),
        discovered.as_ref().is_some_and(|g| g.root == game_root),
    );
    let game = discovered
        .unwrap_or_else(|| validate_game_root(&game_root).expect("RIFT_BENCH_GAME holds DD2.exe"));

    let status = read_status(&game.root);
    println!(
        "RefStatus: installed = {}, version = {:?}, build date = {:?}, loose loader = {:?}, pak directory = {:?}, faulty files = {}",
        status.installed,
        status.version,
        status.build_date,
        status.loose_file_loader,
        status.pak_directory,
        status.faulty_files.len()
    );

    let fluffy = std::env::var_os("RIFT_BENCH_FLUFFY")
        .map(PathBuf::from)
        .and_then(|root| discover_fluffy_in(&[root]));
    println!(
        "Fluffy: found = {}, installed.ini = {}",
        fluffy.is_some(),
        fluffy.as_ref().is_some_and(|f| f.installed_ini.is_some())
    );
    let ledger = fluffy
        .and_then(|f| f.installed_ini)
        .map(|ini| read_ledger(&ini).expect("installed.ini reads"));
    if let Some(ledger) = &ledger {
        let files: usize = ledger.mods.iter().map(|m| m.files.len()).sum();
        let without_files = ledger.mods.iter().filter(|m| m.files.is_empty()).count();
        println!(
            "Fluffy ledger: {} mods ({} with no files), {} files, game path recorded = {}",
            ledger.mods.len(),
            without_files,
            files,
            ledger.game_install_path.is_some()
        );
    }

    let report = read_pak_load_report(&game.root);
    match &report {
        Some(report) => println!(
            "pak load report: log started = {:?}, cached = {}, injected = {}, slots = {:?}, error = {}",
            report.log_started,
            report.cached.len(),
            report.injected.len(),
            report.injected.iter().map(|i| i.slot).collect::<Vec<_>>(),
            report.error.is_some()
        ),
        None => println!("pak load report: no REFramework log"),
    }

    let state_dir = tempfile::tempdir().unwrap();
    let journal = Journal::load(state_dir.path(), &game.root).unwrap();
    let ctx = DeployContext {
        game: &game,
        state_dir: state_dir.path(),
        ref_status: &status,
        fluffy: ledger.as_ref(),
        game_running: false,
    };
    let state = read_game_state(&ctx, &journal);
    let owned_by =
        |want: fn(&PakOwner) -> bool| state.paks.iter().filter(|p| want(&p.owner)).count();
    let loaded_as =
        |want: fn(&PakLoadState) -> bool| state.paks.iter().filter(|p| want(&p.load)).count();
    println!(
        "GameState: paks = {} (riftstone {}, fluffy {}, unowned {}), loaded {}, pending {}, not loaded {}, unknown {}",
        state.paks.len(),
        owned_by(|o| matches!(o, PakOwner::Riftstone { .. })),
        owned_by(|o| matches!(o, PakOwner::Fluffy { .. })),
        owned_by(|o| matches!(o, PakOwner::Unowned)),
        loaded_as(|l| matches!(l, PakLoadState::Loaded { .. })),
        loaded_as(|l| matches!(l, PakLoadState::Pending)),
        loaded_as(|l| matches!(l, PakLoadState::NotLoaded)),
        loaded_as(|l| matches!(l, PakLoadState::Unknown)),
    );
    println!(
        "GameState: fluffy unadopted = {} ({} files present of {}), strays = {}, missing = {}, crash reporter renamed = {}, pak count = {}, outdated installed = {}",
        state.fluffy_unadopted.len(),
        state.fluffy_unadopted.iter().map(|m| m.present_files).sum::<usize>(),
        state.fluffy_unadopted.iter().map(|m| m.file_count).sum::<usize>(),
        state.strays.len(),
        state.missing.len(),
        state.crash_reporter_renamed,
        state.pak_count,
        state.outdated_files,
    );
    for check in preflight(&state) {
        println!("  check {:<18} {:?}", check.id, check.level);
    }
    assert!(
        fs::read_dir(state_dir.path()).unwrap().next().is_none(),
        "the bench wrote nothing, not even a journal"
    );
    println!("elapsed {} ms", started.elapsed().as_millis());
}
