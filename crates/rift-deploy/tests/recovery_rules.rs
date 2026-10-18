//! § rift-deploy rule 3, the uninstall half: an uninstall a crash cut short
//! is rolled back at the next launch, and the journal's cleanup list only
//! ever deletes the Riftstone's own leftovers (a leftover that cannot go yet
//! stays listed, and a launch that changed nothing saves nothing).

mod common;

use std::fs;

use common::{assert_no_temporaries, Bench, TestMod};
use rift_core::RiftError;
use rift_deploy::{
    journal_path, recover, Below, Intent, IntentAction, IntentStep, Journal, Leftover,
};
use serde_json::json;

const DELETE: &str = "natives/stm/delete.mesh";
const RESTORE: &str = "natives/stm/restore.mesh";

/// Install a mod with one file over nothing and one over an original, then
/// replay the first `done` moves of its uninstall by hand and save the
/// intent — the state a crash leaves behind.
fn crashed_uninstall(
    done: usize,
) -> (
    Bench,
    TestMod,
    std::collections::BTreeMap<String, Option<Vec<u8>>>,
    String,
) {
    let bench = Bench::new();
    bench.write(RESTORE, b"the original");
    let cape = TestMod::new("Ermine Cape")
        .loose(DELETE, b"cape delete")
        .loose(RESTORE, b"cape restore");
    let mut journal = bench.journal();
    cape.install(&bench, &mut journal);
    let installed = bench.snapshot();
    let Below::Backup { store, .. } = journal.installs[0].files[1].below.clone() else {
        panic!("the second file has a backup");
    };
    let seq = 77;
    let trash = |p: &str| format!("{p}.riftstone-trash-{seq}");
    let steps = vec![
        IntentStep::Remove {
            game_path: DELETE.into(),
            trash: trash(DELETE),
        },
        IntentStep::Restore {
            game_path: RESTORE.into(),
            trash: Some(trash(RESTORE)),
            backup: store.clone(),
        },
    ];
    let root = bench.root().to_path_buf();
    let state = bench.state().to_path_buf();
    // The moves of the uninstall, in order: delete → trash; restore → trash;
    // backup → restore.
    let moves: [Box<dyn Fn()>; 3] = [
        Box::new({
            let root = root.clone();
            move || fs::rename(root.join(DELETE), root.join(trash(DELETE))).unwrap()
        }),
        Box::new({
            let root = root.clone();
            move || fs::rename(root.join(RESTORE), root.join(trash(RESTORE))).unwrap()
        }),
        Box::new({
            let (root, state, store) = (root.clone(), state.clone(), store.clone());
            move || fs::rename(state.join(&store), root.join(RESTORE)).unwrap()
        }),
    ];
    for apply in moves.iter().take(done) {
        apply();
    }
    journal.pending = Some(Intent {
        action: IntentAction::Uninstall,
        seq,
        option_id: cape.id(),
        touched: vec![DELETE.into(), RESTORE.into()],
        started_at: "2026-10-16T10:00:00Z".into(),
        option_name: "Ermine Cape".into(),
        steps,
        phase: 0,
        dirs: Vec::new(),
    });
    journal.next_seq = seq + 1;
    journal.save(bench.state()).unwrap();
    (bench, cape, installed, store)
}

#[test]
fn an_uninstall_cut_short_at_any_move_is_rolled_back() {
    for done in 0..=3 {
        let (bench, cape, installed, store) = crashed_uninstall(done);
        let mut relaunched = bench.journal();
        let report = recover(&bench.ctx(), &mut relaunched).unwrap().unwrap();
        assert_eq!(report.action, IntentAction::Uninstall);
        assert_eq!(
            bench.snapshot(),
            installed,
            "after {done} moves: the mod is whole again"
        );
        assert_eq!(
            fs::read(bench.state().join(&store)).unwrap(),
            b"the original",
            "after {done} moves: the original is back in the store"
        );
        assert!(relaunched.find(&cape.id()).is_some(), "still installed");
        assert!(bench.journal().pending.is_none());
        assert_no_temporaries(&bench);
        // …and it uninstalls cleanly afterwards.
        cape.uninstall(&bench, &mut relaunched);
        assert_eq!(bench.read(RESTORE).unwrap(), b"the original");
        assert!(!bench.exists(DELETE));
    }
}

#[test]
fn the_cleanup_list_deletes_only_the_riftstones_own_leftovers() {
    let bench = Bench::new();
    bench.write("natives/stm/a.mesh.riftstone-trash-5", b"old bytes");
    bench.write("natives/stm/real.mesh", b"a real mod file");
    fs::create_dir_all(bench.state().join("backups/5/natives/stm")).unwrap();
    fs::write(
        bench.state().join("backups/5/natives/stm/a.mesh"),
        b"superseded",
    )
    .unwrap();
    fs::write(bench.dir.path().join("outside.txt"), b"not the Riftstone's").unwrap();

    let mut journal = Journal::new(bench.root());
    journal.cleanup = vec![
        Leftover::Game {
            path: "natives/stm/a.mesh.riftstone-trash-5".into(),
        },
        Leftover::Store {
            path: "backups/5/natives/stm/a.mesh".into(),
        },
        // Not the Riftstone's: no trash mark, a climb out, an absolute path.
        Leftover::Game {
            path: "natives/stm/real.mesh".into(),
        },
        Leftover::Game {
            path: "../outside.txt.riftstone-trash-1".into(),
        },
        Leftover::Store {
            path: "backups/../../outside.txt".into(),
        },
    ];
    journal.save(bench.state()).unwrap();

    let mut relaunched = bench.journal();
    assert_eq!(
        recover(&bench.ctx(), &mut relaunched).unwrap(),
        None,
        "nothing was pending"
    );
    assert!(!bench.exists("natives/stm/a.mesh.riftstone-trash-5"));
    assert!(
        !bench.state().join("backups/5").exists(),
        "emptied store folders are pruned"
    );
    assert_eq!(
        bench.read("natives/stm/real.mesh").unwrap(),
        b"a real mod file"
    );
    assert!(bench.dir.path().join("outside.txt").exists());
    assert!(bench.journal().cleanup.is_empty());
}

#[test]
fn a_leftover_that_cannot_go_yet_saves_nothing_at_launch() {
    let bench = Bench::new();
    // A folder sits where the leftover trash file should be: deleting a file
    // there fails on every platform, so the entry stays for the next try.
    let stuck = "natives/stm/x.mesh.riftstone-trash-2";
    bench.write(
        &format!("{stuck}/inside.txt"),
        b"a folder, not the trash file",
    );
    // A journal an older build wrote (schema 1), listing that leftover.
    let journal_file = journal_path(bench.state(), bench.root());
    let written = serde_json::to_string_pretty(&json!({
        "schema": 1,
        "gameRoot": bench.root(),
        "installs": [],
        "pakOrder": [],
        "pending": null,
        "nextSeq": 3,
        "cleanup": [{ "place": "game", "path": stuck }],
        "parked": [],
        "createdDirs": []
    }))
    .unwrap();
    fs::write(&journal_file, &written).unwrap();

    for launch in 1..=2 {
        let mut relaunched = bench.journal();
        assert_eq!(recover(&bench.ctx(), &mut relaunched).unwrap(), None);
        assert_eq!(
            relaunched.cleanup,
            vec![Leftover::Game { path: stuck.into() }],
            "launch {launch}: still listed for the next try"
        );
        assert_eq!(
            fs::read_to_string(&journal_file).unwrap(),
            written,
            "launch {launch}: nothing changed, so nothing was saved (schema 1 stays as written)"
        );
    }
    assert!(
        bench.root().join(stuck).is_dir(),
        "the folder was left alone"
    );

    // Once the leftover is gone, the next launch drops it, and that change is
    // the journal's next write: schema 2.
    fs::remove_dir_all(bench.root().join(stuck)).unwrap();
    let mut relaunched = bench.journal();
    assert_eq!(recover(&bench.ctx(), &mut relaunched).unwrap(), None);
    let on_disk: serde_json::Value =
        serde_json::from_slice(&fs::read(&journal_file).unwrap()).unwrap();
    assert_eq!(on_disk["schema"], 2);
    assert_eq!(on_disk["cleanup"], json!([]));
}

#[test]
fn an_intent_with_an_unsafe_path_is_never_acted_on() {
    let bench = Bench::new();
    fs::write(bench.dir.path().join("outside.mesh"), b"outside").unwrap();
    fs::create_dir_all(bench.dir.path().join("outside-empty-folder")).unwrap();
    fs::write(bench.state().join("not-a-backup.mesh"), b"app data").unwrap();
    let mut journal = Journal::new(bench.root());
    journal.pending = Some(Intent {
        action: IntentAction::Install,
        seq: 3,
        option_id: rift_core::OptionId("x#y".into()),
        touched: vec!["../outside.mesh".into()],
        started_at: "2026-10-16T10:00:00Z".into(),
        option_name: "Hostile Journal".into(),
        steps: vec![
            IntentStep::Place {
                game_path: "../outside.mesh".into(),
                size: 7,
                backup: None,
                owner: None,
                fingerprint: None,
                unextracted: false,
            },
            // A backup slot outside backups/ is not the Riftstone's either.
            IntentStep::Place {
                game_path: "natives/stm/a.mesh".into(),
                size: 8,
                backup: Some("not-a-backup.mesh".into()),
                owner: None,
                fingerprint: None,
                unextracted: false,
            },
        ],
        phase: 0,
        dirs: vec!["../outside-empty-folder".into()],
    });
    journal.save(bench.state()).unwrap();
    let mut relaunched = bench.journal();
    let report = recover(&bench.ctx(), &mut relaunched).unwrap().unwrap();
    assert_eq!(report.files_rolled_back, 0);
    assert!(bench.dir.path().join("outside.mesh").exists());
    assert!(bench.dir.path().join("outside-empty-folder").exists());
    assert!(!bench.exists("natives/stm/a.mesh"));
    assert!(bench.state().join("not-a-backup.mesh").exists());
}

#[test]
fn recover_refuses_a_journal_of_another_game_folder() {
    let bench = Bench::new();
    let mut foreign = Journal::new(&bench.dir.path().join("Another Copy"));
    let err = recover(&bench.ctx(), &mut foreign).unwrap_err();
    assert!(matches!(err, RiftError::Journal { .. }));
}

// ---------------------------------------------------------------------------
// Rollbacks keep every original on record and never leave a path empty
// ---------------------------------------------------------------------------

/// Install over an original, then turn the finished install back into the
/// pending intent a crash just before the commit leaves behind (its
/// fingerprints saved: every part was renamed into place).
fn crashed_before_commit(bench: &Bench, path: &str, bytes: &[u8]) -> (Journal, String) {
    let mut journal = bench.journal();
    let glow = TestMod::new("Lantern Glow Recolor").target(
        path,
        if path.contains('/') {
            rift_core::DeployTarget::Loose {
                game_path: path.into(),
            }
        } else {
            rift_core::DeployTarget::Root {
                game_path: path.into(),
            }
        },
        bytes,
    );
    glow.install(bench, &mut journal);
    let record = journal.installs.pop().unwrap();
    let Below::Backup { store, owner } = record.files[0].below.clone() else {
        panic!("the original was backed up");
    };
    journal.pending = Some(Intent {
        action: IntentAction::Install,
        seq: record.seq,
        option_id: record.option_id.clone(),
        touched: vec![path.into()],
        started_at: record.installed_at.clone(),
        option_name: record.name.clone(),
        steps: vec![IntentStep::Place {
            game_path: record.files[0].game_path.clone(),
            size: record.files[0].size,
            backup: Some(store.clone()),
            owner: Some(owner),
            fingerprint: record.files[0].fingerprint.clone(),
            unextracted: false,
        }],
        phase: 0,
        dirs: std::mem::take(&mut journal.created_dirs),
    });
    journal.save(bench.state()).unwrap();
    (journal, store)
}

#[test]
fn an_original_that_cannot_go_back_after_a_crash_is_kept_on_record() {
    let bench = Bench::new();
    bench.write("natives/stm/glow.mesh", b"the original file");
    let (_, store) = crashed_before_commit(&bench, "natives/stm/glow.mesh", b"glow bytes");
    // Before the relaunch, another tool replaces the file.
    bench.write("natives/stm/glow.mesh", b"another tool's file, longer");

    let mut relaunched = bench.journal();
    let report = recover(&bench.ctx(), &mut relaunched).unwrap().unwrap();
    assert_eq!(report.files_kept, 1);
    assert_eq!(
        bench.read("natives/stm/glow.mesh").unwrap(),
        b"another tool's file, longer"
    );
    assert_eq!(
        fs::read(bench.state().join(&store)).unwrap(),
        b"the original file"
    );
    assert!(
        common::journal_text(&bench).contains(&store),
        "the original is on record, not orphaned"
    );
    assert_eq!(report.parked.len(), 1);
    assert_eq!(report.parked[0].store, store);
    assert_eq!(report.parked[0].owner, rift_deploy::BelowOwner::Unowned);
    assert_eq!(bench.journal().parked, report.parked);
    assert!(bench.journal().pending.is_none());
}

#[cfg(unix)]
#[test]
fn a_backup_that_cannot_go_back_yet_never_leaves_the_path_empty() {
    if !common::permissions_bite() {
        return;
    }
    let bench = Bench::new();
    bench.write("lantern.ini", b"the original ini");
    let (_, store) = crashed_before_commit(&bench, "lantern.ini", b"glow ini");
    let slot_dir = bench.state().join(&store);
    let slot_dir = slot_dir.parent().unwrap().to_path_buf();
    common::set_read_only(&slot_dir, true, true);

    let mut relaunched = bench.journal();
    let failed = recover(&bench.ctx(), &mut relaunched);
    assert!(failed.is_err(), "the backup cannot move yet");
    assert_eq!(
        bench.read("lantern.ini").as_deref(),
        Some(b"glow ini".as_slice()),
        "the path is never left empty"
    );
    assert!(bench.journal().pending.is_some(), "it stays pending");

    common::set_read_only(&slot_dir, false, true);
    let mut relaunched = bench.journal();
    recover(&bench.ctx(), &mut relaunched).unwrap().unwrap();
    assert_eq!(bench.read("lantern.ini").unwrap(), b"the original ini");
    assert!(bench.store_files().is_empty());
}

#[test]
fn a_restore_cut_short_mid_copy_leaves_no_partial_file() {
    let (bench, _, installed, _) = crashed_uninstall(2);
    // A cross-volume restore copy was being written when the crash came.
    bench.write(&format!("{RESTORE}.riftstone-part"), b"half a co");
    let mut relaunched = bench.journal();
    recover(&bench.ctx(), &mut relaunched).unwrap().unwrap();
    assert_eq!(bench.snapshot(), installed);
    assert_no_temporaries(&bench);
}

#[test]
fn a_backup_cut_short_mid_copy_leaves_nothing_in_the_store() {
    let bench = Bench::new();
    bench.write("natives/stm/b.mesh", b"original b");
    let before = bench.snapshot();
    let mut journal = Journal::new(bench.root());
    journal.pending = Some(Intent {
        action: IntentAction::Install,
        seq: 9,
        option_id: rift_core::OptionId("x#y".into()),
        touched: vec!["natives/stm/b.mesh".into()],
        started_at: "2026-10-16T10:00:00Z".into(),
        option_name: "Vernworth Cloaks".into(),
        steps: vec![IntentStep::Place {
            game_path: "natives/stm/b.mesh".into(),
            size: 5,
            backup: Some("backups/9/natives/stm/b.mesh".into()),
            owner: Some(rift_deploy::BelowOwner::Unowned),
            fingerprint: None,
            unextracted: false,
        }],
        phase: 0,
        dirs: Vec::new(),
    });
    journal.next_seq = 10;
    journal.save(bench.state()).unwrap();
    // The cross-volume copy into the store was half done.
    fs::create_dir_all(bench.state().join("backups/9/natives/stm")).unwrap();
    fs::write(
        bench
            .state()
            .join("backups/9/natives/stm/b.mesh.riftstone-part"),
        b"origi",
    )
    .unwrap();

    let mut relaunched = bench.journal();
    recover(&bench.ctx(), &mut relaunched).unwrap().unwrap();
    assert_eq!(bench.snapshot(), before);
    assert!(bench.store_files().is_empty(), "{:?}", bench.store_files());
}

/// A crash after the renames, before the commit: a file another tool wrote
/// over one of the finished files — the same size — is not this install's,
/// and its fingerprint says so.
#[test]
fn a_same_size_file_over_a_finished_part_survives_recovery() {
    let bench = Bench::new();
    let mut journal = bench.journal();
    let glow = TestMod::new("Lantern Glow Recolor")
        .loose("natives/stm/a.mesh", b"glow a mesh")
        .loose("natives/stm/b.mesh", b"glow b mesh");
    glow.install(&bench, &mut journal);
    let record = journal.installs.pop().unwrap();
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
                backup: None,
                owner: None,
                fingerprint: f.fingerprint.clone(),
                unextracted: false,
            })
            .collect(),
        phase: 0,
        dirs: std::mem::take(&mut journal.created_dirs),
    });
    journal.save(bench.state()).unwrap();
    bench.write("natives/stm/b.mesh", b"RIVAL BYTES");

    let mut relaunched = bench.journal();
    let report = recover(&bench.ctx(), &mut relaunched).unwrap().unwrap();
    assert_eq!((report.files_rolled_back, report.files_kept), (1, 1));
    assert!(!bench.exists("natives/stm/a.mesh"), "its own file went");
    assert_eq!(bench.read("natives/stm/b.mesh").unwrap(), b"RIVAL BYTES");
    assert_no_temporaries(&bench);
}

/// An empty folder at a target during an undo can only be the install's own
/// leftover: it is pruned, and the original comes back.
#[test]
fn an_empty_folder_at_a_target_is_pruned_and_the_original_returns() {
    let bench = Bench::new();
    bench.write("natives/stm/a", b"the original a");
    let before = bench.snapshot();
    fs::create_dir_all(bench.state().join("backups/4/natives/stm")).unwrap();
    fs::rename(
        bench.root().join("natives/stm/a"),
        bench.state().join("backups/4/natives/stm/a"),
    )
    .unwrap();
    fs::create_dir_all(bench.root().join("natives/stm/a/b")).unwrap();
    let mut journal = Journal::new(bench.root());
    journal.pending = Some(Intent {
        action: IntentAction::Install,
        seq: 4,
        option_id: rift_core::OptionId("x#y".into()),
        touched: vec!["natives/stm/a".into()],
        started_at: "2026-10-16T10:00:00Z".into(),
        option_name: "Broken Archive".into(),
        steps: vec![IntentStep::Place {
            game_path: "natives/stm/a".into(),
            size: 6,
            backup: Some("backups/4/natives/stm/a".into()),
            owner: Some(rift_deploy::BelowOwner::Unowned),
            fingerprint: Some("00".repeat(16)),
            unextracted: false,
        }],
        phase: 0,
        dirs: Vec::new(),
    });
    journal.next_seq = 5;
    journal.save(bench.state()).unwrap();

    let mut relaunched = bench.journal();
    let report = recover(&bench.ctx(), &mut relaunched).unwrap().unwrap();
    assert!(report.parked.is_empty());
    assert_eq!(bench.snapshot(), before);
    assert!(bench.store_files().is_empty());
}

/// Putting a kept file back is journaled too: a crash half-way through the
/// swap is rolled back at the next launch, and the file stays kept aside.
#[test]
fn a_swap_cut_short_is_rolled_back_and_the_file_stays_kept() {
    let bench = Bench::new();
    bench.write("natives/stm/glow.mesh", b"original");
    let mut journal = bench.journal();
    let glow = TestMod::new("Lantern Glow Recolor").loose("natives/stm/glow.mesh", b"glow bytes");
    glow.install(&bench, &mut journal);
    bench.write("natives/stm/glow.mesh", b"the player's edit");
    let kept = glow.uninstall(&bench, &mut journal).parked[0].clone();
    let saved = bench.journal();

    // The swap's first move happened (the edit went into the store), then
    // the crash came before the kept file moved back.
    let swap_store = "backups/90/natives/stm/glow.mesh";
    fs::create_dir_all(bench.state().join("backups/90/natives/stm")).unwrap();
    fs::rename(
        bench.root().join("natives/stm/glow.mesh"),
        bench.state().join(swap_store),
    )
    .unwrap();
    journal.pending = Some(Intent {
        action: IntentAction::RestoreParked,
        seq: 90,
        option_id: rift_core::OptionId(String::new()),
        touched: vec!["natives/stm/glow.mesh".into()],
        started_at: "2026-10-16T10:00:00Z".into(),
        option_name: "the file kept aside for natives/stm/glow.mesh".into(),
        steps: vec![
            IntentStep::Park {
                game_path: "natives/stm/glow.mesh".into(),
                store: swap_store.into(),
            },
            IntentStep::Restore {
                game_path: "natives/stm/glow.mesh".into(),
                trash: None,
                backup: kept.store.clone(),
            },
        ],
        phase: 0,
        dirs: Vec::new(),
    });
    journal.next_seq = 91;
    journal.save(bench.state()).unwrap();

    let mut relaunched = bench.journal();
    let report = recover(&bench.ctx(), &mut relaunched).unwrap().unwrap();
    assert_eq!(report.action, IntentAction::RestoreParked);
    assert_eq!(
        bench.read("natives/stm/glow.mesh").unwrap(),
        b"the player's edit"
    );
    assert_eq!(relaunched.parked, saved.parked, "still kept aside");
    assert_eq!(
        fs::read(bench.state().join(&kept.store)).unwrap(),
        b"original"
    );
    assert!(!bench.state().join(swap_store).exists());
}

/// A reinstall cut short just after the new copy moved the original aside,
/// then another tool wrote the path before the relaunch. The undo leaves the
/// other tool's file where it is, keeps the original on record, and deletes
/// nothing — the old copy's bytes stay under their temporary name.
#[test]
fn a_reinstall_cut_short_never_moves_a_file_another_tool_wrote() {
    const P: &str = "natives/stm/glow.mesh";
    let bench = Bench::new();
    bench.write(P, b"the original");
    let mut journal = bench.journal();
    let glow = TestMod::new("Lantern Glow Recolor").loose(P, b"glow v1");
    glow.install(&bench, &mut journal);
    let Below::Backup {
        store: old_slot, ..
    } = journal.installs[0].files[0].below.clone()
    else {
        panic!("the original was backed up");
    };
    let root = bench.root().to_path_buf();
    let state = bench.state().to_path_buf();
    let trash = format!("{P}.riftstone-trash-7");
    let new_slot = format!("backups/7/{P}");
    // The uninstall half: the old copy to its trash name, the original back.
    fs::rename(root.join(P), root.join(&trash)).unwrap();
    fs::rename(state.join(&old_slot), root.join(P)).unwrap();
    // The install half: the original moved aside, the new part extracted.
    fs::create_dir_all(state.join("backups/7/natives/stm")).unwrap();
    fs::rename(root.join(P), state.join(&new_slot)).unwrap();
    bench.write(&format!("{P}.riftstone-part"), b"glow v2");
    journal.pending = Some(Intent {
        action: IntentAction::Install,
        seq: 7,
        option_id: glow.id(),
        touched: vec![P.into()],
        started_at: "2026-10-16T10:00:00Z".into(),
        option_name: "Lantern Glow Recolor".into(),
        steps: vec![
            IntentStep::Restore {
                game_path: P.into(),
                trash: Some(trash.clone()),
                backup: old_slot.clone(),
            },
            IntentStep::Place {
                game_path: P.into(),
                size: 7,
                backup: Some(new_slot.clone()),
                owner: Some(rift_deploy::BelowOwner::Unowned),
                fingerprint: Some("00".repeat(16)),
                unextracted: false,
            },
        ],
        phase: 0,
        dirs: Vec::new(),
    });
    journal.next_seq = 8;
    journal.save(bench.state()).unwrap();
    // Before the relaunch, another tool writes the path.
    bench.write(P, b"another tool's glow");

    let mut relaunched = bench.journal();
    let report = recover(&bench.ctx(), &mut relaunched).unwrap().unwrap();
    assert_eq!(bench.read(P).unwrap(), b"another tool's glow");
    assert_eq!(report.parked.len(), 1);
    assert_eq!(report.parked[0].store, new_slot);
    assert_eq!(fs::read(state.join(&new_slot)).unwrap(), b"the original");
    assert_eq!(
        bench.read(&trash).unwrap(),
        b"glow v1",
        "the old copy's bytes are not deleted"
    );
    assert!(!bench.exists(&format!("{P}.riftstone-part")));
    assert!(relaunched.pending.is_none());
}
