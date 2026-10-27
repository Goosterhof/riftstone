//! THE FRAY's desk (#00015 §19): the eye's folder read as it grows, one frame per pull.
//!
//! The eye is the REFramework script in `<game>\reframework\autorun\`; it writes its lines under
//! `<game>\reframework\data\riftstone\probe\`. The desk follows the newest session there, feeds
//! every new line to [`rift_fray::Fray`], and answers the `fray` window's `fray_frame` with the
//! fight on the stele and the shelf. It reads the eye's own folder and checks the eye's one file
//! by its exact path: it never lists the autorun folder, whose other scripts are the player's.
//! Nothing here writes.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rift_fray::frame::{Feed, FeedState, Frame, SkillKind};
use rift_fray::{newest_session, session_clock, Fray, Settings, Tail};
use rift_muster::Codex;

use crate::state::lock;

/// The eye's file name in the autorun folder.
pub const EYE_SCRIPT: &str = "riftstone_probe.lua";

/// A feed is stale once no heartbeat has arrived for this long (FQ.3's fallback).
pub const STALE_AFTER: Duration = Duration::from_secs(3);

/// The eye's paths under one game folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EyePaths {
    pub reframework: PathBuf,
    pub script: PathBuf,
    pub data: PathBuf,
}

impl EyePaths {
    pub fn under(game_root: &Path) -> Self {
        let reframework = game_root.join("reframework");
        EyePaths {
            script: reframework.join("autorun").join(EYE_SCRIPT),
            data: reframework.join("data").join("riftstone").join("probe"),
            reframework,
        }
    }
}

struct Following {
    dir: PathBuf,
    tail: Tail,
    fray: Fray,
    /// When the desk last read a heartbeat line (the eye's os.clock is not the wall's).
    beat_seen: Option<Instant>,
    beat_t: Option<f64>,
}

#[derive(Default)]
struct Desk {
    following: Option<Following>,
    codex: Option<Option<Arc<Codex>>>,
}

/// The desk, managed beside the spine.
#[derive(Default)]
pub struct FrayDesk {
    desk: Mutex<Desk>,
}

/// What the spine knows of the game when the window pulls.
#[derive(Debug, Clone, Default)]
pub struct GameView {
    pub root: Option<PathBuf>,
    pub running: bool,
    /// When the spine first saw this run, epoch ms.
    pub started_ms: Option<u64>,
}

impl FrayDesk {
    /// Read whatever the eye wrote since the last pull and build the frame. `codex` names the
    /// vocations from the game's own text; it is asked once and kept.
    pub fn frame(
        &self,
        game: &GameView,
        now: Instant,
        codex: impl FnOnce() -> Option<Arc<Codex>>,
    ) -> Frame {
        let mut desk = lock(&self.desk);
        let paths = game.root.as_deref().map(EyePaths::under);
        let reframework = paths.as_ref().is_some_and(|p| p.reframework.is_dir());
        let eye_installed = paths.as_ref().is_some_and(|p| p.script.is_file());

        if let Some(paths) = &paths {
            desk.follow(&paths.data, now);
        }
        if desk.codex.is_none() && desk.following.is_some() {
            desk.codex = Some(codex());
        }

        let (state, ago, recording, reason) = match &desk.following {
            None => (FeedState::None, None, None, None),
            Some(f) => {
                let ago = f
                    .beat_seen
                    .map(|seen| now.saturating_duration_since(seen).as_secs_f64());
                let fresh = ago.is_some_and(|a| a < STALE_AFTER.as_secs_f64());
                if f.fray.stopped() {
                    (
                        FeedState::None,
                        ago,
                        Some(false),
                        Some("stopped".to_owned()),
                    )
                } else if !game.running {
                    (FeedState::None, ago, None, None)
                } else if fresh {
                    (FeedState::Live, ago, Some(true), None)
                } else {
                    (FeedState::Stale, ago, None, None)
                }
            }
        };
        let feed = Feed {
            state,
            last_beat_ago_s: ago,
            game_running: game.running,
            game_started_ms: game.started_ms,
            reframework,
            eye_installed,
            eye_recording: recording,
            eye_reason: reason,
        };
        let mut frame = match &desk.following {
            Some(f) => f.fray.frame(feed),
            None => Frame {
                feed,
                party: Vec::new(),
                fight: None,
                recent: Vec::new(),
            },
        };
        if let Some(Some(codex)) = &desk.codex {
            name_from_codex(&mut frame, codex);
        }
        frame
    }
}

impl Desk {
    fn follow(&mut self, dir: &Path, now: Instant) {
        let Some(ses) = newest_session(dir) else {
            return;
        };
        let same = self
            .following
            .as_ref()
            .is_some_and(|f| f.dir == dir && f.tail.session() == ses);
        if !same {
            self.following = Some(Following {
                dir: dir.to_path_buf(),
                fray: Fray::new(&ses, session_clock(dir, &ses), Settings::default()),
                tail: Tail::new(dir, &ses),
                beat_seen: None,
                beat_t: None,
            });
        }
        let Some(f) = &mut self.following else { return };
        let lines = match f.tail.poll() {
            Ok(lines) => lines,
            Err(err) => {
                log::warn!("the Fray could not read the eye's lines: {}", err.kind());
                return;
            }
        };
        for line in &lines {
            f.fray.ingest(line);
        }
        let beat = f.fray.last_beat_t();
        if beat.is_some() && beat != f.beat_t {
            // A session read for the first time may be an old one: its beats are not "now".
            if same {
                f.beat_seen = Some(now);
            }
            f.beat_t = beat;
        }
    }
}

/// Names the vocations and the custom skills from the game's own text. The eye's `sk` is the id
/// THE MUSTER reads from the save (measured on the first real session: every skill a member used
/// fell in its own vocation's block), so the same codex names both. Actions and shells keep their
/// ids: the game's text has no names for them.
fn name_from_codex(frame: &mut Frame, codex: &Codex) {
    let fights = frame.fight.iter_mut().chain(frame.recent.iter_mut());
    let in_fights = fights.flat_map(|f| f.members.iter_mut()).flatten();
    for member in in_fights.chain(frame.party.iter_mut().flatten()) {
        if let Some(id) = member.vocation.id.and_then(|id| u32::try_from(id).ok()) {
            member.vocation.name = codex.vocation(id).map(str::to_owned);
        }
        for skill in &mut member.skills {
            if skill.kind == SkillKind::Skill {
                skill.name = skill
                    .id
                    .parse::<u32>()
                    .ok()
                    .and_then(|id| codex.skill(id))
                    .map(str::to_owned);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SES: &str = "1790000004_8120e8";

    fn eye(root: &Path, lines: &[&str]) -> EyePaths {
        let paths = EyePaths::under(root);
        std::fs::create_dir_all(&paths.data).unwrap();
        std::fs::create_dir_all(paths.script.parent().unwrap()).unwrap();
        std::fs::write(
            paths.data.join(format!("session-{SES}.json")),
            r#"{"started":{"clock":4.0,"time":1790000004}}"#,
        )
        .unwrap();
        std::fs::write(
            paths.data.join(format!("events-{SES}-0.ndjson")),
            lines.join("\n") + "\n",
        )
        .unwrap();
        paths
    }

    const ROSTER: &str = r#"{"seq":1,"rg":1,"t":4.0,"gt":0.0,"k":"rost","members":[{"slot":0,"a":4096,"cid":1,"job":5}]}"#;
    const HIT: &str = r#"{"seq":2,"rg":1,"t":5.0,"gt":1.0,"k":"hit","dmg1":120.0,"atk":{"how":"owner","a":4096,"cid":1,"slot":0,"sk":3},"vic":{"vid":1,"cid":9,"tdp0":0.0,"tdp1":120.0}}"#;
    const BEAT: &str = r#"{"seq":3,"rg":1,"t":5.5,"gt":1.5,"k":"hb","slots":[{"slot":0,"hp":50.0,"maxhp":100.0}]}"#;

    #[test]
    fn should_say_none_without_a_game_folder() {
        let frame = FrayDesk::default().frame(&GameView::default(), Instant::now(), || None);
        assert_eq!(frame.feed.state, FeedState::None);
        assert!(!frame.feed.eye_installed);
        assert!(frame.fight.is_none());
    }

    #[test]
    fn should_follow_the_eye_and_go_stale_without_beats() {
        let dir = tempfile::tempdir().unwrap();
        let paths = eye(dir.path(), &[ROSTER, HIT]);
        std::fs::write(&paths.script, "-- the eye").unwrap();
        let desk = FrayDesk::default();
        let game = GameView {
            root: Some(dir.path().to_path_buf()),
            running: true,
            started_ms: Some(1_790_000_000_000),
        };
        let t0 = Instant::now();

        let first = desk.frame(&game, t0, || None);
        assert_eq!(first.feed.game_started_ms, Some(1_790_000_000_000));
        assert!(
            first.party[0].is_some(),
            "the eye's roster stands outside the fight too"
        );
        assert!(first.feed.eye_installed && first.feed.reframework);
        assert_eq!(first.fight.as_ref().map(|f| f.h_party), Some(120));
        assert_eq!(first.feed.state, FeedState::Stale, "no heartbeat read yet");

        std::fs::write(
            paths.data.join(format!("events-{SES}-0.ndjson")),
            [ROSTER, HIT, BEAT].join("\n") + "\n",
        )
        .unwrap();
        let live = desk.frame(&game, t0 + Duration::from_secs(1), || None);
        assert_eq!(live.feed.state, FeedState::Live);
        assert_eq!(
            live.fight
                .as_ref()
                .and_then(|f| f.members[0].as_ref())
                .and_then(|m| m.hp),
            Some(0.5)
        );

        let later = desk.frame(&game, t0 + Duration::from_secs(5), || None);
        assert_eq!(later.feed.state, FeedState::Stale);

        let closed = desk.frame(
            &GameView {
                running: false,
                ..game
            },
            t0 + Duration::from_secs(6),
            || None,
        );
        assert_eq!(closed.feed.state, FeedState::None);
        assert!(
            closed.fight.is_some(),
            "the last fight stays on the stele after the game closes"
        );
    }

    #[test]
    fn should_name_skills_and_vocations_from_the_codex_in_and_out_of_the_fight() {
        let dir = tempfile::tempdir().unwrap();
        let action = r#"{"seq":3,"rg":1,"t":5.2,"gt":1.2,"k":"hit","dmg1":30.0,"atk":{"how":"owner","a":4096,"cid":1,"slot":0,"ah":4096},"vic":{"vid":1,"cid":9,"tdp0":120.0,"tdp1":150.0}}"#;
        let unnamed = r#"{"seq":4,"rg":1,"t":5.4,"gt":1.4,"k":"hit","dmg1":10.0,"atk":{"how":"owner","a":4096,"cid":1,"slot":0,"sk":77},"vic":{"vid":1,"cid":9,"tdp0":150.0,"tdp1":160.0}}"#;
        eye(dir.path(), &[ROSTER, HIT, action, unnamed]);
        let game = GameView {
            root: Some(dir.path().to_path_buf()),
            running: true,
            started_ms: None,
        };
        // Invented names only: the game's own text never enters the repo.
        let codex: Codex = serde_json::from_value(serde_json::json!({
            "items": {}, "vocations": {"5": "Brawler"}, "inclinations": {},
            "specializations": {}, "augments": {}, "skills": {"3": "Gullwing Rush"},
        }))
        .unwrap();
        let codex = Arc::new(codex);
        let frame = FrayDesk::default().frame(&game, Instant::now(), || Some(codex));

        let member = frame
            .fight
            .as_ref()
            .and_then(|f| f.members[0].as_ref())
            .unwrap();
        assert_eq!(member.vocation.name.as_deref(), Some("Brawler"));
        let names: Vec<_> = member
            .skills
            .iter()
            .map(|k| (k.kind, k.id.as_str(), k.name.as_deref()))
            .collect();
        assert!(names.contains(&(SkillKind::Skill, "3", Some("Gullwing Rush"))));
        assert!(
            names.contains(&(SkillKind::Skill, "77", None)),
            "a skill the codex lacks keeps its id"
        );
        assert!(
            names
                .iter()
                .any(|(kind, _, name)| *kind == SkillKind::Action && name.is_none()),
            "an action is never named from the skill text"
        );
        let outside = frame.party[0].as_ref().unwrap();
        assert_eq!(
            outside.vocation.name.as_deref(),
            Some("Brawler"),
            "the party outside a fight is named too"
        );
    }

    #[test]
    fn should_say_stopped_after_the_eyes_stop_line() {
        let dir = tempfile::tempdir().unwrap();
        let stop = r#"{"seq":4,"rg":1,"t":6.0,"gt":2.0,"k":"stop","why":"time"}"#;
        eye(dir.path(), &[ROSTER, HIT, BEAT, stop]);
        let game = GameView {
            root: Some(dir.path().to_path_buf()),
            running: true,
            started_ms: Some(1_790_000_000_000),
        };
        let frame = FrayDesk::default().frame(&game, Instant::now(), || None);
        assert_eq!(
            (frame.feed.eye_recording, frame.feed.eye_reason.as_deref()),
            (Some(false), Some("stopped"))
        );
        assert_eq!(
            frame.fight.map(|f| f.state),
            Some(rift_fray::frame::FightState::Settled)
        );
    }
}
