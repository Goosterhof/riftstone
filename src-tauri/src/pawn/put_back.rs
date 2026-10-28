//! Put back, narrowed (BUILD-SPEC § v0.7, the run ledger; the Heretic's
//! Assay, WC5): what a press can honestly undo of one summoning.
//!
//! - **remove:** the installs whose `actor` is that run's, still in the
//!   journal (a player's reinstall would have made them the player's). Each
//!   is uninstalled newest first through the journal, after the investor
//!   reviews it.
//! - **review:** what that run uninstalled that was not its own install. It
//!   is shown, never acted on in bulk, each with its Undo where that still
//!   applies, else *Install again from the Rift*.
//!
//! It never promises the world as it was. An install's id is its option id,
//! as the journal keys it.

use std::collections::BTreeSet;

use rift_deploy::{Actor, Journal};
use serde::Serialize;

use crate::pawn::mailbox::{run_installs, Outcome, RunLedger};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveItem {
    pub install_id: String,
    pub name: String,
    pub seq: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Undo {
    Available,
    ReinstallFromRift,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewItem {
    pub install_id: String,
    pub name: String,
    pub at: String,
    pub undo: Undo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PutBack {
    pub run_id: String,
    pub remove: Vec<RemoveItem>,
    pub review: Vec<ReviewItem>,
    /// The press's own promise.
    pub sentence: String,
}

fn installs_of(n: usize) -> String {
    match n {
        1 => "the 1 install".to_string(),
        n => format!("the {n} installs"),
    }
}

/// The press's sentence: what it removes, and what it leaves for review.
pub fn sentence(remove: usize, review: usize) -> String {
    let first = if remove == 0 {
        "Tamsin left no install of her own from this summoning in your world.".to_string()
    } else {
        format!(
            "Removes {} Tamsin made in this summoning.",
            installs_of(remove)
        )
    };
    match review {
        0 => first,
        1 => format!("{first} She also uninstalled 1 of yours; it is listed below."),
        n => format!("{first} She also uninstalled {n} of yours; they are listed below."),
    }
}

/// Plan Put back for `run_id`. `undo_available` says whether the journal's
/// own Undo still applies to an uninstalled option (its package is still in
/// the library).
pub fn plan_put_back(
    run_id: &str,
    ledger: &RunLedger,
    journal: &Journal,
    undo_available: &dyn Fn(&str) -> bool,
) -> PutBack {
    let actor = Actor::Pawn {
        run_id: run_id.to_string(),
    };
    let mut remove: Vec<RemoveItem> = journal
        .installs
        .iter()
        .filter(|install| install.actor == actor)
        .map(|install| RemoveItem {
            install_id: install.option_id.0.clone(),
            name: install.name.clone(),
            seq: install.seq,
        })
        .collect();
    remove.sort_by_key(|item| std::cmp::Reverse(item.seq));

    let own: BTreeSet<String> = run_installs(ledger).into_keys().collect();
    let review = ledger
        .ops
        .iter()
        .filter(|op| op.tool == "uninstall" && op.outcome == Outcome::Done)
        .filter_map(|op| {
            let install_id = op.install_id.clone()?;
            if own.contains(&install_id) {
                return None;
            }
            let name = op
                .answer
                .data
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(&install_id)
                .to_string();
            Some(ReviewItem {
                undo: if undo_available(&install_id) {
                    Undo::Available
                } else {
                    Undo::ReinstallFromRift
                },
                install_id,
                name,
                at: op.at.clone(),
            })
        })
        .collect::<Vec<_>>();
    PutBack {
        run_id: run_id.to_string(),
        sentence: sentence(remove.len(), review.len()),
        remove,
        review,
    }
}

/// The ids a `put_back` press may act on, newest first: every one must be the
/// run's own still-installed install, or the whole press is refused
/// (`notThisRun`) naming the first that is not.
pub fn checked_put_back(
    run_id: &str,
    journal: &Journal,
    install_ids: &[String],
) -> Result<Vec<String>, String> {
    let actor = Actor::Pawn {
        run_id: run_id.to_string(),
    };
    let mut chosen = Vec::new();
    for id in install_ids {
        let install = journal
            .installs
            .iter()
            .find(|install| install.option_id.0 == *id && install.actor == actor)
            .ok_or_else(|| id.clone())?;
        if !chosen.iter().any(|(seq, _)| *seq == install.seq) {
            chosen.push((install.seq, id.clone()));
        }
    }
    chosen.sort_by_key(|(seq, _)| std::cmp::Reverse(*seq));
    Ok(chosen.into_iter().map(|(_, id)| id).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pawn::mailbox::{Answer, LedgerOp};
    use rift_core::OptionId;
    use rift_deploy::{InstalledOption, Origin};
    use std::path::{Path, PathBuf};

    const RUN: &str = "0b5e2c6e-4a8d-4d3e-9f4b-2a1c7d9e8f00";
    const OTHER_RUN: &str = "11111111-2222-4333-8444-555555555555";

    fn install(seq: u64, id: &str, name: &str, actor: Actor) -> InstalledOption {
        InstalledOption {
            seq,
            option_id: OptionId(id.into()),
            name: name.into(),
            package_file: format!("{name}.zip"),
            package_path: PathBuf::from(format!("/mods/{name}.zip")),
            package_size: 1,
            option_root: String::new(),
            version: None,
            installed_at: "2026-10-28T12:00:00Z".into(),
            game_build: None,
            origin: Origin::Riftstone,
            files: Vec::new(),
            actor,
        }
    }

    fn pawn(run: &str) -> Actor {
        Actor::Pawn { run_id: run.into() }
    }

    fn op(tool: &str, install_id: &str, outcome: Outcome, name: &str) -> LedgerOp {
        LedgerOp {
            id: format!("{tool}-{install_id}"),
            tool: tool.into(),
            install_id: Some(install_id.into()),
            option_id: Some(install_id.into()),
            outcome,
            at: "2026-10-28T12:05:00Z".into(),
            answer: Answer {
                id: format!("{tool}-{install_id}"),
                outcome,
                text: String::new(),
                data: serde_json::json!({ "installId": install_id, "name": name }),
            },
        }
    }

    fn ledger(ops: Vec<LedgerOp>) -> RunLedger {
        RunLedger {
            run_id: RUN.into(),
            started_at: "2026-10-28T12:00:00Z".into(),
            ended_at: None,
            ops,
        }
    }

    fn journal(installs: Vec<InstalledOption>) -> Journal {
        let mut journal = Journal::new(Path::new("/g"));
        journal.installs = installs;
        journal
    }

    #[test]
    fn a_run_that_only_uninstalled_a_players_install_removes_nothing_and_lists_it() {
        // The Assay's counterexample (WC5).
        let journal = journal(vec![install(1, "aa#Glow", "Lantern Glow", Actor::Player)]);
        let mut world = journal.clone();
        world.installs.clear(); // Tamsin uninstalled it.
        let ledger = ledger(vec![op(
            "uninstall",
            "aa#Glow",
            Outcome::Done,
            "Lantern Glow",
        )]);
        let plan = plan_put_back(RUN, &ledger, &world, &|_| true);
        assert!(plan.remove.is_empty());
        assert_eq!(
            plan.review,
            [ReviewItem {
                install_id: "aa#Glow".into(),
                name: "Lantern Glow".into(),
                at: "2026-10-28T12:05:00Z".into(),
                undo: Undo::Available,
            }]
        );
        assert_eq!(
            plan.sentence,
            "Tamsin left no install of her own from this summoning in your world. She also uninstalled 1 of yours; it is listed below."
        );
    }

    #[test]
    fn remove_is_the_runs_own_installs_still_there_newest_first() {
        let world = journal(vec![
            install(1, "aa#Glow", "Lantern Glow", Actor::Player),
            install(2, "ab#Stature", "Stature", pawn(RUN)),
            install(3, "ac#Other", "Other Run's", pawn(OTHER_RUN)),
            install(5, "ad#Fray", "Fray Eye", pawn(RUN)),
            install(
                4,
                "ae#Reinstalled",
                "Reinstalled by the player",
                Actor::Player,
            ),
        ]);
        let ledger = ledger(vec![
            op("install", "ab#Stature", Outcome::Done, "Stature"),
            op("install", "ad#Fray", Outcome::Done, "Fray Eye"),
            // Tamsin installed this, the player reinstalled it: it is theirs now.
            op("install", "ae#Reinstalled", Outcome::Done, "Reinstalled"),
            // Tamsin installed and removed this one herself: nothing to review.
            op("install", "af#Gone", Outcome::Done, "Gone"),
            op("uninstall", "af#Gone", Outcome::Done, "Gone"),
            // A refused uninstall changed nothing.
            op("uninstall", "aa#Glow", Outcome::Refused, "Lantern Glow"),
        ]);
        let plan = plan_put_back(RUN, &ledger, &world, &|_| false);
        let ids: Vec<&str> = plan.remove.iter().map(|r| r.install_id.as_str()).collect();
        assert_eq!(ids, ["ad#Fray", "ab#Stature"]);
        assert!(plan.review.is_empty());
        assert_eq!(
            plan.sentence,
            "Removes the 2 installs Tamsin made in this summoning."
        );
    }

    #[test]
    fn a_gone_package_offers_a_reinstall_from_the_rift() {
        let ledger = ledger(vec![
            op("uninstall", "aa#Glow", Outcome::Done, "Lantern Glow"),
            op("uninstall", "ag#Hair", Outcome::Done, "Hair"),
        ]);
        let plan = plan_put_back(
            RUN,
            &ledger,
            &journal(vec![install(9, "ab#S", "S", pawn(RUN))]),
            &|id| id == "aa#Glow",
        );
        assert_eq!(plan.review[0].undo, Undo::Available);
        assert_eq!(plan.review[1].undo, Undo::ReinstallFromRift);
        assert_eq!(
            plan.sentence,
            "Removes the 1 install Tamsin made in this summoning. She also uninstalled 2 of yours; they are listed below."
        );
    }

    #[test]
    fn a_press_naming_anything_but_the_runs_own_installs_is_refused_whole() {
        let world = journal(vec![
            install(1, "aa#Glow", "Lantern Glow", Actor::Player),
            install(2, "ab#Stature", "Stature", pawn(RUN)),
            install(3, "ac#Other", "Other", pawn(OTHER_RUN)),
            install(4, "ad#Fray", "Fray", pawn(RUN)),
        ]);
        let ids = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            checked_put_back(RUN, &world, &ids(&["ab#Stature", "ad#Fray"])).unwrap(),
            ["ad#Fray", "ab#Stature"]
        );
        assert_eq!(
            checked_put_back(RUN, &world, &ids(&["ab#Stature", "aa#Glow"])),
            Err("aa#Glow".into())
        );
        assert_eq!(
            checked_put_back(RUN, &world, &ids(&["ac#Other"])),
            Err("ac#Other".into())
        );
        assert_eq!(
            checked_put_back(RUN, &world, &ids(&["zz#Missing"])),
            Err("zz#Missing".into())
        );
        assert_eq!(
            checked_put_back(RUN, &world, &ids(&["ab#Stature", "ab#Stature"])).unwrap(),
            ["ab#Stature"],
            "a repeated id is acted on once"
        );
    }
}
