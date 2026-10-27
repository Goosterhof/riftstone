//! The stone's tests, over invented eye lines (every address, id and number is made up here).

use serde_json::{json, Value};

use crate::eye::{newest_session, session_clock, Tail};
use crate::frame::{Feed, FeedState, FightState, SkillKind, Types};
use crate::fray::{Credit, Fray, Settings};
use crate::record::{parse_line, Attacker, Line};

const SES: &str = "1790000004_8120e8";
const ODALRIC: u64 = 0x2C5_0000_1000;
const WREN: u64 = 0x2C5_0000_2000;
const BRAMBLE: u64 = 0x2C5_0000_3000;
const SABLE: u64 = 0x2C5_0000_4000;
const CORVIN: u64 = 0x2C5_0000_5000;

/// Each invented character's own CharacterID.
fn cid_of(a: u64) -> i64 {
    130_000 + ((a >> 12) & 0xff) as i64
}

struct Eye {
    seq: u64,
    rg: i64,
    lines: Vec<Line>,
}

impl Eye {
    fn new() -> Self {
        Eye {
            seq: 0,
            rg: 0,
            lines: Vec::new(),
        }
    }

    fn push(&mut self, gt: f64, kind: &str, mut body: Value) -> &mut Self {
        self.seq += 1;
        let o = body.as_object_mut().expect("an object");
        o.insert("v".into(), json!(0));
        o.insert("ses".into(), json!(SES));
        o.insert("seg".into(), json!(0));
        o.insert("seq".into(), json!(self.seq));
        o.insert("rg".into(), json!(self.rg));
        o.insert("t".into(), json!(4.0 + gt));
        o.insert("gt".into(), json!(gt));
        o.insert("k".into(), json!(kind));
        self.lines
            .push(parse_line(&body.to_string()).expect("an invented line parses"));
        self
    }

    fn roster(&mut self, gt: f64, members: &[(i64, u64, i64)]) -> &mut Self {
        self.rg += 1;
        let members: Vec<Value> = members
            .iter()
            .map(|(slot, a, job)| json!({"slot": slot, "a": a, "cid": cid_of(*a), "job": job}))
            .collect();
        self.push(gt, "rost", json!({ "members": members }))
    }

    fn party(&mut self) -> &mut Self {
        self.roster(
            0.0,
            &[
                (0, ODALRIC, 5),
                (1, WREN, 4),
                (2, BRAMBLE, 6),
                (3, SABLE, 3),
            ],
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn hit(
        &mut self,
        gt: f64,
        slot: i64,
        sk: i64,
        ah: u64,
        vid: u64,
        dmg: f64,
        tdp: (f64, f64),
    ) -> &mut Self {
        let a = [ODALRIC, WREN, BRAMBLE, SABLE]
            .get(slot as usize)
            .copied()
            .unwrap_or(CORVIN);
        self.push(
            gt,
            "hit",
            json!({
                "dmg0": dmg, "dmg1": dmg, "cmp": [dmg, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                "atk": {"how": "owner", "a": a, "cid": cid_of(a), "slot": slot, "sk": sk, "ah": ah, "inparty": true},
                "vic": {"vid": vid, "a": 0x2C5_0009_0000_u64 + vid, "cid": 133179, "slot": -1, "tdp0": tdp.0, "tdp1": tdp.1},
            }),
        )
    }

    fn fray(&self) -> Fray {
        let mut fray = Fray::new(SES, None, Settings::default());
        for line in &self.lines {
            fray.ingest(line);
        }
        fray
    }
}

fn feed() -> Feed {
    Feed {
        state: FeedState::Live,
        last_beat_ago_s: Some(0.2),
        game_running: true,
        game_started_ms: None,
        reframework: true,
        eye_installed: true,
        eye_recording: Some(true),
        eye_reason: None,
    }
}

fn atk(how: Option<&str>, a: Option<u64>, slot: Option<i64>) -> Attacker {
    Attacker {
        how: how.map(str::to_owned),
        a,
        cid: a.map(cid_of),
        slot,
        ..Attacker::default()
    }
}

#[test]
fn should_credit_by_the_analyzers_attacker_label() {
    let mut eye = Eye::new();
    eye.roster(0.0, &[(0, ODALRIC, 5)]);
    let fray = eye.fray();
    let rg = Some(1);
    assert_eq!(fray.credit(rg, None), Credit::Unattributed);
    assert_eq!(
        fray.credit(rg, Some(&atk(Some("gimmick"), None, None))),
        Credit::World
    );
    assert_eq!(
        fray.credit(rg, Some(&atk(Some("none"), None, None))),
        Credit::Unattributed
    );
    assert_eq!(
        fray.credit(rg, Some(&atk(None, Some(ODALRIC), Some(0)))),
        Credit::Unattributed
    );
    assert_eq!(
        fray.credit(rg, Some(&atk(Some("owner"), Some(ODALRIC), Some(2)))),
        Credit::Party(2)
    );
    // No slot written: the roster under the line's generation decides, by address and CharacterID.
    assert_eq!(
        fray.credit(rg, Some(&atk(Some("shell"), Some(ODALRIC), None))),
        Credit::Party(0)
    );
    assert_eq!(
        fray.credit(rg, Some(&atk(Some("shell"), Some(CORVIN), None))),
        Credit::NonParty
    );
    assert_eq!(
        fray.credit(rg, Some(&atk(Some("shell"), None, None))),
        Credit::Unattributed
    );
    assert_eq!(
        fray.credit(Some(9), Some(&atk(Some("shell"), Some(ODALRIC), None))),
        Credit::Unattributed
    );
    assert_eq!(
        fray.credit(rg, Some(&atk(Some("owner"), Some(CORVIN), Some(-1)))),
        Credit::NonParty
    );
}

#[test]
fn should_sum_a_fight_into_two_scopes_that_never_mix() {
    let mut eye = Eye::new();
    eye.party()
        .hit(1.0, 0, 12, 0x3A9F, 1, 400.0, (0.0, 400.0))
        .hit(2.0, 0, 0, 0x3A9F, 1, 100.0, (400.0, 500.0))
        .hit(3.0, 1, 7, 0x51, 1, 250.0, (500.0, 750.0))
        .push(3.5, "hit", json!({"dmg1": 60.0, "atk": {"how": "none"}, "vic": {"vid": 1, "cid": 133179}}))
        .push(4.0, "hit", json!({"dmg1": 40.0, "atk": {"how": "gimmick", "gim": 77}, "vic": {"vid": 1, "cid": 133179}}));
    let frame = eye.fray().frame(feed());
    let f = frame.fight.expect("a fight is open");
    assert_eq!(f.state, FightState::Open);
    assert_eq!(f.h_party, 750);
    assert_eq!(f.unattributed.damage, 60);
    assert_eq!(f.world.as_ref().map(|w| w.damage), Some(40));
    assert_eq!(f.h_all, 850);
    assert_eq!(
        f.ledger,
        Some(750),
        "the game's ledger moves with the party's hits only"
    );
    assert_eq!(f.duration_s, 3.0);

    let odalric = f.members[0].as_ref().expect("the Arisen's card");
    assert_eq!((odalric.damage, odalric.hits), (500, 2));
    assert_eq!(odalric.vocation.id, Some(5));
    let skill = odalric
        .skills
        .iter()
        .find(|s| s.kind == SkillKind::Skill)
        .expect("the custom skill");
    assert_eq!(
        (skill.id.as_str(), skill.damage, skill.biggest),
        ("12", 400, 400)
    );
    let action = odalric
        .skills
        .iter()
        .find(|s| s.kind == SkillKind::Action)
        .expect("the bare action");
    assert_eq!((action.id.as_str(), action.damage), ("3A9F", 100));
    assert_eq!(
        odalric.parts.map(|p| p.iter().sum::<u64>()),
        Some(500),
        "the additive split sums to the damage"
    );
    assert_eq!(f.types, Types::Additive);
    assert_eq!(
        f.biggest.as_ref().map(|b| (b.slot, b.damage)),
        Some((0, 400))
    );
    assert!(
        f.members[2].as_ref().is_some_and(|m| m.damage == 0),
        "a member who dealt nothing still has a card"
    );
}

#[test]
fn should_settle_after_quiet_unless_the_game_says_battle() {
    let mut eye = Eye::new();
    eye.party().hit(1.0, 0, 12, 1, 1, 100.0, (0.0, 100.0)).push(
        5.0,
        "sig",
        json!({"name": "battle", "val": true}),
    );
    eye.push(12.0, "hb", json!({"slots": []}));
    assert!(
        eye.fray()
            .frame(feed())
            .fight
            .is_some_and(|f| f.state == FightState::Open),
        "battle holds it open past the quiet"
    );
    eye.push(13.0, "sig", json!({"name": "battle", "val": false}))
        .push(14.0, "hb", json!({"slots": []}));
    let frame = eye.fray().frame(feed());
    let f = frame.fight.expect("the settled fight stays on the stele");
    assert_eq!(f.state, FightState::Settled);
    assert_eq!(
        f.duration_s, 0.0,
        "a settled fight lasts from its first hit to its last"
    );
    assert_eq!(frame.recent.len(), 1);

    eye.hit(20.0, 1, 7, 1, 2, 50.0, (0.0, 50.0));
    let frame = eye.fray().frame(feed());
    assert_eq!(
        frame.fight.map(|f| (f.n, f.state)),
        Some((2, FightState::Open))
    );
    assert_eq!(frame.recent.len(), 1);
}

#[test]
fn should_settle_even_in_battle_after_the_hard_quiet() {
    let mut eye = Eye::new();
    eye.party()
        .push(0.5, "sig", json!({"name": "battle", "val": true}))
        .hit(1.0, 0, 12, 1, 1, 100.0, (0.0, 100.0));
    eye.push(32.0, "hb", json!({"slots": []}));
    assert_eq!(
        eye.fray().frame(feed()).fight.map(|f| f.state),
        Some(FightState::Settled)
    );
}

#[test]
fn should_settle_at_the_stop_line_and_keep_five() {
    let mut eye = Eye::new();
    eye.party();
    for n in 0..7_u32 {
        let gt = f64::from(n) * 20.0;
        eye.hit(gt + 1.0, 0, 12, 1, u64::from(n) + 1, 10.0, (0.0, 10.0));
    }
    eye.push(200.0, "stop", json!({"why": "time"}));
    let fray = eye.fray();
    assert!(fray.stopped());
    let frame = fray.frame(feed());
    assert_eq!(frame.recent.len(), 5);
    assert_eq!(
        frame.recent.iter().map(|f| f.n).collect::<Vec<_>>(),
        vec![7, 6, 5, 4, 3]
    );
}

#[test]
fn should_mark_the_killing_blow() {
    let mut eye = Eye::new();
    eye.party().hit(1.0, 2, 33, 1, 1, 900.0, (0.0, 900.0));
    eye.push(1.1, "die", json!({"vic": {"vid": 1}, "atk": {"how": "owner", "a": BRAMBLE, "cid": cid_of(BRAMBLE), "slot": 2, "sk": 33}}));
    let f = eye.fray().frame(feed()).fight.expect("a fight");
    assert_eq!(f.kills.len(), 1);
    assert_eq!(
        (f.kills[0].slot, f.kills[0].key.as_deref()),
        (Some(2), Some("33"))
    );
    assert!(f.members[2].as_ref().is_some_and(|m| m.skills[0].last_blow));
}

#[test]
fn should_freeze_a_member_who_leaves_and_not_credit_a_fifth() {
    let mut eye = Eye::new();
    eye.party().hit(1.0, 3, 9, 1, 1, 100.0, (0.0, 100.0));
    eye.roster(
        2.0,
        &[
            (0, ODALRIC, 5),
            (1, WREN, 4),
            (2, BRAMBLE, 6),
            (3, CORVIN, 2),
        ],
    );
    eye.push(
        3.0,
        "hit",
        json!({"dmg1": 70.0, "atk": {"how": "owner", "a": CORVIN, "cid": cid_of(CORVIN), "slot": 3, "sk": 9}, "vic": {"vid": 1, "tdp0": 100.0, "tdp1": 170.0}}),
    );
    let f = eye.fray().frame(feed()).fight.expect("a fight");
    let sable = f.members[3].as_ref().expect("the departed card");
    assert!(sable.left);
    assert_eq!(sable.damage, 100);
    assert_eq!(
        f.unattributed.damage, 70,
        "four places held: a fifth character's hits are nobody's credit"
    );
}

#[test]
fn should_count_lost_records_inside_a_fight() {
    let mut eye = Eye::new();
    eye.party().hit(1.0, 0, 12, 1, 1, 10.0, (0.0, 10.0));
    eye.seq += 3;
    eye.hit(2.0, 0, 12, 1, 1, 10.0, (10.0, 20.0));
    assert_eq!(eye.fray().frame(feed()).fight.map(|f| f.gaps), Some(3));
}

#[test]
fn should_call_the_types_a_mix_when_the_parts_do_not_add_up() {
    let mut eye = Eye::new();
    eye.party().push(
        1.0,
        "hit",
        json!({"dmg1": 100.0, "cmp": [80.0, 0.0, 0.0, 0.0, 0.0, 60.0, 0.0], "atk": {"how": "owner", "a": ODALRIC, "slot": 0, "sk": 1}, "vic": {"vid": 1}}),
    );
    let f = eye.fray().frame(feed()).fight.expect("a fight");
    assert_eq!(f.types, Types::Mix);
    let m = f.members[0].as_ref().expect("a card");
    assert!(m.parts.is_none());
    assert_eq!(m.mix, Some([80, 0, 0, 0, 0, 60, 0]));
}

/// The Heretic's finding on PR #3: 100 whole slash and a 1000 magick roll capped at the 10 HP
/// its foe had credit 110; the parts must say 100 slash and 10 magick, not the rolls rescaled.
#[test]
fn should_cap_a_hits_parts_with_its_credit() {
    let mut eye = Eye::new();
    eye.party().push(
        1.0,
        "hit",
        json!({"dmg1": 100.0, "cmp": [100.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], "atk": {"how": "owner", "a": ODALRIC, "slot": 0, "sk": 1}, "vic": {"vid": 1, "hp0": 500.0}}),
    );
    eye.push(
        1.5,
        "hit",
        json!({"dmg1": 1000.0, "cmp": [0.0, 0.0, 0.0, 0.0, 0.0, 1000.0, 0.0], "atk": {"how": "owner", "a": ODALRIC, "slot": 0, "sk": 2}, "vic": {"vid": 1, "hp0": 10.0}}),
    );
    let f = eye.fray().frame(feed()).fight.expect("a fight");
    assert_eq!(f.types, Types::Additive);
    let m = f.members[0].as_ref().expect("a card");
    assert_eq!(m.damage, 110);
    assert_eq!(m.parts, Some([100, 0, 0, 0, 0, 10, 0]));
    assert_eq!(m.mix, Some([100, 0, 0, 0, 0, 10, 0]));
}

#[test]
fn should_read_null_as_unknown_never_zero() {
    let line = parse_line(r#"{"v":0,"seq":5,"rg":1,"t":1.0,"gt":"nan","k":"hit","dmg1":null,"cmp":[1,2,3],"vic":{"vid":0},"atk":7}"#).expect("parses");
    assert_eq!(line.gt, None);
    let crate::record::Record::Hit(hit) = line.body else {
        panic!("a hit")
    };
    assert_eq!((hit.dmg, hit.cmp, hit.atk), (None, None, None));
    assert_eq!(hit.vic.and_then(|v| v.vid), None, "vid 0 is no foe");
}

#[test]
fn should_serialize_the_window_contract_in_camel_case() {
    let mut eye = Eye::new();
    eye.party().hit(1.0, 0, 12, 1, 1, 10.0, (0.0, 10.0));
    let v = serde_json::to_value(eye.fray().frame(feed())).expect("serializes");
    for key in [
        "hParty",
        "hAll",
        "durationS",
        "partyFormed",
        "endedAt",
        "unattributed",
        "kills",
    ] {
        assert!(v["fight"].get(key).is_some(), "fight.{key}");
    }
    assert_eq!(v["fight"]["members"][0]["role"], "arisen");
    assert_eq!(v["fight"]["members"][1]["role"], "mainPawn");
    assert_eq!(v["feed"]["state"], "live");
    assert!(v["fight"]["members"][0].get("noDamageSkills").is_some());
    assert_eq!(v["fight"]["members"][0]["skills"][0]["firstAt"], 0.0);
}

#[test]
fn should_write_the_settled_time_from_the_session_clock() {
    let mut eye = Eye::new();
    eye.party().hit(1.0, 0, 12, 1, 1, 10.0, (0.0, 10.0));
    let mut fray = Fray::new(
        SES,
        Some(crate::SessionClock {
            clock: 4.0,
            unix: 1_790_000_004,
        }),
        Settings::default(),
    )
    .with_time_format(|unix| {
        chrono::DateTime::from_timestamp(unix, 0)
            .map(|t| t.format("%H:%M:%S").to_string())
            .unwrap_or_default()
    });
    for line in &eye.lines {
        fray.ingest(line);
    }
    fray.finish();
    // The hit's os.clock is 5.0: one second after the start at 14:13:24 UTC.
    assert_eq!(
        fray.frame(feed()).fight.and_then(|f| f.ended_at_ms),
        Some(1_790_000_005_000)
    );
    assert_eq!(
        fray.frame(feed()).fight.and_then(|f| f.ended_at).as_deref(),
        Some("14:13:25")
    );
}

#[test]
fn should_tail_whole_lines_only_and_pick_the_newest_session() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let p = dir.path();
    std::fs::write(
        p.join("session-1790000001_aaaaaa.json"),
        r#"{"started":{"clock":1.0,"time":1790000001}}"#,
    )
    .expect("write");
    std::fs::write(
        p.join(format!("session-{SES}.json")),
        r#"{"started":{"clock":4.0,"time":1790000004}}"#,
    )
    .expect("write");
    std::fs::write(p.join("session-nonsense.json"), "{}").expect("write");
    assert_eq!(newest_session(p).as_deref(), Some(SES));
    assert_eq!(session_clock(p, SES).map(|c| c.unix), Some(1_790_000_004));

    let line = |seq: u64| format!(r#"{{"v":0,"seq":{seq},"t":1.0,"gt":1.0,"k":"hb","slots":[]}}"#);
    let seg0 = p.join(format!("events-{SES}-0.ndjson"));
    std::fs::write(
        &seg0,
        format!("{}\n{}\n{}", line(1), line(2), &line(3)[..10]),
    )
    .expect("write");
    let mut tail = Tail::new(p, SES);
    let first = tail.poll().expect("poll");
    assert_eq!(first.iter().map(|l| l.seq).collect::<Vec<_>>(), vec![1, 2]);

    std::fs::write(&seg0, format!("{}\n{}\n{}\n", line(1), line(2), line(3))).expect("write");
    std::fs::write(
        p.join(format!("events-{SES}-1.ndjson")),
        format!("{}\nnot json\n", line(4)),
    )
    .expect("write");
    let second = tail.poll().expect("poll");
    assert_eq!(second.iter().map(|l| l.seq).collect::<Vec<_>>(), vec![3, 4]);
    assert_eq!(tail.bad_lines, 1);
    assert!(tail.poll().expect("poll").is_empty());
}

/// The Heretic's finding on PR #3: a later segment the OS will not read must not cost the lines
/// an earlier segment gave in the same poll.
#[test]
fn should_keep_the_lines_read_before_a_segment_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    let p = dir.path();
    let line = |seq: u64| format!(r#"{{"v":0,"seq":{seq},"t":1.0,"gt":1.0,"k":"hb","slots":[]}}"#);
    std::fs::write(
        p.join(format!("events-{SES}-0.ndjson")),
        format!("{}\n{}\n", line(1), line(2)),
    )
    .expect("write");
    // A directory under the segment's name: it opens, and reading it fails.
    let seg1 = p.join(format!("events-{SES}-1.ndjson"));
    std::fs::create_dir(&seg1).expect("mkdir");
    let mut tail = Tail::new(p, SES);

    let first = tail.poll().expect("the lines before the failure come back");
    assert_eq!(first.iter().map(|l| l.seq).collect::<Vec<_>>(), vec![1, 2]);
    assert!(
        tail.poll().is_err(),
        "with nothing read, the failure is said"
    );

    std::fs::remove_dir(&seg1).expect("rmdir");
    std::fs::write(&seg1, format!("{}\n", line(3))).expect("write");
    let after = tail.poll().expect("poll");
    assert_eq!(after.iter().map(|l| l.seq).collect::<Vec<_>>(), vec![3]);
}

#[test]
fn should_carry_the_party_outside_a_fight() {
    let mut eye = Eye::new();
    assert!(
        eye.fray().frame(feed()).party.is_empty(),
        "no roster read yet"
    );
    eye.roster(0.0, &[(0, ODALRIC, 5), (2, BRAMBLE, 6)]).push(
        0.5,
        "hb",
        json!({"slots": [{"slot": 0, "hp": 30.0, "maxhp": 120.0, "st": 10.0, "stmax": 10.0}]}),
    );
    let frame = eye.fray().frame(feed());
    assert!(frame.fight.is_none());
    assert_eq!(frame.party.len(), 4);
    let arisen = frame.party[0].as_ref().expect("the Arisen's place");
    assert_eq!(
        (arisen.hp, arisen.st, arisen.vocation.id),
        (Some(0.25), Some(1.0), Some(5))
    );
    assert!(frame.party[1].is_none(), "an empty place");
    assert!(frame.party[2]
        .as_ref()
        .is_some_and(|m| m.damage == 0 && m.skills.is_empty()));
}

#[test]
fn should_keep_a_card_through_a_slot_change_and_a_respawn() {
    // The first real session: a hired pawn left, the Mage moved from slot 3 to slot 2, and came
    // back from a respawn under a new address. She is one person throughout.
    let mut eye = Eye::new();
    eye.party().hit(1.0, 3, 9, 1, 1, 100.0, (0.0, 100.0));
    eye.roster(2.0, &[(0, ODALRIC, 5), (1, WREN, 4), (2, SABLE, 3)]);
    let respawned = SABLE + 0x10_0000;
    eye.push(
        3.0,
        "hit",
        json!({"dmg1": 60.0, "atk": {"how": "owner", "a": respawned, "cid": cid_of(SABLE), "slot": 2, "sk": 9}, "vic": {"vid": 1}}),
    );
    eye.roster(4.0, &[(0, ODALRIC, 5), (1, WREN, 4), (2, CORVIN, 2)]);
    let f = eye.fray().frame(feed()).fight.expect("a fight");
    let sable = f.members[3].as_ref().expect("her card stays in its place");
    assert_eq!((sable.damage, sable.hits, sable.left), (160, 2, true));
    let bramble = f.members[2].as_ref().expect("the departed sorcerer's card");
    assert!(bramble.left);
    assert_eq!(f.unattributed.damage, 0);
}

#[test]
fn should_cap_the_credit_at_the_hp_the_foe_had() {
    let mut eye = Eye::new();
    eye.party().push(
        1.0,
        "hit",
        json!({"dmg1": 12245.0, "cmp": [0.0, 0.0, 0.0, 12245.0, 0.0, 0.0, 0.0],
               "atk": {"how": "shell", "a": BRAMBLE, "cid": cid_of(BRAMBLE), "slot": 2, "shell": {"paramid": 7}},
               "vic": {"vid": 1, "hp0": 3000.0, "hp1": 0.0, "tdp0": 0.0, "tdp1": 3000.0}}),
    );
    eye.push(
        1.5,
        "hit",
        json!({"dmg1": 900.0, "atk": {"how": "shell", "a": BRAMBLE, "cid": cid_of(BRAMBLE), "slot": 2, "shell": {"paramid": 7}},
               "vic": {"vid": 2, "hp0": 0.0, "hp1": 0.0}}),
    );
    let f = eye.fray().frame(feed()).fight.expect("a fight");
    let bramble = f.members[2].as_ref().expect("a card");
    assert_eq!(
        (bramble.damage, bramble.hits),
        (3000, 1),
        "overkill and a dead target take no HP"
    );
    assert_eq!(
        bramble.parts.map(|p| p[3]),
        Some(3000),
        "the parts follow the credited damage"
    );
    assert_eq!(f.ledger, Some(3000));
}

#[test]
fn should_open_fights_on_party_damage_only_and_keep_others_apart() {
    let mut eye = Eye::new();
    eye.party()
        .push(0.5, "hit", json!({"dmg1": 586.0, "atk": {"how": "gimmick"}, "vic": {"vid": 9}}))
        .push(0.6, "hit", json!({"dmg1": 90.0, "atk": {"how": "owner", "a": CORVIN, "cid": cid_of(CORVIN), "slot": -1}, "vic": {"vid": 9}}));
    assert!(
        eye.fray().frame(feed()).fight.is_none(),
        "the world and the others open no fight"
    );
    eye.hit(1.0, 0, 12, 1, 1, 100.0, (0.0, 100.0))
        .push(1.2, "hit", json!({"dmg1": 40.0, "atk": {"how": "owner", "a": CORVIN, "cid": cid_of(CORVIN), "slot": -1}, "vic": {"vid": 1}}))
        .push(1.3, "hit", json!({"dmg1": 55.0, "atk": {"how": "owner", "a": CORVIN, "cid": cid_of(CORVIN), "slot": -1}, "vic": {"vid": 9}}));
    let f = eye.fray().frame(feed()).fight.expect("a fight");
    assert_eq!(
        f.others.as_ref().map(|o| (o.damage, o.hits)),
        Some((40, 1)),
        "only on the party's foes"
    );
    assert_eq!(f.h_all, 100, "the others stand outside every source");
    assert!(f.world.is_none());
}
