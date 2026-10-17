//! The relink pass (v0.3): an install whose package is no longer in the mods
//! folder — a folder that moved before folder ids followed their names, or an
//! adopted install carrying a stand-in id — is matched again to the library
//! option it came from, when exactly one option fits.
//!
//! [`propose`] is pure: it reads the journal's installs and the library's
//! options and says which install should take which option's id. The spine
//! hands its answer to [`crate::engine::rekey`], which rewrites the id
//! everywhere the journal mentions it in one save.
//!
//! An option FITS an install when, compared case-blind as NTFS compares:
//!
//! 1. the option root is the install's recorded root;
//! 2. every loose, script and root file the install recorded is in the
//!    option's payload at the same game path;
//! 3. a recorded pak asks only that the option carries paks (the pak's name
//!    in `pak_mods/` is the stone's own `zrift-NNNN-` name, not the
//!    package's);
//! 4. when the root is empty, or no loose, script or root file speaks for the
//!    install (it has only paks), the install's name is the option's name or
//!    its package's.
//!
//! A proposal is made only for exactly one fitting option, never onto an id
//! the journal already holds, and never onto an id two installs both fit.

use std::collections::{BTreeMap, HashMap, HashSet};

use rift_core::{lower_key, DeployTarget, ModOption, OptionId, PackageId};

use crate::journal::{Below, BelowOwner, DeployKind, InstalledOption};

/// Which installs should take which library option's id: `install id →
/// option id`, for every install whose package no option in `options`
/// belongs to and that exactly one option fits (see the module docs).
///
/// `installs` is the journal's whole install list: an id it holds (an
/// install's, or a backup owner's) is never proposed as a target. `options`
/// is every option of every package in the mods folder; an install counts as
/// missing when none of them belongs to its package (a package that yielded
/// no option counts as absent). An option's package name is read from the
/// first crumb of its `trail`, which the catalog starts at the package's
/// display name.
pub fn propose(
    installs: &[InstalledOption],
    options: &[&ModOption],
) -> BTreeMap<OptionId, OptionId> {
    let mut seen: HashSet<&OptionId> = HashSet::new();
    let library: Vec<&ModOption> = options
        .iter()
        .copied()
        .filter(|option| seen.insert(&option.id))
        .collect();
    let present: HashSet<&PackageId> = library.iter().map(|option| &option.package_id).collect();
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
    let targets: HashSet<String> = option
        .payload
        .iter()
        .filter_map(|file| file.target.game_path())
        .map(lower_key)
        .collect();
    let carries_paks = option
        .payload
        .iter()
        .any(|file| matches!(file.target, DeployTarget::Pak { .. }));
    let mut spoken = false;
    for file in &install.files {
        match file.kind {
            DeployKind::Pak => {
                if !carries_paks {
                    return false;
                }
            }
            DeployKind::Loose | DeployKind::Reframework | DeployKind::Root => {
                if !targets.contains(&lower_key(&file.game_path)) {
                    return false;
                }
                spoken = true;
            }
        }
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
