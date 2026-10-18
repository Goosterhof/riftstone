//! The Muster (v0.4) through the spine: invented saves in an invented save folder, named from an
//! invented game pak, read-only. Every person and thing here is invented (rift-muster's
//! fixtures).

mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use filetime::FileTime;
use rift_core::{RiftError, SaveReason};
use rift_muster::fixtures::{self, ACCOUNT};
use rift_muster::saves::SaveFolder;
use riftstone_lib::commands::muster::{
    get_muster_impl, get_muster_slots_impl, open_saves_folder_impl,
};
use riftstone_lib::dto::{MusterFolderProblem, MusterRole, NamesSource};
use riftstone_lib::persist::DataDirs;
use riftstone_lib::play::Launched;
use riftstone_lib::state::AppState;
use support::Bench;

/// A save folder at the place Steam keeps one, under the bench.
fn save_folder(bench: &Bench, account: u32) -> PathBuf {
    let folder = bench.dir.path().join(format!(
        "Steam/userdata/{account}/2054970/remote/win64_save"
    ));
    fs::create_dir_all(&folder).unwrap();
    folder
}

fn put(folder: &Path, name: &str, bytes: &[u8], minutes_ago: i64) {
    let path = folder.join(name);
    fs::write(&path, bytes).unwrap();
    let when = FileTime::from_unix_time(1_792_000_000 - minutes_ago * 60, 0);
    filetime::set_file_mtime(&path, when).unwrap();
}

/// A bench whose game folder holds the invented pak, with the invented party in two slots.
fn muster_bench() -> (Bench, PathBuf) {
    let bench = Bench::new();
    fs::write(bench.game.join("re_chunk_000.pak"), fixtures::game_pak()).unwrap();
    let folder = save_folder(&bench, ACCOUNT);
    put(&folder, "data009Slot.bin", &fixtures::save(9), 1);
    put(&folder, "data008Slot.bin", &fixtures::save(8), 60);
    // The game's system data and the character creator's files are no slots.
    put(&folder, "data00-1.bin", &fixtures::save(9), 0);
    put(&folder, "SS1_data000.bin", b"not a slot", 0);
    (bench, folder)
}

fn open(
    bench: &Bench,
    folders: Vec<SaveFolder>,
) -> (
    std::sync::Arc<AppState>,
    std::sync::Arc<riftstone_lib::events::RecordedEvents>,
) {
    let (state, events) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    state.fix_save_folders(folders);
    (state, events)
}

#[test]
fn the_slots_come_newest_first_with_their_arisen() {
    let (bench, folder) = muster_bench();
    let (state, _) = open(
        &bench,
        vec![SaveFolder {
            account_id: ACCOUNT,
            path: folder,
        }],
    );
    let view = get_muster_slots_impl(&state);
    assert_eq!(view.problem, None);
    assert_eq!(view.names, NamesSource::Game);
    assert_eq!(view.account_count, 1);
    let ids: Vec<&str> = view.slots.iter().map(|s| s.slot_id.as_str()).collect();
    assert_eq!(
        ids,
        ["data009Slot", "data008Slot"],
        "newest first; system data is no slot"
    );
    let first = &view.slots[0];
    assert_eq!((first.slot, first.account, first.problem), (9, None, None));
    let arisen = first.arisen.as_ref().unwrap();
    assert_eq!(arisen.name.as_deref(), Some("Odalric"));
    assert_eq!(
        (arisen.level, arisen.vocation.name.as_deref()),
        (41, Some("Warrior"))
    );
    assert!(first.saved_at.ends_with('Z'), "{}", first.saved_at);
}

#[test]
fn the_party_reads_whole_and_named_from_the_game() {
    let (bench, folder) = muster_bench();
    let (state, _) = open(
        &bench,
        vec![SaveFolder {
            account_id: ACCOUNT,
            path: folder,
        }],
    );
    let view = get_muster_impl(&state, "data009Slot").unwrap();
    assert_eq!(
        (view.names, view.names_note.as_deref()),
        (NamesSource::Game, None)
    );
    assert_eq!(
        (view.gold, view.rift_crystals),
        (Some(284_310), Some(6_420))
    );

    let arisen = view.arisen.unwrap();
    assert_eq!(arisen.role, MusterRole::Arisen);
    assert_eq!(arisen.member_key.len(), 16);
    let gear: Vec<Option<&str>> = arisen
        .gear
        .iter()
        .map(|g| g.item.as_ref().and_then(|i| i.name.as_deref()))
        .collect();
    assert_eq!(
        gear,
        [
            Some("Ashen Greatsword"),
            None,
            Some("Warden's Barbute"),
            Some("Lanternwright's Hauberk"),
            Some("Pilgrim's Greaves"),
            Some("Mantle of the Ninth Bell"),
            Some("Ring of Low Tides"),
            Some("Ring of the Still Hour"),
        ]
    );
    assert!(
        arisen.gear[1].item.is_none(),
        "the empty sub weapon is null, not a nameless item"
    );
    let augments: Vec<&str> = arisen
        .augments
        .iter()
        .filter_map(|a| a.name.as_deref())
        .collect();
    assert_eq!(augments, ["Ironhide", "Long Stride", "Second Wind"]);
    let vocations: Vec<(&str, i64)> = arisen
        .vocations
        .iter()
        .map(|v| (v.vocation.name.as_deref().unwrap(), v.experience))
        .collect();
    assert_eq!(
        vocations,
        [
            ("Warrior", 412_300),
            ("Fighter", 198_440),
            ("Archer", 36_010)
        ]
    );
    assert!(arisen.pawn.is_none() && arisen.creator.is_none());

    let wren = view.main_pawn.unwrap();
    let traits = wren.pawn.unwrap();
    assert_eq!(
        traits.inclination.and_then(|i| i.name).as_deref(),
        Some("Calm")
    );
    assert_eq!(
        traits.specialization.and_then(|s| s.name).as_deref(),
        Some("Healer")
    );
    let skills: Vec<&str> = wren
        .skills
        .iter()
        .filter_map(|s| s.name.as_deref())
        .collect();
    assert_eq!(skills, ["Shadow Feint", "Quick Cut", "Hook and Run"]);

    assert_eq!(view.hired.len(), 2);
    let bramble = &view.hired[0];
    assert_eq!(
        bramble
            .creator
            .as_ref()
            .map(|c| (c.level, c.vocation.name.as_deref())),
        Some((52, Some("Warrior")))
    );
    assert_eq!(
        bramble.skills[0].name.as_deref(),
        Some("Mending Light"),
        "a skill named through a reference"
    );
    assert!(bramble.pawn.as_ref().unwrap().specialization.is_none());
    let sable = &view.hired[1];
    let ring = sable.gear[6].item.as_ref().unwrap();
    assert_eq!(
        (ring.id, ring.name.as_deref()),
        (40_917, None),
        "an id the game's text lacks keeps its number"
    );
}

/// A save of another character on the same account: the invented party, its Arisen and main
/// pawn renamed (invented names too).
fn other_character(slot: i32) -> Vec<u8> {
    let mut party = fixtures::party();
    party[0].name = "Hester";
    party[1].name = "Tobin";
    rift_save::seal(&fixtures::payload(&party, slot, 1_200), ACCOUNT, true)
}

#[test]
fn slots_tell_their_character_by_the_arisen_with_their_main_pawn_within_one_account() {
    let (bench, folder) = muster_bench();
    put(&folder, "data005Slot.bin", &other_character(5), 30);
    let mut damaged = fixtures::save(3);
    let at = damaged.len() / 2;
    damaged[at] ^= 0x40;
    put(&folder, "data003Slot.bin", &damaged, 200);
    // The same two names on another account are another character.
    let other = save_folder(&bench, ACCOUNT + 7);
    put(
        &other,
        "data009Slot.bin",
        &rift_save::seal(
            &fixtures::payload(&fixtures::party(), 9, 5),
            ACCOUNT + 7,
            true,
        ),
        5,
    );
    let (state, _) = open(
        &bench,
        vec![
            SaveFolder {
                account_id: ACCOUNT,
                path: folder,
            },
            SaveFolder {
                account_id: ACCOUNT + 7,
                path: other,
            },
        ],
    );
    let view = get_muster_slots_impl(&state);
    let slot = |id: &str| {
        view.slots
            .iter()
            .find(|s| s.slot_id == id)
            .unwrap_or_else(|| panic!("{id} is listed"))
    };
    let odalric = slot("data009Slot@1").character.clone().unwrap();
    assert_eq!(odalric.len(), 16);
    assert!(
        odalric
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
        "16 lowercase hex digits"
    );
    assert_eq!(
        slot("data008Slot@1").character.as_deref(),
        Some(odalric.as_str()),
        "two saves of one party are one character"
    );
    let hester = slot("data005Slot@1").character.clone().unwrap();
    assert_ne!(hester, odalric, "another Arisen is another character");
    assert_ne!(
        slot("data009Slot@2").character.as_deref(),
        Some(odalric.as_str()),
        "the same names on another account are another character"
    );
    assert_eq!(
        slot("data003Slot@1").character,
        None,
        "a slot that would not open"
    );
    assert_eq!(
        slot("data009Slot@1").main_pawn_name.as_deref(),
        Some("Wren")
    );
    assert_eq!(
        slot("data005Slot@1").main_pawn_name.as_deref(),
        Some("Tobin")
    );
    assert_eq!(slot("data003Slot@1").main_pawn_name, None);
    // The key is made from private names and crosses only as the slot's own field.
    let wire = serde_json::to_string(&view).unwrap();
    assert_eq!(wire.matches(&odalric).count(), 2, "{wire}");
}

#[test]
fn every_problem_is_listed_and_voiced_by_its_reason() {
    let (bench, folder) = muster_bench();
    put(
        &folder,
        "data001Slot.bin",
        &rift_save::seal(
            &fixtures::payload(&fixtures::party(), 1, 1),
            ACCOUNT + 7,
            true,
        ),
        300,
    );
    let mut damaged = fixtures::save(3);
    let at = damaged.len() / 2;
    damaged[at] ^= 0x40;
    put(&folder, "data003Slot.bin", &damaged, 200);
    let (state, _) = open(
        &bench,
        vec![SaveFolder {
            account_id: ACCOUNT,
            path: folder,
        }],
    );
    let view = get_muster_slots_impl(&state);
    let problems: Vec<(&str, Option<SaveReason>)> = view
        .slots
        .iter()
        .map(|s| (s.slot_id.as_str(), s.problem))
        .collect();
    assert_eq!(
        problems,
        [
            ("data009Slot", None),
            ("data008Slot", None),
            ("data003Slot", Some(SaveReason::Damaged)),
            ("data001Slot", Some(SaveReason::OtherAccount)),
        ]
    );
    for (slot, reason) in [
        ("data003Slot", SaveReason::Damaged),
        ("data001Slot", SaveReason::OtherAccount),
    ] {
        match get_muster_impl(&state, slot) {
            Err(RiftError::Save {
                reason: got,
                detail,
            }) => {
                assert_eq!(got, reason);
                assert!(detail.contains(&format!("{slot}.bin")), "{detail}");
                assert!(!detail.contains("userdata"), "never the path: {detail}");
            }
            other => panic!("{slot}: {other:?}"),
        }
    }
}

#[test]
fn a_save_of_another_account_on_this_pc_opens_and_slot_ids_name_the_account() {
    let (bench, folder) = muster_bench();
    let other = save_folder(&bench, ACCOUNT + 7);
    put(
        &other,
        "data009Slot.bin",
        &rift_save::seal(
            &fixtures::payload(&fixtures::party(), 9, 5),
            ACCOUNT + 7,
            true,
        ),
        5,
    );
    // A copy of the second account's save in the first account's folder still opens.
    put(
        &folder,
        "data002Slot.bin",
        &rift_save::seal(
            &fixtures::payload(&fixtures::party(), 2, 5),
            ACCOUNT + 7,
            true,
        ),
        30,
    );
    let (state, _) = open(
        &bench,
        vec![
            SaveFolder {
                account_id: ACCOUNT,
                path: folder,
            },
            SaveFolder {
                account_id: ACCOUNT + 7,
                path: other,
            },
        ],
    );
    let view = get_muster_slots_impl(&state);
    assert_eq!(view.account_count, 2);
    let ids: Vec<(&str, Option<u32>)> = view
        .slots
        .iter()
        .map(|s| (s.slot_id.as_str(), s.account))
        .collect();
    assert_eq!(
        ids,
        [
            ("data009Slot@1", Some(1)),
            ("data009Slot@2", Some(2)),
            ("data002Slot@1", Some(1)),
            ("data008Slot@1", Some(1))
        ]
    );
    assert!(view.slots.iter().all(|s| s.problem.is_none()));
    assert_eq!(
        get_muster_impl(&state, "data009Slot@2").unwrap().gold,
        Some(5)
    );
}

#[test]
fn no_steam_and_no_saves_are_told_apart() {
    let bench = Bench::new();
    let (state, _) = bench.open(bench.config(DataDirs::under(&bench.data_home())));
    let view = get_muster_slots_impl(&state);
    assert_eq!(
        (view.problem, view.slots.len(), view.account_count),
        (Some(MusterFolderProblem::NoSteam), 0, 0)
    );
    let empty = save_folder(&bench, ACCOUNT);
    state.fix_save_folders(vec![SaveFolder {
        account_id: ACCOUNT,
        path: empty,
    }]);
    assert_eq!(
        get_muster_slots_impl(&state).problem,
        Some(MusterFolderProblem::NoSaves)
    );
}

#[test]
fn without_the_games_text_the_party_shows_by_its_numbers() {
    let (bench, folder) = muster_bench();
    fs::remove_file(bench.game.join("re_chunk_000.pak")).unwrap();
    let (state, _) = open(
        &bench,
        vec![SaveFolder {
            account_id: ACCOUNT,
            path: folder,
        }],
    );
    let view = get_muster_impl(&state, "data009Slot").unwrap();
    assert_eq!(view.names, NamesSource::Missing);
    assert!(view
        .names_note
        .as_deref()
        .unwrap()
        .contains("re_chunk_000.pak"));
    let arisen = view.arisen.unwrap();
    assert_eq!((arisen.vocation.id, arisen.vocation.name), (5, None));
    assert!(arisen
        .gear
        .iter()
        .filter_map(|g| g.item.as_ref())
        .all(|i| i.name.is_none()));
    assert_eq!(
        arisen.name.as_deref(),
        Some("Odalric"),
        "the save's own names still show"
    );
    assert_eq!(get_muster_slots_impl(&state).names, NamesSource::Missing);
}

#[test]
fn the_games_names_are_cached_beside_the_pak_size_and_time() {
    let (bench, folder) = muster_bench();
    let pak = bench.game.join("re_chunk_000.pak");
    let dirs = DataDirs::under(&bench.data_home());
    {
        let (state, _) = open(
            &bench,
            vec![SaveFolder {
                account_id: ACCOUNT,
                path: folder.clone(),
            }],
        );
        assert_eq!(
            get_muster_impl(&state, "data009Slot").unwrap().names,
            NamesSource::Game
        );
    }
    assert!(dirs
        .cache
        .join(riftstone_lib::muster::CODEX_CACHE)
        .is_file());
    // The same size and time with unreadable bytes: a fresh stone names from the cache.
    let stamp = FileTime::from_last_modification_time(&fs::metadata(&pak).unwrap());
    let size = fs::metadata(&pak).unwrap().len() as usize;
    fs::write(&pak, vec![0u8; size]).unwrap();
    filetime::set_file_mtime(&pak, stamp).unwrap();
    let (state, _) = open(
        &bench,
        vec![SaveFolder {
            account_id: ACCOUNT,
            path: folder.clone(),
        }],
    );
    assert_eq!(
        get_muster_impl(&state, "data009Slot").unwrap().names,
        NamesSource::Game
    );
    // A game update (a new time) reads the pak again; these bytes do not read.
    filetime::set_file_mtime(&pak, FileTime::from_unix_time(1_800_000_000, 0)).unwrap();
    let (state, _) = open(
        &bench,
        vec![SaveFolder {
            account_id: ACCOUNT,
            path: folder,
        }],
    );
    assert_eq!(
        get_muster_impl(&state, "data009Slot").unwrap().names,
        NamesSource::Missing
    );
}

#[test]
fn a_slot_that_left_the_folder_is_gone() {
    let (bench, folder) = muster_bench();
    let (state, _) = open(
        &bench,
        vec![SaveFolder {
            account_id: ACCOUNT,
            path: folder.clone(),
        }],
    );
    assert_eq!(get_muster_slots_impl(&state).slots.len(), 2);
    fs::remove_file(folder.join("data008Slot.bin")).unwrap();
    assert!(matches!(
        get_muster_impl(&state, "data008Slot"),
        Err(RiftError::Save {
            reason: SaveReason::Gone,
            ..
        })
    ));
    assert_eq!(get_muster_slots_impl(&state).slots.len(), 1);
}

#[test]
fn the_saves_folder_is_shown_and_its_path_never_crosses() {
    let (bench, folder) = muster_bench();
    let (state, _) = open(
        &bench,
        vec![SaveFolder {
            account_id: ACCOUNT,
            path: folder.clone(),
        }],
    );
    open_saves_folder_impl(&state, "data009Slot").unwrap();
    assert_eq!(
        bench.launcher.seen().last(),
        Some(&Launched::Folder(folder))
    );
    assert!(matches!(
        open_saves_folder_impl(&state, "data777Slot"),
        Err(RiftError::Save {
            reason: SaveReason::Gone,
            ..
        })
    ));
}

#[test]
fn the_window_hears_when_the_game_saves_and_reads_the_new_slot() {
    let (bench, folder) = muster_bench();
    let (state, events) = open(
        &bench,
        vec![SaveFolder {
            account_id: ACCOUNT,
            path: folder.clone(),
        }],
    );
    assert_eq!(
        get_muster_impl(&state, "data009Slot").unwrap().gold,
        Some(284_310)
    );
    // The game saves: a richer party, a second later.
    fs::write(
        folder.join("data009Slot.bin"),
        rift_save::seal(
            &fixtures::payload(&fixtures::party(), 9, 291_040),
            ACCOUNT,
            true,
        ),
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while events
        .named(riftstone_lib::events::MUSTER_CHANGED)
        .is_empty()
    {
        assert!(
            Instant::now() < deadline,
            "no rift://muster-changed after the game saved"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(
        get_muster_impl(&state, "data009Slot").unwrap().gold,
        Some(291_040)
    );
}

#[test]
fn reading_the_muster_writes_nothing_to_the_save_folder() {
    let (bench, folder) = muster_bench();
    let before: Vec<(String, Vec<u8>)> = {
        let mut files: Vec<_> = fs::read_dir(&folder)
            .unwrap()
            .flatten()
            .map(|e| {
                (
                    e.file_name().to_string_lossy().into_owned(),
                    fs::read(e.path()).unwrap(),
                )
            })
            .collect();
        files.sort();
        files
    };
    let (state, _) = open(
        &bench,
        vec![SaveFolder {
            account_id: ACCOUNT,
            path: folder.clone(),
        }],
    );
    get_muster_slots_impl(&state);
    for slot in ["data009Slot", "data008Slot"] {
        get_muster_impl(&state, slot).unwrap();
    }
    let mut after: Vec<_> = fs::read_dir(&folder)
        .unwrap()
        .flatten()
        .map(|e| {
            (
                e.file_name().to_string_lossy().into_owned(),
                fs::read(e.path()).unwrap(),
            )
        })
        .collect();
    after.sort();
    assert_eq!(before, after, "the Muster only reads (RS-8)");
}

// --- The General's review of PR #1 (2026-10-18) --------------------------------

/// The slot's problem in a fresh listing.
fn listed_problem(state: &AppState, slot: &str) -> Option<SaveReason> {
    get_muster_slots_impl(state)
        .slots
        .into_iter()
        .find(|s| s.slot_id == slot)
        .unwrap_or_else(|| panic!("{slot} is listed"))
        .problem
}

/// A slot another program holds for a moment (the game, Steam Cloud, an antivirus) is being
/// written, not gone, and it is read again the next time rather than remembered.
#[test]
fn a_slot_that_will_not_be_read_is_being_written_and_read_again() {
    let (bench, folder) = muster_bench();
    let slot = folder.join("data009Slot.bin");
    let (state, _) = open(
        &bench,
        vec![SaveFolder {
            account_id: ACCOUNT,
            path: folder.clone(),
        }],
    );
    let held = hold(&slot);
    assert_eq!(
        listed_problem(&state, "data009Slot"),
        Some(SaveReason::BeingWritten),
        "held, not gone"
    );
    match get_muster_impl(&state, "data009Slot") {
        Err(RiftError::Save { reason, .. }) => assert_eq!(reason, SaveReason::BeingWritten),
        other => panic!("{other:?}"),
    }
    drop(held);
    // Let go at the same size and time: it reads at once, so nothing held was remembered.
    assert_eq!(listed_problem(&state, "data009Slot"), None);
    assert!(get_muster_impl(&state, "data009Slot").is_ok());
}

/// Hold `path` the way another program can: Unix takes the read permission away, Windows opens
/// it with no sharing.
#[cfg(unix)]
fn hold(path: &Path) -> impl Drop {
    use std::os::unix::fs::PermissionsExt;
    struct Held(PathBuf);
    impl Drop for Held {
        fn drop(&mut self) {
            fs::set_permissions(&self.0, fs::Permissions::from_mode(0o644)).unwrap();
        }
    }
    fs::set_permissions(path, fs::Permissions::from_mode(0o000)).unwrap();
    assert!(
        fs::read(path).is_err(),
        "the bench runs as a user permissions bind (not root)"
    );
    Held(path.to_path_buf())
}

#[cfg(windows)]
fn hold(path: &Path) -> impl Drop {
    use std::os::windows::fs::OpenOptionsExt;
    struct Held(#[allow(dead_code)] fs::File);
    impl Drop for Held {
        fn drop(&mut self) {}
    }
    Held(
        fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(path)
            .unwrap(),
    )
}

/// The game empties a slot before it writes the new save: an empty file gets the same second
/// look as a short one instead of reading as damaged at once.
#[test]
fn a_slot_the_game_has_just_emptied_is_read_again_before_it_is_judged() {
    let (bench, folder) = muster_bench();
    let (state, _) = open(
        &bench,
        vec![SaveFolder {
            account_id: ACCOUNT,
            path: folder.clone(),
        }],
    );
    // The names are read first, so the slot's first read starts at once below.
    get_muster_impl(&state, "data008Slot").unwrap();
    let slot = folder.join("data009Slot.bin");
    let full = fs::read(&slot).unwrap();
    fs::write(&slot, b"").unwrap();
    let writer = std::thread::spawn({
        let slot = slot.clone();
        move || {
            std::thread::sleep(Duration::from_millis(50));
            fs::write(&slot, full).unwrap();
        }
    });
    let read = get_muster_impl(&state, "data009Slot");
    writer.join().unwrap();
    assert!(read.is_ok(), "read again once it was written: {read:?}");
}

/// A file far larger than any save is told as damaged from its size alone.
#[test]
fn a_file_larger_than_any_save_is_damaged_and_never_read_whole() {
    let (bench, folder) = muster_bench();
    let huge = folder.join("data004Slot.bin");
    // Sparse where the disk allows it: nothing is written.
    fs::File::create(&huge)
        .unwrap()
        .set_len(rift_save::MAX_FILE + 1)
        .unwrap();
    let (state, _) = open(
        &bench,
        vec![SaveFolder {
            account_id: ACCOUNT,
            path: folder.clone(),
        }],
    );
    assert_eq!(
        listed_problem(&state, "data004Slot"),
        Some(SaveReason::Damaged)
    );
    fs::remove_file(huge).unwrap();
}

/// A pak whose names will not read is read once per change of the pak, not on every call.
#[test]
fn a_pak_whose_names_do_not_read_is_not_read_again_until_it_changes() {
    let (bench, folder) = muster_bench();
    let pak = bench.game.join("re_chunk_000.pak");
    fs::write(&pak, vec![0u8; 4096]).unwrap();
    let (state, _) = open(
        &bench,
        vec![SaveFolder {
            account_id: ACCOUNT,
            path: folder,
        }],
    );
    for _ in 0..3 {
        assert_eq!(get_muster_slots_impl(&state).names, NamesSource::Missing);
    }
    assert_eq!(
        get_muster_impl(&state, "data009Slot").unwrap().names,
        NamesSource::Missing
    );
    assert_eq!(
        riftstone_lib::muster::codex_builds(&state),
        1,
        "the failed build is remembered"
    );
    filetime::set_file_mtime(&pak, FileTime::from_unix_time(1_800_000_000, 0)).unwrap();
    assert_eq!(get_muster_slots_impl(&state).names, NamesSource::Missing);
    assert_eq!(
        riftstone_lib::muster::codex_builds(&state),
        2,
        "a changed pak is read again"
    );
}

/// Two cold reads at once build the names once: the second waits for the first.
#[test]
fn two_cold_reads_build_the_names_once() {
    let (bench, folder) = muster_bench();
    let (state, _) = open(
        &bench,
        vec![SaveFolder {
            account_id: ACCOUNT,
            path: folder,
        }],
    );
    let start = std::sync::Arc::new(std::sync::Barrier::new(2));
    let readers: Vec<_> = (0..2)
        .map(|_| {
            let state = std::sync::Arc::clone(&state);
            let start = std::sync::Arc::clone(&start);
            std::thread::spawn(move || {
                start.wait();
                riftstone_lib::muster::codex(&state).is_ok()
            })
        })
        .collect();
    for reader in readers {
        assert!(reader.join().unwrap());
    }
    assert_eq!(riftstone_lib::muster::codex_builds(&state), 1);
}
