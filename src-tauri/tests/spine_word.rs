//! The player's word (v0.5, ruling 10, WINDOW-CONTRACT § v0.5) through the spine: an invented
//! older version unpacked as a folder and linked by mod id, the way Find links one, beside the
//! newer version's pinned download. Nexus can pin the folder to nothing, so the stone asks; the
//! player's yes joins it into the family, a no is remembered, and a join can be taken back.
//! Every name and id here is invented.

mod support;

use std::fs;
use std::sync::Arc;

use rift_core::{LinkSource, NexusLink, PackageId, RiftError};
use riftstone_lib::commands::{library, nexus, versions};
use riftstone_lib::dto::{LibraryPackage, LibraryView};
use riftstone_lib::persist::DataDirs;
use riftstone_lib::state::AppState;
use serde_json::json;
use support::{write_zip, Bench};

const MOD: u64 = 7140;
const GROUP: u64 = 714_000;
const FOLDER: &str = "Hearth Embers";
const NEW: &str = "Hearth Embers 1.1.zip";
const OTHER: &str = "Hearth Embers Ash Patch.zip";
const SCRIPT: &str = "reframework/autorun/hearth_embers.lua";
const DATA: &str = "reframework/data/hearth_embers.json";
const SPARKS: &str = "reframework/data/hearth_embers_sparks.json";

struct Word {
    bench: Bench,
    dirs: DataDirs,
    folder: PackageId,
    new: PackageId,
    other: PackageId,
}

impl Word {
    fn open(&self) -> Arc<AppState> {
        let (state, _) = self.bench.open(self.bench.config(self.dirs.clone()));
        assert!(state.scan_blocking());
        state
    }
}

fn entry<'a>(view: &'a LibraryView, id: &PackageId) -> &'a LibraryPackage {
    view.packages
        .iter()
        .find(|p| p.package.id == id.0)
        .unwrap_or_else(|| panic!("{id} is listed"))
}

fn pinned(file_id: u64, group_id: u64, version: &str, uploaded_ms: u64) -> NexusLink {
    NexusLink {
        mod_id: MOD,
        file_id: Some(file_id),
        group_id: Some(group_id),
        sqid: None,
        version: Some(version.into()),
        uploaded_ms: Some(uploaded_ms),
        source: LinkSource::Download,
    }
}

/// The bench: the older version unpacked as a folder (no top folder of its own, as a player
/// unpacks one), the newer version's download pinned to its file, and another file of the same
/// mod, pinned to another group, whose names are not the folder's.
fn word_bench() -> Word {
    let bench = Bench::new();
    let folder = bench.mods.join(FOLDER);
    for (path, bytes) in [
        ("modinfo.ini", &b"name=Hearth Embers\r\nversion=1.0\r\n"[..]),
        (SCRIPT, b"-- embers 1.0"),
        (DATA, b"{\"glow\": 1}"),
    ] {
        let at = folder.join(path);
        fs::create_dir_all(at.parent().unwrap()).unwrap();
        fs::write(at, bytes).unwrap();
    }
    write_zip(
        &bench.mods.join(NEW),
        &[
            (
                "Hearth Embers/modinfo.ini",
                b"name=Hearth Embers\r\nversion=1.1\r\n",
            ),
            ("Hearth Embers/screenshot.png", b"png"),
            (&format!("Hearth Embers/{SCRIPT}"), b"-- embers 1.1"),
            (&format!("Hearth Embers/{DATA}"), b"{\"glow\": 2}"),
            (&format!("Hearth Embers/{SPARKS}"), b"{\"sparks\": 1}"),
        ],
    );
    write_zip(
        &bench.mods.join(OTHER),
        &[
            ("Ash Patch/modinfo.ini", b"name=Hearth Embers Ash Patch\r\n"),
            ("Ash Patch/reframework/autorun/hearth_ash.lua", b"-- ash"),
        ],
    );
    let archive = |name: &str| {
        PackageId::for_archive(name, fs::metadata(bench.mods.join(name)).unwrap().len())
    };
    let (new, other) = (archive(NEW), archive(OTHER));
    let dirs = DataDirs::under(&bench.data_home());
    let file = dirs.nexus_file();
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(
        &file,
        serde_json::to_vec(&json!({
            "schema": 1,
            "packages": {
                new.0.clone(): { "link": pinned(3002, GROUP, "1.1", 2_000) },
                other.0.clone(): { "link": pinned(3100, GROUP + 1, "1.0", 1_500) },
            }
        }))
        .unwrap(),
    )
    .unwrap();
    Word {
        bench,
        dirs,
        folder: PackageId::for_folder(FOLDER),
        new,
        other,
    }
}

/// The folder linked by mod id, as Find or a pasted mod page links it.
fn found(w: &Word) -> Arc<AppState> {
    let state = w.open();
    nexus::link_package_impl(&state, &w.folder.0, &MOD.to_string()).unwrap();
    state
}

fn refusal(result: Result<(), RiftError>) -> String {
    match result {
        Err(RiftError::Refused { reason }) => reason,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn an_unpacked_version_linked_by_mod_id_is_asked_about_never_joined_on_its_own() {
    let w = word_bench();
    let state = found(&w);
    let view = library::get_library_impl(&state);
    let folder = entry(&view, &w.folder);
    assert!(folder.family.is_none(), "names never make a family");
    let question = folder
        .version_question
        .as_ref()
        .expect("the stone asks about the folder");
    assert_eq!(
        question.of_package_id, w.new.0,
        "the pinned download, not the other file"
    );
    assert_eq!(question.of_file_name, NEW);
    assert_eq!(question.of_version.as_deref(), Some("1.1"));
    assert_eq!(
        (question.shared_files, question.files),
        (2, 2),
        "the script and the data; modinfo.ini and the screenshot never count"
    );
    assert!(entry(&view, &w.new).family.is_none());
    assert!(
        entry(&view, &w.new).version_question.is_none(),
        "a pinned package is never asked about"
    );
    assert!(entry(&view, &w.other).version_question.is_none());
}

#[test]
fn the_players_yes_joins_the_folder_into_the_family_and_it_can_be_put_away() {
    let w = word_bench();
    let state = found(&w);
    versions::join_versions_impl(&state, &w.folder.0, &w.new.0).unwrap();

    let view = library::get_library_impl(&state);
    let lead = entry(&view, &w.new)
        .family
        .as_ref()
        .expect("one family now");
    assert_eq!(
        lead.lead, w.new.0,
        "the joined version has no upload time, so it never leads"
    );
    assert_eq!(lead.older.len(), 1);
    assert_eq!(lead.older[0].package_id, w.folder.0);
    assert_eq!(lead.older[0].version, None, "a folder has no Nexus version");
    assert_eq!(
        lead.older[0].matches.len(),
        1,
        "the one option each holds is the same option"
    );
    let folder = entry(&view, &w.folder);
    assert_eq!(folder.family.as_ref().unwrap().lead, w.new.0);
    assert!(folder.version_question.is_none(), "answered");
    assert!(
        entry(&view, &w.other).family.is_none(),
        "another file of the same mod stays apart"
    );
    let rows = nexus::get_updates_impl(&state).rows;
    assert!(
        rows.iter().all(|row| row.package_id != w.folder.0),
        "one row per family"
    );
    let link = rows
        .iter()
        .find(|row| row.package_id == w.new.0)
        .unwrap()
        .older
        .first()
        .map(|older| older.package_id.clone());
    assert_eq!(link.as_deref(), Some(w.folder.0.as_str()));

    // nexus.json keeps the word, so a restart keeps the family.
    let on_disk: serde_json::Value =
        serde_json::from_slice(&fs::read(w.dirs.nexus_file()).unwrap()).unwrap();
    let record = &on_disk["packages"][&w.folder.0];
    assert_eq!(record["link"]["source"], "joined");
    assert_eq!(record["link"]["groupId"], GROUP);
    assert_eq!(record["link"]["fileId"], serde_json::Value::Null);
    assert_eq!(
        record["joinedOver"]["source"], "manual",
        "the link it had before"
    );
    drop(state);
    let state = w.open();
    let view = library::get_library_impl(&state);
    assert_eq!(entry(&view, &w.new).family.as_ref().unwrap().older.len(), 1);

    // The v0.5 machinery takes it from there: the folder is an older version with no install.
    let away = versions::put_away_impl(&state, &w.folder.0).unwrap();
    assert_eq!(away.landed_as, FOLDER);
    assert!(!w.bench.mods.join(FOLDER).exists());
    assert!(w
        .bench
        .mods
        .with_file_name("mods (older versions)")
        .join(FOLDER)
        .join(SCRIPT)
        .is_file());
}

#[test]
fn the_players_no_is_remembered_and_taking_a_join_back_counts_as_one() {
    let w = word_bench();
    let state = found(&w);
    versions::decline_version_question_impl(&state, &w.folder.0, &w.new.0).unwrap();
    let asked = |state: &AppState| {
        entry(&library::get_library_impl(state), &w.folder)
            .version_question
            .is_some()
    };
    assert!(!asked(&state), "a no takes the question away");
    drop(state);
    let state = w.open();
    assert!(!asked(&state), "and nexus.json remembers it");

    // A join needs no question, and taking it back restores the link it had.
    versions::join_versions_impl(&state, &w.folder.0, &w.new.0).unwrap();
    versions::part_versions_impl(&state, &w.folder.0).unwrap();
    let view = library::get_library_impl(&state);
    let folder = entry(&view, &w.folder);
    assert!(folder.family.is_none());
    assert!(folder.version_question.is_none(), "taken back is a no");
    let row = nexus::get_updates_impl(&state)
        .rows
        .into_iter()
        .find(|row| row.package_id == w.folder.0)
        .unwrap();
    let link = row.link.expect("still linked by mod id");
    assert_eq!(
        (link.mod_id, link.file_id, link.group_id),
        (MOD, None, None)
    );
    assert_eq!(link.source, riftstone_lib::dto::LinkSource::Manual);
    assert!(
        refusal(versions::part_versions_impl(&state, &w.folder.0)).contains("nothing to take back"),
        "only a joined link can be taken back"
    );
}

#[test]
fn the_word_is_refused_where_it_cannot_hold() {
    let w = word_bench();
    let state = w.open();
    // Not linked yet: the folder carries no mod.
    assert!(
        refusal(versions::join_versions_impl(&state, &w.folder.0, &w.new.0))
            .contains("not linked to a Nexus mod")
    );
    nexus::link_package_impl(&state, &w.folder.0, &MOD.to_string()).unwrap();
    // A package pinned to its own file says whose version it is.
    assert!(
        refusal(versions::join_versions_impl(&state, &w.new.0, &w.other.0))
            .contains("already pinned")
    );
    // The file it joins must name a group.
    assert!(refusal(versions::join_versions_impl(
        &state,
        &w.other.0,
        &w.folder.0
    ))
    .contains("not pinned to a file"));
    assert!(refusal(versions::join_versions_impl(
        &state,
        &w.folder.0,
        &w.folder.0
    ))
    .contains("version of itself"));
    // Another mod.
    nexus::link_package_impl(&state, &w.folder.0, "7141").unwrap();
    assert!(
        refusal(versions::join_versions_impl(&state, &w.folder.0, &w.new.0))
            .contains("different Nexus mods")
    );
    assert!(matches!(
        versions::join_versions_impl(&state, "00000000000000ff", &w.new.0),
        Err(RiftError::EntryNotFound { .. })
    ));
}

#[test]
fn names_that_do_not_match_ask_nothing_but_the_word_still_joins() {
    let w = word_bench();
    let state = w.open();
    // The ash patch's names are not the folder's: link the folder, decline the download's
    // group, and nothing is left to ask about.
    nexus::link_package_impl(&state, &w.folder.0, &MOD.to_string()).unwrap();
    versions::decline_version_question_impl(&state, &w.folder.0, &w.new.0).unwrap();
    let view = library::get_library_impl(&state);
    assert!(entry(&view, &w.folder).version_question.is_none());
    // The player may still say it.
    versions::join_versions_impl(&state, &w.folder.0, &w.other.0).unwrap();
    let view = library::get_library_impl(&state);
    assert_eq!(
        entry(&view, &w.folder).family.as_ref().unwrap().lead,
        w.other.0
    );
}

/// The older-versions folder, beside the bench's mods folder.
fn older_folder(w: &Word) -> std::path::PathBuf {
    w.bench.mods.with_file_name("mods (older versions)")
}

/// A player's own package already holds the folder's name there, so a put-away takes ` (2)`.
fn occupy_older_name(w: &Word) {
    let taken = older_folder(w).join(FOLDER);
    fs::create_dir_all(&taken).unwrap();
    fs::write(taken.join("existing.txt"), b"the player's own").unwrap();
}

/// RS-A2 (the Heretic's Assay, 2026-10-20): the record follows the new name onto disk before
/// the move, so a save that fails refuses the move instead of reporting a put-away that the next
/// launch forgets the join of.
#[test]
fn a_put_away_that_cannot_save_the_record_first_moves_nothing() {
    let w = word_bench();
    let state = found(&w);
    versions::join_versions_impl(&state, &w.folder.0, &w.new.0).unwrap();
    occupy_older_name(&w);
    let file = w.dirs.nexus_file();
    let durable = fs::read(&file).unwrap();
    // A write that fails at nexus.json, as a full disk or a refused permission does.
    fs::remove_file(&file).unwrap();
    fs::create_dir(&file).unwrap();

    match versions::put_away_impl(&state, &w.folder.0) {
        Err(RiftError::Io { detail }) => assert_eq!(
            detail,
            "its Nexus record could not be saved under the new name first"
        ),
        other => panic!("expected the move refused, got {other:?}"),
    }
    assert!(
        w.bench.mods.join(FOLDER).join(SCRIPT).is_file(),
        "still in the mods folder"
    );
    assert!(!older_folder(&w).join("Hearth Embers (2)").exists());
    let view = library::get_library_impl(&state);
    assert!(
        entry(&view, &w.folder).family.is_some(),
        "the record written ahead is taken back in memory too"
    );
    drop(state);

    fs::remove_dir(&file).unwrap();
    fs::write(&file, durable).unwrap();
    let state = w.open();
    let view = library::get_library_impl(&state);
    assert_eq!(
        entry(&view, &w.folder).family.as_ref().unwrap().lead,
        w.new.0
    );
}

#[test]
fn a_renamed_put_away_keeps_the_join_on_disk_across_a_restart_and_back() {
    let w = word_bench();
    let state = found(&w);
    versions::join_versions_impl(&state, &w.folder.0, &w.new.0).unwrap();
    occupy_older_name(&w);

    let away = versions::put_away_impl(&state, &w.folder.0).unwrap();
    assert_eq!(away.landed_as, "Hearth Embers (2)");
    let renamed = PackageId::for_folder(&away.landed_as);
    let on_disk: serde_json::Value =
        serde_json::from_slice(&fs::read(w.dirs.nexus_file()).unwrap()).unwrap();
    assert_eq!(on_disk["packages"][&renamed.0]["link"]["source"], "joined");
    assert!(
        on_disk["packages"].get(&w.folder.0).is_none(),
        "the old name's record goes once the move landed"
    );
    drop(state);

    let state = w.open();
    let shelf = versions::get_older_versions_impl(&state).unwrap();
    let row = shelf
        .items
        .iter()
        .find(|item| item.file_name == away.landed_as)
        .unwrap();
    assert!(
        row.newer_in_mods,
        "the restart still knows its newer version"
    );
    let back = versions::bring_back_impl(&state, &away.landed_as).unwrap();
    state.wait_until_scanned();
    let view = library::get_library_impl(&state);
    let returned = entry(&view, &PackageId::for_folder(&back.landed_as));
    assert_eq!(returned.family.as_ref().unwrap().lead, w.new.0);
}

#[test]
fn a_bring_back_that_cannot_save_the_record_first_stays_away() {
    let w = word_bench();
    let state = found(&w);
    versions::join_versions_impl(&state, &w.folder.0, &w.new.0).unwrap();
    let away = versions::put_away_impl(&state, &w.folder.0).unwrap();
    assert_eq!(away.landed_as, FOLDER);
    // Another folder of that name lands in the mods folder meanwhile: the way back takes ` (2)`.
    fs::create_dir_all(w.bench.mods.join(FOLDER)).unwrap();
    fs::write(w.bench.mods.join(FOLDER).join("readme.txt"), b"another").unwrap();
    let file = w.dirs.nexus_file();
    fs::remove_file(&file).unwrap();
    fs::create_dir(&file).unwrap();

    match versions::bring_back_impl(&state, &away.landed_as) {
        Err(RiftError::Io { detail }) => assert_eq!(
            detail,
            "its Nexus record could not be saved under the new name first"
        ),
        other => panic!("expected the move refused, got {other:?}"),
    }
    assert!(
        older_folder(&w).join(FOLDER).join(SCRIPT).is_file(),
        "still away"
    );
    assert!(!w.bench.mods.join("Hearth Embers (2)").exists());
}
