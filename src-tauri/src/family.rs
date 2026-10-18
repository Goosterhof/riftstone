//! Versions of one Nexus file in the mods folder (v0.5, WINDOW-CONTRACT § v0.5): the shelf's
//! families by rift-nexus's family rule, the options an install of one version replaces, and
//! where the older-versions folder sits.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rift_core::{lower_key, ModOption, NexusLink, OptionId, Package, PackageId};
use rift_nexus::FamilyMember;

use crate::dto::{FamilyView, OlderOptionMatch, OlderVersionView};
use crate::library::Shelf;
use crate::nexus::NexusStore;

/// The older-versions folder: beside the mods folder, named for it (`mods (older versions)`).
/// `None` for a mods folder with no folder around it (a drive's root), where the older-versions
/// folder could only sit inside the library.
pub fn older_folder(mods: &Path) -> Option<PathBuf> {
    let mut name = mods.file_name()?.to_os_string();
    name.push(" (older versions)");
    Some(mods.parent()?.join(name))
}

/// The shelf's packages as the family rule reads them, against their links (`links[i]` is
/// `packages[i]`'s).
fn members<'a>(
    packages: &'a [Package],
    links: &'a [Option<NexusLink>],
    store: &NexusStore,
) -> Vec<FamilyMember<'a>> {
    packages
        .iter()
        .zip(links)
        .map(|(package, link)| FamilyMember {
            file_name: &package.file_name,
            link: link.as_ref(),
            newer_file_id: link
                .as_ref()
                .and_then(|l| store.newer_file_id(&package.id, l)),
        })
        .collect()
}

/// Is a newer version of the file a put-away version (`file_name`, its kept `link`, the newer
/// file its last check named) in the mods folder now? The family rule over the shelf and it.
pub fn newer_in_mods(
    shelf: &Shelf,
    store: &NexusStore,
    file_name: &str,
    link: &NexusLink,
    newer_file_id: Option<u64>,
) -> bool {
    let packages = shelf.packages();
    let links: Vec<_> = packages.iter().map(|p| store.link_for(p)).collect();
    let mut all = members(packages, &links, store);
    let away = all.len();
    all.push(FamilyMember {
        file_name,
        link: Some(link),
        newer_file_id,
    });
    rift_nexus::families(&all)
        .iter()
        .find(|family| family.members.contains(&away))
        .is_some_and(|family| family.lead() != away)
}

/// The shelf's families, each newest first, with every member's Nexus file version.
#[derive(Debug, Default)]
pub struct Families {
    of: HashMap<PackageId, usize>,
    members: Vec<Vec<PackageId>>,
    versions: HashMap<PackageId, Option<String>>,
}

impl Families {
    pub fn of_shelf(shelf: &Shelf, store: &NexusStore) -> Families {
        let packages = shelf.packages();
        let links: Vec<_> = packages.iter().map(|p| store.link_for(p)).collect();
        let mut families = Families::default();
        for (n, family) in rift_nexus::families(&members(packages, &links, store))
            .iter()
            .enumerate()
        {
            let ids: Vec<PackageId> = family
                .members
                .iter()
                .map(|&i| packages[i].id.clone())
                .collect();
            for &i in &family.members {
                families.of.insert(packages[i].id.clone(), n);
                families.versions.insert(
                    packages[i].id.clone(),
                    links[i].as_ref().and_then(|l| l.version.clone()),
                );
            }
            families.members.push(ids);
        }
        families
    }

    /// The package's family, newest first; `None` when it is alone.
    pub fn family(&self, id: &PackageId) -> Option<&[PackageId]> {
        self.of.get(id).map(|&n| self.members[n].as_slice())
    }

    pub fn lead(&self, id: &PackageId) -> Option<&PackageId> {
        self.family(id).map(|members| &members[0])
    }

    /// A member of a family that a newer version leads.
    pub fn is_older(&self, id: &PackageId) -> bool {
        self.lead(id).is_some_and(|lead| lead != id)
    }

    pub fn version(&self, id: &PackageId) -> Option<String> {
        self.versions.get(id).cloned().flatten()
    }

    /// The members older than `id`, newest first; empty when it is alone or the oldest.
    pub fn older_than(&self, id: &PackageId) -> &[PackageId] {
        self.family(id)
            .and_then(|members| {
                let at = members.iter().position(|m| m == id)?;
                Some(&members[at + 1..])
            })
            .unwrap_or(&[])
    }

    /// Are these packages, two or more, all versions of one file?
    pub fn one_family(&self, ids: &[PackageId]) -> bool {
        let mut families = ids.iter().map(|id| self.of.get(id));
        match families.next() {
            Some(Some(first)) => ids.len() > 1 && families.all(|f| f == Some(first)),
            _ => false,
        }
    }

    /// The package's `family` for the library view; `installed(option)` says what is in the
    /// game.
    pub fn view(
        &self,
        shelf: &Shelf,
        id: &PackageId,
        installed: &impl Fn(&OptionId) -> bool,
    ) -> Option<FamilyView> {
        let members = self.family(id)?;
        let lead = &members[0];
        let older = if lead == id {
            self.older_versions(shelf, &members[1..], installed)
        } else {
            Vec::new()
        };
        Some(FamilyView {
            lead: lead.0.clone(),
            version: self.version(id),
            older,
        })
    }

    /// These members (older versions of `members[0]`'s family) as the window lists them inside
    /// their lead, each with its options that are the same option as one of the lead's.
    pub fn older_versions(
        &self,
        shelf: &Shelf,
        members: &[PackageId],
        installed: &impl Fn(&OptionId) -> bool,
    ) -> Vec<OlderVersionView> {
        let lead = members
            .first()
            .and_then(|id| self.lead(id))
            .and_then(|id| shelf.package(id));
        members
            .iter()
            .filter_map(|id| shelf.package(id))
            .map(|package| OlderVersionView {
                package_id: package.id.0.clone(),
                file_name: package.file_name.clone(),
                version: self.version(&package.id),
                installed: package.options.iter().filter(|o| installed(&o.id)).count(),
                matches: lead.map_or_else(Vec::new, |lead| {
                    package
                        .options
                        .iter()
                        .filter_map(|option| {
                            let same = lead_match(option, package, lead)?;
                            Some(OlderOptionMatch {
                                option_id: option.id.0.clone(),
                                lead_option_id: same.id.0.clone(),
                                installed: installed(&option.id),
                            })
                        })
                        .collect()
                }),
            })
            .collect()
    }
}

/// `other`'s option that is the same option as `option` of `own`: the same root, else the same
/// name (any case), else the one installable option each package has. A version that renamed
/// its top folder (`Lantern Glow v1.1/`) still finds its older self.
pub fn matching_option<'a>(
    option: &ModOption,
    own: &Package,
    other: &'a Package,
) -> Option<&'a ModOption> {
    let installable = |p: &'a Package| p.options.iter().filter(|o| !o.menu_only);
    if let Some(same) = other.options.iter().find(|o| o.root == option.root) {
        return Some(same);
    }
    let name = lower_key(&option.name);
    if let Some(same) = other.options.iter().find(|o| lower_key(&o.name) == name) {
        return Some(same);
    }
    let own_installable = own.options.iter().filter(|o| !o.menu_only).count();
    let mut theirs = installable(other);
    match (own_installable, theirs.next(), theirs.next()) {
        (1, Some(only), None) => Some(only),
        _ => None,
    }
}

/// The lead's option that is the same option as `option` of an older version: the one whose
/// install would take its place ([`matching_option`] from the lead's side). `None`: only the
/// older version holds it.
pub fn lead_match<'a>(
    option: &ModOption,
    older: &Package,
    lead: &'a Package,
) -> Option<&'a ModOption> {
    lead.options
        .iter()
        .filter(|candidate| !candidate.menu_only)
        .find(|candidate| {
            matching_option(candidate, lead, older).is_some_and(|same| same.id == option.id)
        })
}

/// The installs an install of `option` (of `package`) takes the place of: the matching option
/// of every other version in its family, where that option is installed.
pub fn replaced(
    families: &Families,
    shelf: &Shelf,
    package: &Package,
    option: &ModOption,
    installed: impl Fn(&OptionId) -> bool,
) -> Vec<OptionId> {
    let Some(members) = families.family(&package.id) else {
        return Vec::new();
    };
    members
        .iter()
        .filter(|id| **id != package.id)
        .filter_map(|id| shelf.package(id))
        .filter_map(|other| matching_option(option, package, other))
        .map(|o| o.id.clone())
        .filter(|id| installed(id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_older_versions_folder_sits_beside_the_mods_folder_named_for_it() {
        assert_eq!(
            older_folder(Path::new("/games/The Riftstone/mods")).as_deref(),
            Some(Path::new("/games/The Riftstone/mods (older versions)"))
        );
        assert_eq!(
            older_folder(Path::new("/bench/my mods")).as_deref(),
            Some(Path::new("/bench/my mods (older versions)"))
        );
        assert_eq!(
            older_folder(Path::new("/")),
            None,
            "a root has no folder around it"
        );
    }
}
