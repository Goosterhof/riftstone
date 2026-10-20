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
//! the rule holds: the named member is at least as new as the member that named it, whatever its
//! own link says (a link pasted as a file URL carries a file id and no upload time). The order
//! keeps every proof, condensing the members whose proofs run in a circle (two checks at
//! different times can name each other) into one block, and among the blocks a proof leaves free
//! it takes the one with the highest priority first. A member's priority is the newest recency
//! among itself and every member whose proofs reach it, so the newest version leads and a
//! version between a proven pair never leads over them. Recency is the latest upload time, then
//! the higher file id; a member whose link states neither, and that no proof lifts, comes last.
//! Ties fall to the member's own recency, then the file name, so the order is the same however
//! the members are given. `tests/family_order.rs` holds the rule to all of that over 30,000
//! invented families.

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

/// One family's members, newest first (the rule in the module's head).
///
/// 1. `reach[a][b]`: `a`'s proofs reach `b`, directly or through others; `b` must come before
///    `a` unless `b`'s proofs reach `a` back, which puts the two in one block.
/// 2. A member's priority is the newest recency among itself and every member that reaches it.
///    A named member's priority is never below its namer's, so the order below is sorted by
///    priority as well as keeping every proof.
/// 3. A block is ready once every member its members reach outside it is placed. The ready block
///    with the highest priority goes next, then the higher own recency, then the first file name;
///    inside a block, own recency, then file name.
///
/// The family is a handful of versions, so the search is a plain closure over a small matrix.
fn newest_order(members: &[FamilyMember], set: Vec<usize>) -> Vec<usize> {
    let n = set.len();
    let at = |k: usize| &members[set[k]];
    let mut reach = vec![vec![false; n]; n];
    for (a, row) in reach.iter_mut().enumerate() {
        for (b, cell) in row.iter_mut().enumerate() {
            *cell = a != b && named_newer(at(a), at(b));
        }
    }
    for via in 0..n {
        let onward = reach[via].clone();
        for row in &mut reach {
            if row[via] {
                for (cell, &next) in row.iter_mut().zip(&onward) {
                    *cell |= next;
                }
            }
        }
    }
    let priority: Vec<Recency> = (0..n)
        .map(|k| {
            (0..n)
                .filter(|&j| j == k || reach[j][k])
                .map(|j| recency(at(j)))
                .max()
                .unwrap_or((None, None))
        })
        .collect();
    let together = |a: usize, b: usize| a == b || (reach[a][b] && reach[b][a]);
    // Each member's block, by its first member; a member's place inside it, by own recency.
    let own = |a: usize, b: usize| {
        recency(at(b))
            .cmp(&recency(at(a)))
            .then_with(|| at(a).file_name.cmp(at(b).file_name))
    };
    let mut blocks: Vec<Vec<usize>> = Vec::new();
    for k in 0..n {
        match blocks.iter_mut().find(|block| together(block[0], k)) {
            Some(block) => block.push(k),
            None => blocks.push(vec![k]),
        }
    }
    for block in &mut blocks {
        block.sort_by(|&a, &b| own(a, b));
    }
    let mut placed = vec![false; n];
    let mut out = Vec::with_capacity(n);
    while !blocks.is_empty() {
        let ready = |block: &Vec<usize>| {
            block
                .iter()
                .all(|&a| (0..n).all(|b| !reach[a][b] || placed[b] || block.contains(&b)))
        };
        // A block's key: its priority (one for all its members), then its newest member.
        let key = |block: &Vec<usize>| (priority[block[0]], block[0]);
        let next = blocks
            .iter()
            .enumerate()
            .filter(|(_, block)| ready(block))
            .min_by(|(_, x), (_, y)| {
                key(y)
                    .0
                    .cmp(&key(x).0)
                    .then_with(|| own(key(x).1, key(y).1))
            })
            .map(|(i, _)| i)
            .expect("the blocks of a family form no cycle, so one is always ready");
        for k in blocks.remove(next) {
            placed[k] = true;
            out.push(set[k]);
        }
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

    /// The family's file names, newest first, for members given in this order.
    fn order_of(members: &[FamilyMember]) -> Vec<String> {
        let families = families(members);
        assert_eq!(families.len(), 1, "one family");
        families[0]
            .members
            .iter()
            .map(|&i| members[i].file_name.to_string())
            .collect()
    }

    /// Every ordering of `members`, by Heap's algorithm.
    fn permutations<'a>(members: &[FamilyMember<'a>]) -> Vec<Vec<FamilyMember<'a>>> {
        fn heap<'a>(
            k: usize,
            items: &mut Vec<FamilyMember<'a>>,
            out: &mut Vec<Vec<FamilyMember<'a>>>,
        ) {
            if k <= 1 {
                out.push(items.clone());
                return;
            }
            for i in 0..k {
                heap(k - 1, items, out);
                let j = if k % 2 == 0 { i } else { 0 };
                items.swap(j, k - 1);
            }
        }
        let mut out = Vec::new();
        heap(members.len(), &mut members.to_vec(), &mut out);
        out
    }

    #[test]
    fn a_cycle_of_two_proofs_ends_and_leads_the_same_however_the_members_are_given() {
        // Two checks at different times, after a re-upload in one group: each named the other
        // newer. The lift ends (both rise to the newer recency, then nothing moves), and the
        // tie falls to each one's own recency: the later upload leads.
        let a = link(7140, Some(1), Some(9), Some(100));
        let b = link(7140, Some(2), Some(9), Some(200));
        let members = [
            member("a.zip", Some(&a), Some(2)),
            member("b.zip", Some(&b), Some(1)),
        ];
        for given in permutations(&members) {
            assert_eq!(order_of(&given), ["b.zip", "a.zip"]);
        }
    }

    #[test]
    fn a_cycle_of_three_proofs_ends_and_leads_the_same_however_the_members_are_given() {
        // A named B, B named C, C named A; one of them states no upload time, and an outsider
        // sits below them by time. Every ordering of the input gives the same family order.
        let a = link(7140, Some(1), Some(9), Some(100));
        let b = link(7140, Some(2), Some(9), None);
        let c = link(7140, Some(3), Some(9), Some(300));
        let x = link(7140, Some(4), Some(9), Some(50));
        let members = [
            member("a.zip", Some(&a), Some(2)),
            member("b.zip", Some(&b), Some(3)),
            member("c.zip", Some(&c), Some(1)),
            member("x.zip", Some(&x), None),
        ];
        let first = order_of(&members);
        assert_eq!(first.last().map(String::as_str), Some("x.zip"), "{first:?}");
        for given in permutations(&members) {
            assert_eq!(order_of(&given), first);
        }
    }

    #[test]
    fn a_proof_into_a_cycle_waits_for_the_cycle() {
        // The General's round 3: B and D name each other newer (a cycle), A named D newer, and C
        // is ordered by nothing. A must wait for D, cycle or no cycle; it went out first.
        let a = link(7140, Some(1), Some(9), Some(20));
        let b = link(7140, Some(2), Some(9), Some(10));
        let c = link(7140, Some(3), Some(9), None);
        let d = link(7140, Some(4), Some(9), None);
        let members = [
            member("a.zip", Some(&a), Some(4)),
            member("b.zip", Some(&b), Some(4)),
            member("c.zip", Some(&c), None),
            member("d.zip", Some(&d), Some(2)),
        ];
        assert_eq!(order_of(&members), ["b.zip", "d.zip", "a.zip", "c.zip"]);
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
