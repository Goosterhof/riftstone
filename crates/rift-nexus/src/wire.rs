//! The wire: GraphQL query text, request bodies, and the raw response shapes as
//! Nexus sends them (field names and types confirmed against the live schema
//! on 2026-10-16 — see the crate docs). Parsing is lenient where the live API
//! showed variance: BigInts arrive as strings, booleans as 0/1, versions may be
//! `""`, `modFile` may be null.

use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::error::NexusError;
use crate::types::{
    BrowsePage, DownloadMirror, EndorseStatus, FileCategory, FileUpdate, Md5Hit, ModCandidate,
    NexusAccount, NexusFile, NexusFileDetail, NexusMod, V1FileList,
};
use crate::{
    BROWSE_PAGE_SIZE, CANDIDATES_PER_FIND, FILE_DESCRIPTION_CHARS, GAME_DOMAIN, GAME_ID,
    MODS_PER_REQUEST,
};

/// Batched mod lookup. Unknown ids are left out of `nodes`; order is not kept.
/// Since v0.2 each node also brings the mod's labels: `category: String!`,
/// `tags: [LegacyTag!]!` (`{ name }`) and `adultContent: Boolean`, so the
/// labels ride the update check's own POST and cost no request of their own.
pub(crate) const MODS_QUERY: &str = "query RiftstoneMods($ids: [CompositeDomainWithIdInput!]!, $count: Int) { \
     legacyModsByDomain(ids: $ids, count: $count) { totalCount nodes { modId name version updatedAt status \
     category tags { name } adultContent } } }";

/// The `ModFile` fields every file query selects.
pub(crate) const FILE_FIELDS: &str =
    "fileId groupId sqid name version category date primary sizeInBytes";

/// MD5 lookup across every game (filtered to Dragon's Dogma 2 after the fact).
pub(crate) const MD5_QUERY: &str =
    "query RiftstoneFileHashes($md5s: [String!]!) { fileHashes(md5s: $md5s) { \
     md5 fileName fileSize gameId modFileId \
     modFile { modId fileId groupId sqid name version category date primary sizeInBytes } } }";

pub(crate) fn mods_body(ids: &[u64]) -> Value {
    let ids: Vec<Value> = ids
        .iter()
        .map(|id| json!({ "gameDomain": GAME_DOMAIN, "modId": id }))
        .collect();
    json!({ "query": MODS_QUERY, "variables": { "ids": ids, "count": MODS_PER_REQUEST } })
}

/// The alias a mod's `modFiles` answer arrives under.
pub(crate) fn mod_files_alias(mod_id: u64) -> String {
    format!("m{mod_id}")
}

/// One POST, several mods: `{ m12: modFiles(modId: 12, gameId: 6234) { … } m34: … }`.
/// The ids are integers formatted by Rust — nothing user-typed reaches the text.
pub(crate) fn mod_files_body(ids: &[u64]) -> Value {
    let mut query = String::from("query RiftstoneModFiles {");
    for id in ids {
        let alias = mod_files_alias(*id);
        let _ = write!(
            query,
            " {alias}: modFiles(modId: {id}, gameId: {GAME_ID}) {{ {FILE_FIELDS} }}"
        );
    }
    query.push_str(" }");
    json!({ "query": query })
}

pub(crate) fn md5_body(md5s: &[String]) -> Value {
    json!({ "query": MD5_QUERY, "variables": { "md5s": md5s } })
}

// ---------------------------------------------------------------------------
// The GraphQL envelope
// ---------------------------------------------------------------------------

/// A GraphQL answer, opened but not yet trusted: `data` (`None` when absent or
/// null) and every error that came with it.
///
/// GraphQL can send both at once: a field that failed comes back null in
/// `data` and is named by an error's `path`, and a nullable list that failed
/// reads exactly like an empty one. So no answer that carries errors is taken
/// at face value. Each query states which errors it can survive, and a lookup
/// that failed never reads as "nothing found":
///
/// - `legacyModsByDomain` survives none ([`GraphQlAnswer::into_data`]).
/// - `fileHashes` survives only errors inside another game's upload
///   ([`read_file_hashes`]).
/// - aliased `modFiles`: an error whose path names one mod's alias leaves that
///   mod unchecked; any other error fails the batch ([`read_mod_files`]).
#[derive(Debug)]
pub(crate) struct GraphQlAnswer {
    data: Option<Value>,
    errors: Vec<GraphQlError>,
}

/// `{ "message": "...", "path": ["m12", 0, "groupId"], "extensions": { "code": "MD5_INVALID" } }`
#[derive(Debug, Deserialize)]
struct GraphQlError {
    #[serde(default)]
    message: Option<String>,
    /// Field names and list indexes from the root to the field that failed.
    /// Kept raw: anything but an array reads as "no path".
    #[serde(default)]
    path: Value,
    #[serde(default)]
    extensions: Value,
}

impl GraphQlError {
    fn describe(&self) -> String {
        let code = self.extensions.get("code").and_then(Value::as_str);
        let message = self
            .message
            .as_deref()
            .map(str::trim)
            .filter(|message| !message.is_empty())
            .unwrap_or("an unnamed GraphQL error");
        match code {
            Some(code) => format!("{message} [{code}]"),
            None => message.to_string(),
        }
    }

    fn path(&self) -> &[Value] {
        self.path.as_array().map_or(&[], Vec::as_slice)
    }
}

impl GraphQlAnswer {
    /// Open an answer body. Anything but a JSON object is not GraphQL.
    pub(crate) fn parse(body: &[u8]) -> Result<Self, NexusError> {
        #[derive(Deserialize)]
        struct Raw {
            data: Option<Value>,
            errors: Option<Vec<GraphQlError>>,
        }
        let raw: Raw = serde_json::from_slice(body).map_err(|err| {
            NexusError::BadResponse(format!("the answer is not GraphQL JSON: {err}"))
        })?;
        Ok(GraphQlAnswer {
            data: raw.data.filter(|data| !data.is_null()),
            errors: raw.errors.unwrap_or_default(),
        })
    }

    /// `data` as `T`, trusted only when the answer carried no error at all.
    pub(crate) fn into_data<T: DeserializeOwned>(self) -> Result<T, NexusError> {
        if let Some(error) = self.errors.first() {
            return Err(self.failure(error));
        }
        decode_data(self.data)
    }

    /// The `BadResponse` for an answer that `error` (one of its own) sinks.
    fn failure(&self, error: &GraphQlError) -> NexusError {
        let more = self.errors.len().saturating_sub(1);
        NexusError::BadResponse(match more {
            0 => error.describe(),
            1 => format!("{} (and 1 more error)", error.describe()),
            _ => format!("{} (and {more} more errors)", error.describe()),
        })
    }
}

fn decode_data<T: DeserializeOwned>(data: Option<Value>) -> Result<T, NexusError> {
    let data =
        data.ok_or_else(|| NexusError::BadResponse("the answer carried no data".to_string()))?;
    serde_json::from_value(data).map_err(|err| {
        NexusError::BadResponse(format!("the answer had an unexpected shape: {err}"))
    })
}

/// The first GraphQL error message in a body, if it is a GraphQL error body.
pub(crate) fn first_error_message(body: &[u8]) -> Option<String> {
    let answer = GraphQlAnswer::parse(body).ok()?;
    answer.errors.first().map(GraphQlError::describe)
}

// ---------------------------------------------------------------------------
// legacyModsByDomain
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModsData {
    pub legacy_mods_by_domain: ModPage,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModPage {
    pub total_count: Option<u64>,
    #[serde(default)]
    pub nodes: Vec<WireMod>,
}

/// One `Mod` node. The label fields are kept raw and read leniently: they
/// decorate the update check, so a label in a shape this build does not
/// expect reads as no label instead of failing the check it rides on. (A
/// label whose resolver FAILED comes with a GraphQL error, and any error
/// refuses the whole page first — see [`GraphQlAnswer::into_data`]: an
/// `adultContent` that could not be read never passes for "not adult".)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WireMod {
    mod_id: Option<Value>,
    name: Option<String>,
    version: Option<String>,
    updated_at: Option<String>,
    status: Option<String>,
    category: Option<Value>,
    tags: Option<Value>,
    adult_content: Option<Value>,
}

impl WireMod {
    pub(crate) fn into_mod(self) -> Option<NexusMod> {
        let mod_id = self.mod_id.as_ref().and_then(lenient_u64)?;
        Some(NexusMod {
            mod_id,
            name: self.name.unwrap_or_default().trim().to_string(),
            version: non_empty(self.version),
            updated_ms: self.updated_at.as_deref().and_then(rfc3339_ms),
            status: self.status.unwrap_or_default().trim().to_string(),
            category: category_name(mod_id, self.category.as_ref()),
            tags: tag_names(mod_id, self.tags.as_ref()),
            adult: self.adult_content.as_ref().is_some_and(lenient_bool),
        })
    }
}

/// `category: String!`, trimmed. Absent, null, blank or not a string: none.
fn category_name(mod_id: u64, value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(name) => Some(name.trim().to_string()).filter(|name| !name.is_empty()),
        Value::Null => None,
        other => {
            log::debug!("mod {mod_id}: ignoring a category that is not a string: {other}");
            None
        }
    }
}

/// `tags: [LegacyTag!]!` (`[{ "name": "Utilities for Players" }, …]`) as
/// names, in Nexus's order and spelling, trimmed. A blank name is dropped, and
/// so is a repeat of a name already kept, compared case-insensitively, as the
/// window's filters compare. A list of plain strings is read the same way,
/// should Nexus ever flatten `LegacyTag`. Anything that is not a list reads as
/// no tags.
fn tag_names(mod_id: u64, value: Option<&Value>) -> Vec<String> {
    let tags = match value {
        None | Some(Value::Null) => return Vec::new(),
        Some(Value::Array(tags)) => tags,
        Some(other) => {
            log::debug!("mod {mod_id}: ignoring tags that are not a list: {other}");
            return Vec::new();
        }
    };
    let mut seen = HashSet::new();
    tags.iter()
        .filter_map(|tag| match tag {
            Value::Object(fields) => fields.get("name").and_then(Value::as_str),
            Value::String(name) => Some(name.as_str()),
            _ => None,
        })
        .map(str::trim)
        .filter(|name| !name.is_empty() && seen.insert(name.to_lowercase()))
        .map(str::to_string)
        .collect()
}

// ---------------------------------------------------------------------------
// mods: the name search (v0.2, Find on Nexus)
// ---------------------------------------------------------------------------

/// How `ModsFilter.nameStemmed` compares the terms with a mod's name
/// (measured on the anonymous API, 2026-10-16).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NameMatch {
    /// Every term, in any order; stems match (`cloak` finds `Cloaks`).
    Matches,
    /// Every term, each with a leading and a trailing wildcard.
    Wildcard,
}

impl NameMatch {
    /// The `op` of the filter value, a GraphQL enum literal.
    pub(crate) fn op(self) -> &'static str {
        match self {
            NameMatch::Matches => "MATCHES",
            NameMatch::Wildcard => "WILDCARD",
        }
    }
}

/// The `Mod` fields a find selects.
pub(crate) const FIND_FIELDS: &str = "modId name version author uploader { name } category \
     modCategory { name } adultContent downloads endorsements summary updatedAt game { domainName }";

/// The find's query text: Dragon's Dogma 2 mods whose name holds the terms,
/// most relevant first, [`CANDIDATES_PER_FIND`] of them. It depends on the
/// match mode alone. The terms travel ONLY as the `$terms` variable, so
/// nothing a file or folder name holds ever becomes query text.
pub(crate) fn find_query(mode: NameMatch) -> String {
    format!(
        "query RiftstoneFindMods($terms: String!) {{ mods(filter: {{ \
         gameDomainName: [{{ value: \"{GAME_DOMAIN}\", op: EQUALS }}], \
         nameStemmed: [{{ value: $terms, op: {op} }}] }}, \
         sort: [{{ relevance: {{ direction: DESC }} }}], count: {CANDIDATES_PER_FIND}) \
         {{ totalCount nodes {{ {FIND_FIELDS} }} }} }}",
        op = mode.op()
    )
}

pub(crate) fn find_body(terms: &str, mode: NameMatch) -> Value {
    json!({ "query": find_query(mode), "variables": { "terms": terms } })
}

/// The answer of a find or a browse: one `mods` page.
#[derive(Debug, Deserialize)]
pub(crate) struct FindData {
    mods: FindPage,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FindPage {
    /// Every match Nexus counts, across pages. Kept raw, read leniently.
    #[serde(default)]
    total_count: Option<Value>,
    /// Kept raw: one node in a shape this build does not expect is skipped,
    /// it does not sink the others.
    #[serde(default)]
    nodes: Vec<Value>,
}

/// The candidates of one `mods` page, in Nexus's order: a node without a
/// usable mod id or a name, or from another game, is skipped; a repeated mod
/// id counts once.
fn page_candidates(nodes: Vec<Value>) -> impl Iterator<Item = ModCandidate> {
    let mut seen = HashSet::new();
    nodes
        .into_iter()
        .filter_map(|node| match serde_json::from_value::<WireCandidate>(node) {
            Ok(wire) => wire.into_candidate(),
            Err(err) => {
                log::debug!("skipping a mods result in an unexpected shape: {err}");
                None
            }
        })
        .filter(move |candidate| seen.insert(candidate.mod_id))
}

/// The candidates of one find answer, in Nexus's order (relevance), at most
/// [`CANDIDATES_PER_FIND`] (see [`page_candidates`] for what is skipped).
pub(crate) fn read_candidates(data: FindData) -> Vec<ModCandidate> {
    page_candidates(data.mods.nodes)
        .take(CANDIDATES_PER_FIND)
        .collect()
}

// ---------------------------------------------------------------------------
// mods: browsing (v0.3)
// ---------------------------------------------------------------------------

/// The longest browse text that goes to Nexus, in characters.
pub(crate) const BROWSE_TEXT_CHARS: usize = 200;
/// The furthest a page may start: `offset` is a GraphQL `Int` (32-bit), and
/// the page's own mods must fit after it.
pub(crate) const LARGEST_BROWSE_OFFSET: u64 = i32::MAX as u64 - BROWSE_PAGE_SIZE as u64;

/// The browse text as it goes out: whitespace collapsed, at most
/// [`BROWSE_TEXT_CHARS`] characters (cut on a character boundary).
pub(crate) fn browse_text(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    collapsed
        .chars()
        .take(BROWSE_TEXT_CHARS)
        .collect::<String>()
        .trim_end()
        .to_string()
}

/// The browse query text: Dragon's Dogma 2's mods, `sort` descending, one
/// page of [`BROWSE_PAGE_SIZE`] from `offset`. With text, the filter also
/// wants every word in the mod's name (`nameStemmed MATCHES`), and the words
/// travel ONLY as the `$text` variable; without text the query declares no
/// variable at all (GraphQL refuses an unused one). The filters combine as
/// the filter object's default AND: never a top-level `op: OR`, which would
/// let mods of other games in. `sort_key` and the integers are this crate's
/// own words, never anything typed.
pub(crate) fn browse_query(sort_key: &str, with_text: bool, offset: u64) -> String {
    let (variables, name_filter) = if with_text {
        (
            "($text: String!)",
            ", nameStemmed: [{ value: $text, op: MATCHES }]",
        )
    } else {
        ("", "")
    };
    format!(
        "query RiftstoneBrowseMods{variables} {{ mods(filter: {{ \
         gameDomainName: [{{ value: \"{GAME_DOMAIN}\", op: EQUALS }}]{name_filter} }}, \
         sort: [{{ {sort_key}: {{ direction: DESC }} }}], offset: {offset}, \
         count: {BROWSE_PAGE_SIZE}) {{ totalCount nodes {{ {FIND_FIELDS} }} }} }}"
    )
}

/// The browse POST body: the text (already [`browse_text`]-ed) as the only
/// variable, or no variables at all.
pub(crate) fn browse_body(text: &str, sort_key: &str, offset: u64) -> Value {
    let query = browse_query(sort_key, !text.is_empty(), offset);
    if text.is_empty() {
        json!({ "query": query })
    } else {
        json!({ "query": query, "variables": { "text": text } })
    }
}

/// One browse answer as a page: `total` from `totalCount` (absent: what this
/// page shows, so the window offers no further page), every usable node.
pub(crate) fn read_browse_page(data: FindData, offset: u64) -> BrowsePage {
    let mods: Vec<ModCandidate> = page_candidates(data.mods.nodes).collect();
    let total = data
        .mods
        .total_count
        .as_ref()
        .and_then(lenient_u64)
        .unwrap_or(offset + mods.len() as u64);
    BrowsePage {
        total,
        offset,
        mods,
    }
}

/// The ids a link can be made from: Nexus's GraphQL `Int` is 32-bit.
const LINKABLE_MOD_IDS: std::ops::RangeInclusive<u64> = 1..=(i32::MAX as u64);
/// The longest summary a candidate keeps, in characters.
const SUMMARY_CHARS: usize = 200;

/// One node of a find answer. Every field is kept raw and read leniently.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WireCandidate {
    mod_id: Option<Value>,
    name: Option<Value>,
    version: Option<Value>,
    author: Option<Value>,
    uploader: Option<Value>,
    category: Option<Value>,
    mod_category: Option<Value>,
    adult_content: Option<Value>,
    downloads: Option<Value>,
    endorsements: Option<Value>,
    summary: Option<Value>,
    updated_at: Option<Value>,
    game: Option<Value>,
}

impl WireCandidate {
    /// The candidate, or `None` for a node without a mod id a link can be
    /// made from, without a name, or naming another game. (The query asks
    /// for Dragon's Dogma 2 only; a node that says nothing of its game is
    /// taken at the filter's word.)
    pub(crate) fn into_candidate(self) -> Option<ModCandidate> {
        let mod_id = self
            .mod_id
            .as_ref()
            .and_then(lenient_u64)
            .filter(|id| LINKABLE_MOD_IDS.contains(id))?;
        let name = text(self.name.as_ref())?;
        let game = self
            .game
            .as_ref()
            .and_then(|game| text(game.get("domainName")));
        if let Some(domain) = game.filter(|domain| !domain.eq_ignore_ascii_case(GAME_DOMAIN)) {
            log::debug!("skipping mod {mod_id} of another game ({domain})");
            return None;
        }
        let named =
            |field: &Option<Value>| field.as_ref().and_then(|value| text(value.get("name")));
        Some(ModCandidate {
            mod_id,
            name,
            author: text(self.author.as_ref()).or_else(|| named(&self.uploader)),
            version: text(self.version.as_ref()),
            category: text(self.category.as_ref()).or_else(|| named(&self.mod_category)),
            adult: self.adult_content.as_ref().is_some_and(lenient_bool),
            downloads: self.downloads.as_ref().and_then(lenient_u64),
            endorsements: self.endorsements.as_ref().and_then(lenient_u64),
            summary: self.summary.as_ref().and_then(summary_text),
            updated_ms: self.updated_at.as_ref().and_then(time_ms_signed),
        })
    }
}

/// A string (or a number, written out), trimmed; blank is none.
fn text(value: Option<&Value>) -> Option<String> {
    let text = match value? {
        Value::String(text) => text.trim().to_string(),
        Value::Number(number) => number.to_string(),
        _ => return None,
    };
    (!text.is_empty()).then_some(text)
}

/// Whitespace collapsed to single spaces, cut at [`SUMMARY_CHARS`]
/// characters (never inside one); blank is none.
fn summary_text(value: &Value) -> Option<String> {
    collapsed_text(value, SUMMARY_CHARS)
}

/// A string with its whitespace collapsed to single spaces, cut at
/// `max_chars` characters (never inside one); blank (or not a string) is none.
fn collapsed_text(value: &Value, max_chars: usize) -> Option<String> {
    let collapsed = value
        .as_str()?
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let cut: String = collapsed.chars().take(max_chars).collect();
    let cut = cut.trim_end();
    (!cut.is_empty()).then(|| cut.to_string())
}

/// `updatedAt` → ms since the epoch: RFC 3339 (`2024-05-04T01:30:45Z`), an
/// ISO-8601 time without an offset (read as UTC), or unix seconds.
/// Anything else is none.
fn time_ms_signed(value: &Value) -> Option<i64> {
    match value {
        Value::String(text) => {
            let text = text.trim();
            if let Ok(at) = chrono::DateTime::parse_from_rfc3339(text) {
                return Some(at.timestamp_millis());
            }
            chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.f")
                .ok()
                .map(|at| at.and_utc().timestamp_millis())
        }
        Value::Number(number) => number.as_i64().and_then(|secs| secs.checked_mul(1000)),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// modFiles (aliased)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WireModFile {
    mod_id: Option<Value>,
    file_id: Option<Value>,
    group_id: Option<Value>,
    sqid: Option<String>,
    name: Option<String>,
    version: Option<String>,
    category: Option<String>,
    date: Option<Value>,
    primary: Option<Value>,
    size_in_bytes: Option<Value>,
    /// v0.3, [`mod_file_details_body`] only: kept raw, read leniently.
    #[serde(default)]
    description: Option<Value>,
    /// v0.3, [`mod_file_details_body`] only: `[String!]!`, kept raw.
    #[serde(default)]
    changelog_text: Option<Value>,
}

impl WireModFile {
    /// The file with its description and patch notes (v0.3), or `None`
    /// without a file id.
    pub(crate) fn into_detail(mut self) -> Option<NexusFileDetail> {
        let description = self
            .description
            .take()
            .and_then(|value| collapsed_text(&value, FILE_DESCRIPTION_CHARS));
        let changelog = changelog_lines(self.changelog_text.take());
        Some(NexusFileDetail {
            file: self.into_file()?,
            description,
            changelog,
        })
    }

    pub(crate) fn into_file(self) -> Option<NexusFile> {
        let file_id = self.file_id.as_ref().and_then(lenient_u64)?;
        Some(NexusFile {
            file_id,
            group_id: self.group_id.as_ref().and_then(lenient_u64),
            sqid: non_empty(self.sqid),
            name: self.name.unwrap_or_default().trim().to_string(),
            version: non_empty(self.version),
            category: self
                .category
                .as_deref()
                .map_or(FileCategory::Unknown, FileCategory::from_nexus),
            uploaded_ms: self.date.as_ref().and_then(lenient_time_ms).unwrap_or(0),
            primary: self.primary.as_ref().is_some_and(lenient_bool),
            size_bytes: self.size_in_bytes.as_ref().and_then(lenient_u64),
        })
    }
}

/// One aliased `modFiles` POST, read.
#[derive(Debug, Default)]
pub(crate) struct ModFilesRound {
    /// Every file of each mod that answered cleanly, oldest first. A mod
    /// Nexus does not know answers `[]`.
    pub files: BTreeMap<u64, Vec<NexusFile>>,
    /// Mods left unchecked this time, with the technical reason: an error
    /// named the mod's alias, or the answer had no usable list for it.
    pub refused: BTreeMap<u64, String>,
    /// Mods the answer said nothing about, because a refused mod's error
    /// nulled the whole `data` (GraphQL null propagation: `modFiles` returns a
    /// non-null list). They were not at fault; ask for them again. Always
    /// smaller than the batch that was asked, so asking again ends.
    pub unanswered: Vec<u64>,
}

/// Read one aliased `modFiles` answer for the mods in `asked`.
///
/// An error whose path starts at an alias that was asked (`["m12", 0, "groupId"]`)
/// leaves THAT mod unchecked, even when some of its data arrived. Any other
/// error (no path, a path naming nothing that was asked) fails the whole
/// batch, as does a body that is not GraphQL: those are failures of the
/// service, not of one mod, so asking mod by mod would only repeat them.
pub(crate) fn read_mod_files(
    answer: GraphQlAnswer,
    asked: &[u64],
) -> Result<ModFilesRound, NexusError> {
    let mut round = ModFilesRound::default();
    for error in &answer.errors {
        let alias = match error.path() {
            [Value::String(alias), ..] => Some(alias.as_str()),
            _ => None,
        };
        let Some(&mod_id) = alias.and_then(|alias| {
            asked
                .iter()
                .find(|&&mod_id| mod_files_alias(mod_id) == alias)
        }) else {
            return Err(answer.failure(error));
        };
        round
            .refused
            .entry(mod_id)
            .or_insert_with(|| error.describe());
    }

    let Some(data) = answer.data else {
        if round.refused.is_empty() {
            return Err(NexusError::BadResponse(
                "the answer carried no data".to_string(),
            ));
        }
        round.unanswered = asked
            .iter()
            .copied()
            .filter(|mod_id| !round.refused.contains_key(mod_id))
            .collect();
        return Ok(round);
    };
    let Value::Object(mut lists) = data else {
        return Err(NexusError::BadResponse(
            "the answer had an unexpected shape: the file lists are not keyed by mod".to_string(),
        ));
    };
    for &mod_id in asked {
        if round.refused.contains_key(&mod_id) {
            continue;
        }
        let list = lists
            .remove(&mod_files_alias(mod_id))
            .unwrap_or(Value::Null);
        match serde_json::from_value::<Option<Vec<WireModFile>>>(list) {
            Ok(Some(list)) => {
                let mut files: Vec<NexusFile> = list
                    .into_iter()
                    .filter_map(WireModFile::into_file)
                    .collect();
                files.sort_by_key(|file| (file.uploaded_ms, file.file_id));
                round.files.insert(mod_id, files);
            }
            Ok(None) => {
                round.refused.insert(
                    mod_id,
                    "the answer held no file list for this mod".to_string(),
                );
            }
            Err(err) => {
                round.refused.insert(
                    mod_id,
                    format!("the file list had an unexpected shape: {err}"),
                );
            }
        }
    }
    for alias in lists.keys() {
        log::debug!("ignoring unexpected modFiles alias {alias:?}");
    }
    Ok(round)
}

// ---------------------------------------------------------------------------
// modFiles with what the player reads (v0.3)
// ---------------------------------------------------------------------------

/// The `ModFile` fields a mod's file list for reading selects: the update
/// rules' facts plus the description and the patch notes.
pub(crate) const FILE_DETAIL_FIELDS: &str =
    "fileId groupId sqid name version category date primary sizeInBytes description changelogText";

/// One mod's files with their descriptions and patch notes. The id is an
/// integer formatted by Rust — nothing typed reaches the text.
pub(crate) fn mod_file_details_body(mod_id: u64) -> Value {
    json!({ "query": format!(
        "query RiftstoneModFileDetails {{ modFiles(modId: {mod_id}, gameId: {GAME_ID}) \
         {{ {FILE_DETAIL_FIELDS} }} }}"
    ) })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModFileDetailsData {
    /// Kept raw: a node in a shape this build does not expect is skipped.
    mod_files: Option<Vec<Value>>,
}

/// One mod's files for reading, oldest first. A node without a file id (or
/// in a shape this build does not expect) is skipped; a null list is a
/// refusal (`modFiles` is a non-null list, so null means it failed), never
/// "no files". An unknown mod answers `[]`: no files.
pub(crate) fn read_file_details(
    data: ModFileDetailsData,
) -> Result<Vec<NexusFileDetail>, NexusError> {
    let nodes = data.mod_files.ok_or_else(|| {
        NexusError::BadResponse("the answer held no file list for this mod".to_string())
    })?;
    let mut details: Vec<NexusFileDetail> = nodes
        .into_iter()
        .filter_map(|node| match serde_json::from_value::<WireModFile>(node) {
            Ok(wire) => wire.into_detail(),
            Err(err) => {
                log::debug!("skipping a mod file in an unexpected shape: {err}");
                None
            }
        })
        .collect();
    details.sort_by_key(|detail| (detail.file.uploaded_ms, detail.file.file_id));
    Ok(details)
}

/// `changelogText: [String!]!` → its lines, each trimmed, blank ones
/// dropped. A single string is read as one entry per line; anything else
/// is no notes.
fn changelog_lines(value: Option<Value>) -> Vec<String> {
    let entries: Vec<String> = match value {
        Some(Value::Array(entries)) => entries
            .into_iter()
            .filter_map(|entry| match entry {
                Value::String(text) => Some(text),
                _ => None,
            })
            .collect(),
        Some(Value::String(text)) => text.lines().map(str::to_string).collect(),
        _ => Vec::new(),
    };
    entries
        .iter()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

// ---------------------------------------------------------------------------
// fileHashes
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileHashesData {
    file_hashes: Option<Vec<WireFileHash>>,
}

/// The raw hits of one `fileHashes` answer.
///
/// One hash can match hundreds of uploads across every game on Nexus, so an
/// error is survivable only when its path points INSIDE a hit whose `gameId`
/// is known and is not Dragon's Dogma 2 (`["fileHashes", 3, "modFile"]`, hit 3
/// from another game): a failure there says nothing about this game. Any other
/// error (on the list itself, on a Dragon's Dogma 2 hit, on a hit whose game is
/// unknown, or with no path) fails the lookup. A lookup that failed must never
/// read as "no Nexus file has this MD5". A null list with no error at all is
/// read as no hits.
pub(crate) fn read_file_hashes(answer: GraphQlAnswer) -> Result<Vec<WireFileHash>, NexusError> {
    let hits = answer.data.as_ref().and_then(|data| data.get("fileHashes"));
    if let Some(error) = answer
        .errors
        .iter()
        .find(|error| !inside_another_games_hit(error, hits))
    {
        return Err(answer.failure(error));
    }
    for error in &answer.errors {
        log::debug!(
            "ignoring an error about another game's upload: {}",
            error.describe()
        );
    }
    let data: FileHashesData = decode_data(answer.data)?;
    Ok(data.file_hashes.unwrap_or_default())
}

/// Does this error's path point inside a hit for a game other than Dragon's
/// Dogma 2?
fn inside_another_games_hit(error: &GraphQlError, hits: Option<&Value>) -> bool {
    let [Value::String(field), index, _, ..] = error.path() else {
        return false;
    };
    let Some(index) = index.as_u64().and_then(|index| usize::try_from(index).ok()) else {
        return false;
    };
    field == "fileHashes"
        && hits
            .and_then(|hits| hits.get(index))
            .and_then(|hit| hit.get("gameId"))
            .and_then(lenient_u64)
            .is_some_and(|game_id| game_id != GAME_ID)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WireFileHash {
    md5: Option<String>,
    file_name: Option<String>,
    file_size: Option<Value>,
    game_id: Option<Value>,
    mod_file_id: Option<Value>,
    mod_file: Option<WireModFile>,
}

impl WireFileHash {
    /// A Dragon's Dogma 2 hit, or `None` (another game, or no mod file behind
    /// the hash — without `modFile` there is no mod id to link to).
    pub(crate) fn into_hit(self) -> Option<Md5Hit> {
        if self.game_id.as_ref().and_then(lenient_u64) != Some(GAME_ID) {
            return None;
        }
        let md5 = self.md5?.trim().to_ascii_lowercase();
        let mut mod_file = self.mod_file?;
        let mod_id = mod_file.mod_id.as_ref().and_then(lenient_u64)?;
        if mod_file.file_id.is_none() {
            mod_file.file_id = self.mod_file_id;
        }
        let file = mod_file.into_file()?;
        Some(Md5Hit {
            md5,
            mod_id,
            file_id: file.file_id,
            version: file.version,
            name: file.name,
            size_bytes: self
                .file_size
                .as_ref()
                .and_then(lenient_u64)
                .or(file.size_bytes),
            group_id: file.group_id,
            sqid: file.sqid,
            uploaded_ms: Some(file.uploaded_ms).filter(|ms| *ms > 0),
            category: file.category,
            file_name: self.file_name.unwrap_or_default(),
        })
    }
}

// ---------------------------------------------------------------------------
// v1 REST
// ---------------------------------------------------------------------------

/// `GET /v1/users/validate.json`. The answer echoes the key back and names
/// the account's e-mail; neither is read into memory beyond serde's skip.
/// The flags are read leniently (a boolean, 0/1 or `"true"`), and each has
/// an older spelling with a `?` that still counts.
#[derive(Debug, Deserialize)]
pub(crate) struct V1Validate {
    user_id: Option<Value>,
    name: Option<String>,
    is_premium: Option<Value>,
    #[serde(rename = "is_premium?")]
    is_premium_legacy: Option<Value>,
    is_supporter: Option<Value>,
    #[serde(rename = "is_supporter?")]
    is_supporter_legacy: Option<Value>,
}

impl V1Validate {
    pub(crate) fn into_account(self) -> Option<NexusAccount> {
        let name = non_empty(self.name)?;
        let flag = |value: Option<Value>, legacy: Option<Value>| {
            value
                .filter(|value| !value.is_null())
                .or(legacy)
                .as_ref()
                .is_some_and(lenient_bool)
        };
        Some(NexusAccount {
            name,
            user_id: self
                .user_id
                .as_ref()
                .and_then(lenient_u64)
                .filter(|id| *id > 0),
            is_premium: flag(self.is_premium, self.is_premium_legacy),
            is_supporter: flag(self.is_supporter, self.is_supporter_legacy),
        })
    }
}

/// `GET /v1/games/{domain}/mods/{id}/files.json`.
#[derive(Debug, Deserialize)]
pub(crate) struct V1Files {
    #[serde(default)]
    files: Vec<V1File>,
    #[serde(default)]
    file_updates: Vec<V1FileUpdate>,
}

#[derive(Debug, Deserialize)]
struct V1File {
    file_id: Option<Value>,
    /// v0.3: the archive's name as uploaded (`Lantern Glow-2207-1-2-1700000000.zip`).
    file_name: Option<String>,
    name: Option<String>,
    version: Option<String>,
    category_id: Option<Value>,
    category_name: Option<String>,
    is_primary: Option<Value>,
    uploaded_timestamp: Option<Value>,
    size_in_bytes: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct V1FileUpdate {
    old_file_id: Option<Value>,
    new_file_id: Option<Value>,
    uploaded_timestamp: Option<Value>,
}

impl V1Files {
    pub(crate) fn into_list(self) -> V1FileList {
        let mut file_names = BTreeMap::new();
        let mut files: Vec<NexusFile> = self
            .files
            .into_iter()
            .filter_map(|file| {
                let category = match file.category_name.as_deref().map(FileCategory::from_nexus) {
                    Some(category) if category != FileCategory::Unknown => category,
                    _ => file
                        .category_id
                        .as_ref()
                        .and_then(lenient_u64)
                        .map_or(FileCategory::Unknown, FileCategory::from_v1_id),
                };
                let file_id = file.file_id.as_ref().and_then(lenient_u64)?;
                if let Some(archive) = non_empty(file.file_name) {
                    file_names.insert(file_id, archive);
                }
                Some(NexusFile {
                    file_id,
                    group_id: None,
                    sqid: None,
                    name: file.name.unwrap_or_default().trim().to_string(),
                    version: non_empty(file.version),
                    category,
                    uploaded_ms: file
                        .uploaded_timestamp
                        .as_ref()
                        .and_then(lenient_time_ms)
                        .unwrap_or(0),
                    primary: file.is_primary.as_ref().is_some_and(lenient_bool),
                    size_bytes: file.size_in_bytes.as_ref().and_then(lenient_u64),
                })
            })
            .collect();
        files.sort_by_key(|file| (file.uploaded_ms, file.file_id));
        let updates = self
            .file_updates
            .into_iter()
            .filter_map(|update| {
                Some(FileUpdate {
                    old_file_id: update.old_file_id.as_ref().and_then(lenient_u64)?,
                    new_file_id: update.new_file_id.as_ref().and_then(lenient_u64)?,
                    uploaded_ms: update.uploaded_timestamp.as_ref().and_then(lenient_time_ms),
                })
            })
            .collect();
        V1FileList {
            files,
            updates,
            file_names,
        }
    }
}

// ---------------------------------------------------------------------------
// v1 REST, v0.3: download links, tracked mods, endorsements
// ---------------------------------------------------------------------------

/// A v1 answer body as JSON, or the refusal that says it was not.
fn v1_json(body: &[u8], what: &str) -> Result<Value, NexusError> {
    serde_json::from_slice(body)
        .map_err(|err| NexusError::BadResponse(format!("the {what} answer is not JSON: {err}")))
}

/// A v1 answer that must be a list.
fn v1_list(body: &[u8], what: &str) -> Result<Vec<Value>, NexusError> {
    match v1_json(body, what)? {
        Value::Array(items) => Ok(items),
        _ => Err(NexusError::BadResponse(format!(
            "the {what} answer is not a list"
        ))),
    }
}

/// `download_link.json`: `[{ "name", "short_name", "URI" }]`, Nexus's
/// preferred mirror first. A mirror without a link, or whose link is not
/// `https` (`http` only towards a loopback test server), is dropped; the
/// links are never logged. None left is a refusal: there is nothing to
/// download from.
pub(crate) fn read_download_links(body: &[u8]) -> Result<Vec<DownloadMirror>, NexusError> {
    #[derive(Deserialize)]
    struct WireMirror {
        #[serde(rename = "URI", alias = "uri")]
        uri: Option<String>,
        name: Option<String>,
        short_name: Option<String>,
    }
    let offered = v1_list(body, "download link")?;
    let count = offered.len();
    let mirrors: Vec<DownloadMirror> = offered
        .into_iter()
        .filter_map(|item| serde_json::from_value::<WireMirror>(item).ok())
        .filter_map(|mirror| {
            let uri = mirror.uri.map(|uri| uri.trim().to_string())?;
            crate::download::usable_link(&uri)?;
            let name = non_empty(mirror.name).unwrap_or_else(|| "Nexus Mods".to_string());
            Some(DownloadMirror {
                short_name: non_empty(mirror.short_name).unwrap_or_else(|| name.clone()),
                name,
                uri,
            })
        })
        .collect();
    if mirrors.len() < count {
        log::warn!(
            "dropped {} of {count} download mirrors: no https link",
            count - mirrors.len()
        );
    }
    if mirrors.is_empty() {
        return Err(NexusError::BadResponse(
            "Nexus offered no https download link for this file".to_string(),
        ));
    }
    Ok(mirrors)
}

/// The mod id of one v1 list entry, when it is a Dragon's Dogma 2 mod a
/// link can be made from (`domain_name` compared case-insensitively).
fn dd2_mod_id(item: &Value) -> Option<u64> {
    let domain = item.get("domain_name").and_then(Value::as_str)?;
    if !domain.trim().eq_ignore_ascii_case(GAME_DOMAIN) {
        return None;
    }
    item.get("mod_id")
        .and_then(lenient_u64)
        .filter(|id| LINKABLE_MOD_IDS.contains(id))
}

/// `user/tracked_mods.json`: `[{ "mod_id", "domain_name" }]` → the Dragon's
/// Dogma 2 mod ids, in Nexus's order, each once.
pub(crate) fn read_tracked_mods(body: &[u8]) -> Result<Vec<u64>, NexusError> {
    let mut seen = HashSet::new();
    Ok(v1_list(body, "tracked mods")?
        .iter()
        .filter_map(dd2_mod_id)
        .filter(|id| seen.insert(*id))
        .collect())
}

/// `user/endorsements.json`: `[{ "mod_id", "domain_name", "date", "version",
/// "status" }]` → each Dragon's Dogma 2 mod this account decided on, in
/// Nexus's order. `Undecided` (and any status this build does not know) is
/// no entry. A mod listed twice keeps its latest decision: the later `date`,
/// else the later entry.
pub(crate) fn read_endorsements(body: &[u8]) -> Result<Vec<(u64, EndorseStatus)>, NexusError> {
    let mut decided: Vec<(u64, EndorseStatus, u64)> = Vec::new();
    for item in v1_list(body, "endorsements")? {
        let Some(mod_id) = dd2_mod_id(&item) else {
            continue;
        };
        let Some(status) = item
            .get("status")
            .and_then(Value::as_str)
            .and_then(EndorseStatus::from_nexus)
        else {
            continue;
        };
        let date = item.get("date").and_then(lenient_time_ms).unwrap_or(0);
        match decided.iter_mut().find(|(id, _, _)| *id == mod_id) {
            Some(entry) if date >= entry.2 => *entry = (mod_id, status, date),
            Some(_) => {}
            None => decided.push((mod_id, status, date)),
        }
    }
    Ok(decided
        .into_iter()
        .map(|(mod_id, status, _)| (mod_id, status))
        .collect())
}

/// An `application/x-www-form-urlencoded` body: letters, digits and `*-._`
/// as they are, a space as `+`, every other byte as `%XX`.
pub(crate) fn form_body(pairs: &[(&str, &str)]) -> String {
    fn encode(text: &str, out: &mut String) {
        for byte in text.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' => {
                    out.push(char::from(byte));
                }
                b' ' => out.push('+'),
                _ => {
                    let _ = write!(out, "%{byte:02X}");
                }
            }
        }
    }
    let mut body = String::new();
    for (index, (name, value)) in pairs.iter().enumerate() {
        if index > 0 {
            body.push('&');
        }
        encode(name, &mut body);
        body.push('=');
        encode(value, &mut body);
    }
    body
}

// ---------------------------------------------------------------------------
// Lenient scalars
// ---------------------------------------------------------------------------

/// A non-negative integer sent as a number or as a numeric string (BigInt).
pub(crate) fn lenient_u64(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

/// `primary: Int!` (0/1) — also accepts a real boolean or "true"/"1".
pub(crate) fn lenient_bool(value: &Value) -> bool {
    match value {
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64().is_some_and(|n| n != 0.0),
        Value::String(text) => matches!(
            text.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes"
        ),
        _ => false,
    }
}

/// Unix seconds (number or numeric string) or an RFC 3339 string → ms.
pub(crate) fn lenient_time_ms(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => number.as_u64().and_then(|secs| secs.checked_mul(1000)),
        Value::String(text) => match text.trim().parse::<u64>() {
            Ok(secs) => secs.checked_mul(1000),
            Err(_) => rfc3339_ms(text),
        },
        _ => None,
    }
}

/// `2026-10-14T14:23:50Z` → ms since the epoch.
pub(crate) fn rfc3339_ms(text: &str) -> Option<u64> {
    chrono::DateTime::parse_from_rfc3339(text.trim())
        .ok()
        .and_then(|at| u64::try_from(at.timestamp_millis()).ok())
}

fn non_empty(text: Option<String>) -> Option<String> {
    text.map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mod_files_query_aliases_every_mod() {
        let body = mod_files_body(&[12, 3400]);
        let query = body["query"].as_str().unwrap();
        assert!(query.contains("m12: modFiles(modId: 12, gameId: 6234) {"));
        assert!(query.contains("m3400: modFiles(modId: 3400, gameId: 6234) {"));
        assert_eq!(query.matches(FILE_FIELDS).count(), 2);
    }

    #[test]
    fn the_mods_body_names_the_game_domain_for_every_id() {
        let body = mods_body(&[7, 8]);
        assert_eq!(
            body["variables"]["ids"],
            json!([{"gameDomain": "dragonsdogma2", "modId": 7}, {"gameDomain": "dragonsdogma2", "modId": 8}])
        );
        assert_eq!(body["variables"]["count"], 200);
        assert!(body["query"]
            .as_str()
            .unwrap()
            .contains("legacyModsByDomain("));
    }

    #[test]
    fn a_mod_file_parses_from_the_live_shape() {
        // Types as the live API sends them: sizeInBytes a string, primary 0/1.
        let wire: WireModFile = serde_json::from_value(json!({
            "fileId": 501, "groupId": 77001, "sqid": "aB3dE5fG7", "name": " Lantern Glow Recolor ",
            "version": "", "category": "OLD_VERSION", "date": 1_700_000_000,
            "primary": 1, "sizeInBytes": "28653782"
        }))
        .unwrap();
        let file = wire.into_file().unwrap();
        assert_eq!(file.file_id, 501);
        assert_eq!(file.group_id, Some(77001));
        assert_eq!(file.name, "Lantern Glow Recolor");
        assert_eq!(file.version, None, "an empty version is no version");
        assert_eq!(file.category, FileCategory::OldVersion);
        assert_eq!(file.uploaded_ms, 1_700_000_000_000);
        assert!(file.primary);
        assert_eq!(file.size_bytes, Some(28_653_782));
    }

    #[test]
    fn a_mod_file_without_an_id_is_dropped_and_nulls_are_tolerated() {
        let wire: WireModFile = serde_json::from_value(json!({ "name": "x" })).unwrap();
        assert!(wire.into_file().is_none());
        let wire: WireModFile = serde_json::from_value(json!({
            "fileId": "9", "groupId": null, "sqid": null, "name": null, "version": null,
            "category": "BRAND_NEW", "date": null, "primary": false, "sizeInBytes": null
        }))
        .unwrap();
        let file = wire.into_file().unwrap();
        assert_eq!(
            (file.file_id, file.group_id, file.uploaded_ms),
            (9, None, 0)
        );
        assert_eq!(file.category, FileCategory::Unknown);
        assert!(!file.primary);
    }

    fn answer(body: &str) -> GraphQlAnswer {
        GraphQlAnswer::parse(body.as_bytes()).unwrap()
    }

    fn bad_response(result: Result<impl std::fmt::Debug, NexusError>) -> String {
        match result {
            Err(NexusError::BadResponse(reason)) => reason,
            other => panic!("expected BadResponse, got {other:?}"),
        }
    }

    #[test]
    fn clean_data_decodes_and_any_error_sinks_it() {
        let ok: ModsData = answer(
            r#"{"data":{"legacyModsByDomain":{"totalCount":1,"nodes":[{"modId":4,"name":"A","version":"1","updatedAt":"2026-10-14T14:23:50Z","status":"published"}]}}}"#,
        )
        .into_data()
        .unwrap();
        let node = ok
            .legacy_mods_by_domain
            .nodes
            .into_iter()
            .next()
            .unwrap()
            .into_mod()
            .unwrap();
        assert_eq!(node.updated_ms, Some(1_791_987_830_000));
        assert_eq!(
            bad_response(answer(
                r#"{"errors":[{"message":"Field 'nope' doesn't exist","extensions":{"code":"undefinedField"}}],"data":null}"#,
            )
            .into_data::<ModsData>()),
            "Field 'nope' doesn't exist [undefinedField]"
        );
        // Data AND errors: the page cannot be trusted, however whole it looks.
        assert_eq!(
            bad_response(answer(
                r#"{"data":{"legacyModsByDomain":{"totalCount":0,"nodes":[]}},
                    "errors":[{"message":"Search backend unavailable"},{"message":"retry"},{"message":"later"}]}"#,
            )
            .into_data::<ModsData>()),
            "Search backend unavailable (and 2 more errors)"
        );
        assert!(bad_response(answer("{}").into_data::<ModsData>()).contains("no data"));
        assert!(bad_response(GraphQlAnswer::parse(b"<html>")).contains("not GraphQL JSON"));
        assert!(bad_response(GraphQlAnswer::parse(b"[1, 2]")).contains("not GraphQL JSON"));
    }

    #[test]
    fn errors_with_odd_paths_or_no_message_still_parse() {
        let parsed = answer(
            r#"{"data":null,"errors":[{"path":"not-a-list"},{"message":null,"path":["m1"]},{"message":"  "}]}"#,
        );
        assert_eq!(parsed.errors.len(), 3);
        assert!(parsed.errors[0].path().is_empty());
        assert_eq!(parsed.errors[1].describe(), "an unnamed GraphQL error");
        assert_eq!(parsed.errors[2].describe(), "an unnamed GraphQL error");
        assert_eq!(
            first_error_message(br#"{"errors":[{"message":"x","extensions":{"code":"Y"}}]}"#),
            Some("x [Y]".to_string())
        );
        assert_eq!(first_error_message(b"<html>"), None);
    }

    #[test]
    fn an_error_naming_one_alias_leaves_only_that_mod_unchecked() {
        let round = read_mod_files(
            answer(
                r#"{"data":{"m1":[{"fileId":5,"date":20},{"fileId":4,"date":10}],"m2":[{"fileId":6}],"m3":[]},
                    "errors":[{"message":"groupId could not be resolved","path":["m2",0,"groupId"]}]}"#,
            ),
            &[1, 2, 3],
        )
        .unwrap();
        assert_eq!(
            round.files[&1]
                .iter()
                .map(|f| f.file_id)
                .collect::<Vec<_>>(),
            vec![4, 5],
            "oldest first"
        );
        assert!(round.files[&3].is_empty(), "an unknown mod answers []");
        assert!(
            !round.files.contains_key(&2),
            "data that arrived with an error is not trusted"
        );
        assert_eq!(round.refused[&2], "groupId could not be resolved");
        assert!(round.unanswered.is_empty());
    }

    #[test]
    fn a_nulled_batch_names_the_culprit_and_hands_back_the_rest() {
        let round = read_mod_files(
            answer(
                r#"{"data":null,"errors":[{"message":"Mod not available","path":["m666"],"extensions":{"code":"MOD_UNAVAILABLE"}}]}"#,
            ),
            &[1, 666, 2],
        )
        .unwrap();
        assert!(round.files.is_empty());
        assert_eq!(round.refused[&666], "Mod not available [MOD_UNAVAILABLE]");
        assert_eq!(round.unanswered, vec![1, 2], "asked again, as one batch");
    }

    #[test]
    fn a_null_or_missing_or_misshapen_list_is_unchecked_never_empty() {
        let round = read_mod_files(
            answer(r#"{"data":{"m1":null,"m3":[{"fileId":"9"}],"m4":"what","m99":[]}}"#),
            &[1, 2, 3, 4],
        )
        .unwrap();
        assert_eq!(round.files.keys().copied().collect::<Vec<_>>(), vec![3]);
        assert!(round.refused[&1].contains("no file list"));
        assert!(
            round.refused[&2].contains("no file list"),
            "left out entirely"
        );
        assert!(round.refused[&4].contains("unexpected shape"));
        assert!(round.unanswered.is_empty());
    }

    #[test]
    fn an_error_about_no_asked_mod_fails_the_whole_batch() {
        for body in [
            r#"{"data":{"m1":null,"m2":null},"errors":[{"message":"Search backend unavailable"}]}"#,
            r#"{"data":null,"errors":[{"message":"Query too complex"}]}"#,
            r#"{"data":{"m1":[]},"errors":[{"message":"stray","path":["m999"]}]}"#,
            r#"{"data":{"m1":[]},"errors":[{"message":"odd","path":[0,"m1"]}]}"#,
        ] {
            let reason = bad_response(read_mod_files(answer(body), &[1, 2]));
            assert!(!reason.is_empty(), "{body}");
        }
        assert!(bad_response(read_mod_files(answer(r#"{"data":null}"#), &[1])).contains("no data"));
        assert!(
            bad_response(read_mod_files(answer(r#"{"data":[1]}"#), &[1]))
                .contains("not keyed by mod")
        );
    }

    #[test]
    fn md5_errors_survive_only_inside_another_games_hit() {
        let hits = r#"{"fileHashes":[
            {"md5":"9e107d9d372bb6826bd81d3542a419d6","gameId":110,"modFile":null},
            {"md5":"9e107d9d372bb6826bd81d3542a419d6","gameId":6234,"modFileId":31,
             "modFile":{"modId":4242,"fileId":31,"name":"Lantern","version":"2","category":"MAIN","date":1700000000}},
            {"md5":"9e107d9d372bb6826bd81d3542a419d6","gameId":null,"modFile":null}]}"#;
        let with_error = |error: &str| answer(&format!(r#"{{"data":{hits},"errors":[{error}]}}"#));
        // Another game's upload failed: Dragon's Dogma 2's hit still counts.
        let survived = read_file_hashes(with_error(
            r#"{"message":"hidden mod","path":["fileHashes",0,"modFile"]}"#,
        ))
        .unwrap();
        assert_eq!(survived.len(), 3);
        // Everything else sinks the lookup.
        for error in [
            r#"{"message":"boom","path":["fileHashes",1,"modFile"]}"#,
            r#"{"message":"boom","path":["fileHashes",2,"modFile"]}"#,
            r#"{"message":"boom","path":["fileHashes",0]}"#,
            r#"{"message":"boom","path":["fileHashes"]}"#,
            r#"{"message":"boom","path":["fileHashes",7,"modFile"]}"#,
            r#"{"message":"boom"}"#,
        ] {
            assert_eq!(
                bad_response(read_file_hashes(with_error(error))),
                "boom",
                "{error}"
            );
        }
        assert_eq!(
            bad_response(read_file_hashes(answer(
                r#"{"data":{"fileHashes":null},"errors":[{"message":"Internal server error","path":["fileHashes"]}]}"#,
            ))),
            "Internal server error"
        );
        assert!(
            read_file_hashes(answer(r#"{"data":{"fileHashes":null}}"#))
                .unwrap()
                .is_empty(),
            "a null list with no error is no hits"
        );
    }

    #[test]
    fn hits_from_other_games_or_without_a_mod_file_are_dropped() {
        let hit = |game: Value, mod_file: Value| -> Option<Md5Hit> {
            serde_json::from_value::<WireFileHash>(json!({
                "md5": "9E107D9D372BB6826BD81D3542A419D6", "fileName": "lantern.zip",
                "fileSize": "2048", "gameId": game, "modFileId": 31, "modFile": mod_file
            }))
            .unwrap()
            .into_hit()
        };
        let mod_file = json!({ "modId": 4242, "groupId": 9, "sqid": "Qq1", "name": "Lantern",
            "version": "2", "category": "MAIN", "date": 1_700_000_000, "primary": 1, "sizeInBytes": null });
        let found = hit(json!(6234), mod_file.clone()).unwrap();
        assert_eq!(found.md5, "9e107d9d372bb6826bd81d3542a419d6");
        assert_eq!(
            (found.mod_id, found.file_id),
            (4242, 31),
            "fileId falls back to modFileId"
        );
        assert_eq!(found.size_bytes, Some(2048), "the hashed file's own size");
        assert_eq!(found.file_name, "lantern.zip");
        assert!(hit(json!(110), mod_file).is_none());
        assert!(hit(json!(6234), Value::Null).is_none());
    }

    #[test]
    fn v1_answers_parse_without_keeping_the_key() {
        let account: V1Validate = serde_json::from_value(json!({
            "user_id": 1, "key": "do-not-keep", "name": "LanternKeeper",
            "is_premium?": true, "is_supporter?": false, "email": "x@example.invalid"
        }))
        .unwrap();
        let account = account.into_account().unwrap();
        assert_eq!(account.name, "LanternKeeper");
        assert!(account.is_premium, "the legacy `is_premium?` key counts");
        let files: V1Files = serde_json::from_value(json!({
            "files": [
                { "file_id": 2, "name": "Lantern", "version": "2", "category_id": 1, "category_name": "MAIN",
                  "is_primary": true, "uploaded_timestamp": 1_700_000_100, "size_in_bytes": 10 },
                { "file_id": 1, "name": "Lantern", "version": "1", "category_id": 4, "category_name": null,
                  "is_primary": false, "uploaded_timestamp": 1_700_000_000, "size_in_bytes": null }
            ],
            "file_updates": [ { "old_file_id": 1, "new_file_id": 2, "uploaded_timestamp": 1_700_000_100 } ]
        }))
        .unwrap();
        let list = files.into_list();
        assert_eq!(list.files[0].file_id, 1, "oldest first");
        assert_eq!(
            list.files[0].category,
            FileCategory::OldVersion,
            "category_id fallback"
        );
        assert_eq!(list.files[1].category, FileCategory::Main);
        assert_eq!(list.successor_of(1), Some(2));
    }

    fn wire_mod(node: Value) -> NexusMod {
        serde_json::from_value::<WireMod>(node)
            .unwrap()
            .into_mod()
            .unwrap()
    }

    #[test]
    fn the_mods_query_asks_for_the_labels_beside_the_status() {
        // One selection, one POST: the labels ride the lookup the update
        // check already makes.
        assert!(
            MODS_QUERY.contains("nodes { modId name version updatedAt status category tags { name } adultContent } }"),
            "{MODS_QUERY}"
        );
        assert_eq!(MODS_QUERY.matches("legacyModsByDomain(").count(), 1);
    }

    #[test]
    fn labels_parse_from_the_live_shape() {
        let lantern = wire_mod(json!({
            "modId": 2207, "name": "Lantern Glow Recolor", "version": "2.1",
            "updatedAt": "2026-09-30T08:01:02Z", "status": "published",
            "category": " Visuals ",
            "tags": [{ "name": "Lore-Friendly" }, { "name": " Utilities for Players " }],
            "adultContent": true
        }));
        assert_eq!(lantern.category.as_deref(), Some("Visuals"));
        assert_eq!(lantern.tags, ["Lore-Friendly", "Utilities for Players"]);
        assert!(lantern.adult);

        let untagged = wire_mod(json!({
            "modId": 5150, "name": "Vernworth Cloaks Collection", "status": "published",
            "category": "Armour", "tags": [], "adultContent": false
        }));
        assert_eq!(untagged.category.as_deref(), Some("Armour"));
        assert!(untagged.tags.is_empty(), "most mods carry no tags");
        assert!(!untagged.adult);
    }

    #[test]
    fn tags_keep_nexus_order_and_spelling_and_drop_blanks_and_repeats() {
        let tags = tag_names(
            2207,
            Some(&json!([
                { "name": "Lore-Friendly" },
                { "name": "  " },
                { "name": "lore-friendly" },
                { "name": null },
                { "id": 31 },
                { "name": "Utilities for Players" },
                "LORE-FRIENDLY",
                "Cloaks",
                42
            ])),
        );
        assert_eq!(
            tags,
            ["Lore-Friendly", "Utilities for Players", "Cloaks"],
            "first spelling wins; a repeat in another case is the same tag"
        );
        assert!(tag_names(2207, Some(&json!("Cloaks"))).is_empty());
        assert!(tag_names(2207, Some(&json!({ "name": "Cloaks" }))).is_empty());
        assert!(tag_names(2207, Some(&Value::Null)).is_empty());
        assert!(tag_names(2207, None).is_empty());
    }

    #[test]
    fn an_older_answer_without_labels_still_parses_as_unlabeled() {
        // The v0.1 selection: no category, no tags, no adultContent.
        let old = wire_mod(json!({
            "modId": 5150, "name": "Vernworth Cloaks Collection", "version": "1",
            "updatedAt": "2026-10-14T14:23:50Z", "status": "published"
        }));
        assert_eq!(
            (old.mod_id, old.name.as_str(), old.version.as_deref()),
            (5150, "Vernworth Cloaks Collection", Some("1"))
        );
        assert_eq!(old.updated_ms, Some(1_791_987_830_000));
        assert!(old.is_available());
        assert_eq!(old.category, None, "no category: the answer carried none");
        assert!(old.tags.is_empty());
        assert!(!old.adult);
    }

    #[test]
    fn odd_label_values_read_as_no_label_and_never_sink_the_mod() {
        let odd = |category: Value, tags: Value, adult: Value| {
            let node = json!({ "modId": 2207, "name": "Lantern Glow Recolor", "status": "published",
                               "category": category, "tags": tags, "adultContent": adult });
            wire_mod(node)
        };
        let shapes = odd(json!(7), json!("Cloaks"), Value::Null);
        assert_eq!(shapes.mod_id, 2207, "the mod itself still reads");
        assert_eq!(
            (shapes.category, shapes.tags.len(), shapes.adult),
            (None, 0, false)
        );
        assert_eq!(odd(json!("   "), json!(null), json!(0)).category, None);
        assert!(
            odd(Value::Null, json!([]), json!(1)).adult,
            "0/1 as elsewhere"
        );
        assert!(odd(Value::Null, json!([]), json!("true")).adult);
        assert!(!odd(Value::Null, json!([]), json!("no")).adult);
    }

    // -- the name search (v0.2, Find on Nexus) ---------------------------------

    #[test]
    fn the_find_query_carries_the_terms_only_as_a_variable() {
        for (mode, op) in [
            (NameMatch::Matches, "MATCHES"),
            (NameMatch::Wildcard, "WILDCARD"),
        ] {
            let query = find_query(mode);
            assert!(
                query.starts_with("query RiftstoneFindMods($terms: String!) { mods(filter: { "),
                "{query}"
            );
            assert!(
                query.contains(r#"gameDomainName: [{ value: "dragonsdogma2", op: EQUALS }]"#),
                "{query}"
            );
            assert!(
                query.contains(&format!("nameStemmed: [{{ value: $terms, op: {op} }}]")),
                "{query}"
            );
            assert!(
                query.contains("sort: [{ relevance: { direction: DESC } }], count: 5)"),
                "{query}"
            );
            assert!(
                query.contains(&format!("nodes {{ {FIND_FIELDS} }}")),
                "{query}"
            );
            assert_eq!(query.matches('{').count(), query.matches('}').count());
            // Whatever a name holds, the query text stays the same: it rides
            // in the variables, never in the text.
            for terms in ["Lantern Glow", "x\" } }) { evil", "$terms", ""] {
                let body = find_body(terms, mode);
                assert_eq!(body["query"], json!(query), "{terms:?}");
                assert_eq!(body["variables"], json!({ "terms": terms }), "{terms:?}");
            }
        }
    }

    fn candidate(node: Value) -> Option<ModCandidate> {
        serde_json::from_value::<WireCandidate>(node)
            .unwrap()
            .into_candidate()
    }

    #[test]
    fn a_candidate_parses_from_the_measured_shape() {
        let found = candidate(json!({
            "modId": 2207, "name": " Lantern Glow Recolor ", "version": "2.1",
            "author": "Vern", "uploader": { "name": "VernUploads" },
            "category": "Visuals", "modCategory": { "name": "Lighting" },
            "adultContent": false, "downloads": 1520, "endorsements": 88,
            "summary": "  Warmer   lanterns\nfor the\tcity. ",
            "updatedAt": "2024-05-04T01:30:45Z", "game": { "domainName": "dragonsdogma2" }
        }))
        .unwrap();
        assert_eq!(
            found,
            ModCandidate {
                mod_id: 2207,
                name: "Lantern Glow Recolor".into(),
                author: Some("Vern".into()),
                version: Some("2.1".into()),
                category: Some("Visuals".into()),
                adult: false,
                downloads: Some(1520),
                endorsements: Some(88),
                summary: Some("Warmer lanterns for the city.".into()),
                updated_ms: Some(1_714_786_245_000),
            }
        );
    }

    #[test]
    fn a_candidate_falls_back_to_the_uploader_and_the_mod_category() {
        let found = candidate(json!({
            "modId": "2207", "name": "Lantern Glow Recolor", "version": "",
            "author": "  ", "uploader": { "name": "VernUploads" },
            "category": null, "modCategory": { "name": "Lighting" },
            "adultContent": null, "downloads": null, "endorsements": "12",
            "summary": null, "updatedAt": "last tuesday"
        }))
        .unwrap();
        assert_eq!(found.author.as_deref(), Some("VernUploads"));
        assert_eq!(found.category.as_deref(), Some("Lighting"));
        assert_eq!(found.version, None, "an empty version is no version");
        assert!(!found.adult, "a null adultContent is not adult");
        assert_eq!((found.downloads, found.endorsements), (None, Some(12)));
        assert_eq!(found.summary, None);
        assert_eq!(found.updated_ms, None, "an unparseable time is none");

        // Neither author nor uploader, neither category.
        let bare = candidate(json!({ "modId": 2207, "name": "Ermine Cape" })).unwrap();
        assert_eq!(
            (bare.author, bare.category, bare.updated_ms),
            (None, None, None)
        );
        assert!(
            candidate(json!({ "modId": 2207, "name": "Ermine Cape", "adultContent": true }))
                .unwrap()
                .adult
        );
    }

    #[test]
    fn a_long_summary_is_cut_at_two_hundred_characters_never_inside_one() {
        let long = format!("{} tail", "\u{e9}t\u{e9} ".repeat(60)); // 240+ chars of multi-byte text
        let found =
            candidate(json!({ "modId": 1, "name": "Ermine Cape", "summary": long })).unwrap();
        let summary = found.summary.unwrap();
        assert!(
            summary.chars().count() <= 200,
            "{}",
            summary.chars().count()
        );
        assert!(summary.chars().count() >= 199);
        assert!(
            long.starts_with(&summary),
            "a prefix, cut on a character boundary"
        );
        assert!(!summary.ends_with(' '));
    }

    #[test]
    fn update_times_read_as_rfc_3339_naive_iso_or_unix_seconds() {
        let at = |value: Value| time_ms_signed(&value);
        assert_eq!(at(json!("2024-05-04T01:30:45Z")), Some(1_714_786_245_000));
        assert_eq!(
            at(json!("2024-05-04T03:30:45+02:00")),
            Some(1_714_786_245_000)
        );
        assert_eq!(
            at(json!("2024-05-04T01:30:45.250Z")),
            Some(1_714_786_245_250)
        );
        assert_eq!(at(json!("2024-05-04T01:30:45")), Some(1_714_786_245_000));
        assert_eq!(at(json!(1_714_786_245)), Some(1_714_786_245_000));
        assert_eq!(at(json!("1714786245")), None);
        assert_eq!(at(json!("2024-05-04")), None);
        assert_eq!(at(json!(true)), None);
    }

    #[test]
    fn nodes_without_an_id_or_a_name_or_from_another_game_are_skipped() {
        for node in [
            json!({ "name": "Ermine Cape" }),
            json!({ "modId": 0, "name": "Ermine Cape" }),
            json!({ "modId": 1_u64 << 40, "name": "Ermine Cape" }),
            json!({ "modId": 2207 }),
            json!({ "modId": 2207, "name": "   " }),
            json!({ "modId": 2207, "name": "Ermine Cape", "game": { "domainName": "skyrimspecialedition" } }),
        ] {
            assert_eq!(candidate(node.clone()), None, "{node}");
        }
        // A node that names no game is taken at the filter's word.
        for game in [
            json!(null),
            json!({}),
            json!({ "domainName": "DragonsDogma2" }),
        ] {
            assert!(
                candidate(json!({ "modId": 2207, "name": "Ermine Cape", "game": game })).is_some(),
                "{game}"
            );
        }
    }

    #[test]
    fn candidates_keep_nexus_order_count_once_and_stop_at_five() {
        let node = |id: u64| json!({ "modId": id, "name": format!("Lantern Glow {id}") });
        let data: FindData = serde_json::from_value(json!({ "mods": { "totalCount": 9, "nodes": [
            node(7), "not a node", node(3), node(7), { "modId": 4 }, node(9), node(1), node(5), node(8)
        ] } }))
        .unwrap();
        let ids: Vec<u64> = read_candidates(data).iter().map(|c| c.mod_id).collect();
        assert_eq!(ids, vec![7, 3, 9, 1, 5]);
        let empty: FindData =
            serde_json::from_value(json!({ "mods": { "totalCount": 0, "nodes": [] } })).unwrap();
        assert!(read_candidates(empty).is_empty());
    }

    // -- v0.3: the key's extras ------------------------------------------------

    #[test]
    fn the_account_reads_its_id_and_both_flags_in_either_spelling() {
        let read = |answer: Value| {
            serde_json::from_value::<V1Validate>(answer)
                .unwrap()
                .into_account()
                .unwrap()
        };
        let current = read(json!({
            "user_id": 4242, "key": "do-not-keep", "name": "Arisen of Vernworth",
            "is_premium": true, "is_supporter": true, "email": "arisen@example.invalid"
        }));
        assert_eq!(
            current,
            NexusAccount {
                name: "Arisen of Vernworth".into(),
                user_id: Some(4242),
                is_premium: true,
                is_supporter: true,
            }
        );
        let legacy = read(json!({
            "user_id": "4242", "name": "Arisen of Vernworth",
            "is_premium?": false, "is_supporter?": 1
        }));
        assert_eq!(legacy.user_id, Some(4242), "a numeric string counts");
        assert!(!legacy.is_premium && legacy.is_supporter, "0/1 counts too");
        // The current spelling wins; a null falls back to the older one.
        let both = read(json!({
            "name": "Arisen of Vernworth", "is_premium": null, "is_premium?": true,
            "is_supporter": false, "is_supporter?": true, "user_id": 0
        }));
        assert!(both.is_premium && !both.is_supporter);
        assert_eq!(both.user_id, None, "no account has id 0");
        let shown = format!(
            "{:?}",
            read(json!({ "name": "Arisen of Vernworth", "key": "do-not-keep" }))
        );
        assert!(!shown.contains("do-not-keep"), "{shown}");
    }

    #[test]
    fn files_json_names_each_files_archive() {
        let files: V1Files = serde_json::from_value(json!({
            "files": [
                { "file_id": 9001, "file_name": " Lantern Glow Recolor-2207-2-1-1700000000.zip ", "name": "Lantern" },
                { "file_id": 9002, "file_name": "", "name": "Lantern" },
                { "file_id": 9003, "name": "Lantern" },
                { "file_name": "orphan.zip" }
            ],
            "file_updates": []
        }))
        .unwrap();
        let list = files.into_list();
        assert_eq!(
            list.file_name(9001),
            Some("Lantern Glow Recolor-2207-2-1-1700000000.zip")
        );
        assert_eq!(list.file_name(9002), None, "a blank name is no name");
        assert_eq!(list.file_name(9003), None);
        assert_eq!(list.file_names.len(), 1);
    }

    #[test]
    fn download_links_keep_nexus_order_and_only_https() {
        let mirrors = read_download_links(
            br#"[
                {"name":"Nexus CDN","short_name":"Nexus CDN","URI":"https://cdn.example.invalid/2207/9001/a.zip?sig=1"},
                {"name":"Plain","short_name":"Plain","URI":"http://cdn.example.invalid/2207/9001/a.zip"},
                {"name":"Odd","short_name":"Odd","URI":"ftp://cdn.example.invalid/a.zip"},
                {"name":"Empty","short_name":"Empty"},
                {"name":"Amsterdam","URI":"https://ams.example.invalid/2207/9001/a.zip"},
                {"uri":"http://127.0.0.1:9/a.zip"},
                "not a mirror"
            ]"#,
        )
        .unwrap();
        let names: Vec<(&str, &str)> = mirrors
            .iter()
            .map(|m| (m.name.as_str(), m.short_name.as_str()))
            .collect();
        assert_eq!(
            names,
            vec![
                ("Nexus CDN", "Nexus CDN"),
                ("Amsterdam", "Amsterdam"),
                ("Nexus Mods", "Nexus Mods")
            ],
            "preferred first; plain http, other schemes and blanks dropped; loopback kept"
        );
        assert_eq!(mirrors[2].uri, "http://127.0.0.1:9/a.zip");
        for body in [
            &b"[]"[..],
            br#"[{"URI":"http://cdn.example.invalid/a.zip"}]"#,
        ] {
            let reason = bad_response(read_download_links(body));
            assert!(reason.contains("no https download link"), "{reason}");
            assert!(
                !reason.contains("example.invalid"),
                "the link stays out: {reason}"
            );
        }
        assert!(bad_response(read_download_links(
            br#"{"URI":"https://a.example.invalid/x"}"#
        ))
        .contains("not a list"));
        assert!(bad_response(read_download_links(b"<html>")).contains("not JSON"));
    }

    #[test]
    fn tracked_mods_are_this_games_each_once() {
        let ids = read_tracked_mods(
            br#"[
                {"mod_id":2207,"domain_name":"dragonsdogma2"},
                {"mod_id":4242,"domain_name":"skyrimspecialedition"},
                {"mod_id":"5150","domain_name":"DragonsDogma2"},
                {"mod_id":2207,"domain_name":"dragonsdogma2"},
                {"mod_id":0,"domain_name":"dragonsdogma2"},
                {"domain_name":"dragonsdogma2"},
                {"mod_id":6006}
            ]"#,
        )
        .unwrap();
        assert_eq!(ids, vec![2207, 5150]);
        assert!(read_tracked_mods(b"[]").unwrap().is_empty());
        assert!(bad_response(read_tracked_mods(br#"{"message":"odd"}"#)).contains("not a list"));
    }

    #[test]
    fn endorsements_keep_decisions_and_the_latest_one_wins() {
        let decided = read_endorsements(
            br#"[
                {"mod_id":2207,"domain_name":"dragonsdogma2","date":1700000000,"version":"1","status":"Endorsed"},
                {"mod_id":5150,"domain_name":"dragonsdogma2","date":1700000000,"version":"2","status":"Abstained"},
                {"mod_id":6006,"domain_name":"dragonsdogma2","date":1700000000,"version":"1","status":"Undecided"},
                {"mod_id":4242,"domain_name":"skyrimspecialedition","date":1700000000,"version":"1","status":"Endorsed"},
                {"mod_id":2207,"domain_name":"dragonsdogma2","date":1600000000,"version":"0","status":"Abstained"},
                {"mod_id":5150,"domain_name":"dragonsdogma2","date":1800000000,"version":"3","status":"ENDORSED"}
            ]"#,
        )
        .unwrap();
        assert_eq!(
            decided,
            vec![
                (2207, EndorseStatus::Endorsed),
                (5150, EndorseStatus::Endorsed)
            ],
            "undecided and other games are no entry; an older decision does not overwrite a newer one"
        );
    }

    #[test]
    fn form_bodies_are_url_encoded() {
        assert_eq!(form_body(&[("mod_id", "2207")]), "mod_id=2207");
        assert_eq!(
            form_body(&[("Version", "1.2 beta+ünï/&=")]),
            "Version=1.2+beta%2B%C3%BCn%C3%AF%2F%26%3D"
        );
        assert_eq!(form_body(&[("a", "1"), ("b", "*-._~")]), "a=1&b=*-._%7E");
        assert_eq!(form_body(&[]), "");
    }

    // -- v0.3: reading and browsing --------------------------------------------

    #[test]
    fn the_file_details_query_asks_for_the_notes_of_one_mod() {
        let body = mod_file_details_body(2207);
        let query = body["query"].as_str().unwrap();
        assert!(
            query.contains("modFiles(modId: 2207, gameId: 6234) {"),
            "{query}"
        );
        assert!(query.contains(FILE_DETAIL_FIELDS), "{query}");
        assert!(FILE_DETAIL_FIELDS.contains("description changelogText"));
        assert_eq!(query.matches('{').count(), query.matches('}').count());
        assert!(body.get("variables").is_none());
    }

    #[test]
    fn file_details_read_the_live_shapes() {
        let data: ModFileDetailsData = serde_json::from_value(json!({ "modFiles": [
            { "fileId": 9002, "groupId": 31, "sqid": "Rk4vLmN2Q", "name": "Lantern Glow Recolor",
              "version": "2.1", "category": "MAIN", "date": 1_700_000_100, "primary": 1,
              "sizeInBytes": "2048", "description": "  Warmer\n\nlanterns   for the city.  ",
              "changelogText": ["  Ember tint.", "", "   ", "Fixed the Vernworth gate glow. "] },
            { "fileId": 9001, "groupId": 31, "name": "Lantern Glow Recolor", "version": "2.0",
              "category": "ARCHIVED", "date": 1_700_000_000, "primary": 0, "sizeInBytes": null,
              "description": null, "changelogText": [] },
            { "name": "no id" },
            "not a file"
        ] }))
        .unwrap();
        let details = read_file_details(data).unwrap();
        assert_eq!(
            details.iter().map(|d| d.file.file_id).collect::<Vec<_>>(),
            vec![9001, 9002],
            "oldest first; nodes without an id skipped"
        );
        let newest = &details[1];
        assert_eq!(newest.file.size_bytes, Some(2048));
        assert_eq!(newest.file.group_id, Some(31));
        assert_eq!(newest.file.category, FileCategory::Main);
        assert!(newest.file.primary);
        assert_eq!(newest.file.uploaded_ms, 1_700_000_100_000);
        assert_eq!(
            newest.description.as_deref(),
            Some("Warmer lanterns for the city.")
        );
        assert_eq!(
            newest.changelog,
            vec!["Ember tint.", "Fixed the Vernworth gate glow."]
        );
        let oldest = &details[0];
        assert_eq!(
            (oldest.description.as_deref(), oldest.changelog.len()),
            (None, 0)
        );
        assert_eq!(oldest.file.category, FileCategory::Archived);

        let unknown: ModFileDetailsData =
            serde_json::from_value(json!({ "modFiles": [] })).unwrap();
        assert!(
            read_file_details(unknown).unwrap().is_empty(),
            "an unknown mod has no files"
        );
        let failed: ModFileDetailsData =
            serde_json::from_value(json!({ "modFiles": null })).unwrap();
        assert!(bad_response(read_file_details(failed)).contains("no file list"));
    }

    #[test]
    fn a_long_description_is_cut_at_four_hundred_characters() {
        let long = "Ember lanterns glow. ".repeat(40); // 840 characters
        let wire: WireModFile =
            serde_json::from_value(json!({ "fileId": 1, "description": long })).unwrap();
        let description = wire.into_detail().unwrap().description.unwrap();
        assert!(description.chars().count() <= FILE_DESCRIPTION_CHARS);
        assert!(description.chars().count() >= FILE_DESCRIPTION_CHARS - 1);
        assert!(long.starts_with(&description));
    }

    #[test]
    fn changelog_text_reads_a_list_or_a_block_and_nothing_else() {
        assert_eq!(
            changelog_lines(Some(json!(["a", " b ", "", 3, null]))),
            vec!["a", "b"]
        );
        assert_eq!(
            changelog_lines(Some(json!("first line\r\n\nsecond line"))),
            vec!["first line", "second line"]
        );
        assert!(changelog_lines(Some(json!({ "a": 1 }))).is_empty());
        assert!(changelog_lines(None).is_empty());
    }

    #[test]
    fn the_browse_query_carries_text_only_as_a_variable_and_never_or() {
        for (sort, text, offset) in [
            ("endorsements", "", 0),
            ("downloads", "lantern glow", 40),
            ("updatedAt", "", 20),
            (
                "relevance",
                "x\" } ] }) { __schema { types { name } } } #",
                0,
            ),
        ] {
            let body = browse_body(text, sort, offset);
            let query = body["query"].as_str().unwrap();
            assert!(
                query.contains(r#"gameDomainName: [{ value: "dragonsdogma2", op: EQUALS }]"#),
                "{query}"
            );
            assert!(
                query.contains(&format!("sort: [{{ {sort}: {{ direction: DESC }} }}]")),
                "{query}"
            );
            assert!(
                query.contains(&format!("offset: {offset}, count: 20)")),
                "{query}"
            );
            assert!(query.contains("totalCount nodes {"), "{query}");
            assert!(!query.contains("OR"), "never a top-level OR: {query}");
            assert_eq!(query.matches('{').count(), query.matches('}').count());
            if text.is_empty() {
                assert!(
                    query.starts_with("query RiftstoneBrowseMods { mods("),
                    "{query}"
                );
                assert!(
                    !query.contains("$text") && !query.contains("nameStemmed"),
                    "{query}"
                );
                assert!(body.get("variables").is_none());
            } else {
                assert!(
                    query.starts_with("query RiftstoneBrowseMods($text: String!) { mods("),
                    "{query}"
                );
                assert!(
                    query.contains("nameStemmed: [{ value: $text, op: MATCHES }]"),
                    "{query}"
                );
                assert!(!query.contains(text), "the text is no query text: {query}");
                assert_eq!(body["variables"], json!({ "text": text }));
            }
        }
    }

    #[test]
    fn browse_text_is_collapsed_and_capped() {
        assert_eq!(browse_text("  lantern \n\t glow  "), "lantern glow");
        assert_eq!(browse_text("   "), "");
        let long = "\u{e9}".repeat(BROWSE_TEXT_CHARS + 50);
        assert_eq!(browse_text(&long).chars().count(), BROWSE_TEXT_CHARS);
    }

    #[test]
    fn a_browse_page_counts_from_total_count_and_keeps_every_usable_node() {
        let node = |id: u64| json!({ "modId": id, "name": format!("Lantern Glow {id}") });
        let mut nodes: Vec<Value> = (1..=20).map(node).collect();
        nodes.push(node(3));
        nodes.push(json!({ "modId": 77, "name": "Elsewhere", "game": { "domainName": "skyrimspecialedition" } }));
        let data: FindData =
            serde_json::from_value(json!({ "mods": { "totalCount": "1234", "nodes": nodes } }))
                .unwrap();
        let page = read_browse_page(data, 40);
        assert_eq!(page.total, 1234, "a BigInt string counts");
        assert_eq!(page.offset, 40);
        assert_eq!(
            page.mods.iter().map(|m| m.mod_id).collect::<Vec<_>>(),
            (1..=20).collect::<Vec<_>>(),
            "no cap of five here; a repeat and another game's node left out"
        );
        let silent: FindData =
            serde_json::from_value(json!({ "mods": { "nodes": [node(9)] } })).unwrap();
        assert_eq!(
            read_browse_page(silent, 60).total,
            61,
            "no totalCount: no further page"
        );
    }

    #[test]
    fn lenient_scalars() {
        assert_eq!(lenient_u64(&json!("123")), Some(123));
        assert_eq!(lenient_u64(&json!(-1)), None);
        assert_eq!(lenient_u64(&json!("x")), None);
        assert!(
            lenient_bool(&json!(1)) && lenient_bool(&json!("true")) && lenient_bool(&json!(true))
        );
        assert!(!lenient_bool(&json!(0)) && !lenient_bool(&json!(null)));
        assert_eq!(lenient_time_ms(&json!(2)), Some(2000));
        assert_eq!(lenient_time_ms(&json!("1970-01-01T00:00:03Z")), Some(3000));
        assert_eq!(rfc3339_ms("not a date"), None);
    }
}
