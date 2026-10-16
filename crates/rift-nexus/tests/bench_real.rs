//! Bench: one live look at Nexus Mods. Never part of the default test run —
//! `#[ignore]`d AND gated on `RIFT_BENCH_NEXUS_MOD` naming the Dragon's Dogma 2
//! mod to ask about (the id a download of yours carries):
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
use rift_nexus::{pin_link, update_state, NexusClient, UpdateState};

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
