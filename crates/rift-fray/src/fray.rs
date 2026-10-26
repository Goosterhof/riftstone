//! The fold: the eye's lines in, fights out.
//!
//! Credit follows the reviewed analyzer's `attackerLabel` (probe/analyze.mjs §4) to the letter: a
//! hit whose attacker resolved as `gimmick` is the world's; one with no attacker, or `none`, is
//! unattributed; otherwise the slot the eye wrote, or the slot of (address, CharacterID) under the
//! line's roster generation, names a party member, and any other character is not the party. The
//! damage credited is the post's `dmg1`.
//!
//! A fight opens at the first damage dealt to a foe and settles once no foe has been hit for
//! [`Settings::quiet_s`] game seconds while the game does not say battle, or for
//! [`Settings::hard_quiet_s`] whatever it says, or at the eye's stop line. A member who leaves
//! mid-fight keeps what they dealt, frozen; a newcomer's hits in that fight are unattributed
//! (#00015 RF.3 and FQ.21's fallback: a roster change applies between fights).

use std::collections::{BTreeMap, HashMap, VecDeque};

use crate::frame::{
    Attribution, Biggest, Feed, Fight, FightState, Foe, Frame, Kill, Member, Parts, Role, Skill,
    SkillKind, SkillsState, Source, Types, Vocation,
};
use crate::record::{Attacker, Hit, Line, Record, RosterMember, SlotVitals, PARTS};

/// The four places on the screen: the Arisen, the main pawn, two hired pawns.
pub const PLACES: usize = 4;

/// A hit counts as additive when its parts sum to its damage within max(1 HP, 1%) (G7's rule),
/// and a fight's types are additive when this share of its readable hits is (G7's 95%).
const ADDITIVE_ABS: f64 = 1.0;
const ADDITIVE_REL: f64 = 0.01;
const ADDITIVE_SHARE: f64 = 0.95;

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// Game seconds without a hit on a foe, battle not signalled, before a fight settles.
    pub quiet_s: f64,
    /// Game seconds without a hit on a foe before a fight settles even while the game says battle.
    pub hard_quiet_s: f64,
    /// Settled fights kept for THE STRATA (RF.26).
    pub shelf: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            quiet_s: 8.0,
            hard_quiet_s: 30.0,
            shelf: 5,
        }
    }
}

/// Where a hit's damage goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Credit {
    Party(i64),
    World,
    Unattributed,
    NonParty,
}

/// The session file's start: os.clock `clock` was the wall's `unix` second.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SessionClock {
    pub clock: f64,
    pub unix: i64,
}

/// How a settled fight's wall-clock time is written: "21:04" in the local zone by default.
pub type TimeFormat = fn(i64) -> String;

pub fn local_time(unix: i64) -> String {
    chrono::DateTime::from_timestamp(unix, 0)
        .map(|t| t.with_timezone(&chrono::Local).format("%H:%M").to_string())
        .unwrap_or_default()
}

#[derive(Debug, Clone, Default)]
struct SkillAcc {
    kind: Option<SkillKind>,
    damage: f64,
    hits: u32,
    biggest: f64,
    last_blow: bool,
    first_at: f64,
}

#[derive(Debug, Clone, Default)]
struct MemberAcc {
    damage: f64,
    hits: u32,
    skills: BTreeMap<String, SkillAcc>,
    cmp: [f64; PARTS],
    cmp_hits: u32,
}

#[derive(Debug, Clone, Default)]
struct SourceAcc {
    damage: f64,
    hits: u32,
    biggest: f64,
}

impl SourceAcc {
    fn add(&mut self, dmg: f64) {
        self.damage += dmg;
        self.hits += 1;
        self.biggest = self.biggest.max(dmg);
    }

    fn frame(&self) -> Source {
        Source {
            damage: round(self.damage),
            hits: self.hits,
            biggest: round(self.biggest),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct FoeAcc {
    cid: Option<i64>,
    tdp_first: Option<f64>,
    tdp_last: Option<f64>,
}

#[derive(Debug, Clone)]
struct FightAcc {
    n: u32,
    start_gt: f64,
    last_hit_gt: f64,
    last_hit_t: f64,
    roster: Vec<RosterMember>,
    members: [MemberAcc; PLACES],
    left: [bool; PLACES],
    unattributed: SourceAcc,
    world: SourceAcc,
    foes: BTreeMap<u64, FoeAcc>,
    kills: Vec<Kill>,
    gaps: u64,
    cmp_readable: u32,
    cmp_additive: u32,
    keys_readable: bool,
    biggest: Option<(usize, String, f64)>,
}

impl FightAcc {
    fn new(n: u32, gt: f64, t: f64, roster: Vec<RosterMember>) -> Self {
        FightAcc {
            n,
            start_gt: gt,
            last_hit_gt: gt,
            last_hit_t: t,
            roster,
            members: Default::default(),
            left: [false; PLACES],
            unattributed: SourceAcc::default(),
            world: SourceAcc::default(),
            foes: BTreeMap::new(),
            kills: Vec::new(),
            gaps: 0,
            cmp_readable: 0,
            cmp_additive: 0,
            keys_readable: false,
            biggest: None,
        }
    }

    /// The place a party credit lands in, or None when the hit belongs to no card: a slot past
    /// the four, or a character who was not in that slot when the fight opened.
    fn place(&self, slot: i64, atk: Option<&Attacker>) -> Option<usize> {
        let place = usize::try_from(slot).ok().filter(|p| *p < PLACES)?;
        if self.roster.is_empty() {
            return Some(place);
        }
        let held = self.roster.iter().find(|m| m.slot == slot)?;
        match (held.a, atk.and_then(|a| a.a)) {
            (Some(was), Some(now)) if was != now => None,
            _ => Some(place),
        }
    }
}

/// The stone's running view of one eye session.
#[derive(Debug, Clone)]
pub struct Fray {
    settings: Settings,
    ses: String,
    clock: Option<SessionClock>,
    time_format: TimeFormat,
    rosters: HashMap<i64, Vec<RosterMember>>,
    roster_now: Vec<RosterMember>,
    vitals: [Option<SlotVitals>; PLACES],
    battle: Option<bool>,
    paused: bool,
    foe_tdp: HashMap<u64, f64>,
    foe_cid: HashMap<u64, i64>,
    open: Option<FightAcc>,
    recent: VecDeque<Fight>,
    fights: u32,
    last_seq: Option<u64>,
    last_gt: f64,
    last_beat_t: Option<f64>,
    stopped: bool,
}

impl Fray {
    pub fn new(ses: impl Into<String>, clock: Option<SessionClock>, settings: Settings) -> Self {
        Fray {
            settings,
            ses: ses.into(),
            clock,
            time_format: local_time,
            rosters: HashMap::new(),
            roster_now: Vec::new(),
            vitals: Default::default(),
            battle: None,
            paused: false,
            foe_tdp: HashMap::new(),
            foe_cid: HashMap::new(),
            open: None,
            recent: VecDeque::new(),
            fights: 0,
            last_seq: None,
            last_gt: 0.0,
            last_beat_t: None,
            stopped: false,
        }
    }

    pub fn with_time_format(mut self, format: TimeFormat) -> Self {
        self.time_format = format;
        self
    }

    pub fn session(&self) -> &str {
        &self.ses
    }

    /// The eye wrote its stop line: nothing more will come from this session.
    pub fn stopped(&self) -> bool {
        self.stopped
    }

    /// The os.clock of the last heartbeat, so the spine can tell a live feed from a stale one.
    pub fn last_beat_t(&self) -> Option<f64> {
        self.last_beat_t
    }

    pub fn ingest(&mut self, line: &Line) {
        if let Some(last) = self.last_seq {
            if line.seq > last + 1 {
                if let Some(acc) = &mut self.open {
                    acc.gaps += line.seq - last - 1;
                }
            }
        }
        self.last_seq = Some(self.last_seq.map_or(line.seq, |last| last.max(line.seq)));
        if let Some(gt) = line.gt {
            self.last_gt = self.last_gt.max(gt);
        }
        self.settle_if_quiet();

        match &line.body {
            Record::Roster(members) => self.roster(line.rg, members),
            Record::Hit(hit) => self.hit(line, hit),
            Record::Die(die) => self.die(line, die.vid, die.atk.as_ref()),
            Record::Foe(foe) => {
                if let Some(vid) = foe.vid {
                    if let Some(cid) = foe.cid {
                        self.foe_cid.insert(vid, cid);
                    }
                    self.ledger(vid, foe.tdp);
                }
            }
            Record::Gone {
                vid: Some(vid),
                tdp,
            } => self.ledger(*vid, *tdp),
            Record::Gone { .. } | Record::Other => {}
            Record::Signal { name, val } => match name.as_str() {
                "battle" => self.battle = *val,
                "pausedAny" => self.paused = val.unwrap_or(false),
                _ => {}
            },
            Record::Heartbeat(slots) => {
                self.last_beat_t = Some(line.t);
                for v in slots {
                    if let Some(place) = usize::try_from(v.slot).ok().filter(|p| *p < PLACES) {
                        self.vitals[place] = Some(v.clone());
                    }
                }
            }
            Record::Stop => {
                self.settle();
                self.stopped = true;
            }
        }
    }

    /// Settle whatever is open: the session's files have been read to their end for good.
    pub fn finish(&mut self) {
        self.settle();
    }

    /// The analyzer's attacker label (analyze.mjs `attackerLabel`), over this session's rosters.
    pub fn credit(&self, rg: Option<i64>, atk: Option<&Attacker>) -> Credit {
        let Some(a) = atk else {
            return Credit::Unattributed;
        };
        match a.how.as_deref() {
            Some("gimmick") => return Credit::World,
            None | Some("none") => return Credit::Unattributed,
            _ => {}
        }
        let slot = match a.slot {
            Some(slot) => slot,
            None => {
                let Some(addr) = a.a else {
                    return Credit::Unattributed;
                };
                let Some(roster) = rg.and_then(|rg| self.rosters.get(&rg)) else {
                    return Credit::Unattributed;
                };
                let cid = a.cid.unwrap_or(-1);
                roster
                    .iter()
                    .find(|m| m.a == Some(addr) && m.cid.unwrap_or(-1) == cid)
                    .map_or(-1, |m| m.slot)
            }
        };
        if slot >= 0 {
            Credit::Party(slot)
        } else {
            Credit::NonParty
        }
    }

    pub fn frame(&self, feed: Feed) -> Frame {
        let fight = match &self.open {
            Some(acc) => Some(self.build(acc, FightState::Open, self.last_gt - acc.start_gt)),
            None => self.recent.front().cloned(),
        };
        Frame {
            feed,
            party: self.party_now(),
            fight,
            recent: self.recent.iter().cloned().collect(),
        }
    }

    /// The party as the eye last read it, outside any fight: the roster's places with their
    /// vitals, no damage and no skills. Empty before the eye's first roster.
    fn party_now(&self) -> Vec<Option<Member>> {
        if self.roster_now.is_empty() {
            return Vec::new();
        }
        (0..PLACES)
            .map(|place| {
                let held = self
                    .roster_now
                    .iter()
                    .find(|r| usize::try_from(r.slot).ok() == Some(place))?;
                let vitals = self.vitals[place].as_ref();
                Some(Member {
                    slot: place,
                    role: Role::of_slot(place),
                    name: None,
                    vocation: Vocation {
                        id: held.job.or_else(|| vitals.and_then(|v| v.job)),
                        name: None,
                    },
                    hp: vitals.and_then(|v| fraction(v.hp, v.maxhp)),
                    st: vitals.and_then(|v| fraction(v.st, v.stmax)),
                    down: vitals.and_then(|v| v.hp).is_some_and(|hp| hp <= 0.0),
                    left: false,
                    damage: 0,
                    dps: None,
                    hits: 0,
                    skills: Vec::new(),
                    no_damage_skills: None,
                    parts: None,
                    mix: None,
                })
            })
            .collect()
    }

    fn roster(&mut self, rg: Option<i64>, members: &[RosterMember]) {
        if let Some(rg) = rg {
            self.rosters.insert(rg, members.to_vec());
        }
        self.roster_now = members.to_vec();
        if let Some(acc) = &mut self.open {
            if acc.roster.is_empty() {
                acc.roster = members.to_vec();
                return;
            }
            for held in &acc.roster {
                let Some(place) = usize::try_from(held.slot).ok().filter(|p| *p < PLACES) else {
                    continue;
                };
                let stays = members
                    .iter()
                    .any(|m| m.slot == held.slot && (m.a == held.a || m.a.is_none()));
                if !stays {
                    acc.left[place] = true;
                }
            }
        }
    }

    fn hit(&mut self, line: &Line, hit: &Hit) {
        let Some(vic) = &hit.vic else { return };
        let Some(vid) = vic.vid else { return };
        let before = self.foe_tdp.get(&vid).copied();
        if let Some(cid) = vic.cid {
            self.foe_cid.insert(vid, cid);
        }
        if let Some(tdp) = vic.tdp1 {
            self.foe_tdp.insert(vid, tdp);
        }
        let dmg = hit.dmg.unwrap_or(0.0);
        if dmg <= 0.0 && self.open.is_none() {
            return;
        }
        let gt = line.gt.unwrap_or(self.last_gt);
        if self.open.is_none() {
            self.fights += 1;
            self.open = Some(FightAcc::new(
                self.fights,
                gt,
                line.t,
                self.roster_now.clone(),
            ));
        }
        let credit = self.credit(line.rg, hit.atk.as_ref());
        let cid = self.foe_cid.get(&vid).copied();
        let Some(acc) = &mut self.open else { return };

        let foe = acc.foes.entry(vid).or_insert_with(|| FoeAcc {
            cid,
            tdp_first: vic.tdp0.or(before),
            tdp_last: None,
        });
        foe.cid = foe.cid.or(cid);
        foe.tdp_last = vic.tdp1.or(foe.tdp_last);

        if dmg <= 0.0 {
            return;
        }
        acc.last_hit_gt = acc.last_hit_gt.max(gt);
        acc.last_hit_t = acc.last_hit_t.max(line.t);
        if let Some(cmp) = hit.cmp {
            acc.cmp_readable += 1;
            let sum: f64 = cmp.iter().sum();
            if (sum - dmg).abs() <= ADDITIVE_ABS.max(ADDITIVE_REL * dmg.abs()) {
                acc.cmp_additive += 1;
            }
        }

        let place = match credit {
            Credit::Party(slot) => acc.place(slot, hit.atk.as_ref()),
            _ => None,
        };
        match (credit, place) {
            (Credit::Party(_), Some(place)) => {
                let atk = hit.atk.as_ref();
                if atk.is_some_and(|a| a.sk.is_some() || a.ah.is_some()) {
                    acc.keys_readable = true;
                }
                let (kind, id) = key_of(atk);
                let at = gt - acc.start_gt;
                let m = &mut acc.members[place];
                m.damage += dmg;
                m.hits += 1;
                if let Some(cmp) = hit.cmp {
                    for (sum, part) in m.cmp.iter_mut().zip(cmp) {
                        *sum += part;
                    }
                    m.cmp_hits += 1;
                }
                let skill = m
                    .skills
                    .entry(skill_key(kind, &id))
                    .or_insert_with(|| SkillAcc {
                        kind: Some(kind),
                        first_at: at,
                        ..SkillAcc::default()
                    });
                skill.damage += dmg;
                skill.hits += 1;
                skill.biggest = skill.biggest.max(dmg);
                if acc.biggest.as_ref().is_none_or(|(_, _, big)| dmg > *big) {
                    acc.biggest = Some((place, id, dmg));
                }
            }
            (Credit::World | Credit::NonParty, _) => acc.world.add(dmg),
            _ => acc.unattributed.add(dmg),
        }
    }

    fn die(&mut self, line: &Line, vid: Option<u64>, atk: Option<&Attacker>) {
        let Some(vid) = vid else { return };
        let credit = self.credit(line.rg, atk);
        let gt = line.gt.unwrap_or(self.last_gt);
        let Some(acc) = &mut self.open else { return };
        if !acc.foes.contains_key(&vid) {
            return;
        }
        let place = match credit {
            Credit::Party(slot) => acc.place(slot, atk),
            _ => None,
        };
        let (kind, id) = key_of(atk);
        if let Some(place) = place {
            if let Some(skill) = acc.members[place].skills.get_mut(&skill_key(kind, &id)) {
                skill.last_blow = true;
            }
        }
        acc.kills.push(Kill {
            slot: place,
            key: place.map(|_| id),
            at: gt - acc.start_gt,
        });
    }

    fn ledger(&mut self, vid: u64, tdp: Option<f64>) {
        let Some(tdp) = tdp else { return };
        self.foe_tdp.insert(vid, tdp);
        if let Some(foe) = self.open.as_mut().and_then(|acc| acc.foes.get_mut(&vid)) {
            foe.tdp_last = Some(tdp);
        }
    }

    fn settle_if_quiet(&mut self) {
        let Some(acc) = &self.open else { return };
        let quiet = self.last_gt - acc.last_hit_gt;
        if quiet >= self.settings.hard_quiet_s
            || (quiet >= self.settings.quiet_s && self.battle != Some(true))
        {
            self.settle();
        }
    }

    fn settle(&mut self) {
        let Some(acc) = self.open.take() else { return };
        let fight = self.build(&acc, FightState::Settled, acc.last_hit_gt - acc.start_gt);
        self.recent.push_front(fight);
        self.recent.truncate(self.settings.shelf);
    }

    fn build(&self, acc: &FightAcc, state: FightState, duration_s: f64) -> Fight {
        let duration_s = duration_s.max(0.0);
        let types = if acc.cmp_readable == 0 {
            Types::Unknown
        } else if f64::from(acc.cmp_additive) >= ADDITIVE_SHARE * f64::from(acc.cmp_readable) {
            Types::Additive
        } else {
            Types::Mix
        };

        let members: Vec<Option<Member>> = (0..PLACES)
            .map(|place| {
                let m = &acc.members[place];
                let held = acc
                    .roster
                    .iter()
                    .find(|r| usize::try_from(r.slot).ok() == Some(place));
                if held.is_none() && m.hits == 0 {
                    return None;
                }
                let vitals = self.vitals[place].as_ref();
                let damage = round(m.damage);
                let mix = (m.cmp_hits > 0).then(|| m.cmp.map(round));
                let parts =
                    (types == Types::Additive && m.cmp_hits > 0).then(|| partition(&m.cmp, damage));
                Some(Member {
                    slot: place,
                    role: Role::of_slot(place),
                    name: None,
                    vocation: Vocation {
                        id: held
                            .and_then(|r| r.job)
                            .or_else(|| vitals.and_then(|v| v.job)),
                        name: None,
                    },
                    hp: vitals.and_then(|v| fraction(v.hp, v.maxhp)),
                    st: vitals.and_then(|v| fraction(v.st, v.stmax)),
                    down: vitals.and_then(|v| v.hp).is_some_and(|hp| hp <= 0.0),
                    left: acc.left[place],
                    damage,
                    dps: (duration_s > 0.0).then(|| m.damage / duration_s),
                    hits: m.hits,
                    skills: m
                        .skills
                        .iter()
                        .map(|(key, s)| Skill {
                            kind: s.kind.unwrap_or(SkillKind::Unknown),
                            id: key
                                .split_once(':')
                                .map_or(key.as_str(), |(_, id)| id)
                                .to_owned(),
                            name: None,
                            damage: round(s.damage),
                            hits: s.hits,
                            biggest: round(s.biggest),
                            last_blow: s.last_blow,
                            first_at: s.first_at,
                        })
                        .collect(),
                    no_damage_skills: None,
                    parts,
                    mix,
                })
            })
            .collect();

        let h_party: u64 = members.iter().flatten().map(|m| m.damage).sum();
        let unattributed = acc.unattributed.frame();
        let world = (acc.world.hits > 0).then(|| acc.world.frame());
        let h_all = h_party + unattributed.damage + world.as_ref().map_or(0, |w| w.damage);

        let mut ledger: Option<f64> = None;
        let mut by_cid: BTreeMap<Option<i64>, u32> = BTreeMap::new();
        for foe in acc.foes.values() {
            *by_cid.entry(foe.cid).or_default() += 1;
            if let (Some(first), Some(last)) = (foe.tdp_first, foe.tdp_last) {
                *ledger.get_or_insert(0.0) += (last - first).max(0.0);
            }
        }

        let ended_ms = match (state, self.clock) {
            (FightState::Settled, Some(c)) => {
                Some(((c.unix as f64 + (acc.last_hit_t - c.clock)) * 1000.0).round() as i64)
            }
            _ => None,
        };
        let ended_at = ended_ms.map(|ms| (self.time_format)(ms.div_euclid(1000)));

        Fight {
            id: format!("{}-{}", self.ses, acc.n),
            n: acc.n,
            state,
            duration_s,
            paused: state == FightState::Open && self.paused,
            foes: by_cid
                .into_iter()
                .map(|(cid, count)| Foe {
                    cid,
                    name: None,
                    count,
                })
                .collect(),
            ended_at,
            ended_at_ms: ended_ms,
            members,
            h_party,
            h_all,
            unattributed,
            world,
            ledger: ledger.map(round),
            party_formed: !acc.roster.is_empty(),
            attribution: Attribution::Unqualified,
            types,
            skills: if acc.keys_readable {
                SkillsState::Available
            } else {
                SkillsState::Unavailable
            },
            kills: acc.kills.clone(),
            gaps: acc.gaps,
            biggest: acc.biggest.as_ref().map(|(slot, key, dmg)| Biggest {
                slot: *slot,
                key: key.clone(),
                damage: round(*dmg),
            }),
        }
    }
}

/// A damage key: the custom skill when one is in use, else the shell's param id, else the
/// action hash (contract §1 G1), else unknown.
pub fn key_of(atk: Option<&Attacker>) -> (SkillKind, String) {
    let Some(a) = atk else {
        return (SkillKind::Unknown, "unknown".to_owned());
    };
    if let Some(sk) = a.sk.filter(|s| *s > 0) {
        (SkillKind::Skill, sk.to_string())
    } else if let Some(param) = a.shell_param {
        (SkillKind::Action, format!("shell {param}"))
    } else if let Some(ah) = a.ah.filter(|h| *h != 0) {
        (SkillKind::Action, format!("{ah:X}"))
    } else {
        (SkillKind::Unknown, "unknown".to_owned())
    }
}

fn skill_key(kind: SkillKind, id: &str) -> String {
    let tag = match kind {
        SkillKind::Skill => "s",
        SkillKind::Action => "a",
        SkillKind::Unknown => "u",
    };
    format!("{tag}:{id}")
}

fn round(v: f64) -> u64 {
    if v.is_finite() && v > 0.0 {
        v.round() as u64
    } else {
        0
    }
}

fn fraction(value: Option<f64>, max: Option<f64>) -> Option<f64> {
    match (value, max) {
        (Some(v), Some(m)) if m > 0.0 => Some((v / m).clamp(0.0, 1.0)),
        _ => None,
    }
}

/// The additive split, scaled to the member's rounded damage so the parts sum to it exactly; the
/// rounding remainder goes on the largest part (the audition's `partsFor`).
fn partition(cmp: &[f64; PARTS], damage: u64) -> Parts {
    let total: f64 = cmp.iter().sum();
    if total <= 0.0 || damage == 0 {
        return [0; PARTS];
    }
    let scale = damage as f64 / total;
    let mut parts = cmp.map(|v| round(v * scale));
    let largest = (0..PARTS)
        .max_by(|a, b| parts[*a].cmp(&parts[*b]).then(b.cmp(a)))
        .unwrap_or(0);
    let sum: u64 = parts.iter().sum();
    if sum > damage {
        parts[largest] = parts[largest].saturating_sub(sum - damage);
    } else {
        parts[largest] += damage - sum;
    }
    parts
}
