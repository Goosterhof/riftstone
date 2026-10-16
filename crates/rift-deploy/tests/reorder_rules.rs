//! § The DD2 load rules — `reorder_paks`: the given order becomes the pak
//! order, every managed pak is renamed to `zrift-<pos>-<stem>.pak` in two
//! journaled phases, never while the game runs.

mod common;

use common::{assert_no_temporaries, fluffy_mod, Bench, TestMod};
use rift_core::{OptionId, RiftError};
use rift_deploy::paks::ntfs_order_key;
use rift_deploy::{
    adopt_fluffy, recover, reorder_paks, FluffyLedger, Intent, IntentAction, IntentStep,
};

fn pak_mod(name: &str, pak: &str) -> TestMod {
    TestMod::new(name).pak(pak, format!("KPKA {name}").as_bytes())
}

#[test]
fn a_new_order_renames_every_pak_to_its_position() {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let a = pak_mod("Lantern Glow Recolor", "glow.pak");
    let b = pak_mod("Vernworth Cloaks", "cloaks.pak");
    let c = pak_mod("Ermine Cape", "cape.pak");
    for m in [&a, &b, &c] {
        m.install(&bench, &mut journal);
    }
    reorder_paks(&bench.ctx(), &mut journal, &[c.id(), a.id(), b.id()]).unwrap();

    assert_eq!(
        bench.names_in("pak_mods"),
        [
            "zrift-0000-cape.pak",
            "zrift-0001-glow.pak",
            "zrift-0002-cloaks.pak"
        ]
    );
    assert_eq!(
        bench.read("pak_mods/zrift-0000-cape.pak").unwrap(),
        b"KPKA Ermine Cape"
    );
    assert_eq!(journal.pak_order, [c.id(), a.id(), b.id()]);
    assert_eq!(
        journal.find(&a.id()).unwrap().files[0].game_path,
        "pak_mods/zrift-0001-glow.pak"
    );
    assert_eq!(bench.journal(), journal);
    assert!(journal.pending.is_none());
    assert_no_temporaries(&bench);

    // File-name order is load order, and it matches the pak order.
    let mut names = bench.names_in("pak_mods");
    names.sort_by_key(|n| ntfs_order_key(n));
    assert_eq!(
        names,
        [
            "zrift-0000-cape.pak",
            "zrift-0001-glow.pak",
            "zrift-0002-cloaks.pak"
        ]
    );

    // The same order again renames nothing.
    let before = bench.snapshot();
    reorder_paks(&bench.ctx(), &mut journal, &[c.id(), a.id(), b.id()]).unwrap();
    assert_eq!(bench.snapshot(), before);
}

#[test]
fn paks_of_one_name_swap_through_temporaries() {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let a = pak_mod("Armour A", "armour.pak");
    let b = pak_mod("Armour B", "armour.pak");
    a.install(&bench, &mut journal);
    b.install(&bench, &mut journal);
    assert_eq!(
        bench.names_in("pak_mods"),
        ["zrift-0000-armour.pak", "zrift-0001-armour.pak"]
    );
    reorder_paks(&bench.ctx(), &mut journal, &[b.id(), a.id()]).unwrap();
    assert_eq!(
        bench.read("pak_mods/zrift-0000-armour.pak").unwrap(),
        b"KPKA Armour B"
    );
    assert_eq!(
        bench.read("pak_mods/zrift-0001-armour.pak").unwrap(),
        b"KPKA Armour A"
    );
    assert_no_temporaries(&bench);
}

#[test]
fn a_running_game_or_a_wrong_order_is_refused_and_nothing_moves() {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let a = pak_mod("Lantern Glow Recolor", "glow.pak");
    let b = pak_mod("Vernworth Cloaks", "cloaks.pak");
    let loose = TestMod::new("Ermine Cape").loose("natives/stm/cape.mesh", b"cape");
    for m in [&a, &b, &loose] {
        m.install(&bench, &mut journal);
    }
    let before = bench.snapshot();
    let saved = bench.journal();
    let refused = |result: Result<(), RiftError>| matches!(result, Err(RiftError::Refused { .. }));

    let running = bench.ctx_with(None, true);
    assert!(refused(reorder_paks(
        &running,
        &mut journal,
        &[b.id(), a.id()]
    )));
    assert!(
        refused(reorder_paks(&bench.ctx(), &mut journal, &[b.id()])),
        "one left out"
    );
    assert!(
        refused(reorder_paks(
            &bench.ctx(),
            &mut journal,
            &[b.id(), a.id(), a.id()]
        )),
        "twice"
    );
    assert!(
        refused(reorder_paks(
            &bench.ctx(),
            &mut journal,
            &[b.id(), a.id(), loose.id()]
        )),
        "no paks"
    );
    assert!(refused(reorder_paks(
        &bench.ctx(),
        &mut journal,
        &[b.id(), a.id(), OptionId("nobody#nothing".into())]
    )));
    assert_eq!(bench.snapshot(), before);
    assert_eq!(bench.journal(), saved);
}

#[test]
fn a_stray_holding_a_target_name_blocks_the_reorder() {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let a = pak_mod("Lantern Glow Recolor", "glow.pak");
    let b = pak_mod("Vernworth Cloaks", "cloaks.pak");
    a.install(&bench, &mut journal);
    b.install(&bench, &mut journal);
    bench.write(
        "pak_mods/zrift-0000-cloaks.pak",
        b"KPKA a stray with that very name",
    );
    let before = bench.snapshot();
    let err = reorder_paks(&bench.ctx(), &mut journal, &[b.id(), a.id()]).unwrap_err();
    assert!(err.to_string().contains("does not manage"), "{err}");
    assert_eq!(bench.snapshot(), before);
}

#[test]
fn adopted_fluffy_paks_join_the_order_and_take_zrift_names() {
    let bench = Bench::new();
    bench.write("pak_mods/x0000-Older Armour.pak", b"KPKA older");
    bench.write("pak_mods/x0001-Lantern Glow.pak", b"KPKA glow");
    let ledger = FluffyLedger {
        game_install_path: None,
        mods: vec![
            fluffy_mod(
                "Lantern Glow-v2",
                "Lantern Glow Recolor",
                &["pak_mods/x0001-Lantern Glow.pak"],
            ),
            fluffy_mod(
                "Older",
                "Older Armour",
                &["pak_mods/x0000-Older Armour.pak"],
            ),
        ],
    };
    let ctx = bench.ctx_with(Some(&ledger), false);
    let mut journal = bench.journal();
    adopt_fluffy(&ctx, &mut journal, &ledger, &|_| None).unwrap();
    let older = OptionId("fluffy#Older".into());
    let glow = OptionId("fluffy#Lantern Glow-v2".into());
    assert_eq!(journal.pak_order, [older.clone(), glow.clone()]);

    let newest = pak_mod("Vernworth Cloaks", "cloaks.pak");
    newest.install_with(&ctx, &mut journal).unwrap();
    assert!(
        bench.exists("pak_mods/zrift-0002-cloaks.pak"),
        "after the adopted pair"
    );

    reorder_paks(
        &ctx,
        &mut journal,
        &[newest.id(), glow.clone(), older.clone()],
    )
    .unwrap();
    assert_eq!(
        bench.names_in("pak_mods"),
        [
            "zrift-0000-cloaks.pak",
            "zrift-0001-Lantern Glow.pak",
            "zrift-0002-Older Armour.pak"
        ],
        "Fluffy's x-prefix is stripped from the stem"
    );
    assert_eq!(
        bench.read("pak_mods/zrift-0002-Older Armour.pak").unwrap(),
        b"KPKA older"
    );
}

/// Build the journal a crash in the middle of a reorder leaves behind.
fn crashed_reorder(
    phase: u8,
) -> (
    Bench,
    rift_deploy::Journal,
    std::collections::BTreeMap<String, Option<Vec<u8>>>,
) {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let a = pak_mod("Armour A", "armour.pak");
    let b = pak_mod("Armour B", "armour.pak");
    a.install(&bench, &mut journal);
    b.install(&bench, &mut journal);
    let before = bench.snapshot();
    let steps = vec![
        IntentStep::Rename {
            from: "pak_mods/zrift-0000-armour.pak".into(),
            temp: "pak_mods/zrift-0000-armour.pak.riftstone-reorder-0".into(),
            to: "pak_mods/zrift-0001-armour.pak".into(),
        },
        IntentStep::Rename {
            from: "pak_mods/zrift-0001-armour.pak".into(),
            temp: "pak_mods/zrift-0001-armour.pak.riftstone-reorder-1".into(),
            to: "pak_mods/zrift-0000-armour.pak".into(),
        },
    ];
    let root = bench.root().to_path_buf();
    let mv = |from: &str, to: &str| std::fs::rename(root.join(from), root.join(to)).unwrap();
    // Phase 0 finished for both paks…
    mv(
        "pak_mods/zrift-0000-armour.pak",
        "pak_mods/zrift-0000-armour.pak.riftstone-reorder-0",
    );
    if phase == 0 {
        // …or, for phase 0, only for the first.
    } else {
        mv(
            "pak_mods/zrift-0001-armour.pak",
            "pak_mods/zrift-0001-armour.pak.riftstone-reorder-1",
        );
        // …and phase 1 for the first pak only.
        mv(
            "pak_mods/zrift-0000-armour.pak.riftstone-reorder-0",
            "pak_mods/zrift-0001-armour.pak",
        );
    }
    journal.pending = Some(Intent {
        action: IntentAction::Reorder,
        seq: 99,
        option_id: OptionId(String::new()),
        touched: vec![],
        started_at: "2026-10-16T10:00:00Z".into(),
        option_name: "the pak load order".into(),
        steps,
        phase,
        dirs: Vec::new(),
    });
    journal.save(bench.state()).unwrap();
    (bench, journal, before)
}

#[test]
fn a_crash_in_either_reorder_phase_is_rolled_back() {
    for phase in [0u8, 1] {
        let (bench, _, before) = crashed_reorder(phase);
        assert_ne!(bench.snapshot(), before);
        let mut relaunched = bench.journal();
        let report = recover(&bench.ctx(), &mut relaunched).unwrap().unwrap();
        assert_eq!(report.action, IntentAction::Reorder);
        assert_eq!(
            bench.snapshot(),
            before,
            "phase {phase}: the old names are back"
        );
        assert_eq!(
            bench.read("pak_mods/zrift-0000-armour.pak").unwrap(),
            b"KPKA Armour A"
        );
        assert!(bench.journal().pending.is_none());
    }
}

#[test]
fn a_taken_reorder_name_refuses_the_reorder_and_moves_nothing() {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let a = pak_mod("Lantern Glow Recolor", "glow.pak");
    let b = pak_mod("Vernworth Cloaks", "cloaks.pak");
    a.install(&bench, &mut journal);
    b.install(&bench, &mut journal);
    bench.write(
        "pak_mods/zrift-0000-glow.pak.riftstone-reorder-1",
        b"someone's file under our temporary name",
    );
    let before = bench.snapshot();
    let err = reorder_paks(&bench.ctx(), &mut journal, &[b.id(), a.id()]).unwrap_err();
    assert!(err.to_string().contains("riftstone-reorder"), "{err}");
    assert_eq!(bench.snapshot(), before);
    assert!(journal.pending.is_none());
}

/// Why a new pak's number sits above every zrift- name instead of at its
/// position in the pak order: after an uninstall leaves a gap, the position
/// (2 here) is already worn by C, and a stem that sorts first ("a") would
/// load BEFORE C although it is the newest install. File-name order must
/// equal the pak order; a reorder renumbers from 0000.
#[test]
fn names_keep_the_pak_order_after_a_gap() {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let a = pak_mod("Armour A", "x.pak");
    let b = pak_mod("Vernworth Cloaks", "b.pak");
    let c = pak_mod("Ermine Cape", "c.pak");
    for m in [&a, &b, &c] {
        m.install(&bench, &mut journal);
    }
    a.uninstall(&bench, &mut journal);
    let d = pak_mod("Ancient Armour", "a.pak");
    d.install(&bench, &mut journal);
    assert_eq!(journal.pak_order, [b.id(), c.id(), d.id()]);
    let mut names = bench.names_in("pak_mods");
    names.sort_by_key(|n| ntfs_order_key(n));
    assert_eq!(
        names,
        ["zrift-0001-b.pak", "zrift-0002-c.pak", "zrift-0003-a.pak"],
        "load order = pak order: the newest install loads last"
    );
    // A reorder closes the gap.
    reorder_paks(&bench.ctx(), &mut journal, &[b.id(), c.id(), d.id()]).unwrap();
    assert_eq!(
        bench.names_in("pak_mods"),
        ["zrift-0000-b.pak", "zrift-0001-c.pak", "zrift-0002-a.pak"]
    );
}
