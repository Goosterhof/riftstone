//! What the player sets for Tamsin's Claude Code on THE BENCH (0.7.2): the
//! permission mode, the effort and the model, kept in `pawn.json` and passed
//! as flags at each summon. Pure.
//!
//! The stone launches her `--restricted`, and restricted Claude Code reads no
//! user settings file, so a `/effort` or `/model` she is given in the
//! terminal lasts that summoning only. These three are the stone's to carry
//! (the investor's report, 2026-10-31: *"it goes back to default always"*).
//!
//! None of them is part of the fence's ground (`fence::ground` names the
//! default session): they set what Claude Code asks and which model answers,
//! inside the fence, and never a mount, the environment or the filter.

use serde::{Deserialize, Serialize};

/// Who approves her actions. Bypass and `dontAsk` are not offered: RS-9.4
/// keeps `disableBypassPermissionsMode` in her settings, and `--restricted`
/// refuses bypass on its own too.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PawnMode {
    /// Every edit and command waits for the player (`default`).
    #[default]
    Ask,
    /// Edits in the workshop go through; commands still ask.
    AcceptEdits,
    /// She plans and changes nothing until the player approves the plan.
    Plan,
    /// Claude Code's own classifier approves what it judges safe.
    Auto,
}

impl PawnMode {
    /// The `--permission-mode` word. `default` is the one every probe
    /// record before 0.7.2 was proved with; Claude Code 2.1.316 still takes it.
    pub fn flag(self) -> &'static str {
        match self {
            PawnMode::Ask => "default",
            PawnMode::AcceptEdits => "acceptEdits",
            PawnMode::Plan => "plan",
            PawnMode::Auto => "auto",
        }
    }
}

/// `--effort`. `Default` passes no flag, so Claude Code picks its own.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PawnEffort {
    #[default]
    Default,
    Low,
    Medium,
    High,
    Xhigh,
    /// Claude Code's `max`.
    #[serde(rename = "max")]
    Maximum,
}

impl PawnEffort {
    pub fn flag(self) -> Option<&'static str> {
        match self {
            PawnEffort::Default => None,
            PawnEffort::Low => Some("low"),
            PawnEffort::Medium => Some("medium"),
            PawnEffort::High => Some("high"),
            PawnEffort::Xhigh => Some("xhigh"),
            PawnEffort::Maximum => Some("max"),
        }
    }
}

/// `--model`, by Claude Code's own aliases (each names the newest model of
/// its family). `Default` passes no flag.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PawnModel {
    #[default]
    Default,
    Fable,
    Opus,
    Sonnet,
    Haiku,
}

impl PawnModel {
    pub fn flag(self) -> Option<&'static str> {
        match self {
            PawnModel::Default => None,
            PawnModel::Fable => Some("fable"),
            PawnModel::Opus => Some("opus"),
            PawnModel::Sonnet => Some("sonnet"),
            PawnModel::Haiku => Some("haiku"),
        }
    }
}

/// The three, as `pawn.json` keeps them and `set_pawn_session` sets them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PawnSession {
    pub mode: PawnMode,
    pub effort: PawnEffort,
    pub model: PawnModel,
}

impl PawnSession {
    /// The flags, in this order: `--permission-mode` always, then `--effort`
    /// and `--model` when one is chosen. The default session's flags are
    /// exactly the argv tail every summon passed before 0.7.2.
    pub fn args(&self) -> Vec<String> {
        let mut args = vec![
            "--permission-mode".to_string(),
            self.mode.flag().to_string(),
        ];
        if let Some(effort) = self.effort.flag() {
            args.extend(["--effort".to_string(), effort.to_string()]);
        }
        if let Some(model) = self.model.flag() {
            args.extend(["--model".to_string(), model.to_string()]);
        }
        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_session_is_the_argv_every_earlier_summon_passed() {
        assert_eq!(
            PawnSession::default().args(),
            ["--permission-mode", "default"]
        );
    }

    #[test]
    fn a_chosen_effort_and_model_follow_the_mode() {
        let session = PawnSession {
            mode: PawnMode::AcceptEdits,
            effort: PawnEffort::Xhigh,
            model: PawnModel::Opus,
        };
        assert_eq!(
            session.args(),
            [
                "--permission-mode",
                "acceptEdits",
                "--effort",
                "xhigh",
                "--model",
                "opus"
            ]
        );
    }

    #[test]
    fn every_mode_names_a_word_claude_code_takes_and_none_skips_approval_whole() {
        // Claude Code 2.1.316's choices, plus `default`, which it still takes.
        let taken = ["default", "acceptEdits", "plan", "auto"];
        for mode in [
            PawnMode::Ask,
            PawnMode::AcceptEdits,
            PawnMode::Plan,
            PawnMode::Auto,
        ] {
            assert!(taken.contains(&mode.flag()), "{mode:?}");
        }
    }

    #[test]
    fn every_effort_is_one_claude_code_lists() {
        let listed = ["low", "medium", "high", "xhigh", "max"];
        for effort in [
            PawnEffort::Low,
            PawnEffort::Medium,
            PawnEffort::High,
            PawnEffort::Xhigh,
            PawnEffort::Maximum,
        ] {
            assert!(listed.contains(&effort.flag().unwrap()), "{effort:?}");
        }
    }

    #[test]
    fn the_window_words_read_back_and_a_bypass_word_is_refused() {
        let session: PawnSession = serde_json::from_str(
            r#"{ "mode": "acceptEdits", "effort": "xhigh", "model": "fable" }"#,
        )
        .unwrap();
        assert_eq!(session.mode, PawnMode::AcceptEdits);
        assert_eq!(session.effort, PawnEffort::Xhigh);
        assert_eq!(session.model, PawnModel::Fable);
        for word in ["bypassPermissions", "dontAsk", "default"] {
            let json = format!(r#"{{ "mode": "{word}" }}"#);
            assert!(
                serde_json::from_str::<PawnSession>(&json).is_err(),
                "{word}"
            );
        }
    }

    #[test]
    fn a_missing_field_reads_as_its_default() {
        let session: PawnSession = serde_json::from_str(r#"{ "effort": "max" }"#).unwrap();
        assert_eq!(
            session,
            PawnSession {
                effort: PawnEffort::Maximum,
                ..PawnSession::default()
            }
        );
    }
}
