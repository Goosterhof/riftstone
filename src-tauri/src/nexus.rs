//! What the spine knows about each package on Nexus Mods, kept in `nexus.json`.
//!
//! A package's link comes from, in order of authority:
//!
//! 1. the player: a pasted URL or mod id (`manual`), or an explicit unlink —
//!    which also silences the catalog's guess below;
//! 2. an MD5 lookup (`md5`), or (v0.3) a download the stone made itself
//!    (`download`, written before the scan that finds the archive) — both
//!    already pinned to the exact file;
//! 3. a pin made by an update check (`pin_link` against the mod's file list),
//!    which keeps the link's original source;
//! 4. the catalog: the Nexus download file name (`filename`) or a modinfo
//!    `homepage=` (`homepage`) — known mod, file not pinned yet.
//!
//! Update checks are anonymous (GraphQL v2, no key). A mod whose files Nexus
//! did not list is "could not be checked" — never "no files" — and its row
//! carries the reason as an `error` state.
//!
//! **Labels (v0.2).** The batched mod lookup every check makes also brings
//! each mod's category, tags and adult flag. They belong to the Nexus MOD, so
//! the store keeps them per mod id ([`NexusStore::mod_labels`]) and a package
//! takes the labels of whichever mod its link names now: a package linked by
//! its file name, an MD5 match or by hand is labeled once a check has heard
//! of that mod, an unlinked one has none, and a re-link takes the other mod's.
//! Labels are kept like verdicts: a check that did not ask about a mod (its
//! package was out of the mods folder just then) leaves them as they were.
//!
//! **Moved packages (v0.3).** A record is keyed by its package's id. A folder
//! package's id follows its name since v0.3, and the relink pass can find a
//! package under a new id; [`NexusStore::move_records`] lets the record
//! follow (the spine's migration, after every scan).
//! The spine hands them to the catalog's index after every check, every link
//! change and every scan ([`NexusStore::package_labels`]).

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use rift_catalog::PackageLabels;
use rift_core::{LinkSource, NexusLink, Package, PackageId};
use rift_nexus::{ModFilesReport, NexusError, NexusMod, V1Quota};

use crate::dto::{self, NexusQuota, UpdateBadge, UpdateRow, UpdateStateView};

/// How long a check stays fresh before the launch auto-check runs again.
pub const AUTO_CHECK_AFTER_MS: u64 = 12 * 60 * 60 * 1000;

/// `nexus.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusStore {
    #[serde(default)]
    pub schema: u32,
    /// When the last update check finished, ms since the epoch.
    #[serde(default)]
    pub last_check_ms: Option<u64>,
    /// Per package id.
    #[serde(default)]
    pub packages: BTreeMap<String, LinkRecord>,
    /// v0.2: what the checks heard about each mod's labels, per mod id, kept
    /// while a package on the shelf or a record's link or verdict names the
    /// mod ([`NexusStore::record_labels`]). Absent from a v0.1 file: every
    /// package starts unlabeled.
    #[serde(default)]
    pub mod_labels: BTreeMap<u64, ModLabels>,
}

/// One mod's labels on Nexus (v0.2), in Nexus's spelling.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModLabels {
    /// Every live mod has one; `None` when the answer carried none.
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Nexus flags the mod as adult content.
    #[serde(default)]
    pub adult: bool,
}

impl ModLabels {
    fn heard(answer: &NexusMod) -> ModLabels {
        ModLabels {
            category: answer.category.clone(),
            tags: answer.tags.clone(),
            adult: answer.adult,
        }
    }

    /// The answer said nothing about labels (an answer without the label
    /// fields reads as no category, no tags, not adult): kept as no labels,
    /// never as "Nexus said not adult".
    fn is_blank(&self) -> bool {
        self.category.is_none() && self.tags.is_empty() && !self.adult
    }
}

/// What the spine remembers about one package.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkRecord {
    /// The link as it stands: manual, MD5, or pinned by a check. `None` means
    /// "use the catalog's guess" — unless `unlinked`.
    #[serde(default)]
    pub link: Option<NexusLink>,
    /// The player removed the link; the catalog's guess stays silent too.
    #[serde(default)]
    pub unlinked: bool,
    /// The mod's name on Nexus, from the last check.
    #[serde(default)]
    pub mod_name: Option<String>,
    /// The last check's verdict for the link it was made for.
    #[serde(default)]
    pub state: Option<UpdateStateView>,
    /// The mod the verdict belongs to (a re-link to another mod forgets it).
    #[serde(default)]
    pub state_mod_id: Option<u64>,
    #[serde(default)]
    pub checked_ms: Option<u64>,
    /// v0.5 (ruling 10): the link the package had before the player joined it to another
    /// package's file, `None` for the catalog's guess. Read only while the link is `joined`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub joined_over: Option<NexusLink>,
    /// v0.5 (ruling 10): the file groups the player said this package is not a version of.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub declined_groups: Vec<u64>,
}

impl NexusStore {
    pub const SCHEMA: u32 = 1;

    pub fn new() -> NexusStore {
        NexusStore {
            schema: NexusStore::SCHEMA,
            ..NexusStore::default()
        }
    }

    /// The link a package carries now (see the module docs for the order).
    pub fn link_for(&self, package: &Package) -> Option<NexusLink> {
        match self.packages.get(&package.id.0) {
            Some(record) if record.unlinked => None,
            Some(LinkRecord {
                link: Some(link), ..
            }) => Some(link.clone()),
            _ => package.nexus.clone(),
        }
    }

    /// The last verdict for this package's current link, if it still applies.
    fn verdict(&self, id: &PackageId, link: &NexusLink) -> Option<&LinkRecord> {
        self.packages
            .get(&id.0)
            .filter(|record| record.state.is_some() && record.state_mod_id == Some(link.mod_id))
    }

    /// A check that named this package's exact file as another package's newer one, while this
    /// package has no verdict of its own (v0.5: a version just downloaded, whose older version's
    /// check led to it). Any record `nexus.json` keeps counts, an older version put away in the
    /// older-versions folder too; the latest such check wins.
    fn vouched(&self, package: &Package, link: &NexusLink) -> Option<&LinkRecord> {
        let file_id = link.file_id?;
        self.packages
            .iter()
            .filter(|(id, _)| **id != package.id.0)
            .map(|(_, record)| record)
            .filter(|record| record.state_mod_id == Some(link.mod_id))
            .filter(|record| match record.state.as_ref() {
                Some(UpdateStateView::UpdateAvailable { newer }) => newer.file_id == file_id,
                Some(UpdateStateView::LocalArchived { newer: Some(newer) }) => {
                    newer.file_id == file_id
                }
                _ => false,
            })
            .max_by_key(|record| record.checked_ms)
    }

    /// The link `nexus.json` keeps for a package id, whether or not the package is on the shelf
    /// (v0.5: a version in the older-versions folder keeps its record there); `None` when the
    /// record holds none or the player unlinked it.
    pub fn record_link(&self, id: &PackageId) -> Option<NexusLink> {
        self.packages
            .get(&id.0)
            .filter(|record| !record.unlinked)
            .and_then(|record| record.link.clone())
    }

    /// The mod's name on Nexus as the package's last check heard it, while that check still
    /// applies to its link.
    pub fn mod_name(&self, id: &PackageId, link: &NexusLink) -> Option<String> {
        self.verdict(id, link)?.mod_name.clone()
    }

    /// A pasted link (or a fresh MD5 match) replaces whatever was there.
    pub fn set_link(&mut self, package: &Package, link: NexusLink) {
        self.set_link_for(&package.id, link);
    }

    /// [`NexusStore::set_link`] by id, for a package the shelf does not hold
    /// yet (v0.3: a download links its archive before the scan that finds it).
    pub fn set_link_for(&mut self, package_id: &PackageId, link: NexusLink) {
        let record = self.packages.entry(package_id.0.clone()).or_default();
        if record.state_mod_id != Some(link.mod_id) {
            record.state = None;
            record.state_mod_id = None;
            record.mod_name = None;
            record.checked_ms = None;
        }
        record.link = Some(link);
        record.unlinked = false;
    }

    /// v0.5 (ruling 10): the player's word that `package`, whose link is `link`, is a version of
    /// the file `group_id` names. The link keeps its mod, file id, version and upload time and
    /// takes the group, `source: Joined`; the link the record held before stays beside it for
    /// [`NexusStore::part`].
    pub fn join(&mut self, package: &Package, link: NexusLink, group_id: u64) {
        let before = self.record_link(&package.id);
        self.set_link(
            package,
            NexusLink {
                group_id: Some(group_id),
                source: LinkSource::Joined,
                ..link
            },
        );
        if let Some(record) = self.packages.get_mut(&package.id.0) {
            record.joined_over = before;
        }
    }

    /// v0.5 (ruling 10): takes a `joined` link back to the one the package had before, and
    /// remembers the group as a no. False when the link is not `joined`.
    pub fn part(&mut self, package: &Package) -> bool {
        let Some(record) = self.packages.get_mut(&package.id.0) else {
            return false;
        };
        let Some(link) = record.link.take_if(|l| l.source == LinkSource::Joined) else {
            return false;
        };
        record.link = record.joined_over.take();
        if let Some(group_id) = link.group_id {
            if !record.declined_groups.contains(&group_id) {
                record.declined_groups.push(group_id);
            }
        }
        true
    }

    /// v0.5 (ruling 10): the player's no, `package` is not a version of `group_id`'s file.
    pub fn decline(&mut self, package_id: &PackageId, group_id: u64) {
        let record = self.packages.entry(package_id.0.clone()).or_default();
        if !record.declined_groups.contains(&group_id) {
            record.declined_groups.push(group_id);
        }
    }

    /// v0.5 (ruling 10): the file groups the player said this package is not a version of.
    pub fn declined(&self, id: &PackageId) -> &[u64] {
        self.packages
            .get(&id.0)
            .map_or(&[], |record| record.declined_groups.as_slice())
    }

    /// The player removed the link.
    pub fn unlink(&mut self, package: &Package) {
        self.packages.insert(
            package.id.0.clone(),
            LinkRecord {
                unlinked: true,
                ..LinkRecord::default()
            },
        );
    }

    /// One package's row on the Nexus page.
    pub fn row(&self, package: &Package) -> UpdateRow {
        let link = self.link_for(package);
        let own = link.as_ref().and_then(|l| self.verdict(&package.id, l));
        let vouched = match (&link, own) {
            (Some(l), None) => self.vouched(package, l),
            _ => None,
        };
        let verdict = own.or(vouched);
        let state = match (&link, own, vouched) {
            (None, _, _) => UpdateStateView::Unlinked,
            (Some(_), Some(record), _) => {
                record.state.clone().unwrap_or(UpdateStateView::NotChecked)
            }
            (Some(_), None, Some(_)) => UpdateStateView::Current,
            (Some(_), None, None) => UpdateStateView::NotChecked,
        };
        let local_version = link
            .as_ref()
            .and_then(|l| l.version.clone())
            .or_else(|| package_version(package));
        UpdateRow {
            package_id: package.id.0.clone(),
            package_name: package.display_name.clone(),
            link: link.as_ref().map(dto::NexusLink::from),
            mod_name: verdict.and_then(|r| r.mod_name.clone()),
            local_version,
            local_uploaded_ms: link.as_ref().and_then(|l| l.uploaded_ms),
            state,
            page_url: link.as_ref().map(|l| rift_nexus::mod_page_url(l.mod_id)),
            checked_ms: verdict.and_then(|r| r.checked_ms),
            older: Vec::new(),
        }
    }

    /// The file the package's last check named as newer than its own, while that check still
    /// applies to its link (v0.5: one of the family rule's two ties).
    pub fn newer_file_id(&self, id: &PackageId, link: &NexusLink) -> Option<u64> {
        match self.verdict(id, link)?.state.as_ref()? {
            UpdateStateView::UpdateAvailable { newer } => Some(newer.file_id),
            UpdateStateView::LocalArchived { newer: Some(newer) } => Some(newer.file_id),
            _ => None,
        }
    }

    /// The badge on a search card: `None` when the package is not linked.
    pub fn badge(&self, package: &Package) -> Option<UpdateBadge> {
        let link = self.link_for(package)?;
        let Some(verdict) = self.verdict(&package.id, &link) else {
            return Some(if self.vouched(package, &link).is_some() {
                UpdateBadge::Current
            } else {
                UpdateBadge::Unknown
            });
        };
        Some(match verdict.state.as_ref() {
            Some(UpdateStateView::UpdateAvailable { .. }) => UpdateBadge::Available,
            Some(UpdateStateView::Current) => UpdateBadge::Current,
            _ => UpdateBadge::Unknown,
        })
    }

    /// Record one package's check: the pinned link (when the check pinned
    /// it), the mod's name and the verdict.
    pub fn record_check(&mut self, package: &Package, outcome: CheckOutcome, now_ms: u64) {
        let catalog_link = package.nexus.clone();
        let record = self.packages.entry(package.id.0.clone()).or_default();
        if record.unlinked {
            return;
        }
        if let Some(pinned) = outcome.pinned {
            // Keep a catalog guess implicit until a pin actually adds to it.
            if record.link.is_some() || catalog_link.as_ref() != Some(&pinned) {
                record.link = Some(pinned);
            }
        }
        record.mod_name = outcome.mod_name;
        record.state = Some(outcome.state);
        record.state_mod_id = Some(outcome.mod_id);
        record.checked_ms = Some(now_ms);
    }

    /// Record the labels a check heard (v0.2). `asked` are the mod ids the
    /// check looked up (the mods the packages on the shelf link to now);
    /// `answered` is what Nexus sent back. A mod Nexus answered for takes the
    /// labels of the answer. Every other mod keeps what an earlier check
    /// heard, the way a package's verdict outlives a check that did not judge
    /// it: a linked package whose root is out of the library (removed,
    /// unreadable at launch, or not scanned yet) is not asked about, and
    /// still has its labels once it is back. Only a mod nothing names any
    /// more is forgotten: no package the check asked for, and no record's
    /// link or verdict (an unlink or a re-link elsewhere ends both).
    pub fn record_labels(&mut self, asked: &[u64], answered: &HashMap<u64, NexusMod>) {
        for &mod_id in asked {
            let Some(answer) = answered.get(&mod_id) else {
                continue;
            };
            let heard = ModLabels::heard(answer);
            if heard.is_blank() {
                self.mod_labels.remove(&mod_id);
            } else {
                self.mod_labels.insert(mod_id, heard);
            }
        }
        let named: HashSet<u64> = asked.iter().copied().chain(self.named_mods()).collect();
        self.mod_labels.retain(|mod_id, _| named.contains(mod_id));
    }

    /// The mods the records name: each package's own link and the mod its
    /// last verdict was for (a catalog guess that was checked leaves only the
    /// latter). An unlinked package names none.
    fn named_mods(&self) -> impl Iterator<Item = u64> + '_ {
        self.packages
            .values()
            .filter(|record| !record.unlinked)
            .flat_map(|record| {
                record
                    .link
                    .as_ref()
                    .map(|link| link.mod_id)
                    .into_iter()
                    .chain(record.state_mod_id)
            })
    }

    /// Give records the ids their packages carry now (v0.3: `from → to`). A
    /// record moves only onto an id that has none of its own, and is never
    /// merged into one. `true` when anything moved.
    pub fn move_records(&mut self, moves: &BTreeMap<PackageId, PackageId>) -> bool {
        let mut moved = false;
        for (from, to) in moves {
            if from == to || self.packages.contains_key(&to.0) {
                continue;
            }
            if let Some(record) = self.packages.remove(&from.0) {
                self.packages.insert(to.0.clone(), record);
                moved = true;
            }
        }
        moved
    }

    /// `to` holds exactly what `from` holds (its record, or none), ahead of a move that takes
    /// `to`'s name (RS-A2). Unlike [`NexusStore::move_records`] it writes over a record `to`
    /// had: the caller has made sure no package on disk carries `to`, so such a record is a
    /// leftover. True when `to` changed.
    pub fn mirror_record(&mut self, from: &PackageId, to: &PackageId) -> bool {
        let record = self.packages.get(&from.0).cloned();
        if self.packages.get(&to.0) == record.as_ref() {
            return false;
        }
        match record {
            Some(record) => self.packages.insert(to.0.clone(), record),
            None => self.packages.remove(&to.0),
        };
        true
    }

    /// Forget the record under `id`. True when there was one.
    pub fn drop_record(&mut self, id: &PackageId) -> bool {
        self.packages.remove(&id.0).is_some()
    }

    /// The labels a package carries now: those of the mod its link names, as
    /// the latest check heard them. `None` when it is unlinked, or no check
    /// has heard of its mod yet.
    pub fn labels_for(&self, package: &Package) -> Option<&ModLabels> {
        let link = self.link_for(package)?;
        self.mod_labels.get(&link.mod_id)
    }

    /// Every package's labels, for the catalog's index
    /// (`Catalog::set_labels`): the labeled packages only.
    pub fn package_labels(&self, packages: &[Package]) -> HashMap<PackageId, PackageLabels> {
        packages
            .iter()
            .filter_map(|package| {
                let labels = self.labels_for(package)?;
                Some((
                    package.id.clone(),
                    PackageLabels {
                        category: labels.category.clone(),
                        tags: labels.tags.clone(),
                        adult: labels.adult,
                    },
                ))
            })
            .collect()
    }
}

/// The local version a package claims when Nexus does not say: the first
/// option's modinfo `version=`.
fn package_version(package: &Package) -> Option<String> {
    package
        .options
        .iter()
        .find_map(|o| o.info.as_ref().and_then(|i| i.version.clone()))
}

/// One package's verdict from a check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckOutcome {
    pub mod_id: u64,
    pub pinned: Option<NexusLink>,
    pub mod_name: Option<String>,
    pub state: UpdateStateView,
}

/// Decide one link's verdict from what the check fetched. `files` absent
/// from the report means Nexus did not list them this time.
pub fn judge(
    link: &NexusLink,
    mods: &HashMap<u64, NexusMod>,
    report: &ModFilesReport,
) -> CheckOutcome {
    let mod_info = mods.get(&link.mod_id);
    let mod_name = mod_info.map(|m| m.name.clone());
    if let Some(refusal) = report.refused.get(&link.mod_id) {
        return CheckOutcome {
            mod_id: link.mod_id,
            pinned: None,
            mod_name,
            state: UpdateStateView::Error {
                message: refusal.to_string(),
            },
        };
    }
    let Some(files) = report.files.get(&link.mod_id) else {
        return CheckOutcome {
            mod_id: link.mod_id,
            pinned: None,
            mod_name,
            state: UpdateStateView::Error {
                message: "Nexus Mods did not list this mod's files this time, so it could not be checked. Try again later.".into(),
            },
        };
    };
    // A joined link keeps the group the player's word gave it (ruling 10): a pin copies the
    // listed file's group, which v1's file list leaves out, and the join would dissolve under
    // a source that still reads `joined`. Only `Take it back` parts it.
    let pinned = rift_nexus::pin_link(link, files).map(|pin| match link.source {
        LinkSource::Joined => NexusLink {
            group_id: link.group_id,
            ..pin
        },
        _ => pin,
    });
    let effective = pinned.as_ref().unwrap_or(link);
    let state = rift_nexus::update_state(effective, mod_info, files);
    CheckOutcome {
        mod_id: link.mod_id,
        pinned,
        mod_name,
        state: (&state).into(),
    }
}

/// A Nexus failure in the shared vocabulary: rift-nexus names the reason
/// (offline, rate limited, …) the window voices it by.
pub fn nexus_error(err: NexusError) -> rift_core::RiftError {
    err.into()
}

/// The v1 budget as the window shows it; `None` until a v1 answer carried all
/// four numbers.
pub fn quota_view(quota: &V1Quota) -> Option<NexusQuota> {
    Some(NexusQuota {
        hourly_remaining: quota.hourly_remaining?,
        hourly_limit: quota.hourly_limit?,
        daily_remaining: quota.daily_remaining?,
        daily_limit: quota.daily_limit?,
        hourly_reset_ms: quota
            .hourly_reset
            .as_deref()
            .and_then(|stamp| chrono::DateTime::parse_from_rfc3339(stamp.trim()).ok())
            .and_then(|at| u64::try_from(at.timestamp_millis()).ok()),
    })
}

/// Is the launch auto-check due?
pub fn auto_check_due(last_check_ms: Option<u64>, now_ms: u64) -> bool {
    last_check_ms.is_none_or(|last| now_ms.saturating_sub(last) >= AUTO_CHECK_AFTER_MS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rift_core::{LinkSource, PackageFormat, PackageId};
    use rift_nexus::{FileCategory, NexusFile};
    use std::path::PathBuf;

    fn package(nexus: Option<NexusLink>) -> Package {
        Package {
            id: PackageId("00000000000000aa".into()),
            path: PathBuf::from("/mods/Vernworth Cloaks 2207 8 2026-10-14T14-23Z Rk4vLmN2Q.zip"),
            file_name: "Vernworth Cloaks 2207 8 2026-10-14T14-23Z Rk4vLmN2Q.zip".into(),
            display_name: "Vernworth Cloaks".into(),
            format: PackageFormat::Zip,
            size: 10,
            modified_ms: 1,
            entry_count: 1,
            options: vec![],
            nexus,
            problems: vec![],
        }
    }

    fn filename_link() -> NexusLink {
        NexusLink {
            mod_id: 2207,
            file_id: None,
            group_id: None,
            sqid: Some("Rk4vLmN2Q".into()),
            version: Some("8".into()),
            uploaded_ms: Some(1_791_987_780_000),
            source: LinkSource::Filename,
        }
    }

    fn file(
        file_id: u64,
        sqid: &str,
        version: &str,
        uploaded_ms: u64,
        category: FileCategory,
    ) -> NexusFile {
        NexusFile {
            file_id,
            group_id: Some(31),
            sqid: Some(sqid.into()),
            name: "Vernworth Cloaks".into(),
            version: Some(version.into()),
            category,
            uploaded_ms,
            primary: false,
            size_bytes: None,
        }
    }

    fn published() -> HashMap<u64, NexusMod> {
        HashMap::from([(
            2207,
            NexusMod {
                mod_id: 2207,
                name: "Vernworth Cloaks Collection".into(),
                version: Some("1".into()),
                updated_ms: None,
                status: "published".into(),
                category: Some("Armour".into()),
                tags: vec!["Cloaks".into(), "Lore-Friendly".into()],
                adult: false,
            },
        )])
    }

    #[test]
    fn the_catalog_guess_stands_until_the_player_or_a_check_speaks() {
        let mut store = NexusStore::new();
        let pkg = package(Some(filename_link()));
        assert_eq!(store.link_for(&pkg), Some(filename_link()));
        assert_eq!(store.row(&pkg).state, UpdateStateView::NotChecked);
        assert_eq!(store.badge(&pkg), Some(UpdateBadge::Unknown));
        store.unlink(&pkg);
        assert_eq!(store.link_for(&pkg), None);
        assert_eq!(store.row(&pkg).state, UpdateStateView::Unlinked);
        assert_eq!(store.badge(&pkg), None);
        let manual = NexusLink {
            mod_id: 99,
            file_id: None,
            group_id: None,
            sqid: None,
            version: None,
            uploaded_ms: None,
            source: LinkSource::Manual,
        };
        store.set_link(&pkg, manual.clone());
        assert_eq!(store.link_for(&pkg), Some(manual));
        let row = store.row(&pkg);
        assert_eq!(
            row.page_url.as_deref(),
            Some("https://www.nexusmods.com/dragonsdogma2/mods/99")
        );
        assert_eq!(row.link.unwrap().source, dto::LinkSource::Manual);
    }

    #[test]
    fn a_check_pins_the_file_and_finds_the_newer_one_in_its_group() {
        let link = filename_link();
        let report = ModFilesReport {
            files: BTreeMap::from([(
                2207,
                vec![
                    file(
                        2391,
                        "Rk4vLmN2Q",
                        "8",
                        1_791_987_780_000,
                        FileCategory::Archived,
                    ),
                    file(
                        9700,
                        "Zz9yXwV8U",
                        "9",
                        1_792_000_000_000,
                        FileCategory::Main,
                    ),
                ],
            )]),
            refused: BTreeMap::new(),
        };
        let outcome = judge(&link, &published(), &report);
        assert_eq!(outcome.pinned.as_ref().and_then(|l| l.file_id), Some(2391));
        assert_eq!(
            outcome.pinned.as_ref().map(|l| l.source),
            Some(LinkSource::Filename)
        );
        match &outcome.state {
            UpdateStateView::UpdateAvailable { newer } => assert_eq!(newer.file_id, 9700),
            other => panic!("expected an update, got {other:?}"),
        }
        assert_eq!(
            outcome.mod_name.as_deref(),
            Some("Vernworth Cloaks Collection")
        );

        let mut store = NexusStore::new();
        let pkg = package(Some(link));
        store.record_check(&pkg, outcome, 42);
        let row = store.row(&pkg);
        assert_eq!(row.checked_ms, Some(42));
        assert_eq!(
            row.link.unwrap().file_id,
            Some(2391),
            "the pin is remembered"
        );
        assert_eq!(store.badge(&pkg), Some(UpdateBadge::Available));
    }

    #[test]
    fn a_check_keeps_the_group_a_joined_link_took_by_the_players_word() {
        // A pasted file URL, joined to group 31: its file is listed with no group, as v1 lists.
        let joined = NexusLink {
            mod_id: 2207,
            file_id: Some(2391),
            group_id: Some(31),
            sqid: None,
            version: None,
            uploaded_ms: None,
            source: LinkSource::Joined,
        };
        let mut listed = file(
            2391,
            "Rk4vLmN2Q",
            "8",
            1_791_987_780_000,
            FileCategory::Main,
        );
        listed.group_id = None;
        let report = ModFilesReport {
            files: BTreeMap::from([(2207, vec![listed])]),
            refused: BTreeMap::new(),
        };
        let outcome = judge(&joined, &published(), &report);
        let pinned = outcome
            .pinned
            .clone()
            .expect("the check pins the listed file");
        assert_eq!(pinned.group_id, Some(31), "the join stands");
        assert_eq!(pinned.source, LinkSource::Joined);
        assert_eq!(
            pinned.uploaded_ms,
            Some(1_791_987_780_000),
            "the pin still adds"
        );

        let mut store = NexusStore::new();
        let pkg = package(None);
        store.set_link(&pkg, joined);
        store.record_check(&pkg, outcome, 42);
        assert_eq!(store.link_for(&pkg).and_then(|l| l.group_id), Some(31));

        // any other link takes the listed file's group, as before
        let manual = NexusLink {
            source: LinkSource::Manual,
            ..pinned
        };
        let outcome = judge(&manual, &published(), &report);
        assert_eq!(outcome.pinned.and_then(|l| l.group_id), None);
    }

    #[test]
    fn unlisted_and_refused_mods_are_could_not_check_never_no_files() {
        let link = filename_link();
        let silent = judge(&link, &published(), &ModFilesReport::default());
        assert!(matches!(silent.state, UpdateStateView::Error { .. }));
        let refused = ModFilesReport {
            files: BTreeMap::new(),
            refused: BTreeMap::from([(2207, NexusError::BadResponse("boom".into()))]),
        };
        match judge(&link, &published(), &refused).state {
            UpdateStateView::Error { message } => assert!(message.contains("boom")),
            other => panic!("expected an error row, got {other:?}"),
        }
    }

    #[test]
    fn a_relink_to_another_mod_forgets_the_old_verdict() {
        let mut store = NexusStore::new();
        let pkg = package(Some(filename_link()));
        store.record_check(
            &pkg,
            CheckOutcome {
                mod_id: 2207,
                pinned: None,
                mod_name: Some("Vernworth Cloaks Collection".into()),
                state: UpdateStateView::Current,
            },
            7,
        );
        assert_eq!(store.row(&pkg).state, UpdateStateView::Current);
        store.set_link(
            &pkg,
            NexusLink {
                mod_id: 5,
                file_id: None,
                group_id: None,
                sqid: None,
                version: None,
                uploaded_ms: None,
                source: LinkSource::Manual,
            },
        );
        let row = store.row(&pkg);
        assert_eq!(row.state, UpdateStateView::NotChecked);
        assert_eq!(row.mod_name, None);
        assert_eq!(row.checked_ms, None);
    }

    #[test]
    fn the_quota_needs_all_four_numbers_and_parses_the_reset() {
        assert_eq!(quota_view(&V1Quota::default()), None);
        let quota = V1Quota {
            daily_limit: Some(20_000),
            daily_remaining: Some(19_840),
            daily_reset: None,
            hourly_limit: Some(500),
            hourly_remaining: Some(486),
            hourly_reset: Some("2026-10-16T12:00:00+00:00".into()),
        };
        let view = quota_view(&quota).unwrap();
        assert_eq!(view.hourly_remaining, 486);
        assert_eq!(view.daily_limit, 20_000);
        assert_eq!(view.hourly_reset_ms, Some(1_792_152_000_000));
    }

    #[test]
    fn the_auto_check_waits_twelve_hours() {
        assert!(auto_check_due(None, 0));
        assert!(!auto_check_due(
            Some(1_000),
            1_000 + AUTO_CHECK_AFTER_MS - 1
        ));
        assert!(auto_check_due(Some(1_000), 1_000 + AUTO_CHECK_AFTER_MS));
    }

    fn manual(mod_id: u64) -> NexusLink {
        NexusLink {
            mod_id,
            file_id: None,
            group_id: None,
            sqid: None,
            version: None,
            uploaded_ms: None,
            source: LinkSource::Manual,
        }
    }

    /// Another download, unlinked (no Nexus file name).
    fn other_package() -> Package {
        Package {
            id: PackageId("00000000000000bb".into()),
            file_name: "Lantern Glow Recolor.zip".into(),
            display_name: "Lantern Glow Recolor".into(),
            ..package(None)
        }
    }

    fn mod_answer(mod_id: u64, category: Option<&str>, tags: &[&str], adult: bool) -> NexusMod {
        NexusMod {
            mod_id,
            name: format!("Mod {mod_id}"),
            status: "published".into(),
            category: category.map(str::to_string),
            tags: tags.iter().map(|t| (*t).to_string()).collect(),
            adult,
            ..NexusMod::default()
        }
    }

    #[test]
    fn labels_belong_to_the_mod_and_follow_whichever_link_a_package_has_now() {
        let mut store = NexusStore::new();
        let cloaks = package(Some(filename_link()));
        let lantern = other_package();
        assert_eq!(store.labels_for(&cloaks), None, "no check has run");
        assert!(store
            .package_labels(std::slice::from_ref(&cloaks))
            .is_empty());

        store.record_labels(&[2207], &published());
        let heard = store
            .labels_for(&cloaks)
            .expect("the check heard of mod 2207");
        assert_eq!(heard.category.as_deref(), Some("Armour"));
        assert_eq!(heard.tags, vec!["Cloaks", "Lore-Friendly"]);
        assert!(!heard.adult);

        // A second package linked by hand to the same mod is labeled at once:
        // the labels are the mod's, and the check already heard them.
        store.set_link(&lantern, manual(2207));
        let labels = store.package_labels(&[cloaks.clone(), lantern.clone()]);
        assert_eq!(labels.len(), 2);
        assert_eq!(
            labels[&lantern.id],
            PackageLabels {
                category: Some("Armour".into()),
                tags: vec!["Cloaks".into(), "Lore-Friendly".into()],
                adult: false,
            }
        );

        // Unlinked: no labels. Re-linked to a mod no check has heard of: none.
        store.unlink(&cloaks);
        assert_eq!(store.labels_for(&cloaks), None);
        store.set_link(&lantern, manual(5150));
        assert_eq!(store.labels_for(&lantern), None);
        assert!(store
            .package_labels(&[cloaks.clone(), lantern.clone()])
            .is_empty());
    }

    #[test]
    fn a_check_keeps_the_labels_of_the_mods_it_asked_for_and_forgets_the_rest() {
        let mut store = NexusStore::new();
        store.record_labels(
            &[2207, 99],
            &HashMap::from([
                (2207, mod_answer(2207, Some("Armour"), &["Cloaks"], false)),
                (99, mod_answer(99, Some("Visuals"), &[], true)),
            ]),
        );
        assert_eq!(store.mod_labels.len(), 2);

        // The next check: 2207 is still linked but Nexus left it out of its
        // answer; 99 is linked no more; 4242 is new; 6006 answered without
        // any label at all (an answer shape that does not carry them).
        store.record_labels(
            &[2207, 4242, 6006],
            &HashMap::from([
                (
                    4242,
                    mod_answer(4242, Some("Visuals"), &["Utilities for Players"], false),
                ),
                (6006, mod_answer(6006, None, &[], false)),
            ]),
        );
        let ids: Vec<u64> = store.mod_labels.keys().copied().collect();
        assert_eq!(
            ids,
            vec![2207, 4242],
            "unanswered keeps, unlinked goes, blank is no labels"
        );
        assert_eq!(store.mod_labels[&2207].tags, vec!["Cloaks"]);

        // An answer carrying only the adult flag still veils.
        store.record_labels(&[7], &HashMap::from([(7, mod_answer(7, None, &[], true))]));
        assert!(store.mod_labels[&7].adult);
    }

    #[test]
    fn a_check_keeps_the_labels_of_a_linked_package_that_is_off_the_shelf() {
        // The cloaks were linked and checked; then their root went out of the
        // library (removed, or unreadable at launch), so the next check asked
        // Nexus about nothing — the shelf held no linked package at all.
        let mut store = NexusStore::new();
        let cloaks = package(Some(filename_link()));
        store.record_check(
            &cloaks,
            CheckOutcome {
                mod_id: 2207,
                pinned: None,
                mod_name: Some("Vernworth Cloaks Collection".into()),
                state: UpdateStateView::Current,
            },
            7,
        );
        store.record_labels(&[2207], &published());
        store.record_labels(&[], &HashMap::new());
        assert_eq!(
            store
                .labels_for(&cloaks)
                .and_then(|l| l.category.as_deref()),
            Some("Armour"),
            "the verdict survives the check, and so do the labels"
        );
        assert_eq!(store.row(&cloaks).state, UpdateStateView::Current);

        // A link made by hand names its mod even before any check judged it.
        let lantern = other_package();
        store.set_link(&lantern, manual(4242));
        store.record_labels(
            &[4242],
            &HashMap::from([(
                4242,
                mod_answer(4242, Some("Visuals"), &["Utilities for Players"], false),
            )]),
        );
        store.record_labels(&[], &HashMap::new());
        assert!(store.labels_for(&lantern).is_some(), "named by its link");

        // Only an unlink (or a re-link elsewhere) lets a mod's labels go.
        store.unlink(&cloaks);
        store.set_link(&lantern, manual(5150));
        store.record_labels(&[], &HashMap::new());
        assert!(
            store.mod_labels.is_empty(),
            "nothing names 2207 or 4242 any more: {:?}",
            store.mod_labels
        );
    }

    #[test]
    fn a_v01_nexus_file_loads_unlabeled_and_labels_survive_a_save() {
        let v01 = serde_json::json!({
            "schema": 1,
            "lastCheckMs": 42,
            "packages": { "00000000000000aa": { "link": null, "unlinked": true } }
        });
        let store: NexusStore = serde_json::from_value(v01).unwrap();
        assert!(store.mod_labels.is_empty());
        assert!(store.packages["00000000000000aa"].unlinked);

        let mut store = NexusStore::new();
        store.record_labels(&[2207], &published());
        let saved = serde_json::to_value(&store).unwrap();
        assert_eq!(saved["modLabels"]["2207"]["category"], "Armour");
        assert_eq!(
            serde_json::from_value::<NexusStore>(saved).unwrap(),
            store,
            "nexus.json keeps the labels between launches"
        );
    }

    #[test]
    fn rate_limits_carry_the_reason_the_window_voices() {
        let err = nexus_error(NexusError::RateLimited {
            retry_after_s: Some(30),
        });
        let rift_core::RiftError::Nexus { reason, .. } = &err else {
            panic!("a Nexus failure stays a Nexus failure");
        };
        assert_eq!(*reason, Some(rift_core::NexusReason::RateLimited));
        let wire = serde_json::to_value(&err).unwrap();
        assert_eq!(wire["detail"]["reason"], "rateLimited");
    }

    #[test]
    fn a_record_follows_its_package_onto_a_free_id_only() {
        let mut store = NexusStore::new();
        let old = PackageId("00000000000000aa".into());
        let new = PackageId("00000000000000bb".into());
        let taken = PackageId("00000000000000cc".into());
        let elsewhere = PackageId("00000000000000dd".into());
        store.packages.insert(
            old.0.clone(),
            LinkRecord {
                link: Some(manual(2207)),
                ..LinkRecord::default()
            },
        );
        store.packages.insert(
            taken.0.clone(),
            LinkRecord {
                unlinked: true,
                ..LinkRecord::default()
            },
        );
        store.packages.insert(
            elsewhere.0.clone(),
            LinkRecord {
                link: Some(manual(5150)),
                ..LinkRecord::default()
            },
        );
        assert!(!store.move_records(&BTreeMap::new()));
        // Onto an id with a record of its own: nothing is merged or lost.
        assert!(!store.move_records(&BTreeMap::from([(elsewhere.clone(), taken.clone())])));
        assert!(store.packages[&taken.0].unlinked);
        assert_eq!(store.packages[&elsewhere.0].link, Some(manual(5150)));
        // Onto a free id: the record moves whole.
        assert!(store.move_records(&BTreeMap::from([(old.clone(), new.clone())])));
        assert!(!store.packages.contains_key(&old.0));
        assert_eq!(store.packages[&new.0].link, Some(manual(2207)));
        assert!(
            !store.move_records(&BTreeMap::from([(old, new)])),
            "a second pass finds nothing to move"
        );
    }
}
