//! `read_game_state` and `preflight` — what is in the game right now, and
//! the checks the home frame shows.

mod common;

use common::{adopted, Bench, TestMod};
use rift_core::OptionId;
use rift_deploy::{
    preflight, read_game_state, Below, CheckAction, CheckLevel, DeployKind, GameState,
    PakLoadState, PakOwner, RefStatus, RefSwitch,
};

const LOG_HEAD: &str = "[2026-10-14 21:34:14.970] [REFramework] [info] REFramework entry\r\n\
[2026-10-14 21:34:14.972] [REFramework] [info] Tag: v1.5.9.1\r\n\
[2026-10-14 21:34:14.972] [REFramework] [info] Commits past tag: 514\r\n";

fn injected(file: &str, slot: u32) -> String {
    format!(
        "[2026-10-14 21:34:29.301] [REFramework] [info] [IntegrityCheckBypass]: injecting C:\\Games\\DD2\\pak_mods\\{file} as native-form 're_chunk_000.pak.sub_000.pak.patch_{:03}.pak' (a6=1, slot={slot}, auto-assigned)\r\n",
        slot - 102
    )
}

struct World {
    bench: Bench,
    glow: TestMod,
    cape: TestMod,
    late: TestMod,
    older: OptionId,
}

/// A lived-in game folder: three installs (one loaded at the last launch,
/// one installed before it yet never loaded, one installed after it), an
/// adopted install holding a numbered pak from another tool, strays, missing
/// files, a nested pak, a `.PAK`.
fn world() -> World {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let glow = TestMod::new("Lantern Glow Recolor")
        .pak("glow.pak", b"KPKA glow")
        .loose("natives/stm/glow.mesh", b"glow");
    let cape = TestMod::new("Ermine Cape")
        .pak("cape.pak", b"KPKA cape")
        .loose("natives/stm/cape/top_wb_f.mesh.240423143", b"old cape")
        .loose("natives/stm/cape/vanishes.mesh", b"gone soon");
    let late = TestMod::new("Late Arrival").pak("late.pak", b"KPKA late");
    glow.install(&bench, &mut journal);
    cape.install(&bench, &mut journal);
    late.install(&bench, &mut journal);
    // The first two were installed before the last launch (21:34 local on
    // 2026-10-14), the third after it — whatever the bench's clock and zone.
    journal.installs[0].installed_at = "2026-10-01T08:00:00Z".into();
    journal.installs[1].installed_at = "2026-10-01T08:00:00Z".into();
    journal.installs[2].installed_at = "2026-10-15T12:00:00Z".into();
    // The journal took over a mod another tool had put here: its numbered
    // pak, a loose file, and one more file that is gone since.
    bench.write("pak_mods/x0000-Numbered Leftover.pak", b"KPKA oldpak");
    bench.write("natives/stm/leftover.mesh", b"orphan");
    let older = adopted(
        &mut journal,
        "adopted#Leftover-v1",
        "Numbered Leftover",
        "Leftover-v1",
        &[
            (
                "pak_mods/x0000-Numbered Leftover.pak",
                DeployKind::Pak,
                11,
                Below::Nothing,
            ),
            (
                "natives/stm/leftover.mesh",
                DeployKind::Loose,
                6,
                Below::Nothing,
            ),
            (
                "natives/stm/leftover_gone.mesh",
                DeployKind::Loose,
                4,
                Below::Nothing,
            ),
        ],
    );
    let older_id = older.option_id.clone();
    journal.installs.push(older);
    journal.pak_order.insert(0, older_id.clone());
    journal.save(bench.state()).unwrap();
    std::fs::remove_file(bench.root().join("natives/stm/cape/vanishes.mesh")).unwrap();

    bench.write("pak_mods/zz-unknown.pak", b"KPKA unknown");
    bench.write("pak_mods/sub/nested.pak", b"KPKA nested");
    bench.write("pak_mods/SHOUTING.PAK", b"KPKA shouting");
    bench.write("pak_mods/readme.txt", b"notes");
    bench.write("natives/stm/stray.mesh", b"stray");
    bench.write("reframework/autorun/stray.lua", b"-- stray");
    bench.write("reframework/data/runtime.json", b"{}"); // runtime data is not a stray
    bench.write(
        "re2_framework_log.txt",
        format!(
            "{LOG_HEAD}{}{}",
            injected("x0000-Numbered Leftover.pak", 103),
            injected("zrift-0000-glow.pak", 104)
        )
        .as_bytes(),
    );
    World {
        bench,
        glow,
        cape,
        late,
        older: older_id,
    }
}

fn state_of(w: &World) -> GameState {
    let mut bench_game = w.bench.game.clone();
    bench_game.build_id = Some("25300000".into());
    let ctx = rift_deploy::DeployContext {
        game: &bench_game,
        ..w.bench.ctx_with(false)
    };
    read_game_state(&ctx, &w.bench.journal())
}

#[test]
fn the_state_lists_every_pak_with_its_owner_and_load_state() {
    let w = world();
    let state = state_of(&w);
    let names: Vec<&str> = state.paks.iter().map(|p| p.game_path.as_str()).collect();
    assert_eq!(
        names,
        [
            "pak_mods/SHOUTING.PAK",
            "pak_mods/sub/nested.pak",
            "pak_mods/x0000-Numbered Leftover.pak",
            "pak_mods/zrift-0000-glow.pak",
            "pak_mods/zrift-0001-cape.pak",
            "pak_mods/zrift-0002-late.pak",
            "pak_mods/zz-unknown.pak",
        ],
        "in the order NTFS lists them"
    );
    let pak = |name: &str| state.paks.iter().find(|p| p.file_name == name).unwrap();
    assert!(matches!(
        &pak("zrift-0000-glow.pak").load,
        PakLoadState::Loaded { slot: 104, .. }
    ));
    assert_eq!(pak("zrift-0001-cape.pak").load, PakLoadState::NotLoaded);
    assert_eq!(pak("zrift-0002-late.pak").load, PakLoadState::Pending);
    assert!(matches!(
        &pak("zrift-0000-glow.pak").owner,
        PakOwner::Riftstone { option_id, adopted: false, .. } if *option_id == w.glow.id()
    ));
    assert!(matches!(
        &pak("x0000-Numbered Leftover.pak").owner,
        PakOwner::Riftstone { option_id, name, adopted: true }
            if *option_id == w.older && name == "Numbered Leftover"
    ));
    assert!(matches!(
        &pak("x0000-Numbered Leftover.pak").load,
        PakLoadState::Loaded { slot: 103, .. }
    ));
    assert_eq!(pak("zz-unknown.pak").owner, PakOwner::Unowned);
    assert!(pak("nested.pak").nested);
    assert!(!pak("SHOUTING.PAK").loads);
    assert_eq!(pak("SHOUTING.PAK").load_index, None);
    assert_eq!(pak("nested.pak").load_index, Some(0));
    assert_eq!(state.pak_count, 6, "the .PAK never loads");
    assert_eq!(state.pak_count_warning, 100);

    let summary = state
        .installs
        .iter()
        .find(|i| i.option_id == w.cape.id())
        .unwrap();
    assert_eq!(summary.paks.len(), 1);
    assert_eq!(summary.paks[0].load, PakLoadState::NotLoaded);
    assert_eq!(summary.kinds.loose, 2);
    let late = state
        .installs
        .iter()
        .find(|i| i.option_id == w.late.id())
        .unwrap();
    assert_eq!(late.paks[0].load, PakLoadState::Pending);
    assert_eq!(state.pak_order.len(), 4);
    assert_eq!(state.pak_order[0], w.older, "the numbered pak loads first");
    let older = state
        .installs
        .iter()
        .find(|i| i.option_id == w.older)
        .unwrap();
    assert!(
        matches!(&older.origin, rift_deploy::Origin::Adopted { label } if label == "Leftover-v1")
    );
    assert_eq!(older.paks.len(), 1);
    assert_eq!((older.kinds.pak, older.kinds.loose), (1, 2));
}

#[test]
fn the_state_finds_strays_and_the_missing_files_of_every_install() {
    let w = world();
    let state = state_of(&w);
    let strays: Vec<&str> = state.strays.iter().map(|s| s.game_path.as_str()).collect();
    assert_eq!(
        strays,
        [
            "natives/stm/stray.mesh",
            "pak_mods/SHOUTING.PAK",
            "pak_mods/readme.txt",
            "pak_mods/sub/nested.pak",
            "pak_mods/zz-unknown.pak",
            "reframework/autorun/stray.lua",
        ]
    );
    let missing: Vec<(&str, &str)> = state
        .missing
        .iter()
        .map(|m| (m.game_path.as_str(), m.option_name.as_str()))
        .collect();
    assert_eq!(
        missing,
        [
            ("natives/stm/cape/vanishes.mesh", "Ermine Cape"),
            ("natives/stm/leftover_gone.mesh", "Numbered Leftover"),
        ],
        "an adopted install's files are watched like any other's"
    );
    let change = state.build_changed.clone().unwrap();
    assert_eq!(
        (change.recorded.as_str(), change.now.as_str()),
        ("25290080", "25300000")
    );
    assert_eq!(state.outdated_files, 1);
    assert!(!state.interrupted);
    let json = serde_json::to_value(&state).unwrap();
    assert_eq!(json["paks"][3]["load"]["kind"], "loaded");
    assert_eq!(
        json["paks"][3]["load"]["nativeName"],
        "re_chunk_000.pak.sub_000.pak.patch_002.pak"
    );
    assert_eq!(json["paks"][2]["owner"]["kind"], "riftstone");
    assert_eq!(json["paks"][2]["owner"]["adopted"], true);
    assert_eq!(json["installs"][3]["origin"]["kind"], "adopted");
    let mut keys: Vec<&str> = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "buildChanged",
            "game",
            "installs",
            "interrupted",
            "missing",
            "outdatedFiles",
            "pakCount",
            "pakCountWarning",
            "pakLoad",
            "pakOrder",
            "paks",
            "parked",
            "reframework",
            "running",
            "strays",
        ],
        "the state carries exactly these fields"
    );
}

#[test]
fn preflight_names_every_problem_most_severe_first() {
    let w = world();
    let state = state_of(&w);
    let checks = preflight(&state);
    let mut ids: Vec<&str> = checks.iter().map(|c| c.id.as_str()).collect();
    ids.sort_unstable();
    assert_eq!(
        ids,
        [
            "buildChanged",
            "missingFiles",
            "outdatedFormats",
            "pakExtension",
            "pakNested",
            "paksLoaded",
            "reframework",
            "strayPakOrder",
            "strays",
        ],
        "exactly these checks, no others"
    );
    assert!(checks.windows(2).all(|pair| pair[0].level >= pair[1].level));
    let check = |id: &str| checks.iter().find(|c| c.id == id).unwrap();
    assert_eq!(check("reframework").level, CheckLevel::Ok);
    assert_eq!(
        check("paksLoaded").level,
        CheckLevel::Warn,
        "one managed pak never loaded"
    );
    assert!(check("missingFiles").detail.contains("Reinstall"));
    let strays = check("strays");
    assert_eq!(strays.level, CheckLevel::Info);
    assert_eq!(
        strays.title,
        "6 file(s) in mod folders belong to no install"
    );
    assert!(strays
        .detail
        .contains("no install in the journal claims them"));
    assert_eq!(strays.action, None);
}

#[test]
fn preflight_offers_the_switches_and_the_download() {
    let w = world();
    let mut state = state_of(&w);
    state.reframework = RefStatus {
        installed: true,
        pak_directory: Some(false),
        loose_file_loader: None,
        ..RefStatus::default()
    };
    state.running = true;
    let checks = preflight(&state);
    let check = |id: &str| checks.iter().find(|c| c.id == id).cloned().unwrap();
    assert_eq!(
        check("pakDirectory").action,
        Some(CheckAction::EnableSwitch {
            switch_key: RefSwitch::PakDirectory
        })
    );
    assert_eq!(
        check("looseFileLoader").action,
        Some(CheckAction::EnableSwitch {
            switch_key: RefSwitch::LooseFileLoader
        })
    );
    assert_eq!(check("gameRunning").level, CheckLevel::Info);
    let wire = serde_json::to_value(check("pakDirectory").action).unwrap();
    assert_eq!(
        wire,
        serde_json::json!({ "kind": "enableSwitch", "switch": "pakDirectory" })
    );

    state.reframework = RefStatus::default();
    let checks = preflight(&state);
    let missing = checks.iter().find(|c| c.id == "reframework").unwrap();
    assert_eq!(missing.level, CheckLevel::Error);
    assert!(
        matches!(&missing.action, Some(CheckAction::OpenUrl { url }) if url.contains("REFramework"))
    );
    assert_eq!(checks[0].level, CheckLevel::Error);
}

#[test]
fn an_empty_game_needs_little_and_a_crowded_one_is_warned() {
    let bench = Bench::new();
    let state = read_game_state(&bench.ctx(), &bench.journal());
    assert!(state.paks.is_empty() && state.strays.is_empty() && state.missing.is_empty());
    assert_eq!(state.build_changed, None);
    let checks = preflight(&state);
    assert_eq!(checks.len(), 1, "{checks:?}");
    assert_eq!(checks[0].id, "reframework");

    for n in 0..101 {
        bench.write(&format!("pak_mods/x{n:04}-filler.pak"), b"KPKA");
    }
    let state = read_game_state(&bench.ctx(), &bench.journal());
    assert_eq!(state.pak_count, 101);
    assert!(preflight(&state)
        .iter()
        .any(|c| c.id == "pakCount" && c.level == CheckLevel::Warn));
}

#[test]
fn reading_the_state_writes_nothing() {
    let w = world();
    let before = w.bench.snapshot();
    let journal_before = w.bench.journal();
    let _ = state_of(&w);
    assert_eq!(w.bench.snapshot(), before);
    assert_eq!(w.bench.journal(), journal_before);
}
