//! Save backups use invented raw files; no parser or real save is involved.
mod support;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use rift_muster::saves::SaveFolder;
use riftstone_lib::save_backup;

fn sources(root: &std::path::Path) -> Vec<SaveFolder> {
    [17, 42]
        .into_iter()
        .map(|account_id| {
            let path = root.join(format!(
                "Steam/userdata/{account_id}/2054970/remote/win64_save"
            ));
            fs::create_dir_all(&path).unwrap();
            for (name, bytes) in [
                ("data009Slot.bin", b"invented encrypted slot".as_slice()),
                ("data00-1.bin", b"invented system data"),
                ("SS1_data000.bin", b"invented creator data"),
            ] {
                fs::write(path.join(name), bytes).unwrap();
            }
            SaveFolder { account_id, path }
        })
        .collect()
}

#[test]
fn backup_keeps_every_raw_file_and_a_verified_manifest_without_changing_sources() {
    let dir = tempfile::tempdir().unwrap();
    let folders = sources(dir.path());
    let destination = dir.path().join("Backups");
    fs::create_dir(&destination).unwrap();
    let report = save_backup::create(&folders, &destination, &[], || false).unwrap();
    assert_eq!((report.accounts, report.files), (2, 6));
    let made = PathBuf::from(&report.path);
    assert!(made.join("manifest.json").is_file());
    assert!(made.join("RESTORE.txt").is_file());
    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(made.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(receipt["original_encrypted_files"], true);
    for (i, folder) in folders.iter().enumerate() {
        for name in ["data009Slot.bin", "data00-1.bin", "SS1_data000.bin"] {
            assert_eq!(
                fs::read(made.join(format!("account-{}/win64_save/{name}", i + 1))).unwrap(),
                fs::read(folder.path.join(name)).unwrap()
            );
            assert_eq!(
                receipt["accounts"][i]["files"][name]["sha256"]
                    .as_str()
                    .unwrap()
                    .len(),
                64
            );
        }
    }
    let second = save_backup::create(&folders, &destination, &[], || false).unwrap();
    assert_ne!(
        report.path, second.path,
        "a later backup never replaces the first"
    );
    assert!(made.join("manifest.json").is_file());
}

#[test]
fn running_game_and_protected_destinations_are_refused_without_writes() {
    let dir = tempfile::tempdir().unwrap();
    let folders = sources(dir.path());
    let backup = dir.path().join("Backups");
    fs::create_dir(&backup).unwrap();
    assert!(save_backup::create(&folders, &backup, &[], || true).is_err());
    assert_eq!(fs::read_dir(&backup).unwrap().count(), 0);
    for destination in [&folders[0].path, &dir.path().join("Steam/userdata")] {
        assert!(save_backup::create(&folders, destination, &[], || false).is_err());
    }
    assert!(
        save_backup::create(&folders, &backup, std::slice::from_ref(&backup), || false).is_err()
    );
    assert_eq!(fs::read_dir(&backup).unwrap().count(), 0);
}

#[test]
fn changed_sources_cancel_and_remove_only_the_new_attempt() {
    let dir = tempfile::tempdir().unwrap();
    let folders = sources(dir.path());
    let backup = dir.path().join("Backups");
    fs::create_dir(&backup).unwrap();
    fs::write(backup.join("keep.txt"), b"the player's own").unwrap();
    let calls = AtomicUsize::new(0);
    let outcome = save_backup::create(&folders, &backup, &[], || {
        if calls.fetch_add(1, Ordering::SeqCst) == 1 {
            fs::write(
                folders[0].path.join("data009Slot.bin"),
                b"changed during backup",
            )
            .unwrap();
        }
        false
    });
    assert!(outcome.is_err());
    assert_eq!(fs::read_dir(&backup).unwrap().count(), 1);
    assert_eq!(
        fs::read(backup.join("keep.txt")).unwrap(),
        b"the player's own"
    );
}

#[test]
fn a_new_source_file_or_a_game_start_during_copy_leaves_no_completed_backup() {
    for start_game in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let folders = sources(dir.path());
        let backup = dir.path().join("Backups");
        fs::create_dir(&backup).unwrap();
        let calls = AtomicUsize::new(0);
        let outcome = save_backup::create(&folders, &backup, &[], || {
            let during_copy = calls.fetch_add(1, Ordering::SeqCst) == 2;
            if during_copy && !start_game {
                fs::write(folders[0].path.join("new-system-file.bin"), b"new").unwrap();
            }
            during_copy && start_game
        });
        assert!(outcome.is_err());
        assert_eq!(fs::read_dir(&backup).unwrap().count(), 0);
        assert_eq!(
            fs::read(folders[0].path.join("data009Slot.bin")).unwrap(),
            b"invented encrypted slot"
        );
    }
}

#[cfg(unix)]
#[test]
fn source_links_and_a_destination_link_back_into_saves_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let folders = sources(dir.path());
    let backup = dir.path().join("Backups");
    fs::create_dir(&backup).unwrap();
    std::os::unix::fs::symlink(&folders[0].path, dir.path().join("save-link")).unwrap();
    assert!(save_backup::create(&folders, &dir.path().join("save-link"), &[], || false).is_err());
    std::os::unix::fs::symlink(backup.join("outside"), folders[0].path.join("linked.bin")).unwrap();
    assert!(save_backup::create(&folders, &backup, &[], || false).is_err());
    assert_eq!(fs::read_dir(&backup).unwrap().count(), 0);
}
