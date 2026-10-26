//! fray-read — read one eye session from its folder and print its fights, numbers only.
//!
//! ```text
//! fray-read <the eye's folder> [--ses <id>] [--json] [--quiet-s <seconds>]
//! ```
//!
//! It prints ids, never names: no member, foe, skill or resource name leaves the game folder
//! through it, so its output can be pasted into a review. `--json` prints the frames instead.

use std::path::PathBuf;
use std::process::ExitCode;

use rift_fray::frame::{Feed, FeedState, Fight, SkillKind};
use rift_fray::{newest_session, session_clock, Fray, Settings, Tail};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut dir: Option<PathBuf> = None;
    let mut ses: Option<String> = None;
    let mut json = false;
    // A readout shows every fight of the session, not only the screen's shelf of five.
    let mut settings = Settings {
        shelf: usize::MAX,
        ..Settings::default()
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--ses" => ses = args.next(),
            "--json" => json = true,
            "--quiet-s" => match args.next().and_then(|s| s.parse().ok()) {
                Some(s) => settings.quiet_s = s,
                None => return usage("--quiet-s takes a number of game seconds"),
            },
            _ if dir.is_none() => dir = Some(PathBuf::from(arg)),
            _ => return usage("one folder only"),
        }
    }
    let Some(dir) = dir else {
        return usage("name the eye's folder");
    };
    let Some(ses) = ses.or_else(|| newest_session(&dir)) else {
        eprintln!(
            "The stone found no session in that folder. Has the eye run since it was installed?"
        );
        return ExitCode::FAILURE;
    };

    let mut fray = Fray::new(&ses, session_clock(&dir, &ses), settings);
    let mut tail = Tail::new(&dir, &ses);
    let lines = match tail.poll() {
        Ok(lines) => lines,
        Err(e) => {
            eprintln!("The stone could not read the session's lines: {e}");
            return ExitCode::FAILURE;
        }
    };
    let read = lines.len();
    for line in &lines {
        fray.ingest(line);
    }
    let stopped = fray.stopped();
    fray.finish();

    let feed = Feed {
        state: FeedState::None,
        last_beat_ago_s: None,
        game_running: false,
        game_started_ms: None,
        reframework: true,
        eye_installed: true,
        eye_recording: None,
        eye_reason: None,
    };
    let frame = fray.frame(feed);
    if json {
        match serde_json::to_string_pretty(&frame) {
            Ok(text) => println!("{text}"),
            Err(e) => {
                eprintln!("The frame would not serialize: {e}");
                return ExitCode::FAILURE;
            }
        }
        return ExitCode::SUCCESS;
    }

    println!(
        "session {ses}: {read} lines read, {} unreadable, {}",
        tail.bad_lines,
        if stopped {
            "the eye's stop line is on disk"
        } else {
            "no stop line (the eye may still run, or stopped without one)"
        }
    );
    println!("{} fights", frame.recent.len());
    for fight in frame.recent.iter().rev() {
        print_fight(fight);
    }
    ExitCode::SUCCESS
}

fn usage(why: &str) -> ExitCode {
    eprintln!(
        "{why}\nusage: fray-read <the eye's folder> [--ses <id>] [--json] [--quiet-s <seconds>]"
    );
    ExitCode::FAILURE
}

fn print_fight(f: &Fight) {
    let foes: Vec<String> = f
        .foes
        .iter()
        .map(|x| {
            format!(
                "cid {} x{}",
                x.cid.map_or("?".into(), |c| c.to_string()),
                x.count
            )
        })
        .collect();
    println!();
    println!(
        "FIGHT {} · {:.1} s · ended {} · {}",
        f.n,
        f.duration_s,
        f.ended_at.as_deref().unwrap_or("?"),
        foes.join(", ")
    );
    let honesty = match f.ledger {
        Some(l) if l > 0 => format!(
            "{:.1}% of the game's ledger {l}",
            100.0 * f.h_party as f64 / l as f64
        ),
        _ => "no ledger".into(),
    };
    println!(
        "  party {} · every source {} · {honesty} · types {:?} · gaps {}",
        f.h_party, f.h_all, f.types, f.gaps
    );
    for m in f.members.iter().flatten() {
        let dps = m.dps.map_or("—".into(), |d| format!("{d:.0}/s"));
        let share = if f.h_all > 0 {
            100.0 * m.damage as f64 / f.h_all as f64
        } else {
            0.0
        };
        println!(
            "  slot {} {:?} job {}: {} dmg ({share:.0}%), {dps}, {} hits{}",
            m.slot,
            m.role,
            m.vocation.id.map_or("?".into(), |j| j.to_string()),
            m.damage,
            m.hits,
            if m.left { ", left" } else { "" }
        );
        let mut skills: Vec<_> = m.skills.iter().collect();
        skills.sort_by_key(|s| std::cmp::Reverse(s.damage));
        for s in skills.iter().take(6) {
            let kind = match s.kind {
                SkillKind::Skill => "skill",
                SkillKind::Action => "action",
                SkillKind::Unknown => "unknown",
            };
            let share = if m.damage > 0 {
                100.0 * s.damage as f64 / m.damage as f64
            } else {
                0.0
            };
            println!(
                "      {kind} {}: {} ({share:.0}%), {} hits, biggest {}{}",
                s.id,
                s.damage,
                s.hits,
                s.biggest,
                if s.last_blow { ", a killing blow" } else { "" }
            );
        }
        if let Some(p) = m.parts {
            println!(
                "      parts slash {} blow {} shoot {} magick {} enchant {} element {} fixed {}",
                p[0], p[1], p[2], p[3], p[4], p[5], p[6]
            );
        }
    }
    println!(
        "  unattributed {} ({} hits)",
        f.unattributed.damage, f.unattributed.hits
    );
    if let Some(w) = &f.world {
        println!("  the world {} ({} hits)", w.damage, w.hits);
    }
    if let Some(o) = &f.others {
        println!(
            "  others, outside the party and outside every source {} ({} hits)",
            o.damage, o.hits
        );
    }
    println!("  kills {}", f.kills.len());
}
