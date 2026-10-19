//! Versions of one Nexus file in the mods folder (v0.5, WINDOW-CONTRACT § v0.5): the shelf's
//! families by rift-nexus's family rule, the options an install of one version replaces, and
//! where the older-versions folder sits. Ruling 10 adds the question the stone asks before the
//! player's word joins a version Nexus cannot pin ([`version_questions`]).

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use rift_core::{lower_key, ArchiveEntry, ModOption, NexusLink, OptionId, Package, PackageId};
use rift_nexus::FamilyMember;

use crate::dto::{FamilyView, OlderOptionMatch, OlderVersionView, VersionQuestionView};
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

/// The file names a package holds as the version question compares them: without their
/// folders, in any case, the catalog's own `modinfo.ini` and `screenshot.*` left out.
fn compared_names(entries: &[ArchiveEntry]) -> BTreeSet<String> {
    entries
        .iter()
        .filter_map(|entry| entry.path.rsplit(['/', '\\']).next())
        .map(lower_key)
        .filter(|name| {
            !name.is_empty() && name != "modinfo.ini" && !name.starts_with("screenshot.")
        })
        .collect()
}

/// The question a package's file names make likely against another's: how many names they
/// share, when they share one and one's names are all among the other's.
fn likely_shared(own: &BTreeSet<String>, theirs: &BTreeSet<String>) -> Option<usize> {
    let shared = own.intersection(theirs).count();
    (shared > 0 && (own.is_subset(theirs) || theirs.is_subset(own))).then_some(shared)
}

/// Every package the stone asks about (ruling 10, WINDOW-CONTRACT § v0.5, The player's word): a
/// package whose link names a mod and no file group, in no family, beside another package of
/// the same mod pinned to a file group the player has not said no to, when their file names
/// make it likely. Several groups: the most names in common, then the newest. The question
/// names the group's newest package.
pub fn version_questions(
    shelf: &Shelf,
    store: &NexusStore,
    families: &Families,
) -> HashMap<PackageId, VersionQuestionView> {
    let packages = shelf.packages();
    let links: Vec<_> = packages.iter().map(|p| store.link_for(p)).collect();
    let pinned: Vec<(usize, &NexusLink, u64)> = links
        .iter()
        .enumerate()
        .filter_map(|(i, link)| {
            let link = link.as_ref()?;
            Some((i, link, link.group_id?))
        })
        .collect();
    let mut names: HashMap<usize, BTreeSet<String>> = HashMap::new();
    let mut names_of = |i: usize| -> BTreeSet<String> {
        names
            .entry(i)
            .or_insert_with(|| compared_names(shelf.entries(&packages[i].id)))
            .clone()
    };
    // The newest package of a pinned one's file: its family's lead, else itself.
    let newest = |i: usize| -> usize {
        families
            .lead(&packages[i].id)
            .and_then(|lead| packages.iter().position(|p| &p.id == lead))
            .unwrap_or(i)
    };
    let recency = |i: usize| links[i].as_ref().map(|l| (l.uploaded_ms, l.file_id));
    let mut questions = HashMap::new();
    for (i, package) in packages.iter().enumerate() {
        let Some(link) = links[i].as_ref() else {
            continue;
        };
        if link.group_id.is_some() || families.family(&package.id).is_some() {
            continue;
        }
        let own = names_of(i);
        let declined = store.declined(&package.id);
        // (shared names, the group's newest package)
        let mut best: Option<(usize, usize)> = None;
        for &(j, other, group_id) in &pinned {
            if other.mod_id != link.mod_id || declined.contains(&group_id) {
                continue;
            }
            let Some(shared) = likely_shared(&own, &names_of(j)) else {
                continue;
            };
            let lead = newest(j);
            let better = best.is_none_or(|(best_shared, best_lead)| {
                (shared, recency(lead)) > (best_shared, recency(best_lead))
            });
            if better {
                best = Some((shared, lead));
            }
        }
        if let Some((shared, lead)) = best {
            let of = &packages[lead];
            questions.insert(
                package.id.clone(),
                VersionQuestionView {
                    of_package_id: of.id.0.clone(),
                    of_file_name: of.file_name.clone(),
                    of_version: links[lead].as_ref().and_then(|l| l.version.clone()),
                    shared_files: shared,
                    files: own.len(),
                },
            );
        }
    }
    questions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(paths: &[&str]) -> Vec<ArchiveEntry> {
        paths
            .iter()
            .map(|path| ArchiveEntry {
                path: (*path).to_string(),
                size: 1,
            })
            .collect()
    }

    #[test]
    fn the_question_compares_file_names_without_folders_case_or_the_catalogs_own_files() {
        let folder = compared_names(&entries(&[
            "modinfo.ini",
            "reframework/autorun/Lantern Glow.lua",
            "reframework/data/lantern_glow.json",
        ]));
        let zip = compared_names(&entries(&[
            "Lantern Glow Recolor/modinfo.ini",
            "Lantern Glow Recolor/screenshot.png",
            "Lantern Glow Recolor/reframework/autorun/lantern glow.lua",
            "Lantern Glow Recolor/reframework/data/lantern_glow.json",
            "Lantern Glow Recolor/reframework/data/lantern_glow_dusk.json",
        ]));
        assert_eq!(folder.len(), 2, "modinfo.ini never counts");
        assert_eq!(zip.len(), 3, "nor does a screenshot");
        assert_eq!(
            likely_shared(&folder, &zip),
            Some(2),
            "the older one's names all reappear"
        );
        let other_file = compared_names(&entries(&["reframework/autorun/Cloak Sway.lua"]));
        assert_eq!(likely_shared(&other_file, &zip), None, "nothing in common");
        let crossing = compared_names(&entries(&[
            "reframework/autorun/lantern glow.lua",
            "reframework/autorun/Cloak Sway.lua",
        ]));
        assert_eq!(
            likely_shared(&crossing, &zip),
            None,
            "one name in common, but neither holds only the other's names"
        );
        assert_eq!(
            likely_shared(&compared_names(&entries(&["modinfo.ini"])), &zip),
            None,
            "a package of nothing but modinfo.ini is asked about nothing"
        );
    }

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
