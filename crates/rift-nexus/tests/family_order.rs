//! The family order as a property, not a list of cases (the General's rounds 3 and 4 on PR #2):
//! three rounds each found the hand-picked cases missed the next shape, so this gate draws 30,000
//! invented families of two to five versions, with random upload times and file ids (some
//! repeated), chains of named-newer proofs, cycles of them, proofs naming files outside the
//! family, and now and then a package of another mod beside them, and holds the order to what the
//! rule promises. The oracle below works the promise out on its own, by search, and shares no
//! code with the rule.
//!
//! A block is one member, or the members whose proofs reach each other (a cycle). A member ranks
//! before another by its own recency, newest first, then by file name.
//!
//! What every order must keep:
//! 1. every member of the family, once, and no package of another mod;
//! 2. a proof: when a member's check names another's file as newer, and no proof leads back, the
//!    named member comes first;
//! 3. priority never rises down the order. A member's priority is the newest recency among itself
//!    and every member whose proofs reach it, so the newest version in the family leads and a
//!    version between a proven pair never leads over them;
//! 4. the same order however the members are given;
//! 5. each block sits together;
//! 6. inside a block, members go by rank;
//! 7. between two blocks of one priority, a block whose first member ranks higher comes later only
//!    when it had to wait: one of its members reaches a member outside it that was not yet placed
//!    when the earlier block went.

use rift_core::{LinkSource, NexusLink};
use rift_nexus::{families, FamilyMember};

/// xorshift64*: a fixed seed, so a failure is the same failure on every bench.
struct Draw(u64);

impl Draw {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }

    fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = usize::try_from(self.below(i as u64 + 1)).unwrap();
            items.swap(i, j);
        }
    }
}

struct Invented {
    names: Vec<String>,
    links: Vec<NexusLink>,
    newer: Vec<Option<u64>>,
    /// The family's members are the first `family`; a package after them is another mod's.
    family: usize,
}

/// One family: one mod, one file group, so the group ties every member and only the order is
/// under test. Upload times come from a small range so ties happen; some members state none,
/// and now and then two share a file id. A quarter of the draws add a package of another mod in
/// the same group number, with a file id that may be a member's: it must stay out of the family.
fn invent(draw: &mut Draw) -> Invented {
    let n = 2 + usize::try_from(draw.below(4)).unwrap();
    let mut file_ids: Vec<u64> = (1..=n as u64).collect();
    draw.shuffle(&mut file_ids);
    if draw.below(6) == 0 {
        file_ids[0] = file_ids[1];
    }
    let outsider = draw.below(4) == 0;
    let mut names = Vec::new();
    let mut links = Vec::new();
    let mut newer = Vec::new();
    let inside_ids = file_ids.iter().map(|&id| Some(id));
    let outside_id = outsider.then_some(None);
    for (i, own_id) in inside_ids.chain(outside_id).enumerate() {
        let inside = own_id.is_some();
        names.push(format!("v{i}.zip"));
        links.push(NexusLink {
            mod_id: if inside { 7140 } else { 9001 },
            file_id: (draw.below(8) != 0)
                .then(|| own_id.unwrap_or_else(|| 1 + draw.below(n as u64))),
            group_id: Some(9),
            sqid: None,
            version: None,
            uploaded_ms: (draw.below(3) != 0).then(|| 10 * draw.below(5)),
            source: LinkSource::Download,
        });
        newer.push(match draw.below(6) {
            0..=2 => None,
            3 | 4 => Some(1 + draw.below(n as u64)),
            _ => Some(100 + draw.below(3)),
        });
    }
    Invented {
        names,
        links,
        newer,
        family: n,
    }
}

fn members(inv: &Invented) -> Vec<FamilyMember<'_>> {
    (0..inv.names.len())
        .map(|i| FamilyMember {
            file_name: &inv.names[i],
            link: Some(&inv.links[i]),
            newer_file_id: inv.newer[i],
        })
        .collect()
}

/// The family's order as file names, for the members given in `given` order.
fn order(inv: &Invented, given: &[usize]) -> Vec<String> {
    let all = members(inv);
    let shuffled: Vec<FamilyMember> = given.iter().map(|&i| all[i]).collect();
    let found = families(&shuffled);
    assert_eq!(
        found.len(),
        1,
        "one group ties one family; another mod's package joins none"
    );
    found[0]
        .members
        .iter()
        .map(|&k| shuffled[k].file_name.to_string())
        .collect()
}

type Recency = (Option<u64>, Option<u64>);

/// The oracle: which member's proofs reach which, by search, over the family's members only.
fn reaches(inv: &Invented) -> Vec<Vec<bool>> {
    let n = inv.family;
    let names_newer = |a: usize, b: usize| matches!((inv.newer[a], inv.links[b].file_id), (Some(x), Some(y)) if x == y);
    let mut reach = vec![vec![false; n]; n];
    for (start, row) in reach.iter_mut().enumerate() {
        let mut stack = vec![start];
        while let Some(at) = stack.pop() {
            for (next, seen) in row.iter_mut().enumerate() {
                if next != at && names_newer(at, next) && !*seen {
                    *seen = true;
                    stack.push(next);
                }
            }
        }
    }
    reach
}

fn check(inv: &Invented, draw: &mut Draw, case: usize) {
    let n = inv.family;
    let all = inv.names.len();
    let reach = reaches(inv);
    let recency = |i: usize| -> Recency { (inv.links[i].uploaded_ms, inv.links[i].file_id) };
    let priority = |i: usize| -> Recency {
        (0..n)
            .filter(|&k| k == i || reach[k][i])
            .map(recency)
            .max()
            .unwrap()
    };
    let given: Vec<usize> = (0..all).collect();
    let first = order(inv, &given);
    let at = |name: &str| first.iter().position(|f| f == name).unwrap();
    let context = || {
        let rows: Vec<String> = (0..all)
            .map(|i| {
                format!(
                    "{} mod {} file {:?} up {:?} newer {:?}",
                    inv.names[i],
                    inv.links[i].mod_id,
                    inv.links[i].file_id,
                    inv.links[i].uploaded_ms,
                    inv.newer[i]
                )
            })
            .collect();
        format!("case {case}: {rows:?} → {first:?}")
    };

    // 1. every member of the family, once, and no package of another mod
    let mut sorted = first.clone();
    sorted.sort();
    let mut expected = inv.names[..n].to_vec();
    expected.sort();
    assert_eq!(sorted, expected, "{}", context());

    // 2. a proof that no proof leads back from puts the named member first
    for (a, row) in reach.iter().enumerate() {
        for (b, &a_reaches_b) in row.iter().enumerate() {
            if a_reaches_b && !reach[b][a] {
                assert!(
                    at(&inv.names[b]) < at(&inv.names[a]),
                    "{} names {} newer: {}",
                    inv.names[a],
                    inv.names[b],
                    context()
                );
            }
        }
    }

    // 3. priority never rises down the order
    let by_name = |name: &str| inv.names.iter().position(|f| f == name).unwrap();
    for pair in first.windows(2) {
        assert!(
            priority(by_name(&pair[0])) >= priority(by_name(&pair[1])),
            "{} before {}: {}",
            pair[0],
            pair[1],
            context()
        );
    }

    // 5–7: blocks, rank inside them, and rank between blocks of one priority
    let together = |a: usize, b: usize| a == b || (reach[a][b] && reach[b][a]);
    // rank: own recency, newest first, then file name
    let ranks_before = |a: usize, b: usize| {
        recency(a) > recency(b) || (recency(a) == recency(b) && inv.names[a] < inv.names[b])
    };
    let placed: Vec<usize> = first.iter().map(|f| by_name(f)).collect();
    let pos = |m: usize| placed.iter().position(|&p| p == m).unwrap();
    for pair in placed.windows(2) {
        let (x, y) = (pair[0], pair[1]);
        if together(x, y) {
            assert!(
                ranks_before(x, y),
                "{} before {} in one cycle: {}",
                inv.names[x],
                inv.names[y],
                context()
            );
        }
    }
    let heads: Vec<usize> = placed
        .iter()
        .copied()
        .filter(|&m| (0..n).all(|k| !together(k, m) || pos(k) >= pos(m)))
        .collect();
    for &m in &placed {
        let head = heads.iter().copied().find(|&h| together(h, m)).unwrap();
        let block_len = (0..n).filter(|&k| together(k, m)).count();
        assert!(
            pos(m) - pos(head) < block_len,
            "{}'s cycle does not sit together: {}",
            inv.names[m],
            context()
        );
    }
    for (i, &x) in heads.iter().enumerate() {
        for &y in &heads[i + 1..] {
            if priority(x) == priority(y) && ranks_before(y, x) {
                let waited = (0..n).any(|m| {
                    together(m, y)
                        && (0..n).any(|z| reach[m][z] && !together(z, y) && pos(z) >= pos(x))
                });
                assert!(
                    waited,
                    "{} outranks {} at one priority and waited on nothing: {}",
                    inv.names[y],
                    inv.names[x],
                    context()
                );
            }
        }
    }

    // 4. the same order however the members are given
    for _ in 0..4 {
        let mut shuffled = given.clone();
        draw.shuffle(&mut shuffled);
        assert_eq!(
            order(inv, &shuffled),
            first,
            "given {shuffled:?}: {}",
            context()
        );
    }
}

#[test]
fn every_family_order_keeps_its_proofs_its_priority_and_its_order_however_it_is_given() {
    let mut draw = Draw(0x5249_4654_5354_4f4e);
    let mut cycles = 0;
    let mut outsiders = 0;
    for case in 0..30_000 {
        let inv = invent(&mut draw);
        let reach = reaches(&inv);
        if (0..inv.family).any(|i| reach[i][i]) {
            cycles += 1;
        }
        if inv.names.len() > inv.family {
            outsiders += 1;
        }
        check(&inv, &mut draw, case);
    }
    // the draw reaches the shape that broke round 3, and often
    assert!(
        cycles > 500,
        "only {cycles} families with a cycle of proofs"
    );
    assert!(
        outsiders > 5_000,
        "only {outsiders} draws with another mod's package"
    );
}
