//! The party in a parsed save: who stands at the stone, and what each one carries.
//!
//! Every fact here was read off the investor's own save (BUILD-SPEC § v0.4, The ground truth)
//! and is found by class and field NAME: the stream files a class under the MurmurHash3 of its
//! type name and a field under that of its field name, so this module carries names, never
//! magic numbers. It walks every class rather than one path, so a title update that moves a
//! list does not lose the party.

use std::collections::HashMap;

use rift_save::{field_hash, Class, Root, Value};

use crate::private::Private;

/// How many vocations the game has.
pub const VOCATIONS: usize = 10;

/// The eight gear slots in the game's order (`_EquipSlot` 0–7).
pub const GEAR_SLOTS: usize = 8;

/// One save slot's party.
#[derive(Debug, Clone, PartialEq)]
pub struct Muster {
    /// The game's slot number (`SaveFileDetail.Slot`).
    pub slot: i64,
    pub arisen: Option<Member>,
    pub main_pawn: Option<Member>,
    /// In the order the save keeps their records.
    pub hired: Vec<Member>,
    pub gold: Option<i64>,
    pub rift_crystals: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Arisen,
    MainPawn,
    HiredPawn,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    /// Stable for one pawn across re-reads: FNV-1a 64 of the record's own row id.
    pub key: u64,
    pub role: Role,
    pub name: Option<Private>,
    pub level: i64,
    pub experience: i64,
    /// The current vocation, 1–10 (0 when the save names none).
    pub vocation: u32,
    /// Experience per vocation: index 0 is vocation 1.
    pub vocation_experience: [i64; VOCATIONS],
    pub gains: Gains,
    /// Item ids by slot: main weapon, sub weapon, head, body, legs, cloak, ring, ring.
    pub gear: [Option<i64>; GEAR_SLOTS],
    /// The current vocation's augments, in the game's order, empty places left out.
    pub augments: Vec<u32>,
    /// The current vocation's skills, likewise.
    pub skills: Vec<u32>,
    pub pawn: Option<PawnTraits>,
    pub creator: Option<Creator>,
}

/// What the levels gave (`HumanStatusContext._IncreaseParams`), the integers the save keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Gains {
    pub health: i64,
    pub stamina: i64,
    pub strength: i64,
    pub defence: i64,
    pub magick: i64,
    pub magick_defence: i64,
    pub weight: i64,
    pub knockdown: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PawnTraits {
    pub inclination: Option<Inclination>,
    /// The game's specialization number, 1 and up; `None` when the pawn has none.
    pub specialization: Option<u32>,
}

/// A pawn's inclination. The save keeps the game's PersonalityID values (these four,
/// measured); the game's text calls them Kindhearted, Calm, Simple and Straightforward.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inclination {
    Kind,
    Cool,
    Simple,
    Frank,
}

impl Inclination {
    pub fn from_id(id: i64) -> Option<Inclination> {
        match id {
            1_477_799_926 => Some(Inclination::Kind),
            -477_469_187 => Some(Inclination::Cool),
            81_586_066 => Some(Inclination::Simple),
            1_301_972_698 => Some(Inclination::Frank),
            _ => None,
        }
    }

    pub fn id(self) -> i64 {
        match self {
            Inclination::Kind => 1_477_799_926,
            Inclination::Cool => -477_469_187,
            Inclination::Simple => 81_586_066,
            Inclination::Frank => 1_301_972_698,
        }
    }

    /// Its place in the game's text (`Personality_00` … `_03`).
    pub fn index(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Creator {
    pub level: i64,
    pub vocation: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MusterError {
    #[error("the save holds no party")]
    NoParty,
    #[error("the file is the game's system data, not a save slot")]
    SystemData,
}

struct Names {
    detail: u32,
    record: u32,
    item_manager: u32,
    player: u32,
    main_pawn: u32,
    sub_pawn: u32,
    pawn_data: u32,
    pawn_owner: u32,
    status: u32,
    job: u32,
    ability: u32,
    skill: u32,
    character_id: u32,
    unique_id: u32,
}

impl Names {
    fn new() -> Self {
        Names {
            detail: field_hash("via.storage.saveService.SaveFileDetail"),
            record: field_hash("app.ContextDatabaseSaveDataRecord"),
            item_manager: field_hash("app.ItemManager.GameSaveData"),
            player: field_hash("app.PlayerPersonalDataContext"),
            main_pawn: field_hash("app.MainPawnDataContext"),
            sub_pawn: field_hash("app.SubPawnDataContext"),
            pawn_data: field_hash("app.PawnDataContext"),
            pawn_owner: field_hash("app.PawnOwnerContext"),
            status: field_hash("app.HumanStatusContext"),
            job: field_hash("app.JobContext"),
            ability: field_hash("app.AbilityContext"),
            skill: field_hash("app.HumanSkillContext"),
            character_id: field_hash("app.CharacterIDContext"),
            unique_id: field_hash("app.UniqueIDContext"),
        }
    }
}

fn int(class: &Class, name: &str) -> Option<i64> {
    class.get(name).and_then(Value::as_i64)
}

fn text(class: &Class, name: &str) -> Option<Private> {
    class
        .get(name)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(Private::new)
}

fn array<'a>(class: &'a Class, name: &str) -> &'a [Value] {
    class.get(name).and_then(Value::as_array).unwrap_or(&[])
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The ids set for one vocation in a per-vocation list (`EquipedAbilities` → `Abilities`),
/// zeros (empty places) left out.
fn loadout(list: &[Value], vocation: u32, inner: &str) -> Vec<u32> {
    list.get(vocation as usize)
        .and_then(Value::as_class)
        .map(|c| {
            array(c, inner)
                .iter()
                .filter_map(Value::as_i64)
                .filter(|&id| id > 0)
                .map(|id| id as u32)
                .collect()
        })
        .unwrap_or_default()
}

/// Read the party out of a parsed save.
pub fn read(roots: &[Root]) -> Result<Muster, MusterError> {
    let n = Names::new();
    let slot = roots
        .iter()
        .find(|(_, c)| c.type_hash == n.detail)
        .and_then(|(_, c)| int(c, "Slot"))
        .unwrap_or(0);
    if slot < 0 {
        return Err(MusterError::SystemData);
    }

    let mut records: Vec<HashMap<u32, &Class>> = Vec::new();
    let mut gear: HashMap<u32, [Option<i64>; GEAR_SLOTS]> = HashMap::new();
    let (mut gold, mut rift_crystals) = (None, None);
    rift_save::stream::walk(roots, &mut |class| {
        if class.type_hash == n.record {
            let contexts = array(class, "Contexts")
                .iter()
                .filter_map(Value::as_class)
                .map(|c| (c.type_hash, c))
                .collect();
            records.push(contexts);
        } else if class.type_hash == n.item_manager {
            for item in array(class, "ItemStorageSaves")
                .iter()
                .filter_map(Value::as_class)
            {
                // Plain items and their Expand subclass alike: whatever carries the fields.
                let equipped = item.get("_IsEquipped").and_then(Value::as_bool) == Some(true);
                let (Some(chara), Some(id), Some(slot)) = (
                    int(item, "_CharaId"),
                    int(item, "_ItemId"),
                    int(item, "_EquipSlot"),
                ) else {
                    continue;
                };
                if equipped && (0..GEAR_SLOTS as i64).contains(&slot) && id > 0 {
                    let place = &mut gear.entry(chara as u32).or_default()[slot as usize];
                    place.get_or_insert(id);
                }
            }
            if let Some(money) = class.get("MoneySave").and_then(Value::as_class) {
                gold = int(money, "_Gold");
                rift_crystals = int(money, "_Rim");
            }
        }
    });

    let mut muster = Muster {
        slot,
        arisen: None,
        main_pawn: None,
        hired: Vec::new(),
        gold,
        rift_crystals,
    };
    for contexts in &records {
        let Some(job) = contexts.get(&n.job) else {
            continue;
        };
        let role = if contexts.contains_key(&n.player) {
            Role::Arisen
        } else if contexts.contains_key(&n.main_pawn) {
            Role::MainPawn
        } else if contexts.contains_key(&n.sub_pawn) {
            Role::HiredPawn
        } else {
            continue;
        };
        let vocation = int(job, "CurrentJob")
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or(0);
        let mut vocation_experience = [0i64; VOCATIONS];
        for (place, value) in vocation_experience
            .iter_mut()
            .zip(array(job, "CulmativeExp"))
        {
            *place = value.as_i64().unwrap_or(0);
        }
        let status = contexts.get(&n.status);
        let gains = status
            .and_then(|s| s.get("_IncreaseParams"))
            .and_then(Value::as_class)
            .map(|g| Gains {
                health: int(g, "_HitPoint").unwrap_or(0),
                stamina: int(g, "_Stamina").unwrap_or(0),
                strength: int(g, "_Attack").unwrap_or(0),
                defence: int(g, "_Defence").unwrap_or(0),
                magick: int(g, "_MagicAttack").unwrap_or(0),
                magick_defence: int(g, "_MagicDefence").unwrap_or(0),
                weight: int(g, "_Weight").unwrap_or(0),
                knockdown: int(g, "_Blow").unwrap_or(0),
            })
            .unwrap_or_default();
        let chara = contexts
            .get(&n.character_id)
            .and_then(|c| int(c, "CharacterID"))
            .map(|id| id as u32);
        let key = contexts
            .get(&n.unique_id)
            .and_then(|c| c.get("UniqueID"))
            .and_then(Value::as_class)
            .map(|u| {
                let mut bytes = match u.get("_RowID") {
                    Some(Value::Struct(b)) => b.clone(),
                    _ => Vec::new(),
                };
                bytes.extend_from_slice(&int(u, "_Index").unwrap_or(0).to_le_bytes());
                fnv1a(&bytes)
            })
            .unwrap_or_else(|| fnv1a(&chara.unwrap_or(0).to_le_bytes()));
        let name = match role {
            Role::Arisen => contexts.get(&n.player).and_then(|c| text(c, "Name")),
            _ => contexts.get(&n.pawn_data).and_then(|c| text(c, "_Name")),
        };
        let pawn = (role != Role::Arisen).then(|| {
            let data = contexts.get(&n.pawn_data);
            PawnTraits {
                inclination: data
                    .and_then(|d| int(d, "_Personality"))
                    .and_then(Inclination::from_id),
                specialization: data
                    .and_then(|d| int(d, "_SubSkill"))
                    .filter(|&s| s > 0)
                    .map(|s| s as u32),
            }
        });
        let creator = (role == Role::HiredPawn)
            .then(|| contexts.get(&n.pawn_owner))
            .flatten()
            .map(|o| Creator {
                level: int(o, "_Level").unwrap_or(0),
                vocation: int(o, "_Job")
                    .and_then(|v| u32::try_from(v).ok())
                    .unwrap_or(0),
            });
        let member = Member {
            key,
            role,
            name,
            level: status.and_then(|s| int(s, "_Level")).unwrap_or(0),
            experience: status.and_then(|s| int(s, "_Exp")).unwrap_or(0),
            vocation,
            vocation_experience,
            gains,
            gear: chara
                .and_then(|c| gear.get(&c).copied())
                .unwrap_or_default(),
            augments: contexts
                .get(&n.ability)
                .map(|a| loadout(array(a, "EquipedAbilities"), vocation, "Abilities"))
                .unwrap_or_default(),
            skills: contexts
                .get(&n.skill)
                .map(|s| loadout(array(s, "EquipedSkills"), vocation, "Skills"))
                .unwrap_or_default(),
            pawn,
            creator,
        };
        match role {
            Role::Arisen if muster.arisen.is_none() => muster.arisen = Some(member),
            Role::MainPawn if muster.main_pawn.is_none() => muster.main_pawn = Some(member),
            Role::HiredPawn => muster.hired.push(member),
            _ => {}
        }
    }
    if muster.arisen.is_none() && muster.main_pawn.is_none() && muster.hired.is_empty() {
        return Err(MusterError::NoParty);
    }
    Ok(muster)
}
