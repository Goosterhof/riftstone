//! READ-ONLY bench: a COPY of a real save and the real game pak. Ignored and env-gated.
//!
//! RIFT_BENCH_SAVE=<a copy of a slot> RIFT_BENCH_SAVE_ACCOUNT=<account id> \
//! RIFT_BENCH_GAME_PAK=<the game folder>/re_chunk_000.pak \
//!   cargo test -p rift-muster --test bench_real -- --ignored --nocapture
//!
//! Prints counts and how many things got a name, never a name, an id or a value.

use rift_muster::{read_slot, Codex, Member};

fn named(member: &Member, codex: &Codex) -> (usize, usize) {
    let mut total = 0;
    let mut named = 0;
    let mut count = |found: bool| {
        total += 1;
        named += usize::from(found);
    };
    for id in member.gear.iter().flatten() {
        count(codex.item(*id).is_some());
    }
    for id in &member.augments {
        count(codex.augment(*id).is_some());
    }
    for id in &member.skills {
        count(codex.skill(*id).is_some());
    }
    count(codex.vocation(member.vocation).is_some());
    if let Some(traits) = member.pawn {
        if let Some(i) = traits.inclination {
            count(codex.inclination(i).is_some());
        }
        if let Some(s) = traits.specialization {
            count(codex.specialization(s).is_some());
        }
    }
    (named, total)
}

#[test]
#[ignore = "bench: needs RIFT_BENCH_SAVE, RIFT_BENCH_SAVE_ACCOUNT and RIFT_BENCH_GAME_PAK"]
fn a_real_party_is_read_and_named() {
    let (Ok(save), Ok(account), Ok(pak)) = (
        std::env::var("RIFT_BENCH_SAVE"),
        std::env::var("RIFT_BENCH_SAVE_ACCOUNT"),
        std::env::var("RIFT_BENCH_GAME_PAK"),
    ) else {
        eprintln!("bench skipped: the three variables are not set");
        return;
    };
    let bytes = std::fs::read(&save).expect("read the copy");
    let muster =
        read_slot(&bytes, account.trim().parse().expect("a number")).expect("the party reads");
    let pak =
        rift_pak::game::GamePak::open(std::path::Path::new(&pak)).expect("the game pak opens");
    let codex = Codex::from_pak(&pak).expect("the codex builds");
    println!("bench: codex counts (items, vocations, inclinations, specializations, augments, skills) {:?}", codex.counts());
    let members: Vec<&Member> = muster
        .arisen
        .iter()
        .chain(muster.main_pawn.iter())
        .chain(muster.hired.iter())
        .collect();
    println!(
        "bench: {} members ({} hired); gold {}, crystals {}",
        members.len(),
        muster.hired.len(),
        if muster.gold.is_some() {
            "read"
        } else {
            "absent"
        },
        if muster.rift_crystals.is_some() {
            "read"
        } else {
            "absent"
        }
    );
    for m in &members {
        let (named, total) = named(m, &codex);
        println!(
            "bench: {:?}: name {}, gear {}/8, augments {}, skills {}, named {named}/{total}",
            m.role,
            if m.name.is_some() { "set" } else { "absent" },
            m.gear.iter().flatten().count(),
            m.augments.len(),
            m.skills.len()
        );
    }
    assert!(muster.arisen.is_some());
}
