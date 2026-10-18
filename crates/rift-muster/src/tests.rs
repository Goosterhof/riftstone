//! The Muster read from an invented save, named from an invented game pak.

use rift_pak::game::GamePak;
use rift_save::stream::write::{stream, WClass, W};

use crate::fixtures::{self, ACCOUNT};
use crate::{read_slot, Codex, Inclination, MusterError, Role, SlotError};

fn invented_codex() -> Codex {
    let mut file = tempfile::NamedTempFile::new().expect("temp file");
    std::io::Write::write_all(&mut file, &fixtures::game_pak()).expect("write");
    let pak = GamePak::open(file.path()).expect("the invented pak opens");
    Codex::from_pak(&pak).expect("the codex builds")
}

#[test]
fn the_invented_party_reads_whole() {
    let muster = read_slot(&fixtures::save(9), ACCOUNT).unwrap();
    assert_eq!(muster.slot, 9);
    assert_eq!(
        (muster.gold, muster.rift_crystals),
        (Some(284_310), Some(6_420))
    );

    let arisen = muster.arisen.as_ref().unwrap();
    assert_eq!(arisen.role, Role::Arisen);
    assert_eq!(arisen.name.as_ref().map(|n| n.expose()), Some("Odalric"));
    assert_eq!(
        (arisen.level, arisen.experience, arisen.vocation),
        (41, 1_284_550, 5)
    );
    assert_eq!(arisen.vocation_experience[4], 412_300);
    assert_eq!(arisen.gains.health, 1_240);
    assert_eq!(arisen.gains.knockdown, 140);
    assert_eq!(
        arisen.gear,
        [
            Some(9001),
            None,
            Some(9003),
            Some(9004),
            Some(9005),
            Some(9006),
            Some(9007),
            Some(9008)
        ]
    );
    assert_eq!(arisen.augments, [4, 24, 25]);
    assert_eq!(arisen.skills, [52, 53, 54]);
    assert!(arisen.pawn.is_none() && arisen.creator.is_none());

    let wren = muster.main_pawn.as_ref().unwrap();
    assert_eq!(wren.role, Role::MainPawn);
    let traits = wren.pawn.unwrap();
    assert_eq!(
        (traits.inclination, traits.specialization),
        (Some(Inclination::Cool), Some(2))
    );
    assert_eq!(wren.gear[1], None, "an empty sub weapon stays empty");
    assert_eq!(wren.gear[7], None);

    assert_eq!(muster.hired.len(), 2);
    let bramble = &muster.hired[0];
    assert_eq!(bramble.role, Role::HiredPawn);
    assert_eq!(
        bramble.pawn.unwrap().specialization,
        None,
        "no specialization reads as none"
    );
    assert_eq!(
        bramble.creator.map(|c| (c.level, c.vocation)),
        Some((52, 5))
    );
    assert_ne!(
        bramble.key, muster.hired[1].key,
        "every member has its own key"
    );
}

#[test]
fn armour_stored_as_the_expand_subclass_is_still_worn() {
    // The investor's own Arisen's body and leg armour sat in ExpandItemStorageBackup.
    let muster = read_slot(&fixtures::save(9), ACCOUNT).unwrap();
    let arisen = muster.arisen.unwrap();
    assert_eq!((arisen.gear[3], arisen.gear[4]), (Some(9004), Some(9005)));
}

#[test]
fn an_unequipped_item_is_not_worn() {
    let muster = read_slot(&fixtures::save(9), ACCOUNT).unwrap();
    assert!(!muster.arisen.unwrap().gear.contains(&Some(9999)));
}

#[test]
fn a_member_keeps_its_key_across_a_re_read() {
    let a = read_slot(&fixtures::save(9), ACCOUNT).unwrap();
    let b = read_slot(&fixtures::save(9), ACCOUNT).unwrap();
    assert_eq!(a.hired[1].key, b.hired[1].key);
}

#[test]
fn the_codex_names_everything_its_texts_hold() {
    let codex = invented_codex();
    assert_eq!(codex.item(9004), Some("Lanternwright's Hauberk"));
    assert_eq!(
        codex.item(9036),
        Some("Mantle of Small Hours"),
        "items from the chunked file too"
    );
    assert_eq!(
        codex.item(40_917),
        None,
        "an id the texts lack stays unnamed"
    );
    assert_eq!(codex.vocation(5), Some("Warrior"));
    assert_eq!(codex.inclination(Inclination::Cool), Some("Calm"));
    assert_eq!(codex.inclination(Inclination::Frank), None);
    assert_eq!(codex.specialization(2), Some("Healer"));
    assert_eq!(
        codex.specialization(9),
        Some("Scholar"),
        "from the title update's file"
    );
    assert_eq!(codex.augment(4), Some("Ironhide"));
    assert_eq!(codex.augment(24), Some("Long Stride"));
    assert_eq!(codex.augment(29), Some("Deep Well"));
    assert_eq!(codex.skill(52), Some("Riven Arc"));
    assert_eq!(
        codex.skill(101),
        Some("Hook and Run"),
        "the appended Thief skill"
    );
    assert_eq!(
        codex.skill(62),
        Some("Mending Light"),
        "a skill whose text is a reference"
    );
    assert_eq!(codex.skill(500), None);
    assert_eq!(codex.counts()[0], 24);
}

#[test]
fn the_codex_survives_a_cache_round_trip() {
    let codex = invented_codex();
    let json = serde_json::to_string(&codex).unwrap();
    assert_eq!(serde_json::from_str::<Codex>(&json).unwrap(), codex);
}

#[test]
fn a_name_never_reaches_a_debug_print() {
    let muster = read_slot(&fixtures::save(9), ACCOUNT).unwrap();
    let printed = format!("{muster:?}");
    for name in ["Odalric", "Wren", "Bramble", "Sable", "An Invented Creator"] {
        assert!(!printed.contains(name), "{name} leaked into Debug");
    }
}

#[test]
fn the_system_data_file_is_no_slot() {
    let payload = fixtures::payload(&fixtures::party(), -1, 0);
    let save = rift_save::seal(&payload, ACCOUNT, false);
    assert_eq!(
        read_slot(&save, ACCOUNT),
        Err(SlotError::Muster(MusterError::SystemData))
    );
}

#[test]
fn a_save_without_a_party_says_so() {
    let detail = WClass::new("via.storage.saveService.SaveFileDetail").with("Slot", W::I32(3));
    let save = rift_save::seal(&stream(&[("SaveFileDetail", detail)]), ACCOUNT, true);
    assert_eq!(
        read_slot(&save, ACCOUNT),
        Err(SlotError::Muster(MusterError::NoParty))
    );
}

#[test]
fn another_accounts_slot_is_a_save_error() {
    assert_eq!(
        read_slot(&fixtures::save(9), ACCOUNT + 1),
        Err(SlotError::Save(rift_save::SaveError::WrongAccount))
    );
}
