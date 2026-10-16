//! § rift-deploy rule 4 — the overwrite stack — and rule 7 — never delete
//! what you do not own.

mod common;

use common::{assert_no_temporaries, fluffy_mod, journal_text, Bench, TestMod};
use rift_core::RiftError;
use rift_deploy::{
    discard_parked, plan_install, plan_uninstall, preflight, read_game_state, restore_parked,
    uninstall, Below, BelowOwner, CheckAction, CheckLevel, FluffyLedger, OwnerKind, RemovalAction,
    WriteAction,
};

const P: &str = "natives/stm/character/stacked_wb_f.mesh.260421070";

fn three_layers() -> [TestMod; 3] {
    [
        TestMod::new("Layer A").loose(P, b"bytes of layer A"),
        TestMod::new("Layer B").loose(P, b"bytes of layer B, longer"),
        TestMod::new("Layer C").loose(P, b"C"),
    ]
}

const ORDERS: [[usize; 3]; 6] = [
    [0, 1, 2],
    [0, 2, 1],
    [1, 0, 2],
    [1, 2, 0],
    [2, 0, 1],
    [2, 1, 0],
];

/// Install A, B, C over one path, uninstall them in `order`, and check the
/// bytes at the path after every step: always the newest remaining layer's,
/// else `original` (or nothing).
fn run_order(order: [usize; 3], original: Option<&[u8]>) {
    let bench = Bench::new();
    if let Some(bytes) = original {
        bench.write(P, bytes);
    }
    let layers = three_layers();
    let mut journal = bench.journal();
    for layer in &layers {
        layer.install(&bench, &mut journal);
    }
    assert_eq!(bench.read(P).unwrap(), b"C");

    let mut remaining = vec![0usize, 1, 2];
    for &gone in &order {
        layers[gone].uninstall(&bench, &mut journal);
        remaining.retain(|&n| n != gone);
        let expected: Option<Vec<u8>> = match remaining.last() {
            Some(&top) => Some(layers[top].files[0].1.clone()),
            None => original.map(<[u8]>::to_vec),
        };
        assert_eq!(
            bench.read(P),
            expected,
            "order {order:?}: after uninstalling layer {gone}, {remaining:?} remain"
        );
        assert_eq!(bench.journal(), journal);
    }
    assert!(journal.installs.is_empty());
    assert!(
        bench.store_files().is_empty(),
        "order {order:?}: the store ends clean"
    );
    assert!(journal.cleanup.is_empty());
    if original.is_none() {
        assert!(
            !bench.exists("natives"),
            "order {order:?}: no folder left behind"
        );
    }
    assert_no_temporaries(&bench);
}

#[test]
fn three_layers_uninstalled_in_every_order_end_clean() {
    for order in ORDERS {
        run_order(order, None);
    }
}

#[test]
fn three_layers_over_an_original_end_with_the_original() {
    for order in ORDERS {
        run_order(order, Some(b"the original file"));
    }
}

#[test]
fn the_stack_links_each_layer_to_the_one_below() {
    let bench = Bench::new();
    let [a, b, c] = three_layers();
    let mut journal = bench.journal();
    a.install(&bench, &mut journal);
    b.install(&bench, &mut journal);
    let plan = plan_install(&bench.ctx(), &journal, &c.package, &c.option);
    assert_eq!(
        plan.writes[0].action,
        WriteAction::OverwriteRiftstone {
            option_name: "Layer B".into(),
            option_id: b.id()
        }
    );
    c.install(&bench, &mut journal);
    assert_eq!(journal.installs[0].files[0].below, Below::Nothing);
    assert!(matches!(
        &journal.installs[1].files[0].below,
        Below::Backup { owner: BelowOwner::Riftstone { option_id }, .. } if *option_id == a.id()
    ));
    assert!(matches!(
        &journal.installs[2].files[0].below,
        Below::Backup { owner: BelowOwner::Riftstone { option_id }, .. } if *option_id == b.id()
    ));
    let plan = plan_uninstall(&bench.ctx(), &journal, &b.id());
    assert_eq!(
        plan.removals[0].action,
        RemovalAction::HandDown {
            to_option_name: "Layer C".into(),
            to_option_id: c.id()
        }
    );
    let report = b.uninstall(&bench, &mut journal);
    assert_eq!(report.files_handed_down, 1);
    assert_eq!(
        bench.read(P).unwrap(),
        b"C",
        "a hand-down never touches the game file"
    );
    // C now holds A's bytes in its slot.
    assert!(matches!(
        &journal.installs[1].files[0].below,
        Below::Backup { owner: BelowOwner::Riftstone { option_id }, .. } if *option_id == a.id()
    ));
}

#[test]
fn a_file_changed_by_something_else_is_left_alone() {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let glow = TestMod::new("Lantern Glow Recolor").loose("natives/stm/glow.mesh", b"glow bytes");
    glow.install(&bench, &mut journal);
    // Another tool writes the same path — the SAME size, different bytes.
    bench.write("natives/stm/glow.mesh", b"GLOW BYTES");

    let plan = plan_uninstall(&bench.ctx(), &journal, &glow.id());
    assert_eq!(plan.removals[0].action, RemovalAction::KeepChanged);
    let report = glow.uninstall(&bench, &mut journal);
    assert_eq!(report.files_kept, 1);
    assert_eq!(report.files_removed, 0);
    assert_eq!(bench.read("natives/stm/glow.mesh").unwrap(), b"GLOW BYTES");
    assert!(journal.installs.is_empty());
}

#[test]
fn a_changed_file_over_a_backup_is_kept_and_the_backup_is_not_forced_back() {
    let bench = Bench::new();
    bench.write("natives/stm/glow.mesh", b"original");
    let mut journal = bench.journal();
    let glow = TestMod::new("Lantern Glow Recolor").loose("natives/stm/glow.mesh", b"glow bytes");
    glow.install(&bench, &mut journal);
    bench.write("natives/stm/glow.mesh", b"someone else's newer file");
    let report = glow.uninstall(&bench, &mut journal);
    assert_eq!(report.files_kept, 1);
    assert_eq!(
        bench.read("natives/stm/glow.mesh").unwrap(),
        b"someone else's newer file"
    );
}

#[test]
fn a_file_already_gone_is_not_an_error() {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let glow = TestMod::new("Lantern Glow Recolor")
        .loose("natives/stm/glow.mesh", b"glow")
        .loose("natives/stm/other.mesh", b"other");
    glow.install(&bench, &mut journal);
    std::fs::remove_file(bench.root().join("natives/stm/glow.mesh")).unwrap();
    let plan = plan_uninstall(&bench.ctx(), &journal, &glow.id());
    assert_eq!(plan.removals[0].action, RemovalAction::AlreadyGone);
    assert_eq!(plan.removals[1].action, RemovalAction::Delete);
    glow.uninstall(&bench, &mut journal);
    assert!(!bench.exists("natives"));
}

#[test]
fn a_missing_top_layer_retires_and_the_next_install_inherits_its_backup() {
    let bench = Bench::new();
    bench.write(P, b"the original file");
    let mut journal = bench.journal();
    let [a, b, _] = three_layers();
    a.install(&bench, &mut journal);
    // Someone deletes A's file by hand.
    std::fs::remove_file(bench.root().join(P)).unwrap();

    let plan = plan_install(&bench.ctx(), &journal, &b.package, &b.option);
    assert_eq!(plan.writes[0].action, WriteAction::Create);
    b.install(&bench, &mut journal);
    assert!(
        journal.installs[0].files.is_empty(),
        "A's claim on the path retired"
    );
    assert!(matches!(
        &journal.installs[1].files[0].below,
        Below::Backup {
            owner: BelowOwner::Unowned,
            ..
        }
    ));
    b.uninstall(&bench, &mut journal);
    assert_eq!(
        bench.read(P).unwrap(),
        b"the original file",
        "the original still comes back"
    );
    a.uninstall(&bench, &mut journal);
    assert_eq!(bench.read(P).unwrap(), b"the original file");
    assert!(bench.store_files().is_empty());
}

#[test]
fn strays_and_unknown_files_are_never_removed() {
    let bench = Bench::new();
    bench.write("natives/stm/stray.mesh", b"nobody's");
    bench.write("reframework/autorun/stray.lua", b"-- nobody's");
    bench.write("pak_mods/zz-unknown.pak", b"KPKA nobody's");
    let mut journal = bench.journal();
    let glow = TestMod::new("Lantern Glow Recolor")
        .loose("natives/stm/glow.mesh", b"glow")
        .script("reframework/autorun/glow.lua", b"-- glow")
        .pak("glow.pak", b"KPKA glow");
    glow.install(&bench, &mut journal);
    glow.uninstall(&bench, &mut journal);
    assert_eq!(bench.read("natives/stm/stray.mesh").unwrap(), b"nobody's");
    assert_eq!(
        bench.read("reframework/autorun/stray.lua").unwrap(),
        b"-- nobody's"
    );
    assert_eq!(
        bench.read("pak_mods/zz-unknown.pak").unwrap(),
        b"KPKA nobody's"
    );
    assert_eq!(bench.names_in("pak_mods"), ["zz-unknown.pak"]);
    // The vanilla files never moved.
    assert_eq!(
        bench.read("re_chunk_000.pak").unwrap(),
        b"KPKA vanilla chunk"
    );
    assert_eq!(
        bench.read("dlc/re_dlc_stm_2581000.pak").unwrap(),
        b"KPKA dlc"
    );
}

// ---------------------------------------------------------------------------
// A layer whose file something else replaced is no longer the layer below
// ---------------------------------------------------------------------------

/// Fluffy deploys over X; Y then installs over Fluffy's file. Y's backup
/// holds FLUFFY's bytes, so uninstalling X must never hand them down into
/// the bin — in either order, Fluffy's file comes back when both are gone.
#[test]
fn a_fluffy_file_deployed_over_an_install_is_never_handed_down_into_the_bin() {
    const SHARED: &str = "natives/stm/shared.mesh";
    for lower_first in [true, false] {
        let bench = Bench::new();
        let x = TestMod::new("Layer X").loose(SHARED, b"X bytes!");
        let y = TestMod::new("Layer Y").loose(SHARED, b"Y bytes!");
        let mut journal = bench.journal();
        x.install(&bench, &mut journal);
        bench.write(SHARED, b"Fluffy's ermine bytes");
        let ledger = FluffyLedger {
            game_install_path: None,
            mods: vec![fluffy_mod("Ermine-v1", "Ermine Cape", &[SHARED])],
        };
        let ctx = bench.ctx_with(Some(&ledger), false);

        let plan = plan_install(&ctx, &journal, &y.package, &y.option);
        assert_eq!(
            plan.writes[0].action,
            WriteAction::OverwriteFluffy {
                section: "Ermine-v1".into()
            },
            "the bytes at the path are Fluffy's, not Layer X's"
        );
        y.install_with(&ctx, &mut journal).unwrap();
        assert!(matches!(
            &journal.find(&y.id()).unwrap().files[0].below,
            Below::Backup { owner: BelowOwner::Fluffy { section }, .. } if section == "Ermine-v1"
        ));

        if lower_first {
            uninstall(&ctx, &mut journal, &x.id()).unwrap();
            assert_eq!(bench.read(SHARED).unwrap(), b"Y bytes!");
            uninstall(&ctx, &mut journal, &y.id()).unwrap();
        } else {
            uninstall(&ctx, &mut journal, &y.id()).unwrap();
            uninstall(&ctx, &mut journal, &x.id()).unwrap();
        }
        assert_eq!(
            bench.read(SHARED).unwrap(),
            b"Fluffy's ermine bytes",
            "lower first = {lower_first}: Fluffy's file survives"
        );
        assert!(bench.store_files().is_empty());
        assert_no_temporaries(&bench);
    }
}

/// The same with a file nobody owns: X ships a config, the player rewrites
/// it, Y installs over the rewrite. Uninstalling X must not delete the
/// player's edit out of Y's backup slot.
#[test]
fn an_edited_config_under_a_later_install_survives_uninstalling_the_first() {
    const CONFIG: &str = "reframework/data/glow/config.json";
    for lower_first in [true, false] {
        let bench = Bench::new();
        let x = TestMod::new("Glow Script").script(CONFIG, b"{\"glow\":1}");
        let y = TestMod::new("Glow Script Plus").script(CONFIG, b"{\"glow\":3,\"plus\":1}");
        let mut journal = bench.journal();
        x.install(&bench, &mut journal);
        bench.write(CONFIG, b"{\"glow\":2, \"edited\":true}");

        let plan = plan_install(&bench.ctx(), &journal, &y.package, &y.option);
        assert_eq!(plan.writes[0].action, WriteAction::OverwriteUnowned);
        y.install(&bench, &mut journal);
        if lower_first {
            x.uninstall(&bench, &mut journal);
            y.uninstall(&bench, &mut journal);
        } else {
            y.uninstall(&bench, &mut journal);
            x.uninstall(&bench, &mut journal);
        }
        assert_eq!(
            bench.read(CONFIG).unwrap(),
            b"{\"glow\":2, \"edited\":true}",
            "lower first = {lower_first}: the player's edit comes back"
        );
        assert!(bench.store_files().is_empty());
    }
}

/// A hand-down deletes the slot above only when it really holds the lower
/// layer's bytes. A slot holding anything else is kept as the upper layer's
/// own below.
#[test]
fn a_hand_down_never_deletes_a_slot_that_does_not_hold_the_lower_layers_bytes() {
    let bench = Bench::new();
    let [a, b, _] = three_layers();
    let mut journal = bench.journal();
    a.install(&bench, &mut journal);
    b.install(&bench, &mut journal);
    let Below::Backup { store, .. } = journal.installs[1].files[0].below.clone() else {
        panic!("B backs up A");
    };
    std::fs::write(bench.state().join(&store), b"not the bytes of layer A").unwrap();

    a.uninstall(&bench, &mut journal);
    assert_eq!(
        std::fs::read(bench.state().join(&store)).unwrap(),
        b"not the bytes of layer A",
        "the slot was kept"
    );
    b.uninstall(&bench, &mut journal);
    assert_eq!(
        bench.read(P).unwrap(),
        b"not the bytes of layer A",
        "B's uninstall puts back what its slot held"
    );
    assert!(bench.store_files().is_empty());
}

// ---------------------------------------------------------------------------
// An original that cannot go back is kept on record, never orphaned
// ---------------------------------------------------------------------------

#[test]
fn a_changed_file_over_a_backup_keeps_the_original_on_record() {
    let bench = Bench::new();
    bench.write("natives/stm/glow.mesh", b"original");
    let mut journal = bench.journal();
    let glow = TestMod::new("Lantern Glow Recolor").loose("natives/stm/glow.mesh", b"glow bytes");
    glow.install(&bench, &mut journal);
    let Below::Backup { store, .. } = journal.installs[0].files[0].below.clone() else {
        panic!("the original was backed up");
    };
    bench.write("natives/stm/glow.mesh", b"another tool's rewrite");

    let report = glow.uninstall(&bench, &mut journal);
    assert_eq!((report.files_kept, report.files_restored), (1, 0));
    assert_eq!(
        bench.read("natives/stm/glow.mesh").unwrap(),
        b"another tool's rewrite"
    );
    assert_eq!(
        std::fs::read(bench.state().join(&store)).unwrap(),
        b"original",
        "the original is still in the store"
    );
    assert!(
        journal_text(&bench).contains(&store),
        "…and the journal still knows where it is"
    );
    assert_eq!(report.parked.len(), 1, "the report names it");
    assert_eq!(report.parked[0].store, store);
    assert_eq!(report.parked[0].game_path, "natives/stm/glow.mesh");
    assert_eq!(report.parked[0].owner, BelowOwner::Unowned);
    assert_eq!(bench.journal().parked, report.parked, "on record on disk");

    // The game state lists it, and preflight offers to review it.
    let state = read_game_state(&bench.ctx(), &journal);
    assert_eq!(state.parked.len(), 1);
    let kept = &state.parked[0];
    assert_eq!(kept.size, Some(8));
    assert!(kept.occupied);
    assert_eq!(kept.owner, "the file that was there before");
    assert_eq!(kept.owner_kind, OwnerKind::Unowned);
    let checks = preflight(&state);
    let check = checks.iter().find(|c| c.id == "parkedBackups").unwrap();
    assert_eq!(check.level, CheckLevel::Warn);
    assert_eq!(check.action, Some(CheckAction::ReviewParked));

    // Putting it back swaps it with the rewrite, which is kept aside in turn.
    let restored = restore_parked(&bench.ctx(), &mut journal, &store).unwrap();
    assert_eq!(bench.read("natives/stm/glow.mesh").unwrap(), b"original");
    let swapped = restored.swapped_out.expect("the rewrite was kept aside");
    assert_eq!(
        std::fs::read(bench.state().join(&swapped.store)).unwrap(),
        b"another tool's rewrite"
    );
    assert_eq!(journal.parked, std::slice::from_ref(&swapped));
    assert_eq!(bench.journal(), journal);
    // …and back again.
    restore_parked(&bench.ctx(), &mut journal, &swapped.store).unwrap();
    assert_eq!(
        bench.read("natives/stm/glow.mesh").unwrap(),
        b"another tool's rewrite"
    );
    assert_eq!(journal.parked.len(), 1);
    // Discarding throws the kept copy away and touches nothing in the game.
    let kept_store = journal.parked[0].store.clone();
    discard_parked(&bench.ctx(), &mut journal, &kept_store).unwrap();
    assert!(journal.parked.is_empty() && bench.journal().parked.is_empty());
    assert!(bench.store_files().is_empty());
    assert_eq!(
        bench.read("natives/stm/glow.mesh").unwrap(),
        b"another tool's rewrite"
    );
    assert_no_temporaries(&bench);
}

#[test]
fn a_kept_file_never_goes_back_over_an_install_or_a_fluffy_file() {
    let bench = Bench::new();
    bench.write("natives/stm/glow.mesh", b"original");
    let mut journal = bench.journal();
    let glow = TestMod::new("Lantern Glow Recolor").loose("natives/stm/glow.mesh", b"glow bytes");
    glow.install(&bench, &mut journal);
    bench.write("natives/stm/glow.mesh", b"a rewrite");
    let store = glow.uninstall(&bench, &mut journal).parked[0].store.clone();

    // An install now owns the path.
    let cape = TestMod::new("Ermine Cape").loose("natives/stm/glow.mesh", b"ermine");
    cape.install(&bench, &mut journal);
    let before = bench.snapshot();
    let err = restore_parked(&bench.ctx(), &mut journal, &store).unwrap_err();
    assert!(err.to_string().contains("Ermine Cape"), "{err}");
    assert_eq!(bench.snapshot(), before);
    cape.uninstall(&bench, &mut journal);
    assert_eq!(bench.read("natives/stm/glow.mesh").unwrap(), b"a rewrite");

    // Fluffy's file sits there now.
    bench.write("natives/stm/glow.mesh", b"fluffy's glow");
    let ledger = FluffyLedger {
        game_install_path: None,
        mods: vec![fluffy_mod(
            "Glow-v1",
            "Fluffy Glow",
            &["natives/stm/glow.mesh"],
        )],
    };
    let ctx = bench.ctx_with(Some(&ledger), false);
    let err = restore_parked(&ctx, &mut journal, &store).unwrap_err();
    assert!(err.to_string().contains("Fluffy Glow"), "{err}");
    assert_eq!(
        bench.read("natives/stm/glow.mesh").unwrap(),
        b"fluffy's glow"
    );

    // Once the path is free, it simply goes back.
    std::fs::remove_file(bench.root().join("natives/stm/glow.mesh")).unwrap();
    let report = restore_parked(&ctx, &mut journal, &store).unwrap();
    assert!(report.swapped_out.is_none());
    assert_eq!(bench.read("natives/stm/glow.mesh").unwrap(), b"original");
    assert!(journal.parked.is_empty());
    assert!(bench.store_files().is_empty());
    // An unknown handle is refused plainly.
    assert!(restore_parked(&ctx, &mut journal, "backups/1/nothing").is_err());
    assert!(discard_parked(&ctx, &mut journal, "backups/1/nothing").is_err());

    // The handle the window still holds is out of date now (the file went
    // back). The refusal names the game path, never the store's inner path,
    // and sends the player to the list the window reads again.
    let before = bench.snapshot();
    for stale in [
        restore_parked(&ctx, &mut journal, &store).unwrap_err(),
        discard_parked(&ctx, &mut journal, &store).unwrap_err(),
    ] {
        let RiftError::Refused { reason } = &stale else {
            panic!("a stale handle is a refusal: {stale:?}");
        };
        assert!(reason.contains("natives/stm/glow.mesh"), "{reason}");
        assert!(!reason.contains("backups/"), "{reason}");
        assert!(reason.contains("PARKED"), "{reason}");
        assert!(!reason.contains("Reload the game state"), "{reason}");
    }
    assert_eq!(bench.snapshot(), before, "nothing moved");
}

/// An original under A, B and C, the top file edited by the player, then
/// every uninstall order: the edit stays in place, and exactly one file — the
/// original — stays in the store, on record.
#[test]
fn an_edited_top_over_three_layers_keeps_one_original_on_record_in_every_order() {
    for order in ORDERS {
        let bench = Bench::new();
        bench.write(P, b"the original file");
        let layers = three_layers();
        let mut journal = bench.journal();
        for layer in &layers {
            layer.install(&bench, &mut journal);
        }
        bench.write(P, b"EDITED BY THE PLAYER");
        for &gone in &order {
            layers[gone].uninstall(&bench, &mut journal);
        }
        assert_eq!(
            bench.read(P).unwrap(),
            b"EDITED BY THE PLAYER",
            "order {order:?}"
        );
        let store = bench.store_files();
        assert_eq!(store.len(), 1, "order {order:?}: {store:?}");
        assert_eq!(
            std::fs::read(bench.state().join("backups").join(&store[0])).unwrap(),
            b"the original file",
            "order {order:?}"
        );
        assert!(
            journal_text(&bench).contains(&format!("backups/{}", store[0])),
            "order {order:?}: the original is on record"
        );
        assert_eq!(journal.parked.len(), 1, "order {order:?}: one record");
        assert_eq!(journal.parked[0].store, format!("backups/{}", store[0]));
        assert_eq!(journal.parked[0].owner, BelowOwner::Unowned);
        assert!(journal.installs.is_empty());
        assert!(journal.cleanup.is_empty(), "order {order:?}");
        assert_eq!(bench.journal(), journal);
    }
}
