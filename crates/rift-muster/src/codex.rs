//! The game's own names for what a party carries, read from the player's own install at run
//! time: never bundled, never in the repo (BUILD-SPEC § v0.4, ruling 2).
//!
//! A [`Codex`] is built once per game update from seventeen of the game's text files (it is
//! `serde`, so the spine caches it beside the pak's size and time). The id rules, each
//! measured on the investor's party:
//!
//! - items: `item_name_<id>`;
//! - vocations: `job_name_NN`;
//! - inclinations: `Personality_00` … `_03`;
//! - specializations: N is `Subskill_{N−1:02}_00`;
//! - augments: id `4 + (vocation − 1) × 5 + i` for vocations 1–9 is `jobNN_ability_{i:02}`;
//!   vocation 10's two (ids 49, 50) are `job10_ability_01`, `_02` (its text counts from 01);
//! - skills: one block of ids per vocation (`SKILL_BLOCKS`), `jobNN_skill_{id − first:02}`, and
//!   id 101 — a Thief skill a title update appended — is `job04_skill_12`.
//!
//! A text may be a reference (`<REF job03_skill_00>`: the Sorcerer shares a skill with the
//! Mage), resolved through the entry it names. Ids beyond these rules stay unnamed.

use std::collections::{BTreeMap, HashMap};

use rift_pak::game::{GamePak, GamePakError};
use rift_pak::msg::{Msg, MsgError};
use serde::{Deserialize, Serialize};

use crate::party::Inclination;

/// The text files a codex is built from (paths from `natives/`).
pub const TEXTS: [&str; 17] = [
    "natives/stm/message/ui/itemname.msg.24",
    "natives/stm/message/ui/itemname_end.msg.24",
    "natives/stm/appdata/extracontents/contents_0011/itemname_contents_0011.msg.24",
    "natives/stm/message/ui/job.msg.24",
    "natives/stm/message/ui/personality.msg.24",
    "natives/stm/message/ui/subskill.msg.24",
    "natives/stm/message/ui/subskill_tu_end.msg.24",
    "natives/stm/message/ui/job01act.msg.24",
    "natives/stm/message/ui/job02act.msg.24",
    "natives/stm/message/ui/job03act.msg.24",
    "natives/stm/message/ui/job04act.msg.24",
    "natives/stm/message/ui/job05act.msg.24",
    "natives/stm/message/ui/job06act.msg.24",
    "natives/stm/message/ui/job07act.msg.24",
    "natives/stm/message/ui/job08act.msg.24",
    "natives/stm/message/ui/job09act.msg.24",
    "natives/stm/message/ui/job10act.msg.24",
];

/// The custom-skill id blocks per vocation, first and last id (measured).
pub const SKILL_BLOCKS: [(u32, u32); 10] = [
    (1, 12),
    (13, 23),
    (24, 37),
    (38, 49),
    (50, 61),
    (62, 69),
    (70, 79),
    (80, 91),
    (92, 99),
    (100, 100),
];

/// The title update's appended Thief skill, and where its text sits.
const APPENDED_SKILLS: [(u32, u32, u32); 1] = [(101, 4, 12)];

/// How deep a `<REF …>` chain may go.
const MAX_REFS: usize = 4;

#[derive(Debug, thiserror::Error)]
pub enum CodexError {
    #[error(transparent)]
    Pak(#[from] GamePakError),
    #[error("a text file of the game would not read: {0}")]
    Msg(#[from] MsgError),
    #[error("the game's pak holds none of the text files the names live in")]
    NoTexts,
}

/// The game's names, by id.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Codex {
    items: BTreeMap<i64, String>,
    vocations: BTreeMap<u32, String>,
    inclinations: BTreeMap<u8, String>,
    specializations: BTreeMap<u32, String>,
    augments: BTreeMap<u32, String>,
    skills: BTreeMap<u32, String>,
}

/// Every entry's English text across the files read, for names and for `<REF …>` targets.
struct Texts(HashMap<String, String>);

impl Texts {
    fn resolve(&self, key: &str, depth: usize) -> Option<String> {
        let raw = self.0.get(key)?;
        let mut out = String::with_capacity(raw.len());
        let mut rest = raw.as_str();
        while let Some(start) = rest.find('<') {
            out.push_str(&rest[..start]);
            let Some(end) = rest[start..].find('>') else {
                out.push_str(&rest[start..]);
                rest = "";
                break;
            };
            let tag = &rest[start + 1..start + end];
            if let Some(target) = tag.strip_prefix("REF ") {
                if depth >= MAX_REFS {
                    return None;
                }
                out.push_str(&self.resolve(target.trim(), depth + 1)?);
            }
            // Any other tag (colour, placeholders) carries no name: drop it.
            rest = &rest[start + end + 1..];
        }
        out.push_str(rest);
        let out = out.trim().to_string();
        (!out.is_empty()).then_some(out)
    }
}

fn numbered<T: std::str::FromStr>(name: &str, prefix: &str, suffix: &str) -> Option<T> {
    name.strip_prefix(prefix)?
        .strip_suffix(suffix)?
        .parse()
        .ok()
}

impl Codex {
    /// Build the codex from the game's own pak.
    pub fn from_pak(pak: &GamePak) -> Result<Codex, CodexError> {
        let mut msgs = Vec::new();
        for path in TEXTS {
            if let Some(bytes) = pak.read(path)? {
                msgs.push(Msg::parse(&bytes)?);
            }
        }
        if msgs.is_empty() {
            return Err(CodexError::NoTexts);
        }
        Ok(Codex::from_msgs(&msgs))
    }

    /// Build the codex from parsed text files (any subset; what is missing stays unnamed).
    pub fn from_msgs(msgs: &[Msg]) -> Codex {
        let mut all = HashMap::new();
        for msg in msgs {
            for name in msg.names() {
                if let Some(text) = msg.english(name) {
                    all.insert(name.to_string(), text.to_string());
                }
            }
        }
        let texts = Texts(all);
        let mut codex = Codex::default();
        for key in texts.0.keys() {
            let Some(name) = texts.resolve(key, 0) else {
                continue;
            };
            if let Some(id) = numbered::<i64>(key, "item_name_", "") {
                codex.items.insert(id, name);
            } else if let Some(id) = numbered::<u32>(key, "job_name_", "") {
                codex.vocations.insert(id, name);
            } else if let Some(i) = numbered::<u8>(key, "Personality_", "").filter(|i| *i < 4) {
                codex.inclinations.insert(i, name);
            } else if let Some(i) = numbered::<u32>(key, "Subskill_", "_00") {
                codex.specializations.insert(i + 1, name);
            }
        }
        for vocation in 1..=10u32 {
            let count = if vocation == 10 { 2 } else { 5 };
            for i in 0..count {
                let (id, index) = if vocation == 10 {
                    (49 + i, i + 1)
                } else {
                    (4 + (vocation - 1) * 5 + i, i)
                };
                if let Some(name) =
                    texts.resolve(&format!("job{vocation:02}_ability_{index:02}"), 0)
                {
                    codex.augments.insert(id, name);
                }
            }
        }
        for (vocation, (first, last)) in (1u32..).zip(SKILL_BLOCKS) {
            for id in first..=last {
                if let Some(name) =
                    texts.resolve(&format!("job{vocation:02}_skill_{:02}", id - first), 0)
                {
                    codex.skills.insert(id, name);
                }
            }
        }
        for (id, vocation, index) in APPENDED_SKILLS {
            if let Some(name) = texts.resolve(&format!("job{vocation:02}_skill_{index:02}"), 0) {
                codex.skills.insert(id, name);
            }
        }
        codex
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.vocations.is_empty()
    }

    pub fn item(&self, id: i64) -> Option<&str> {
        self.items.get(&id).map(String::as_str)
    }

    pub fn vocation(&self, id: u32) -> Option<&str> {
        self.vocations.get(&id).map(String::as_str)
    }

    pub fn inclination(&self, inclination: Inclination) -> Option<&str> {
        self.inclinations
            .get(&inclination.index())
            .map(String::as_str)
    }

    pub fn specialization(&self, id: u32) -> Option<&str> {
        self.specializations.get(&id).map(String::as_str)
    }

    pub fn augment(&self, id: u32) -> Option<&str> {
        self.augments.get(&id).map(String::as_str)
    }

    pub fn skill(&self, id: u32) -> Option<&str> {
        self.skills.get(&id).map(String::as_str)
    }

    /// How many names of each kind the codex holds: items, vocations, inclinations,
    /// specializations, augments, skills. For logs and benches, which print counts only.
    pub fn counts(&self) -> [usize; 6] {
        [
            self.items.len(),
            self.vocations.len(),
            self.inclinations.len(),
            self.specializations.len(),
            self.augments.len(),
            self.skills.len(),
        ]
    }
}
