//! The pawn's mailbox (BUILD-SPEC § v0.7, *The tools: a mailbox, not a
//! socket*): what the stone does with a request Tamsin's relay left in her
//! run's `requests\`.
//!
//! Every request is untrusted input. Tamsin can write `requests\` directly,
//! so a forged request must be no stronger than the tool call it imitates:
//! [`parse_request`] takes exactly `{ id, tool, args }`, one known tool, exact
//! argument types and no extra keys, and the [`Broker`] answers it through the
//! same [`Errands`] the window's own presses use, so a forged `install` meets
//! RS-3's refusals word for word.
//!
//! The broker is idempotent per run. An id is reserved before its work
//! starts: a request whose id is already recorded gets the recorded answer, and
//! one whose id is still in flight joins it, so one mutation is ever made for
//! one id. Retiring a run stops new work, lets a mutation already started
//! finish and be recorded, and drops a held Play.
//!
//! The folder I/O (handles, reparse points, the 64 KiB cap on the read) is
//! the summon's; this module is given bytes and a file name.

use std::collections::HashMap;
use std::sync::{Condvar, Mutex};

use rift_deploy::Actor;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// A request is refused unparsed past this many bytes.
pub const MAX_REQUEST_BYTES: usize = 64 * 1024;
/// At most this many requests are worked on at once in one run.
pub const MAX_IN_FLIGHT: usize = 4;
/// `search_library`'s query, in characters.
pub const MAX_QUERY_CHARS: usize = 200;
/// `search_library`'s `limit`.
pub const MAX_SEARCH_LIMIT: u64 = 50;
/// `read_reframework_log`'s `tailLines`.
pub const MAX_TAIL_LINES: u64 = 2000;
/// An option or install id, in bytes: longer than any the library makes.
pub const MAX_ID_BYTES: usize = 512;

/// `<uuid>.json` with a lowercase UUID: the only name a request may have.
/// Returns the UUID.
pub fn request_id_of(name: &str) -> Option<&str> {
    let id = name.strip_suffix(".json")?;
    crate::pawn::fence::is_uuid(id).then_some(id)
}

/// One tool call, its arguments typed exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tool {
    SearchLibrary {
        query: String,
        limit: u64,
    },
    GetOption {
        option_id: String,
    },
    ListInstalled,
    PlanInstall {
        option_id: String,
    },
    Install {
        option_id: String,
        plan_token: String,
    },
    Uninstall {
        install_id: String,
    },
    Play,
    ReadReframeworkLog {
        tail_lines: u64,
    },
}

impl Tool {
    pub fn name(&self) -> &'static str {
        match self {
            Self::SearchLibrary { .. } => "search_library",
            Self::GetOption { .. } => "get_option",
            Self::ListInstalled => "list_installed",
            Self::PlanInstall { .. } => "plan_install",
            Self::Install { .. } => "install",
            Self::Uninstall { .. } => "uninstall",
            Self::Play => "play",
            Self::ReadReframeworkLog { .. } => "read_reframework_log",
        }
    }

    /// Does this call change the game folder? Its outcome is written to the
    /// run's ledger before its answer.
    pub fn mutates(&self) -> bool {
        matches!(self, Self::Install { .. } | Self::Uninstall { .. })
    }
}

/// A request that passed every check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub id: String,
    pub tool: Tool,
}

/// Why a request was refused before any work. `id` is the file's own UUID
/// when the name was a proper one, so the refusal can be answered; a request
/// with no proper name is deleted unanswered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Malformed {
    pub id: Option<String>,
    pub sentence: String,
}

fn malformed(id: Option<&str>, sentence: &str) -> Malformed {
    Malformed {
        id: id.map(str::to_string),
        sentence: sentence.to_string(),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    id: String,
    tool: String,
    args: serde_json::Map<String, Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoArgs {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SearchArgs {
    query: String,
    limit: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct OptionArgs {
    option_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct InstallArgs {
    option_id: String,
    plan_token: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct UninstallArgs {
    install_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct LogArgs {
    tail_lines: u64,
}

fn args<T: serde::de::DeserializeOwned>(
    id: &str,
    raw: serde_json::Map<String, Value>,
) -> Result<T, Malformed> {
    serde_json::from_value(Value::Object(raw)).map_err(|_| {
        malformed(
            Some(id),
            "The stone refused that call: its arguments are not the tool's own.",
        )
    })
}

fn plain_id(id: &str, value: &str, what: &str) -> Result<(), Malformed> {
    if value.is_empty() || value.len() > MAX_ID_BYTES || value.chars().any(char::is_control) {
        return Err(malformed(
            Some(id),
            &format!("The stone refused that call: the {what} is not one it made."),
        ));
    }
    Ok(())
}

/// Check one request, by its file name and its bytes, before any work: the
/// name, the size, UTF-8 JSON, exactly `{ id, tool, args }` with `id` equal to
/// the file's UUID, one known tool, and that tool's exact arguments.
pub fn parse_request(name: &str, bytes: &[u8]) -> Result<Request, Malformed> {
    let Some(id) = request_id_of(name) else {
        return Err(malformed(
            None,
            "A request without a proper name was removed unread.",
        ));
    };
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(malformed(
            Some(id),
            "The stone refused that call: the request is larger than 64 KiB.",
        ));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| {
        malformed(
            Some(id),
            "The stone refused that call: the request is not UTF-8.",
        )
    })?;
    let envelope: Envelope = serde_json::from_str(text).map_err(|_| {
        malformed(
            Some(id),
            "The stone refused that call: the request is not exactly { id, tool, args }.",
        )
    })?;
    if envelope.id != id {
        return Err(malformed(
            Some(id),
            "The stone refused that call: its id is not its file's name.",
        ));
    }
    let tool = match envelope.tool.as_str() {
        "search_library" => {
            let a: SearchArgs = args(id, envelope.args)?;
            if a.query.chars().count() > MAX_QUERY_CHARS || a.query.chars().any(char::is_control) {
                return Err(malformed(
                    Some(id),
                    "The stone refused that search: the query is longer than 200 characters.",
                ));
            }
            if !(1..=MAX_SEARCH_LIMIT).contains(&a.limit) {
                return Err(malformed(
                    Some(id),
                    "The stone refused that search: the limit is 1 to 50.",
                ));
            }
            Tool::SearchLibrary {
                query: a.query,
                limit: a.limit,
            }
        }
        "get_option" | "plan_install" => {
            let a: OptionArgs = args(id, envelope.args)?;
            plain_id(id, &a.option_id, "option id")?;
            if envelope.tool == "get_option" {
                Tool::GetOption {
                    option_id: a.option_id,
                }
            } else {
                Tool::PlanInstall {
                    option_id: a.option_id,
                }
            }
        }
        "list_installed" => {
            let _: NoArgs = args(id, envelope.args)?;
            Tool::ListInstalled
        }
        "install" => {
            let a: InstallArgs = args(id, envelope.args)?;
            plain_id(id, &a.option_id, "option id")?;
            if a.plan_token.len() != 64
                || !a
                    .plan_token
                    .bytes()
                    .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
            {
                return Err(malformed(
                    Some(id),
                    "The stone refused that install: the plan token is not one it gave.",
                ));
            }
            Tool::Install {
                option_id: a.option_id,
                plan_token: a.plan_token,
            }
        }
        "uninstall" => {
            let a: UninstallArgs = args(id, envelope.args)?;
            plain_id(id, &a.install_id, "install id")?;
            Tool::Uninstall {
                install_id: a.install_id,
            }
        }
        "play" => {
            let _: NoArgs = args(id, envelope.args)?;
            Tool::Play
        }
        "read_reframework_log" => {
            let a: LogArgs = args(id, envelope.args)?;
            if !(1..=MAX_TAIL_LINES).contains(&a.tail_lines) {
                return Err(malformed(
                    Some(id),
                    "The stone refused that read: tailLines is 1 to 2000.",
                ));
            }
            Tool::ReadReframeworkLog {
                tail_lines: a.tail_lines,
            }
        }
        _ => {
            return Err(malformed(
                Some(id),
                "The stone refused that call: it has no such tool.",
            ))
        }
    };
    Ok(Request {
        id: id.to_string(),
        tool,
    })
}

// ---------------------------------------------------------------------------
// Answers
// ---------------------------------------------------------------------------

/// What the stone did with a call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    Done,
    Refused,
    Held,
    /// The stone may have done this and could not record that it did: the
    /// world is as `list_installed` shows it. Also how a mutation stands in
    /// the ledger on disk between its start and its outcome.
    Unknown,
}

/// The answer the stone writes to `answers\<id>.json`, in the shape the relay
/// returns to Claude Code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    pub id: String,
    pub outcome: Outcome,
    /// One sentence: the hands row's text, or the stone's refusal verbatim.
    pub text: String,
    #[serde(default)]
    pub data: Value,
}

impl Answer {
    fn refused(id: &str, sentence: impl Into<String>) -> Self {
        Self {
            id: id.to_string(),
            outcome: Outcome::Refused,
            text: sentence.into(),
            data: Value::Null,
        }
    }

    /// The bytes written to `answers\`.
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

/// The stone's own operations, as the window's presses run them. The broker
/// calls nothing else; a refusal is the stone's own sentence, verbatim.
pub trait Errands: Send + Sync {
    fn search_library(&self, query: &str, limit: u64) -> Result<Value, String>;
    fn get_option(&self, option_id: &str) -> Result<Value, String>;
    fn list_installed(&self) -> Result<Value, String>;
    /// The plan, and the library's generation and the journal's sequence it
    /// was made against.
    fn plan_install(&self, option_id: &str) -> Result<PlannedInstall, String>;
    /// Install through the RS-4 journal, recording `actor`, **only if the plan
    /// is still `expected`**: the implementation re-plans and compares the
    /// plan, the library's generation and the journal's sequence under the
    /// same lock that commits the install (the window's own install takes it
    /// too), and refuses with [`STALE_PLAN`] otherwise. A check made before
    /// this call is advice; this one is the gate (the Heretic's WB1).
    fn install(
        &self,
        option_id: &str,
        actor: &Actor,
        expected: &PlannedInstall,
    ) -> Result<Done, String>;
    /// `plan_uninstall` + `uninstall` through the journal. The answer says
    /// whose install it removed ([`Done::removed`]).
    fn uninstall(&self, install_id: &str, actor: &Actor) -> Result<Done, String>;
    /// Show the held Play in the window: run `run_id`'s request
    /// `held_request_id` waits for the Arisen.
    fn hold_play(&self, run_id: &str, held_request_id: &str) -> Result<(), String>;
    /// Drop `run_id`'s held Play, if one is held (the run retired).
    fn drop_held_play(&self, run_id: &str);
    fn read_reframework_log(&self, tail_lines: u64) -> Result<Value, String>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlannedInstall {
    pub plan: Value,
    pub library_generation: u64,
    pub journal_seq: u64,
}

/// A mutation that was made.
#[derive(Debug, Clone, PartialEq)]
pub struct Done {
    pub install_id: Option<String>,
    pub option_id: Option<String>,
    /// The hands row: `installed Stature 0.2.0 · 7 files`.
    pub summary: String,
    pub data: Value,
    /// For an uninstall: the install it removed, as the journal recorded it
    /// at that moment. An option id can be installed again by someone else,
    /// so whose it was is captured here, never inferred later (WB4).
    pub removed: Option<Removed>,
}

/// The install an uninstall removed: who placed it, and its journal `seq`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Removed {
    pub actor: Actor,
    pub seq: u64,
}

/// The stale plan's refusal, the same sentence wherever it is found.
pub const STALE_PLAN: &str =
    "stalePlan: the library or the game changed since that plan. Call plan_install again.";

/// The refusal while the run's ledger cannot be written.
pub const LEDGER_DOWN: &str = "The stone could not write Tamsin's ledger, so it changed nothing. It will not change your game for her until the ledger can be written; tell the Arisen.";

/// The plan token: a hash of the plan, the library's generation, the
/// journal's sequence and the run. `install` refuses `stalePlan` unless the
/// token still matches what a fresh plan would give.
pub fn plan_token(planned: &PlannedInstall, run_id: &str) -> String {
    let mut hash = Sha256::new();
    let plan = serde_json::to_vec(&planned.plan).unwrap_or_default();
    for part in [
        plan.as_slice(),
        &planned.library_generation.to_le_bytes(),
        &planned.journal_seq.to_le_bytes(),
        run_id.as_bytes(),
    ] {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part);
    }
    hash.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------
// The run's ledger and the broker
// ---------------------------------------------------------------------------

/// One call the run acted on, as `pawn-runs.json` keeps it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerOp {
    pub id: String,
    pub tool: String,
    pub install_id: Option<String>,
    pub option_id: Option<String>,
    pub outcome: Outcome,
    /// RFC 3339, UTC.
    pub at: String,
    /// What was answered, so a replayed id gets these exact bytes again.
    pub answer: Answer,
    /// For an uninstall that was done: whose install it removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removed: Option<Removed>,
}

/// One summoning's ledger.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunLedger {
    pub run_id: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub ops: Vec<LedgerOp>,
    /// The Play request waiting for the Arisen, if one is (at most one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub held: Option<HeldPlay>,
    /// The Arisen's answers to held Plays, in order (BUILD-SPEC § v0.7, The
    /// held Play's decision): never written into the held call's answer.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decisions: Vec<PlayDecision>,
}

/// The Play request waiting for the Arisen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeldPlay {
    pub held_request_id: String,
    pub since: String,
}

/// What the Arisen did with a held Play. `played` only once the game started.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Decision {
    Played,
    Declined,
}

/// One decision, and whether a later `play` call has carried it to Tamsin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayDecision {
    pub held_request_id: String,
    pub decision: Decision,
    pub at: String,
    pub delivered: bool,
}

/// Why the Arisen's press on a held Play did nothing (WINDOW-CONTRACT § v0.7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayRefusal {
    /// No Play is waiting.
    NothingHeld,
    /// The press names a run or a request that is not the one waiting now.
    StaleHeld,
    /// Play was pressed and the launch refused or failed: the stone's own
    /// sentence. The request stays waiting.
    Launch(String),
    /// The run's ledger cannot be written, which stops every change in the
    /// run, this one too (the Heretic's WA2): nothing launches, and the
    /// request stays waiting.
    LedgerDown,
}

/// What the Arisen's press did. `persisted` is false when the decision was
/// made (the game started, or the Arisen declined) but could not be written
/// to the run's ledger: the window says so (the Heretic's WA3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayAnswered {
    pub decision: Decision,
    pub persisted: bool,
}

/// One answered call, for the `rift://pawn-hands` event.
#[derive(Debug, Clone, PartialEq)]
pub struct Handled {
    pub answer: Answer,
    pub tool: Option<String>,
    /// The call was a replay, or joined one in flight, and was not acted on.
    pub repeated: bool,
    pub install_id: Option<String>,
}

enum Slot {
    InFlight,
    Recorded(Answer),
}

struct BrokerState {
    ledger: RunLedger,
    slots: HashMap<String, Slot>,
    in_flight: usize,
    retiring: bool,
    /// Calls that found their id in flight and waited for it.
    joined: usize,
    /// The sink refused a write: no further mutation is started (WB2).
    ledger_down: bool,
}

/// A retired run: its closed ledger, and whether the sink kept every write.
#[derive(Debug, Clone, PartialEq)]
pub struct Retired {
    pub ledger: RunLedger,
    pub persisted: bool,
}

/// Where a run's ledger is kept before each mutation's answer: the spine
/// writes `pawn-runs.json`, the tests keep it in memory.
pub trait LedgerSink: Send + Sync {
    fn record(&self, ledger: &RunLedger) -> Result<(), String>;
}

/// One run's broker: the reservation of ids, the ledger and retirement.
pub struct Broker<'a> {
    run_id: String,
    errands: &'a dyn Errands,
    sink: &'a dyn LedgerSink,
    clock: fn() -> String,
    state: Mutex<BrokerState>,
    settled: Condvar,
}

/// RFC 3339, UTC, to the second.
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

impl<'a> Broker<'a> {
    pub fn new(
        run_id: &str,
        errands: &'a dyn Errands,
        sink: &'a dyn LedgerSink,
        clock: fn() -> String,
    ) -> Self {
        let ledger = RunLedger {
            run_id: run_id.to_string(),
            started_at: clock(),
            ended_at: None,
            ops: Vec::new(),
            held: None,
            decisions: Vec::new(),
        };
        Self {
            run_id: run_id.to_string(),
            errands,
            sink,
            clock,
            state: Mutex::new(BrokerState {
                ledger,
                slots: HashMap::new(),
                in_flight: 0,
                retiring: false,
                joined: 0,
                ledger_down: false,
            }),
            settled: Condvar::new(),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BrokerState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn ledger(&self) -> RunLedger {
        self.lock().ledger.clone()
    }

    /// How many calls joined one in flight, and is the run retiring? (The
    /// tests wait on these instead of on a clock.)
    #[cfg(test)]
    fn watch(&self) -> (usize, bool) {
        let state = self.lock();
        (state.joined, state.retiring)
    }

    /// Handle one request file: refuse it if malformed, answer a recorded id
    /// from the ledger, join an id in flight, else reserve the id and act.
    /// `None` when the request had no proper name (deleted unanswered).
    pub fn handle(&self, name: &str, bytes: &[u8]) -> Option<Handled> {
        let request = match parse_request(name, bytes) {
            Ok(request) => request,
            Err(bad) => {
                let id = bad.id?;
                return Some(Handled {
                    answer: Answer::refused(&id, bad.sentence),
                    tool: None,
                    repeated: false,
                    install_id: None,
                });
            }
        };
        let mut state = self.lock();
        loop {
            if state.retiring {
                return Some(Handled {
                    answer: Answer::refused(
                        &request.id,
                        "Tamsin has departed; the stone took no more calls from this summoning.",
                    ),
                    tool: Some(request.tool.name().into()),
                    repeated: false,
                    install_id: None,
                });
            }
            match state.slots.get(&request.id) {
                Some(Slot::Recorded(answer)) => {
                    return Some(Handled {
                        answer: answer.clone(),
                        tool: Some(request.tool.name().into()),
                        repeated: true,
                        install_id: None,
                    });
                }
                Some(Slot::InFlight) => {
                    state.joined += 1;
                    state = self
                        .settled
                        .wait(state)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                }
                None if state.in_flight >= MAX_IN_FLIGHT => {
                    return Some(Handled {
                        answer: Answer::refused(
                            &request.id,
                            "The stone is already doing four things for Tamsin; call again when one is answered.",
                        ),
                        tool: Some(request.tool.name().into()),
                        repeated: false,
                        install_id: None,
                    });
                }
                None => break,
            }
        }
        if request.tool == Tool::Play {
            // Play calls are taken one at a time, under the broker's lock: a
            // decision is delivered in the same ledger write as the answer
            // that carries it.
            let answer = self.play(&mut state, &request.id);
            state
                .slots
                .insert(request.id.clone(), Slot::Recorded(answer.clone()));
            drop(state);
            self.settled.notify_all();
            return Some(Handled {
                answer,
                tool: Some(request.tool.name().into()),
                repeated: false,
                install_id: None,
            });
        }
        let mutates = request.tool.mutates();
        if mutates {
            // The intent is on disk before the mutation starts: a crash or a
            // lost answer leaves `unknown` in the ledger, never nothing. A
            // sink that refuses stops the mutation before it begins.
            let intent = self.intent(&request);
            state.ledger.ops.push(intent);
            if state.ledger_down || self.sink.record(&state.ledger).is_err() {
                state.ledger_down = true;
                state.ledger.ops.pop();
                return Some(Handled {
                    answer: Answer::refused(&request.id, LEDGER_DOWN),
                    tool: Some(request.tool.name().into()),
                    repeated: false,
                    install_id: None,
                });
            }
        }
        state.slots.insert(request.id.clone(), Slot::InFlight);
        state.in_flight += 1;
        drop(state);

        let (mut answer, op) = self.act(&request);

        let mut state = self.lock();
        if let Some(op) = op {
            if let Some(slot) = state.ledger.ops.iter_mut().rev().find(|o| o.id == op.id) {
                *slot = op;
            }
            // Written before the answer is. If it cannot be, the mutation
            // stands but its record does not: the answer says so, and no
            // further mutation starts.
            if self.sink.record(&state.ledger).is_err() {
                state.ledger_down = true;
                if answer.outcome == Outcome::Done {
                    answer.outcome = Outcome::Unknown;
                    answer.text = format!(
                        "unknown: the stone {} but could not record it. Call list_installed to see the world as it is.",
                        answer.text
                    );
                }
            }
        }
        state
            .slots
            .insert(request.id.clone(), Slot::Recorded(answer.clone()));
        state.in_flight -= 1;
        drop(state);
        self.settled.notify_all();
        let install_id = answer
            .data
            .get("installId")
            .and_then(Value::as_str)
            .map(str::to_string);
        Some(Handled {
            answer,
            tool: Some(request.tool.name().into()),
            repeated: false,
            install_id,
        })
    }

    fn act(&self, request: &Request) -> (Answer, Option<LedgerOp>) {
        let id = request.id.as_str();
        let read = |result: Result<Value, String>, text: &str| match result {
            Ok(data) => Answer {
                id: id.to_string(),
                outcome: Outcome::Done,
                text: text.to_string(),
                data,
            },
            Err(sentence) => Answer::refused(id, sentence),
        };
        let actor = Actor::Pawn {
            run_id: self.run_id.clone(),
        };
        match &request.tool {
            Tool::SearchLibrary { query, limit } => (
                read(
                    self.errands.search_library(query, *limit),
                    "searched the library",
                ),
                None,
            ),
            Tool::GetOption { option_id } => (
                read(self.errands.get_option(option_id), "read an option"),
                None,
            ),
            Tool::ListInstalled => {
                let mut answer = read(self.errands.list_installed(), "listed the installs");
                if answer.outcome == Outcome::Done {
                    // Every list carries the Play, read and never consumed.
                    let mut map = match std::mem::take(&mut answer.data) {
                        Value::Object(map) => map,
                        other => serde_json::Map::from_iter([("installed".to_string(), other)]),
                    };
                    map.insert("play".into(), self.play_view());
                    answer.data = Value::Object(map);
                }
                (answer, None)
            }
            Tool::PlanInstall { option_id } => {
                let answer = match self.errands.plan_install(option_id) {
                    Ok(planned) => Answer {
                        id: id.to_string(),
                        outcome: Outcome::Done,
                        text: "planned an install".into(),
                        data: serde_json::json!({
                            "plan": planned.plan,
                            "planToken": plan_token(&planned, &self.run_id),
                        }),
                    },
                    Err(sentence) => Answer::refused(id, sentence),
                };
                (answer, None)
            }
            Tool::Install {
                option_id,
                plan_token: token,
            } => {
                let answer = match self.errands.plan_install(option_id) {
                    Err(sentence) => Answer::refused(id, sentence),
                    Ok(fresh) if plan_token(&fresh, &self.run_id) != *token => {
                        Answer::refused(id, STALE_PLAN)
                    }
                    // The plan the token names is handed to the install,
                    // which checks it again where it commits.
                    Ok(fresh) => match self.errands.install(option_id, &actor, &fresh) {
                        Ok(done) => done_answer(id, done, option_id, None).0,
                        Err(sentence) => Answer::refused(id, sentence),
                    },
                };
                let op = self.op(request, &answer, Some(option_id.clone()), None);
                (answer, Some(op))
            }
            Tool::Uninstall { install_id } => {
                let (answer, removed) = match self.errands.uninstall(install_id, &actor) {
                    Ok(done) => done_answer(id, done, "", Some(install_id)),
                    Err(sentence) => (Answer::refused(id, sentence), None),
                };
                let op = self.op(request, &answer, None, removed);
                (answer, Some(op))
            }
            // Answered under the broker's lock in `handle`.
            Tool::Play => (
                Answer::refused(id, "The stone answers Play under its own lock."),
                None,
            ),
            Tool::ReadReframeworkLog { tail_lines } => (
                read(
                    self.errands.read_reframework_log(*tail_lines),
                    "read the REFramework log",
                ),
                None,
            ),
        }
    }

    /// A mutation as the ledger holds it before it starts: `unknown`.
    fn intent(&self, request: &Request) -> LedgerOp {
        let started = Answer {
            id: request.id.clone(),
            outcome: Outcome::Unknown,
            text: "started".into(),
            data: Value::Null,
        };
        let option_id = match &request.tool {
            Tool::Install { option_id, .. } => Some(option_id.clone()),
            _ => None,
        };
        self.op(request, &started, option_id, None)
    }

    fn op(
        &self,
        request: &Request,
        answer: &Answer,
        option_id: Option<String>,
        removed: Option<Removed>,
    ) -> LedgerOp {
        let field = |key: &str| {
            answer
                .data
                .get(key)
                .and_then(Value::as_str)
                .map(str::to_string)
        };
        LedgerOp {
            id: request.id.clone(),
            tool: request.tool.name().into(),
            install_id: field("installId").or(match &request.tool {
                Tool::Uninstall { install_id } => Some(install_id.clone()),
                _ => None,
            }),
            option_id: field("optionId").or(option_id),
            outcome: answer.outcome,
            at: (self.clock)(),
            answer: answer.clone(),
            removed,
        }
    }

    /// The Play as `list_installed` shows it, read and never consumed:
    /// `{ waiting, lastDecision }`.
    fn play_view(&self) -> Value {
        let state = self.lock();
        let last = state.ledger.decisions.last().map(|d| {
            serde_json::json!({
                "heldRequestId": d.held_request_id,
                "decision": d.decision,
                "at": d.at,
            })
        });
        serde_json::json!({
            "waiting": state.ledger.held.as_ref().map(|h| h.held_request_id.clone()),
            "lastDecision": last,
        })
    }

    /// A new `play` call (BUILD-SPEC § v0.7, The held Play's decision): an
    /// undelivered decision is delivered and nothing new is held; a request
    /// already waiting is named and nothing new is held; else this call is
    /// held. Each changes the ledger in one write with the answer.
    fn play(&self, state: &mut BrokerState, id: &str) -> Answer {
        if state.ledger_down {
            return Answer::refused(id, LEDGER_DOWN);
        }
        if let Some(index) = state.ledger.decisions.iter().position(|d| !d.delivered) {
            let decision = state.ledger.decisions[index].clone();
            let text = match decision.decision {
                Decision::Played => {
                    format!("played at {}: the Arisen started the game", decision.at)
                }
                Decision::Declined => "declined: the Arisen said not now".to_string(),
            };
            let answer = Answer {
                id: id.to_string(),
                outcome: Outcome::Done,
                text,
                data: serde_json::json!({
                    "heldRequestId": decision.held_request_id,
                    "decision": decision.decision,
                    "at": decision.at,
                }),
            };
            state.ledger.decisions[index].delivered = true;
            state.ledger.ops.push(self.play_op(id, &answer));
            if self.sink.record(&state.ledger).is_err() {
                state.ledger.decisions[index].delivered = false;
                state.ledger.ops.pop();
                state.ledger_down = true;
                return Answer::refused(id, LEDGER_DOWN);
            }
            return answer;
        }
        if let Some(held) = &state.ledger.held {
            return Answer {
                id: id.to_string(),
                outcome: Outcome::Held,
                text: "held: a Play is already waiting for the Arisen".into(),
                data: serde_json::json!({ "heldRequestId": held.held_request_id }),
            };
        }
        let answer = Answer {
            id: id.to_string(),
            outcome: Outcome::Held,
            text: "held: waiting for the Arisen".into(),
            data: serde_json::json!({ "heldRequestId": id }),
        };
        state.ledger.held = Some(HeldPlay {
            held_request_id: id.to_string(),
            since: (self.clock)(),
        });
        state.ledger.ops.push(self.play_op(id, &answer));
        if self.sink.record(&state.ledger).is_err() {
            state.ledger.held = None;
            state.ledger.ops.pop();
            state.ledger_down = true;
            return Answer::refused(id, LEDGER_DOWN);
        }
        if let Err(sentence) = self.errands.hold_play(&self.run_id, id) {
            state.ledger.held = None;
            state.ledger.ops.pop();
            let _ = self.sink.record(&state.ledger);
            return Answer::refused(id, sentence);
        }
        answer
    }

    fn play_op(&self, id: &str, answer: &Answer) -> LedgerOp {
        LedgerOp {
            id: id.to_string(),
            tool: "play".into(),
            install_id: None,
            option_id: None,
            outcome: answer.outcome,
            at: (self.clock)(),
            answer: answer.clone(),
            removed: None,
        }
    }

    /// The Arisen's press on a held Play (`answer_pawn_play`). It acts only
    /// when `run_id` and `held_request_id` are this live run and its waiting
    /// request, checked under the broker's lock, which is held through
    /// `launch` so nothing can change between the check and the game
    /// starting. `played` is recorded only when `launch` started the game; a
    /// launch that refused or failed records nothing and the request stays
    /// waiting.
    pub fn answer_play(
        &self,
        run_id: &str,
        held_request_id: &str,
        accept: bool,
        launch: impl FnOnce() -> Result<(), String>,
    ) -> Result<PlayAnswered, PlayRefusal> {
        let mut state = self.lock();
        if run_id != self.run_id || state.retiring {
            return Err(PlayRefusal::StaleHeld);
        }
        match &state.ledger.held {
            None => return Err(PlayRefusal::NothingHeld),
            Some(held) if held.held_request_id != held_request_id => {
                return Err(PlayRefusal::StaleHeld)
            }
            Some(_) => {}
        }
        if state.ledger_down {
            return Err(PlayRefusal::LedgerDown);
        }
        let decision = if accept {
            launch().map_err(PlayRefusal::Launch)?;
            Decision::Played
        } else {
            Decision::Declined
        };
        state.ledger.held = None;
        state.ledger.decisions.push(PlayDecision {
            held_request_id: held_request_id.to_string(),
            decision,
            at: (self.clock)(),
            delivered: false,
        });
        let persisted = match self.sink.record(&state.ledger) {
            Ok(()) => true,
            Err(err) => {
                log::warn!("the pawn's run ledger could not record the Arisen's answer: {err}");
                state.ledger_down = true;
                false
            }
        };
        Ok(PlayAnswered {
            decision,
            persisted,
        })
    }

    /// Retire the run (BUILD-SPEC § v0.7, runs): take no new calls, wait for
    /// every call already started to be recorded, drop a held Play, and close
    /// the ledger. `persisted` is false when any write of this run's ledger
    /// failed, the closing one included: the window says so, and the ledger
    /// on disk is not complete.
    pub fn retire(&self) -> Retired {
        let mut state = self.lock();
        state.retiring = true;
        while state.in_flight > 0 {
            state = self
                .settled
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        // A held Play dies with its run, unanswered; its decisions stay.
        state.ledger.held = None;
        self.errands.drop_held_play(&self.run_id);
        state.ledger.ended_at = Some((self.clock)());
        if let Err(err) = self.sink.record(&state.ledger) {
            log::warn!("the pawn's run ledger could not be closed: {err}");
            state.ledger_down = true;
        }
        Retired {
            ledger: state.ledger.clone(),
            persisted: !state.ledger_down,
        }
    }
}

fn done_answer(
    id: &str,
    done: Done,
    option_id: &str,
    install_id: Option<&str>,
) -> (Answer, Option<Removed>) {
    let mut data = match done.data {
        Value::Object(map) => map,
        Value::Null => serde_json::Map::new(),
        other => {
            let mut map = serde_json::Map::new();
            map.insert("result".into(), other);
            map
        }
    };
    let install_id = done.install_id.or(install_id.map(str::to_string));
    if let Some(install_id) = install_id {
        data.insert("installId".into(), Value::String(install_id));
    }
    let option_id = done
        .option_id
        .or((!option_id.is_empty()).then(|| option_id.to_string()));
    if let Some(option_id) = option_id {
        data.insert("optionId".into(), Value::String(option_id));
    }
    let answer = Answer {
        id: id.to_string(),
        outcome: Outcome::Done,
        text: done.summary,
        data: Value::Object(data),
    };
    (answer, done.removed)
}

/// `pawn-runs.json`: the last [`KEPT_RUNS`] runs, newest last. A run's entry is
/// replaced in place as it grows.
pub const KEPT_RUNS: usize = 20;

pub fn merge_ledger(kept: &mut Vec<RunLedger>, ledger: &RunLedger) {
    match kept.iter_mut().find(|r| r.run_id == ledger.run_id) {
        Some(entry) => *entry = ledger.clone(),
        None => kept.push(ledger.clone()),
    }
    if kept.len() > KEPT_RUNS {
        let extra = kept.len() - KEPT_RUNS;
        kept.drain(..extra);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    const RUN: &str = "0b5e2c6e-4a8d-4d3e-9f4b-2a1c7d9e8f00";
    const ID: &str = "11111111-2222-4333-8444-555555555555";

    fn clock() -> String {
        "2026-10-28T12:00:00Z".into()
    }

    fn name(id: &str) -> String {
        format!("{id}.json")
    }

    fn body(value: Value) -> Vec<u8> {
        serde_json::to_vec(&value).unwrap()
    }

    /// An invented stone: one library generation, a journal sequence that
    /// moves on every install, and RS-3's refusal for one option.
    #[derive(Default)]
    struct Stone {
        generation: AtomicUsize,
        seq: AtomicUsize,
        installs: AtomicUsize,
        uninstalls: AtomicUsize,
        held: Mutex<Vec<String>>,
        dropped: Mutex<Vec<String>>,
        /// Slows install so a duplicate can arrive while it runs.
        gate: Mutex<Option<Arc<std::sync::Barrier>>>,
        /// Runs after the broker's token check and before the install's own
        /// gate: something else changing the world in between.
        meanwhile: Mutex<Option<fn(&Stone)>>,
        /// The one lock the install's check and commit share.
        commit: Mutex<()>,
        /// Whose install the next uninstall removes.
        removes: Mutex<Option<Removed>>,
    }

    impl Errands for Stone {
        fn search_library(&self, query: &str, limit: u64) -> Result<Value, String> {
            Ok(serde_json::json!({ "query": query, "limit": limit, "hits": [] }))
        }
        fn get_option(&self, option_id: &str) -> Result<Value, String> {
            Ok(serde_json::json!({ "optionId": option_id }))
        }
        fn list_installed(&self) -> Result<Value, String> {
            Ok(serde_json::json!([]))
        }
        fn plan_install(&self, option_id: &str) -> Result<PlannedInstall, String> {
            Ok(PlannedInstall {
                plan: serde_json::json!({ "optionId": option_id, "files": 7 }),
                library_generation: self.generation.load(Ordering::SeqCst) as u64,
                journal_seq: self.seq.load(Ordering::SeqCst) as u64,
            })
        }
        fn install(
            &self,
            option_id: &str,
            actor: &Actor,
            expected: &PlannedInstall,
        ) -> Result<Done, String> {
            assert_eq!(actor, &Actor::Pawn { run_id: RUN.into() });
            if option_id.contains("vanilla") {
                return Err("refused: that would write over a file the game shipped with.".into());
            }
            let barrier = self.gate.lock().unwrap().clone();
            if let Some(barrier) = barrier {
                barrier.wait();
                barrier.wait();
            }
            if let Some(meanwhile) = *self.meanwhile.lock().unwrap() {
                meanwhile(self);
            }
            let _commit = self.commit.lock().unwrap();
            if self.plan_install(option_id)? != *expected {
                return Err(STALE_PLAN.into());
            }
            let n = self.installs.fetch_add(1, Ordering::SeqCst) + 1;
            self.seq.fetch_add(1, Ordering::SeqCst);
            Ok(Done {
                install_id: Some(format!("install-{n}")),
                option_id: Some(option_id.into()),
                summary: "installed Stature 0.2.0 · 7 files".into(),
                data: Value::Null,
                removed: None,
            })
        }
        fn uninstall(&self, install_id: &str, _actor: &Actor) -> Result<Done, String> {
            self.uninstalls.fetch_add(1, Ordering::SeqCst);
            Ok(Done {
                install_id: Some(install_id.into()),
                option_id: Some("00000000000000aa#Glow".into()),
                summary: "uninstalled Lantern Glow".into(),
                data: Value::Null,
                removed: self.removes.lock().unwrap().clone(),
            })
        }
        fn hold_play(&self, run_id: &str, held_request_id: &str) -> Result<(), String> {
            self.held
                .lock()
                .unwrap()
                .push(format!("{run_id}/{held_request_id}"));
            Ok(())
        }
        fn drop_held_play(&self, run_id: &str) {
            self.dropped.lock().unwrap().push(run_id.into());
        }
        fn read_reframework_log(&self, tail_lines: u64) -> Result<Value, String> {
            Ok(serde_json::json!({ "lines": [], "count": 0, "asked": tail_lines }))
        }
    }

    /// The ledger as written, in order; it refuses every write from the
    /// `fail_from`-th on (counting from 0) when that is set.
    #[derive(Default)]
    struct Sink(Mutex<Vec<RunLedger>>, Mutex<Option<usize>>);

    impl LedgerSink for Sink {
        fn record(&self, ledger: &RunLedger) -> Result<(), String> {
            let mut written = self.0.lock().unwrap();
            if self
                .1
                .lock()
                .unwrap()
                .is_some_and(|from| written.len() >= from)
            {
                return Err("the disk is full".into());
            }
            written.push(ledger.clone());
            Ok(())
        }
    }

    impl Sink {
        fn failing_from(from: usize) -> Self {
            Self(Mutex::default(), Mutex::new(Some(from)))
        }
    }

    fn planned_token(stone: &Stone, option: &str) -> String {
        plan_token(&stone.plan_install(option).unwrap(), RUN)
    }

    #[test]
    fn every_malformed_request_is_refused_before_any_work() {
        let good = serde_json::json!({ "id": ID, "tool": "list_installed", "args": {} });
        assert!(parse_request(&name(ID), &body(good)).is_ok());
        let token = "a".repeat(64);
        let cases: Vec<(&str, String, Vec<u8>)> = vec![
            ("not JSON", name(ID), b"{ not json".to_vec()),
            ("not UTF-8", name(ID), vec![0xff, 0xfe, b'{', b'}']),
            ("oversized", name(ID), {
                let mut big = body(serde_json::json!({ "id": ID, "tool": "search_library",
                    "args": { "query": "x", "limit": 1 } }));
                big.resize(MAX_REQUEST_BYTES + 1, b' ');
                big
            }),
            (
                "an unknown tool",
                name(ID),
                body(serde_json::json!({ "id": ID, "tool": "write_file", "args": {} })),
            ),
            (
                "an extra key",
                name(ID),
                body(
                    serde_json::json!({ "id": ID, "tool": "list_installed", "args": {}, "path": "C:/" }),
                ),
            ),
            (
                "an extra argument",
                name(ID),
                body(
                    serde_json::json!({ "id": ID, "tool": "list_installed", "args": { "all": true } }),
                ),
            ),
            (
                "a missing argument",
                name(ID),
                body(
                    serde_json::json!({ "id": ID, "tool": "install", "args": { "optionId": "a#b" } }),
                ),
            ),
            (
                "a wrong type",
                name(ID),
                body(
                    serde_json::json!({ "id": ID, "tool": "search_library", "args": { "query": "x", "limit": "50" } }),
                ),
            ),
            (
                "a limit too high",
                name(ID),
                body(
                    serde_json::json!({ "id": ID, "tool": "search_library", "args": { "query": "x", "limit": 51 } }),
                ),
            ),
            (
                "a query too long",
                name(ID),
                body(
                    serde_json::json!({ "id": ID, "tool": "search_library", "args": { "query": "x".repeat(201), "limit": 5 } }),
                ),
            ),
            (
                "a tail too long",
                name(ID),
                body(
                    serde_json::json!({ "id": ID, "tool": "read_reframework_log", "args": { "tailLines": 2001 } }),
                ),
            ),
            (
                "a forged token",
                name(ID),
                body(
                    serde_json::json!({ "id": ID, "tool": "install", "args": { "optionId": "a#b", "planToken": "../../x" } }),
                ),
            ),
            (
                "an empty option id",
                name(ID),
                body(
                    serde_json::json!({ "id": ID, "tool": "get_option", "args": { "optionId": "" } }),
                ),
            ),
            (
                "an id that is not the file's",
                name(ID),
                body(serde_json::json!({ "id": RUN, "tool": "list_installed", "args": {} })),
            ),
            (
                "a duplicate key",
                name(ID),
                format!(r#"{{"id":"{ID}","tool":"play","tool":"install","args":{{}}}}"#)
                    .into_bytes(),
            ),
            (
                "args that are not an object",
                name(ID),
                body(serde_json::json!({ "id": ID, "tool": "list_installed", "args": [] })),
            ),
            (
                "a plan token in upper case",
                name(ID),
                body(
                    serde_json::json!({ "id": ID, "tool": "install", "args": { "optionId": "a#b", "planToken": token.to_uppercase() } }),
                ),
            ),
        ];
        for (what, file, bytes) in cases {
            let refused = parse_request(&file, &bytes).unwrap_err();
            assert_eq!(
                refused.id.as_deref(),
                Some(ID),
                "{what} is answered under its own id"
            );
            assert!(refused.sentence.starts_with("The stone refused"), "{what}");
        }
        for bad_name in [
            "x.json",
            "11111111-2222-4333-8444-555555555555.JSON",
            "11111111-2222-4333-8444-555555555555.json.tmp",
            "11111111-2222-4333-8444-55555555555G.json",
            "11111111-2222-4333-8444-555555555555",
            "../11111111-2222-4333-8444-555555555555.json",
            "11111111-2222-4333-8444-5555555555551.json",
        ] {
            let refused = parse_request(bad_name, b"{}").unwrap_err();
            assert_eq!(refused.id, None, "{bad_name} is removed unanswered");
        }
    }

    #[test]
    fn a_refused_request_is_answered_and_a_nameless_one_is_not() {
        let stone = Stone::default();
        let sink = Sink::default();
        let broker = Broker::new(RUN, &stone, &sink, clock);
        let handled = broker.handle(&name(ID), b"[]").unwrap();
        assert_eq!(handled.answer.outcome, Outcome::Refused);
        assert!(broker.handle("evil.json", b"{}").is_none());
        assert_eq!(stone.installs.load(Ordering::SeqCst), 0);
    }

    fn install_request(id: &str, option: &str, token: &str) -> Vec<u8> {
        body(serde_json::json!({ "id": id, "tool": "install",
            "args": { "optionId": option, "planToken": token } }))
    }

    #[test]
    fn install_needs_a_fresh_plan_and_records_the_pawn() {
        let stone = Stone::default();
        let sink = Sink::default();
        let broker = Broker::new(RUN, &stone, &sink, clock);
        let plan = broker
            .handle(
                &name(ID),
                &body(serde_json::json!({ "id": ID, "tool": "plan_install", "args": { "optionId": "00000000000000ab#Stature" } })),
            )
            .unwrap();
        let token = plan.answer.data["planToken"].as_str().unwrap().to_string();
        assert_eq!(token, planned_token(&stone, "00000000000000ab#Stature"));

        // The library moves on: the token is stale.
        stone.generation.fetch_add(1, Ordering::SeqCst);
        let stale_id = "22222222-2222-4333-8444-555555555555";
        let stale = broker
            .handle(
                &name(stale_id),
                &install_request(stale_id, "00000000000000ab#Stature", &token),
            )
            .unwrap();
        assert_eq!(stale.answer.outcome, Outcome::Refused);
        assert!(stale.answer.text.starts_with("stalePlan"));
        assert_eq!(stone.installs.load(Ordering::SeqCst), 0);

        // A token from another run is stale here too.
        let other_run = plan_token(&stone.plan_install("00000000000000ab#Stature").unwrap(), ID);
        let id3 = "33333333-2222-4333-8444-555555555555";
        let foreign = broker
            .handle(
                &name(id3),
                &install_request(id3, "00000000000000ab#Stature", &other_run),
            )
            .unwrap();
        assert!(foreign.answer.text.starts_with("stalePlan"));

        let fresh = planned_token(&stone, "00000000000000ab#Stature");
        let id4 = "44444444-2222-4333-8444-555555555555";
        let done = broker
            .handle(
                &name(id4),
                &install_request(id4, "00000000000000ab#Stature", &fresh),
            )
            .unwrap();
        assert_eq!(done.answer.outcome, Outcome::Done);
        assert_eq!(done.answer.text, "installed Stature 0.2.0 · 7 files");
        assert_eq!(done.install_id.as_deref(), Some("install-1"));
        let ledger = broker.ledger();
        let last = ledger.ops.last().unwrap();
        assert_eq!(last.install_id.as_deref(), Some("install-1"));
        assert_eq!(last.option_id.as_deref(), Some("00000000000000ab#Stature"));
        // Every mutation (refused ones included) is recorded twice: its
        // intent before it starts, its outcome before its answer.
        assert_eq!(ledger.ops.len(), 3);
        assert_eq!(sink.0.lock().unwrap().len(), 6);
    }

    #[test]
    fn a_forged_install_meets_the_tools_own_refusal_word_for_word() {
        let stone = Stone::default();
        let sink = Sink::default();
        let broker = Broker::new(RUN, &stone, &sink, clock);
        let option = "00000000000000ac#vanilla-override";
        let token = planned_token(&stone, option);
        let handled = broker
            .handle(&name(ID), &install_request(ID, option, &token))
            .unwrap();
        assert_eq!(handled.answer.outcome, Outcome::Refused);
        assert_eq!(
            handled.answer.text,
            "refused: that would write over a file the game shipped with."
        );
    }

    #[test]
    fn a_replayed_id_gets_the_ledgers_answer_without_acting() {
        let stone = Stone::default();
        let sink = Sink::default();
        let broker = Broker::new(RUN, &stone, &sink, clock);
        let option = "00000000000000ab#Stature";
        let token = planned_token(&stone, option);
        let first = broker
            .handle(&name(ID), &install_request(ID, option, &token))
            .unwrap();
        let again = broker
            .handle(&name(ID), &install_request(ID, option, &token))
            .unwrap();
        assert!(!first.repeated);
        assert!(again.repeated);
        assert_eq!(first.answer, again.answer);
        assert_eq!(stone.installs.load(Ordering::SeqCst), 1);
        // Even with other bytes under the same id: the id was acted on.
        let other = broker
            .handle(&name(ID), &body(serde_json::json!({ "id": ID, "tool": "uninstall", "args": { "installId": "install-1" } })))
            .unwrap();
        assert!(other.repeated);
        assert_eq!(stone.uninstalls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_duplicate_sent_while_the_first_runs_joins_it() {
        let stone = Stone::default();
        let barrier = Arc::new(std::sync::Barrier::new(2));
        *stone.gate.lock().unwrap() = Some(Arc::clone(&barrier));
        let sink = Sink::default();
        let broker = Broker::new(RUN, &stone, &sink, clock);
        let option = "00000000000000ab#Stature";
        let token = planned_token(&stone, option);
        let request = install_request(ID, option, &token);
        let (first, second) = std::thread::scope(|scope| {
            let first = scope.spawn(|| broker.handle(&name(ID), &request).unwrap());
            barrier.wait(); // the first is inside install now
            let second = scope.spawn(|| broker.handle(&name(ID), &request).unwrap());
            // Let the first finish only once the duplicate is waiting on it.
            while broker.watch().0 == 0 {
                std::thread::yield_now();
            }
            barrier.wait();
            (first.join().unwrap(), second.join().unwrap())
        });
        assert_eq!(stone.installs.load(Ordering::SeqCst), 1, "one mutation");
        assert_eq!(first.answer, second.answer, "two identical answers");
        assert!(second.repeated);
        assert_eq!(
            broker.watch().0,
            1,
            "the duplicate joined the first in flight"
        );
    }

    #[test]
    fn retiring_lets_a_running_mutation_finish_records_it_and_drops_the_held_play() {
        let stone = Stone::default();
        let barrier = Arc::new(std::sync::Barrier::new(2));
        *stone.gate.lock().unwrap() = Some(Arc::clone(&barrier));
        let sink = Sink::default();
        let broker = Broker::new(RUN, &stone, &sink, clock);
        let play_id = "55555555-2222-4333-8444-555555555555";
        let held = broker
            .handle(
                &name(play_id),
                &body(serde_json::json!({ "id": play_id, "tool": "play", "args": {} })),
            )
            .unwrap();
        assert_eq!(held.answer.outcome, Outcome::Held);
        assert_eq!(
            stone.held.lock().unwrap().as_slice(),
            [format!("{RUN}/{play_id}")]
        );

        let option = "00000000000000ab#Stature";
        let token = planned_token(&stone, option);
        let request = install_request(ID, option, &token);
        let retired = std::thread::scope(|scope| {
            let install = scope.spawn(|| broker.handle(&name(ID), &request).unwrap());
            barrier.wait(); // install is running
            let retire = scope.spawn(|| broker.retire());
            while !broker.watch().1 {
                std::thread::yield_now();
            }
            // A new call while retiring is refused, not acted on.
            let late_id = "66666666-2222-4333-8444-555555555555";
            let late = broker
                .handle(&name(late_id), &install_request(late_id, option, &token))
                .unwrap();
            assert_eq!(late.answer.outcome, Outcome::Refused);
            barrier.wait();
            install.join().unwrap();
            retire.join().unwrap()
        });
        assert!(retired.persisted);
        let ledger = retired.ledger;
        assert_eq!(stone.installs.load(Ordering::SeqCst), 1);
        let installs: Vec<&LedgerOp> = ledger
            .ops
            .iter()
            .filter(|op| op.tool == "install")
            .collect();
        assert_eq!(installs.len(), 1, "the mutation in flight was recorded");
        assert_eq!(installs[0].outcome, Outcome::Done);
        assert_eq!(ledger.held, None, "the held Play died with its run");
        assert_eq!(ledger.ended_at.as_deref(), Some("2026-10-28T12:00:00Z"));
        assert_eq!(stone.dropped.lock().unwrap().as_slice(), [RUN]);
        assert_eq!(sink.0.lock().unwrap().last().unwrap(), &ledger);
    }

    #[test]
    fn a_world_that_moves_between_the_check_and_the_commit_refuses_stale() {
        // The Heretic's WB1: the broker's token check is advice; the install
        // re-checks the plan it was handed where it commits.
        let stone = Stone::default();
        *stone.meanwhile.lock().unwrap() = Some(|stone: &Stone| {
            stone.seq.fetch_add(1, Ordering::SeqCst);
        });
        let sink = Sink::default();
        let broker = Broker::new(RUN, &stone, &sink, clock);
        let option = "00000000000000ab#Stature";
        let token = planned_token(&stone, option);
        let handled = broker
            .handle(&name(ID), &install_request(ID, option, &token))
            .unwrap();
        assert_eq!(handled.answer.outcome, Outcome::Refused);
        assert_eq!(handled.answer.text, STALE_PLAN);
        assert_eq!(
            stone.installs.load(Ordering::SeqCst),
            0,
            "nothing installed"
        );
        assert_eq!(broker.ledger().ops[0].outcome, Outcome::Refused);
    }

    #[test]
    fn the_intent_is_on_disk_before_the_mutation_starts() {
        let stone = Stone::default();
        let sink = Sink::default();
        let broker = Broker::new(RUN, &stone, &sink, clock);
        let option = "00000000000000ab#Stature";
        let token = planned_token(&stone, option);
        broker
            .handle(&name(ID), &install_request(ID, option, &token))
            .unwrap();
        let written = sink.0.lock().unwrap();
        assert_eq!(written[0].ops[0].outcome, Outcome::Unknown, "the intent");
        assert_eq!(written[0].ops[0].option_id.as_deref(), Some(option));
        assert_eq!(
            written[1].ops[0].outcome,
            Outcome::Done,
            "the outcome, in place"
        );
        assert_eq!(written[1].ops.len(), 1);
    }

    #[test]
    fn a_ledger_that_cannot_be_written_stops_every_mutation_before_it_starts() {
        // The Heretic's WB2, first half: a sink that refuses the intent.
        let stone = Stone::default();
        let sink = Sink::failing_from(0);
        let broker = Broker::new(RUN, &stone, &sink, clock);
        let option = "00000000000000ab#Stature";
        let token = planned_token(&stone, option);
        let first = broker
            .handle(&name(ID), &install_request(ID, option, &token))
            .unwrap();
        assert_eq!(first.answer.outcome, Outcome::Refused);
        assert_eq!(first.answer.text, LEDGER_DOWN);
        assert_eq!(stone.installs.load(Ordering::SeqCst), 0);
        // The sink recovers; the run stays stopped (its ledger is not whole).
        *sink.1.lock().unwrap() = None;
        let uninstall_id = "22222222-2222-4333-8444-555555555555";
        let second = broker
            .handle(
                &name(uninstall_id),
                &body(serde_json::json!({ "id": uninstall_id,
                "tool": "uninstall", "args": { "installId": "aa#Glow" } })),
            )
            .unwrap();
        assert_eq!(second.answer.text, LEDGER_DOWN);
        assert_eq!(stone.uninstalls.load(Ordering::SeqCst), 0);
        // Reading tools still answer.
        let read_id = "33333333-2222-4333-8444-555555555555";
        let read = broker
            .handle(
                &name(read_id),
                &body(serde_json::json!({ "id": read_id,
                "tool": "list_installed", "args": {} })),
            )
            .unwrap();
        assert_eq!(read.answer.outcome, Outcome::Done);
        assert!(!broker.retire().persisted);
    }

    #[test]
    fn a_done_mutation_whose_outcome_cannot_be_written_answers_unknown() {
        // The Heretic's WB2, second half: the intent was written, the
        // mutation was made, the outcome could not be recorded.
        let stone = Stone::default();
        let sink = Sink::failing_from(1);
        let broker = Broker::new(RUN, &stone, &sink, clock);
        let option = "00000000000000ab#Stature";
        let token = planned_token(&stone, option);
        let handled = broker
            .handle(&name(ID), &install_request(ID, option, &token))
            .unwrap();
        assert_eq!(stone.installs.load(Ordering::SeqCst), 1, "it was made");
        assert_eq!(
            handled.answer.outcome,
            Outcome::Unknown,
            "and never claimed as recorded"
        );
        assert!(handled.answer.text.starts_with("unknown:"));
        assert_eq!(
            sink.0.lock().unwrap()[0].ops[0].outcome,
            Outcome::Unknown,
            "the disk says it may have happened"
        );
        // A replay gets the same honest answer, and nothing more is started.
        let again = broker
            .handle(&name(ID), &install_request(ID, option, &token))
            .unwrap();
        assert_eq!(again.answer, handled.answer);
        let next_id = "44444444-2222-4333-8444-555555555555";
        let next = broker
            .handle(
                &name(next_id),
                &install_request(next_id, option, &planned_token(&stone, option)),
            )
            .unwrap();
        assert_eq!(next.answer.text, LEDGER_DOWN);
        assert_eq!(stone.installs.load(Ordering::SeqCst), 1);
        assert!(!broker.retire().persisted);
    }

    #[test]
    fn an_uninstall_records_whose_install_it_removed() {
        let stone = Stone::default();
        *stone.removes.lock().unwrap() = Some(Removed {
            actor: Actor::Player,
            seq: 7,
        });
        let sink = Sink::default();
        let broker = Broker::new(RUN, &stone, &sink, clock);
        broker
            .handle(
                &name(ID),
                &body(serde_json::json!({ "id": ID, "tool": "uninstall",
                "args": { "installId": "aa#Glow" } })),
            )
            .unwrap();
        let op = &broker.ledger().ops[0];
        assert_eq!(
            op.removed,
            Some(Removed {
                actor: Actor::Player,
                seq: 7
            })
        );
        let json = serde_json::to_value(op).unwrap();
        assert_eq!(
            json["removed"],
            serde_json::json!({ "actor": { "kind": "player" }, "seq": 7 })
        );
    }

    fn play(broker: &Broker<'_>, id: &str) -> Answer {
        broker
            .handle(
                &name(id),
                &body(serde_json::json!({ "id": id, "tool": "play", "args": {} })),
            )
            .unwrap()
            .answer
    }

    fn list(broker: &Broker<'_>, id: &str) -> Value {
        broker
            .handle(
                &name(id),
                &body(serde_json::json!({ "id": id, "tool": "list_installed", "args": {} })),
            )
            .unwrap()
            .answer
            .data
    }

    const H1: &str = "a1111111-2222-4333-8444-555555555555";
    const H2: &str = "a2222222-2222-4333-8444-555555555555";
    const H3: &str = "a3333333-2222-4333-8444-555555555555";
    const L1: &str = "b1111111-2222-4333-8444-555555555555";
    const L2: &str = "b2222222-2222-4333-8444-555555555555";

    #[test]
    fn one_play_is_held_per_run_and_its_answer_never_changes() {
        let stone = Stone::default();
        let sink = Sink::default();
        let broker = Broker::new(RUN, &stone, &sink, clock);
        let first = play(&broker, H1);
        assert_eq!(first.outcome, Outcome::Held);
        assert_eq!(first.data["heldRequestId"], H1);
        // A new id while one waits holds nothing new and names the one waiting.
        let second = play(&broker, H2);
        assert_eq!(second.outcome, Outcome::Held);
        assert_eq!(second.data["heldRequestId"], H1);
        assert_eq!(stone.held.lock().unwrap().len(), 1, "one hold");
        // The Arisen declines; a retry of the held id still gets `held`.
        assert_eq!(
            broker.answer_play(RUN, H1, false, || unreachable!()),
            Ok(PlayAnswered {
                decision: Decision::Declined,
                persisted: true
            })
        );
        assert_eq!(
            play(&broker, H1),
            first,
            "the held answer is the ledger's forever"
        );
    }

    #[test]
    fn a_decision_is_read_by_list_installed_and_delivered_once_by_a_new_play() {
        let stone = Stone::default();
        let sink = Sink::default();
        let broker = Broker::new(RUN, &stone, &sink, clock);
        play(&broker, H1);
        assert_eq!(
            list(&broker, L1)["play"],
            serde_json::json!({ "waiting": H1, "lastDecision": null })
        );
        broker
            .answer_play(RUN, H1, false, || unreachable!())
            .unwrap();
        let read = serde_json::json!({ "waiting": null, "lastDecision":
            { "heldRequestId": H1, "decision": "declined", "at": "2026-10-28T12:00:00Z" } });
        assert_eq!(list(&broker, L2)["play"], read, "read, never consumed");
        assert_eq!(list(&broker, L2)["play"], read);

        let delivered = play(&broker, H2);
        assert_eq!(delivered.outcome, Outcome::Done);
        assert_eq!(delivered.text, "declined: the Arisen said not now");
        assert_eq!(
            stone.held.lock().unwrap().len(),
            1,
            "delivery holds nothing new"
        );
        // Delivered in the same write as the answer that carries it.
        let written = sink.0.lock().unwrap().last().unwrap().clone();
        assert!(written.decisions[0].delivered);
        assert_eq!(written.ops.last().unwrap().id, H2);
        assert_eq!(written.ops.last().unwrap().answer, delivered);

        // Asking again is a further new call: it holds.
        let again = play(&broker, H3);
        assert_eq!(again.outcome, Outcome::Held);
        assert_eq!(again.data["heldRequestId"], H3);
        assert_eq!(
            play(&broker, H2),
            delivered,
            "a retry of the delivering id repeats it"
        );
    }

    #[test]
    fn only_a_game_that_started_is_played() {
        let stone = Stone::default();
        let sink = Sink::default();
        let broker = Broker::new(RUN, &stone, &sink, clock);
        play(&broker, H1);
        let refused = broker.answer_play(RUN, H1, true, || {
            Err("refused: close Dragon's Dogma 2 first.".into())
        });
        assert_eq!(
            refused,
            Err(PlayRefusal::Launch(
                "refused: close Dragon's Dogma 2 first.".into()
            ))
        );
        assert!(broker.ledger().decisions.is_empty(), "nothing recorded");
        assert_eq!(
            broker.ledger().held.unwrap().held_request_id,
            H1,
            "still waiting"
        );
        let mut launched = false;
        assert_eq!(
            broker.answer_play(RUN, H1, true, || {
                launched = true;
                Ok(())
            }),
            Ok(PlayAnswered {
                decision: Decision::Played,
                persisted: true
            })
        );
        assert!(launched);
        assert_eq!(
            play(&broker, H2).text,
            "played at 2026-10-28T12:00:00Z: the Arisen started the game"
        );
    }

    #[test]
    fn a_stopped_ledger_stops_the_arisens_play_too() {
        // The Heretic's WA2: the run's ledger stop governs the host's press.
        let stone = Stone::default();
        let sink = Sink::default();
        let broker = Broker::new(RUN, &stone, &sink, clock);
        play(&broker, H1);
        *sink.1.lock().unwrap() = Some(0); // every write from now on fails
        let uninstall = body(serde_json::json!({ "id": ID, "tool": "uninstall",
            "args": { "installId": "aa#Glow" } }));
        assert_eq!(
            broker.handle(&name(ID), &uninstall).unwrap().answer.text,
            LEDGER_DOWN
        );
        let never = || -> Result<(), String> { panic!("nothing may launch") };
        assert_eq!(
            broker.answer_play(RUN, H1, true, never),
            Err(PlayRefusal::LedgerDown)
        );
        assert_eq!(
            broker.answer_play(RUN, H1, false, never),
            Err(PlayRefusal::LedgerDown)
        );
        assert_eq!(
            broker.ledger().held.unwrap().held_request_id,
            H1,
            "still waiting"
        );
    }

    #[test]
    fn a_decision_that_could_not_be_written_says_so() {
        // The Heretic's WA3: the game started, the decision did not reach
        // the disk; the answer says both.
        let stone = Stone::default();
        let sink = Sink::default();
        let broker = Broker::new(RUN, &stone, &sink, clock);
        play(&broker, H1);
        *sink.1.lock().unwrap() = Some(0);
        let mut launched = false;
        let answered = broker.answer_play(RUN, H1, true, || {
            launched = true;
            Ok(())
        });
        assert!(launched, "the game did start");
        assert_eq!(
            answered,
            Ok(PlayAnswered {
                decision: Decision::Played,
                persisted: false
            })
        );
        assert!(!broker.retire().persisted, "the run's ledger is not whole");
    }

    #[test]
    fn a_press_for_another_run_or_request_is_stale_and_launches_nothing() {
        let stone = Stone::default();
        let sink = Sink::default();
        let broker = Broker::new(RUN, &stone, &sink, clock);
        let never = || -> Result<(), String> { panic!("nothing may launch") };
        assert_eq!(
            broker.answer_play(RUN, H1, true, never),
            Err(PlayRefusal::NothingHeld)
        );
        play(&broker, H1);
        assert_eq!(
            broker.answer_play(ID, H1, true, never),
            Err(PlayRefusal::StaleHeld),
            "another run"
        );
        assert_eq!(
            broker.answer_play(RUN, H2, true, never),
            Err(PlayRefusal::StaleHeld),
            "another request"
        );
        // The same run's earlier request, after it was answered and a new one held.
        broker.answer_play(RUN, H1, false, never).unwrap();
        play(&broker, H2); // delivers the decline
        play(&broker, H3); // holds anew
        assert_eq!(
            broker.answer_play(RUN, H1, true, never),
            Err(PlayRefusal::StaleHeld),
            "an earlier request"
        );
        broker.retire();
        assert_eq!(
            broker.answer_play(RUN, H3, true, never),
            Err(PlayRefusal::StaleHeld),
            "a retired run"
        );
        assert!(broker
            .ledger()
            .decisions
            .iter()
            .all(|d| d.decision == Decision::Declined));
    }

    #[test]
    fn the_ledger_file_keeps_twenty_runs_and_replaces_a_growing_one() {
        let mut kept = Vec::new();
        for i in 0..25 {
            merge_ledger(
                &mut kept,
                &RunLedger {
                    run_id: format!("run-{i}"),
                    started_at: clock(),
                    ended_at: None,
                    ops: Vec::new(),
                    held: None,
                    decisions: Vec::new(),
                },
            );
        }
        assert_eq!(kept.len(), KEPT_RUNS);
        assert_eq!(kept[0].run_id, "run-5");
        let mut grown = kept[19].clone();
        grown.ended_at = Some(clock());
        merge_ledger(&mut kept, &grown);
        assert_eq!(kept.len(), KEPT_RUNS);
        assert_eq!(kept[19], grown);
    }

    #[test]
    fn the_answer_is_the_shape_the_relay_reads() {
        let answer = Answer {
            id: ID.into(),
            outcome: Outcome::Held,
            text: "held: waiting for the Arisen".into(),
            data: Value::Null,
        };
        let json: Value = serde_json::from_slice(&answer.to_bytes()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "id": ID, "outcome": "held", "text": "held: waiting for the Arisen", "data": null })
        );
    }
}
