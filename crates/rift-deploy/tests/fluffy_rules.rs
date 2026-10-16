//! § rift-deploy rule 6 — Fluffy adoption — and RS-5: Fluffy is a
//! neighbour; its files are read, never written, and nothing it installed is
//! deleted until it has been adopted.

mod common;

use std::fs;

use common::{fluffy_mod, Bench, TestMod};
use rift_core::OptionId;
use rift_deploy::fluffy::parse_ledger;
use rift_deploy::{
    adopt_fluffy, plan_install, plan_uninstall, uninstall, Below, BelowOwner, DeployKind,
    FluffyLedger, FluffyMod, Origin, RemovalAction, WriteAction,
};

const INSTALLED_INI: &str = "GameInstallService=Steam\r\nGameInstallPath=C:\\Games\\Dragons Dogma 2\r\n\
[RE Framework]\r\nModName=RE Framework\r\ninstalltype=normal\r\nfile=dinput8.dll\r\n\
[Lantern Glow-v2]\r\nModName=Lantern Glow Recolor\r\nfile=pak_mods\\x0001-Lantern Glow.pak\r\n\
[Ermine-v1]\r\nModName=Ermine Cape\r\nfile=natives/stm/cape.mesh\r\nfile=reframework\\data\\ermine.json\r\nfile=natives/stm/gone.mesh\r\n\
[Root Tweak]\r\nModName=Root Tweak\r\nfile=tweak.ini\r\n\
[Older Pak]\r\nModName=Older Armour\r\nfile=pak_mods/x0000-Older Armour.pak\r\n\
[Menu Only]\r\nModName=Vernworth Cloaks Collection\r\n";

struct Neighbour {
    bench: Bench,
    ledger: FluffyLedger,
    ini_path: std::path::PathBuf,
}

/// A fake game folder Fluffy has been deploying into, and its ledger on disk.
fn neighbour() -> Neighbour {
    let bench = Bench::new();
    bench.write("dinput8.dll", b"MZ reframework");
    bench.write("pak_mods/x0001-Lantern Glow.pak", b"KPKA glow");
    bench.write("natives/stm/cape.mesh", b"fluffy's cape");
    bench.write("reframework/data/ermine.json", b"{}");
    bench.write("tweak.ini", b"[tweak]");
    bench.write("pak_mods/x0000-Older Armour.pak", b"KPKA older");
    let fluffy_dir = bench.dir.path().join("modmanager/Games/DragonsDogma2");
    fs::create_dir_all(&fluffy_dir).unwrap();
    let ini_path = fluffy_dir.join("installed.ini");
    fs::write(&ini_path, INSTALLED_INI).unwrap();
    let ledger = rift_deploy::read_ledger(&ini_path).unwrap();
    Neighbour {
        bench,
        ledger,
        ini_path,
    }
}

fn match_ermine(m: &FluffyMod) -> Option<(OptionId, String)> {
    (m.section == "Ermine-v1").then(|| {
        (
            OptionId("00000000000000ee#Ermine".into()),
            "Ermine Cape (library)".into(),
        )
    })
}

#[test]
fn adoption_records_every_file_with_the_right_below() {
    let n = neighbour();
    let ini_before = fs::read(&n.ini_path).unwrap();
    let ini_mtime = fs::metadata(&n.ini_path).unwrap().modified().unwrap();
    let game_before = n.bench.snapshot();
    let ctx = n.bench.ctx_with(Some(&n.ledger), false);
    let mut journal = n.bench.journal();

    let report = adopt_fluffy(&ctx, &mut journal, &n.ledger, &match_ermine).unwrap();
    assert_eq!(report.adopted, 5);
    assert_eq!(report.already_adopted, 0);
    assert_eq!(
        report.missing_files, 1,
        "natives/stm/gone.mesh is not on disk"
    );
    assert_eq!(
        report.unmatched.len(),
        5,
        "all but Ermine; the menu entry has no files"
    );
    assert!(report.unmatched.contains(&"Menu Only".to_string()));
    assert_eq!(n.bench.journal(), journal, "adoption saves the journal");

    let by_section = |section: &str| {
        journal
            .installs
            .iter()
            .find(|i| {
                i.origin
                    == Origin::AdoptedFromFluffy {
                        section: section.into(),
                    }
            })
            .unwrap_or_else(|| panic!("{section} adopted"))
    };
    let reframework = by_section("RE Framework");
    assert_eq!(reframework.files[0].game_path, "dinput8.dll");
    assert_eq!(
        reframework.files[0].below,
        Below::Nothing,
        "REFramework is not vanilla"
    );
    assert_eq!(reframework.files[0].kind, DeployKind::Root);
    let ermine = by_section("Ermine-v1");
    assert_eq!(ermine.option_id, OptionId("00000000000000ee#Ermine".into()));
    assert_eq!(ermine.name, "Ermine Cape (library)");
    assert_eq!(ermine.files.len(), 2);
    assert!(ermine.files.iter().all(|f| f.below == Below::Nothing));
    assert_eq!(ermine.files[1].game_path, "reframework/data/ermine.json");
    assert_eq!(ermine.files[1].kind, DeployKind::Reframework);
    let tweak = by_section("Root Tweak");
    assert_eq!(
        tweak.files[0].below,
        Below::UnknownVanilla,
        "another root file may be vanilla"
    );
    assert_eq!(tweak.option_id, OptionId("fluffy#Root Tweak".into()));
    let glow = by_section("Lantern Glow-v2");
    assert_eq!(glow.name, "Lantern Glow Recolor");
    assert_eq!(glow.files[0].kind, DeployKind::Pak);
    let older = by_section("Older Pak");
    assert_eq!(
        journal.pak_order,
        [older.option_id.clone(), glow.option_id.clone()],
        "adopted paks join in their current file-name order"
    );

    // Adoption moves nothing and never writes Fluffy's ledger.
    assert_eq!(n.bench.snapshot(), game_before);
    assert_eq!(fs::read(&n.ini_path).unwrap(), ini_before);
    assert_eq!(
        fs::metadata(&n.ini_path).unwrap().modified().unwrap(),
        ini_mtime
    );

    // Adopting again adopts nothing new.
    let again = adopt_fluffy(&ctx, &mut journal, &n.ledger, &match_ermine).unwrap();
    assert_eq!(again.adopted, 0);
    assert_eq!(again.already_adopted, 5);
}

#[test]
fn an_adopted_mod_uninstalls_only_what_it_owns() {
    let n = neighbour();
    let ctx = n.bench.ctx_with(Some(&n.ledger), false);
    let mut journal = n.bench.journal();
    adopt_fluffy(&ctx, &mut journal, &n.ledger, &match_ermine).unwrap();

    let tweak = OptionId("fluffy#Root Tweak".into());
    let plan = plan_uninstall(&ctx, &journal, &tweak);
    assert_eq!(plan.removals[0].action, RemovalAction::KeepVanilla);
    uninstall(&ctx, &mut journal, &tweak).unwrap();
    assert!(
        n.bench.exists("tweak.ini"),
        "a possibly-vanilla root file stays"
    );

    uninstall(&ctx, &mut journal, &OptionId("fluffy#RE Framework".into())).unwrap();
    assert!(!n.bench.exists("dinput8.dll"), "REFramework itself goes");

    uninstall(
        &ctx,
        &mut journal,
        &OptionId("00000000000000ee#Ermine".into()),
    )
    .unwrap();
    assert!(!n.bench.exists("natives/stm/cape.mesh"));
    assert!(!n.bench.exists("reframework/data/ermine.json"));
    assert!(fs::read(&n.ini_path).unwrap() == INSTALLED_INI.as_bytes());
}

#[test]
fn unadopted_fluffy_files_are_backed_up_never_deleted() {
    let n = neighbour();
    let ctx = n.bench.ctx_with(Some(&n.ledger), false);
    let mut journal = n.bench.journal();
    let cape = TestMod::new("Vernworth Cape").loose("natives/stm/cape.mesh", b"vernworth cape");
    cape.install_with(&ctx, &mut journal).unwrap();
    uninstall(&ctx, &mut journal, &cape.id()).unwrap();
    assert_eq!(
        n.bench.read("natives/stm/cape.mesh").unwrap(),
        b"fluffy's cape"
    );
}

#[test]
fn adoption_slides_a_fluffy_layer_under_a_riftstone_install() {
    let n = neighbour();
    let ctx = n.bench.ctx_with(Some(&n.ledger), false);
    let mut journal = n.bench.journal();
    let cape = TestMod::new("Vernworth Cape").loose("natives/stm/cape.mesh", b"vernworth cape");
    let plan = plan_install(&ctx, &journal, &cape.package, &cape.option);
    assert_eq!(
        plan.writes[0].action,
        WriteAction::OverwriteFluffy {
            section: "Ermine-v1".into()
        }
    );
    cape.install_with(&ctx, &mut journal).unwrap();

    adopt_fluffy(&ctx, &mut journal, &n.ledger, &match_ermine).unwrap();
    let ermine_id = OptionId("00000000000000ee#Ermine".into());
    // The install's backup slot now names the adopted Ermine layer…
    let cape_file = &journal.find(&cape.id()).unwrap().files[0];
    assert!(matches!(
        &cape_file.below,
        Below::Backup { owner: BelowOwner::Riftstone { option_id }, .. } if *option_id == ermine_id
    ));
    // …whose recorded bytes are Fluffy's, read from that slot.
    let ermine = journal.find(&ermine_id).unwrap();
    let ermine_cape = ermine
        .files
        .iter()
        .find(|f| f.game_path == "natives/stm/cape.mesh")
        .unwrap();
    assert_eq!(ermine_cape.size, b"fluffy's cape".len() as u64);
    assert_eq!(
        n.bench.read("natives/stm/cape.mesh").unwrap(),
        b"vernworth cape"
    );

    // Uninstalling Ermine hands down; the file on top does not change.
    let plan = plan_uninstall(&ctx, &journal, &ermine_id);
    assert!(plan
        .removals
        .iter()
        .any(|r| matches!(r.action, RemovalAction::HandDown { .. })));
    uninstall(&ctx, &mut journal, &ermine_id).unwrap();
    assert_eq!(
        n.bench.read("natives/stm/cape.mesh").unwrap(),
        b"vernworth cape"
    );
    uninstall(&ctx, &mut journal, &cape.id()).unwrap();
    assert!(!n.bench.exists("natives/stm/cape.mesh"));
    assert!(n.bench.store_files().is_empty());
}

#[test]
fn a_path_two_fluffy_mods_list_belongs_to_the_later_one() {
    let bench = Bench::new();
    bench.write("natives/stm/shared.mesh", b"the later mod's bytes");
    let ledger = FluffyLedger {
        game_install_path: None,
        mods: vec![
            fluffy_mod(
                "Early",
                "Early Mod",
                &["natives/stm/shared.mesh", "natives/stm/early.mesh"],
            ),
            fluffy_mod("Late", "Late Mod", &["natives/stm/shared.mesh"]),
        ],
    };
    bench.write("natives/stm/early.mesh", b"early only");
    let ctx = bench.ctx_with(Some(&ledger), false);
    let mut journal = bench.journal();
    let report = adopt_fluffy(&ctx, &mut journal, &ledger, &|_| None).unwrap();
    assert_eq!(report.adopted, 2);
    assert_eq!(report.shared_files, 1);
    let early = journal.find(&OptionId("fluffy#Early".into())).unwrap();
    assert_eq!(early.files.len(), 1);
    assert_eq!(early.files[0].game_path, "natives/stm/early.mesh");
    let late = journal.find(&OptionId("fluffy#Late".into())).unwrap();
    assert_eq!(late.files[0].game_path, "natives/stm/shared.mesh");
}

#[test]
fn adoption_never_takes_paths_outside_the_mod_places() {
    let bench = Bench::new();
    bench.write("natives/stm/fine.mesh", b"fine");
    let ledger = parse_ledger(
        b"[Old Patch Style]\nModName=Old Patch Style\nfile=re_chunk_000.pak.patch_001.pak\nfile=steam_api64.dll\nfile=natives/stm/fine.mesh\n",
    );
    let ctx = bench.ctx_with(Some(&ledger), false);
    let mut journal = bench.journal();
    let report = adopt_fluffy(&ctx, &mut journal, &ledger, &|_| None).unwrap();
    assert_eq!(report.adopted, 1);
    assert_eq!(
        report.refused_files, 2,
        "vanilla files are never adopted, so never deleted"
    );
    assert_eq!(journal.installs[0].files.len(), 1);
}

#[test]
fn a_file_fluffy_lists_twice_in_two_spellings_is_adopted_once() {
    let bench = Bench::new();
    bench.write("natives/stm/twice.mesh", b"twice");
    let ledger = FluffyLedger {
        game_install_path: None,
        mods: vec![fluffy_mod(
            "Twice",
            "Twice Listed",
            &["natives/stm/twice.mesh", "natives/STM/twice.mesh"],
        )],
    };
    let ctx = bench.ctx_with(Some(&ledger), false);
    let mut journal = bench.journal();
    adopt_fluffy(&ctx, &mut journal, &ledger, &|_| None).unwrap();
    let twice = OptionId("fluffy#Twice".into());
    assert_eq!(journal.find(&twice).unwrap().files.len(), 1);
    uninstall(&ctx, &mut journal, &twice).expect("the adopted mod uninstalls");
    assert!(!bench.exists("natives/stm/twice.mesh"));

    // The ledger reader keeps one spelling too.
    let parsed =
        parse_ledger(b"[Twice]\nfile=natives/stm/twice.mesh\nfile=natives\\STM\\twice.mesh\n");
    assert_eq!(parsed.mods[0].files, ["natives/stm/twice.mesh"]);
}

#[cfg(unix)]
#[test]
fn an_adoption_whose_save_fails_changes_nothing_in_memory() {
    if !common::permissions_bite() {
        return;
    }
    let n = neighbour();
    let ctx = n.bench.ctx_with(Some(&n.ledger), false);
    let mut journal = n.bench.journal();
    let before = journal.clone();
    common::set_read_only(n.bench.state(), true, false);
    let result = adopt_fluffy(&ctx, &mut journal, &n.ledger, &match_ermine);
    common::set_read_only(n.bench.state(), false, false);
    assert!(result.is_err());
    assert_eq!(journal, before, "the journal in memory is the one on disk");
}
