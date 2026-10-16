//! § rift-deploy rules 1, 2, 3, 5, 8, 9 and the DD2 load rules — each one a
//! test, against a fake game folder.

mod common;

use std::panic::{catch_unwind, AssertUnwindSafe};

use common::{assert_no_temporaries, fluffy_mod, Bench, TestMod};
use rift_core::{DeployTarget, OptionId, RiftError};
use rift_deploy::{
    install, plan_install, plan_uninstall, recover, uninstall, Below, BelowOwner, DeployKind,
    FluffyLedger, Intent, IntentAction, IntentStep, PlanWarning, RefStatus, RemovalAction,
    WriteAction,
};

// ---------------------------------------------------------------------------
// The basics
// ---------------------------------------------------------------------------

#[test]
fn a_loose_install_lands_and_is_journaled_before_and_after() {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let glow = TestMod::new("Lantern Glow Recolor")
        .loose("natives/stm/effect/lantern.tex.251211553", b"glow")
        .script("reframework/autorun/lantern_glow.lua", b"-- glow");

    let plan = plan_install(&bench.ctx(), &journal, &glow.package, &glow.option);
    assert_eq!(plan.refused, None);
    assert_eq!(plan.writes.len(), 2);
    assert!(plan.writes.iter().all(|w| w.action == WriteAction::Create));
    assert_eq!(plan.total_bytes, 4 + 7);
    assert!(plan.warnings.is_empty(), "{:?}", plan.warnings);

    let report = glow.install(&bench, &mut journal);
    assert_eq!(report.files_written, 2);
    assert_eq!(report.backups, 0);
    assert!(!report.restart_needed);
    assert_eq!(
        bench
            .read("natives/stm/effect/lantern.tex.251211553")
            .unwrap(),
        b"glow"
    );
    assert_eq!(
        bench.read("reframework/autorun/lantern_glow.lua").unwrap(),
        b"-- glow"
    );

    let on_disk = bench.journal();
    assert_eq!(
        on_disk, journal,
        "the journal on disk is the journal in memory"
    );
    assert!(on_disk.pending.is_none());
    let record = &on_disk.installs[0];
    assert_eq!(record.name, "Lantern Glow Recolor");
    assert_eq!(record.game_build.as_deref(), Some("25290080"));
    let file = &record.files[0];
    assert_eq!(file.below, Below::Nothing);
    assert_eq!(file.kind, DeployKind::Loose);
    assert_eq!(file.size, 4);
    assert_eq!(
        file.entry.as_deref(),
        Some("Lantern Glow Recolor/natives/stm/effect/lantern.tex.251211553")
    );
    assert!(file.fingerprint.is_some());
    assert_no_temporaries(&bench);

    let removed = glow.uninstall(&bench, &mut journal);
    assert_eq!(removed.files_removed, 2);
    assert!(
        !bench.exists("natives"),
        "empty folders the install made are pruned"
    );
    assert!(!bench.exists("reframework"));
    assert!(bench.journal().installs.is_empty());
    assert!(
        bench.journal().cleanup.is_empty(),
        "the trash is deleted after the commit"
    );
    assert_no_temporaries(&bench);
}

#[test]
fn a_menu_entry_and_a_foreign_option_are_refused() {
    let bench = Bench::new();
    let journal = bench.journal();
    let menu = TestMod::new("Vernworth Cloaks Collection");
    let plan = plan_install(&bench.ctx(), &journal, &menu.package, &menu.option);
    assert!(plan.refused.unwrap().contains("nothing to install"));

    let glow = TestMod::new("Lantern Glow Recolor").loose("natives/stm/a.mesh", b"a");
    let other = TestMod::new("Ermine Cape");
    let plan = plan_install(&bench.ctx(), &journal, &other.package, &glow.option);
    assert!(plan.refused.is_some());
}

// ---------------------------------------------------------------------------
// Rule 1 — the target guard
// ---------------------------------------------------------------------------

#[test]
fn hostile_payloads_are_refused_and_nothing_is_written() {
    let hostile: Vec<(&str, DeployTarget)> = vec![
        (
            "outside",
            DeployTarget::Loose {
                game_path: "../outside/evil.mesh".into(),
            },
        ),
        (
            "exe",
            DeployTarget::Root {
                game_path: "DD2.exe".into(),
            },
        ),
        (
            "exe-case",
            DeployTarget::Root {
                game_path: "dd2.EXE".into(),
            },
        ),
        (
            "dlc-root",
            DeployTarget::Root {
                game_path: "dlc/x.pak".into(),
            },
        ),
        (
            "dlc-loose",
            DeployTarget::Loose {
                game_path: "dlc/x.pak".into(),
            },
        ),
        (
            "patch",
            DeployTarget::Root {
                game_path: "re_chunk_000.pak.patch_016.pak".into(),
            },
        ),
        (
            "chunk",
            DeployTarget::Root {
                game_path: "re_chunk_000.pak".into(),
            },
        ),
        (
            "steam",
            DeployTarget::Root {
                game_path: "steam_api64.dll".into(),
            },
        ),
        (
            "storage",
            DeployTarget::Reframework {
                game_path: "reframework/../_storage_/x.dll".into(),
            },
        ),
        (
            "climb",
            DeployTarget::Reframework {
                game_path: "reframework/../../outside.lua".into(),
            },
        ),
        (
            "subfolder",
            DeployTarget::Root {
                game_path: "bin/x64/evil.dll".into(),
            },
        ),
        (
            "trailing-dot",
            DeployTarget::Root {
                game_path: "DD2.exe.".into(),
            },
        ),
        (
            "kind-lie",
            DeployTarget::Loose {
                game_path: "reframework/autorun/x.lua".into(),
            },
        ),
    ];
    for (label, target) in hostile {
        let bench = Bench::new();
        let mut journal = bench.journal();
        // A harmless file first: the refusal must stop the WHOLE install.
        let evil = TestMod::new("Lantern Glow Recolor")
            .loose("natives/stm/harmless.mesh", b"fine")
            .target("payload/evil", target, b"evil bytes");
        let before = bench.snapshot();

        let plan = plan_install(&bench.ctx(), &journal, &evil.package, &evil.option);
        assert!(plan.refused.is_some(), "{label}: the plan must refuse");
        assert!(
            plan.writes.is_empty(),
            "{label}: a refused plan lists no writes"
        );
        let err = evil.install_with(&bench.ctx(), &mut journal).unwrap_err();
        assert!(matches!(err, RiftError::Refused { .. }), "{label}: {err}");

        assert_eq!(
            bench.snapshot(),
            before,
            "{label}: nothing in the game folder changed"
        );
        assert!(
            !bench.dir.path().join("outside").exists(),
            "{label}: nothing outside it either"
        );
        assert!(journal.installs.is_empty() && journal.pending.is_none());
        assert!(
            bench.store_files().is_empty(),
            "{label}: nothing was backed up"
        );
    }
}

// ---------------------------------------------------------------------------
// Rule 2 — backups before overwrites
// ---------------------------------------------------------------------------

#[test]
fn an_unowned_file_is_backed_up_before_it_is_overwritten_and_restored_after() {
    let bench = Bench::new();
    bench.write("natives/stm/shared.mesh", b"original bytes");
    let mut journal = bench.journal();
    let cape = TestMod::new("Ermine Cape").loose("natives/stm/shared.mesh", b"ermine");

    let plan = plan_install(&bench.ctx(), &journal, &cape.package, &cape.option);
    assert_eq!(plan.writes[0].action, WriteAction::OverwriteUnowned);

    let report = cape.install(&bench, &mut journal);
    assert_eq!(report.backups, 1);
    assert_eq!(bench.read("natives/stm/shared.mesh").unwrap(), b"ermine");
    let seq = journal.installs[0].seq;
    let store = format!("backups/{seq}/natives/stm/shared.mesh");
    assert_eq!(
        std::fs::read(bench.state().join(&store)).unwrap(),
        b"original bytes"
    );
    assert_eq!(
        journal.installs[0].files[0].below,
        Below::Backup {
            store,
            owner: BelowOwner::Unowned
        }
    );

    let plan = plan_uninstall(&bench.ctx(), &journal, &cape.id());
    assert!(matches!(
        plan.removals[0].action,
        RemovalAction::RestoreBackup { .. }
    ));
    let report = cape.uninstall(&bench, &mut journal);
    assert_eq!(report.files_restored, 1);
    assert_eq!(
        bench.read("natives/stm/shared.mesh").unwrap(),
        b"original bytes"
    );
    assert!(bench.store_files().is_empty());
    assert_no_temporaries(&bench);
}

#[test]
fn a_fluffy_file_is_backed_up_under_its_section() {
    let bench = Bench::new();
    bench.write("natives/stm/cape.mesh", b"fluffy's cape");
    let ledger = FluffyLedger {
        game_install_path: None,
        mods: vec![fluffy_mod(
            "Ermine-v1",
            "Ermine Cape",
            &["natives/stm/cape.mesh"],
        )],
    };
    let ctx = bench.ctx_with(Some(&ledger), false);
    let mut journal = bench.journal();
    let cape = TestMod::new("Vernworth Cape").loose("natives/stm/cape.mesh", b"vernworth");

    let plan = plan_install(&ctx, &journal, &cape.package, &cape.option);
    assert_eq!(
        plan.writes[0].action,
        WriteAction::OverwriteFluffy {
            section: "Ermine-v1".into()
        }
    );
    assert!(plan.warnings.contains(&PlanWarning::Conflicts {
        option_names: vec!["Ermine Cape".into()],
        paths: 1
    }));
    cape.install_with(&ctx, &mut journal).unwrap();
    assert!(matches!(
        &journal.installs[0].files[0].below,
        Below::Backup { owner: BelowOwner::Fluffy { section }, .. } if section == "Ermine-v1"
    ));
    uninstall(&ctx, &mut journal, &cape.id()).unwrap();
    assert_eq!(
        bench.read("natives/stm/cape.mesh").unwrap(),
        b"fluffy's cape"
    );
}

// ---------------------------------------------------------------------------
// Rule 3 — write-ahead, failure, crash, recovery
// ---------------------------------------------------------------------------

fn crash_test_mod() -> TestMod {
    TestMod::new("Vernworth Cloaks Collection")
        .loose("natives/stm/new/c.mesh", b"new c")
        .loose("natives/stm/a.mesh", b"mod a")
        .loose("natives/stm/b.mesh", b"mod b overwrites")
        .script("reframework/autorun/d.lua", b"-- d")
}

#[test]
fn a_source_failing_on_the_third_file_leaves_nothing_behind() {
    let bench = Bench::new();
    bench.write("natives/stm/b.mesh", b"original b");
    let mut journal = bench.journal();
    let cloaks = crash_test_mod();
    let before = bench.snapshot();

    let mut source = cloaks.source().failing_at(3);
    let err = install(
        &bench.ctx(),
        &mut journal,
        &cloaks.package,
        &cloaks.option,
        &mut source,
        &mut |_| {},
    )
    .unwrap_err();
    assert!(matches!(err, RiftError::CorruptArchive { .. }), "{err}");

    assert_eq!(
        bench.snapshot(),
        before,
        "parts deleted, folders pruned, the backup restored"
    );
    assert_eq!(bench.read("natives/stm/b.mesh").unwrap(), b"original b");
    let on_disk = bench.journal();
    assert!(on_disk.pending.is_none(), "pending is cleared");
    assert!(on_disk.installs.is_empty());
    assert!(journal.pending.is_none() && journal.installs.is_empty());
    assert!(bench.store_files().is_empty());
}

#[test]
fn a_crash_mid_install_is_rolled_back_at_the_next_launch() {
    let bench = Bench::new();
    bench.write("natives/stm/b.mesh", b"original b");
    let mut journal = bench.journal();
    let cloaks = crash_test_mod();
    let before = bench.snapshot();

    let mut source = cloaks.source().panicking_at(4);
    let crashed = catch_unwind(AssertUnwindSafe(|| {
        install(
            &bench.ctx(),
            &mut journal,
            &cloaks.package,
            &cloaks.option,
            &mut source,
            &mut |_| {},
        )
    }));
    assert!(crashed.is_err(), "the fake source crashed the install");

    // What the next launch finds: the intent on disk and half an extraction
    // in the game folder. Originals move aside only once every part is
    // extracted, so the one under b is still in place.
    let mut relaunched = bench.journal();
    let intent = relaunched
        .pending
        .clone()
        .expect("the intent survived the crash");
    assert_eq!(intent.action, IntentAction::Install);
    assert_eq!(intent.option_name, "Vernworth Cloaks Collection");
    assert_eq!(
        bench.read("natives/stm/b.mesh").unwrap(),
        b"original b",
        "the original has not moved yet"
    );
    assert!(bench.exists("natives/stm/a.mesh.riftstone-part"));
    assert_ne!(bench.snapshot(), before);

    // Nothing else may run until the intent is rolled back.
    let blocked = plan_install(&bench.ctx(), &relaunched, &cloaks.package, &cloaks.option);
    assert!(blocked.refused.unwrap().contains("interrupted"));

    let report = recover(&bench.ctx(), &mut relaunched)
        .unwrap()
        .expect("there was something to roll back");
    assert_eq!(report.action, IntentAction::Install);
    assert_eq!(
        report.files_rolled_back, 3,
        "three paths: the three parts extracted before the crash"
    );
    assert_eq!(report.files_kept, 0);
    assert!(report.parked.is_empty());
    assert_eq!(bench.snapshot(), before);
    assert!(bench.journal().pending.is_none());
    assert!(bench.store_files().is_empty());
    // Recovery is idempotent.
    assert_eq!(recover(&bench.ctx(), &mut relaunched).unwrap(), None);
}

#[test]
fn recover_rolls_back_an_intent_whose_files_were_all_in_place() {
    // A crash after the last rename but before the commit: every file is in
    // place, the backups are in the store, the journal still says pending.
    let bench = Bench::new();
    bench.write("natives/stm/b.mesh", b"original b");
    let before = bench.snapshot();
    let mut journal = bench.journal();
    let cloaks = crash_test_mod();
    cloaks.install(&bench, &mut journal);

    let record = journal.installs.pop().unwrap();
    journal.pak_order.clear();
    // The intent as the engine saves it once every part is extracted: each
    // step carries its fingerprint, and the folders the install made are
    // listed in the intent, not yet in the journal.
    journal.pending = Some(Intent {
        action: IntentAction::Install,
        seq: record.seq,
        option_id: record.option_id.clone(),
        touched: record.files.iter().map(|f| f.game_path.clone()).collect(),
        started_at: record.installed_at.clone(),
        option_name: record.name.clone(),
        steps: record
            .files
            .iter()
            .map(|f| IntentStep::Place {
                game_path: f.game_path.clone(),
                size: f.size,
                backup: match &f.below {
                    Below::Backup { store, .. } => Some(store.clone()),
                    _ => None,
                },
                owner: match &f.below {
                    Below::Backup { owner, .. } => Some(owner.clone()),
                    _ => None,
                },
                fingerprint: f.fingerprint.clone(),
                unextracted: false,
            })
            .collect(),
        phase: 0,
        dirs: std::mem::take(&mut journal.created_dirs),
    });
    journal.save(bench.state()).unwrap();

    let mut relaunched = bench.journal();
    let report = recover(&bench.ctx(), &mut relaunched).unwrap().unwrap();
    assert_eq!(report.files_rolled_back, 4);
    assert_eq!(bench.snapshot(), before);
    assert!(bench.store_files().is_empty());
}

#[test]
fn a_file_someone_else_wrote_after_a_crash_survives_recovery() {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let glow = TestMod::new("Lantern Glow Recolor").loose("natives/stm/glow.mesh", b"glow bytes");
    let mut source = glow.source().panicking_at(1);
    let crashed = catch_unwind(AssertUnwindSafe(|| {
        install(
            &bench.ctx(),
            &mut journal,
            &glow.package,
            &glow.option,
            &mut source,
            &mut |_| {},
        )
    }));
    assert!(crashed.is_err());
    // Before the Riftstone restarts, another tool writes the same path.
    bench.write("natives/stm/glow.mesh", b"another tool's bytes, longer");
    let mut relaunched = bench.journal();
    let report = recover(&bench.ctx(), &mut relaunched).unwrap().unwrap();
    assert_eq!(report.files_kept, 1);
    assert_eq!(
        bench.read("natives/stm/glow.mesh").unwrap(),
        b"another tool's bytes, longer"
    );
}

// ---------------------------------------------------------------------------
// Case — one path, whatever the spelling
// ---------------------------------------------------------------------------

#[test]
fn two_spellings_of_one_path_are_one_path() {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let upper = TestMod::new("Lantern Glow Recolor").loose("natives/STM/a.mesh", b"upper");
    let lower = TestMod::new("Ermine Cape").loose("natives/stm/a.mesh", b"lower!");
    upper.install(&bench, &mut journal);

    let plan = plan_install(&bench.ctx(), &journal, &lower.package, &lower.option);
    assert_eq!(
        plan.writes[0].action,
        WriteAction::OverwriteRiftstone {
            option_name: "Lantern Glow Recolor".into(),
            option_id: upper.id()
        },
        "natives/stm/a.mesh is the file natives/STM/a.mesh"
    );
    assert_eq!(
        plan.writes[0].game_path, "natives/STM/a.mesh",
        "the spelling on disk wins"
    );
    lower.install(&bench, &mut journal);

    assert_eq!(bench.names_in("natives"), ["STM"], "one folder, not two");
    assert_eq!(bench.names_in("natives/STM"), ["a.mesh"]);
    assert_eq!(bench.read("natives/STM/a.mesh").unwrap(), b"lower!");
    let file = &journal.installs[1].files[0];
    assert_eq!(file.game_path, "natives/STM/a.mesh");
    assert!(matches!(
        &file.below,
        Below::Backup { owner: BelowOwner::Riftstone { option_id }, .. } if *option_id == upper.id()
    ));
    // One stack: the lower layer hands down, the upper restores.
    let plan = plan_uninstall(&bench.ctx(), &journal, &upper.id());
    assert!(matches!(
        plan.removals[0].action,
        RemovalAction::HandDown { .. }
    ));
    lower.uninstall(&bench, &mut journal);
    assert_eq!(bench.read("natives/STM/a.mesh").unwrap(), b"upper");
    upper.uninstall(&bench, &mut journal);
    assert!(!bench.exists("natives"));
    assert!(bench.store_files().is_empty());
}

#[test]
fn duplicate_spellings_inside_one_option_collapse_to_the_last() {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let messy = TestMod::new("Lantern Glow Recolor")
        .target(
            "one/a.mesh",
            DeployTarget::Loose {
                game_path: "natives/stm/A.mesh".into(),
            },
            b"first",
        )
        .target(
            "two/a.mesh",
            DeployTarget::Loose {
                game_path: "natives/STM/a.mesh".into(),
            },
            b"second",
        );
    let plan = plan_install(&bench.ctx(), &journal, &messy.package, &messy.option);
    assert_eq!(plan.writes.len(), 1);
    messy.install(&bench, &mut journal);
    assert_eq!(plan.writes[0].game_path, "natives/STM/a.mesh");
    assert_eq!(bench.read("natives/STM/a.mesh").unwrap(), b"second");
}

// ---------------------------------------------------------------------------
// Rule 5 and the DD2 load rules — paks
// ---------------------------------------------------------------------------

#[test]
fn paks_land_flat_in_pak_mods_named_in_load_order() {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let glow = TestMod::new("Lantern Glow Recolor")
        .pak("Glow/x0003-Lantern Glow Recolor.PAK", b"KPKA glow");
    let cloaks = TestMod::new("Vernworth Cloaks").pak("Vernworth Cloaks.pak", b"KPKA cloaks");
    let again = TestMod::new("Lantern Glow Again")
        .pak("zrift-0000-Lantern Glow Recolor.pak", b"KPKA again");

    glow.install(&bench, &mut journal);
    cloaks.install(&bench, &mut journal);
    again.install(&bench, &mut journal);

    assert_eq!(
        bench.names_in("pak_mods"),
        [
            "zrift-0000-Lantern Glow Recolor.pak",
            "zrift-0001-Vernworth Cloaks.pak",
            "zrift-0002-Lantern Glow Recolor.pak",
        ],
        "prefix stripped, lowercase .pak, a new number each time — never an overwrite"
    );
    assert_eq!(
        bench
            .read("pak_mods/zrift-0000-Lantern Glow Recolor.pak")
            .unwrap(),
        b"KPKA glow"
    );
    assert_eq!(journal.pak_order, [glow.id(), cloaks.id(), again.id()]);
    assert_eq!(journal.installs[0].files[0].kind, DeployKind::Pak);
    assert_eq!(journal.installs[0].files[0].below, Below::Nothing);

    // An uninstall leaves a gap; the next install still sorts last.
    cloaks.uninstall(&bench, &mut journal);
    assert_eq!(journal.pak_order, [glow.id(), again.id()]);
    let late = TestMod::new("Late Arrival").pak("late.pak", b"KPKA late");
    late.install(&bench, &mut journal);
    assert!(bench.exists("pak_mods/zrift-0003-late.pak"));
}

#[test]
fn a_new_pak_sorts_after_fluffys_leftovers_and_stray_zrift_names() {
    let bench = Bench::new();
    bench.write("pak_mods/x0000-Fluffy Leftover.pak", b"KPKA fluffy");
    bench.write(
        "pak_mods/zrift-0041-Stray From A Lost Journal.pak",
        b"KPKA stray",
    );
    let mut journal = bench.journal();
    let glow = TestMod::new("Lantern Glow Recolor").pak("glow.pak", b"KPKA glow");
    glow.install(&bench, &mut journal);
    assert!(bench.exists("pak_mods/zrift-0042-glow.pak"));
    let mut names = bench.names_in("pak_mods");
    names.sort_by_key(|n| rift_deploy::paks::ntfs_order_key(n));
    assert_eq!(
        names.last().unwrap(),
        "zrift-0042-glow.pak",
        "the newest install loads last and wins"
    );
    // The stray and the leftover are untouched.
    assert_eq!(
        bench.read("pak_mods/x0000-Fluffy Leftover.pak").unwrap(),
        b"KPKA fluffy"
    );
}

#[test]
fn two_paks_of_one_name_in_one_option_both_install() {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let pair = TestMod::new("Paired Armour")
        .pak("Female/armour.pak", b"KPKA female")
        .pak("Male/armour.pak", b"KPKA male");
    pair.install(&bench, &mut journal);
    assert_eq!(
        bench.names_in("pak_mods"),
        ["zrift-0000-armour-2.pak", "zrift-0000-armour.pak"]
    );
    assert_eq!(journal.pak_order, [pair.id()]);
}

#[test]
fn pak_numbers_that_run_out_refuse_with_a_way_forward() {
    let bench = Bench::new();
    bench.write("pak_mods/zrift-9999-last.pak", b"KPKA");
    let journal = bench.journal();
    let glow = TestMod::new("Lantern Glow Recolor").pak("glow.pak", b"KPKA glow");
    let plan = plan_install(&bench.ctx(), &journal, &glow.package, &glow.option);
    assert!(plan.refused.unwrap().contains("Reorder"));
}

// ---------------------------------------------------------------------------
// Rule 8 — the game is running
// ---------------------------------------------------------------------------

#[test]
fn while_the_game_runs_paks_install_with_a_warning_but_never_leave() {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let running = bench.ctx_with(None, true);
    let glow = TestMod::new("Lantern Glow Recolor").pak("glow.pak", b"KPKA glow");
    let cape = TestMod::new("Ermine Cape").loose("natives/stm/cape.mesh", b"cape");

    let plan = plan_install(&running, &journal, &glow.package, &glow.option);
    assert_eq!(
        plan.refused, None,
        "a pak install is never refused for a running game"
    );
    assert!(plan.warnings.contains(&PlanWarning::GameRunning));
    let report = glow.install_with(&running, &mut journal).unwrap();
    assert!(report.restart_needed, "it loads at the next launch");

    let plan = plan_uninstall(&running, &journal, &glow.id());
    assert!(plan.refused.is_some());
    let err = uninstall(&running, &mut journal, &glow.id()).unwrap_err();
    assert!(matches!(err, RiftError::Refused { .. }));
    assert!(bench.exists("pak_mods/zrift-0000-glow.pak"));
    // Reinstalling a pak-bearing option is an uninstall too.
    let plan = plan_install(&running, &journal, &glow.package, &glow.option);
    assert!(plan.refused.is_some());

    // Loose files may come and go while it runs.
    cape.install_with(&running, &mut journal).unwrap();
    let report = uninstall(&running, &mut journal, &cape.id()).unwrap();
    assert!(report.restart_needed);
    assert!(!bench.exists("natives/stm/cape.mesh"));
}

// ---------------------------------------------------------------------------
// Rule 9 and the prerequisites — warnings, never refusals
// ---------------------------------------------------------------------------

#[test]
fn missing_requirements_warn_and_never_refuse() {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let needy = TestMod::new("Ermine Cape")
        .loose("natives/stm/cape.mesh", b"cape")
        .requires(&["Gransys Wardrobe Loader", "RE Framework", "  "]);
    let plan = plan_install(&bench.ctx(), &journal, &needy.package, &needy.option);
    assert_eq!(plan.refused, None);
    assert_eq!(
        plan.warnings,
        [PlanWarning::RequiresMissing {
            name: "Gransys Wardrobe Loader".into()
        }],
        "REFramework counts as installed when its dll is there"
    );

    let loader = TestMod::new("Gransys Wardrobe Loader")
        .script("reframework/autorun/wardrobe_loader.lua", b"--");
    loader.install(&bench, &mut journal);
    let plan = plan_install(&bench.ctx(), &journal, &needy.package, &needy.option);
    assert!(plan.warnings.is_empty(), "{:?}", plan.warnings);

    // A Fluffy install satisfies a requirement too.
    let bench = Bench::new();
    let journal = bench.journal();
    let ledger = FluffyLedger {
        game_install_path: None,
        mods: vec![fluffy_mod("WardrobeLoader", "Gransys Wardrobe Loader", &[])],
    };
    let plan = plan_install(
        &bench.ctx_with(Some(&ledger), false),
        &journal,
        &needy.package,
        &needy.option,
    );
    assert!(plan.warnings.is_empty(), "{:?}", plan.warnings);
}

#[test]
fn prerequisite_warnings_follow_reframeworks_switches() {
    let mut bench = Bench::new();
    let journal = bench.journal();
    let pak = TestMod::new("Lantern Glow Recolor").pak("glow.pak", b"KPKA");
    let loose = TestMod::new("Ermine Cape").loose("natives/stm/cape.mesh", b"cape");
    let warnings = |bench: &Bench, m: &TestMod| {
        plan_install(&bench.ctx(), &journal, &m.package, &m.option).warnings
    };

    bench.reframework = RefStatus::default();
    assert_eq!(warnings(&bench, &pak), [PlanWarning::ReframeworkMissing]);
    assert_eq!(
        warnings(&bench, &loose),
        [PlanWarning::ReframeworkMissing, PlanWarning::LooseLoaderOff],
        "an absent LooseFileLoader key means off"
    );
    let reframework = TestMod::new("RE Framework").root_file("dinput8.dll", b"MZ");
    assert!(
        warnings(&bench, &reframework).is_empty(),
        "REFramework itself needs no REFramework"
    );

    bench.reframework = RefStatus {
        installed: true,
        pak_directory: Some(false),
        loose_file_loader: Some(false),
        ..RefStatus::default()
    };
    assert_eq!(warnings(&bench, &pak), [PlanWarning::PakFolderOff]);
    assert_eq!(warnings(&bench, &loose), [PlanWarning::LooseLoaderOff]);

    bench.reframework = RefStatus {
        installed: true,
        pak_directory: None,
        loose_file_loader: Some(true),
        ..RefStatus::default()
    };
    assert!(
        warnings(&bench, &pak).is_empty(),
        "an absent pak-folder key means on"
    );
    assert!(warnings(&bench, &loose).is_empty());
}

#[test]
fn outdated_formats_and_conflicts_are_counted() {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let old = TestMod::new("Old Engine Armour")
        .loose("natives/stm/armour/top_wb_f.mesh.240423143", b"old mesh")
        .loose("natives/stm/armour/top_wb_f.tex.760230703", b"old tex")
        .loose("natives/stm/armour/top_wb_f.mdf2.51", b"new mdf");
    let plan = plan_install(&bench.ctx(), &journal, &old.package, &old.option);
    assert_eq!(plan.warnings, [PlanWarning::OutdatedFormats { count: 2 }]);

    let first = TestMod::new("Lantern Glow Recolor")
        .loose("natives/stm/a.mesh", b"a")
        .loose("natives/stm/b.mesh", b"b");
    first.install(&bench, &mut journal);
    let second = TestMod::new("Ermine Cape")
        .loose("natives/stm/A.MESH", b"aa")
        .loose("natives/stm/b.mesh", b"bb")
        .loose("natives/stm/c.mesh", b"cc");
    let plan = plan_install(&bench.ctx(), &journal, &second.package, &second.option);
    assert_eq!(
        plan.warnings,
        [PlanWarning::Conflicts {
            option_names: vec!["Lantern Glow Recolor".into()],
            paths: 2
        }]
    );
}

// ---------------------------------------------------------------------------
// Reinstall
// ---------------------------------------------------------------------------

#[test]
fn reinstalling_an_option_brings_it_back_on_top() {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let glow = TestMod::new("Lantern Glow Recolor").loose("natives/stm/a.mesh", b"glow");
    let cape = TestMod::new("Ermine Cape").loose("natives/stm/a.mesh", b"cape!");
    glow.install(&bench, &mut journal);

    let plan = plan_install(&bench.ctx(), &journal, &glow.package, &glow.option);
    assert_eq!(plan.writes[0].action, WriteAction::Reinstall);
    glow.install(&bench, &mut journal);
    assert_eq!(journal.installs.len(), 1, "one record, not two");

    cape.install(&bench, &mut journal);
    glow.install(&bench, &mut journal);
    assert_eq!(bench.read("natives/stm/a.mesh").unwrap(), b"glow");
    assert_eq!(journal.installs.len(), 2);
    glow.uninstall(&bench, &mut journal);
    assert_eq!(bench.read("natives/stm/a.mesh").unwrap(), b"cape!");
    cape.uninstall(&bench, &mut journal);
    assert!(!bench.exists("natives/stm/a.mesh"));
    assert!(bench.store_files().is_empty());
}

#[test]
fn an_uninstall_of_something_never_installed_is_refused() {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let err = uninstall(&bench.ctx(), &mut journal, &OptionId("nope#nothing".into())).unwrap_err();
    assert!(matches!(err, RiftError::Refused { .. }));
}

// ---------------------------------------------------------------------------
// A rollback deletes only what it can prove this install wrote
// ---------------------------------------------------------------------------

fn crash_at_first_file(bench: &Bench, journal: &mut rift_deploy::Journal, glow: &TestMod) {
    let mut source = glow.source().panicking_at(1);
    let crashed = catch_unwind(AssertUnwindSafe(|| {
        install(
            &bench.ctx(),
            journal,
            &glow.package,
            &glow.option,
            &mut source,
            &mut |_| {},
        )
    }));
    assert!(crashed.is_err(), "the fake source crashed the install");
}

#[test]
fn a_same_size_file_written_after_a_crash_survives_recovery() {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let glow = TestMod::new("Lantern Glow Recolor").loose("natives/stm/glow.mesh", b"glow bytes");
    crash_at_first_file(&bench, &mut journal, &glow);
    // Before the Riftstone restarts, another tool writes the path — exactly
    // as many bytes as the install planned.
    bench.write("natives/stm/glow.mesh", b"FLUFFY GLW");
    let mut relaunched = bench.journal();
    let report = recover(&bench.ctx(), &mut relaunched).unwrap().unwrap();
    assert_eq!(report.files_kept, 1);
    assert_eq!(bench.read("natives/stm/glow.mesh").unwrap(), b"FLUFFY GLW");
    assert_no_temporaries(&bench);
}

#[test]
fn a_same_size_file_written_over_an_original_after_a_crash_survives_recovery() {
    let bench = Bench::new();
    bench.write("natives/stm/glow.mesh", b"the original file");
    let mut journal = bench.journal();
    let glow = TestMod::new("Lantern Glow Recolor").loose("natives/stm/glow.mesh", b"glow bytes");
    crash_at_first_file(&bench, &mut journal, &glow);
    bench.write("natives/stm/glow.mesh", b"FLUFFY GLW");
    let mut relaunched = bench.journal();
    recover(&bench.ctx(), &mut relaunched).unwrap().unwrap();
    assert_eq!(
        bench.read("natives/stm/glow.mesh").unwrap(),
        b"FLUFFY GLW",
        "the other tool's file is never deleted to make room"
    );
    assert_no_temporaries(&bench);
}

#[test]
fn a_same_size_file_appearing_during_extraction_is_left_alone() {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let glow = TestMod::new("Lantern Glow Recolor").loose("natives/stm/glow.mesh", b"glow bytes");
    let target = bench.root().join("natives/stm/glow.mesh");
    let mut source = glow.source().after_extract(move || {
        std::fs::write(&target, b"FOREIGN 10").unwrap();
    });
    let err = install(
        &bench.ctx(),
        &mut journal,
        &glow.package,
        &glow.option,
        &mut source,
        &mut |_| {},
    )
    .unwrap_err();
    assert!(matches!(err, RiftError::Refused { .. }), "{err}");
    assert_eq!(
        bench.read("natives/stm/glow.mesh").unwrap(),
        b"FOREIGN 10",
        "the error says it was left alone, and it was"
    );
    assert!(journal.pending.is_none() && bench.journal().pending.is_none());
    assert!(journal.installs.is_empty());
    assert_no_temporaries(&bench);
}

// ---------------------------------------------------------------------------
// Rule 1 against links inside the game folder
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn a_link_inside_the_game_folder_never_leads_a_payload_onto_vanilla_files() {
    let cases: Vec<(&str, &str, DeployTarget)> = vec![
        (
            "natives/vault",
            "",
            DeployTarget::Loose {
                game_path: "natives/vault/re_chunk_000.pak".into(),
            },
        ),
        (
            "natives/door",
            "dlc",
            DeployTarget::Loose {
                game_path: "natives/door/re_dlc_stm_2581000.pak".into(),
            },
        ),
        (
            "reframework/x",
            "",
            DeployTarget::Reframework {
                game_path: "reframework/x/DD2.exe".into(),
            },
        ),
        (
            "reframework/y",
            "",
            DeployTarget::Reframework {
                game_path: "reframework/y/steam_api64.dll".into(),
            },
        ),
        (
            "pak_mods",
            "",
            DeployTarget::Pak {
                file_name: "evil.pak".into(),
            },
        ),
    ];
    for (link, points_to, target) in cases {
        let bench = Bench::new();
        let link_path = bench.root().join(link);
        std::fs::create_dir_all(link_path.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(bench.root().join(points_to), &link_path).unwrap();
        let mut journal = bench.journal();
        let evil =
            TestMod::new("Lantern Glow Recolor").target("payload/evil", target, b"EVIL BYTES");
        let before = bench.snapshot();

        let plan = plan_install(&bench.ctx(), &journal, &evil.package, &evil.option);
        assert!(plan.refused.is_some(), "{link}: the plan must refuse");
        let err = evil.install_with(&bench.ctx(), &mut journal).unwrap_err();
        assert!(matches!(err, RiftError::Refused { .. }), "{link}: {err}");
        assert_eq!(bench.snapshot(), before, "{link}: nothing changed");
        assert_eq!(
            bench.read("re_chunk_000.pak").unwrap(),
            b"KPKA vanilla chunk"
        );
        assert_eq!(
            bench.read("dlc/re_dlc_stm_2581000.pak").unwrap(),
            b"KPKA dlc"
        );
        assert_eq!(bench.read("DD2.exe").unwrap(), b"");
        assert_eq!(bench.read("steam_api64.dll").unwrap(), b"MZ steam");
        assert!(
            bench.store_files().is_empty(),
            "{link}: nothing moved aside"
        );
    }
}

// ---------------------------------------------------------------------------
// The engine's temporary names are never written through or over
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn a_link_at_a_temporary_name_is_never_written_through() {
    let bench = Bench::new();
    let outside = bench.dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("victim.txt"), b"the victim").unwrap();
    std::fs::create_dir_all(bench.root().join("natives/stm")).unwrap();
    std::os::unix::fs::symlink(
        outside.join("victim.txt"),
        bench.root().join("natives/stm/a.mesh.riftstone-part"),
    )
    .unwrap();
    let mut journal = bench.journal();
    let glow = TestMod::new("Lantern Glow Recolor").loose("natives/stm/a.mesh", b"EVIL BYTES");
    let before = bench.snapshot();

    let plan = plan_install(&bench.ctx(), &journal, &glow.package, &glow.option);
    assert!(plan.refused.is_some());
    let err = glow.install_with(&bench.ctx(), &mut journal).unwrap_err();
    assert!(matches!(err, RiftError::Refused { .. }), "{err}");
    assert_eq!(
        std::fs::read(outside.join("victim.txt")).unwrap(),
        b"the victim"
    );
    assert_eq!(
        bench.snapshot(),
        before,
        "the link itself is left alone too"
    );
}

#[test]
fn a_file_already_holding_a_temporary_name_is_never_clobbered() {
    let bench = Bench::new();
    bench.write("natives/stm/a.mesh.riftstone-part", b"someone's file");
    let mut journal = bench.journal();
    let glow = TestMod::new("Lantern Glow Recolor").loose("natives/stm/a.mesh", b"glow");
    let before = bench.snapshot();
    let plan = plan_install(&bench.ctx(), &journal, &glow.package, &glow.option);
    assert!(plan.refused.unwrap().contains("riftstone-part"));
    let err = glow.install_with(&bench.ctx(), &mut journal).unwrap_err();
    assert!(matches!(err, RiftError::Refused { .. }), "{err}");
    assert_eq!(bench.snapshot(), before);
}

// ---------------------------------------------------------------------------
// One payload path cannot be both a file and a folder
// ---------------------------------------------------------------------------

#[test]
fn a_payload_that_is_both_a_file_and_a_folder_is_refused() {
    for (nested_first, original) in [(false, true), (true, true), (false, false), (true, false)] {
        let bench = Bench::new();
        if original {
            bench.write("natives/stm/a", b"ORIGINAL unowned file");
        } else {
            std::fs::create_dir_all(bench.root().join("natives/stm")).unwrap();
        }
        let mut journal = bench.journal();
        let file = ("natives/stm/a", b"file a".as_slice());
        let nested = ("natives/stm/a/b", b"nested b".as_slice());
        let order = if nested_first {
            [nested, file]
        } else {
            [file, nested]
        };
        let mut clash = TestMod::new("Broken Archive");
        for (path, bytes) in order {
            clash = clash.loose(path, bytes);
        }
        let before = bench.snapshot();
        let plan = plan_install(&bench.ctx(), &journal, &clash.package, &clash.option);
        let reason = plan.refused.unwrap_or_default();
        assert!(
            reason.contains(if original {
                "A file sits where"
            } else {
                "both a file and a folder"
            }),
            "nested first = {nested_first}, original = {original}: {reason}"
        );
        let err = clash.install_with(&bench.ctx(), &mut journal).unwrap_err();
        assert!(matches!(err, RiftError::Refused { .. }), "{err}");
        assert_eq!(bench.snapshot(), before, "nested first = {nested_first}");
        assert!(bench.store_files().is_empty());
        assert!(journal.pending.is_none() && journal.installs.is_empty());
    }
}

#[test]
fn a_payload_under_an_existing_file_is_refused() {
    let bench = Bench::new();
    bench.write("natives/stm/a", b"a file, not a folder");
    let mut journal = bench.journal();
    let nested = TestMod::new("Broken Archive").loose("natives/stm/a/b", b"nested b");
    let before = bench.snapshot();
    let plan = plan_install(&bench.ctx(), &journal, &nested.package, &nested.option);
    assert!(plan.refused.is_some());
    assert!(nested.install_with(&bench.ctx(), &mut journal).is_err());
    assert_eq!(bench.snapshot(), before);
}

// ---------------------------------------------------------------------------
// An undo that trips on one file still undoes the rest
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn an_undo_that_trips_on_one_file_still_undoes_the_others() {
    if !common::permissions_bite() {
        return;
    }
    let bench = Bench::new();
    let mut journal = bench.journal();
    let pair = TestMod::new("Paired Meshes")
        .loose("natives/a/x.mesh", b"x mesh")
        .loose("natives/b/y.mesh", b"y mesh");
    let locked = bench.root().join("natives/b");
    let lock = locked.clone();
    let mut source = pair.source().after_extract(move || {
        common::set_read_only(&lock, true, false);
    });
    let err = install(
        &bench.ctx(),
        &mut journal,
        &pair.package,
        &pair.option,
        &mut source,
        &mut |_| {},
    )
    .unwrap_err();
    assert!(
        !bench.exists("natives/a/x.mesh"),
        "x was undone although y's undo failed: {err}"
    );
    assert!(
        journal.pending.is_some(),
        "the unfinished undo stays pending"
    );
    assert!(bench.journal().pending.is_some(), "…on disk too");

    common::set_read_only(&locked, false, false);
    recover(&bench.ctx(), &mut journal).unwrap().unwrap();
    assert!(!bench.exists("natives"));
    assert!(bench.journal().pending.is_none());
}

// ---------------------------------------------------------------------------
// A rollback whose journal save fails says exactly that
// ---------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn a_rollback_whose_journal_save_fails_says_so_and_keeps_pending() {
    if !common::permissions_bite() {
        return;
    }
    let bench = Bench::new();
    let mut journal = bench.journal();
    let glow = TestMod::new("Lantern Glow Recolor")
        .loose("natives/stm/glow.mesh", b"glow")
        .script("reframework/autorun/glow.lua", b"-- glow");
    let before = bench.snapshot();
    let state = bench.state().to_path_buf();
    let mut source = glow.source().after_extract(move || {
        common::set_read_only(&state, true, false);
    });
    let err = install(
        &bench.ctx(),
        &mut journal,
        &glow.package,
        &glow.option,
        &mut source,
        &mut |_| {},
    )
    .unwrap_err()
    .to_string();
    common::set_read_only(bench.state(), false, false);

    assert_eq!(bench.snapshot(), before, "the game folder was put back");
    assert!(
        err.contains("was put back"),
        "the message says the folder is back: {err}"
    );
    assert!(!err.contains("failed too"), "{err}");
    assert_eq!(
        journal.pending.is_some(),
        bench.journal().pending.is_some(),
        "memory and disk agree"
    );
    let report = recover(&bench.ctx(), &mut journal).unwrap();
    assert!(report.is_some());
    assert_eq!(bench.snapshot(), before);
    assert!(bench.journal().pending.is_none());
}

// ---------------------------------------------------------------------------
// Folders the player made are never pruned
// ---------------------------------------------------------------------------

#[test]
fn folders_the_player_made_are_never_pruned() {
    let bench = Bench::new();
    std::fs::create_dir_all(bench.root().join("natives/stm/players_own_folder")).unwrap();
    let mut journal = bench.journal();
    let glow = TestMod::new("Lantern Glow Recolor")
        .loose("natives/stm/players_own_folder/x.mesh", b"x")
        .loose("natives/stm/players_own_folder/new/y.mesh", b"y");
    glow.install(&bench, &mut journal);
    glow.uninstall(&bench, &mut journal);
    assert!(
        bench.exists("natives/stm/players_own_folder"),
        "the uninstall keeps the folder it found"
    );
    assert!(
        !bench.exists("natives/stm/players_own_folder/new"),
        "…and prunes the one it made"
    );

    // A rolled-back install keeps it too.
    let mut source = glow.source().failing_at(2);
    install(
        &bench.ctx(),
        &mut journal,
        &glow.package,
        &glow.option,
        &mut source,
        &mut |_| {},
    )
    .unwrap_err();
    assert!(bench.exists("natives/stm/players_own_folder"));
    assert!(!bench.exists("natives/stm/players_own_folder/new"));
}

// ---------------------------------------------------------------------------
// A reinstall is all or nothing
// ---------------------------------------------------------------------------

#[test]
fn a_reinstall_that_fails_leaves_the_option_installed_as_it_was() {
    let bench = Bench::new();
    bench.write("natives/stm/a.mesh", b"original a");
    let mut journal = bench.journal();
    let glow = TestMod::new("Lantern Glow Recolor")
        .loose("natives/stm/a.mesh", b"glow a")
        .loose("natives/stm/b.mesh", b"glow b");
    glow.install(&bench, &mut journal);
    let installed = bench.snapshot();
    let saved = bench.journal();

    let mut source = glow.source().failing_at(2);
    let err = install(
        &bench.ctx(),
        &mut journal,
        &glow.package,
        &glow.option,
        &mut source,
        &mut |_| {},
    )
    .unwrap_err();
    assert!(matches!(err, RiftError::CorruptArchive { .. }), "{err}");
    assert!(journal.is_installed(&glow.id()), "still installed");
    assert_eq!(bench.snapshot(), installed, "the old install is whole");
    assert_eq!(bench.read("natives/stm/a.mesh").unwrap(), b"glow a");
    let on_disk = bench.journal();
    assert!(on_disk.pending.is_none());
    assert_eq!(on_disk.installs, saved.installs);
    // …and it still uninstalls to the original.
    glow.uninstall(&bench, &mut journal);
    assert_eq!(bench.read("natives/stm/a.mesh").unwrap(), b"original a");
    assert!(bench.store_files().is_empty());
}

#[test]
fn a_reinstall_preview_names_the_pak_the_reinstall_writes() {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let x = TestMod::new("Lantern Glow Recolor").pak("x.pak", b"KPKA x");
    x.install(&bench, &mut journal);
    let plan = plan_install(&bench.ctx(), &journal, &x.package, &x.option);
    assert_eq!(plan.writes[0].game_path, "pak_mods/zrift-0000-x.pak");
    assert_eq!(plan.writes[0].action, WriteAction::Reinstall);
    x.install(&bench, &mut journal);
    assert_eq!(bench.names_in("pak_mods"), ["zrift-0000-x.pak"]);

    // With a pak installed after it, the reinstall moves to the end.
    let b = TestMod::new("Vernworth Cloaks").pak("b.pak", b"KPKA b");
    b.install(&bench, &mut journal);
    let plan = plan_install(&bench.ctx(), &journal, &x.package, &x.option);
    assert_eq!(plan.writes[0].game_path, "pak_mods/zrift-0002-x.pak");
    assert_eq!(plan.writes[0].action, WriteAction::Reinstall);
    x.install(&bench, &mut journal);
    assert_eq!(
        bench.names_in("pak_mods"),
        ["zrift-0001-b.pak", "zrift-0002-x.pak"]
    );
    assert_eq!(journal.pak_order, [b.id(), x.id()]);
}

#[test]
fn a_taken_trash_name_refuses_the_uninstall_and_moves_nothing() {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let glow = TestMod::new("Lantern Glow Recolor").loose("natives/stm/a.mesh", b"glow");
    glow.install(&bench, &mut journal);
    let trash = format!("natives/stm/a.mesh.riftstone-trash-{}", journal.next_seq);
    bench.write(&trash, b"someone's file under our trash name");
    let before = bench.snapshot();
    let err = uninstall(&bench.ctx(), &mut journal, &glow.id()).unwrap_err();
    assert!(err.to_string().contains("riftstone-trash"), "{err}");
    assert_eq!(bench.snapshot(), before);
    assert!(journal.is_installed(&glow.id()) && journal.pending.is_none());
}

#[test]
fn a_payload_using_the_riftstones_temporary_names_is_refused() {
    let bench = Bench::new();
    let journal = bench.journal();
    let sneaky = TestMod::new("Sneaky Archive")
        .loose("natives/stm/a.mesh", b"a")
        .loose("natives/stm/a.mesh.riftstone-part", b"pretend part");
    let plan = plan_install(&bench.ctx(), &journal, &sneaky.package, &sneaky.option);
    assert!(plan.refused.unwrap().contains(".riftstone-"));
}
