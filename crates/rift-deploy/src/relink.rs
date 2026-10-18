//! The relink pass (v0.3): an install whose package is no longer in the mods
//! folder — a folder that moved before folder ids followed their names, or an
//! adopted install carrying a stand-in id — is matched again to the library
//! option it came from, when exactly one option fits.
//!
//! [`propose_on_shelf`] is pure: it reads the journal's installs and the
//! packages of the mods folder, and says which install should take which
//! option's id. The spine confirms each proposal's bytes (below), then hands
//! what holds to [`crate::engine::rekey`], which rewrites the id everywhere
//! the journal mentions it in one save.
//!
//! An install is a candidate only when its package is not on the shelf at
//! all. A package that is there but yielded no option this scan (unreadable,
//! a folder caught mid-copy) still holds its installs: a relink is permanent,
//! so an install must never move to another copy of the mod while its own
//! package only failed to read once.
//!
//! An option FITS an install when, compared case-blind as NTFS compares:
//!
//! 1. the option root is the install's recorded root;
//! 2. every loose, script and root file the install recorded is in the
//!    option's payload at the same game path, at the same size (where two
//!    entries target one path, the later one, as an install writes it);
//! 3. the install's paks are the option's paks by size: as many, of the same
//!    sizes (a pak's name in `pak_mods/` is the stone's own `zrift-NNNN-`
//!    name, not the package's, so its size and their count are what tie it
//!    to the option);
//! 4. when the root is empty, or no loose, script or root file speaks for the
//!    install (it has only paks), the install's name is the option's name or
//!    its package's.
//!
//! A newer version of a mod (the same root, the same game paths, other
//! bytes) is another option, and the install of the old one must never be
//! shown as the new one while the game folder still holds the old files. A
//! moved or an adopted install keeps its sizes, so it still fits; a version
//! that changed a file's size does not. Sizes are not bytes, though: a
//! version that changed no file's size (a recolor whose textures keep their
//! size) fits the sizes too. So the sizes only propose, and the spine
//! confirms the content before it applies a proposal: [`content_check`]
//! names the entries of the candidate's package to read and the
//! fingerprints the journal recorded for the bytes the install wrote,
//! [`read_fingerprints`] reads them through the package source with the
//! engine's own fingerprint, and [`ContentCheck::confirms`] compares. Any
//! mismatch drops the proposal. A file that carries no fingerprint (an
//! install the journal took over may carry none) has its size only. The
//! fingerprint is a file's size and its first and last MiB, not every byte:
//! a change that lies wholly between them, in a file over 2 MiB, is not seen
//! (CLAUDE.md, Known Limitations).
//!
//! A proposal is made only for exactly one fitting option, never onto an id
//! the journal already holds, and never onto an id two installs both fit.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use rift_core::error::RiftResult;
use rift_core::{
    lower_key, DeployTarget, ExtractItem, ModOption, OptionId, Package, PackageId, PackageSource,
    PayloadFile, RiftError,
};

use crate::fsops::{self, PART_SUFFIX};
use crate::journal::{Below, BelowOwner, DeployKind, InstalledOption};

/// Which installs should take which library option's id: `install id →
/// option id`, for every install whose package is not on `shelf` and that
/// exactly one of the shelf's options fits (see the module docs). The fit is
/// by paths and sizes: a proposal for an install that recorded fingerprints
/// waits for its bytes ([`content_check`]) before anyone applies it.
///
/// `shelf` is every package of the mods folder as the last scan found it,
/// those that yielded no option included: an install whose package is on
/// it, readable or not, stays where it is. `installs` is the journal's whole
/// install list: an id it holds (an install's, or a backup owner's) is never
/// proposed as a target. An option's package name is read from the first
/// crumb of its `trail`, which the catalog starts at the package's display
/// name.
pub fn propose_on_shelf(
    installs: &[InstalledOption],
    shelf: &[Package],
) -> BTreeMap<OptionId, OptionId> {
    let present: HashSet<&PackageId> = shelf.iter().map(|package| &package.id).collect();
    let options: Vec<&ModOption> = shelf.iter().flat_map(|package| &package.options).collect();
    proposals_for(installs, &present, &options)
}

/// [`propose_on_shelf`] for a caller that holds only the options. A package
/// counts as on the shelf here when one of `options` belongs to it, so this
/// cannot see a package that yielded no option this scan: an install of an
/// unreadable package is a candidate here although its package is still in
/// the mods folder. Wherever the packages are at hand, call
/// [`propose_on_shelf`].
pub fn propose(
    installs: &[InstalledOption],
    options: &[&ModOption],
) -> BTreeMap<OptionId, OptionId> {
    let present: HashSet<&PackageId> = options.iter().map(|option| &option.package_id).collect();
    proposals_for(installs, &present, options)
}

/// The matcher both doors share: `present` is the shelf's package ids,
/// `options` every option on it.
fn proposals_for(
    installs: &[InstalledOption],
    present: &HashSet<&PackageId>,
    options: &[&ModOption],
) -> BTreeMap<OptionId, OptionId> {
    let mut seen: HashSet<&OptionId> = HashSet::new();
    let library: Vec<&ModOption> = options
        .iter()
        .copied()
        .filter(|option| seen.insert(&option.id))
        .collect();
    // The ids the installs hold: their own, and the owners their backups name.
    let owners = installs
        .iter()
        .flat_map(|install| &install.files)
        .filter_map(|file| match &file.below {
            Below::Backup {
                owner: BelowOwner::Riftstone { option_id },
                ..
            } => Some(option_id),
            _ => None,
        });
    let held: HashSet<&OptionId> = installs
        .iter()
        .map(|install| &install.option_id)
        .chain(owners)
        .collect();

    let mut proposals: Vec<(&OptionId, &OptionId)> = Vec::new();
    for install in installs {
        if present.contains(&install.option_id.package()) {
            continue;
        }
        let mut fitting = library.iter().filter(|option| fits(install, option));
        let (Some(only), None) = (fitting.next(), fitting.next()) else {
            continue;
        };
        if held.contains(&only.id) {
            continue;
        }
        proposals.push((&install.option_id, &only.id));
    }
    let mut claims: HashMap<&OptionId, usize> = HashMap::new();
    for &(_, to) in &proposals {
        *claims.entry(to).or_default() += 1;
    }
    proposals
        .into_iter()
        .filter(|(_, to)| claims.get(to) == Some(&1))
        .map(|(from, to)| (from.clone(), to.clone()))
        .collect()
}

/// Does `option` fit `install` (the four rules of the module docs)?
fn fits(install: &InstalledOption, option: &ModOption) -> bool {
    if option.payload.is_empty() || lower_key(&install.option_root) != lower_key(&option.root) {
        return false;
    }
    // What an install of this option writes: one size per game path (a
    // later entry for a path replaces an earlier one, as the engine's
    // install does), and the size of every pak.
    let mut sizes: HashMap<String, u64> = HashMap::new();
    let mut paks: Vec<u64> = Vec::new();
    for file in &option.payload {
        match &file.target {
            DeployTarget::Pak { .. } => paks.push(file.size),
            DeployTarget::Loose { game_path }
            | DeployTarget::Reframework { game_path }
            | DeployTarget::Root { game_path } => {
                sizes.insert(lower_key(game_path), file.size);
            }
        }
    }
    let mut recorded_paks: Vec<u64> = Vec::new();
    let mut spoken = false;
    for file in &install.files {
        match file.kind {
            DeployKind::Pak => recorded_paks.push(file.size),
            DeployKind::Loose | DeployKind::Reframework | DeployKind::Root => {
                if sizes.get(&lower_key(&file.game_path)) != Some(&file.size) {
                    return false;
                }
                spoken = true;
            }
        }
    }
    paks.sort_unstable();
    recorded_paks.sort_unstable();
    if paks != recorded_paks {
        return false;
    }
    if install.option_root.trim().is_empty() || !spoken {
        return same_name(install, option);
    }
    true
}

/// Is the install's name the option's name or its package's (case-blind)?
fn same_name(install: &InstalledOption, option: &ModOption) -> bool {
    let name = lower_key(install.name.trim());
    !name.is_empty()
        && (name == lower_key(option.name.trim())
            || option
                .trail
                .first()
                .is_some_and(|package| name == lower_key(package.trim())))
}

// ---------------------------------------------------------------------------
// The bytes behind the sizes
// ---------------------------------------------------------------------------

/// What confirming one proposal by its bytes reads, and what it must find
/// there ([`content_check`]). Two equal checks ask the same question of the
/// same entries, so a caller may remember an answer by its check (and the
/// package it read it from).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContentCheck {
    install: OptionId,
    option: OptionId,
    /// Every entry to read, once, with the size its package lists.
    entries: BTreeMap<String, u64>,
    /// Each fingerprinted loose, script or root file: the entry an install
    /// of the option writes at its game path, and the fingerprint recorded
    /// for the file the install wrote there. Sorted.
    files: Vec<(String, String)>,
    /// The fingerprinted paks, one group per size.
    paks: Vec<PakGroup>,
    /// Fingerprinted files at a game path the option writes nothing to.
    unmatched: usize,
}

/// The paks of one size: the fingerprints the install recorded for its paks
/// of that size, and the option's pak entries of that size. A pak's name in
/// `pak_mods/` is the stone's own, so any of the entries may hold any of them.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PakGroup {
    size: u64,
    /// Sorted.
    recorded: Vec<String>,
    entries: Vec<String>,
}

impl ContentCheck {
    /// The install the proposal moves (its id as the journal held it).
    pub fn install(&self) -> &OptionId {
        &self.install
    }

    /// The option it would take.
    pub fn option(&self) -> &OptionId {
        &self.option
    }

    /// Every package entry the check reads, with the size its package lists.
    pub fn entries(&self) -> impl Iterator<Item = (&str, u64)> {
        self.entries
            .iter()
            .map(|(entry, size)| (entry.as_str(), *size))
    }

    /// The bytes reading every entry unpacks, as the package lists them.
    pub fn bytes(&self) -> u64 {
        self.entries
            .values()
            .fold(0u64, |sum, size| sum.saturating_add(*size))
    }

    /// Do these fingerprints (entry → fingerprint, as [`read_fingerprints`]
    /// gives them) match the ones the install recorded (each a file's size
    /// and its first and last MiB, not every byte)? Every
    /// fingerprinted loose, script or root file must be its entry's; the
    /// fingerprinted paks of each size must be found among the option's pak
    /// entries of that size, each entry used once. An entry missing from
    /// `prints` confirms nothing.
    pub fn confirms(&self, prints: &HashMap<String, String>) -> bool {
        if self.unmatched > 0 {
            return false;
        }
        let files = self
            .files
            .iter()
            .all(|(entry, print)| prints.get(entry) == Some(print));
        files
            && self.paks.iter().all(|group| {
                let mut found: Vec<&String> = Vec::with_capacity(group.entries.len());
                for entry in &group.entries {
                    match prints.get(entry) {
                        Some(print) => found.push(print),
                        None => return false,
                    }
                }
                group.recorded.iter().all(|print| {
                    match found.iter().position(|candidate| *candidate == print) {
                        Some(at) => {
                            found.swap_remove(at);
                            true
                        }
                        None => false,
                    }
                })
            })
    }
}

/// What confirms the proposal `install` → `option` beyond its sizes, for a
/// pair the sizes fit ([`propose_on_shelf`]): every loose, script or root
/// file the install recorded a fingerprint for is read from the entry an
/// install of the option writes at its game path (the later one, where two
/// entries target one path), and every pak of the option whose size one of
/// the install's fingerprinted paks has. A file without a fingerprint is
/// held to its size alone. `None` when the install recorded no fingerprint
/// at all (an install the journal took over may carry none): its sizes are
/// all there is.
pub fn content_check(install: &InstalledOption, option: &ModOption) -> Option<ContentCheck> {
    if install.files.iter().all(|file| file.fingerprint.is_none()) {
        return None;
    }
    // What an install of the option writes: one entry per game path (a
    // later entry for a path replaces an earlier one), and its paks by size.
    let mut at_path: HashMap<String, &PayloadFile> = HashMap::new();
    let mut paks_by_size: BTreeMap<u64, Vec<&PayloadFile>> = BTreeMap::new();
    for file in &option.payload {
        match &file.target {
            DeployTarget::Pak { .. } => paks_by_size.entry(file.size).or_default().push(file),
            DeployTarget::Loose { game_path }
            | DeployTarget::Reframework { game_path }
            | DeployTarget::Root { game_path } => {
                at_path.insert(lower_key(game_path), file);
            }
        }
    }
    let mut check = ContentCheck {
        install: install.option_id.clone(),
        option: option.id.clone(),
        entries: BTreeMap::new(),
        files: Vec::new(),
        paks: Vec::new(),
        unmatched: 0,
    };
    let mut recorded_paks: BTreeMap<u64, Vec<String>> = BTreeMap::new();
    for file in &install.files {
        let Some(print) = &file.fingerprint else {
            continue;
        };
        match file.kind {
            DeployKind::Pak => recorded_paks
                .entry(file.size)
                .or_default()
                .push(print.clone()),
            DeployKind::Loose | DeployKind::Reframework | DeployKind::Root => {
                match at_path.get(&lower_key(&file.game_path)) {
                    Some(payload) => {
                        check.entries.insert(payload.entry.clone(), payload.size);
                        check.files.push((payload.entry.clone(), print.clone()));
                    }
                    None => check.unmatched += 1,
                }
            }
        }
    }
    for (size, mut recorded) in recorded_paks {
        recorded.sort_unstable();
        let entries: Vec<String> = paks_by_size
            .get(&size)
            .map(|files| files.iter().map(|file| file.entry.clone()).collect())
            .unwrap_or_default();
        for entry in &entries {
            check.entries.insert(entry.clone(), size);
        }
        check.paks.push(PakGroup {
            size,
            recorded,
            entries,
        });
    }
    check.files.sort_unstable();
    Some(check)
}

/// Read every entry of `check` from `source` (the candidate's package, as
/// the caller opened it) and fingerprint each with the engine's own
/// fingerprint, the one an install records for every file it writes:
/// entry → fingerprint. The bytes pass through `scratch`, an existing folder
/// of the caller's that is neither the game folder nor the mods folder: one
/// extraction pass (a solid archive decompresses once), every part created
/// new, and every part the extractor wrote removed again before this
/// returns. Nothing else is written anywhere.
pub fn read_fingerprints(
    check: &ContentCheck,
    source: &mut dyn PackageSource,
    scratch: &Path,
) -> RiftResult<HashMap<String, String>> {
    let items: Vec<ExtractItem> = check
        .entries
        .keys()
        .enumerate()
        .map(|(n, entry)| ExtractItem {
            entry: entry.clone(),
            dest: scratch.join(format!("{n}{PART_SUFFIX}")),
        })
        .collect();
    let mut written = vec![false; items.len()];
    let extracted = source.extract(&items, &mut |report| {
        if let Some(done) = written.get_mut(report.item) {
            *done = true;
        }
    });
    let prints = extracted.and_then(|()| {
        items
            .iter()
            .map(|item| {
                fsops::fingerprint(&item.dest)
                    .map(|print| (item.entry.clone(), print))
                    .map_err(|err| {
                        RiftError::io(
                            format!("could not read back {} after unpacking it", item.entry),
                            err,
                        )
                    })
            })
            .collect::<RiftResult<HashMap<String, String>>>()
    });
    let parts: Vec<&PathBuf> = items
        .iter()
        .zip(&written)
        .filter(|(_, done)| **done)
        .map(|(item, _)| &item.dest)
        .collect();
    for part in parts {
        if let Err(err) = fsops::remove_file_if_exists(part) {
            log::warn!(
                "could not remove {} after reading it: {err}",
                part.display()
            );
        }
    }
    prints
}
