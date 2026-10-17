//! Adopted installs — installs the journal took over from files already in
//! the game folder (`Origin::Adopted`): no package, no entries, often no
//! fingerprints. Nothing adopts any more; these journals are seeded by hand,
//! the way an older build left them. Each one must follow every rule a
//! Riftstone install follows: it uninstalls only what it recorded, keeps a
//! root file that may be the game's own, hands its backup up to the layer
//! above, and its paks take `zrift-` names in a reorder.

mod common;

use common::{adopted, assert_no_temporaries, Bench, TestMod};
use rift_core::OptionId;
use rift_deploy::{
    plan_install, plan_uninstall, read_game_state, reorder_paks, uninstall, Below, BelowOwner,
    DeployKind, Journal, Origin, OwnerKind, PakOwner, PlanWarning, RemovalAction, WriteAction,
};

const CAPE: &str = "natives/stm/cape.mesh";

/// A game folder holding what three adopted installs recorded, and their
/// journal, saved: a pak, a mod with loose files and a script, and a root
/// tweak that may be the game's own file.
fn seeded() -> (Bench, Journal) {
    let bench = Bench::new();
    bench.write("pak_mods/x0000-Older Armour.pak", b"KPKA older");
    bench.write(CAPE, b"adopted cape");
    bench.write("reframework/data/ermine.json", b"{}");
    bench.write("tweak.ini", b"[tweak]");
    let mut journal = bench.journal();
    let older = adopted(
        &mut journal,
        "adopted#Older Armour-v1",
        "Older Armour",
        "Older Armour-v1",
        &[(
            "pak_mods/x0000-Older Armour.pak",
            DeployKind::Pak,
            10,
            Below::Nothing,
        )],
    );
    let ermine = adopted(
        &mut journal,
        "0123456789abcdef#Ermine",
        "Ermine Cape",
        "Ermine-v1",
        &[
            (CAPE, DeployKind::Loose, 12, Below::Nothing),
            (
                "reframework/data/ermine.json",
                DeployKind::Reframework,
                2,
                Below::Nothing,
            ),
        ],
    );
    let tweak = adopted(
        &mut journal,
        "adopted#Root Tweak",
        "Root Tweak",
        "Root Tweak",
        &[("tweak.ini", DeployKind::Root, 7, Below::UnknownVanilla)],
    );
    journal.pak_order.push(older.option_id.clone());
    journal.installs.extend([older, ermine, tweak]);
    journal.save(bench.state()).unwrap();
    (bench, journal)
}

fn id(text: &str) -> OptionId {
    OptionId(text.into())
}

#[test]
fn a_hand_seeded_journal_of_adopted_installs_loads_and_reads_as_adopted() {
    let (bench, journal) = seeded();
    let loaded = bench.journal();
    assert_eq!(loaded, journal, "the journal on disk is the one seeded");
    assert!(loaded.installs.iter().all(|i| i.is_adopted()));
    assert_eq!(
        loaded.find(&id("adopted#Older Armour-v1")).unwrap().origin,
        Origin::Adopted {
            label: "Older Armour-v1".into()
        }
    );

    let state = read_game_state(&bench.ctx(), &loaded);
    assert_eq!(state.installs.len(), 3);
    assert!(state
        .installs
        .iter()
        .all(|i| matches!(i.origin, Origin::Adopted { .. })));
    assert_eq!(state.paks.len(), 1);
    assert!(matches!(
        &state.paks[0].owner,
        PakOwner::Riftstone { option_id, adopted: true, .. } if *option_id == id("adopted#Older Armour-v1")
    ));
    assert!(state.strays.is_empty(), "{:?}", state.strays);
    assert!(state.missing.is_empty(), "{:?}", state.missing);
    let wire = serde_json::to_value(&state).unwrap();
    assert_eq!(
        wire["installs"][0]["origin"],
        serde_json::json!({ "kind": "adopted", "label": "Older Armour-v1" })
    );
}

#[test]
fn an_adopted_install_keeps_a_root_file_that_may_be_vanilla_and_deletes_the_rest() {
    let (bench, mut journal) = seeded();
    let tweak = id("adopted#Root Tweak");
    let plan = plan_uninstall(&bench.ctx(), &journal, &tweak);
    assert_eq!(plan.refused, None);
    assert_eq!(plan.removals.len(), 1);
    assert_eq!(plan.removals[0].action, RemovalAction::KeepVanilla);
    let report = uninstall(&bench.ctx(), &mut journal, &tweak).unwrap();
    assert_eq!((report.files_removed, report.files_kept), (0, 1));
    assert_eq!(
        bench.read("tweak.ini").unwrap(),
        b"[tweak]",
        "a root file that may be the game's own stays"
    );

    let ermine = id("0123456789abcdef#Ermine");
    let plan = plan_uninstall(&bench.ctx(), &journal, &ermine);
    assert!(plan
        .removals
        .iter()
        .all(|r| r.action == RemovalAction::Delete));
    uninstall(&bench.ctx(), &mut journal, &ermine).unwrap();
    assert!(!bench.exists(CAPE));
    assert!(!bench.exists("reframework/data/ermine.json"));

    uninstall(&bench.ctx(), &mut journal, &id("adopted#Older Armour-v1")).unwrap();
    assert!(!bench.exists("pak_mods/x0000-Older Armour.pak"));
    assert!(journal.installs.is_empty() && journal.pak_order.is_empty());
    assert_eq!(bench.journal(), journal);
    assert!(bench.store_files().is_empty());
    assert_no_temporaries(&bench);
}

#[test]
fn an_adopted_file_a_riftstone_install_overwrites_is_backed_up_under_that_install() {
    let (bench, mut journal) = seeded();
    let ermine = id("0123456789abcdef#Ermine");
    let cape = TestMod::new("Vernworth Cape").loose(CAPE, b"vernworth cape");
    let plan = plan_install(&bench.ctx(), &journal, &cape.package, &cape.option);
    assert_eq!(
        plan.writes[0].action,
        WriteAction::OverwriteRiftstone {
            option_name: "Ermine Cape".into(),
            option_id: ermine.clone(),
        }
    );
    assert!(plan.warnings.contains(&PlanWarning::Conflicts {
        option_names: vec!["Ermine Cape".into()],
        paths: 1
    }));
    cape.install(&bench, &mut journal);
    assert!(matches!(
        &journal.find(&cape.id()).unwrap().files[0].below,
        Below::Backup { owner: BelowOwner::Riftstone { option_id }, .. } if *option_id == ermine
    ));

    let plan = plan_uninstall(&bench.ctx(), &journal, &cape.id());
    assert_eq!(
        plan.removals[0].action,
        RemovalAction::RestoreBackup {
            owner: "Ermine Cape".into(),
            owner_kind: OwnerKind::Adopted,
        },
        "the backup is named for the adopted install it belongs to"
    );
    cape.uninstall(&bench, &mut journal);
    assert_eq!(bench.read(CAPE).unwrap(), b"adopted cape");
    assert!(bench.store_files().is_empty());
}

#[test]
fn an_adopted_layer_hands_down_to_a_riftstone_layer_above_in_either_order() {
    for lower_first in [true, false] {
        let (bench, mut journal) = seeded();
        let ermine = id("0123456789abcdef#Ermine");
        let cape = TestMod::new("Vernworth Cape").loose(CAPE, b"vernworth cape");
        cape.install(&bench, &mut journal);
        if lower_first {
            let plan = plan_uninstall(&bench.ctx(), &journal, &ermine);
            let cape_removal = plan.removals.iter().find(|r| r.game_path == CAPE).unwrap();
            assert_eq!(
                cape_removal.action,
                RemovalAction::HandDown {
                    to_option_name: "Vernworth Cape".into(),
                    to_option_id: cape.id(),
                }
            );
            let report = uninstall(&bench.ctx(), &mut journal, &ermine).unwrap();
            assert_eq!(report.files_handed_down, 1);
            assert_eq!(
                bench.read(CAPE).unwrap(),
                b"vernworth cape",
                "the file on top does not change"
            );
            assert_eq!(
                journal.find(&cape.id()).unwrap().files[0].below,
                Below::Nothing,
                "the layer above inherits what was below the adopted one"
            );
            cape.uninstall(&bench, &mut journal);
        } else {
            cape.uninstall(&bench, &mut journal);
            assert_eq!(bench.read(CAPE).unwrap(), b"adopted cape");
            uninstall(&bench.ctx(), &mut journal, &ermine).unwrap();
        }
        assert!(!bench.exists(CAPE), "lower first = {lower_first}");
        assert!(
            bench.store_files().is_empty(),
            "lower first = {lower_first}"
        );
        assert!(journal.cleanup.is_empty() && journal.parked.is_empty());
        assert_no_temporaries(&bench);
    }
}

#[test]
fn a_backup_kept_aside_for_an_adopted_install_says_whose_it_is() {
    let (bench, mut journal) = seeded();
    let cape = TestMod::new("Vernworth Cape").loose(CAPE, b"vernworth cape");
    cape.install(&bench, &mut journal);
    // Something else rewrites the file on top: the adopted bytes below cannot
    // go back over it, so they are kept aside — still the adopted install's.
    bench.write(CAPE, b"a rewrite by another tool");
    let report = cape.uninstall(&bench, &mut journal);
    assert_eq!(report.parked.len(), 1);
    let state = read_game_state(&bench.ctx(), &journal);
    assert_eq!(state.parked.len(), 1);
    assert_eq!(state.parked[0].owner, "Ermine Cape");
    assert_eq!(state.parked[0].owner_kind, OwnerKind::Adopted);
    assert_eq!(
        serde_json::to_value(&state.parked[0]).unwrap()["ownerKind"],
        "adopted"
    );
    assert_eq!(bench.read(CAPE).unwrap(), b"a rewrite by another tool");
}

#[test]
fn a_path_an_adopted_install_lists_twice_in_two_spellings_uninstalls_once() {
    let bench = Bench::new();
    bench.write("natives/stm/twice.mesh", b"twice");
    let mut journal = bench.journal();
    let twice = adopted(
        &mut journal,
        "adopted#Twice",
        "Twice Listed",
        "Twice",
        &[
            (
                "natives/stm/twice.mesh",
                DeployKind::Loose,
                5,
                Below::Nothing,
            ),
            (
                "natives/STM/twice.mesh",
                DeployKind::Loose,
                5,
                Below::Nothing,
            ),
        ],
    );
    let twice_id = twice.option_id.clone();
    journal.installs.push(twice);
    journal.save(bench.state()).unwrap();

    let plan = plan_uninstall(&bench.ctx(), &journal, &twice_id);
    assert_eq!(plan.refused, None);
    assert_eq!(plan.removals.len(), 1, "one file, however many spellings");
    let report = uninstall(&bench.ctx(), &mut journal, &twice_id).expect("it uninstalls");
    assert_eq!(report.files_removed, 1);
    assert!(!bench.exists("natives/stm/twice.mesh"));
    assert!(journal.installs.is_empty());
    assert_no_temporaries(&bench);
}

#[test]
fn a_reorder_gives_adopted_numbered_paks_zrift_names() {
    let bench = Bench::new();
    bench.write("pak_mods/x0000-Older Armour.pak", b"KPKA older");
    bench.write("pak_mods/x0001-Lantern Glow.pak", b"KPKA glow");
    let mut journal = bench.journal();
    let older = adopted(
        &mut journal,
        "adopted#Older",
        "Older Armour",
        "Older",
        &[(
            "pak_mods/x0000-Older Armour.pak",
            DeployKind::Pak,
            10,
            Below::Nothing,
        )],
    );
    let glow = adopted(
        &mut journal,
        "adopted#Lantern Glow-v2",
        "Lantern Glow Recolor",
        "Lantern Glow-v2",
        &[(
            "pak_mods/x0001-Lantern Glow.pak",
            DeployKind::Pak,
            9,
            Below::Nothing,
        )],
    );
    let (older_id, glow_id) = (older.option_id.clone(), glow.option_id.clone());
    journal.installs.extend([older, glow]);
    let (older, glow) = (older_id, glow_id);
    // The pak order is the order the names load in: x0000 before x0001.
    journal.pak_order = vec![older.clone(), glow.clone()];
    journal.save(bench.state()).unwrap();

    let newest = TestMod::new("Vernworth Cloaks").pak("cloaks.pak", b"KPKA cloaks");
    newest.install(&bench, &mut journal);
    assert!(
        bench.exists("pak_mods/zrift-0002-cloaks.pak"),
        "after the adopted pair"
    );

    reorder_paks(
        &bench.ctx(),
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
        "the numbered x-prefix is stripped from the stem"
    );
    assert_eq!(
        bench.read("pak_mods/zrift-0002-Older Armour.pak").unwrap(),
        b"KPKA older"
    );
    assert_eq!(
        journal.pak_order,
        [newest.id(), glow.clone(), older.clone()]
    );
    assert_eq!(
        journal.find(&older).unwrap().files[0].game_path,
        "pak_mods/zrift-0002-Older Armour.pak"
    );
    assert_eq!(bench.journal(), journal);

    // Renamed, an adopted pak still uninstalls cleanly.
    uninstall(&bench.ctx(), &mut journal, &older).unwrap();
    assert!(!bench.exists("pak_mods/zrift-0002-Older Armour.pak"));
    assert_no_temporaries(&bench);
}
