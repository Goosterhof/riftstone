//! The Nexus Mods facts the Riftstone keeps. Every struct serializes camelCase,
//! like the rest of the Riftstone's model, and round-trips through serde so the
//! window can cache what it learned.

use std::collections::{BTreeMap, BTreeSet};

use rift_core::{LinkSource, NexusLink};
use serde::{Deserialize, Serialize};

use crate::error::NexusError;

/// One mod page, as `legacyModsByDomain` reports it.
///
/// The labels (v0.2: `category`, `tags`, `adult`) arrive in the same answer
/// as the rest, so they cost no request of their own. They belong to the
/// MOD: every file and every option of a package linked to it inherits them.
/// All three are `#[serde(default)]`, so a `NexusMod` stored before v0.2
/// still loads, as unlabeled.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusMod {
    pub mod_id: u64,
    pub name: String,
    /// The MOD-level version. Shown, never compared: authors forget to bump it
    /// (one armour pack reads "1" at mod level while its file is "8"). Updates
    /// are decided per file — see [`crate::update_state`].
    pub version: Option<String>,
    /// When the mod page last changed, ms since the epoch.
    pub updated_ms: Option<u64>,
    /// Nexus's own status string, unparsed: `published` for a live mod;
    /// `hidden`, `removed`, `wastebinned`, `under_moderation`, `not_published`
    /// are the unavailable ones.
    pub status: String,
    /// v0.2: the mod's category on Nexus (`Armour`, `Visuals`, …), in Nexus's
    /// spelling. Every live mod has exactly one, so `None` means the answer
    /// carried none: an older answer shape, or a value stored before v0.2.
    #[serde(default)]
    pub category: Option<String>,
    /// v0.2: the mod's tags on Nexus (`Utilities for Players`, …), often none.
    /// Nexus's spelling and order. Blank names are dropped, and so is a name
    /// that repeats one already kept, compared case-insensitively (the way the
    /// window's filters compare): no mod carries one tag twice.
    #[serde(default)]
    pub tags: Vec<String>,
    /// v0.2: Nexus flags the mod as adult content (`adultContent`). `false`
    /// unless Nexus said `true`: a null, or an answer that did not say, reads
    /// as not flagged.
    #[serde(default)]
    pub adult: bool,
}

impl NexusMod {
    /// Is the mod page live? `published` (and `publish_with_game`, a page that
    /// goes live with its game) count as available; every other status does not.
    pub fn is_available(&self) -> bool {
        let status = self.status.trim();
        status.eq_ignore_ascii_case("published") || status.eq_ignore_ascii_case("publish_with_game")
    }
}

/// A Nexus mod a package may have come from (v0.2, Find on Nexus): one node of
/// the name search [`crate::NexusClient::find_mods`] makes. A candidate links
/// nothing: the player picks one, and a link by its mod id follows.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModCandidate {
    /// Always in 1..=`i32::MAX`: an id a link can be made from.
    pub mod_id: u64,
    /// Nexus's name for the mod, trimmed; never empty.
    pub name: String,
    /// The author Nexus shows (`author`), else the uploader's name.
    pub author: Option<String>,
    /// The MOD-level version, as Nexus writes it (shown, never compared).
    pub version: Option<String>,
    /// `category`, else `modCategory.name`.
    pub category: Option<String>,
    /// Nexus flags the mod as adult content. A null flag reads `false`; a
    /// flag whose resolver FAILED comes with a GraphQL error, and the whole
    /// answer is refused before it could read as "not adult".
    pub adult: bool,
    pub downloads: Option<u64>,
    pub endorsements: Option<u64>,
    /// Nexus's summary, whitespace-collapsed, at most 200 characters.
    pub summary: Option<String>,
    /// When the mod page last changed (`updatedAt`), ms since the epoch.
    pub updated_ms: Option<i64>,
}

/// A file's category on its mod page (v2 `ModFileCategory`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FileCategory {
    Main,
    Update,
    Optional,
    OldVersion,
    Miscellaneous,
    Removed,
    Archived,
    /// A category Nexus added after this build of the Riftstone.
    #[serde(other)]
    Unknown,
}

impl FileCategory {
    /// From Nexus's enum name (`MAIN`, `OLD_VERSION`, …), any case.
    pub fn from_nexus(name: &str) -> Self {
        match name.trim().to_ascii_uppercase().as_str() {
            "MAIN" => FileCategory::Main,
            "UPDATE" | "UPDATES" => FileCategory::Update,
            "OPTIONAL" => FileCategory::Optional,
            "OLD_VERSION" | "OLD" => FileCategory::OldVersion,
            "MISCELLANEOUS" | "MISC" => FileCategory::Miscellaneous,
            "REMOVED" | "DELETED" => FileCategory::Removed,
            "ARCHIVED" => FileCategory::Archived,
            _ => FileCategory::Unknown,
        }
    }

    /// From v1's numeric `category_id` (1 main … 7 archived).
    pub fn from_v1_id(id: u64) -> Self {
        match id {
            1 => FileCategory::Main,
            2 => FileCategory::Update,
            3 => FileCategory::Optional,
            4 => FileCategory::OldVersion,
            5 => FileCategory::Miscellaneous,
            6 => FileCategory::Removed,
            7 => FileCategory::Archived,
            _ => FileCategory::Unknown,
        }
    }

    /// Retired files are never offered as an update: removed, archived, or
    /// marked as an old version by the author.
    pub fn is_retired(self) -> bool {
        matches!(
            self,
            FileCategory::Removed | FileCategory::Archived | FileCategory::OldVersion
        )
    }
}

/// One file on a mod page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusFile {
    pub file_id: u64,
    /// Every version of one logical file shares a group id (observed, not
    /// documented — the update rule's primary signal).
    pub group_id: Option<u64>,
    /// The file's short unique id; the last token of a 2026-style download name.
    pub sqid: Option<String>,
    /// The file's name on the page (not the archive's file name).
    pub name: String,
    /// The FILE version (`""` on Nexus becomes `None`).
    pub version: Option<String>,
    pub category: FileCategory,
    /// Upload time, ms since the epoch.
    pub uploaded_ms: u64,
    /// The mod page's primary file (the big download button).
    pub primary: bool,
    pub size_bytes: Option<u64>,
}

/// One Nexus file whose archive has the MD5 that was asked about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Md5Hit {
    /// The hash that matched (lower-case hex) — map hits back to packages by it.
    pub md5: String,
    pub mod_id: u64,
    pub file_id: u64,
    pub version: Option<String>,
    /// The file's name on the mod page.
    pub name: String,
    pub size_bytes: Option<u64>,
    pub group_id: Option<u64>,
    pub sqid: Option<String>,
    /// Upload time, ms since the epoch.
    pub uploaded_ms: Option<u64>,
    pub category: FileCategory,
    /// The archive's own file name as uploaded.
    pub file_name: String,
}

impl Md5Hit {
    /// A fully pinned link — MD5 knows the exact file, so no [`crate::pin_link`]
    /// step is needed.
    pub fn to_link(&self) -> NexusLink {
        NexusLink {
            mod_id: self.mod_id,
            file_id: Some(self.file_id),
            group_id: self.group_id,
            sqid: self.sqid.clone(),
            version: self.version.clone(),
            uploaded_ms: self.uploaded_ms,
            source: LinkSource::Md5,
        }
    }
}

/// What [`crate::NexusClient::mod_files_report`] learned: each answered mod's
/// files, and why Nexus would not list the others. Serializes for the window
/// (map keys become strings); it is a call result, not a fact to cache, so it
/// does not deserialize.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModFilesReport {
    /// Every file on each answered mod page, oldest first. A mod Nexus does not
    /// know maps to an empty list.
    pub files: BTreeMap<u64, Vec<NexusFile>>,
    /// Mods Nexus would not list this time, each with the error to show on its
    /// row. Never in `files`: not checked, never "no files".
    pub refused: BTreeMap<u64, NexusError>,
}

/// The account behind a personal API key (v1 `users/validate.json`). Never
/// the key the answer echoes, never the e-mail: neither is read into memory.
/// The name is the player's own: shown in the window, never logged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusAccount {
    pub name: String,
    /// v0.3: the account's numeric id (`user_id`). An nxm link names the
    /// account it was made for; the spine compares the two.
    #[serde(default)]
    pub user_id: Option<u64>,
    /// Premium accounts may download straight from the stone
    /// ([`crate::NexusClient::download_links`] without a grant).
    pub is_premium: bool,
    /// v0.3: `is_supporter` (or the older `is_supporter?`).
    #[serde(default)]
    pub is_supporter: bool,
}

/// v1 `files.json`: the files plus the author's explicit replacement chain.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct V1FileList {
    pub files: Vec<NexusFile>,
    pub updates: Vec<FileUpdate>,
    /// v0.3: each file's archive name as uploaded (`file_name`), by file id —
    /// the name a download lands under. Untrusted text: clean it before it
    /// names anything on disk. Files whose answer carried no name are absent.
    #[serde(default)]
    pub file_names: BTreeMap<u64, String>,
}

/// One link of v1's `file_updates` chain: `old_file_id` was replaced by
/// `new_file_id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileUpdate {
    pub old_file_id: u64,
    pub new_file_id: u64,
    pub uploaded_ms: Option<u64>,
}

impl V1FileList {
    /// The archive name `file_id` was uploaded as (v0.3), if the answer said.
    pub fn file_name(&self, file_id: u64) -> Option<&str> {
        self.file_names.get(&file_id).map(String::as_str)
    }

    /// Follow the replacement chain from `file_id` to its newest successor —
    /// the fallback when group ids mislead. `None` when nothing replaced it.
    /// Cycle-safe; when a file was replaced twice, the later replacement wins.
    pub fn successor_of(&self, file_id: u64) -> Option<u64> {
        let mut seen = BTreeSet::from([file_id]);
        let mut current = file_id;
        loop {
            let next = self
                .updates
                .iter()
                .filter(|u| u.old_file_id == current)
                .max_by_key(|u| (u.uploaded_ms.unwrap_or(0), u.new_file_id))
                .map(|u| u.new_file_id);
            match next {
                Some(id) if seen.insert(id) => current = id,
                _ => break,
            }
        }
        (current != file_id).then_some(current)
    }
}

// ---------------------------------------------------------------------------
// v0.3 — the key's extras, a mod's files, browsing
// ---------------------------------------------------------------------------

/// One place Nexus will serve a file from (v1 `download_link.json`), in
/// Nexus's order: the first is its preferred mirror.
///
/// The `uri` is a secret: the CDN signs it for this account and this file.
/// It is handed to [`crate::Downloader::download_to`] and nowhere else: it
/// never reaches a log line, an error or the window, so this type has no
/// `Serialize` and its `Debug` shows `<redacted>` in its place.
#[derive(Clone, PartialEq, Eq)]
pub struct DownloadMirror {
    /// The mirror's name (`Nexus CDN`, a city).
    pub name: String,
    pub short_name: String,
    /// The download link itself: `https` (or `http` to a loopback test server).
    pub uri: String,
}

impl std::fmt::Debug for DownloadMirror {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DownloadMirror")
            .field("name", &self.name)
            .field("short_name", &self.short_name)
            .field("uri", &"<redacted>")
            .finish()
    }
}

/// What [`crate::NexusClient::confirm_download`] learned of a downloaded
/// archive's MD5. (Hits for other files only are an error,
/// [`crate::NexusError::NotTheFile`].)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DownloadCheck {
    /// Nexus names this exact mod and file for the MD5.
    Confirmed,
    /// Nexus knows no Dragon's Dogma 2 file with this MD5 yet: the size
    /// alone vouches for the archive.
    Unknown,
}

/// An account's decided endorsement of a mod (v1 `user/endorsements.json`).
/// Nexus's third state, `Undecided`, is no entry at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EndorseStatus {
    Endorsed,
    Abstained,
}

impl EndorseStatus {
    /// From Nexus's spelling (`Endorsed`, `Abstained`), any case; `None` for
    /// `Undecided` and anything else.
    pub fn from_nexus(status: &str) -> Option<Self> {
        match status.trim().to_ascii_lowercase().as_str() {
            "endorsed" => Some(EndorseStatus::Endorsed),
            "abstained" => Some(EndorseStatus::Abstained),
            _ => None,
        }
    }
}

/// One file of a mod page with what the player reads before downloading it
/// (v0.3, v2 `modFiles` with `description` and `changelogText`). Anonymous.
///
/// `file` carries the facts the update rules work on (`fileId`, `groupId`,
/// `sqid`, `name`, `version`, `category`, `date`, `primary`, `sizeInBytes`),
/// so [`crate::pin_link`] and [`crate::update_state`] take
/// `details.iter().map(|d| d.file.clone())` as they are. Serializes flat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusFileDetail {
    #[serde(flatten)]
    pub file: NexusFile,
    /// The file's description, whitespace collapsed to single spaces, at most
    /// [`crate::FILE_DESCRIPTION_CHARS`] characters (cut on a character
    /// boundary); `None` when blank.
    #[serde(default)]
    pub description: Option<String>,
    /// The file's own patch notes (`changelogText`), one entry per line as
    /// Nexus wrote them, each trimmed; blank lines dropped.
    #[serde(default)]
    pub changelog: Vec<String>,
}

/// How [`crate::NexusClient::browse`] orders Dragon's Dogma 2's mods.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BrowseSort {
    /// Most endorsed first.
    #[default]
    Endorsements,
    /// Most downloaded first.
    Downloads,
    /// Most recently updated first.
    Updated,
    /// Best match for the text first. Without text there is nothing to be
    /// relevant to, and the page is sorted by endorsements instead.
    Relevance,
}

impl BrowseSort {
    /// The `ModsSort` key, given whether the query carries text.
    pub(crate) fn key(self, has_text: bool) -> &'static str {
        match self {
            BrowseSort::Endorsements => "endorsements",
            BrowseSort::Downloads => "downloads",
            BrowseSort::Updated => "updatedAt",
            BrowseSort::Relevance if has_text => "relevance",
            BrowseSort::Relevance => "endorsements",
        }
    }
}

/// One page of Nexus's Dragon's Dogma 2 mods to ask for (v0.3, browse).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseQuery {
    /// The words the player typed. `""` browses the whole game by `sort`. It
    /// travels only as a GraphQL variable, never as query text.
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub sort: BrowseSort,
    /// How many mods to skip: 0, 20, 40 … ([`crate::BROWSE_PAGE_SIZE`] a page).
    #[serde(default)]
    pub offset: u64,
}

/// One page of browsing (v0.3).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowsePage {
    /// Every match Nexus counts, across all pages (`totalCount`).
    pub total: u64,
    /// The offset this page starts at, as it was asked for.
    pub offset: u64,
    /// The page's mods in Nexus's order; the page size is however many came.
    pub mods: Vec<ModCandidate>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nexus_mod(status: &str) -> NexusMod {
        NexusMod {
            mod_id: 4242,
            name: "Lantern Glow Recolor".into(),
            version: Some("1".into()),
            status: status.into(),
            ..NexusMod::default()
        }
    }

    #[test]
    fn a_mod_stored_before_v0_2_loads_unlabeled() {
        let old: NexusMod = serde_json::from_value(serde_json::json!({
            "modId": 4242, "name": "Lantern Glow Recolor", "version": "1",
            "updatedMs": 1_791_987_830_000_u64, "status": "published"
        }))
        .unwrap();
        assert_eq!(old.category, None);
        assert!(old.tags.is_empty());
        assert!(!old.adult);
        assert!(old.is_available());
    }

    #[test]
    fn labels_round_trip_under_their_camel_case_names() {
        let labelled = NexusMod {
            category: Some("Visuals".into()),
            tags: vec!["Lore-Friendly".into(), "Utilities for Players".into()],
            adult: true,
            ..nexus_mod("published")
        };
        let json = serde_json::to_value(&labelled).unwrap();
        assert_eq!(json["category"], "Visuals");
        assert_eq!(
            json["tags"],
            serde_json::json!(["Lore-Friendly", "Utilities for Players"])
        );
        assert_eq!(json["adult"], true);
        let back: NexusMod = serde_json::from_value(json).unwrap();
        assert_eq!(back, labelled);
    }

    #[test]
    fn availability_follows_the_status_string() {
        assert!(nexus_mod("published").is_available());
        assert!(nexus_mod(" Published ").is_available());
        for gone in [
            "hidden",
            "removed",
            "wastebinned",
            "under_moderation",
            "not_published",
            "",
        ] {
            assert!(
                !nexus_mod(gone).is_available(),
                "{gone} should be unavailable"
            );
        }
    }

    #[test]
    fn categories_parse_from_both_apis_and_survive_the_unknown() {
        assert_eq!(
            FileCategory::from_nexus("OLD_VERSION"),
            FileCategory::OldVersion
        );
        assert_eq!(FileCategory::from_nexus("archived"), FileCategory::Archived);
        assert_eq!(
            FileCategory::from_nexus("SOMETHING_NEW"),
            FileCategory::Unknown
        );
        assert_eq!(FileCategory::from_v1_id(1), FileCategory::Main);
        assert_eq!(FileCategory::from_v1_id(7), FileCategory::Archived);
        assert_eq!(FileCategory::from_v1_id(99), FileCategory::Unknown);
        assert!(FileCategory::Archived.is_retired());
        assert!(!FileCategory::Update.is_retired());
        let cat: FileCategory = serde_json::from_str("\"oldVersion\"").unwrap();
        assert_eq!(cat, FileCategory::OldVersion);
        let cat: FileCategory = serde_json::from_str("\"fromTheFuture\"").unwrap();
        assert_eq!(cat, FileCategory::Unknown);
    }

    #[test]
    fn an_md5_hit_is_a_pinned_link() {
        let hit = Md5Hit {
            md5: "9e107d9d372bb6826bd81d3542a419d6".into(),
            mod_id: 4242,
            file_id: 777,
            version: Some("2.1".into()),
            name: "Lantern Glow Recolor".into(),
            size_bytes: Some(1024),
            group_id: Some(31),
            sqid: Some("aB3dE5fG7".into()),
            uploaded_ms: Some(1_700_000_000_000),
            category: FileCategory::Main,
            file_name: "Lantern Glow Recolor-4242-2-1-1700000000.zip".into(),
        };
        let link = hit.to_link();
        assert_eq!(
            (link.mod_id, link.file_id, link.group_id, link.source),
            (4242, Some(777), Some(31), LinkSource::Md5)
        );
        assert_eq!(link.uploaded_ms, Some(1_700_000_000_000));
    }

    #[test]
    fn a_download_mirror_never_shows_its_link() {
        let mirror = DownloadMirror {
            name: "Nexus CDN".into(),
            short_name: "Nexus CDN".into(),
            uri: "https://cdn.example.invalid/2207/9001/lantern.zip?md5=signed&expires=1".into(),
        };
        let shown = format!("{mirror:?}");
        assert!(
            shown.contains("Nexus CDN") && shown.contains("<redacted>"),
            "{shown}"
        );
        assert!(
            !shown.contains("example.invalid") && !shown.contains("signed"),
            "{shown}"
        );
    }

    #[test]
    fn endorse_statuses_read_nexus_spelling_and_skip_undecided() {
        assert_eq!(
            EndorseStatus::from_nexus("Endorsed"),
            Some(EndorseStatus::Endorsed)
        );
        assert_eq!(
            EndorseStatus::from_nexus(" ABSTAINED "),
            Some(EndorseStatus::Abstained)
        );
        assert_eq!(EndorseStatus::from_nexus("Undecided"), None);
        assert_eq!(EndorseStatus::from_nexus(""), None);
        assert_eq!(
            serde_json::to_value(EndorseStatus::Abstained).unwrap(),
            "abstained"
        );
    }

    #[test]
    fn a_file_detail_serializes_flat() {
        let detail = NexusFileDetail {
            file: NexusFile {
                file_id: 9001,
                group_id: Some(31),
                sqid: Some("Rk4vLmN2Q".into()),
                name: "Lantern Glow Recolor".into(),
                version: Some("2.1".into()),
                category: FileCategory::Main,
                uploaded_ms: 1_700_000_000_000,
                primary: true,
                size_bytes: Some(2048),
            },
            description: Some("Warmer lanterns.".into()),
            changelog: vec!["Ember tint.".into()],
        };
        let json = serde_json::to_value(&detail).unwrap();
        assert_eq!(json["fileId"], 9001);
        assert_eq!(json["groupId"], 31);
        assert_eq!(json["description"], "Warmer lanterns.");
        assert_eq!(json["changelog"], serde_json::json!(["Ember tint."]));
        let back: NexusFileDetail = serde_json::from_value(json).unwrap();
        assert_eq!(back, detail);
    }

    #[test]
    fn browse_sorts_map_to_nexus_keys_and_relevance_needs_text() {
        assert_eq!(BrowseSort::Endorsements.key(false), "endorsements");
        assert_eq!(BrowseSort::Downloads.key(true), "downloads");
        assert_eq!(BrowseSort::Updated.key(false), "updatedAt");
        assert_eq!(BrowseSort::Relevance.key(true), "relevance");
        assert_eq!(BrowseSort::Relevance.key(false), "endorsements");
        let query: BrowseQuery = serde_json::from_value(
            serde_json::json!({ "text": "lantern", "sort": "updated", "offset": 40 }),
        )
        .unwrap();
        assert_eq!(
            query,
            BrowseQuery {
                text: "lantern".into(),
                sort: BrowseSort::Updated,
                offset: 40
            }
        );
    }

    #[test]
    fn an_account_stored_before_v0_3_still_loads() {
        let old: NexusAccount = serde_json::from_value(serde_json::json!({
            "name": "Arisen of Vernworth", "isPremium": true
        }))
        .unwrap();
        assert_eq!(old.user_id, None);
        assert!(old.is_premium && !old.is_supporter);
    }

    #[test]
    fn the_v1_list_names_each_files_archive() {
        let list = V1FileList {
            file_names: BTreeMap::from([(
                9001,
                "Lantern Glow Recolor-2207-2-1-1700000000.zip".into(),
            )]),
            ..V1FileList::default()
        };
        assert_eq!(
            list.file_name(9001),
            Some("Lantern Glow Recolor-2207-2-1-1700000000.zip")
        );
        assert_eq!(list.file_name(9002), None);
        let old: V1FileList =
            serde_json::from_value(serde_json::json!({ "files": [], "updates": [] })).unwrap();
        assert!(
            old.file_names.is_empty(),
            "a list stored before v0.3 still loads"
        );
    }

    #[test]
    fn the_v1_replacement_chain_is_followed_to_its_end() {
        let list = V1FileList {
            files: vec![],
            file_names: BTreeMap::new(),
            updates: vec![
                FileUpdate {
                    old_file_id: 1,
                    new_file_id: 2,
                    uploaded_ms: Some(10),
                },
                FileUpdate {
                    old_file_id: 2,
                    new_file_id: 3,
                    uploaded_ms: Some(20),
                },
                FileUpdate {
                    old_file_id: 3,
                    new_file_id: 1,
                    uploaded_ms: Some(30),
                }, // a cycle must not hang
                FileUpdate {
                    old_file_id: 8,
                    new_file_id: 9,
                    uploaded_ms: None,
                },
            ],
        };
        assert_eq!(list.successor_of(1), Some(3));
        // 2 → 3 → 1 → (2 again: stop) — the walk ends at the last new id.
        assert_eq!(list.successor_of(2), Some(1));
        assert_eq!(list.successor_of(8), Some(9));
        assert_eq!(list.successor_of(9), None);
        assert_eq!(list.successor_of(42), None);
    }
}
