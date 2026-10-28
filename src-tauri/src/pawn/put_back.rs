//! Put back, narrowed (BUILD-SPEC § v0.7, the run ledger; the Heretic's
//! Assay, WC5): what a press can honestly undo of one summoning.
//!
//! - **remove:** the installs whose `actor` is that run's, still in the
//!   journal (a player's reinstall would have made them the player's). Each
//!   is uninstalled newest first through the journal, after the investor
//!   reviews it.
//! - **review:** what that run uninstalled that was not its own install, by
//!   whose install the journal said it was at the moment it was removed
//!   ([`crate::pawn::mailbox::Removed`]; an option id can be installed again
//!   by someone else, so it is never the test: the Heretic's WB4). An
//!   uninstall whose outcome is `unknown`, or that recorded no owner, is
//!   listed too. Shown, never acted on in bulk, each with its Undo where that
//!   still applies, else *Install again from the Rift*.
//!
//! It never promises the world as it was. An install's id is its option id,
//! as the journal keys it.

use rift_deploy::{Actor, Journal};
use serde::Serialize;

use crate::pawn::mailbox::{Outcome, RunLedger};

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
    /// What the ledger can say about it, and no more (the Heretic's WB5).
    pub known: Known,
}

/// How much the ledger knows about one uninstall on the review list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Known {
    /// Done, and the install it removed was the player's.
    Yours,
    /// Done, and the install it removed was placed in another summoning.
    AnotherSummoning,
    /// The outcome is unknown, or whose install it was was not recorded: it
    /// may or may not have been removed, and may or may not have been yours.
    Uncertain,
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
pub fn sentence(remove: usize, review: &[ReviewItem]) -> String {
    let first = if remove == 0 {
        "Tamsin left no install of her own from this summoning in your world.".to_string()
    } else {
        format!(
            "Removes {} Tamsin made in this summoning.",
            installs_of(remove)
        )
    };
    let count = |known: Known| review.iter().filter(|item| item.known == known).count();
    let (yours, another, uncertain) = (
        count(Known::Yours),
        count(Known::AnotherSummoning),
        count(Known::Uncertain),
    );
    let mut done = Vec::new();
    if yours > 0 {
        done.push(format!("{yours} of yours"));
    }
    if another > 0 {
        done.push(format!("{another} placed in another summoning"));
    }
    let mut parts = vec![first];
    if !done.is_empty() {
        parts.push(format!("She also uninstalled {}.", done.join(" and ")));
    }
    match uncertain {
        0 => {}
        1 => parts
            .push("The stone cannot tell whether 1 more was uninstalled, or whose it was.".into()),
        n => parts.push(format!(
            "The stone cannot tell whether {n} more were uninstalled, or whose they were."
        )),
    }
    if !review.is_empty() {
        parts.push(if review.len() == 1 {
            "It is listed below.".into()
        } else {
            "They are listed below.".into()
        });
    }
    parts.join(" ")
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

    let review = ledger
        .ops
        .iter()
        .filter(|op| {
            op.tool == "uninstall" && matches!(op.outcome, Outcome::Done | Outcome::Unknown)
        })
        .filter_map(|op| {
            let install_id = op.install_id.clone()?;
            // Her own install, removed by her: nothing of the player's to
            // review. Anything else, or an owner not recorded, is listed.
            if op.outcome == Outcome::Done
                && op
                    .removed
                    .as_ref()
                    .is_some_and(|removed| removed.actor == actor)
            {
                return None;
            }
            let name = op
                .answer
                .data
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(&install_id)
                .to_string();
            let known = match (&op.outcome, &op.removed) {
                (Outcome::Done, Some(removed)) if removed.actor == Actor::Player => Known::Yours,
                (Outcome::Done, Some(_)) => Known::AnotherSummoning,
                _ => Known::Uncertain,
            };
            Some(ReviewItem {
                known,
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
        sentence: sentence(remove.len(), &review),
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
    use crate::pawn::mailbox::{Answer, LedgerOp, Removed};
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
        removing(tool, install_id, outcome, name, None)
    }

    /// An uninstall that recorded whose install it removed.
    fn removing(
        tool: &str,
        install_id: &str,
        outcome: Outcome,
        name: &str,
        removed: Option<Actor>,
    ) -> LedgerOp {
        LedgerOp {
            removed: removed.map(|actor| Removed { actor, seq: 1 }),
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
            held: None,
            decisions: Vec::new(),
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
        let ledger = ledger(vec![removing(
            "uninstall",
            "aa#Glow",
            Outcome::Done,
            "Lantern Glow",
            Some(Actor::Player),
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
                known: Known::Yours,
            }]
        );
        assert_eq!(
            plan.sentence,
            "Tamsin left no install of her own from this summoning in your world. She also uninstalled 1 of yours. It is listed below."
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
            removing(
                "uninstall",
                "af#Gone",
                Outcome::Done,
                "Gone",
                Some(pawn(RUN)),
            ),
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
            removing(
                "uninstall",
                "aa#Glow",
                Outcome::Done,
                "Lantern Glow",
                Some(Actor::Player),
            ),
            removing(
                "uninstall",
                "ag#Hair",
                Outcome::Done,
                "Hair",
                Some(Actor::Player),
            ),
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
            "Removes the 1 install Tamsin made in this summoning. She also uninstalled 2 of yours. They are listed below."
        );
    }

    #[test]
    fn an_option_id_used_again_by_the_player_is_still_the_players() {
        // The Heretic's WB4 timeline: Tamsin installs A, removes her own A,
        // the player installs A again, Tamsin removes the player's A.
        let ledger = ledger(vec![
            op("install", "aa#A", Outcome::Done, "A"),
            removing("uninstall", "aa#A", Outcome::Done, "A", Some(pawn(RUN))),
            removing("uninstall", "aa#A", Outcome::Done, "A", Some(Actor::Player)),
        ]);
        let plan = plan_put_back(RUN, &ledger, &journal(vec![]), &|_| true);
        assert!(plan.remove.is_empty());
        assert_eq!(plan.review.len(), 1, "the player's A is listed");
        assert_eq!(plan.review[0].install_id, "aa#A");
    }

    #[test]
    fn an_uninstall_of_unknown_outcome_or_owner_is_listed_for_review() {
        let ledger = ledger(vec![
            removing("uninstall", "aa#Glow", Outcome::Unknown, "Glow", None),
            removing("uninstall", "ab#Hair", Outcome::Done, "Hair", None),
            removing(
                "uninstall",
                "ac#Own",
                Outcome::Unknown,
                "Own",
                Some(pawn(RUN)),
            ),
            removing(
                "uninstall",
                "ad#Other",
                Outcome::Done,
                "Other",
                Some(pawn(OTHER_RUN)),
            ),
        ]);
        let plan = plan_put_back(RUN, &ledger, &journal(vec![]), &|_| true);
        let ids: Vec<&str> = plan.review.iter().map(|r| r.install_id.as_str()).collect();
        assert_eq!(ids, ["aa#Glow", "ab#Hair", "ac#Own", "ad#Other"]);
        let known: Vec<Known> = plan.review.iter().map(|r| r.known).collect();
        assert_eq!(
            known,
            [
                Known::Uncertain,
                Known::Uncertain,
                Known::Uncertain,
                Known::AnotherSummoning
            ]
        );
        assert_eq!(
            plan.sentence,
            "Tamsin left no install of her own from this summoning in your world. She also uninstalled 1 placed in another summoning. The stone cannot tell whether 3 more were uninstalled, or whose they were. They are listed below."
        );
    }

    #[test]
    fn an_uncertain_uninstall_of_her_own_is_never_called_yours() {
        // The Heretic's WB5: an unknown outcome that recorded this run's own
        // install must not read as a completed uninstall of the player's.
        let ledger = ledger(vec![removing(
            "uninstall",
            "ac#Own",
            Outcome::Unknown,
            "Own",
            Some(pawn(RUN)),
        )]);
        let plan = plan_put_back(RUN, &ledger, &journal(vec![]), &|_| true);
        assert_eq!(plan.review.len(), 1);
        assert!(!plan.sentence.contains("of yours"), "{}", plan.sentence);
        assert!(
            !plan.sentence.contains("She also uninstalled"),
            "{}",
            plan.sentence
        );
        assert_eq!(
            plan.sentence,
            "Tamsin left no install of her own from this summoning in your world. The stone cannot tell whether 1 more was uninstalled, or whose it was. It is listed below."
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
