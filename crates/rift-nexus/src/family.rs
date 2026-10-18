//! Versions of one Nexus file in the mods folder (v0.5): which packages are one file's versions,
//! and which of them leads.
//!
//! Two packages are versions of one file when both link to the same mod and either both links
//! name the same file group (Nexus gives every version of one file one group id; an update check
//! or a download pins it), or one package's last check named the other's exact file as its newer
//! file. A family is every package tied to another that way, directly or through others. Names
//! and sizes alone never tie anything: they cannot tell a new version from another file.
//!
//! The newest leads: the latest upload time, then the higher file id. A member whose link states
//! neither comes last, by file name.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use rift_core::NexusLink;

/// A package as the family rule reads it.
#[derive(Debug, Clone, Copy)]
pub struct FamilyMember<'a> {
    pub file_name: &'a str,
    pub link: Option<&'a NexusLink>,
    /// The file id the package's last check named as its newer file, if it named one.
    pub newer_file_id: Option<u64>,
}

/// One file's versions, as indexes into the members, newest first. Never fewer than two.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Family {
    pub members: Vec<usize>,
}

impl Family {
    /// The newest version's index.
    pub fn lead(&self) -> usize {
        self.members[0]
    }
}

fn tied(a: &FamilyMember, b: &FamilyMember) -> bool {
    let (Some(la), Some(lb)) = (a.link, b.link) else {
        return false;
    };
    if la.mod_id != lb.mod_id {
        return false;
    }
    let same_group = matches!((la.group_id, lb.group_id), (Some(x), Some(y)) if x == y);
    let named = |newer: Option<u64>, file: Option<u64>| matches!((newer, file), (Some(n), Some(f)) if n == f);
    same_group || named(a.newer_file_id, lb.file_id) || named(b.newer_file_id, la.file_id)
}

/// Newest first: the latest upload, then the higher file id, then the file name.
fn newest_first(a: &FamilyMember, b: &FamilyMember) -> Ordering {
    let uploaded = |m: &FamilyMember| m.link.and_then(|l| l.uploaded_ms);
    let file = |m: &FamilyMember| m.link.and_then(|l| l.file_id);
    uploaded(b)
        .cmp(&uploaded(a))
        .then_with(|| file(b).cmp(&file(a)))
        .then_with(|| a.file_name.cmp(b.file_name))
}

fn root(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

/// Every family among `members`, in the order their leads appear in `members`.
pub fn families(members: &[FamilyMember]) -> Vec<Family> {
    let mut by_mod: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
    for (i, member) in members.iter().enumerate() {
        if let Some(link) = member.link {
            by_mod.entry(link.mod_id).or_default().push(i);
        }
    }
    let mut parent: Vec<usize> = (0..members.len()).collect();
    for same_mod in by_mod.values() {
        for (n, &i) in same_mod.iter().enumerate() {
            for &j in &same_mod[n + 1..] {
                if tied(&members[i], &members[j]) {
                    let (ri, rj) = (root(&mut parent, i), root(&mut parent, j));
                    if ri != rj {
                        parent[rj] = ri;
                    }
                }
            }
        }
    }
    let mut sets: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..members.len() {
        let r = root(&mut parent, i);
        sets.entry(r).or_default().push(i);
    }
    let mut out: Vec<Family> = sets
        .into_values()
        .filter(|set| set.len() > 1)
        .map(|mut set| {
            set.sort_by(|&a, &b| newest_first(&members[a], &members[b]));
            Family { members: set }
        })
        .collect();
    out.sort_by_key(Family::lead);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rift_core::LinkSource;

    fn link(
        mod_id: u64,
        file_id: Option<u64>,
        group_id: Option<u64>,
        uploaded_ms: Option<u64>,
    ) -> NexusLink {
        NexusLink {
            mod_id,
            file_id,
            group_id,
            sqid: None,
            version: None,
            uploaded_ms,
            source: LinkSource::Download,
        }
    }

    fn member<'a>(
        file_name: &'a str,
        link: Option<&'a NexusLink>,
        newer: Option<u64>,
    ) -> FamilyMember<'a> {
        FamilyMember {
            file_name,
            link,
            newer_file_id: newer,
        }
    }

    #[test]
    fn the_same_mod_and_file_group_is_one_file_and_the_newest_leads() {
        let old = link(2207, Some(2391), Some(220_700), Some(1_000));
        let new = link(2207, Some(2410), Some(220_700), Some(2_000));
        let other = link(2207, Some(2392), Some(220_701), Some(3_000));
        let members = [
            member("Lantern Glow 1.0.zip", Some(&old), None),
            member("Lantern Glow Patch.zip", Some(&other), None),
            member("Lantern Glow 1.1.zip", Some(&new), None),
        ];
        let families = families(&members);
        assert_eq!(
            families,
            [Family {
                members: vec![2, 0]
            }],
            "another file of the same mod stays apart"
        );
        assert_eq!(families[0].lead(), 2);
    }

    #[test]
    fn a_check_that_named_the_other_file_as_newer_ties_them_without_a_group() {
        let old = link(2207, Some(2391), None, Some(1_000));
        let new = link(2207, Some(2410), Some(220_700), Some(2_000));
        let members = [
            member("old.zip", Some(&old), Some(2410)),
            member("new.zip", Some(&new), None),
        ];
        assert_eq!(
            families(&members),
            [Family {
                members: vec![1, 0]
            }]
        );
    }

    #[test]
    fn ties_carry_through_a_middle_version() {
        let v1 = link(7140, Some(1), None, Some(100));
        let v2 = link(7140, Some(2), Some(9), Some(200));
        let v3 = link(7140, Some(3), Some(9), Some(300));
        let members = [
            member("v1.zip", Some(&v1), Some(2)),
            member("v3.zip", Some(&v3), None),
            member("v2.zip", Some(&v2), None),
        ];
        assert_eq!(
            families(&members),
            [Family {
                members: vec![1, 2, 0]
            }]
        );
    }

    #[test]
    fn names_sizes_and_unpinned_links_never_make_a_family() {
        let pinned = link(2207, Some(2391), Some(220_700), Some(1_000));
        let unpinned = link(2207, None, None, None);
        let elsewhere = link(9001, Some(2410), Some(220_700), Some(2_000));
        let members = [
            member("Lantern Glow 1.0.zip", Some(&pinned), None),
            member("Lantern Glow 1.1.zip", Some(&unpinned), None),
            member("Lantern Glow.zip", None, None),
            member("Lantern Glow 2.zip", Some(&elsewhere), None),
        ];
        assert!(
            families(&members).is_empty(),
            "the same group number on another mod is another file"
        );
    }

    #[test]
    fn upload_time_then_file_id_then_name_decide_the_lead() {
        let a = link(5, Some(10), Some(1), Some(500));
        let b = link(5, Some(11), Some(1), Some(500));
        let c = link(5, None, Some(1), None);
        let d = link(5, None, Some(1), None);
        let members = [
            member("a.zip", Some(&a), None),
            member("d.zip", Some(&d), None),
            member("b.zip", Some(&b), None),
            member("c.zip", Some(&c), None),
        ];
        assert_eq!(
            families(&members),
            [Family {
                members: vec![2, 0, 3, 1]
            }]
        );
    }
}
