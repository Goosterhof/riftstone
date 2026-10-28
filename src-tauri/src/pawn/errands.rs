//! The stone's own operations behind Tamsin's tools (BUILD-SPEC § v0.7, *The
//! tools*): [`StoneErrands`] answers the [`Broker`](crate::pawn::mailbox::Broker)
//! with the very functions the window's presses call, so a forged request
//! meets RS-3's refusals word for word.
//!
//! `install` checks the plan its token named **inside the turn that commits**
//! ([`deploy::install_checked`]): the same deploy lock the window's own
//! install takes, so nothing changes between the check and the first byte
//! (the Heretic's WB1). `uninstall` names the install it removed as the
//! journal held it in that turn (WB4).
//!
//! [`RunsFile`] keeps `pawn-runs.json`: the last 20 runs' ledgers, each
//! written before a mutation starts and again before its answer (WB2).

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rift_core::{OptionId, RiftError};
use rift_deploy::Actor;
use serde_json::Value;

use crate::commands::{deploy, library};
use crate::dto::{SearchFilters, SearchQuery};
use crate::family::Families;
use crate::pawn::mailbox::{
    merge_ledger, Done, Errands, LedgerSink, PlannedInstall, Removed, RunLedger, STALE_PLAN,
};
use crate::persist;
use crate::state::{lock, AppState};

/// How much of the end of the REFramework log is ever read.
const LOG_TAIL_BYTES: u64 = 4 * 1024 * 1024;

/// The sentence a refusal carries to Tamsin: the stone's own reason, verbatim.
fn sentence(err: RiftError) -> String {
    match err {
        RiftError::Refused { reason } => reason,
        other => other.to_string(),
    }
}

fn value<T: serde::Serialize>(view: &T) -> Result<Value, String> {
    serde_json::to_value(view)
        .map_err(|err| format!("The stone could not write its answer: {err}."))
}

/// The real stone, as Tamsin's tools reach it.
pub struct StoneErrands {
    state: Arc<AppState>,
}

impl StoneErrands {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    fn name_of(&self, option_id: &str) -> String {
        self.state
            .shelf()
            .option(&OptionId(option_id.to_string()))
            .map_or_else(|| option_id.to_string(), |(_, option)| option.name.clone())
    }
}

impl Errands for StoneErrands {
    fn search_library(&self, query: &str, limit: u64) -> Result<Value, String> {
        let hits = library::search_impl(
            &self.state,
            &SearchQuery {
                text: query.to_string(),
                filters: SearchFilters::default(),
                limit: limit as usize,
            },
        );
        value(&hits)
    }

    fn get_option(&self, option_id: &str) -> Result<Value, String> {
        value(&library::get_option_impl(&self.state, option_id).map_err(sentence)?)
    }

    fn list_installed(&self) -> Result<Value, String> {
        value(&deploy::get_installed_impl(&self.state).map_err(sentence)?)
    }

    fn plan_install(&self, option_id: &str) -> Result<PlannedInstall, String> {
        // The generation is read before the shelf, so a rescan in between
        // makes the token stale rather than the plan silently newer.
        let library_generation = self.state.library_generation();
        let shelf = self.state.shelf();
        let (package, option) = shelf
            .option(&OptionId(option_id.to_string()))
            .ok_or_else(|| format!("There is no option {option_id} in the library."))?;
        let families = Families::of_shelf(&shelf, &lock(&self.state.nexus));
        let (view, journal_seq) = self
            .state
            .plan(|ctx, journal| {
                (
                    deploy::plan_view(&families, &shelf, ctx, journal, package, option),
                    journal.next_seq,
                )
            })
            .map_err(sentence)?;
        Ok(PlannedInstall {
            plan: value(&view)?,
            library_generation,
            journal_seq,
        })
    }

    fn install(
        &self,
        option_id: &str,
        actor: &Actor,
        expected: &PlannedInstall,
    ) -> Result<Done, String> {
        let state = &self.state;
        let check = |view: &crate::dto::InstallPlanView, journal: &rift_deploy::Journal| {
            let same = serde_json::to_value(view).ok().as_ref() == Some(&expected.plan)
                && journal.next_seq == expected.journal_seq
                && state.library_generation() == expected.library_generation;
            if same {
                Ok(())
            } else {
                Err(RiftError::refused(STALE_PLAN))
            }
        };
        let result =
            deploy::install_checked(state, option_id, actor, Some(&check)).map_err(sentence)?;
        Ok(Done {
            install_id: Some(result.option_id.clone()),
            option_id: Some(result.option_id.clone()),
            summary: format!(
                "installed {} · {} files",
                self.name_of(option_id),
                result.files_written
            ),
            data: value(&result)?,
            removed: None,
        })
    }

    fn uninstall(&self, install_id: &str, _actor: &Actor) -> Result<Done, String> {
        let name = self.name_of(install_id);
        let (result, removed) =
            deploy::uninstall_recorded(&self.state, install_id).map_err(sentence)?;
        let mut data = value(&result)?;
        if let Value::Object(map) = &mut data {
            map.insert("name".into(), Value::String(name.clone()));
        }
        Ok(Done {
            install_id: Some(install_id.to_string()),
            option_id: Some(result.option_id.clone()),
            summary: format!("uninstalled {name} · {} files", result.files_removed),
            data,
            removed: removed.map(|(actor, seq)| Removed { actor, seq }),
        })
    }

    fn hold_play(&self, _run_id: &str) -> Result<(), String> {
        // The held Play's desk (BUILD-SPEC § v0.7, The held Play's decision)
        // is the next slice; until it stands, nothing is held.
        Err("The stone cannot hold a Play for Tamsin yet; ask the Arisen to press Play.".into())
    }

    fn drop_held_play(&self, _run_id: &str) {}

    fn read_reframework_log(&self, tail_lines: u64) -> Result<Value, String> {
        let game = self.state.world().game.ok_or_else(|| {
            "Dragon's Dogma 2 is not installed where the stone looked.".to_string()
        })?;
        let path = game.root.join(rift_deploy::reframework::LOG_FILE);
        let lines = tail(&path, tail_lines as usize).map_err(|_| {
            "There is no REFramework log yet: start the game once with REFramework installed."
                .to_string()
        })?;
        Ok(serde_json::json!({ "count": lines.len(), "lines": lines }))
    }
}

/// The last `count` lines of a file, read only from its last 4 MiB, never
/// written.
fn tail(path: &std::path::Path, count: usize) -> std::io::Result<Vec<String>> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let start = len.saturating_sub(LOG_TAIL_BYTES);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.take(LOG_TAIL_BYTES).read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes);
    let mut lines: Vec<&str> = text.lines().collect();
    if start > 0 && !lines.is_empty() {
        lines.remove(0); // the first line was cut by the 4 MiB window
    }
    let from = lines.len().saturating_sub(count);
    Ok(lines[from..]
        .iter()
        .map(|line| (*line).to_string())
        .collect())
}

/// `pawn-runs.json`: the last 20 runs, newest last, each entry replaced in
/// place as its run grows. Written atomically; one writer at a time.
pub struct RunsFile {
    path: PathBuf,
    write: Mutex<()>,
}

impl RunsFile {
    pub const NAME: &'static str = "pawn-runs.json";

    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            write: Mutex::new(()),
        }
    }

    pub fn runs(&self) -> Vec<RunLedger> {
        persist::read_json(&self.path).unwrap_or_default()
    }
}

impl LedgerSink for RunsFile {
    fn record(&self, ledger: &RunLedger) -> Result<(), String> {
        let _one = self
            .write
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // A damaged file was set aside whole (`.damaged`) by the load; one
        // that cannot be read is never written over.
        let mut kept = match persist::load_json::<Vec<RunLedger>>(&self.path) {
            persist::Loaded::Found(kept) => kept,
            persist::Loaded::Absent | persist::Loaded::Damaged => Vec::new(),
            persist::Loaded::Unreadable(why) => return Err(why),
        };
        merge_ledger(&mut kept, ledger);
        persist::write_json(&self.path, &kept, true).map_err(|err| err.to_string())
    }
}
