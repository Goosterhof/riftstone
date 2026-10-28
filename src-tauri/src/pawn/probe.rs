//! The fence's probe (BUILD-SPEC § v0.7, *The probe: deterministic, no
//! model*): small processes in the **identical** fence, each one's result read
//! from outside it. Pure here: the steps' scripts, the judgement of what was
//! observed, the record (`pawn-fence.json`) and the chip it earns. The desk
//! runs the steps and reads the disk.
//!
//! Every step's script avoids the double quote (`wsl.exe` does not read `\"`
//! back), and every path in it is single-quoted.

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

/// The contract's table, C and 1–12, for these targets.
pub fn steps(t: &Targets) -> Vec<Step> {
    let n = &t.nonce;
    let q = |path: &str| sh_quote(path);
    let step = |id, expect, script: String| Step { id, expect, script };
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
        step(
            "12",
            Expect::Fail,
            "python3 -c 'import fcntl, pty, termios; m, s = pty.openpty(); fcntl.ioctl(s, termios.TIOCSTI, bytes([120]))'"
                .into(),
        ),
    ]
}

/// What a step did, as the stone saw it from outside the fence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub exit_ok: bool,
    /// The step's stdout and stderr together.
    pub output: String,
    /// For a write step: did the file appear on the real disk?
    pub landed: Option<bool>,
}

/// One judged step, as the record keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepRecord {
    pub step: String,
    pub expected: Expect,
    /// One plain line of what was seen.
    pub observed: String,
    pub ok: bool,
    /// `os` for every refusal the fence makes (no step runs Claude Code, so
    /// no permission rule takes part).
    pub layer: String,
}

/// Facts the judgement needs beyond each step's own sight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    pub nonce: String,
    pub claude_version: String,
    /// The secret the stone set on its own side (and in WSLENV) for step 10.
    pub secret: String,
    /// The names in the real `userdata`, which step 3's output must not carry.
    pub userdata_names: Vec<String>,
}

/// The line that says why: Python's last line (its error, under the
/// traceback), else the first non-empty one.
fn first_line(output: &str) -> String {
    if output.contains("Traceback") {
        if let Some(last) = output.lines().rev().find(|line| !line.trim().is_empty()) {
            return last.chars().take(160).collect();
        }
    }
    first_line_only(output)
}

fn first_line_only(output: &str) -> String {
    output
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("")
        .chars()
        .take(160)
        .collect()
}

/// Judge one step against the contract's row.
pub fn judge(step: &Step, seen: &Seen, ctx: &Context) -> StepRecord {
    let carries = |text: &str| !text.is_empty() && seen.output.contains(text);
    let (ok, observed) = match step.id {
        "C" => (
            seen.landed == Some(true),
            if seen.landed == Some(true) {
                "landed in the workshop".into()
            } else {
                format!("did not land: {}", first_line(&seen.output))
            },
        ),
        "1" | "2" | "5" | "6" | "7" => {
            let landed = seen.landed == Some(true);
            (
                !landed,
                if landed {
                    "LANDED outside the workshop".into()
                } else {
                    format!("did not land: {}", first_line(&seen.output))
                },
            )
        }
        "3" => {
            let named = ctx.userdata_names.iter().any(|name| carries(name));
            (
                !seen.exit_ok && !named,
                if named {
                    "LISTED the saves' Steam folder".into()
                } else if seen.output.trim().is_empty() {
                    "refused (the stone knows no Steam folder here)".into()
                } else {
                    format!("refused: {}", first_line(&seen.output))
                },
            )
        }
        "4" | "9" => {
            let read = carries(&ctx.nonce);
            (
                !read,
                if read {
                    "READ the stone's canary".into()
                } else {
                    format!("refused: {}", first_line(&seen.output))
                },
            )
        }
        "8" => (
            seen.exit_ok && carries(&ctx.claude_version),
            format!("answered: {}", first_line(&seen.output)),
        ),
        "10" => {
            let leaked = carries(&ctx.secret);
            (
                !leaked,
                if leaked {
                    "CARRIED the stone's secret".into()
                } else {
                    "the environment carried no secret".into()
                },
            )
        }
        "11" => {
            let landed = seen.landed == Some(true);
            (
                !landed,
                if landed {
                    "CHANGED the answers folder".into()
                } else {
                    format!("refused: {}", first_line(&seen.output))
                },
            )
        }
        "12" => (
            !seen.exit_ok,
            if seen.exit_ok {
                "TIOCSTI was allowed".into()
            } else {
                format!("refused: {}", first_line(&seen.output).trim())
            },
        ),
        other => (false, format!("an unknown step {other}")),
    };
    StepRecord {
        step: step.id.into(),
        expected: step.expect,
        observed,
        ok,
        layer: "os".into(),
    }
}

/// `pawn-fence.json`: what a probe proved, on which ground.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FenceRecord {
    pub ground: Ground,
    pub proved_at: String,
    pub control: bool,
    pub steps: Vec<StepRecord>,
}

impl FenceRecord {
    /// A step that must not land, or must fail, did otherwise: the fence let
    /// something through. (A failed control or an unexpected `claude
    /// --version` is not a breach: the record simply does not hold.)
    pub fn breached(&self) -> bool {
        self.steps
            .iter()
            .any(|step| !step.ok && matches!(step.expected, Expect::NotLand | Expect::Fail))
    }

    pub fn holds(&self) -> bool {
        self.control && self.steps.iter().all(|step| step.ok)
    }
}

/// The chip: `contained` only for a record that holds on the current ground;
/// `breached` for a record that let something through; else `unprobed`, with
/// the sentence for a record proved on another ground.
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
        ("unprobed", Some("the last probe did not complete".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn targets() -> Targets {
        Targets {
            nonce: "n0nce".into(),
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

    #[test]
    fn the_table_is_the_contracts_and_no_script_carries_a_double_quote() {
        let all = steps(&targets());
        let ids: Vec<&str> = all.iter().map(|s| s.id).collect();
        assert_eq!(
            ids,
            ["C", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12"]
        );
        for step in &all {
            assert!(!step.script.contains('"'), "{}: {}", step.id, step.script);
        }
        assert!(all[1]
            .script
            .contains("'/mnt/c/Other/The Rift/mods/probe-n0nce.txt'"));
        assert!(all[6].script.starts_with(
            "./probe/planted.exe /c 'echo x > C:\\Other\\The Rift\\mods\\probe-pe-n0nce.txt'"
        ));
        assert_eq!(all[0].expect, Expect::Land);
        assert_eq!(all[8].expect, Expect::Succeed);
    }

    fn seen(exit_ok: bool, output: &str, landed: Option<bool>) -> Seen {
        Seen {
            exit_ok,
            output: output.into(),
            landed,
        }
    }

    #[test]
    fn each_row_judges_what_was_seen_outside_the_fence() {
        let all = steps(&targets());
        let by = |id: &str| all.iter().find(|s| s.id == id).unwrap().clone();
        let c = ctx();
        assert!(judge(&by("C"), &seen(true, "", Some(true)), &c).ok);
        assert!(
            !judge(
                &by("C"),
                &seen(false, "Read-only file system", Some(false)),
                &c
            )
            .ok
        );
        assert!(
            judge(
                &by("1"),
                &seen(false, "No such file or directory", Some(false)),
                &c
            )
            .ok
        );
        assert!(!judge(&by("1"), &seen(true, "", Some(true)), &c).ok);
        assert!(
            judge(
                &by("3"),
                &seen(false, "No such file or directory", None),
                &c
            )
            .ok
        );
        assert!(
            !judge(&by("3"), &seen(true, "12345678\n", None), &c).ok,
            "listed"
        );
        assert!(
            !judge(&by("3"), &seen(false, "12345678", None), &c).ok,
            "named an entry"
        );
        assert!(judge(&by("4"), &seen(false, "No such file", None), &c).ok);
        assert!(!judge(&by("4"), &seen(true, "n0nce", None), &c).ok);
        assert!(judge(&by("8"), &seen(true, "2.1.312 (Claude Code)", None), &c).ok);
        assert!(!judge(&by("8"), &seen(true, "2.1.320 (Claude Code)", None), &c).ok);
        assert!(
            judge(
                &by("10"),
                &seen(true, "PATH=/usr/bin\nHOME=/home/a", None),
                &c
            )
            .ok
        );
        assert!(
            !judge(
                &by("10"),
                &seen(true, "RIFTSTONE_PROBE_SECRET=s3cret", None),
                &c
            )
            .ok
        );
        assert!(
            judge(
                &by("11"),
                &seen(false, "Read-only file system", Some(false)),
                &c
            )
            .ok
        );
        assert!(
            judge(
                &by("12"),
                &seen(false, "PermissionError: [Errno 1]", None),
                &c
            )
            .ok
        );
        assert!(!judge(&by("12"), &seen(true, "", None), &c).ok);
        assert!(judge(&by("6"), &seen(false, "cannot execute", Some(false)), &c).ok);
        assert_eq!(
            judge(&by("6"), &seen(false, "x", Some(false)), &c).layer,
            "os"
        );
    }

    fn record(claude: &str, oks: &[bool]) -> FenceRecord {
        let all = steps(&targets());
        FenceRecord {
            ground: ground(claude),
            proved_at: "2026-10-28T20:00:00Z".into(),
            control: true,
            steps: all
                .iter()
                .skip(1)
                .zip(oks.iter().copied().chain(std::iter::repeat(true)))
                .map(|(step, ok)| StepRecord {
                    step: step.id.into(),
                    expected: step.expect,
                    observed: String::new(),
                    ok,
                    layer: "os".into(),
                })
                .collect(),
        }
    }

    #[test]
    fn the_chip_claims_contained_only_for_a_holding_record_on_this_ground() {
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
            chip(Some(&record("2.1.312", &[false])), &here).0,
            "breached"
        );
        assert_eq!(
            chip(Some(&record("2.1.312", &[false])), &ground("9")).0,
            "breached"
        );
        // A failed control is not a breach, and not contained either.
        let mut no_control = record("2.1.312", &[]);
        no_control.control = false;
        assert_eq!(chip(Some(&no_control), &here).0, "unprobed");
    }
}
