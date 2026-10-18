//! The key's extras on Nexus (v0.3): the mods the account tracks and its
//! endorsements — `get_nexus_standings`, `set_tracked`, `set_endorsement`.
//!
//! Every one of these is a metered v1 call, so the spine asks Nexus only
//! when the window asks: the standings are cached for the session
//! ([`crate::account::StandingsCache`]) and `refresh` asks again. The tracked
//! mods are named through one anonymous v2 lookup (no quota). A change
//! (track, untrack, endorse, abstain) answers with the standings as they
//! stand after it, patched in place: no second round of v1 calls.

use std::collections::HashMap;
use std::sync::atomic::Ordering;

use rift_core::RiftError;
use rift_nexus::{NexusClient, NexusError, NexusMod};

use crate::account::StandingsCache;
use crate::commands::{blocking, Spine};
use crate::dto::{EndorseStatus, EndorsementView, NexusCandidate, NexusStandings};
use crate::events;
use crate::nexus::nexus_error;
use crate::persist;
use crate::state::{lock, read, AppState};

/// The client with the key, or the `noKey` refusal.
fn keyed_client(state: &AppState) -> Result<NexusClient, RiftError> {
    if !state.has_api_key.load(Ordering::SeqCst) {
        return Err(NexusError::NoApiKey.into());
    }
    let client = read(&state.nexus_client).clone();
    if !client.has_api_key() {
        return Err(NexusError::NoApiKey.into());
    }
    Ok(client)
}

/// How many packages of the library are linked to each mod right now.
pub fn linked_counts(state: &AppState) -> HashMap<u64, usize> {
    let shelf = state.shelf();
    let store = lock(&state.nexus);
    let mut counts = HashMap::new();
    for package in shelf.packages() {
        if let Some(link) = store.link_for(package) {
            *counts.entry(link.mod_id).or_insert(0) += 1;
        }
    }
    counts
}

/// The standings as the window reads them, with each tracked mod's linked
/// packages counted now.
fn standings_view(state: &AppState, cache: &StandingsCache) -> NexusStandings {
    let linked = linked_counts(state);
    NexusStandings {
        tracked: cache
            .tracked
            .iter()
            .map(|(mod_id, known)| {
                NexusCandidate::of_tracked(
                    *mod_id,
                    known.as_ref(),
                    linked.get(mod_id).copied().unwrap_or(0),
                )
            })
            .collect(),
        endorsements: cache
            .endorsements
            .iter()
            .map(|(mod_id, status)| EndorsementView {
                mod_id: *mod_id,
                status: (*status).into(),
            })
            .collect(),
        checked_ms: cache.checked_ms,
    }
}

/// Ask Nexus for the standings: tracked mods and endorsements (v1, two
/// metered calls), the tracked mods named in one v2 lookup.
fn ask(client: &NexusClient, epoch: u64) -> Result<StandingsCache, RiftError> {
    let tracked = client.tracked_mods().map_err(nexus_error)?;
    let endorsements = client.endorsements().map_err(nexus_error)?;
    let mut named: HashMap<u64, NexusMod> = if tracked.is_empty() {
        HashMap::new()
    } else {
        client
            .mods(&tracked)
            .map_err(nexus_error)?
            .into_iter()
            .map(|found| (found.mod_id, found))
            .collect()
    };
    Ok(StandingsCache {
        epoch,
        tracked: tracked
            .into_iter()
            .map(|mod_id| (mod_id, named.remove(&mod_id)))
            .collect(),
        endorsements,
        checked_ms: persist::now_ms(),
    })
}

/// Keep `cache` for the session, unless the key changed while Nexus answered.
fn keep(state: &AppState, cache: &StandingsCache) {
    let epoch = lock(&state.account).epoch;
    if cache.epoch == epoch {
        *lock(&state.standings) = Some(cache.clone());
    }
}

/// The cached standings of the key in use, if any.
fn cached(state: &AppState, epoch: u64) -> Option<StandingsCache> {
    lock(&state.standings)
        .as_ref()
        .filter(|cache| cache.epoch == epoch)
        .cloned()
}

/// `get_nexus_standings`: the session's cache, or Nexus again with
/// `refresh` (or when nothing was heard yet). No key → `noKey`.
pub fn get_nexus_standings_impl(
    state: &AppState,
    refresh: bool,
) -> Result<NexusStandings, RiftError> {
    let epoch = lock(&state.account).epoch;
    let client = keyed_client(state)?;
    if !refresh {
        if let Some(cache) = cached(state, epoch) {
            return Ok(standings_view(state, &cache));
        }
    }
    let cache = ask(&client, epoch)?;
    keep(state, &cache);
    Ok(standings_view(state, &cache))
}

/// The standings after a change Nexus accepted: the cache patched in place,
/// or (nothing heard yet this session) asked whole.
fn after_change(
    state: &AppState,
    client: &NexusClient,
    epoch: u64,
    patch: impl FnOnce(&mut StandingsCache),
) -> Result<NexusStandings, RiftError> {
    let cache = match cached(state, epoch) {
        Some(mut cache) => {
            patch(&mut cache);
            cache.checked_ms = persist::now_ms();
            cache
        }
        None => ask(client, epoch)?,
    };
    keep(state, &cache);
    state.events.changed(events::UPDATES_CHANGED);
    Ok(standings_view(state, &cache))
}

/// `set_tracked`: track or untrack a mod on Nexus (v1 POST/DELETE
/// `user/tracked_mods.json?domain_name=dragonsdogma2`).
pub fn set_tracked_impl(
    state: &AppState,
    mod_id: u64,
    tracked: bool,
) -> Result<NexusStandings, RiftError> {
    let epoch = lock(&state.account).epoch;
    let client = keyed_client(state)?;
    if tracked {
        client.track(mod_id).map_err(nexus_error)?;
    } else {
        client.untrack(mod_id).map_err(nexus_error)?;
    }
    // A newly tracked mod's card: one anonymous lookup, a name at best.
    let card = if tracked {
        match client.mods(&[mod_id]) {
            Ok(found) => found.into_iter().find(|found| found.mod_id == mod_id),
            Err(err) => {
                log::warn!("mod {mod_id} is tracked, but its name could not be read: {err}");
                None
            }
        }
    } else {
        None
    };
    after_change(state, &client, epoch, |cache| {
        if tracked {
            if !cache.tracked.iter().any(|(id, _)| *id == mod_id) {
                cache.tracked.push((mod_id, card));
            }
        } else {
            cache.tracked.retain(|(id, _)| *id != mod_id);
        }
    })
}

/// The version an endorsement names: the one the library holds (the first
/// linked package that knows its version), else the mod's current version
/// from Nexus (anonymous v2), else none.
fn endorse_version(
    state: &AppState,
    client: &NexusClient,
    mod_id: u64,
) -> Result<String, RiftError> {
    let local = {
        let shelf = state.shelf();
        let store = lock(&state.nexus);
        shelf
            .packages()
            .iter()
            .filter(|package| {
                store
                    .link_for(package)
                    .is_some_and(|link| link.mod_id == mod_id)
            })
            .find_map(|package| {
                store
                    .row(package)
                    .local_version
                    .filter(|version| !version.trim().is_empty())
            })
    };
    if let Some(version) = local {
        return Ok(version);
    }
    Ok(client
        .mods(&[mod_id])
        .map_err(nexus_error)?
        .into_iter()
        .find(|found| found.mod_id == mod_id)
        .and_then(|found| found.version)
        .unwrap_or_default())
}

/// `set_endorsement`: endorse a mod or abstain (v1 `endorse.json` /
/// `abstain.json`), naming the version the library holds. Nexus's refusals
/// come back with their reasons (`endorseNotDownloaded`, `endorseTooSoon`,
/// `endorseOwnMod`).
pub fn set_endorsement_impl(
    state: &AppState,
    mod_id: u64,
    status: EndorseStatus,
) -> Result<NexusStandings, RiftError> {
    let epoch = lock(&state.account).epoch;
    let client = keyed_client(state)?;
    let version = endorse_version(state, &client, mod_id)?;
    match status {
        EndorseStatus::Endorsed => client.endorse(mod_id, &version),
        EndorseStatus::Abstained => client.abstain(mod_id, &version),
    }
    .map_err(nexus_error)?;
    let decided: rift_nexus::EndorseStatus = status.into();
    after_change(state, &client, epoch, |cache| {
        match cache.endorsements.iter_mut().find(|(id, _)| *id == mod_id) {
            Some(entry) => entry.1 = decided,
            None => cache.endorsements.push((mod_id, decided)),
        }
    })
}

// -- wrappers ----------------------------------------------------------------

#[tauri::command]
pub async fn get_nexus_standings(
    state: Spine<'_>,
    refresh: bool,
) -> Result<NexusStandings, RiftError> {
    blocking(&state, move |state| {
        get_nexus_standings_impl(state, refresh)
    })
    .await
}

#[tauri::command]
pub async fn set_tracked(
    state: Spine<'_>,
    mod_id: u64,
    tracked: bool,
) -> Result<NexusStandings, RiftError> {
    blocking(&state, move |state| {
        set_tracked_impl(state, mod_id, tracked)
    })
    .await
}

#[tauri::command]
pub async fn set_endorsement(
    state: Spine<'_>,
    mod_id: u64,
    status: EndorseStatus,
) -> Result<NexusStandings, RiftError> {
    blocking(&state, move |state| {
        set_endorsement_impl(state, mod_id, status)
    })
    .await
}
