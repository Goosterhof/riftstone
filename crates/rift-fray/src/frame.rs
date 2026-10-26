//! The frame the `fray` window pulls: `src/types/fray.ts`, field for field. Change one, change
//! both, in the same commit. The shape is #00015 §19.15's "Contract gaps".

use serde::Serialize;

use crate::record::PARTS;

pub type Parts = [u64; PARTS];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SkillKind {
    Skill,
    Action,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub kind: SkillKind,
    pub id: String,
    pub name: Option<String>,
    pub damage: u64,
    pub hits: u32,
    pub biggest: u64,
    pub last_blow: bool,
    pub first_at: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Role {
    Arisen,
    MainPawn,
    HiredPawn,
}

impl Role {
    pub fn of_slot(slot: usize) -> Role {
        match slot {
            0 => Role::Arisen,
            1 => Role::MainPawn,
            _ => Role::HiredPawn,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Vocation {
    pub id: Option<i64>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoDamageSkill {
    pub name: String,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub slot: usize,
    pub role: Role,
    pub name: Option<String>,
    pub vocation: Vocation,
    pub hp: Option<f64>,
    pub st: Option<f64>,
    pub down: bool,
    pub left: bool,
    pub damage: u64,
    pub dps: Option<f64>,
    pub hits: u32,
    pub skills: Vec<Skill>,
    pub no_damage_skills: Option<Vec<NoDamageSkill>>,
    pub parts: Option<Parts>,
    pub mix: Option<Parts>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Foe {
    pub cid: Option<i64>,
    pub name: Option<String>,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Kill {
    pub slot: Option<usize>,
    pub key: Option<String>,
    pub at: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Attribution {
    Checked,
    Unqualified,
    Inconclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Types {
    Additive,
    Mix,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SkillsState {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FightState {
    Open,
    Settled,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub damage: u64,
    pub hits: u32,
    pub biggest: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Biggest {
    pub slot: usize,
    pub key: String,
    pub damage: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fight {
    pub id: String,
    pub n: u32,
    pub state: FightState,
    pub duration_s: f64,
    pub paused: bool,
    pub foes: Vec<Foe>,
    pub ended_at: Option<String>,
    pub ended_at_ms: Option<i64>,
    /// Four entries in slot order; None where the slot is empty.
    pub members: Vec<Option<Member>>,
    pub h_party: u64,
    pub h_all: u64,
    pub unattributed: Source,
    pub world: Option<Source>,
    pub ledger: Option<u64>,
    pub party_formed: bool,
    pub attribution: Attribution,
    pub types: Types,
    pub skills: SkillsState,
    pub kills: Vec<Kill>,
    pub gaps: u64,
    pub biggest: Option<Biggest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FeedState {
    Live,
    Stale,
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Feed {
    pub state: FeedState,
    pub last_beat_ago_s: Option<f64>,
    pub game_running: bool,
    pub game_started_ms: Option<u64>,
    pub reframework: bool,
    pub eye_installed: bool,
    pub eye_recording: Option<bool>,
    pub eye_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Frame {
    pub feed: Feed,
    /// The party outside any fight, four places (empty before the first roster).
    pub party: Vec<Option<Member>>,
    pub fight: Option<Fight>,
    pub recent: Vec<Fight>,
}
