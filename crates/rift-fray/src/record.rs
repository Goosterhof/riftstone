//! One line of the eye's log, read leniently.
//!
//! The eye (the party-screen probe, contract v0.8) writes one JSON object per line. Its legend is
//! the key map; this module reads only the keys the Fray draws from and ignores the rest. Null is
//! never zero (contract §3.1): a value the eye could not read arrives as null, or as the strings
//! `nan`, `inf` and `-inf`, and stays `None` here.

use serde_json::{Map, Value};

/// The seven damage components of a hit, in the eye's order: Slash, Blow, Shoot, Magic, Enchant,
/// NonMagicElement, Fixed (`hit.cmp`).
pub const PARTS: usize = 7;

/// A line's envelope and the record it carries.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// The per-session sequence: a gap means records were lost.
    pub seq: u64,
    /// The roster generation the record was made under.
    pub rg: Option<i64>,
    /// os.clock seconds.
    pub t: f64,
    /// Pause-aware game seconds.
    pub gt: Option<f64>,
    pub body: Record,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Record {
    /// A new roster generation (`rost`): the line's `rg` is that generation.
    Roster(Vec<RosterMember>),
    Hit(Hit),
    /// A foe's death, read in the hook's pre (`die`).
    Die(Die),
    /// A foe sampled every half second (`foe`).
    Foe(FoeSample),
    /// A foe released (`gone`), with its ledger at release.
    Gone {
        vid: Option<u64>,
        tdp: Option<f64>,
    },
    /// A game signal that changed (`sig`): battle, pausedAny, pausedGUI, loadGui, demo.
    Signal {
        name: String,
        val: Option<bool>,
    },
    /// The once-a-second heartbeat (`hb`): the party's vitals.
    Heartbeat(Vec<SlotVitals>),
    /// The last line of a capture (`stop`).
    Stop,
    /// Every kind the Fray does not draw from.
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RosterMember {
    pub slot: i64,
    pub a: Option<u64>,
    pub cid: Option<i64>,
    pub job: Option<i64>,
}

/// The attacker, as the eye resolved it (contract §4).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Attacker {
    /// shell, owner, cached, gimmick or none.
    pub how: Option<String>,
    pub a: Option<u64>,
    pub cid: Option<i64>,
    /// The roster slot under the line's generation, or -1 when not in it.
    pub slot: Option<i64>,
    /// CustomSkillState: the custom skill in use, 0 when none.
    pub sk: Option<i64>,
    /// CurrentActionHash.
    pub ah: Option<u64>,
    /// The shell's ShellParamId, when the hit came from a shell.
    pub shell_param: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Victim {
    /// The foe lifecycle id; None when the victim is no foe (a party member, or nobody).
    pub vid: Option<u64>,
    pub cid: Option<i64>,
    /// The victim Character's HP in the pre and the post: what the hit really took.
    pub hp0: Option<f64>,
    pub hp1: Option<f64>,
    /// TotalDamageFromPlayerParty in the pre and the post: the game's own party ledger.
    pub tdp0: Option<f64>,
    pub tdp1: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Hit {
    /// Damage in the post (`dmg1`): the game's figure before HP clamps it (overkill, an immune
    /// part). The Fray credits the HP the victim lost, and falls back to this when HP is unread.
    pub dmg: Option<f64>,
    pub cmp: Option<[f64; PARTS]>,
    /// None when the line carries no attacker object.
    pub atk: Option<Attacker>,
    pub vic: Option<Victim>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Die {
    pub vid: Option<u64>,
    pub atk: Option<Attacker>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct FoeSample {
    pub vid: Option<u64>,
    pub cid: Option<i64>,
    pub tdp: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SlotVitals {
    pub slot: i64,
    pub hp: Option<f64>,
    pub maxhp: Option<f64>,
    pub st: Option<f64>,
    pub stmax: Option<f64>,
    pub job: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LineError {
    #[error("the line is not JSON")]
    NotJson,
    #[error("the line is not an object")]
    NotObject,
    #[error("the line has no seq or no kind")]
    NoEnvelope,
}

/// Read one line of the log.
pub fn parse_line(text: &str) -> Result<Line, LineError> {
    let value: Value = serde_json::from_str(text).map_err(|_| LineError::NotJson)?;
    let o = value.as_object().ok_or(LineError::NotObject)?;
    let seq = o
        .get("seq")
        .and_then(Value::as_u64)
        .ok_or(LineError::NoEnvelope)?;
    let kind = o
        .get("k")
        .and_then(Value::as_str)
        .ok_or(LineError::NoEnvelope)?;
    let body = match kind {
        "rost" => Record::Roster(
            list(o, "members")
                .iter()
                .filter_map(roster_member)
                .collect(),
        ),
        "hit" => Record::Hit(Hit {
            dmg: num(o, "dmg1"),
            cmp: parts(o.get("cmp")),
            atk: obj(o, "atk").map(attacker),
            vic: obj(o, "vic").map(victim),
        }),
        "die" => Record::Die(Die {
            vid: obj(o, "vic").and_then(|v| vid(v, "vid")),
            atk: obj(o, "atk").map(attacker),
        }),
        "foe" => Record::Foe(FoeSample {
            vid: vid(o, "vid"),
            cid: int(o, "cid"),
            tdp: num(o, "tdp"),
        }),
        "gone" => Record::Gone {
            vid: vid(o, "vid"),
            tdp: num(o, "tdp"),
        },
        "sig" => Record::Signal {
            name: o
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            val: o.get("val").and_then(Value::as_bool),
        },
        "hb" => Record::Heartbeat(list(o, "slots").iter().filter_map(slot_vitals).collect()),
        "stop" => Record::Stop,
        _ => Record::Other,
    };
    Ok(Line {
        seq,
        rg: int(o, "rg"),
        t: num(o, "t").unwrap_or(0.0),
        gt: num(o, "gt"),
        body,
    })
}

fn num(o: &Map<String, Value>, key: &str) -> Option<f64> {
    o.get(key).and_then(Value::as_f64).filter(|v| v.is_finite())
}

fn int(o: &Map<String, Value>, key: &str) -> Option<i64> {
    o.get(key).and_then(Value::as_i64)
}

/// An address: a positive integer. 0 means "no object" in the eye's convention.
fn addr(o: &Map<String, Value>, key: &str) -> Option<u64> {
    o.get(key).and_then(Value::as_u64).filter(|a| *a != 0)
}

/// A vid is a positive integer, never 0.
fn vid(o: &Map<String, Value>, key: &str) -> Option<u64> {
    o.get(key).and_then(Value::as_u64).filter(|v| *v > 0)
}

fn obj<'a>(o: &'a Map<String, Value>, key: &str) -> Option<&'a Map<String, Value>> {
    o.get(key).and_then(Value::as_object)
}

fn list<'a>(o: &'a Map<String, Value>, key: &str) -> &'a [Value] {
    o.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

fn parts(value: Option<&Value>) -> Option<[f64; PARTS]> {
    let items = value?.as_array()?;
    if items.len() != PARTS {
        return None;
    }
    let mut out = [0.0; PARTS];
    for (slot, item) in out.iter_mut().zip(items) {
        *slot = item.as_f64().filter(|v| v.is_finite())?;
    }
    Some(out)
}

fn attacker(o: &Map<String, Value>) -> Attacker {
    Attacker {
        how: o.get("how").and_then(Value::as_str).map(str::to_owned),
        a: addr(o, "a"),
        cid: int(o, "cid"),
        slot: int(o, "slot"),
        sk: int(o, "sk"),
        ah: o.get("ah").and_then(Value::as_u64),
        shell_param: obj(o, "shell").and_then(|s| int(s, "paramid")),
    }
}

fn victim(o: &Map<String, Value>) -> Victim {
    Victim {
        vid: vid(o, "vid"),
        cid: int(o, "cid"),
        hp0: num(o, "hp0"),
        hp1: num(o, "hp1"),
        tdp0: num(o, "tdp0"),
        tdp1: num(o, "tdp1"),
    }
}

fn roster_member(v: &Value) -> Option<RosterMember> {
    let o = v.as_object()?;
    Some(RosterMember {
        slot: int(o, "slot")?,
        a: addr(o, "a"),
        cid: int(o, "cid"),
        job: int(o, "job"),
    })
}

fn slot_vitals(v: &Value) -> Option<SlotVitals> {
    let o = v.as_object()?;
    Some(SlotVitals {
        slot: int(o, "slot")?,
        hp: num(o, "hp"),
        maxhp: num(o, "maxhp"),
        st: num(o, "st"),
        stmax: num(o, "stmax"),
        job: int(o, "job"),
    })
}
