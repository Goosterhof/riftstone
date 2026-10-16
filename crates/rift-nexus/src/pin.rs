//! Pinning a package to its exact Nexus file, and deciding whether a newer one
//! exists.
//!
//! A download name says which mod and roughly which file; Nexus's file list
//! says what exists now. [`pin_link`] finds the local file in that list, and
//! [`update_state`] looks for a newer file of the SAME logical file: same
//! `group_id` (every version of one file shares it), falling back to the same
//! file name when Nexus gives no group. Mod-level version strings are never
//! compared — authors forget to bump them.

use rift_core::NexusLink;
use serde::{Deserialize, Serialize};

use crate::types::{FileCategory, NexusFile, NexusMod};

/// The status [`update_state`] reports when Nexus knows neither the mod nor a
/// single file of it (deleted outright, or never existed).
pub const STATUS_NOT_FOUND: &str = "not_found";

/// How far a download name's upload stamp may sit from a file's upload time
/// and still pin it (§ rift-nexus: ±60 s).
const TIMESTAMP_TOLERANCE_MS: u64 = 60_000;
const MINUTE_MS: u64 = 60_000;

/// One package's update state. Serializes `{ "kind": "updateAvailable", "newer": {…} }`,
/// the shape of the window's `UpdateStateView`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UpdateState {
    /// No newer live file in the local file's group.
    Current,
    /// The newest live (not removed / archived / old-version) file in the same
    /// group, uploaded after the local one. Wins over `LocalArchived`: authors
    /// archive the old file when they upload its successor.
    UpdateAvailable { newer: NexusFile },
    /// The author retired the local file — removed, archived, marked as an old
    /// version, or no longer listed — and nothing in its group replaces it.
    /// `newer`, when set, is the mod's current primary (else main) file
    /// uploaded after the local one: a suggestion, not a same-group successor.
    LocalArchived { newer: Option<NexusFile> },
    /// The mod is known, the exact local file is not (see [`pin_link`]).
    Unpinned,
    /// The mod page is hidden, removed, under moderation — or gone
    /// ([`STATUS_NOT_FOUND`]).
    ModUnavailable { status: String },
}

/// Pin a package to its exact Nexus file. Fills `file_id`, `group_id`,
/// `uploaded_ms`, `version` and `sqid`; keeps `mod_id` and `source`.
///
/// **Exact identifiers first, and they are final.** The `sqid` (2026 download
/// names, MD5 hits, earlier pins) comes first, then the `file_id` (MD5 hits,
/// pasted file URLs, earlier pins). An identifier that names a listed file
/// pins it; the sqid wins if the two disagree. An identifier the list does not
/// hold means the file is gone, and no heuristic may then pick a neighbour:
///
/// - a link that already carries its `file_id` comes back unchanged, still
///   pinned to the gone file, and [`update_state`] reports it retired using the
///   group the link remembers;
/// - a link with only a sqid, or whose sqid is gone while its file id names a
///   listed file (it contradicts itself), gets `None`.
///
/// A sqid counts only against a list that carries sqids (v2's `modFiles` does,
/// v1's `files.json` does not).
///
/// **Only a link with no exact identifier reaches the heuristics:** the upload
/// stamp within ±60 s, with the version as tie-break, then (only for names
/// with no stamp at all) the version string. Several files at that version
/// resolve to the primary one, else to the one main file. A classic stamp is
/// the exact second. A 2026 stamp is the minute, truncated (the link carries a
/// sqid and the stamp has no seconds), so every second of that minute ranks
/// equal.
///
/// `None` when nothing matches or two candidates tie. The window then offers
/// MD5 lookup or a pasted URL.
pub fn pin_link(link: &NexusLink, files: &[NexusFile]) -> Option<NexusLink> {
    let file = match exact_file(link, files) {
        Exact::Listed(file) => file,
        Exact::Gone => return link.file_id.is_some().then(|| link.clone()),
        Exact::Contradiction => return None,
        Exact::Silent => by_upload_time(link, files).or_else(|| by_version(link, files))?,
    };
    Some(NexusLink {
        mod_id: link.mod_id,
        file_id: Some(file.file_id),
        group_id: file.group_id,
        sqid: file.sqid.clone().or_else(|| link.sqid.clone()),
        version: file.version.clone().or_else(|| link.version.clone()),
        uploaded_ms: Some(file.uploaded_ms)
            .filter(|ms| *ms > 0)
            .or(link.uploaded_ms),
        source: link.source,
    })
}

/// Decide one package's update state from its pinned link, the mod (when
/// looked up) and the mod's current file list (as [`crate::NexusClient::mod_files`]
/// returned it — pass only a list that was actually fetched).
///
/// The rule, exactly: within the local file's group (same `group_id`; no group
/// id → same name, case-insensitive), ignore removed, archived and old-version
/// files; the newest by upload time that is newer than the local file — or
/// equally old with a higher `file_id` — is the update.
pub fn update_state(
    link: &NexusLink,
    mod_info: Option<&NexusMod>,
    files: &[NexusFile],
) -> UpdateState {
    match mod_info {
        Some(nexus_mod) if !nexus_mod.is_available() => {
            return UpdateState::ModUnavailable {
                status: nexus_mod.status.clone(),
            }
        }
        None if files.is_empty() => {
            return UpdateState::ModUnavailable {
                status: STATUS_NOT_FOUND.to_string(),
            }
        }
        _ => {}
    }
    let Some(file_id) = link.file_id else {
        return UpdateState::Unpinned;
    };
    let local = files.iter().find(|file| file.file_id == file_id);
    let reference = match local {
        Some(file) => Reference {
            file_id: file.file_id,
            uploaded_ms: file.uploaded_ms,
            group: match file.group_id {
                Some(group_id) => Group::Id(group_id),
                None => Group::Name(file.name.trim().to_lowercase()),
            },
        },
        // The local file is no longer listed: fall back to what the link
        // remembers from when it was pinned.
        None => Reference {
            file_id,
            uploaded_ms: link.uploaded_ms.unwrap_or(0),
            group: link.group_id.map_or(Group::Unknown, Group::Id),
        },
    };
    if let Some(newer) = newest_after(files, &reference, |file| reference.group.holds(file)) {
        return UpdateState::UpdateAvailable {
            newer: newer.clone(),
        };
    }
    if local.is_none_or(|file| file.category.is_retired()) {
        let suggestion = newest_after(files, &reference, |file| file.primary).or_else(|| {
            newest_after(files, &reference, |file| {
                file.category == FileCategory::Main
            })
        });
        return UpdateState::LocalArchived {
            newer: suggestion.cloned(),
        };
    }
    UpdateState::Current
}

struct Reference {
    file_id: u64,
    uploaded_ms: u64,
    group: Group,
}

enum Group {
    Id(u64),
    /// Lower-cased, trimmed file name.
    Name(String),
    Unknown,
}

impl Group {
    fn holds(&self, file: &NexusFile) -> bool {
        match self {
            Group::Id(group_id) => file.group_id == Some(*group_id),
            Group::Name(name) => file.name.trim().to_lowercase() == *name,
            Group::Unknown => false,
        }
    }
}

/// The newest live file in scope that is newer than the reference file
/// (later upload, or the same upload time and a higher file id).
fn newest_after<'a>(
    files: &'a [NexusFile],
    reference: &Reference,
    in_scope: impl Fn(&NexusFile) -> bool,
) -> Option<&'a NexusFile> {
    files
        .iter()
        .filter(|file| file.file_id != reference.file_id && !file.category.is_retired())
        .filter(|file| {
            (file.uploaded_ms, file.file_id) > (reference.uploaded_ms, reference.file_id)
        })
        .filter(|file| in_scope(file))
        .max_by_key(|file| (file.uploaded_ms, file.file_id))
}

/// What a link's exact identifiers say about a file list.
enum Exact<'a> {
    /// The link carries no identifier this list can check.
    Silent,
    /// The link names this listed file.
    Listed(&'a NexusFile),
    /// The link names a file the list no longer holds.
    Gone,
    /// The sqid names a file that is gone while the file id names a listed
    /// one: the link contradicts itself.
    Contradiction,
}

fn exact_file<'a>(link: &NexusLink, files: &'a [NexusFile]) -> Exact<'a> {
    // A list that carries ANY sqid speaks sqids, so a sqid it does not hold is
    // gone. Were one listed file missing its sqid, it would read as gone too.
    // That fails safe: the package stays unpinned, it is never pinned to a
    // neighbour. A list with no sqids at all (v1's) cannot check one.
    let by_sqid = link_sqid(link)
        .filter(|_| files.iter().any(|file| file.sqid.is_some()))
        .map(|sqid| files.iter().find(|file| file.sqid.as_deref() == Some(sqid)));
    let by_file_id = link
        .file_id
        .map(|file_id| files.iter().find(|file| file.file_id == file_id));
    match (by_sqid, by_file_id) {
        (Some(Some(file)), _) | (None, Some(Some(file))) => Exact::Listed(file),
        (Some(None), Some(Some(_))) => Exact::Contradiction,
        (Some(None), _) | (None, Some(None)) => Exact::Gone,
        (None, None) => Exact::Silent,
    }
}

fn link_sqid(link: &NexusLink) -> Option<&str> {
    link.sqid
        .as_deref()
        .map(str::trim)
        .filter(|sqid| !sqid.is_empty())
}

/// How far a file's upload time sits from a download name's stamp, or `None`
/// beyond the ±60 s tolerance. With `minute` precision (a 2026 name) the true
/// upload lies in `[stamp, stamp + 60 s)`: any second of that minute is
/// distance 0, and the window still closes 60 s after the stamp.
fn upload_distance_ms(stamp_ms: u64, file_ms: u64, minute: bool) -> Option<u64> {
    if file_ms.abs_diff(stamp_ms) > TIMESTAMP_TOLERANCE_MS {
        return None;
    }
    Some(if minute && file_ms >= stamp_ms {
        file_ms.saturating_sub(stamp_ms + MINUTE_MS - 1)
    } else {
        file_ms.abs_diff(stamp_ms)
    })
}

fn by_upload_time<'a>(link: &NexusLink, files: &'a [NexusFile]) -> Option<&'a NexusFile> {
    let stamp_ms = link.uploaded_ms?;
    // A 2026 name always carries a sqid, a classic name never does. The 2026
    // stamp is the minute, unless it spelled out its seconds.
    let minute = link_sqid(link).is_some() && stamp_ms % MINUTE_MS == 0;
    let wanted = link.version.as_deref().map(version_key);
    let candidates = files.iter().filter_map(|file| {
        let distance = upload_distance_ms(stamp_ms, file.uploaded_ms, minute)?;
        let version_differs = wanted.as_ref().is_some_and(|wanted| {
            file.version.as_deref().map(version_key).as_ref() != Some(wanted)
        });
        Some((file, (version_differs, distance)))
    });
    unique_best(candidates)
}

fn by_version<'a>(link: &NexusLink, files: &'a [NexusFile]) -> Option<&'a NexusFile> {
    if link.uploaded_ms.is_some() {
        // A timestamp that matched nothing names a file that is no longer
        // listed; a version match would pin the wrong one.
        return None;
    }
    let wanted = version_key(link.version.as_deref()?);
    if wanted.is_empty() {
        return None;
    }
    let same: Vec<&NexusFile> = files
        .iter()
        .filter(|file| file.version.as_deref().map(version_key).as_deref() == Some(wanted.as_str()))
        .collect();
    if let [only] = same.as_slice() {
        return Some(only);
    }
    let primary: Vec<&NexusFile> = same.iter().copied().filter(|file| file.primary).collect();
    if let [only] = primary.as_slice() {
        return Some(only);
    }
    let main: Vec<&NexusFile> = same
        .iter()
        .copied()
        .filter(|file| file.category == FileCategory::Main)
        .collect();
    match main.as_slice() {
        [only] => Some(only),
        _ => None,
    }
}

/// The single best-ranked candidate; `None` on a tie.
fn unique_best<'a, K: Ord + Copy>(
    candidates: impl Iterator<Item = (&'a NexusFile, K)>,
) -> Option<&'a NexusFile> {
    let mut best: Option<(&NexusFile, K)> = None;
    let mut tied = false;
    for (file, rank) in candidates {
        match best {
            Some((_, best_rank)) if rank > best_rank => {}
            Some((_, best_rank)) if rank == best_rank => tied = true,
            _ => {
                best = Some((file, rank));
                tied = false;
            }
        }
    }
    if tied {
        None
    } else {
        best.map(|(file, _)| file)
    }
}

/// `v1.2`, `1-2`, `1_2` and `1.2` are one version; so are `Beta` and `beta`.
fn version_key(version: &str) -> String {
    let lower = version.trim().to_lowercase();
    let bare = match lower.strip_prefix('v') {
        Some(rest) if rest.starts_with(|c: char| c.is_ascii_digit()) => rest,
        _ => lower.as_str(),
    };
    bare.chars()
        .map(|c| if matches!(c, '-' | '_' | ' ') { '.' } else { c })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rift_core::LinkSource;

    const MINUTE: u64 = 60_000;
    /// 2026-10-14T14:23:00Z — minute-aligned, like a 2026 download name.
    const T0: u64 = 1_791_987_780_000;

    fn file(
        file_id: u64,
        group: Option<u64>,
        version: &str,
        category: FileCategory,
        uploaded_ms: u64,
    ) -> NexusFile {
        NexusFile {
            file_id,
            group_id: group,
            sqid: Some(format!("sq{file_id}")),
            name: "Lantern Glow Recolor".into(),
            version: Some(version.into()),
            category,
            uploaded_ms,
            primary: false,
            size_bytes: Some(1000 + file_id),
        }
    }

    fn link() -> NexusLink {
        NexusLink {
            mod_id: 4242,
            file_id: None,
            group_id: None,
            sqid: None,
            version: None,
            uploaded_ms: None,
            source: LinkSource::Filename,
        }
    }

    fn pinned_to(file_id: u64) -> NexusLink {
        NexusLink {
            file_id: Some(file_id),
            ..link()
        }
    }

    fn published() -> NexusMod {
        NexusMod {
            mod_id: 4242,
            name: "Vernworth Cloaks Collection".into(),
            version: Some("1".into()),
            updated_ms: Some(T0),
            status: "published".into(),
            ..NexusMod::default()
        }
    }

    // -- pin_link ------------------------------------------------------------

    #[test]
    fn pins_by_sqid_first() {
        let files = vec![
            file(10, Some(1), "7", FileCategory::Archived, T0 - 9 * MINUTE),
            file(11, Some(1), "8", FileCategory::Main, T0 + 40_000),
        ];
        let local = NexusLink {
            sqid: Some("sq10".into()),
            // A timestamp pointing at file 11 must lose to the exact sqid.
            uploaded_ms: Some(T0),
            version: Some("8".into()),
            ..link()
        };
        let pinned = pin_link(&local, &files).unwrap();
        assert_eq!(pinned.file_id, Some(10));
        assert_eq!(pinned.group_id, Some(1));
        assert_eq!(
            pinned.version.as_deref(),
            Some("7"),
            "the file's own version wins"
        );
        assert_eq!(pinned.uploaded_ms, Some(T0 - 9 * MINUTE));
        assert_eq!(pinned.source, LinkSource::Filename);
        assert_eq!(pinned.mod_id, 4242);
    }

    /// A file as v1's `files.json` lists it: no sqid.
    fn v1_file(file_id: u64, version: &str, category: FileCategory, uploaded_ms: u64) -> NexusFile {
        NexusFile {
            sqid: None,
            ..file(file_id, None, version, category, uploaded_ms)
        }
    }

    /// A 2026 download name: its sqid and its minute stamp.
    fn name_2026(stamp_ms: u64) -> NexusLink {
        NexusLink {
            sqid: Some("Rk4vLmN2Q".into()),
            uploaded_ms: Some(stamp_ms),
            ..link()
        }
    }

    #[test]
    fn pins_a_2026_minute_stamp_to_any_second_of_that_minute() {
        // Against a list without sqids the 2026 name falls back on its stamp.
        // The true file landed 40 s into the minute; a neighbour 10 s before the
        // minute is closer to the raw stamp but outside the minute.
        let files = vec![
            v1_file(20, "8", FileCategory::Main, T0 + 40_000),
            v1_file(21, "8", FileCategory::Optional, T0 - 10_000),
        ];
        assert_eq!(pin_link(&name_2026(T0), &files).unwrap().file_id, Some(20));
        // A classic stamp is the exact second: the same list pins the closer file.
        let classic = NexusLink {
            uploaded_ms: Some(T0),
            ..link()
        };
        assert_eq!(pin_link(&classic, &files).unwrap().file_id, Some(21));
    }

    #[test]
    fn the_2026_window_closes_sixty_seconds_after_the_stamp() {
        let late = vec![v1_file(22, "8", FileCategory::Main, T0 + 70_000)];
        assert_eq!(
            pin_link(&name_2026(T0), &late),
            None,
            "a file 70 s after the stamped minute began is not in it"
        );
        let last_second = vec![v1_file(23, "8", FileCategory::Main, T0 + 59_000)];
        assert_eq!(
            pin_link(&name_2026(T0), &last_second).unwrap().file_id,
            Some(23)
        );
        let early = vec![v1_file(24, "8", FileCategory::Main, T0 - 60_000)];
        assert_eq!(
            pin_link(&name_2026(T0), &early).unwrap().file_id,
            Some(24),
            "±60 s still holds before the stamp"
        );
        // A 2026 stamp that spelled out its seconds is the exact second.
        let seconds = name_2026(T0 + 30_000);
        let files = vec![
            v1_file(25, "8", FileCategory::Main, T0 + 31_000),
            v1_file(26, "8", FileCategory::Main, T0 + 50_000),
        ];
        assert_eq!(pin_link(&seconds, &files).unwrap().file_id, Some(25));
    }

    #[test]
    fn a_classic_stamp_on_a_whole_minute_keeps_sixty_seconds() {
        // A unix-seconds stamp that happens to land on :00 is still a second,
        // not a minute (one classic name in sixty).
        let stamp = 1_711_234_560_000;
        let files = vec![file(
            21,
            Some(4),
            "1.2",
            FileCategory::Main,
            stamp + 100_000,
        )];
        let local = NexusLink {
            uploaded_ms: Some(stamp),
            version: Some("1.2".into()),
            ..link()
        };
        assert_eq!(pin_link(&local, &files), None, "100 s is not ±60 s");
        let near = vec![file(22, Some(4), "1.2", FileCategory::Main, stamp + 50_000)];
        assert_eq!(pin_link(&local, &near).unwrap().file_id, Some(22));
    }

    #[test]
    fn a_2026_sqid_the_list_does_not_hold_pins_nothing() {
        // The list carries sqids, and not this one: the named file is gone.
        // No timestamp may then pick a neighbour, however close.
        let local = NexusLink {
            version: Some("8".into()),
            ..name_2026(T0)
        };
        let different = vec![file(90, Some(3), "2", FileCategory::Optional, T0 + 110_000)];
        assert_eq!(pin_link(&local, &different), None);
        let same_minute = vec![file(91, Some(3), "8", FileCategory::Main, T0 + 30_000)];
        assert_eq!(
            pin_link(&local, &same_minute),
            None,
            "a re-upload in the same minute is another file: its own sqid"
        );
    }

    #[test]
    fn a_link_whose_exact_file_was_deleted_stays_pinned_to_it() {
        // An MD5 link to file 41 (group 4). The author deleted 41; an optional
        // file of another group went up 20 s later.
        let t41 = 1_711_234_567_000;
        let files = vec![file(42, Some(9), "1", FileCategory::Optional, t41 + 20_000)];
        let md5_link = NexusLink {
            mod_id: 4242,
            file_id: Some(41),
            group_id: Some(4),
            sqid: Some("sq41".into()),
            version: Some("8".into()),
            uploaded_ms: Some(t41),
            source: LinkSource::Md5,
        };
        let repinned = pin_link(&md5_link, &files);
        assert_eq!(
            repinned.as_ref(),
            Some(&md5_link),
            "an exact identifier is final"
        );
        let retired = UpdateState::LocalArchived { newer: None };
        assert_eq!(update_state(&md5_link, Some(&published()), &files), retired);
        assert_eq!(
            update_state(&repinned.unwrap(), Some(&published()), &files),
            retired,
            "re-pinning never turns a retired file into a current one"
        );
        // The same with a file id alone (a pasted file URL), and with an empty list.
        assert_eq!(pin_link(&pinned_to(41), &files), Some(pinned_to(41)));
        assert_eq!(pin_link(&pinned_to(41), &[]), Some(pinned_to(41)));
        // Its group, remembered, still finds the successor the author uploaded.
        let successor = vec![file(43, Some(4), "9", FileCategory::Main, t41 + MINUTE)];
        match update_state(&md5_link, Some(&published()), &successor) {
            UpdateState::UpdateAvailable { newer } => assert_eq!(newer.file_id, 43),
            other => panic!("expected the same-group successor, got {other:?}"),
        }
    }

    #[test]
    fn the_sqid_outranks_a_file_id_that_disagrees() {
        let files = vec![
            file(31, Some(1), "1", FileCategory::Main, T0),
            file(32, Some(1), "2", FileCategory::Main, T0 + MINUTE),
        ];
        let both_listed = NexusLink {
            file_id: Some(31),
            sqid: Some("sq32".into()),
            ..link()
        };
        assert_eq!(pin_link(&both_listed, &files).unwrap().file_id, Some(32));
        let file_id_gone = NexusLink {
            file_id: Some(99),
            sqid: Some("sq32".into()),
            ..link()
        };
        assert_eq!(pin_link(&file_id_gone, &files).unwrap().file_id, Some(32));
        let sqid_gone = NexusLink {
            file_id: Some(31),
            sqid: Some("sq99".into()),
            ..link()
        };
        assert_eq!(
            pin_link(&sqid_gone, &files),
            None,
            "the sqid says gone, the file id says listed: no guess"
        );
        // A list without sqids cannot check one: the file id decides.
        let v1 = vec![v1_file(31, "1", FileCategory::Main, T0)];
        assert_eq!(pin_link(&sqid_gone, &v1).unwrap().file_id, Some(31));
    }

    #[test]
    fn pins_a_classic_seconds_stamp_within_sixty_seconds() {
        let stamp = 1_711_234_567_000; // classic names carry the exact second
        let files = vec![
            file(30, Some(4), "1.2", FileCategory::Main, stamp + 3_000),
            file(
                31,
                Some(4),
                "1.1",
                FileCategory::OldVersion,
                stamp - 86_400_000,
            ),
        ];
        let local = NexusLink {
            uploaded_ms: Some(stamp),
            version: Some("1.2".into()),
            ..link()
        };
        assert_eq!(pin_link(&local, &files).unwrap().file_id, Some(30));
        let far = NexusLink {
            uploaded_ms: Some(stamp - 5 * MINUTE),
            ..local.clone()
        };
        assert_eq!(
            pin_link(&far, &files),
            None,
            "61 s+ away is not the same file"
        );
    }

    #[test]
    fn a_timestamp_tie_is_broken_by_version_or_refused() {
        let stamp = 1_711_234_567_000;
        let files = vec![
            file(40, Some(5), "2", FileCategory::Main, stamp + 5_000),
            file(
                41,
                Some(6),
                "2-optional",
                FileCategory::Optional,
                stamp + 5_000,
            ),
        ];
        let with_version = NexusLink {
            uploaded_ms: Some(stamp),
            version: Some("2".into()),
            ..link()
        };
        assert_eq!(pin_link(&with_version, &files).unwrap().file_id, Some(40));
        let without_version = NexusLink {
            uploaded_ms: Some(stamp),
            ..link()
        };
        assert_eq!(
            pin_link(&without_version, &files),
            None,
            "two equal candidates: no guess"
        );
    }

    #[test]
    fn pins_by_version_only_when_the_name_had_no_timestamp() {
        let files = vec![
            file(50, Some(7), "1.3", FileCategory::Main, T0),
            file(
                51,
                Some(7),
                "1.2",
                FileCategory::OldVersion,
                T0 - 99 * MINUTE,
            ),
        ];
        let old_name = NexusLink {
            version: Some("1-3".into()),
            ..link()
        };
        assert_eq!(
            pin_link(&old_name, &files).unwrap().file_id,
            Some(50),
            "1-3 is 1.3"
        );
        let v_prefixed = NexusLink {
            version: Some("v1.2".into()),
            ..link()
        };
        assert_eq!(pin_link(&v_prefixed, &files).unwrap().file_id, Some(51));
        let stamped = NexusLink {
            version: Some("1.3".into()),
            uploaded_ms: Some(T0 - 500 * MINUTE),
            ..link()
        };
        assert_eq!(
            pin_link(&stamped, &files),
            None,
            "a stamp that matches nothing is final"
        );
    }

    #[test]
    fn several_files_at_one_version_resolve_to_the_primary_or_main_one() {
        let mut main = file(60, Some(8), "2.0", FileCategory::Main, T0);
        let optional = file(61, Some(9), "2.0", FileCategory::Optional, T0);
        let local = NexusLink {
            version: Some("2.0".into()),
            ..link()
        };
        assert_eq!(
            pin_link(&local, &[main.clone(), optional.clone()])
                .unwrap()
                .file_id,
            Some(60)
        );
        main.category = FileCategory::Optional;
        let mut primary = optional.clone();
        primary.file_id = 62;
        primary.primary = true;
        assert_eq!(
            pin_link(&local, &[main.clone(), primary]).unwrap().file_id,
            Some(62)
        );
        assert_eq!(
            pin_link(&local, &[main, optional]),
            None,
            "no primary, no single main"
        );
    }

    #[test]
    fn a_known_file_id_pins_directly_and_nothing_pins_to_nothing() {
        let files = vec![file(70, Some(10), "3", FileCategory::Main, T0)];
        let pinned = pin_link(&pinned_to(70), &files).unwrap();
        assert_eq!((pinned.file_id, pinned.group_id), (Some(70), Some(10)));
        assert_eq!(pinned.sqid.as_deref(), Some("sq70"), "filled from the file");
        assert_eq!(
            pin_link(&link(), &files),
            None,
            "a homepage link carries nothing to match"
        );
        assert_eq!(pin_link(&link(), &[]), None);
    }

    // -- update_state -----------------------------------------------------------

    #[test]
    fn a_newer_file_in_the_same_group_is_the_update() {
        let files = vec![
            file(100, Some(1), "7", FileCategory::Main, T0),
            file(101, Some(1), "8", FileCategory::Main, T0 + MINUTE),
            file(
                102,
                Some(1),
                "9-beta",
                FileCategory::Archived,
                T0 + 2 * MINUTE,
            ),
        ];
        match update_state(&pinned_to(100), Some(&published()), &files) {
            UpdateState::UpdateAvailable { newer } => {
                assert_eq!(newer.file_id, 101, "archived 102 is ignored")
            }
            other => panic!("expected an update, got {other:?}"),
        }
        assert_eq!(
            update_state(&pinned_to(101), Some(&published()), &files),
            UpdateState::Current
        );
    }

    #[test]
    fn a_newer_file_in_another_group_is_not_an_update() {
        let files = vec![
            file(110, Some(1), "1", FileCategory::Main, T0),
            file(111, Some(2), "5", FileCategory::Optional, T0 + 9 * MINUTE),
        ];
        assert_eq!(
            update_state(&pinned_to(110), Some(&published()), &files),
            UpdateState::Current
        );
    }

    #[test]
    fn an_archived_local_file_with_a_successor_reports_the_update() {
        // The live pattern: the author archives v7 when v8 goes up.
        let files = vec![
            file(120, Some(1), "7", FileCategory::Archived, T0),
            file(121, Some(1), "8", FileCategory::Main, T0 + MINUTE),
        ];
        match update_state(&pinned_to(120), Some(&published()), &files) {
            UpdateState::UpdateAvailable { newer } => assert_eq!(newer.file_id, 121),
            other => panic!("expected an update, got {other:?}"),
        }
    }

    #[test]
    fn a_retired_local_file_without_a_successor_is_local_archived() {
        let mut other_main = file(131, Some(2), "2", FileCategory::Main, T0 + MINUTE);
        other_main.primary = true;
        let files = vec![
            file(130, Some(1), "1", FileCategory::Archived, T0),
            other_main,
        ];
        match update_state(&pinned_to(130), Some(&published()), &files) {
            UpdateState::LocalArchived {
                newer: Some(suggestion),
            } => assert_eq!(suggestion.file_id, 131),
            other => panic!("expected local-archived with a suggestion, got {other:?}"),
        }
        let alone = vec![file(140, Some(1), "1", FileCategory::Removed, T0)];
        assert_eq!(
            update_state(&pinned_to(140), Some(&published()), &alone),
            UpdateState::LocalArchived { newer: None }
        );
        let old = vec![file(141, Some(1), "1", FileCategory::OldVersion, T0)];
        assert_eq!(
            update_state(&pinned_to(141), Some(&published()), &old),
            UpdateState::LocalArchived { newer: None }
        );
    }

    #[test]
    fn a_local_file_no_longer_listed_uses_the_remembered_group() {
        let files = vec![file(151, Some(1), "8", FileCategory::Main, T0 + MINUTE)];
        let remembered = NexusLink {
            group_id: Some(1),
            uploaded_ms: Some(T0),
            ..pinned_to(150)
        };
        match update_state(&remembered, Some(&published()), &files) {
            UpdateState::UpdateAvailable { newer } => assert_eq!(newer.file_id, 151),
            other => panic!("expected an update, got {other:?}"),
        }
        // Without a remembered group nothing is a same-group successor; the
        // mod's newer main file is offered as a suggestion instead.
        let forgotten = pinned_to(150);
        match update_state(&forgotten, Some(&published()), &files) {
            UpdateState::LocalArchived {
                newer: Some(suggestion),
            } => assert_eq!(suggestion.file_id, 151),
            other => panic!("expected local-archived with a suggestion, got {other:?}"),
        }
        assert_eq!(
            update_state(
                &forgotten,
                Some(&published()),
                &[file(152, Some(1), "0", FileCategory::Optional, T0)]
            ),
            UpdateState::LocalArchived { newer: None },
            "an optional file is no suggestion"
        );
    }

    #[test]
    fn a_hidden_or_vanished_mod_is_unavailable() {
        let hidden = NexusMod {
            status: "hidden".into(),
            ..published()
        };
        let files = vec![file(160, Some(1), "1", FileCategory::Main, T0)];
        assert_eq!(
            update_state(&pinned_to(160), Some(&hidden), &files),
            UpdateState::ModUnavailable {
                status: "hidden".into()
            }
        );
        assert_eq!(
            update_state(&pinned_to(160), None, &[]),
            UpdateState::ModUnavailable {
                status: STATUS_NOT_FOUND.into()
            }
        );
    }

    #[test]
    fn an_unpinned_link_is_unpinned() {
        let files = vec![file(170, Some(1), "1", FileCategory::Main, T0)];
        assert_eq!(
            update_state(&link(), Some(&published()), &files),
            UpdateState::Unpinned
        );
        assert_eq!(update_state(&link(), None, &files), UpdateState::Unpinned);
    }

    #[test]
    fn equal_upload_times_fall_back_to_the_higher_file_id() {
        let files = vec![
            file(180, Some(1), "1", FileCategory::Main, T0),
            file(181, Some(1), "1b", FileCategory::Update, T0),
            file(179, Some(1), "0", FileCategory::Main, T0),
        ];
        match update_state(&pinned_to(180), Some(&published()), &files) {
            UpdateState::UpdateAvailable { newer } => assert_eq!(newer.file_id, 181),
            other => panic!("expected an update, got {other:?}"),
        }
        assert_eq!(
            update_state(&pinned_to(181), Some(&published()), &files),
            UpdateState::Current
        );
    }

    #[test]
    fn without_group_ids_the_file_name_groups_versions() {
        let mut local = file(190, None, "1", FileCategory::Main, T0);
        let mut renamed_same = file(191, None, "2", FileCategory::Main, T0 + MINUTE);
        renamed_same.name = "  LANTERN GLOW RECOLOR ".into();
        let mut other = file(192, None, "3", FileCategory::Main, T0 + 2 * MINUTE);
        other.name = "Lantern Glow Extras".into();
        local.name = "Lantern Glow Recolor".into();
        match update_state(
            &pinned_to(190),
            Some(&published()),
            &[local, renamed_same, other],
        ) {
            UpdateState::UpdateAvailable { newer } => assert_eq!(newer.file_id, 191),
            got => panic!("expected an update by name, got {got:?}"),
        }
    }

    #[test]
    fn the_wire_shape_matches_the_window() {
        let json = serde_json::to_value(UpdateState::LocalArchived { newer: None }).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "kind": "localArchived", "newer": null })
        );
        let json = serde_json::to_value(UpdateState::UpdateAvailable {
            newer: file(1, Some(2), "8", FileCategory::Main, T0),
        })
        .unwrap();
        assert_eq!(json["kind"], "updateAvailable");
        assert_eq!(json["newer"]["fileId"], 1);
        assert_eq!(json["newer"]["uploadedMs"], T0);
        assert_eq!(json["newer"]["category"], "main");
        let json = serde_json::to_value(UpdateState::ModUnavailable {
            status: "hidden".into(),
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "kind": "modUnavailable", "status": "hidden" })
        );
        let back: UpdateState =
            serde_json::from_value(serde_json::json!({ "kind": "current" })).unwrap();
        assert_eq!(back, UpdateState::Current);
    }

    #[test]
    fn version_keys_fold_separators_and_case() {
        assert_eq!(version_key(" V1-2_3 "), "1.2.3");
        assert_eq!(version_key("Beta"), "beta");
        assert_eq!(
            version_key("vanilla"),
            "vanilla",
            "a v is only stripped before a digit"
        );
    }
}
