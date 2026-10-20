//! Versions of one Nexus file in the mods folder (v0.5): which packages are one file's versions,
//! and which of them leads.
//!
//! Two packages are versions of one file when both link to the same mod and either both links
//! name the same file group (Nexus gives every version of one file one group id; an update check
//! or a download pins it), or one package's last check named the other's exact file as its newer
//! file. A family is every package tied to another that way, directly or through others. Names
//! and sizes alone never tie anything: they cannot tell a new version from another file.
//!
//! The newest leads. A check that named another member's file as newer is the one proof of age
//! the rule holds: that member is at least as new as the one whose check named it, whatever its
//! own link says (a link pasted as a file URL carries a file id and no upload time). So it is
//! ordered by the newer of its own recency and the namer's, carried up a chain of such proofs,
//! and then placed before the member that named it. Recency is the latest upload time, then the
//! higher file id; a member whose link states neither, and that no proof lifts, comes last, by
//! file name.

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
    same_group || named_newer(a, b) || named_newer(b, a)
}

/// `a`'s last check named `b`'s exact file as its newer file.
fn named_newer(a: &FamilyMember, b: &FamilyMember) -> bool {
    matches!((a.newer_file_id, b.link.and_then(|l| l.file_id)), (Some(n), Some(f)) if n == f)
}

/// A member's recency as its own link states it: the upload time, then the file id. `None`
/// sorts below any time or id, so a member stating neither comes last.
type Recency = (Option<u64>, Option<u64>);

fn recency(m: &FamilyMember) -> Recency {
    (
        m.link.and_then(|l| l.uploaded_ms),
        m.link.and_then(|l| l.file_id),
    )
}

/// Each member's recency lifted by the proofs: a member a check named as newer is at least as
/// new as the member whose check named it, so it takes the newer of the two, and a chain of
/// proofs carries it on. A fixed point over the family; every round lifts at least one member
/// or stops, and no recency rises above the family's newest, so a cycle of proofs (which Nexus
/// never gives) ends too.
fn lifted(members: &[FamilyMember], set: &[usize]) -> BTreeMap<usize, Recency> {
    let mut at: BTreeMap<usize, Recency> = set.iter().map(|&i| (i, recency(&members[i]))).collect();
    for _ in 0..set.len() {
        let mut moved = false;
        for &namer in set {
            for &named in set {
                if namer != named
                    && named_newer(&members[namer], &members[named])
                    && at[&namer] > at[&named]
                {
                    at.insert(named, at[&namer]);
                    moved = true;
                }
            }
        }
        if !moved {
            break;
        }
    }
    at
}

/// One family's members, newest first. They are sorted by their lifted recency, then their own,
/// then file name; a member a check named as newer has a lifted recency no lower than its namer's,
/// so the two sit together, and the pass below puts the named one first where they tie: a member
/// goes next only when no member still waiting was named newer than it. That is a pass and not a
/// comparator, because the proof is a relation between two members and not a total order
/// (`sort_by` must be given one). A cycle of proofs falls back to the sorted order.
fn newest_order(members: &[FamilyMember], mut set: Vec<usize>) -> Vec<usize> {
    let at = lifted(members, &set);
    set.sort_by(|&a, &b| {
        at[&b]
            .cmp(&at[&a])
            .then_with(|| recency(&members[b]).cmp(&recency(&members[a])))
            .then_with(|| members[a].file_name.cmp(members[b].file_name))
    });
    let mut out = Vec::with_capacity(set.len());
    while !set.is_empty() {
        let ready = |&i: &usize| {
            !set.iter()
                .any(|&j| j != i && named_newer(&members[i], &members[j]))
        };
        let next = set.iter().position(ready).unwrap_or(0);
        out.push(set.remove(next));
    }
    out
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
        .map(|set| Family {
            members: newest_order(members, set),
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
    fn a_check_that_named_the_other_file_as_newer_decides_before_the_upload_times() {
        // A pasted file URL pins a file id and no upload time; the older package's check
        // named that file as newer, so it leads although its own upload time sorts first.
        let older = link(2207, Some(1), None, Some(1_000));
        let mut pasted = link(2207, Some(2410), None, None);
        pasted.source = LinkSource::Manual;
        let members = [
            member("Lantern Glow 1.2.zip", Some(&older), Some(2410)),
            member("Lantern Glow 1.3.zip", Some(&pasted), None),
        ];
        let families = families(&members);
        assert_eq!(
            families,
            [Family {
                members: vec![1, 0]
            }]
        );
        assert_eq!(families[0].lead(), 1);
    }

    #[test]
    fn a_named_pair_keeps_its_order_among_versions_ordered_by_upload_time() {
        // v1 named v2 newer; v3 is ordered by upload time only. v2 has no upload time,
        // so recency alone would put it last, below the v1 that named it newer.
        let v1 = link(7140, Some(1), Some(9), Some(100));
        let v2 = link(7140, Some(2), Some(9), None);
        let v3 = link(7140, Some(3), Some(9), Some(300));
        let members = [
            member("v1.zip", Some(&v1), Some(2)),
            member("v2.zip", Some(&v2), None),
            member("v3.zip", Some(&v3), None),
        ];
        assert_eq!(
            families(&members),
            [Family {
                members: vec![2, 1, 0]
            }]
        );
    }

    #[test]
    fn a_version_between_the_named_pair_never_leads_over_them() {
        // The General's round 2: A (uploaded 100) named B newer, and B has no upload time.
        // X (uploaded 50) is older than A. B is newer than A by proof, A newer than X by
        // time, so B leads; recency alone put A first, and A waiting on B let X out first.
        let x = link(7140, Some(3), Some(9), Some(50));
        let a = link(7140, Some(1), Some(9), Some(100));
        let b = link(7140, Some(2), Some(9), None);
        let members = [
            member("x.zip", Some(&x), None),
            member("a.zip", Some(&a), Some(2)),
            member("b.zip", Some(&b), None),
        ];
        assert_eq!(
            families(&members),
            [Family {
                members: vec![2, 1, 0]
            }]
        );
    }

    #[test]
    fn a_chain_of_proofs_carries_the_newest_time_up_the_chain() {
        // A (uploaded 300) named B newer, B named C newer; neither B nor C has a time.
        // Y (uploaded 200) sits between by time only. The chain leads: C, B, A, then Y.
        let a = link(7140, Some(1), Some(9), Some(300));
        let b = link(7140, Some(2), Some(9), None);
        let c = link(7140, Some(3), Some(9), None);
        let y = link(7140, Some(4), Some(9), Some(200));
        let members = [
            member("y.zip", Some(&y), None),
            member("a.zip", Some(&a), Some(2)),
            member("b.zip", Some(&b), Some(3)),
            member("c.zip", Some(&c), None),
        ];
        assert_eq!(
            families(&members),
            [Family {
                members: vec![3, 2, 1, 0]
            }]
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
