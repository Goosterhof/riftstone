//! An invented party sealed into an invented save, and an invented game pak to name it from.
//! Tests only. Every person, item, augment and skill here is invented; the vocations wear plain
//! English words or invented ones, never the game's own coinages (BUILD-SPEC § v0.4, ruling 2).

use rift_pak::game::fixture::{pak, Stored};
use rift_pak::msg;
use rift_save::stream::write::{stream, WClass, W};

/// The invented account the fixtures are sealed for.
pub const ACCOUNT: u32 = 4_242_001;

/// The game's character ids for the Arisen and the main pawn (`app.CharacterID`), and two
/// invented ones for the hired places.
pub const ARISEN_ID: i32 = -1_403_890_315;
pub const MAIN_PAWN_ID: i32 = -2_011_938_949;
pub const HIRED_IDS: [i32; 2] = [1_111_190, 1_111_160];

/// One invented member, as the fixture writes it.
#[derive(Debug, Clone)]
pub struct Invented {
    pub name: &'static str,
    pub character_id: i32,
    pub row: u8,
    pub level: i32,
    pub experience: i32,
    pub vocation: i32,
    pub vocation_experience: [i32; 10],
    /// Health, stamina, strength, defence, magick, magick defence, weight, knockdown.
    pub gains: [i32; 8],
    /// Item ids by slot; 0 is empty.
    pub gear: [i32; 8],
    /// Expand-class items among the gear: the slots whose item is stored as the subclass.
    pub expand_slots: &'static [usize],
    pub augments: [i32; 7],
    pub skills: [i32; 6],
    pub kind: Kind,
}

#[derive(Debug, Clone)]
pub enum Kind {
    Arisen,
    MainPawn {
        personality: i64,
        specialization: i32,
    },
    Hired {
        personality: i64,
        specialization: i32,
        creator_level: i32,
        creator_vocation: i32,
    },
}

/// The invented party: Odalric the Arisen, Wren the main pawn, Bramble and Sable hired.
pub fn party() -> Vec<Invented> {
    vec![
        Invented {
            name: "Odalric",
            character_id: ARISEN_ID,
            row: 1,
            level: 41,
            experience: 1_284_550,
            vocation: 5,
            vocation_experience: [198_440, 36_010, 0, 0, 412_300, 0, 0, 0, 0, 0],
            gains: [1_240, 610, 312, 268, 96, 118, 6, 140],
            gear: [9001, 0, 9003, 9004, 9005, 9006, 9007, 9008],
            expand_slots: &[3, 4],
            augments: [4, 24, 25, 0, 0, 0, 0],
            skills: [52, 53, 54, 0, 0, 0],
            kind: Kind::Arisen,
        },
        Invented {
            name: "Wren",
            character_id: MAIN_PAWN_ID,
            row: 2,
            level: 39,
            experience: 1_102_300,
            vocation: 4,
            vocation_experience: [0, 120_200, 0, 388_900, 0, 0, 0, 0, 0, 0],
            gains: [880, 940, 276, 172, 88, 104, 2, 60],
            gear: [9011, 0, 9013, 9014, 9015, 9016, 9017, 0],
            expand_slots: &[],
            augments: [19, 20, 0, 0, 0, 0, 0],
            skills: [38, 40, 101, 0, 0, 0],
            kind: Kind::MainPawn {
                personality: -477_469_187,
                specialization: 2,
            },
        },
        Invented {
            name: "Bramble",
            character_id: HIRED_IDS[0],
            row: 3,
            level: 49,
            experience: 2_048_900,
            vocation: 6,
            vocation_experience: [0, 0, 310_500, 0, 0, 702_000, 0, 0, 0, 0],
            gains: [760, 540, 98, 120, 486, 402, 1, 40],
            gear: [9021, 0, 9023, 9024, 9025, 0, 9027, 0],
            expand_slots: &[],
            augments: [29, 30, 31, 32, 0, 0, 0],
            skills: [62, 63, 0, 0, 0, 0],
            kind: Kind::Hired {
                personality: 81_586_066,
                specialization: 0,
                creator_level: 52,
                creator_vocation: 5,
            },
        },
        Invented {
            name: "Sable",
            character_id: HIRED_IDS[1],
            row: 4,
            level: 30,
            experience: 612_400,
            vocation: 3,
            vocation_experience: [0, 0, 244_100, 0, 0, 0, 0, 0, 0, 0],
            gains: [520, 430, 74, 86, 262, 248, 1, 30],
            gear: [9031, 0, 9033, 9034, 9035, 9036, 40_917, 9038],
            expand_slots: &[],
            augments: [14, 0, 0, 0, 0, 0, 0],
            skills: [24, 25, 0, 0, 0, 0],
            kind: Kind::Hired {
                personality: 1_477_799_926,
                specialization: 9,
                creator_level: 30,
                creator_vocation: 2,
            },
        },
    ]
}

fn item(chara: i32, id: i32, slot: usize, expand: bool) -> WClass {
    let class = if expand {
        "app.ItemManager.ExpandItemStorageBackup"
    } else {
        "app.ItemManager.ItemStorageBackup"
    };
    let mut c = WClass::new(class);
    if expand {
        c = c.with("_ExpandValues", W::I32Array(vec![3, 1]));
    }
    c.with("_StorageId", W::I32(0))
        .with("_CharaId", W::U32(chara as u32))
        .with("_ItemId", W::I32(id))
        .with("_Num", W::I32(1))
        .with("_EquipSlot", W::U8(slot as u8))
        .with("_IsEquipped", W::Bool(true))
}

fn record(m: &Invented) -> WClass {
    let mut contexts = vec![
        WClass::new("app.CharacterIDContext").with("CharacterID", W::I32(m.character_id)),
        WClass::new("app.UniqueIDContext").with(
            "UniqueID",
            W::Class(
                WClass::new("app.UniqueID")
                    .with("_RowID", W::Struct(vec![m.row; 16]))
                    .with("_Index", W::I32(i32::from(m.row))),
            ),
        ),
        WClass::new("app.JobContext")
            .with("CurrentJob", W::I32(m.vocation))
            .with("CulmativeExp", W::I32Array(m.vocation_experience.to_vec())),
        WClass::new("app.HumanStatusContext")
            .with("_Level", W::I32(m.level))
            .with("_Exp", W::I32(m.experience))
            .with(
                "_IncreaseParams",
                W::Class(
                    [
                        "_HitPoint",
                        "_Stamina",
                        "_Attack",
                        "_Defence",
                        "_MagicAttack",
                        "_MagicDefence",
                        "_Weight",
                        "_Blow",
                    ]
                    .iter()
                    .zip(m.gains)
                    .fold(WClass::new("app.IncreaseStatus"), |c, (name, v)| {
                        c.with(name, W::I32(v))
                    }),
                ),
            ),
        WClass::new("app.AbilityContext").with(
            "EquipedAbilities",
            W::ClassArray(
                (0..11)
                    .map(|v| {
                        let list = if v == m.vocation {
                            m.augments.to_vec()
                        } else {
                            vec![0; 7]
                        };
                        WClass::new("app.AbilityContext.EquipedAbility")
                            .with("Abilities", W::I32Array(list))
                    })
                    .collect(),
                true,
            ),
        ),
        WClass::new("app.HumanSkillContext").with(
            "EquipedSkills",
            W::ClassArray(
                (0..11)
                    .map(|v| {
                        let list = if v == m.vocation {
                            m.skills.to_vec()
                        } else {
                            vec![0; 6]
                        };
                        WClass::new("app.HumanSkillContext.EquipedSkill")
                            .with("Skills", W::I32Array(list))
                    })
                    .collect(),
                false,
            ),
        ),
    ];
    let pawn = |personality: i64, specialization: i32| {
        WClass::new("app.PawnDataContext")
            .with("_Name", W::Str(m.name.into()))
            .with("_Personality", W::I64(personality))
            .with("_SubSkill", W::I32(specialization))
    };
    match &m.kind {
        Kind::Arisen => contexts
            .push(WClass::new("app.PlayerPersonalDataContext").with("Name", W::Str(m.name.into()))),
        Kind::MainPawn {
            personality,
            specialization,
        } => {
            contexts.push(pawn(*personality, *specialization));
            contexts.push(
                WClass::new("app.MainPawnDataContext").with("_FavorabilityRating", W::F32(0.5)),
            );
        }
        Kind::Hired {
            personality,
            specialization,
            creator_level,
            creator_vocation,
        } => {
            contexts.push(pawn(*personality, *specialization));
            contexts.push(WClass::new("app.SubPawnDataContext").with("_HireTime", W::F64(1.0)));
            contexts.push(
                WClass::new("app.PawnOwnerContext")
                    .with("_Name", W::Str("An Invented Creator".into()))
                    .with("_Level", W::I32(*creator_level))
                    .with("_Job", W::I32(*creator_vocation)),
            );
        }
    }
    WClass::new("app.ContextDatabaseSaveDataRecord").with("Contexts", W::ClassArray(contexts, true))
}

/// The payload of an invented save holding `members`, in slot `slot`.
pub fn payload(members: &[Invented], slot: i32, gold: i64) -> Vec<u8> {
    let mut items = Vec::new();
    for m in members {
        for (s, &id) in m.gear.iter().enumerate() {
            if id > 0 {
                items.push(item(m.character_id, id, s, m.expand_slots.contains(&s)));
            }
        }
    }
    // An unequipped item the reader must leave alone.
    items.push(item(ARISEN_ID, 9999, 0, false).with("_IsEquipped", W::Bool(false)));
    let manager = WClass::new("app.ItemManager.GameSaveData")
        .with("ItemStorageSaves", W::ClassArray(items, false))
        .with(
            "MoneySave",
            W::Class(
                WClass::new("app.ItemManager.MoneyBackup")
                    .with("_Gold", W::I64(gold))
                    .with("_Rim", W::I64(6_420)),
            ),
        );
    let records = WClass::new("app.ContextDatabaseSaveData").with(
        "Records",
        W::ClassArray(
            std::iter::once(
                WClass::new("app.ContextDatabaseSaveDataRecord")
                    .with("Contexts", W::ClassArray(vec![], true)),
            )
            .chain(members.iter().map(record))
            .collect(),
            false,
        ),
    );
    let list = WClass::new("app.SaveDataManager.GameSaveData").with(
        "GameSaveDataList",
        W::ClassArray(
            vec![
                manager,
                WClass::new("app.ContextDBMSSaveData").with("DBSaveData", W::Class(records)),
            ],
            false,
        ),
    );
    let detail = WClass::new("via.storage.saveService.SaveFileDetail")
        .with("Title", W::Str("An Invented Title".into()))
        .with("Detail", W::Str("Invented 1/1".into()))
        .with("Slot", W::I32(slot));
    stream(&[("SaveData", list), ("SaveFileDetail", detail)])
}

/// The invented party sealed into a 2026-style slot (`0x18`) for [`ACCOUNT`].
pub fn save(slot: i32) -> Vec<u8> {
    rift_save::seal(&payload(&party(), slot, 284_310), ACCOUNT, true)
}

/// An invented game pak holding the seventeen texts' invented names.
pub fn game_pak() -> Vec<u8> {
    let langs = [0u32, 1];
    let items = msg::fixture(
        &langs,
        &[
            ("item_name_9001", &["", "Ashen Greatsword"]),
            ("item_name_9003", &["", "Warden's Barbute"]),
            ("item_name_9004", &["", "Lanternwright's Hauberk"]),
            ("item_name_9005", &["", "Pilgrim's Greaves"]),
            ("item_name_9006", &["", "Mantle of the Ninth Bell"]),
            ("item_name_9007", &["", "Ring of Low Tides"]),
            ("item_name_9008", &["", "Ring of the Still Hour"]),
            ("item_name_9011", &["", "Twin Fangs of Harrowmere"]),
            ("item_name_9013", &["", "Fieldmouse Hood"]),
            ("item_name_9014", &["", "Moth-Grey Jerkin"]),
            ("item_name_9015", &["", "Brambleweave Breeches"]),
            ("item_name_9016", &["", "Dusk Shawl"]),
            ("item_name_9017", &["", "Thimble Ring"]),
        ],
    );
    let items_end = msg::fixture(
        &langs,
        &[
            ("item_name_9021", &["", "Staff of the Ninth Ember"]),
            ("item_name_9023", &["", "Cowl of the Grey Choir"]),
            ("item_name_9024", &["", "Robe of Banked Coals"]),
            ("item_name_9025", &["", "Ashwalker's Wraps"]),
            ("item_name_9027", &["", "Ring of the Cold Hearth"]),
            ("item_name_9031", &["", "Wand of Quiet Rain"]),
            ("item_name_9033", &["", "Lamplighter's Cap"]),
            ("item_name_9034", &["", "Wayfarer's Tunic"]),
            ("item_name_9035", &["", "Wayfarer's Leggings"]),
            ("item_name_9036", &["", "Mantle of Small Hours"]),
            ("item_name_9038", &["", "Ring of Lantern Moss"]),
        ],
    );
    let jobs = msg::fixture(
        &langs,
        &[
            ("job_name_01", &["", "Fighter"]),
            ("job_name_02", &["", "Archer"]),
            ("job_name_03", &["", "Mage"]),
            ("job_name_04", &["", "Thief"]),
            ("job_name_05", &["", "Warrior"]),
            ("job_name_06", &["", "Sorcerer"]),
        ],
    );
    let personality = msg::fixture(
        &langs,
        &[
            ("Personality_00", &["", "Kind"]),
            ("Personality_01", &["", "Calm"]),
            ("Personality_02", &["", "Simple"]),
        ],
    );
    let subskill = msg::fixture(
        &langs,
        &[
            ("Subskill_01_00", &["", "Healer"]),
            ("Subskill_01_01", &["", "Mends."]),
        ],
    );
    let subskill_tu = msg::fixture(&langs, &[("Subskill_08_00", &["", "Scholar"])]);
    let job03 = msg::fixture(
        &langs,
        &[
            ("job03_ability_00", &["", "Soft Tread"]),
            ("job03_skill_00", &["", "Mending Light"]),
            ("job03_skill_01", &["", "Frost Thread"]),
        ],
    );
    let job04 = msg::fixture(
        &langs,
        &[
            ("job04_ability_00", &["", "Light Step"]),
            ("job04_ability_01", &["", "Keen Ear"]),
            ("job04_skill_00", &["", "Shadow Feint"]),
            ("job04_skill_02", &["", "Quick Cut"]),
            ("job04_skill_12", &["", "Hook and Run"]),
        ],
    );
    let job05 = msg::fixture(
        &langs,
        &[
            ("job05_ability_00", &["", "Long Stride"]),
            ("job05_ability_01", &["", "Second Wind"]),
            ("job05_skill_02", &["", "Riven Arc"]),
            ("job05_skill_03", &["", "Cinder Rush"]),
            ("job05_skill_04", &["", "Bulwark Turn"]),
        ],
    );
    let job01 = msg::fixture(&langs, &[("job01_ability_00", &["", "Ironhide"])]);
    let job06 = msg::fixture(
        &langs,
        &[
            ("job06_ability_00", &["", "Deep Well"]),
            ("job06_ability_01", &["", "Steady Hand"]),
            ("job06_ability_02", &["", "Slow Burn"]),
            ("job06_ability_03", &["", "Far Sight"]),
            ("job06_skill_00", &["", "<REF job03_skill_00>"]),
            ("job06_skill_01", &["", "Ember Lance"]),
        ],
    );
    pak(
        &[
            (
                "natives/stm/message/ui/itemname.msg.24",
                &items,
                Stored::Zstd,
            ),
            (
                "natives/stm/message/ui/itemname_end.msg.24",
                &items_end,
                Stored::Chunked,
            ),
            ("natives/stm/message/ui/job.msg.24", &jobs, Stored::Deflate),
            (
                "natives/stm/message/ui/personality.msg.24",
                &personality,
                Stored::Raw,
            ),
            (
                "natives/stm/message/ui/subskill.msg.24",
                &subskill,
                Stored::Zstd,
            ),
            (
                "natives/stm/message/ui/subskill_tu_end.msg.24",
                &subskill_tu,
                Stored::Zstd,
            ),
            (
                "natives/stm/message/ui/job01act.msg.24",
                &job01,
                Stored::Zstd,
            ),
            (
                "natives/stm/message/ui/job03act.msg.24",
                &job03,
                Stored::Zstd,
            ),
            (
                "natives/stm/message/ui/job04act.msg.24",
                &job04,
                Stored::Zstd,
            ),
            (
                "natives/stm/message/ui/job05act.msg.24",
                &job05,
                Stored::Zstd,
            ),
            (
                "natives/stm/message/ui/job06act.msg.24",
                &job06,
                Stored::Zstd,
            ),
        ],
        11,
    )
}
