//! Bench: live looks at Nexus Mods. Never part of the default test run. The
//! lookup is `#[ignore]`d AND gated on `RIFT_BENCH_NEXUS_MOD` naming the
//! Dragon's Dogma 2 mod to ask about (the id a download of yours carries); the
//! name search has its own gate (see `live_find_by_an_invented_phrase_and_a_generic_word`):
//!
//! ```sh
//! RIFT_BENCH_NEXUS_MOD=<mod id> RIFT_BENCH_NEXUS_SQID=<sqid, optional> \
//!   cargo test -p rift-nexus --test bench_real -- --ignored --nocapture
//! ```
//!
//! Exactly two anonymous requests: one `mods(&[id])`, one `mod_files(&[id])`.
//! Read-only by nature — the Nexus API is only asked, never told. The id comes
//! from whoever runs it, never from this file: the repo is public, and a
//! player's library is their own business. For the same reason it prints
//! counts and states only — no mod name, no file name, no version.

use rift_core::{LinkSource, NexusLink};
use rift_nexus::{pin_link, update_state, NexusClient, UpdateState, CANDIDATES_PER_FIND};

/// One live look at the name search (v0.2, Find on Nexus): the measured
/// `mods(filter, sort, count)` shape still answers. `#[ignore]`d AND gated on
/// `RIFT_BENCH_NEXUS_FIND=1`:
///
/// ```sh
/// RIFT_BENCH_NEXUS_FIND=1 cargo test -p rift-nexus --test bench_real -- --ignored --nocapture
/// ```
///
/// The phrases are fixed HERE, never read from the environment: an invented
/// one (`lantern glow`) and a generic word (`armor`), so no name from a
/// player's library can reach Nexus through this bench. At most four
/// anonymous requests. Counts only.
#[test]
#[ignore = "talks to the live Nexus Mods API; run with RIFT_BENCH_NEXUS_FIND=1 and --ignored"]
fn live_find_by_an_invented_phrase_and_a_generic_word() {
    if std::env::var("RIFT_BENCH_NEXUS_FIND").ok().as_deref() != Some("1") {
        eprintln!("RIFT_BENCH_NEXUS_FIND is not 1 — the live find bench stays quiet.");
        return;
    }
    let client = NexusClient::new(env!("CARGO_PKG_VERSION"));
    for phrase in ["lantern glow", "armor"] {
        let found = client
            .find_mods(phrase)
            .unwrap_or_else(|err| panic!("the name search for {phrase:?} answered: {err}"));
        println!(
            "live find {phrase:?}: {} candidate(s); {} with an author, {} with a category, \
             {} with a summary, {} with an update time, {} with downloads, {} adult",
            found.len(),
            found.iter().filter(|c| c.author.is_some()).count(),
            found.iter().filter(|c| c.category.is_some()).count(),
            found.iter().filter(|c| c.summary.is_some()).count(),
            found.iter().filter(|c| c.updated_ms.is_some()).count(),
            found.iter().filter(|c| c.downloads.is_some()).count(),
            found.iter().filter(|c| c.adult).count(),
        );
        assert!(found.len() <= CANDIDATES_PER_FIND);
        assert!(found.iter().all(|c| c.mod_id > 0 && !c.name.is_empty()));
    }
}

#[test]
#[ignore = "talks to the live Nexus Mods API; run with RIFT_BENCH_NEXUS_MOD=<id> and --ignored"]
fn live_lookup_of_one_mod() {
    let Some(mod_id) = std::env::var("RIFT_BENCH_NEXUS_MOD")
        .ok()
        .and_then(|id| id.trim().parse::<u64>().ok())
    else {
        eprintln!("RIFT_BENCH_NEXUS_MOD is not a mod id — the live Nexus bench stays quiet.");
        return;
    };
    let sqid = std::env::var("RIFT_BENCH_NEXUS_SQID").ok();
    let client = NexusClient::new(env!("CARGO_PKG_VERSION"));

    let mods = client.mods(&[mod_id]).expect("legacyModsByDomain answered");
    let files = client.mod_files(&[mod_id]).expect("modFiles answered");
    let list = files.get(&mod_id).map(Vec::as_slice).unwrap_or(&[]);
    let groups: std::collections::BTreeSet<_> = list.iter().map(|file| file.group_id).collect();
    println!(
        "live: {} mod record(s), {} file(s) in {} group(s), {} with a sqid, {} primary",
        mods.len(),
        list.len(),
        groups.len(),
        list.iter().filter(|file| file.sqid.is_some()).count(),
        list.iter().filter(|file| file.primary).count()
    );

    // Pin by the given sqid (a 2026 download name's last token), else by the
    // oldest file's own sqid, and ask for the state — offline, on the live list.
    let wanted = sqid.or_else(|| list.first().and_then(|file| file.sqid.clone()));
    let local = NexusLink {
        mod_id,
        file_id: None,
        group_id: None,
        sqid: wanted,
        version: None,
        uploaded_ms: None,
        source: LinkSource::Filename,
    };
    match pin_link(&local, list) {
        Some(pinned) => {
            // The state's name only: its payload carries the newer file's name.
            let state = match update_state(&pinned, mods.first(), list) {
                UpdateState::Current => "current",
                UpdateState::UpdateAvailable { .. } => "update available",
                UpdateState::LocalArchived { .. } => "local file retired",
                UpdateState::Unpinned => "unpinned",
                UpdateState::ModUnavailable { .. } => "mod unavailable",
            };
            println!("pinned by sqid: yes; state: {state}");
        }
        None => println!("pinned by sqid: no"),
    }

    assert!(
        mods.iter().any(|m| m.mod_id == mod_id),
        "Nexus should know the mod"
    );
    assert!(!list.is_empty(), "the mod should list files");
}
