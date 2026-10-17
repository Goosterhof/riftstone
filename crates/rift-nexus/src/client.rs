//! The HTTP side: one blocking client, GraphQL v2 for the public data, v1 REST
//! for the key-only extras, and the manners — identify yourself, batch, back
//! off when told to, and never hammer an endpoint that said "not now".

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Read;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::header::{HeaderMap, ACCEPT, CONTENT_TYPE, RETRY_AFTER};
use reqwest::{Method, StatusCode, Url};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::download::Downloader;
use crate::error::{EndorseRefusal, NexusError};
use crate::hash::is_md5;
use crate::nxm::NxmGrant;
use crate::types::{
    BrowsePage, BrowseQuery, DownloadCheck, DownloadMirror, EndorseStatus, Md5Hit, ModCandidate,
    ModFilesReport, NexusAccount, NexusFile, NexusFileDetail, NexusMod, V1FileList,
};
use crate::wire::{self, NameMatch};
use crate::{
    APP_NAME, DEFAULT_GRAPHQL_URL, DEFAULT_V1_BASE, GAME_DOMAIN, MD5S_PER_REQUEST,
    MODS_PER_REQUEST, MOD_FILES_PER_REQUEST,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
/// How long to sit out after a 429 that named no `Retry-After`.
const DEFAULT_BACKOFF: Duration = Duration::from_secs(30);
/// The longest a single `Retry-After` can bench the client.
const MAX_BACKOFF: Duration = Duration::from_secs(3600);
/// Answers larger than this are refused (the largest seen live: ~190 KB).
const MAX_BODY_BYTES: u64 = 32 * 1024 * 1024;
/// `CompositeDomainWithIdInput.modId` is a GraphQL `Int`: 32-bit signed.
const LARGEST_MOD_ID: u64 = i32::MAX as u64;

/// v1's quota, as its `X-RL-*` headers last reported it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct V1Quota {
    pub daily_limit: Option<u64>,
    pub daily_remaining: Option<u64>,
    /// When the daily bucket refills (as Nexus wrote it, RFC 3339).
    pub daily_reset: Option<String>,
    pub hourly_limit: Option<u64>,
    pub hourly_remaining: Option<u64>,
    /// When the hourly bucket refills (as Nexus wrote it, RFC 3339).
    pub hourly_reset: Option<String>,
}

impl V1Quota {
    /// Both buckets empty: v1 serves 20,000 requests a day, then 500 an hour,
    /// and refuses once both are spent.
    pub fn is_exhausted(&self) -> bool {
        self.daily_remaining == Some(0) && self.hourly_remaining == Some(0)
    }

    fn from_headers(headers: &HeaderMap) -> Option<Self> {
        let text = |name: &str| {
            headers
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(|value| value.trim().to_string())
        };
        let number = |name: &str| text(name).and_then(|value| value.parse::<u64>().ok());
        let quota = V1Quota {
            daily_limit: number("x-rl-daily-limit"),
            daily_remaining: number("x-rl-daily-remaining"),
            daily_reset: text("x-rl-daily-reset"),
            hourly_limit: number("x-rl-hourly-limit"),
            hourly_remaining: number("x-rl-hourly-remaining"),
            hourly_reset: text("x-rl-hourly-reset"),
        };
        (quota != V1Quota::default()).then_some(quota)
    }

    /// Seconds until v1 serves again, when the quota is spent and the hourly
    /// reset is still ahead. `None` = go ahead.
    fn wait_s(&self, now_ms: u64) -> Option<u64> {
        if !self.is_exhausted() {
            return None;
        }
        let reset_ms = self.hourly_reset.as_deref().and_then(wire::rfc3339_ms)?;
        (reset_ms > now_ms).then(|| (reset_ms - now_ms).div_ceil(1000))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Api {
    GraphQl,
    V1,
}

/// Back-off state shared by every clone of a client.
#[derive(Debug, Default)]
struct Throttle {
    cooldown_until: HashMap<Api, Instant>,
    quota: Option<V1Quota>,
}

struct Answer {
    status: StatusCode,
    body: Vec<u8>,
}

/// The Riftstone's Nexus Mods client. Blocking — see the crate docs on threads.
#[derive(Clone)]
pub struct NexusClient {
    /// `Err` when the HTTPS client could not start; every call then answers
    /// [`NexusError::Offline`] instead of panicking at construction.
    http: Result<Client, String>,
    graphql_url: String,
    v1_base: String,
    app_version: String,
    api_key: Option<String>,
    timeout: Duration,
    throttle: Arc<Mutex<Throttle>>,
}

impl std::fmt::Debug for NexusClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NexusClient")
            .field("graphql_url", &self.graphql_url)
            .field("v1_base", &self.v1_base)
            .field("app_version", &self.app_version)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl NexusClient {
    /// A client for the live API. `app_version` goes into `Application-Version`
    /// and the `User-Agent` (anything but `[A-Za-z0-9.+_-]` is dropped; empty
    /// falls back to this crate's version).
    pub fn new(app_version: &str) -> Self {
        let app_version = clean_version(app_version);
        NexusClient {
            http: build_http(&app_version, false),
            graphql_url: DEFAULT_GRAPHQL_URL.to_string(),
            v1_base: DEFAULT_V1_BASE.to_string(),
            app_version,
            api_key: None,
            timeout: DEFAULT_TIMEOUT,
            throttle: Arc::default(),
        }
    }

    /// Point the client elsewhere (tests: a local server). A loopback endpoint
    /// bypasses any proxy configured in the environment.
    pub fn with_endpoints(mut self, graphql_url: &str, v1_base: &str) -> Self {
        self.graphql_url = graphql_url.trim().to_string();
        self.v1_base = v1_base.trim().trim_end_matches('/').to_string();
        let direct = is_loopback(&self.graphql_url) || is_loopback(&self.v1_base);
        self.http = build_http(&self.app_version, direct);
        self
    }

    /// The optional personal API key (v1 extras only; never sent to v2).
    /// Blank counts as none.
    pub fn with_api_key(mut self, key: Option<String>) -> Self {
        self.api_key = key
            .map(|key| key.trim().to_string())
            .filter(|key| !key.is_empty());
        self
    }

    /// The whole-request timeout (default 30 s; connecting is capped at 10 s).
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn has_api_key(&self) -> bool {
        self.api_key.is_some()
    }

    /// The version every request announces.
    pub fn app_version(&self) -> &str {
        &self.app_version
    }

    /// The v1 quota the last v1 answer reported, if any.
    pub fn last_quota(&self) -> Option<V1Quota> {
        self.throttle().quota.clone()
    }

    /// Look mods up: name, mod-level version, last update, status, and the
    /// labels (category, tags, the adult flag), which come in the same answer
    /// and add no request. Batched
    /// [`MODS_PER_REQUEST`] ids per POST; duplicates and ids Nexus cannot hold
    /// (0, above `i32::MAX`) are dropped first. The answer is in request order
    /// and leaves out mods Nexus does not know. An answer that carries any
    /// GraphQL error is refused whole ([`NexusError::BadResponse`]): a page that
    /// came with an error may be missing mods, and a missing mod would read as
    /// "Nexus does not know it".
    pub fn mods(&self, mod_ids: &[u64]) -> Result<Vec<NexusMod>, NexusError> {
        let ids = askable_mod_ids(mod_ids);
        let mut found: HashMap<u64, NexusMod> = HashMap::with_capacity(ids.len());
        for chunk in ids.chunks(MODS_PER_REQUEST) {
            let data: wire::ModsData = self.graphql(&wire::mods_body(chunk))?.into_data()?;
            let page = data.legacy_mods_by_domain;
            if page
                .total_count
                .is_some_and(|total| total > page.nodes.len() as u64)
            {
                log::warn!(
                    "Nexus reported more mods than it returned for one batch; asking for fewer ids \
                     per call may be needed"
                );
            }
            for node in page.nodes {
                if let Some(nexus_mod) = node.into_mod() {
                    if chunk.contains(&nexus_mod.mod_id) {
                        found.insert(nexus_mod.mod_id, nexus_mod);
                    }
                }
            }
        }
        Ok(ids.iter().filter_map(|id| found.remove(id)).collect())
    }

    /// Every file on each mod page, oldest first, aliased [`MOD_FILES_PER_REQUEST`]
    /// mods per POST. A mod Nexus does not know maps to an empty list; a mod
    /// whose files could not be read is ABSENT from the map: treat absent as
    /// "not checked", never as "no files". When nothing could be read and Nexus
    /// refused at least one mod, the refusal of the lowest refused mod id is the
    /// error. [`NexusClient::mod_files_report`] keeps every refusal's reason.
    pub fn mod_files(&self, mod_ids: &[u64]) -> Result<BTreeMap<u64, Vec<NexusFile>>, NexusError> {
        let report = self.mod_files_report(mod_ids)?;
        if report.files.is_empty() {
            if let Some((_, refusal)) = report.refused.into_iter().next() {
                return Err(refusal);
            }
        }
        Ok(report.files)
    }

    /// [`NexusClient::mod_files`], with the refusals kept: why Nexus would not
    /// list each mod it left out, as an error the window can show on that
    /// mod's row.
    ///
    /// When an answer's error names one mod (its path starts at that mod's
    /// alias), that mod alone is left unchecked. If that error nulled the
    /// whole answer (GraphQL null propagation), the other mods of the batch are
    /// asked again as ONE batch. One bad mod cannot sink twenty, and the mods
    /// already refused are not asked again. Every other failure (a body that is
    /// not GraphQL, an error that names no mod that was asked, HTTP, 429,
    /// offline) ends the call at once, because asking again would only repeat it.
    pub fn mod_files_report(&self, mod_ids: &[u64]) -> Result<ModFilesReport, NexusError> {
        let ids = askable_mod_ids(mod_ids);
        let mut report = ModFilesReport::default();
        for chunk in ids.chunks(MOD_FILES_PER_REQUEST) {
            let mut pending = chunk.to_vec();
            // Each round answers or refuses something, so `pending` shrinks
            // every time it is asked again: at most one round per mod.
            while !pending.is_empty() {
                let answer = self.graphql(&wire::mod_files_body(&pending))?;
                let round = wire::read_mod_files(answer, &pending)?;
                report.files.extend(round.files);
                for (mod_id, reason) in round.refused {
                    log::warn!("Nexus could not list the files of mod {mod_id}: {reason}");
                    report
                        .refused
                        .insert(mod_id, NexusError::BadResponse(reason));
                }
                if !round.unanswered.is_empty() {
                    log::warn!(
                        "asking again for the {} mods a refused mod's error took down with it",
                        round.unanswered.len()
                    );
                }
                pending = round.unanswered;
            }
        }
        Ok(report)
    }

    /// Which Dragon's Dogma 2 files have these archive MD5s? Hashes are
    /// normalized to lower case; anything that is not 32 hex digits is skipped
    /// (Nexus rejects the whole call over one). Several hits per hash are
    /// normal (re-uploads): hits come back grouped by hash in the order asked,
    /// best candidate first — live files before retired ones, then newest.
    pub fn files_by_md5(&self, md5s: &[String]) -> Result<Vec<Md5Hit>, NexusError> {
        let mut asked: Vec<String> = Vec::new();
        let mut seen = HashSet::new();
        for raw in md5s {
            let md5 = raw.trim().to_ascii_lowercase();
            if !is_md5(&md5) {
                log::warn!("skipping {raw:?}: an MD5 is 32 hexadecimal digits");
                continue;
            }
            if seen.insert(md5.clone()) {
                asked.push(md5);
            }
        }
        let mut hits = Vec::new();
        for chunk in asked.chunks(MD5S_PER_REQUEST) {
            let answer = self.graphql(&wire::md5_body(chunk))?;
            hits.extend(
                wire::read_file_hashes(answer)?
                    .into_iter()
                    .filter_map(wire::WireFileHash::into_hit),
            );
        }
        let position: HashMap<&str, usize> = asked
            .iter()
            .enumerate()
            .map(|(index, md5)| (md5.as_str(), index))
            .collect();
        hits.retain(|hit| position.contains_key(hit.md5.as_str()));
        hits.sort_by(|a, b| {
            position[a.md5.as_str()]
                .cmp(&position[b.md5.as_str()])
                .then(a.category.is_retired().cmp(&b.category.is_retired()))
                .then(b.uploaded_ms.cmp(&a.uploaded_ms))
                .then(b.file_id.cmp(&a.file_id))
        });
        hits.dedup_by(|a, b| a.md5 == b.md5 && a.mod_id == b.mod_id && a.file_id == b.file_id);
        Ok(hits)
    }

    /// Search Dragon's Dogma 2's mods by name (v0.2, Find on Nexus): the mods
    /// whose name holds every one of `terms` (any order, stems match), most
    /// relevant first, at most [`crate::CANDIDATES_PER_FIND`]. Build the terms
    /// with [`crate::search_terms`].
    ///
    /// One POST of `mods(filter: { gameDomainName, nameStemmed MATCHES },
    /// sort: relevance)`, the terms as a GraphQL variable (never query text).
    /// No candidate → exactly one more POST, the same terms with `WILDCARD`;
    /// so at most two. Blank terms send nothing and find nothing.
    ///
    /// A node is no candidate when its mod id is outside 1..=`i32::MAX` (no
    /// link could be made from it), it has no name, or it names another game;
    /// a repeated mod id counts once. An answer whose nodes are all skipped
    /// is no hit, so the `WILDCARD` retry still runs.
    ///
    /// The update check's manners and refusals: the identifying headers, the
    /// shared back-off after a 429, and an answer that carries any GraphQL
    /// error is refused whole ([`NexusError::BadResponse`]), the retry's too:
    /// a search that failed never reads as "Nexus has no such mod".
    pub fn find_mods(&self, terms: &str) -> Result<Vec<ModCandidate>, NexusError> {
        let terms = terms.trim();
        if terms.is_empty() {
            return Ok(Vec::new());
        }
        for mode in [NameMatch::Matches, NameMatch::Wildcard] {
            let data: wire::FindData = self.graphql(&wire::find_body(terms, mode))?.into_data()?;
            let found = wire::read_candidates(data);
            if !found.is_empty() {
                return Ok(found);
            }
        }
        Ok(Vec::new())
    }

    /// Check the personal API key (v1 `users/validate.json`, which does not
    /// count against the hourly quota): the account's name, id, and whether
    /// it is premium or a supporter. Never the key the answer echoes, never
    /// the e-mail.
    pub fn validate_key(&self) -> Result<NexusAccount, NexusError> {
        let raw: wire::V1Validate = self.v1_get("users/validate.json", false)?;
        raw.into_account().ok_or_else(|| {
            NexusError::BadResponse("the key check answered without an account name".to_string())
        })
    }

    /// v1 `files.json` for one mod: its files plus the author's explicit
    /// `old_file_id → new_file_id` chain — the fallback when group ids mislead —
    /// and (v0.3) each file's archive name ([`V1FileList::file_name`]).
    /// Needs the API key; refused locally while the v1 quota is spent.
    pub fn files_v1(&self, mod_id: u64) -> Result<V1FileList, NexusError> {
        let path = format!("games/{GAME_DOMAIN}/mods/{mod_id}/files.json");
        let raw: wire::V1Files = self.v1_get(&path, true)?;
        Ok(raw.into_list())
    }

    // -- v0.3: the key's extras (v1, metered) ------------------------------

    /// Where one file can be downloaded from (v1 `download_link.json`),
    /// Nexus's preferred mirror first. Premium accounts ask without a grant;
    /// a free account passes the `key` and `expires` of the site's Mod
    /// Manager Download link ([`NxmGrant`]), which travel as the query and
    /// are never logged. Only `https` mirrors are kept (`http` only towards a
    /// loopback test server); none left is [`NexusError::BadResponse`].
    ///
    /// Refusals: 403 → [`NexusError::PremiumRequired`] (no grant, not
    /// premium), 410 → [`NexusError::LinkExpired`], 400 →
    /// [`NexusError::LinkRefused`] (the grant is not for this account or
    /// file), 404 → [`NexusError::FileGone`], 401 → [`NexusError::KeyRejected`].
    /// Refused locally while the v1 quota is spent.
    pub fn download_links(
        &self,
        mod_id: u64,
        file_id: u64,
        grant: Option<&NxmGrant>,
    ) -> Result<Vec<DownloadMirror>, NexusError> {
        let path = format!("games/{GAME_DOMAIN}/mods/{mod_id}/files/{file_id}/download_link.json");
        let expires = grant.map(|grant| grant.expires_s.to_string());
        let query: Vec<(&str, &str)> = match (grant, expires.as_deref()) {
            (Some(grant), Some(expires)) => vec![("key", grant.key.as_str()), ("expires", expires)],
            _ => Vec::new(),
        };
        let answer = self.v1_request(Method::GET, &path, &query, None, true)?;
        match answer.status.as_u16() {
            200..=299 => wire::read_download_links(&answer.body),
            400 => Err(NexusError::LinkRefused),
            401 => Err(NexusError::KeyRejected),
            403 => Err(NexusError::PremiumRequired),
            404 => Err(NexusError::FileGone),
            410 => Err(NexusError::LinkExpired),
            code => Err(NexusError::Http(code)),
        }
    }

    /// The Dragon's Dogma 2 mods this account tracks (v1
    /// `user/tracked_mods.json`), in Nexus's order, each once. Other games'
    /// tracked mods are left out.
    pub fn tracked_mods(&self) -> Result<Vec<u64>, NexusError> {
        let body = self.v1_get_body("user/tracked_mods.json", true)?;
        wire::read_tracked_mods(&body)
    }

    /// Track a mod (v1 POST `user/tracked_mods.json?domain_name=dragonsdogma2`,
    /// form `mod_id`). Already tracked (200) and newly tracked (201) both
    /// succeed. A mod Nexus does not know is [`NexusError::FileGone`].
    pub fn track(&self, mod_id: u64) -> Result<(), NexusError> {
        let answer = self.tracking(Method::POST, mod_id)?;
        match answer.status.as_u16() {
            200..=299 => Ok(()),
            401 => Err(NexusError::KeyRejected),
            404 => Err(NexusError::FileGone),
            code => Err(NexusError::Http(code)),
        }
    }

    /// Stop tracking a mod (v1 DELETE, the same address and form as
    /// [`NexusClient::track`]). A mod that was not tracked (Nexus answers 404)
    /// succeeds too: it is untracked either way.
    pub fn untrack(&self, mod_id: u64) -> Result<(), NexusError> {
        let answer = self.tracking(Method::DELETE, mod_id)?;
        match answer.status.as_u16() {
            200..=299 | 404 => Ok(()),
            401 => Err(NexusError::KeyRejected),
            code => Err(NexusError::Http(code)),
        }
    }

    /// This account's decided endorsements of Dragon's Dogma 2 mods (v1
    /// `user/endorsements.json`). A mod it has not decided on is absent; a
    /// mod listed twice keeps its latest decision.
    pub fn endorsements(&self) -> Result<Vec<(u64, EndorseStatus)>, NexusError> {
        let body = self.v1_get_body("user/endorsements.json", true)?;
        wire::read_endorsements(&body)
    }

    /// Endorse a mod (v1 POST `games/dragonsdogma2/mods/<id>/endorse.json`,
    /// form `Version`: the version the player holds). Nexus's refusals come
    /// back as [`NexusError::EndorseRefused`]: never downloaded from Nexus,
    /// too soon after the download, one's own mod.
    pub fn endorse(&self, mod_id: u64, version: &str) -> Result<(), NexusError> {
        self.endorsement(mod_id, version, "endorse")
    }

    /// Abstain from endorsing a mod (v1 `abstain.json`; as
    /// [`NexusClient::endorse`]).
    pub fn abstain(&self, mod_id: u64, version: &str) -> Result<(), NexusError> {
        self.endorsement(mod_id, version, "abstain")
    }

    // -- v0.3: reading and browsing (v2, anonymous) ------------------------

    /// One mod's files with each one's description and patch notes
    /// (`modFiles` with `description` and `changelogText`), oldest upload
    /// first. Anonymous: the key never goes along. A mod Nexus does not know
    /// answers no files, and so does an id Nexus cannot hold (nothing is
    /// sent). An answer that carries any GraphQL error is refused whole
    /// ([`NexusError::BadResponse`]): a list that failed never reads as "no
    /// files".
    pub fn mod_file_details(&self, mod_id: u64) -> Result<Vec<NexusFileDetail>, NexusError> {
        if !(1..=LARGEST_MOD_ID).contains(&mod_id) {
            log::warn!("skipping mod id {mod_id}: Nexus mod ids run from 1 to {LARGEST_MOD_ID}");
            return Ok(Vec::new());
        }
        let data: wire::ModFileDetailsData = self
            .graphql(&wire::mod_file_details_body(mod_id))?
            .into_data()?;
        wire::read_file_details(data)
    }

    /// One page of Dragon's Dogma 2's mods ([`crate::BROWSE_PAGE_SIZE`] at
    /// `query.offset`), sorted by `query.sort`, descending. Anonymous.
    ///
    /// One POST of `mods(filter: { gameDomainName EQUALS dragonsdogma2 [,
    /// nameStemmed MATCHES $text] }, sort, offset, count)`. The text (whitespace
    /// collapsed, at most 200 characters) travels only as the `$text`
    /// variable; blank text browses the whole game and declares no variable.
    /// `Relevance` without text sorts by endorsements. The filters are never
    /// joined by a top-level `op: OR`. `total` comes from `totalCount`; the
    /// page holds however many usable mods came (another game's node, a node
    /// without a name or with an id outside 1..=`i32::MAX` is skipped; a
    /// repeated id counts once). An answer that carries any GraphQL error is
    /// refused whole.
    pub fn browse(&self, query: &BrowseQuery) -> Result<BrowsePage, NexusError> {
        let text = wire::browse_text(&query.text);
        let offset = query.offset.min(wire::LARGEST_BROWSE_OFFSET);
        let sort_key = query.sort.key(!text.is_empty());
        let data: wire::FindData = self
            .graphql(&wire::browse_body(&text, sort_key, offset))?
            .into_data()?;
        Ok(wire::read_browse_page(data, offset))
    }

    /// v0.3's verifying step for a download: does Nexus know the archive
    /// with this MD5 (from [`crate::Downloaded::md5`]) as exactly this mod's
    /// file? One anonymous `fileHashes` lookup.
    ///
    /// - A hit for `(mod_id, file_id)` → [`DownloadCheck::Confirmed`].
    /// - No Dragon's Dogma 2 hit at all → [`DownloadCheck::Unknown`]: Nexus
    ///   has not hashed the file yet, and the size alone vouches for it.
    /// - Hits for other files only → [`NexusError::NotTheFile`].
    ///
    /// A hash that is not 32 hexadecimal digits is refused
    /// ([`NexusError::BadResponse`]) before anything is sent: it would read
    /// as "unknown" and wave the archive through.
    pub fn confirm_download(
        &self,
        md5: &str,
        mod_id: u64,
        file_id: u64,
    ) -> Result<DownloadCheck, NexusError> {
        let md5 = md5.trim().to_ascii_lowercase();
        if !is_md5(&md5) {
            return Err(NexusError::BadResponse(
                "the archive's MD5 is not 32 hexadecimal digits".to_string(),
            ));
        }
        let hits = self.files_by_md5(&[md5])?;
        if hits.is_empty() {
            Ok(DownloadCheck::Unknown)
        } else if hits
            .iter()
            .any(|hit| hit.mod_id == mod_id && hit.file_id == file_id)
        {
            Ok(DownloadCheck::Confirmed)
        } else {
            Err(NexusError::NotTheFile)
        }
    }

    /// A [`Downloader`] announcing this client's version. It shares nothing
    /// with this client: not the key, not the connection pool.
    pub fn downloader(&self) -> Downloader {
        Downloader::new(&self.app_version)
    }

    // -- plumbing ----------------------------------------------------------

    fn throttle(&self) -> MutexGuard<'_, Throttle> {
        self.throttle.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn http(&self) -> Result<&Client, NexusError> {
        self.http.as_ref().map_err(|reason| {
            NexusError::Offline(format!("the HTTPS client could not start: {reason}"))
        })
    }

    /// POST one GraphQL query; the opened answer, for the caller to judge.
    fn graphql(&self, body: &serde_json::Value) -> Result<wire::GraphQlAnswer, NexusError> {
        let http = self.http()?;
        let answer = self.send(Api::GraphQl, http.post(&self.graphql_url).json(body))?;
        if !answer.status.is_success() {
            if answer.status.is_client_error() {
                if let Some(message) = wire::first_error_message(&answer.body) {
                    // 401 and 403 are not spelled: from anonymous v2 they
                    // never mean the key, and only a rejected key says so.
                    let status = match answer.status.as_u16() {
                        401 | 403 => "not allowed".to_string(),
                        code => format!("HTTP {code}"),
                    };
                    return Err(NexusError::BadResponse(format!("{status}: {message}")));
                }
            }
            return Err(NexusError::Http(answer.status.as_u16()));
        }
        wire::GraphQlAnswer::parse(&answer.body)
    }

    /// The key a v1 call sends: [`NexusError::NoApiKey`] without one,
    /// [`NexusError::KeyRejected`] for one that can never be valid.
    fn v1_key(&self) -> Result<&str, NexusError> {
        let key = self.api_key.as_deref().ok_or(NexusError::NoApiKey)?;
        if !key.bytes().all(|byte| byte.is_ascii_graphic()) {
            // A pasted key with spaces or line breaks inside can never be valid.
            return Err(NexusError::KeyRejected);
        }
        Ok(key)
    }

    /// Refuse a metered call locally while v1's quota is spent and its
    /// hourly reset is still ahead.
    fn quota_gate(&self) -> Result<(), NexusError> {
        let quota_wait = self
            .throttle()
            .quota
            .as_ref()
            .and_then(|quota| quota.wait_s(now_ms()));
        match quota_wait {
            Some(wait) => Err(NexusError::RateLimited {
                retry_after_s: Some(wait),
            }),
            None => Ok(()),
        }
    }

    /// One v1 request with the key, the quota gate for metered calls and
    /// the identifying headers; `query` appended to the address (it may hold
    /// an nxm grant's key, so the address is never logged), `form` sent as
    /// an `application/x-www-form-urlencoded` body. The answer, for the
    /// caller to judge.
    fn v1_request(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, &str)],
        form: Option<&[(&str, &str)]>,
        metered: bool,
    ) -> Result<Answer, NexusError> {
        let key = self.v1_key()?;
        if metered {
            self.quota_gate()?;
        }
        let http = self.http()?;
        let mut url = Url::parse(&format!("{}/{path}", self.v1_base)).map_err(|err| {
            NexusError::Offline(format!(
                "the request was not sent: the v1 address could not be read ({err})"
            ))
        })?;
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query);
        }
        let mut request = http.request(method, url).header("apikey", key);
        if let Some(form) = form {
            request = request
                .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(wire::form_body(form));
        }
        self.send(Api::V1, request)
    }

    /// A v1 GET's body: 2xx, else 401 → [`NexusError::KeyRejected`] and any
    /// other status → [`NexusError::Http`].
    fn v1_get_body(&self, path: &str, metered: bool) -> Result<Vec<u8>, NexusError> {
        let answer = self.v1_request(Method::GET, path, &[], None, metered)?;
        match answer.status.as_u16() {
            200..=299 => Ok(answer.body),
            401 => Err(NexusError::KeyRejected),
            code => Err(NexusError::Http(code)),
        }
    }

    fn v1_get<T: DeserializeOwned>(&self, path: &str, metered: bool) -> Result<T, NexusError> {
        let body = self.v1_get_body(path, metered)?;
        serde_json::from_slice(&body).map_err(|err| {
            NexusError::BadResponse(format!("the v1 answer had an unexpected shape: {err}"))
        })
    }

    /// POST or DELETE `user/tracked_mods.json?domain_name=dragonsdogma2`
    /// with the form `mod_id`.
    fn tracking(&self, method: Method, mod_id: u64) -> Result<Answer, NexusError> {
        let mod_id = mod_id.to_string();
        self.v1_request(
            method,
            "user/tracked_mods.json",
            &[("domain_name", GAME_DOMAIN)],
            Some(&[("mod_id", mod_id.as_str())]),
            true,
        )
    }

    /// POST `games/dragonsdogma2/mods/<id>/<verb>.json` with the form
    /// `Version`. A rejected key is that; then any refusal code Nexus names
    /// (whatever the status) is [`NexusError::EndorseRefused`]; then 2xx is
    /// done and 404 is a mod Nexus does not know.
    fn endorsement(&self, mod_id: u64, version: &str, verb: &str) -> Result<(), NexusError> {
        let path = format!("games/{GAME_DOMAIN}/mods/{mod_id}/{verb}.json");
        let answer = self.v1_request(
            Method::POST,
            &path,
            &[],
            Some(&[("Version", version.trim())]),
            true,
        )?;
        if answer.status == StatusCode::UNAUTHORIZED {
            return Err(NexusError::KeyRejected);
        }
        if let Some(refusal) = EndorseRefusal::from_answer(&String::from_utf8_lossy(&answer.body)) {
            return Err(NexusError::EndorseRefused(refusal));
        }
        match answer.status.as_u16() {
            200..=299 => Ok(()),
            404 => Err(NexusError::FileGone),
            code => Err(NexusError::Http(code)),
        }
    }

    /// Send one request with the identifying headers; turn 429 into a back-off.
    fn send(&self, api: Api, request: RequestBuilder) -> Result<Answer, NexusError> {
        if let Some(wait) = self.cooldown_left(api) {
            return Err(NexusError::RateLimited {
                retry_after_s: Some(wait.as_millis().div_ceil(1000) as u64),
            });
        }
        let response = request
            .header("Application-Name", APP_NAME)
            .header("Application-Version", self.app_version.as_str())
            .header(ACCEPT, "application/json")
            .timeout(self.timeout)
            .send()
            .map_err(transport_error)?;
        let status = response.status();
        if api == Api::V1 {
            if let Some(quota) = V1Quota::from_headers(response.headers()) {
                self.throttle().quota = Some(quota);
            }
        }
        if status == StatusCode::TOO_MANY_REQUESTS {
            let retry_after_s = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(parse_retry_after);
            let wait = retry_after_s
                .map_or(DEFAULT_BACKOFF, Duration::from_secs)
                .min(MAX_BACKOFF);
            self.throttle()
                .cooldown_until
                .insert(api, Instant::now() + wait);
            log::warn!(
                "Nexus Mods answered 429 Too Many Requests; holding {api:?} calls for {}s",
                wait.as_secs()
            );
            return Err(NexusError::RateLimited { retry_after_s });
        }
        let body = read_capped(response)?;
        Ok(Answer { status, body })
    }

    fn cooldown_left(&self, api: Api) -> Option<Duration> {
        let mut throttle = self.throttle();
        let until = *throttle.cooldown_until.get(&api)?;
        let now = Instant::now();
        if until > now {
            Some(until - now)
        } else {
            throttle.cooldown_until.remove(&api);
            None
        }
    }
}

/// The HTTP client. It never follows a redirect: the v1 key travels in a
/// custom `apikey` header, which reqwest strips from no hop (it strips only
/// `Authorization`, `Cookie` and their kin), so a followed redirect would hand
/// the key to whatever host the `Location` names, even over plain http.
/// Nexus's API redirects nowhere: a 3xx comes back as [`NexusError::Http`].
/// It is HTTPS-only unless it talks to a loopback test server (`direct`),
/// which also bypasses any proxy.
fn build_http(app_version: &str, direct: bool) -> Result<Client, String> {
    let mut builder = Client::builder()
        .user_agent(user_agent(app_version))
        .connect_timeout(CONNECT_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none());
    builder = if direct {
        builder.no_proxy()
    } else {
        builder.https_only(true)
    };
    builder.build().map_err(|err| {
        log::error!("the Nexus HTTPS client could not start: {err}");
        err.to_string()
    })
}

pub(crate) fn user_agent(app_version: &str) -> String {
    format!(
        "{APP_NAME}/{app_version} (Dragon's Dogma 2 mod manager; {})",
        std::env::consts::OS
    )
}

pub(crate) fn clean_version(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+' | '_'))
        .take(32)
        .collect();
    if cleaned.is_empty() {
        env!("CARGO_PKG_VERSION").to_string()
    } else {
        cleaned
    }
}

fn is_loopback(url: &str) -> bool {
    Url::parse(url).is_ok_and(|parsed| is_loopback_url(&parsed))
}

/// Does this URL name the machine itself (`localhost`, `127.0.0.0/8`, `::1`)?
pub(crate) fn is_loopback_url(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.trim_start_matches('[').trim_end_matches(']');
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback())
}

/// Mod ids worth asking about, deduplicated, in the order given.
fn askable_mod_ids(mod_ids: &[u64]) -> Vec<u64> {
    let mut seen = HashSet::new();
    mod_ids
        .iter()
        .copied()
        .filter(|&id| {
            let askable = (1..=LARGEST_MOD_ID).contains(&id);
            if !askable {
                log::warn!("skipping mod id {id}: Nexus mod ids run from 1 to {LARGEST_MOD_ID}");
            }
            askable && seen.insert(id)
        })
        .collect()
}

/// `Retry-After` is either delay-seconds or an HTTP-date.
pub(crate) fn parse_retry_after(value: &str) -> Option<u64> {
    let value = value.trim();
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(seconds);
    }
    let at = chrono::DateTime::parse_from_rfc2822(value).ok()?;
    let wait_ms = at.timestamp_millis() - chrono::Utc::now().timestamp_millis();
    Some(u64::try_from(wait_ms).map_or(0, |ms| ms.div_ceil(1000)))
}

fn read_capped(response: Response) -> Result<Vec<u8>, NexusError> {
    let mut body = Vec::new();
    response
        .take(MAX_BODY_BYTES + 1)
        .read_to_end(&mut body)
        .map_err(|err| {
            NexusError::Offline(format!("the answer broke off: {}", root_cause(&err)))
        })?;
    if body.len() as u64 > MAX_BODY_BYTES {
        return Err(NexusError::BadResponse(format!(
            "the answer was larger than {} MB",
            MAX_BODY_BYTES / (1024 * 1024)
        )));
    }
    Ok(body)
}

/// A request that got no answer. reqwest attaches the request's URL to its
/// error, and a URL may carry a secret (an nxm grant's key in a v1 query, a
/// signed download link), so it is stripped before anything reads the error.
pub(crate) fn transport_error(err: reqwest::Error) -> NexusError {
    let err = err.without_url();
    if err.is_timeout() {
        NexusError::Offline("no answer arrived in time".to_string())
    } else if err.is_connect() {
        NexusError::Offline(format!("could not connect: {}", root_cause(&err)))
    } else if err.is_builder() {
        // Refused before it left the machine (plain http to a remote host).
        NexusError::Offline(format!("the request was not sent: {}", root_cause(&err)))
    } else {
        NexusError::Offline(root_cause(&err))
    }
}

/// The innermost error's text — "Connection refused (os error 111)" rather
/// than "error sending request for url (…)".
pub(crate) fn root_cause(err: &(dyn std::error::Error + 'static)) -> String {
    let mut current = err;
    while let Some(next) = current.source() {
        current = next;
    }
    current.to_string()
}

fn now_ms() -> u64 {
    u64::try_from(chrono::Utc::now().timestamp_millis()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderValue;

    #[test]
    fn the_version_header_is_kept_clean() {
        assert_eq!(clean_version("0.1.0"), "0.1.0");
        assert_eq!(clean_version(" 1.2.3-beta+7 "), "1.2.3-beta+7");
        assert_eq!(clean_version("1.0\r\nX-Evil: 1"), "1.0X-Evil1");
        assert_eq!(clean_version(""), env!("CARGO_PKG_VERSION"));
        assert!(user_agent("0.1.0").starts_with("Riftstone/0.1.0 (Dragon's Dogma 2 mod manager; "));
    }

    #[test]
    fn loopback_endpoints_are_recognised() {
        assert!(is_loopback("http://127.0.0.1:4567/v2/graphql"));
        assert!(is_loopback("http://localhost:80/v1"));
        assert!(is_loopback("http://[::1]:9/x"));
        assert!(!is_loopback(DEFAULT_GRAPHQL_URL));
        assert!(!is_loopback("not a url"));
    }

    #[test]
    fn mod_ids_are_deduplicated_and_kept_in_graphql_int_range() {
        assert_eq!(
            askable_mod_ids(&[5, 0, 5, 7, 1 << 40, LARGEST_MOD_ID]),
            vec![5, 7, LARGEST_MOD_ID]
        );
    }

    #[test]
    fn retry_after_reads_seconds_and_http_dates() {
        assert_eq!(parse_retry_after(" 7 "), Some(7));
        assert_eq!(
            parse_retry_after("Wed, 21 Oct 2015 07:28:00 GMT"),
            Some(0),
            "a past date"
        );
        let soon = (chrono::Utc::now() + chrono::Duration::seconds(90)).to_rfc2822();
        let wait = parse_retry_after(&soon).unwrap();
        assert!((85..=91).contains(&wait), "{wait}");
        assert_eq!(parse_retry_after("soon"), None);
    }

    #[test]
    fn v1_quota_headers_are_read_and_honoured() {
        let mut headers = HeaderMap::new();
        assert_eq!(V1Quota::from_headers(&headers), None);
        headers.insert("x-rl-daily-remaining", HeaderValue::from_static("0"));
        headers.insert("x-rl-hourly-remaining", HeaderValue::from_static("0"));
        headers.insert("x-rl-hourly-limit", HeaderValue::from_static("500"));
        headers.insert(
            "x-rl-hourly-reset",
            HeaderValue::from_static("2026-10-16T13:00:00+00:00"),
        );
        let quota = V1Quota::from_headers(&headers).unwrap();
        assert!(quota.is_exhausted());
        assert_eq!(quota.hourly_limit, Some(500));
        let reset = wire::rfc3339_ms("2026-10-16T13:00:00+00:00").unwrap();
        assert_eq!(quota.wait_s(reset - 30_000), Some(30));
        assert_eq!(quota.wait_s(reset + 1), None, "the reset has passed");
        let fresh = V1Quota {
            daily_remaining: Some(19_999),
            ..quota.clone()
        };
        assert_eq!(fresh.wait_s(reset - 30_000), None);
    }

    #[test]
    fn debug_output_never_shows_the_key() {
        let client = NexusClient::new("0.1.0").with_api_key(Some("  super-secret-key  ".into()));
        assert!(client.has_api_key());
        let shown = format!("{client:?}");
        assert!(!shown.contains("super-secret-key"), "{shown}");
        assert!(shown.contains("<redacted>"));
        assert!(!NexusClient::new("0.1.0")
            .with_api_key(Some("   ".into()))
            .has_api_key());
    }

    #[test]
    fn the_client_can_cross_threads() {
        fn assert_send_sync<T: Send + Sync + Clone>() {}
        assert_send_sync::<NexusClient>();
    }
}
