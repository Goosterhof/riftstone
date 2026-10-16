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
    FileCategory, FileUpdate, Md5Hit, NexusAccount, NexusFile, NexusMod, V1FileList,
};
use crate::{GAME_DOMAIN, GAME_ID, MODS_PER_REQUEST};

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
}

impl WireModFile {
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

/// `GET /v1/users/validate.json`. The answer echoes the key back; it is never
/// read into memory beyond serde's skip.
#[derive(Debug, Deserialize)]
pub(crate) struct V1Validate {
    name: Option<String>,
    is_premium: Option<bool>,
    #[serde(rename = "is_premium?")]
    is_premium_legacy: Option<bool>,
}

impl V1Validate {
    pub(crate) fn into_account(self) -> Option<NexusAccount> {
        let name = non_empty(self.name)?;
        Some(NexusAccount {
            name,
            is_premium: self.is_premium.or(self.is_premium_legacy).unwrap_or(false),
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
                Some(NexusFile {
                    file_id: file.file_id.as_ref().and_then(lenient_u64)?,
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
        V1FileList { files, updates }
    }
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
