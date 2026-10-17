//! The relink pass (v0.3): an install whose package is no longer in the mods
//! folder — a folder that moved before folder ids followed their names, or an
//! adopted install carrying a stand-in id — is matched again to the library
//! option it came from, when exactly one option fits.
//!
//! [`propose_on_shelf`] is pure: it reads the journal's installs and the
//! packages of the mods folder, and says which install should take which
//! option's id. The spine hands its answer to [`crate::engine::rekey`], which
//! rewrites the id everywhere the journal mentions it in one save.
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
//! The sizes keep an install on the bytes the game folder holds. A newer
//! version of a mod (the same root, the same game paths, other bytes) is
//! another option, and the install of the old one must never be shown as
//! the new one while the game folder still holds the old files. A moved or
//! an adopted install keeps its sizes, so it still fits. Sizes are not
//! bytes: a version that changed no file's size passes for the same files.
//!
//! A proposal is made only for exactly one fitting option, never onto an id
//! the journal already holds, and never onto an id two installs both fit.

use std::collections::{BTreeMap, HashMap, HashSet};

use rift_core::{lower_key, DeployTarget, ModOption, OptionId, Package, PackageId};

use crate::journal::{Below, BelowOwner, DeployKind, InstalledOption};

/// Which installs should take which library option's id: `install id →
/// option id`, for every install whose package is not on `shelf` and that
/// exactly one of the shelf's options fits (see the module docs).
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
