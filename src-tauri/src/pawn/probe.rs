//! The fence's probe (BUILD-SPEC § v0.7, *The probe: deterministic, no
//! model*): small processes in the **identical** fence, each one's result read
//! from outside it. Pure here: the steps' scripts, the judgement of what was
//! observed, the record (`pawn-fence.json`) and the chip it earns. The desk
//! runs the steps and reads the disk.
//!
//! Every step's script avoids the double quote (`wsl.exe` does not read `\"`
//! back), and every path in it is single-quoted.
//!
//! A refusal counts only when the stone saw it happen (the Heretic's WP1):
//! the step began inside the fence (its first line is the run's marker), it
//! finished, the place it aimed at exists outside, and what came back is the
//! refusal itself. Anything less is `inconclusive`, never `held`. A record
//! earns `contained` only when it is whole: this probe's version, every row of
//! the table and both broker checks, each `held` (WP2).

use serde::{Deserialize, Serialize};

use crate::pawn::fence::{sh_quote, Ground};

/// What a step must do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Expect {
    Land,
    NotLand,
    Fail,
    Succeed,
}

/// The places a probe aims at, as the fence names them, and the fresh nonce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Targets {
    pub nonce: String,
    /// Printed first by every step: the proof that the step began in the fence.
    pub marker: String,
    /// `/mnt/<d>/…/mods` (not mounted in the fence).
    pub mods: String,
    /// The game folder, when the stone knows it.
    pub game: Option<String>,
    /// Steam's `userdata`, when the stone knows Steam.
    pub userdata: Option<String>,
    /// The canary the stone wrote in its own app data (the fence's path).
    pub canary: String,
    /// The distro user's real home.
    pub home: String,
    pub claude_bin: String,
    /// The Windows path (`C:\…\mods`) a Windows `cmd.exe` would write into.
    pub mods_windows: String,
    /// Where WSL mounts Windows' own `cmd.exe`.
    pub cmd_exe: String,
}

/// One step: its contract id, what it must do, and its script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub id: &'static str,
    pub expect: Expect,
    pub script: String,
}

/// The probe-only input's mount inside the fence.
pub const PROBE_INPUT: &str = "/inputs/probe";

/// `TIOCSTI` on Linux.
const TIOCSTI: u32 = 0x5412;

/// The record's shape and the table it was judged by. A record of another
/// version proves nothing about this probe (the Heretic's WP2).
pub const PROBE_VERSION: u32 = 2;

/// The contract's table after the control, in order: the rows a whole record
/// carries, each with what it must do.
pub const TABLE: [(&str, Expect); 12] = [
    ("1", Expect::NotLand),
    ("2", Expect::NotLand),
    ("3", Expect::Fail),
    ("4", Expect::Fail),
    ("5", Expect::NotLand),
    ("6", Expect::NotLand),
    ("7", Expect::NotLand),
    ("8", Expect::Succeed),
    ("9", Expect::Fail),
    ("10", Expect::Fail),
    ("11", Expect::Fail),
    ("12", Expect::Fail),
];

/// The two broker checks that run beside the table, outside the fence
/// (BUILD-SPEC § v0.7): links refused unread; a bad name and 64 KiB + 1
/// refused unparsed.
pub const BROKER: [&str; 2] = ["B1", "B2"];

/// The contract's table, C and 1–12, for these targets.
pub fn steps(t: &Targets) -> Vec<Step> {
    let n = &t.nonce;
    let q = |path: &str| sh_quote(path);
    let marker = &t.marker;
    let step = |id, expect, script: String| Step {
        id,
        expect,
        script: format!("echo {marker}; {script}"),
    };
    let pe_target = format!("echo x > {}\\probe-pe-{n}.txt", t.mods_windows);
    vec![
        step(
            "C",
            Expect::Land,
            format!("mkdir -p probe && printf %s {n} > probe/control.txt"),
        ),
        step(
            "1",
            Expect::NotLand,
            format!("printf x > {}", q(&format!("{}/probe-{n}.txt", t.mods))),
        ),
        step(
            "2",
            Expect::NotLand,
            match &t.game {
                Some(game) => format!("printf x > {}", q(&format!("{game}/probe-{n}.txt"))),
                None => "exit 1".into(),
            },
        ),
        step(
            "3",
            Expect::Fail,
            match &t.userdata {
                Some(userdata) => format!("ls {}", q(userdata)),
                None => "exit 1".into(),
            },
        ),
        step("4", Expect::Fail, format!("cat {}", q(&t.canary))),
        step(
            "5",
            Expect::NotLand,
            format!(
                "printf x > {}",
                q(&format!("{}/.claude/riftstone-probe-{n}", t.home))
            ),
        ),
        step(
            "6",
            Expect::NotLand,
            format!("./probe/planted.exe /c {}", q(&pe_target)),
        ),
        step(
            "7",
            Expect::NotLand,
            format!("{} /c {}", q(&t.cmd_exe), q(&pe_target)),
        ),
        step("8", Expect::Succeed, format!("{} --version", q(&t.claude_bin))),
        step(
            "9",
            Expect::Fail,
            format!("cat {}/link/probe-canary.txt", PROBE_INPUT),
        ),
        step("10", Expect::Fail, "env".into()),
        step(
            "11",
            Expect::Fail,
            "ln -s /tmp /stone/mailbox/answers/x.json; printf x > /stone/mailbox/answers/y.json"
                .into(),
        ),
        // The ioctl itself, through libc, and its errno as the exit status:
        // 41 is EPERM (the filter), 42 is EIO (the kernel's own refusal), 0 is
        // allowed, 40 any other errno. A Python that fails before the call
        // exits 1, which shows nothing.
        step(
            "12",
            Expect::Fail,
            format!(
                "python3 -c 'import ctypes, pty, sys; m, s = pty.openpty(); l = ctypes.CDLL(None, use_errno=True); r = l.ioctl(s, {TIOCSTI}, ctypes.c_char_p(bytes([120]))); e = ctypes.get_errno(); sys.exit(0 if r == 0 else 41 if e == 1 else 42 if e == 5 else 40)'"
            ),
        ),
    ]
}

/// How a step's process ended, as the stone's runner saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ran {
    /// The command could not be built or started: nothing ran.
    NotStarted(String),
    /// It ran past the bound and was killed.
    TimedOut,
    /// It ended without an exit status (a signal).
    Killed,
    Exited(i32),
}

/// What a step did, as the stone saw it from outside the fence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub ran: Ran,
    /// The step's stdout and stderr together.
    pub output: String,
    /// For a write step: did the file appear on the real disk? `None` when
    /// the stone could not look.
    pub landed: Option<bool>,
    /// The facts outside the fence that make this step's result mean
    /// something (the place it aims at exists, the link was made, the secret
    /// reaches WSL without the fence). `Err` names the missing one.
    pub ready: Result<(), String>,
}

/// What a step showed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Verdict {
    /// The step ran in the fence and the stone saw the refusal (for the
    /// control and step 8: saw it succeed).
    Held,
    /// Something crossed that the fence should close.
    Breached,
    /// The step did not show either: it never ran, the place it aims at is
    /// missing, or what came back is not the refusal.
    Inconclusive,
}

/// One judged step, as the record keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepRecord {
    pub step: String,
    pub expected: Expect,
    /// One plain line of what was seen.
    pub observed: String,
    /// `verdict == held`.
    pub ok: bool,
    /// Absent in a record made before the probe's version 2.
    #[serde(default)]
    pub verdict: Option<Verdict>,
    /// `os` for every refusal the fence makes (no step runs Claude Code, so
    /// no permission rule takes part).
    pub layer: String,
}

impl StepRecord {
    pub fn new(step: &str, expected: Expect, verdict: Verdict, observed: String) -> Self {
        Self {
            step: step.into(),
            expected,
            observed,
            ok: verdict == Verdict::Held,
            verdict: Some(verdict),
            layer: "os".into(),
        }
    }

    /// The verdict; for a record made before version 2, read back from `ok`.
    pub fn verdict(&self) -> Verdict {
        self.verdict.unwrap_or(if self.ok {
            Verdict::Held
        } else if matches!(self.expected, Expect::NotLand | Expect::Fail) {
            Verdict::Breached
        } else {
            Verdict::Inconclusive
        })
    }
}

/// Facts the judgement needs beyond each step's own sight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    pub nonce: String,
    /// Every step's first line (see [`Targets::marker`]).
    pub marker: String,
    pub claude_version: String,
    /// The secret the stone set on its own side (and in WSLENV) for step 10.
    pub secret: String,
    /// The names in the real `userdata`, which step 3's output must not carry.
    pub userdata_names: Vec<String>,
}

/// The step's own words: its output without the marker line, and of those
/// the line that says why (Python's last line under a traceback, else the
/// first non-empty one).
fn why(output: &str, marker: &str) -> String {
    let lines: Vec<&str> = output
        .lines()
        .filter(|line| !line.trim().is_empty() && line.trim() != marker)
        .collect();
    let line = if output.contains("Traceback") {
        lines.last()
    } else {
        lines.first()
    };
    line.map(|line| line.trim().chars().take(160).collect())
        .unwrap_or_default()
}

/// Judge one step against the contract's row. Evidence that something
/// crossed is a breach whatever else is missing; a refusal is `held` only
/// when the step began in the fence, finished, aimed at something that
/// exists, and came back with the refusal.
pub fn judge(step: &Step, seen: &Seen, ctx: &Context) -> StepRecord {
    let carries = |text: &str| !text.is_empty() && seen.output.contains(text);
    let began = seen.output.lines().any(|line| line.trim() == ctx.marker);
    let exit = match seen.ran {
        Ran::Exited(code) => Some(code),
        _ => None,
    };
    let said = why(&seen.output, &ctx.marker);
    let landed = seen.landed == Some(true);
    let record =
        |verdict, observed: String| StepRecord::new(step.id, step.expect, verdict, observed);

    // What crossed.
    let crossed: Option<&str> = match step.id {
        "1" | "2" | "5" if landed => Some("LANDED outside the workshop"),
        "6" | "7" if landed => Some("LANDED: a Windows program wrote outside the workshop"),
        "6" | "7" if began && exit == Some(0) => {
            Some("RAN: a Windows program started in the fence")
        }
        "3" if ctx.userdata_names.iter().any(|name| carries(name)) => {
            Some("LISTED the saves' Steam folder")
        }
        "4" | "9" if carries(&ctx.nonce) => Some("READ the stone's canary"),
        "10" if carries(&ctx.secret) => Some("CARRIED the stone's secret"),
        "11" if landed => Some("CHANGED the answers folder"),
        "12" if began && exit == Some(0) => Some("TIOCSTI was allowed"),
        _ => None,
    };
    if let Some(crossed) = crossed {
        return record(Verdict::Breached, crossed.into());
    }

    // Whether the step could show anything at all.
    if let Err(missing) = &seen.ready {
        return record(Verdict::Inconclusive, format!("could not show: {missing}"));
    }
    let ended = match &seen.ran {
        Ran::NotStarted(reason) => Some(format!("did not start: {reason}")),
        Ran::TimedOut => Some("did not finish in 30 s".into()),
        Ran::Killed => Some("ended without an exit status".into()),
        Ran::Exited(_) if !began => Some(format!("never began in the fence: {said}")),
        Ran::Exited(_) => None,
    };
    if let Some(ended) = ended {
        return record(Verdict::Inconclusive, ended);
    }
    let code = exit.unwrap_or(0);
    // The tool's own failure: 1–125. 126 and 127 are the shell saying it could
    // not run the tool at all, which shows nothing about the place it aimed at
    // (the Heretic's cea51c4 follow-up).
    let tool_refused = (1..=125).contains(&code);

    // The refusal itself, row by row.
    let (held, observed) = match step.id {
        "C" => (
            landed,
            if landed {
                "landed in the workshop".into()
            } else {
                format!("did not land: {said}")
            },
        ),
        "1" | "2" | "5" | "11" => match seen.landed {
            None => (false, "the stone could not look".into()),
            Some(_) if code == 0 => (
                false,
                format!("reported a write, yet nothing landed: {said}"),
            ),
            Some(_) if tool_refused => (true, format!("refused: {said}")),
            Some(_) => (false, format!("ended {code}, before the write: {said}")),
        },
        // `sh` answers 127 for a program it cannot find and 126 for one it
        // cannot execute; any other end is not that refusal.
        "6" | "7" => match seen.landed {
            None => (false, "the stone could not look".into()),
            Some(_) if code == 126 || code == 127 => (true, format!("could not run: {said}")),
            Some(_) => (
                false,
                format!("ended {code}, which is not a refusal to run: {said}"),
            ),
        },
        "3" | "4" | "9" if tool_refused => (true, format!("refused: {said}")),
        "3" | "4" | "9" if code == 0 => (false, format!("succeeded, yet showed nothing: {said}")),
        "3" | "4" | "9" => (false, format!("ended {code}, before the read: {said}")),
        "8" if code == 0 && carries(&ctx.claude_version) => (true, format!("answered: {said}")),
        "8" => (
            false,
            format!("did not answer {}: {said}", ctx.claude_version),
        ),
        "10" if code == 0 && seen.output.contains("PATH=") => {
            (true, "the environment carried no secret".into())
        }
        "10" => (false, format!("printed no environment: {said}")),
        "12" if code == 41 => (true, "refused: EPERM".into()),
        "12" if code == 42 => (true, "refused: EIO".into()),
        "12" => (
            false,
            format!("ended {code}, which is not the ioctl's refusal: {said}"),
        ),
        other => (false, format!("an unknown step {other}")),
    };
    record(
        if held {
            Verdict::Held
        } else {
            Verdict::Inconclusive
        },
        observed,
    )
}

/// `pawn-fence.json`: what a probe proved, on which ground.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FenceRecord {
    /// [`PROBE_VERSION`] when written; 0 for a record from before it.
    #[serde(default)]
    pub version: u32,
    pub ground: Ground,
    pub proved_at: String,
    pub control: bool,
    pub steps: Vec<StepRecord>,
    /// The broker checks, `B1` and `B2`.
    #[serde(default)]
    pub broker: Vec<StepRecord>,
}

impl FenceRecord {
    /// Something crossed that the fence should close, in a step or a broker
    /// check. An inconclusive row is not a breach: the record simply does
    /// not hold.
    pub fn breached(&self) -> bool {
        self.steps
            .iter()
            .chain(&self.broker)
            .any(|step| step.verdict() == Verdict::Breached)
    }

    /// Whole and held: this probe's version, the control, exactly the
    /// table's rows in order with their expectations, both broker checks
    /// (each must fail), and every one recorded `held` outright.
    pub fn holds(&self) -> bool {
        let table = self.steps.len() == TABLE.len()
            && self
                .steps
                .iter()
                .zip(TABLE)
                .all(|(step, (id, expect))| step.step == id && step.expected == expect);
        let broker = self.broker.len() == BROKER.len()
            && self
                .broker
                .iter()
                .zip(BROKER)
                .all(|(check, id)| check.step == id && check.expected == Expect::Fail);
        // A version-2 row says its verdict outright: the reading back from
        // `ok` is for an older record's breaches, never for a proof.
        self.version == PROBE_VERSION
            && self.control
            && table
            && broker
            && self
                .steps
                .iter()
                .chain(&self.broker)
                .all(|step| step.verdict == Some(Verdict::Held))
    }

    /// Why a record that did not breach does not hold, in one sentence.
    fn short_of(&self) -> String {
        if self.version != PROBE_VERSION {
            return "the record was made by an earlier probe; run it again".into();
        }
        if !self.control {
            return "the control did not land, so the refusals prove nothing".into();
        }
        let open: Vec<&str> = self
            .steps
            .iter()
            .chain(&self.broker)
            .filter(|step| step.verdict() != Verdict::Held)
            .map(|step| step.step.as_str())
            .collect();
        if open.is_empty() {
            "the record is not whole".into()
        } else {
            format!("the last probe could not show step {}", open.join(", "))
        }
    }
}

/// The chip: `contained` only for a whole, holding record on the current
/// ground; `breached` for a record that let something through; else
/// `unprobed`, with the sentence for why.
pub fn chip(record: Option<&FenceRecord>, current: &Ground) -> (&'static str, Option<String>) {
    let Some(record) = record else {
        return ("unprobed", None);
    };
    if record.breached() {
        return ("breached", None);
    }
    if record.ground != *current {
        let changed = if record.ground.claude_version != current.claude_version {
            format!(
                "proved on claude {}; this is {}",
                record.ground.claude_version, current.claude_version
            )
        } else if record.ground.kernel != current.kernel {
            format!(
                "proved on WSL kernel {}; this is {}",
                record.ground.kernel, current.kernel
            )
        } else if record.ground.distro != current.distro {
            format!(
                "proved in {}; this is {}",
                record.ground.distro, current.distro
            )
        } else {
            "the fence itself changed since it was proved".into()
        };
        return ("unprobed", Some(changed));
    }
    if record.holds() {
        ("contained", None)
    } else {
        ("unprobed", Some(record.short_of()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MARK: &str = "probe-ran-m4rk";

    fn targets() -> Targets {
        Targets {
            nonce: "n0nce".into(),
            marker: MARK.into(),
            mods: "/mnt/c/Other/The Rift/mods".into(),
            game: Some("/mnt/d/Steam/steamapps/common/Dragons Dogma 2".into()),
            userdata: Some("/mnt/c/Program Files (x86)/Steam/userdata".into()),
            canary: "/mnt/c/Users/A/AppData/Local/nl.zmuuzn.riftstone/pawn/probe-canary/probe-canary.txt".into(),
            home: "/home/arisen".into(),
            claude_bin: "/home/arisen/.local/bin/claude".into(),
            mods_windows: r"C:\Other\The Rift\mods".into(),
            cmd_exe: "/mnt/c/Windows/System32/cmd.exe".into(),
        }
    }

    fn ctx() -> Context {
        Context {
            nonce: "n0nce".into(),
            marker: MARK.into(),
            claude_version: "2.1.312".into(),
            secret: "s3cret".into(),
            userdata_names: vec!["12345678".into()],
        }
    }

    fn ground(claude: &str) -> Ground {
        Ground {
            distro: "Ubuntu-24.04".into(),
            claude_version: claude.into(),
            kernel: "6.18".into(),
            digest: "ab".repeat(32),
        }
    }

    fn by(id: &str) -> Step {
        steps(&targets()).into_iter().find(|s| s.id == id).unwrap()
    }

    /// A step that began in the fence and exited `code`.
    fn ran(code: i32, output: &str, landed: Option<bool>) -> Seen {
        Seen {
            ran: Ran::Exited(code),
            output: format!("{MARK}\n{output}"),
            landed,
            ready: Ok(()),
        }
    }

    fn verdict(id: &str, seen: &Seen) -> Verdict {
        judge(&by(id), seen, &ctx()).verdict()
    }

    #[test]
    fn the_table_is_the_contracts_and_no_script_carries_a_double_quote() {
        let all = steps(&targets());
        let ids: Vec<&str> = all.iter().map(|s| s.id).collect();
        assert_eq!(
            ids,
            ["C", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12"]
        );
        for (step, (id, expect)) in all.iter().skip(1).zip(TABLE) {
            assert_eq!((step.id, step.expect), (id, expect));
        }
        for step in &all {
            assert!(!step.script.contains('"'), "{}: {}", step.id, step.script);
            assert!(
                step.script.starts_with(&format!("echo {MARK}; ")),
                "{} begins with the marker",
                step.id
            );
        }
        assert!(all[1]
            .script
            .contains("'/mnt/c/Other/The Rift/mods/probe-n0nce.txt'"));
        assert!(all[6].script.contains(
            "./probe/planted.exe /c 'echo x > C:\\Other\\The Rift\\mods\\probe-pe-n0nce.txt'"
        ));
        assert!(all[12].script.contains("l.ioctl(s, 21522,"));
        assert_eq!(all[0].expect, Expect::Land);
        assert_eq!(all[8].expect, Expect::Succeed);
    }

    #[test]
    fn a_refusal_is_held_only_when_the_stone_saw_it() {
        assert_eq!(verdict("C", &ran(0, "", Some(true))), Verdict::Held);
        assert_eq!(
            verdict(
                "1",
                &ran(2, "cannot create x: Directory nonexistent", Some(false))
            ),
            Verdict::Held
        );
        assert_eq!(
            verdict("3", &ran(2, "ls: cannot access", None)),
            Verdict::Held
        );
        assert_eq!(verdict("4", &ran(1, "No such file", None)), Verdict::Held);
        assert_eq!(
            verdict(
                "6",
                &ran(127, "./probe/planted.exe: not found", Some(false))
            ),
            Verdict::Held
        );
        assert_eq!(
            verdict("8", &ran(0, "2.1.312 (Claude Code)", None)),
            Verdict::Held
        );
        assert_eq!(
            verdict("10", &ran(0, "PATH=/usr/bin\nHOME=/home/a", None)),
            Verdict::Held
        );
        assert_eq!(
            verdict("11", &ran(2, "Read-only file system", Some(false))),
            Verdict::Held
        );
        assert_eq!(verdict("12", &ran(41, "", None)), Verdict::Held);
        assert_eq!(verdict("12", &ran(42, "", None)), Verdict::Held);
        assert_eq!(
            judge(&by("6"), &ran(127, "x", Some(false)), &ctx()).layer,
            "os"
        );
    }

    #[test]
    fn what_crossed_is_a_breach_whatever_else_is_missing() {
        assert_eq!(verdict("1", &ran(0, "", Some(true))), Verdict::Breached);
        let unready = Seen {
            ready: Err("x".into()),
            ..ran(0, "", Some(true))
        };
        assert_eq!(verdict("1", &unready), Verdict::Breached);
        assert_eq!(verdict("3", &ran(0, "12345678\n", None)), Verdict::Breached);
        assert_eq!(verdict("3", &ran(2, "12345678", None)), Verdict::Breached);
        assert_eq!(verdict("4", &ran(0, "n0nce", None)), Verdict::Breached);
        assert_eq!(
            verdict("6", &ran(0, "", Some(false))),
            Verdict::Breached,
            "a Windows program ran"
        );
        assert_eq!(
            verdict("10", &ran(0, "RIFTSTONE_PROBE_SECRET=s3cret", None)),
            Verdict::Breached
        );
        assert_eq!(verdict("12", &ran(0, "", None)), Verdict::Breached);
    }

    /// The Heretic's WP1: a step that never ran, or failed for another
    /// reason, shows no refusal.
    #[test]
    fn an_attempt_that_never_ran_or_never_reached_its_target_is_inconclusive() {
        let not_started = |reason: &str| Seen {
            ran: Ran::NotStarted(reason.into()),
            output: String::new(),
            landed: Some(false),
            ready: Ok(()),
        };
        for id in ["1", "2", "3", "4", "5", "6", "7", "9", "10", "11", "12"] {
            for seen in [
                not_started("the step could not start"),
                not_started("the step's command could not be built"),
                Seen {
                    ran: Ran::TimedOut,
                    ..ran(0, "", Some(false))
                },
                // bwrap failed before the shell: no marker.
                Seen {
                    output: "bwrap: setting up uid map: Permission denied".into(),
                    ..ran(1, "", Some(false))
                },
            ] {
                assert_eq!(verdict(id, &seen), Verdict::Inconclusive, "{id}: {seen:?}");
            }
        }
        // An unrelated Python failure before the ioctl.
        assert_eq!(
            verdict(
                "12",
                &ran(
                    1,
                    "Traceback\nModuleNotFoundError: No module named 'ctypes'",
                    None
                )
            ),
            Verdict::Inconclusive
        );
        // A target the stone knows not to exist, and an observer that failed.
        let missing = Seen {
            ready: Err("the stone knows no game folder here".into()),
            ..ran(1, "", Some(false))
        };
        assert_eq!(verdict("2", &missing), Verdict::Inconclusive);
        assert_eq!(
            verdict("5", &ran(2, "Directory nonexistent", None)),
            Verdict::Inconclusive
        );
        // The shell could not run the tool at all: 127 shows nothing about the
        // place it aimed at (the Heretic's cea51c4 follow-up).
        for id in ["1", "3", "4", "9", "11"] {
            assert_eq!(
                verdict(id, &ran(127, "sh: 1: helper: not found", Some(false))),
                Verdict::Inconclusive,
                "{id}"
            );
        }
        // A write that says it worked but never reached the disk is not a refusal.
        assert_eq!(
            verdict("1", &ran(0, "", Some(false))),
            Verdict::Inconclusive
        );
        // A Windows program that ran and failed for its own reasons is not a refusal to run.
        assert_eq!(
            verdict("7", &ran(1, "The system cannot find the path", Some(false))),
            Verdict::Inconclusive
        );
        // Listing succeeded, yet named nothing it should: not a refusal either.
        assert_eq!(verdict("3", &ran(0, "", None)), Verdict::Inconclusive);
        assert_eq!(
            verdict("8", &ran(0, "2.1.320 (Claude Code)", None)),
            Verdict::Inconclusive
        );
        assert_eq!(
            verdict("C", &ran(2, "Read-only file system", Some(false))),
            Verdict::Inconclusive
        );
    }

    fn row(id: &str, expected: Expect, verdict: Verdict) -> StepRecord {
        StepRecord::new(id, expected, verdict, String::new())
    }

    fn record(claude: &str, verdicts: &[Verdict]) -> FenceRecord {
        FenceRecord {
            version: PROBE_VERSION,
            ground: ground(claude),
            proved_at: "2026-10-28T20:00:00Z".into(),
            control: true,
            steps: TABLE
                .iter()
                .zip(
                    verdicts
                        .iter()
                        .copied()
                        .chain(std::iter::repeat(Verdict::Held)),
                )
                .map(|((id, expect), v)| row(id, *expect, v))
                .collect(),
            broker: BROKER
                .iter()
                .map(|id| row(id, Expect::Fail, Verdict::Held))
                .collect(),
        }
    }

    #[test]
    fn the_chip_claims_contained_only_for_a_whole_holding_record_on_this_ground() {
        let here = ground("2.1.312");
        assert_eq!(chip(None, &here), ("unprobed", None));
        assert_eq!(
            chip(Some(&record("2.1.312", &[])), &here),
            ("contained", None)
        );
        let (chip_text, changed) = chip(Some(&record("2.1.312", &[])), &ground("2.1.320"));
        assert_eq!(chip_text, "unprobed");
        assert_eq!(
            changed.as_deref(),
            Some("proved on claude 2.1.312; this is 2.1.320")
        );
        // Step 1 landed: breached, on any ground.
        assert_eq!(
            chip(Some(&record("2.1.312", &[Verdict::Breached])), &here).0,
            "breached"
        );
        assert_eq!(
            chip(Some(&record("2.1.312", &[Verdict::Breached])), &ground("9")).0,
            "breached"
        );
        // A failed control is not a breach, and not contained either.
        let mut no_control = record("2.1.312", &[]);
        no_control.control = false;
        assert_eq!(chip(Some(&no_control), &here).0, "unprobed");
        // An inconclusive row: unprobed, naming it.
        let open = record("2.1.312", &[Verdict::Held, Verdict::Inconclusive]);
        assert_eq!(
            chip(Some(&open), &here),
            (
                "unprobed",
                Some("the last probe could not show step 2".into())
            )
        );
    }

    /// The Heretic's WP2: a record that is not whole proves nothing.
    #[test]
    fn an_incomplete_or_older_record_earns_nothing() {
        let here = ground("2.1.312");
        let whole = record("2.1.312", &[]);
        let mut empty = whole.clone();
        empty.steps.clear();
        let mut only_eight = whole.clone();
        only_eight.steps.retain(|s| s.step == "8");
        let mut no_broker = whole.clone();
        no_broker.broker.clear();
        let mut doubled = whole.clone();
        doubled.steps[1] = doubled.steps[0].clone();
        let mut relabelled = whole.clone();
        relabelled.steps[0].expected = Expect::Succeed;
        let mut older = whole.clone();
        older.version = 1;
        let mut unsaid = whole.clone();
        for row in unsaid.steps.iter_mut().chain(unsaid.broker.iter_mut()) {
            row.verdict = None;
        }
        let mut broker_expects = whole.clone();
        broker_expects.broker[0].expected = Expect::Succeed;
        for (name, rec) in [
            ("empty", empty),
            ("only step 8", only_eight),
            ("no broker checks", no_broker),
            ("a row twice", doubled),
            ("a row's expectation changed", relabelled),
            ("an older version", older),
            ("version 2 with no verdicts said", unsaid),
            ("a broker check that must succeed", broker_expects),
        ] {
            assert_eq!(chip(Some(&rec), &here).0, "unprobed", "{name}");
            assert!(!rec.holds(), "{name}");
        }
    }

    #[test]
    fn a_record_from_before_the_verdicts_reads_back_and_never_holds() {
        let old = serde_json::json!({
            "ground": ground("2.1.312"),
            "provedAt": "2026-10-28T20:00:00Z",
            "control": true,
            "steps": [{ "step": "1", "expected": "notLand", "observed": "x", "ok": false, "layer": "os" }]
        });
        let rec: FenceRecord = serde_json::from_value(old).unwrap();
        assert_eq!(rec.version, 0);
        assert!(rec.breached(), "a recorded breach stays a breach");
        assert!(!rec.holds());
    }
}
