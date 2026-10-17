//! The data older builds left behind keeps working (WINDOW-CONTRACT § v0.3
//! Legacy data): a schema-1 journal is lifted in memory as it loads and never
//! saved at load; installs take new ids in one save (`rekey`); and the relink
//! matcher finds the library option an install whose package left the mods
//! folder came from.
//!
//! The schema-1 journals below are written by hand. Their legacy kinds are
//! INVENTED (`adoptedFromElsewhere`, `takenOverByHand`, owner `elsewhere`) —
//! the lift recognises only the kinds that stay, so any other spelling must
//! lift the same way.

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use common::{adopted, assert_no_temporaries, Bench, TestMod};
use rift_core::{
    DeployTarget, ModOption, OptionId, PackageId, PayloadFile, PayloadKinds, RiftError,
};
use rift_deploy::relink::propose;
use rift_deploy::{
    journal_path, read_game_state, rekey, rekey_with_files, uninstall, Below, BelowOwner,
    DeployKind, DeployedFile, InstalledOption, Intent, IntentAction, IntentStep, Journal, Origin,
    PakOwner, ParkedBackup, JOURNAL_SCHEMA,
};
use serde_json::{json, Value};

fn id(text: &str) -> OptionId {
    OptionId(text.into())
}

// ---------------------------------------------------------------------------
// The lift
// ---------------------------------------------------------------------------

/// A schema-1 install as an older build wrote it.
fn install_json(
    seq: u64,
    option_id: &str,
    name: &str,
    root: &str,
    origin: Value,
    files: Value,
) -> Value {
    json!({
        "seq": seq,
        "optionId": option_id,
        "name": name,
        "packageFile": "",
        "packagePath": "",
        "packageSize": 0,
        "optionRoot": root,
        "version": null,
        "installedAt": "2026-10-01T08:00:00.000Z",
        "gameBuild": "25290080",
        "origin": origin,
        "files": files,
    })
}

fn file_json(game_path: &str, kind: &str, size: u64, below: Value) -> Value {
    json!({ "gamePath": game_path, "kind": kind, "size": size, "entry": null, "below": below })
}

/// The lived-in schema-1 journal of `game_root`: a Riftstone install whose
/// backups belong to another tool's mod and to an adopted stand-in, three
/// adopted stand-ins (two of them sharing a root), the pak order, two kept
/// files, and an install cut short whose steps name owners too.
fn schema_1(game_root: &Path) -> Value {
    let nothing = json!({ "kind": "nothing" });
    json!({
        "schema": 1,
        "gameRoot": game_root,
        "installs": [
            {
                "seq": 1,
                "optionId": "0000000000000a01#Glow",
                "name": "Lantern Glow Recolor",
                "packageFile": "Lantern Glow Recolor.zip",
                "packagePath": "/mods/Lantern Glow Recolor.zip",
                "packageSize": 1234,
                "optionRoot": "Glow",
                "version": "1.2",
                "installedAt": "2026-10-16T10:00:00.000Z",
                "gameBuild": "25290080",
                "origin": { "kind": "riftstone" },
                "files": [
                    { "gamePath": "natives/stm/glow.mesh", "kind": "loose", "size": 4,
                      "entry": "Glow/natives/stm/glow.mesh",
                      "below": { "kind": "backup", "store": "backups/1/natives/stm/glow.mesh",
                                 "owner": { "kind": "elsewhere", "section": "Ermine-v1" } } },
                    { "gamePath": "natives/stm/armour.mesh", "kind": "loose", "size": 4,
                      "entry": "Glow/natives/stm/armour.mesh",
                      "below": { "kind": "backup", "store": "backups/1/natives/stm/armour.mesh",
                                 "owner": { "kind": "riftstone", "optionId": "elsewhere#Older Armour-v1" } } }
                ]
            },
            install_json(2, "elsewhere#Older Armour-v1", "Older Armour", "Older Armour-v1",
                json!({ "kind": "adoptedFromElsewhere", "section": "Older Armour-v1" }),
                json!([
                    file_json("pak_mods/x0000-Older Armour.pak", "pak", 10, nothing.clone()),
                    file_json("natives/stm/armour.mesh", "loose", 6, nothing.clone()),
                ])),
            install_json(3, "elsewhere#Twice", "Twice Listed", "Twice",
                json!({ "kind": "adoptedFromElsewhere" }),
                json!([file_json("natives/stm/twice.mesh", "loose", 5, nothing.clone())])),
            install_json(4, "another#Twice", "Twice Again", "Twice",
                json!({ "kind": "takenOverByHand", "section": "Twice" }),
                json!([file_json("reframework/autorun/twice.lua", "reframework", 2, nothing)])),
        ],
        "pakOrder": ["elsewhere#Older Armour-v1"],
        "pending": {
            "action": "install",
            "seq": 9,
            "optionId": "0000000000000a02#Cloaks",
            "touched": ["natives/stm/x.mesh", "natives/stm/y.mesh"],
            "startedAt": "2026-10-16T11:00:00.000Z",
            "optionName": "Vernworth Cloaks Collection",
            "steps": [
                { "kind": "place", "gamePath": "natives/stm/x.mesh", "size": 1,
                  "backup": "backups/9/natives/stm/x.mesh",
                  "owner": { "kind": "elsewhere", "section": "Ermine-v1" } },
                { "kind": "place", "gamePath": "natives/stm/y.mesh", "size": 1,
                  "backup": "backups/9/natives/stm/y.mesh",
                  "owner": { "kind": "riftstone", "optionId": "another#Twice" } },
                { "kind": "place", "gamePath": "natives/stm/z.mesh", "size": 1, "backup": null }
            ],
            "phase": 0
        },
        "nextSeq": 10,
        "cleanup": [],
        "parked": [
            { "store": "backups/5/natives/stm/kept.mesh", "gamePath": "natives/stm/kept.mesh",
              "owner": { "kind": "elsewhere", "section": "Ermine-v1" },
              "parkedAt": "2026-10-15T09:00:00.000Z", "reason": "Kept aside." },
            { "store": "backups/6/natives/stm/glow.mesh", "gamePath": "natives/stm/glow.mesh",
              "owner": { "kind": "riftstone", "optionId": "elsewhere#Twice" },
              "parkedAt": "2026-10-15T09:00:00.000Z", "reason": "Kept aside." }
        ],
        "createdDirs": []
    })
}

fn write_raw(state: &Path, game_root: &Path, raw: &Value) -> Vec<u8> {
    let bytes = serde_json::to_vec_pretty(raw).unwrap();
    fs::create_dir_all(state).unwrap();
    fs::write(journal_path(state, game_root), &bytes).unwrap();
    bytes
}

#[test]
fn a_schema_1_journal_is_lifted_in_memory_and_never_saved_at_load() {
    let state = tempfile::tempdir().unwrap();
    let root = Path::new("/games/Dragons Dogma 2");
    let written = write_raw(state.path(), root, &schema_1(root));

    let journal = Journal::load(state.path(), root).unwrap();
    assert_eq!(journal.schema, JOURNAL_SCHEMA);
    assert_eq!(JOURNAL_SCHEMA, 2);
    assert_eq!(
        fs::read(journal_path(state.path(), root)).unwrap(),
        written,
        "loading never writes"
    );

    // Origins: the Riftstone's stays; any other kind is adopted, labelled
    // with its section, else the install's name.
    let origins: Vec<&Origin> = journal.installs.iter().map(|i| &i.origin).collect();
    assert_eq!(
        origins,
        [
            &Origin::Riftstone,
            &Origin::Adopted {
                label: "Older Armour-v1".into()
            },
            &Origin::Adopted {
                label: "Twice Listed".into()
            },
            &Origin::Adopted {
                label: "Twice".into()
            },
        ]
    );
    // Stand-in ids: adopted#<root half>; the second "Twice" counts up. A
    // real id stays as it is.
    let ids: Vec<&str> = journal
        .installs
        .iter()
        .map(|i| i.option_id.0.as_str())
        .collect();
    assert_eq!(
        ids,
        [
            "0000000000000a01#Glow",
            "adopted#Older Armour-v1",
            "adopted#Twice",
            "adopted#Twice#2"
        ]
    );
    assert_eq!(journal.pak_order, [id("adopted#Older Armour-v1")]);
    // Owners: any other kind is unowned; a stand-in owner follows its install.
    let owners: Vec<&Below> = journal.installs[0].files.iter().map(|f| &f.below).collect();
    assert_eq!(
        owners,
        [
            &Below::Backup {
                store: "backups/1/natives/stm/glow.mesh".into(),
                owner: BelowOwner::Unowned
            },
            &Below::Backup {
                store: "backups/1/natives/stm/armour.mesh".into(),
                owner: BelowOwner::Riftstone {
                    option_id: id("adopted#Older Armour-v1")
                }
            },
        ]
    );
    let kept: Vec<&BelowOwner> = journal.parked.iter().map(|k| &k.owner).collect();
    assert_eq!(
        kept,
        [
            &BelowOwner::Unowned,
            &BelowOwner::Riftstone {
                option_id: id("adopted#Twice")
            }
        ]
    );
    let pending = journal.pending.as_ref().unwrap();
    assert_eq!(pending.option_id, id("0000000000000a02#Cloaks"));
    let step_owners: Vec<Option<&BelowOwner>> = pending
        .steps
        .iter()
        .map(|step| match step {
            IntentStep::Place { owner, .. } => owner.as_ref(),
            _ => None,
        })
        .collect();
    assert_eq!(
        step_owners,
        [
            Some(&BelowOwner::Unowned),
            Some(&BelowOwner::Riftstone {
                option_id: id("adopted#Twice#2")
            }),
            None
        ]
    );

    // The next save writes schema 2, and it loads back as it is: no second
    // lift, no legacy kind or stand-in spelling left anywhere.
    journal.save(state.path()).unwrap();
    let text = fs::read_to_string(journal_path(state.path(), root)).unwrap();
    let saved: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(saved["schema"], 2);
    for legacy in [
        "elsewhere",
        "adoptedFromElsewhere",
        "takenOverByHand",
        "another#",
    ] {
        assert!(!text.contains(legacy), "{legacy} survived the lift");
    }
    assert_eq!(Journal::load(state.path(), root).unwrap(), journal);
}

#[test]
fn the_lift_follows_a_stand_in_into_the_pending_intent_and_leaves_an_empty_id_alone() {
    let state = tempfile::tempdir().unwrap();
    let root = Path::new("/g");
    let mut raw = schema_1(root);
    raw["pending"] = json!({
        "action": "uninstall", "seq": 9, "optionId": "elsewhere#Twice",
        "touched": ["natives/stm/twice.mesh"], "startedAt": "2026-10-16T11:00:00.000Z",
        "optionName": "Twice Listed",
        "steps": [{ "kind": "remove", "gamePath": "natives/stm/twice.mesh",
                    "trash": "natives/stm/twice.mesh.riftstone-trash-9" }]
    });
    write_raw(state.path(), root, &raw);
    let journal = Journal::load(state.path(), root).unwrap();
    assert_eq!(journal.pending.unwrap().option_id, id("adopted#Twice"));

    raw["pending"] = json!({
        "action": "reorder", "seq": 9, "optionId": "",
        "touched": [], "startedAt": "2026-10-16T11:00:00.000Z", "steps": [], "phase": 1
    });
    write_raw(state.path(), root, &raw);
    let journal = Journal::load(state.path(), root).unwrap();
    assert_eq!(
        journal.pending.unwrap().option_id,
        id(""),
        "a reorder names no option"
    );
}

#[test]
fn a_schema_1_journal_the_lift_cannot_read_is_damaged_and_left_untouched() {
    let state = tempfile::tempdir().unwrap();
    let root = Path::new("/g");
    for broken in [json!({ "section": "no kind at all" }), json!("riftstone")] {
        let mut raw = schema_1(root);
        raw["installs"][1]["origin"] = broken;
        let written = write_raw(state.path(), root, &raw);
        let err = Journal::load(state.path(), root).unwrap_err();
        assert!(
            matches!(&err, RiftError::Journal { detail } if detail.contains("damaged")),
            "{err}"
        );
        assert_eq!(fs::read(journal_path(state.path(), root)).unwrap(), written);
    }
}

#[test]
fn a_lifted_adopted_install_uninstalls_and_the_next_save_writes_schema_2() {
    let bench = Bench::new();
    bench.write("pak_mods/x0000-Older Armour.pak", b"KPKA older");
    bench.write("natives/stm/armour.mesh", b"armour");
    let raw = json!({
        "schema": 1,
        "gameRoot": bench.root(),
        "installs": [install_json(1, "elsewhere#Older Armour-v1", "Older Armour", "Older Armour-v1",
            json!({ "kind": "adoptedFromElsewhere", "section": "Older Armour-v1" }),
            json!([
                file_json("pak_mods/x0000-Older Armour.pak", "pak", 10, json!({ "kind": "nothing" })),
                file_json("natives/stm/armour.mesh", "loose", 6, json!({ "kind": "nothing" })),
            ]))],
        "pakOrder": ["elsewhere#Older Armour-v1"],
        "pending": null,
        "nextSeq": 2
    });
    write_raw(bench.state(), bench.root(), &raw);
    let mut journal = bench.journal();
    let older = id("adopted#Older Armour-v1");
    let state = read_game_state(&bench.ctx(), &journal);
    assert!(matches!(
        &state.paks[0].owner,
        PakOwner::Riftstone { option_id, adopted: true, .. } if *option_id == older
    ));

    let report = uninstall(&bench.ctx(), &mut journal, &older).unwrap();
    assert_eq!(report.files_removed, 2);
    assert!(!bench.exists("pak_mods/x0000-Older Armour.pak"));
    assert!(!bench.exists("natives/stm/armour.mesh"));
    let saved: Value =
        serde_json::from_slice(&fs::read(journal_path(bench.state(), bench.root())).unwrap())
            .unwrap();
    assert_eq!(saved["schema"], 2, "the uninstall's save wrote schema 2");
    assert_eq!(bench.journal(), journal);
    assert_no_temporaries(&bench);
}

// ---------------------------------------------------------------------------
// Re-keying installs
// ---------------------------------------------------------------------------

const SHARED: &str = "natives/stm/shared.mesh";

/// A, with a pak, sits under B at one path; a kept file names A as its
/// owner too.
fn layered() -> (Bench, Journal, TestMod, TestMod) {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let a = TestMod::new("Lantern Glow Recolor")
        .loose(SHARED, b"glow layer")
        .pak("glow.pak", b"KPKA glow");
    let b = TestMod::new("Vernworth Cloaks").loose(SHARED, b"cloaks layer");
    a.install(&bench, &mut journal);
    b.install(&bench, &mut journal);
    journal.parked.push(ParkedBackup {
        store: "backups/90/natives/stm/kept.mesh".into(),
        game_path: "natives/stm/kept.mesh".into(),
        owner: BelowOwner::Riftstone { option_id: a.id() },
        parked_at: "2026-10-16T12:00:00.000Z".into(),
        reason: "Kept aside.".into(),
    });
    journal.save(bench.state()).unwrap();
    (bench, journal, a, b)
}

#[test]
fn rekey_moves_an_install_and_every_mention_of_it_in_one_save() {
    let (bench, mut journal, a, b) = layered();
    let moved = OptionId::new(
        &PackageId::for_folder("Lantern Glow Recolor"),
        "Lantern Glow Recolor",
    );
    let map = BTreeMap::from([(a.id(), moved.clone())]);
    let files = BTreeMap::from([(moved.clone(), "Lantern Glow Recolor".to_string())]);

    let rekeyed = rekey_with_files(&bench.ctx(), &mut journal, &map, &files).unwrap();
    assert_eq!(rekeyed, 1);
    assert!(journal.find(&a.id()).is_none() && !journal.holds(&a.id()));
    let install = journal.find(&moved).unwrap();
    assert_eq!(install.package_file, "Lantern Glow Recolor");
    assert_eq!(install.origin, Origin::Riftstone);
    assert_eq!(journal.pak_order, std::slice::from_ref(&moved));
    assert!(matches!(
        &journal.find(&b.id()).unwrap().files[0].below,
        Below::Backup { owner: BelowOwner::Riftstone { option_id }, .. } if *option_id == moved
    ));
    assert_eq!(
        journal.parked[0].owner,
        BelowOwner::Riftstone {
            option_id: moved.clone()
        }
    );
    assert_eq!(bench.journal(), journal, "saved");

    // The stack still holds: uninstalling the re-keyed layer hands down to B.
    let report = uninstall(&bench.ctx(), &mut journal, &moved).unwrap();
    assert_eq!(report.files_handed_down, 1);
    assert_eq!(bench.read(SHARED).unwrap(), b"cloaks layer");
    b.uninstall(&bench, &mut journal);
    assert!(!bench.exists(SHARED));
}

#[test]
fn rekey_without_files_keeps_the_package_file() {
    let (bench, mut journal, a, _) = layered();
    let moved = id("0000000000000b01#Lantern Glow Recolor");
    let mut copy = journal.clone();
    assert_eq!(
        copy.rekey(&BTreeMap::from([(a.id(), moved.clone())])),
        Ok(1)
    );
    assert_eq!(
        copy.find(&moved).unwrap().package_file,
        "Lantern Glow Recolor.zip"
    );
    let n = rekey(
        &bench.ctx(),
        &mut journal,
        &BTreeMap::from([(a.id(), moved.clone())]),
    )
    .unwrap();
    assert_eq!(n, 1);
    assert_eq!(journal, copy);
}

#[test]
fn rekey_is_refused_while_a_change_is_pending_and_nothing_changes() {
    let (bench, mut journal, a, _) = layered();
    journal.pending = Some(Intent {
        action: IntentAction::Uninstall,
        seq: journal.next_seq,
        option_id: a.id(),
        touched: Vec::new(),
        started_at: "2026-10-16T12:00:00.000Z".into(),
        option_name: "Lantern Glow Recolor".into(),
        steps: Vec::new(),
        phase: 0,
        dirs: Vec::new(),
    });
    journal.save(bench.state()).unwrap();
    let before = journal.clone();
    let on_disk = fs::read(journal_path(bench.state(), bench.root())).unwrap();
    let map = BTreeMap::from([(a.id(), id("0000000000000b01#Glow"))]);

    let err = rekey(&bench.ctx(), &mut journal, &map).unwrap_err();
    assert!(matches!(err, RiftError::Refused { .. }), "{err}");
    let mut copy = journal.clone();
    assert!(matches!(copy.rekey(&map), Err(RiftError::Refused { .. })));
    assert_eq!(copy, before);
    assert_eq!(journal, before);
    assert_eq!(
        fs::read(journal_path(bench.state(), bench.root())).unwrap(),
        on_disk
    );
}

#[test]
fn rekey_is_refused_onto_an_id_the_journal_holds_and_nothing_changes() {
    let (bench, mut journal, a, b) = layered();
    let before = journal.clone();
    let on_disk = fs::read(journal_path(bench.state(), bench.root())).unwrap();
    // An install's id; two installs into one; an id held only as an owner.
    let dangling = id("0000000000000dd1#Gone");
    journal.parked[0].owner = BelowOwner::Riftstone {
        option_id: dangling.clone(),
    };
    let held_only_as_owner = journal.clone();
    for (map, expected) in [
        (BTreeMap::from([(a.id(), b.id())]), &before),
        (
            BTreeMap::from([
                (a.id(), id("0000000000000b01#One")),
                (b.id(), id("0000000000000b01#One")),
            ]),
            &before,
        ),
        (
            BTreeMap::from([(a.id(), dangling.clone())]),
            &held_only_as_owner,
        ),
    ] {
        let mut working = expected.clone();
        let err = rekey(&bench.ctx(), &mut working, &map).unwrap_err();
        assert!(matches!(err, RiftError::Refused { .. }), "{map:?}: {err}");
        assert_eq!(&working, expected, "{map:?}: nothing changed in memory");
        assert_eq!(
            fs::read(journal_path(bench.state(), bench.root())).unwrap(),
            on_disk,
            "{map:?}: nothing changed on disk"
        );
    }
}

#[test]
fn rekey_ignores_what_the_journal_does_not_hold_and_saves_nothing() {
    let (bench, mut journal, a, _) = layered();
    let path = journal_path(bench.state(), bench.root());
    let modified = fs::metadata(&path).unwrap().modified().unwrap();
    let before = journal.clone();
    let map = BTreeMap::from([
        (a.id(), a.id()),
        (
            id("0000000000000ee1#Never Installed"),
            id("0000000000000ee2#Anywhere"),
        ),
    ]);
    assert_eq!(rekey(&bench.ctx(), &mut journal, &map), Ok(0));
    assert_eq!(journal, before);
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), modified);
}

// ---------------------------------------------------------------------------
// The relink matcher
// ---------------------------------------------------------------------------

/// A library option of the package `package` (its display name, as the
/// catalog's trail starts), at `root`, named `name`, installing `targets`.
fn option(package: &str, root: &str, name: &str, targets: &[DeployTarget]) -> ModOption {
    let package_id = PackageId::for_folder(package);
    let payload: Vec<PayloadFile> = targets
        .iter()
        .map(|target| PayloadFile {
            entry: format!("{root}/{}", target.game_path().unwrap_or("x.pak")),
            target: target.clone(),
            size: 1,
        })
        .collect();
    ModOption {
        id: OptionId::new(&package_id, root),
        package_id,
        root: root.into(),
        name: name.into(),
        info: None,
        screenshot: None,
        kinds: PayloadKinds::tally(&payload),
        menu_only: payload.is_empty(),
        payload,
        skipped: Vec::new(),
        parent: None,
        addon_for: None,
        bundle: None,
        trail: vec![package.to_string()],
    }
}

fn loose(path: &str) -> DeployTarget {
    DeployTarget::Loose {
        game_path: path.into(),
    }
}

fn script(path: &str) -> DeployTarget {
    DeployTarget::Reframework {
        game_path: path.into(),
    }
}

fn pak(name: &str) -> DeployTarget {
    DeployTarget::Pak {
        file_name: name.into(),
    }
}

/// An install the Riftstone made, as the journal recorded it.
fn record(
    option_id: &str,
    name: &str,
    root: &str,
    files: &[(&str, DeployKind)],
) -> InstalledOption {
    InstalledOption {
        seq: 1,
        option_id: id(option_id),
        name: name.into(),
        package_file: format!("{name}.zip"),
        package_path: Path::new("/old/mods").join(format!("{name}.zip")),
        package_size: 1,
        option_root: root.into(),
        version: None,
        installed_at: "2026-10-16T10:00:00.000Z".into(),
        game_build: None,
        origin: Origin::Riftstone,
        files: files
            .iter()
            .map(|(game_path, kind)| DeployedFile {
                game_path: (*game_path).into(),
                kind: *kind,
                size: 1,
                entry: None,
                below: Below::Nothing,
                fingerprint: None,
            })
            .collect(),
    }
}

#[test]
fn relink_finds_the_one_option_a_moved_install_fits() {
    let install = record(
        "0000000000000f01#Glow",
        "Lantern Glow Recolor",
        "glow",
        &[
            ("natives/STM/glow.mesh", DeployKind::Loose),
            ("reframework/autorun/glow.lua", DeployKind::Reframework),
        ],
    );
    let full = option(
        "Lantern Glow Recolor",
        "Glow",
        "Lantern Glow Recolor",
        &[
            loose("natives/stm/glow.mesh"),
            script("reframework/autorun/glow.lua"),
            loose("natives/stm/extra.mesh"),
        ],
    );
    let lite = option(
        "Lantern Glow Lite",
        "Glow",
        "Lantern Glow Recolor",
        &[loose("natives/stm/glow.mesh")],
    );
    let elsewhere = option(
        "Vernworth Cloaks Collection",
        "Cloaks",
        "Lantern Glow Recolor",
        &[
            loose("natives/stm/glow.mesh"),
            script("reframework/autorun/glow.lua"),
        ],
    );
    let proposed = propose(std::slice::from_ref(&install), &[&lite, &full, &elsewhere]);
    assert_eq!(
        proposed,
        BTreeMap::from([(install.option_id.clone(), full.id.clone())]),
        "same root (any case), every recorded file in the payload (any case); more files are fine"
    );
}

#[test]
fn relink_asks_for_the_name_when_the_root_is_empty() {
    let install = record(
        "0000000000000f02#",
        "Pawn Banter Quieter",
        "",
        &[("natives/stm/sound/banter.bnk", DeployKind::Loose)],
    );
    let other_name = option(
        "Banter Pack",
        "",
        "Pawn Banter Quieter (2026)",
        &[loose("natives/stm/sound/banter.bnk")],
    );
    assert!(
        propose(std::slice::from_ref(&install), &[&other_name]).is_empty(),
        "an empty root needs the name too"
    );
    let by_package = option(
        "Pawn Banter Quieter",
        "",
        "banter",
        &[loose("natives/stm/sound/banter.bnk")],
    );
    assert_eq!(
        propose(std::slice::from_ref(&install), &[&other_name, &by_package]),
        BTreeMap::from([(install.option_id.clone(), by_package.id.clone())]),
        "the package's name counts as well as the option's"
    );
    let by_option = option(
        "Banter Pack",
        "",
        "PAWN BANTER QUIETER",
        &[loose("natives/stm/sound/banter.bnk")],
    );
    assert_eq!(
        propose(std::slice::from_ref(&install), &[&by_option]),
        BTreeMap::from([(install.option_id.clone(), by_option.id.clone())]),
        "names compare case-blind"
    );
}

#[test]
fn relink_asks_for_the_name_when_only_paks_speak() {
    let install = record(
        "0000000000000f03#Cloaks",
        "Vernworth Cloaks Collection",
        "Cloaks",
        &[("pak_mods/zrift-0003-cloaks.pak", DeployKind::Pak)],
    );
    let named = option(
        "Vernworth Cloaks 2207",
        "Cloaks",
        "Vernworth Cloaks Collection",
        &[pak("cloaks.pak")],
    );
    let unnamed = option("Other Pack", "Cloaks", "Different", &[pak("cloaks.pak")]);
    let no_paks = option(
        "Vernworth Cloaks Collection",
        "Cloaks",
        "Vernworth Cloaks Collection",
        &[loose("natives/stm/cloak.mesh")],
    );
    assert_eq!(
        propose(
            std::slice::from_ref(&install),
            &[&named, &unnamed, &no_paks]
        ),
        BTreeMap::from([(install.option_id.clone(), named.id.clone())])
    );
    assert!(propose(std::slice::from_ref(&install), &[&unnamed]).is_empty());

    // A pak asks the option to carry paks even when loose files speak.
    let mixed = record(
        "0000000000000f04#Cloaks",
        "Vernworth Cloaks Collection",
        "Cloaks",
        &[
            ("pak_mods/zrift-0004-cloaks.pak", DeployKind::Pak),
            ("natives/stm/cloak.mesh", DeployKind::Loose),
        ],
    );
    assert!(propose(&[mixed], &[&no_paks]).is_empty());
}

#[test]
fn relink_proposes_nothing_ambiguous_and_nothing_onto_a_held_id() {
    let install = record(
        "0000000000000f05#Glow",
        "Lantern Glow Recolor",
        "Glow",
        &[("natives/stm/glow.mesh", DeployKind::Loose)],
    );
    let one = option(
        "Lantern Glow Recolor",
        "Glow",
        "Lantern Glow Recolor",
        &[loose("natives/stm/glow.mesh")],
    );
    let two = option(
        "Lantern Glow Recolor v2",
        "Glow",
        "Lantern Glow Recolor",
        &[loose("natives/stm/glow.mesh")],
    );
    assert!(
        propose(std::slice::from_ref(&install), &[&one, &two]).is_empty(),
        "two options fit: no match"
    );

    // The one option that fits is already an install of its own.
    let mut holder = record(&one.id.0, "Lantern Glow Recolor", "Glow", &[]);
    holder.seq = 2;
    assert!(propose(&[install.clone(), holder], &[&one]).is_empty());
    // …or a backup in the journal names it as its owner.
    let mut over = record(
        "0000000000000f07#Cloaks",
        "Vernworth Cloaks",
        "Cloaks",
        &[("natives/stm/cloak.mesh", DeployKind::Loose)],
    );
    over.files[0].below = Below::Backup {
        store: "backups/3/natives/stm/cloak.mesh".into(),
        owner: BelowOwner::Riftstone {
            option_id: one.id.clone(),
        },
    };
    assert!(propose(&[install.clone(), over], &[&one]).is_empty());

    // Two installs fit the one option: neither takes it.
    let twin = record(
        "0000000000000f06#Glow",
        "Lantern Glow Recolor",
        "Glow",
        &[("natives/stm/glow.mesh", DeployKind::Loose)],
    );
    assert!(propose(&[install.clone(), twin], &[&one]).is_empty());

    // The same option listed twice is still one option.
    assert_eq!(
        propose(std::slice::from_ref(&install), &[&one, &one]).len(),
        1
    );
}

#[test]
fn relink_leaves_an_install_whose_package_is_still_in_the_mods_folder() {
    let here = option(
        "Lantern Glow Recolor",
        "Glow",
        "Lantern Glow Recolor",
        &[loose("natives/stm/other.mesh")],
    );
    let package_id = here.package_id.clone();
    let install = record(
        &OptionId::new(&package_id, "Glow Old").0,
        "Lantern Glow Recolor",
        "Glow",
        &[("natives/stm/glow.mesh", DeployKind::Loose)],
    );
    let fits = option(
        "Lantern Glow Copy",
        "Glow",
        "Lantern Glow Recolor",
        &[loose("natives/stm/glow.mesh")],
    );
    assert!(propose(&[install], &[&here, &fits]).is_empty());
}

#[test]
fn the_relink_pass_finds_a_lifted_stand_in_and_one_save_rekeys_it() {
    let bench = Bench::new();
    bench.write("natives/stm/glow.mesh", b"glow");
    let raw = json!({
        "schema": 1,
        "gameRoot": bench.root(),
        "installs": [install_json(1, "elsewhere#Lantern Glow-v2", "Lantern Glow Recolor", "Lantern Glow-v2",
            json!({ "kind": "adoptedFromElsewhere", "section": "Lantern Glow-v2" }),
            json!([file_json("natives/stm/glow.mesh", "loose", 4, json!({ "kind": "nothing" }))]))],
        "pakOrder": [],
        "pending": null,
        "nextSeq": 2
    });
    write_raw(bench.state(), bench.root(), &raw);
    let mut journal = bench.journal();
    let stand_in = id("adopted#Lantern Glow-v2");
    assert!(journal.is_installed(&stand_in));

    let library = option(
        "Lantern Glow Recolor",
        "Lantern Glow-v2",
        "Lantern Glow Recolor",
        &[loose("natives/stm/glow.mesh")],
    );
    let map = propose(&journal.installs, &[&library]);
    assert_eq!(
        map,
        BTreeMap::from([(stand_in.clone(), library.id.clone())])
    );
    let files = BTreeMap::from([(library.id.clone(), "Lantern Glow Recolor".to_string())]);
    assert_eq!(
        rekey_with_files(&bench.ctx(), &mut journal, &map, &files),
        Ok(1)
    );

    let found = journal.find(&library.id).unwrap();
    assert_eq!(
        found.origin,
        Origin::Adopted {
            label: "Lantern Glow-v2".into()
        },
        "a relinked install keeps its story"
    );
    assert_eq!(found.package_file, "Lantern Glow Recolor");
    assert_eq!(bench.journal(), journal, "one save, schema 2");
    assert!(
        propose(&journal.installs, &[&library]).is_empty(),
        "its package is in the mods folder now: nothing more to do"
    );
}

#[test]
fn an_adopted_install_built_by_hand_relinks_like_a_lifted_one() {
    let mut journal = Journal::new(Path::new("/g"));
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
    let library = option(
        "Older Armour",
        "Older Armour-v1",
        "Older Armour",
        &[pak("Older Armour.pak")],
    );
    let renamed = option(
        "Older Armour",
        "Older Armour-v1",
        "Armour, Older",
        &[pak("Older Armour.pak")],
    );
    assert_eq!(
        propose(std::slice::from_ref(&older), &[&library]),
        BTreeMap::from([(older.option_id.clone(), library.id.clone())])
    );
    assert_eq!(
        propose(std::slice::from_ref(&older), &[&renamed]).len(),
        1,
        "the package's name still matches"
    );
}
