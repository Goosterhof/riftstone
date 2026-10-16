//! rift-nexus — the Riftstone's line to Nexus Mods.
//!
//! Three jobs, all optional (RS-6: everything else works offline):
//!
//! 1. **Look mods up** — names, statuses and every file on a mod page, from
//!    Nexus's public GraphQL v2 API. Anonymous: no account, no API key.
//! 2. **Identify a package** — the MD5 of an archive finds the exact Nexus file
//!    it was downloaded as ([`md5_file`], [`NexusClient::files_by_md5`]).
//! 3. **Decide whether an update exists** — [`pin_link`] ties a package to its
//!    exact file on the mod page, [`update_state`] compares it with the files
//!    the page lists now.
//!
//! # What the live API says (verified 2026-10-16)
//!
//! - `POST https://api.nexusmods.com/v2/graphql` serves public mod data with no
//!   key. Dragon's Dogma 2 is game domain `dragonsdogma2`, game id `6234`
//!   ([`rift_core::dd2`]).
//! - `legacyModsByDomain(ids: [CompositeDomainWithIdInput!]!, offset, count) ->
//!   ModPage!` — `nodes { modId name version updatedAt status }`. 200 ids per call
//!   work; results come back in no particular order and unknown ids are simply
//!   left out. `CompositeDomainWithIdInput.modId` is a GraphQL `Int` (32-bit), so
//!   ids above `i32::MAX` are never sent — one would fail the whole batch.
//!   `updatedAt` is RFC 3339 (`2026-10-14T14:23:50Z`); `status` is a plain string
//!   (`published` for a live mod).
//! - `modFiles(modId: ID!, gameId: ID!) -> [ModFile!]!` — aliased, several mods per
//!   POST. An unknown mod answers `[]`, not an error. Field types: `fileId: Int!`,
//!   `groupId: Int` (nullable), `sqid: String!`, `version: String!` (may be `""`),
//!   `category: ModFileCategory!` (`MAIN UPDATE OPTIONAL OLD_VERSION MISCELLANEOUS
//!   REMOVED ARCHIVED`), `date: Int!` (unix seconds of the upload), `primary: Int!`
//!   (0/1, not a boolean), `sizeInBytes: BigInt` (nullable, sent as a JSON
//!   **string**). Authors often move superseded versions to `ARCHIVED`, not
//!   `OLD_VERSION`.
//! - `fileHashes(md5s: [String!]!) -> [FileHash!]` searches ALL games:
//!   `FileHash { md5 fileName fileSize gameId modFileId createdAt modFile }` where
//!   `fileSize` is a string BigInt, `modFileId` equals `modFile.fileId`, and
//!   `modFile` is nullable (no mod id without it — such hits are dropped). There is
//!   no `modId` on `FileHash` itself. An invalid hash raises `MD5_INVALID`, so
//!   hashes are validated before they are sent. One hash can match hundreds of
//!   uploads across games (the empty file matches ~500); hits are filtered to
//!   game 6234.
//! - Every request carries `Application-Name`, `Application-Version` and a
//!   descriptive `User-Agent` (Nexus's acceptable-use policy). v2 anonymous has no
//!   documented rate limit and sends no quota headers; the client still batches,
//!   and backs off on HTTP 429 (honouring `Retry-After`).
//! - v1 REST (`https://api.nexusmods.com/v1`, header `apikey`) is optional:
//!   [`NexusClient::validate_key`] and [`NexusClient::files_v1`] only. Its quota
//!   headers (`X-RL-*`) are recorded ([`NexusClient::last_quota`]) and honoured.
//!
//! # What the client refuses to believe
//!
//! - **An answer that carries GraphQL `errors`.** GraphQL can return `data` and
//!   `errors` together: a failed field comes back null, so a failed nullable list
//!   reads exactly like an empty one. A lookup that failed must never read as
//!   "nothing found". So [`NexusClient::mods`] refuses any answer with an error,
//!   [`NexusClient::files_by_md5`] tolerates only errors inside another game's
//!   upload, and [`NexusClient::mod_files_report`] leaves unchecked exactly the
//!   mods an error's path names. Each case returns [`NexusError::BadResponse`] or
//!   reports the refusal; none is only logged.
//! - **A redirect.** The client follows none. The key's `apikey` header is not
//!   one reqwest strips when a redirect leaves the host, and Nexus's API never
//!   redirects, so a 3xx is [`NexusError::Http`]. The client is HTTPS-only except
//!   towards a loopback test server.
//!
//! # Threading
//!
//! Every network call is **blocking**. Call them from a worker thread
//! (`tauri::async_runtime::spawn_blocking`, `std::thread`), never from an async
//! task: the underlying `reqwest::blocking` client must not be created, used or
//! dropped on an async runtime thread. A [`NexusClient`] is `Send + Sync + Clone`;
//! clones share one connection pool and one back-off state.

mod client;
mod error;
mod hash;
mod page_url;
mod pin;
mod types;
mod wire;

pub use client::{NexusClient, V1Quota};
pub use error::NexusError;
pub use hash::{is_md5, md5_file};
pub use page_url::{file_page_url, mod_page_url, parse_mod_url};
pub use pin::{pin_link, update_state, UpdateState, STATUS_NOT_FOUND};
pub use types::{
    FileCategory, FileUpdate, Md5Hit, ModFilesReport, NexusAccount, NexusFile, NexusMod, V1FileList,
};

/// Dragon's Dogma 2 on Nexus Mods (re-exported from [`rift_core::dd2`]).
pub const GAME_DOMAIN: &str = rift_core::dd2::NEXUS_GAME_DOMAIN;
/// Dragon's Dogma 2's numeric Nexus game id (re-exported from [`rift_core::dd2`]).
pub const GAME_ID: u64 = rift_core::dd2::NEXUS_GAME_ID;

/// The `Application-Name` header every request carries.
pub const APP_NAME: &str = "Riftstone";
/// Nexus's public GraphQL v2 endpoint.
pub const DEFAULT_GRAPHQL_URL: &str = "https://api.nexusmods.com/v2/graphql";
/// Nexus's v1 REST base (key-only extras).
pub const DEFAULT_V1_BASE: &str = "https://api.nexusmods.com/v1";

/// Mod ids per `legacyModsByDomain` POST (200 verified live).
pub const MODS_PER_REQUEST: usize = 200;
/// Mods per aliased `modFiles` POST.
pub const MOD_FILES_PER_REQUEST: usize = 20;
/// Hashes per `fileHashes` POST.
pub const MD5S_PER_REQUEST: usize = 100;
