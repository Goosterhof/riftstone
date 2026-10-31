//! Update in its place (0.8, investor ruling 2026-10-31: *"can we also easily
//! update the mods from the built section? so i can itterate faster through
//! mod versions?"*; design #00015 §20.14, R20.30–R20.37).
//!
//! One press on a built card lands the new build (bring-through, unchanged:
//! by the digest shown), swaps the earlier build's installs for the new
//! build's matching options in one journaled step each, and then puts the
//! earlier zip away in `<mods> (older versions)`, v0.5's put-away.
//!
//! **Lineage is the stone's own record plus the player's press.** The stone
//! records every build it brought through ([`BroughtRecord`],
//! `pawn-brought.json`): its file, digest, size and the mod names inside it.
//! A new build's *earlier build* is the newest recorded one still in the mods
//! folder, with the same bytes, that shares a `modinfo.ini` name with it. So
//! only a build the stone itself brought through can ever be replaced, never
//! one of the player's other mods; the card names it, and the press is the
//! player's word. A build of the same file name with other bytes cannot be
//! swapped, because both must sit in the mods folder at once: bring-through's
//! own `nameTaken` refuses it, with the recovery (a new version in the name).
//!
//! What is inside a build is read from a copy the stone makes in its own
//! cache ([`crate::pawn::bring::stage_build`]), through the pinned `dist\`
//! handle, never from a path she could replace. It is cached by digest.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rift_core::{BringReason, OptionId, Package, PackageFormat, PackageId, RiftError};
use serde::{Deserialize, Serialize};

use crate::commands::deploy;
use crate::commands::versions;
use crate::dto::PutAwayView;
use crate::family::matching_option;
use crate::library::Shelf;
use crate::pawn::bring::{self, BringOutcome, BringRefusal};
use crate::pawn::desk::{new_run_id, WORKSHOP_CHANGED};
use crate::persist;
use crate::state::AppState;
use rift_deploy::Actor;

/// How many brought builds the record keeps, newest last.
pub const KEPT_BROUGHT: usize = 200;

/// One build the stone brought through, as `pawn-brought.json` keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Brought {
    pub file_name: String,
    pub sha256: String,
    pub bytes: u64,
    /// The `modinfo.ini` names inside it, lowercased and trimmed.
    pub names: Vec<String>,
    /// RFC 3339, UTC.
    pub at: String,
}

/// `pawn-brought.json`: the builds the stone brought through, newest last.
pub struct BroughtRecord {
    path: PathBuf,
    kept: Mutex<Vec<Brought>>,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl BroughtRecord {
    pub const NAME: &'static str = "pawn-brought.json";

    pub fn new(path: PathBuf) -> Self {
        let kept = persist::read_json::<Vec<Brought>>(&path).unwrap_or_default();
        Self {
            path,
            kept: Mutex::new(kept),
        }
    }

    pub fn all(&self) -> Vec<Brought> {
        lock(&self.kept).clone()
    }

    /// Record a build brought through: the same file and bytes again move
    /// to the end, and the oldest beyond [`KEPT_BROUGHT`] go.
    pub fn record(&self, brought: Brought) -> Result<(), RiftError> {
        let mut kept = lock(&self.kept);
        let mut next: Vec<Brought> = kept
            .iter()
            .filter(|b| {
                !(b.file_name.eq_ignore_ascii_case(&brought.file_name)
                    && b.sha256 == brought.sha256)
            })
            .cloned()
            .collect();
        next.push(brought);
        let excess = next.len().saturating_sub(KEPT_BROUGHT);
        next.drain(..excess);
        persist::write_json(&self.path, &next, true)?;
        *kept = next;
        Ok(())
    }
}

/// The builds read so far, by digest: what is inside each.
#[derive(Default)]
pub struct ReadBuilds {
    by_sha: Mutex<HashMap<String, Arc<Package>>>,
}

/// The `modinfo.ini` names of a package's options, lowercased and trimmed:
/// the mod's own name for itself, which is what makes two builds one mod.
pub fn mod_names(package: &Package) -> BTreeSet<String> {
    package
        .options
        .iter()
        .filter_map(|o| o.info.as_ref()?.name.as_deref())
        .map(|name| name.trim().to_lowercase())
        .filter(|name| !name.is_empty())
        .collect()
}

/// What is inside `artifact` now: the stone's copy, scanned as the mods
/// folder will scan it. Returns the copy's digest with the package.
pub fn read_build(
    state: &AppState,
    read: &ReadBuilds,
    workshop: &Path,
    artifact: &str,
) -> Result<(String, Arc<Package>), BringRefusal> {
    let staging = state
        .config
        .dirs
        .cache
        .join("workshop-staging")
        .join(new_run_id());
    let staged = bring::stage_build(workshop, artifact, &staging);
    let result = staged.and_then(|(path, sha, _)| {
        if let Some(known) = lock(&read.by_sha).get(&sha) {
            return Ok((sha, Arc::clone(known)));
        }
        let scanned = rift_catalog::scan::scan_package(&path).map_err(|err| BringRefusal {
            reason: BringReason::NotInDist,
            detail: format!("The stone could not read inside {artifact}: {err}"),
        })?;
        let package = Arc::new(scanned.package);
        lock(&read.by_sha).insert(sha.clone(), Arc::clone(&package));
        Ok((sha, package))
    });
    let _ = std::fs::remove_dir_all(&staging);
    result
}

/// The earlier build of a build named `artifact` holding the mod `names`:
/// the newest one the stone brought through that is still in the mods folder
/// with the same bytes and shares a name. A build of the same file name is
/// never its own earlier build.
pub fn earlier_build<'s>(
    record: &[Brought],
    shelf: &'s Shelf,
    names: &BTreeSet<String>,
    artifact: &str,
) -> Option<(Brought, &'s Package)> {
    if names.is_empty() {
        return None;
    }
    record.iter().rev().find_map(|brought| {
        if brought.file_name.eq_ignore_ascii_case(artifact)
            || !brought.names.iter().any(|n| names.contains(n))
        {
            return None;
        }
        let package = shelf.packages().iter().find(|p| {
            p.format != PackageFormat::Folder
                && p.file_name.eq_ignore_ascii_case(&brought.file_name)
                && p.size == brought.bytes
        })?;
        let (sha, _) = bring::digest_file(&package.path).ok()?;
        (sha == brought.sha256).then(|| (brought.clone(), package))
    })
}

/// One install the press swaps: the earlier build's option for the new one's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapView {
    pub from_option_id: String,
    pub to_option_id: String,
    pub name: String,
    pub from_version: Option<String>,
    pub to_version: Option<String>,
}

/// An install of the earlier build that stays: the new build has no option
/// that is the same option.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StayView {
    pub option_id: String,
    pub name: String,
}

/// The earlier build a press replaces, and what it does to its installs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EarlierView {
    pub file_name: String,
    pub package_id: String,
    pub swaps: Vec<SwapView>,
    pub stays: Vec<StayView>,
    /// The earlier zip goes to the older-versions folder (nothing of it
    /// stays installed).
    pub puts_away: bool,
}

fn version_of(option: &rift_core::ModOption) -> Option<String> {
    option.info.as_ref()?.version.clone()
}

/// The swaps and stays for `earlier`'s installs against `new`'s options.
fn swaps_of(
    earlier: &Package,
    new: &Package,
    installed: &[OptionId],
) -> (Vec<SwapView>, Vec<StayView>) {
    let mut swaps = Vec::new();
    let mut stays = Vec::new();
    for id in installed.iter().filter(|id| id.package() == earlier.id) {
        let Some(from) = earlier.options.iter().find(|o| &o.id == id) else {
            stays.push(StayView {
                option_id: id.0.clone(),
                name: id.0.clone(),
            });
            continue;
        };
        match matching_option(from, earlier, new) {
            Some(to) => swaps.push(SwapView {
                from_option_id: from.id.0.clone(),
                to_option_id: to.id.0.clone(),
                name: to.name.clone(),
                from_version: version_of(from),
                to_version: version_of(to),
            }),
            None => stays.push(StayView {
                option_id: from.id.0.clone(),
                name: from.name.clone(),
            }),
        }
    }
    (swaps, stays)
}

/// The options installed now, as the journal holds them. A journal that
/// cannot be read knows none, and then no earlier build is offered.
fn installed_now(state: &AppState) -> Option<Vec<OptionId>> {
    state
        .plan(|_, journal| {
            journal
                .installs
                .iter()
                .map(|install| install.option_id.clone())
                .collect::<Vec<_>>()
        })
        .ok()
}

/// The card's earlier build for `artifact`, read now, or `None`.
pub fn plan_earlier(
    state: &AppState,
    record: &BroughtRecord,
    new: &Package,
    artifact: &str,
) -> Option<EarlierView> {
    let installed = installed_now(state)?;
    let shelf = state.shelf();
    let (brought, earlier) = earlier_build(&record.all(), &shelf, &mod_names(new), artifact)?;
    let (swaps, stays) = swaps_of(earlier, new, &installed);
    Some(EarlierView {
        file_name: brought.file_name,
        package_id: earlier.id.0.clone(),
        puts_away: stays.is_empty(),
        swaps,
        stays,
    })
}

/// What a press did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResult {
    pub outcome: BringOutcome,
    /// The earlier build it replaced, by file name, when there was one.
    pub earlier: Option<String>,
    pub swapped: Vec<SwapView>,
    pub stays: Vec<StayView>,
    /// The stone's sentence when a swap was refused: the new build is in the
    /// mods folder, and nothing from that swap on was changed.
    pub failed: Option<String>,
    pub put_away: Vec<PutAwayView>,
    pub put_away_failed: Option<String>,
}

fn sentence(err: &RiftError) -> String {
    deploy::refusal(err).unwrap_or_else(|| err.to_string())
}

/// Scan now, or wait for the scan that runs to finish once more.
fn scan_now(state: &Arc<AppState>) {
    if !state.scan_blocking() {
        state.request_scan();
        state.wait_until_scanned();
    }
}

/// Record a build that landed, with the names inside it.
pub fn remember(record: &BroughtRecord, artifact: &str, sha256: &str, package: &Package) {
    let brought = Brought {
        file_name: artifact.to_string(),
        sha256: sha256.to_string(),
        bytes: package.size,
        names: mod_names(package).into_iter().collect(),
        at: crate::pawn::mailbox::now(),
    };
    if let Err(err) = record.record(brought) {
        log::warn!("the stone could not record the build it brought through: {err}");
    }
}

/// The press: `artifact` by the digest the player was shown, replacing the
/// earlier build the player was shown (`earlier_shown`, a package id, or
/// none). Refuses `bring` `earlierChanged` when the stone would now replace
/// another (or none, or one where none was shown).
pub fn apply(
    state: &Arc<AppState>,
    record: &BroughtRecord,
    read: &ReadBuilds,
    workshop: &Path,
    artifact: &str,
    sha256: &str,
    earlier_shown: Option<&str>,
) -> Result<UpdateResult, RiftError> {
    let changed = |now: &str| {
        RiftError::bring(
            BringReason::BytesChanged,
            format!(
                "{artifact} changed since it was shown (shown {}, now {}). Open it again to see the new build.",
                &sha256[..sha256.len().min(16)],
                &now[..now.len().min(16)]
            ),
        )
    };
    let (read_sha, new) = read_build(state, read, workshop, artifact)?;
    if read_sha != sha256 {
        return Err(changed(&read_sha));
    }
    let earlier_now = {
        let shelf = state.shelf();
        earlier_build(&record.all(), &shelf, &mod_names(&new), artifact)
            .map(|(_, package)| package.id.clone())
    };
    if earlier_now.as_ref().map(|id| id.0.as_str()) != earlier_shown {
        return Err(RiftError::bring(
            BringReason::EarlierChanged,
            "The earlier build changed since this was shown: the stone would now replace another. Open it again.",
        ));
    }
    let outcome = bring::bring_through(
        workshop,
        state.library_dir(),
        artifact,
        sha256,
        &new_run_id(),
    )?;
    remember(record, artifact, sha256, &new);
    state.events.changed(WORKSHOP_CHANGED);
    let mut result = UpdateResult {
        outcome,
        earlier: None,
        swapped: Vec::new(),
        stays: Vec::new(),
        failed: None,
        put_away: Vec::new(),
        put_away_failed: None,
    };
    let Some(earlier_id) = earlier_now else {
        state.request_scan();
        return Ok(result);
    };
    scan_now(state);
    swap(state, artifact, &earlier_id, &mut result);
    Ok(result)
}

/// The swaps and the put-away, once the new build is on the shelf.
fn swap(state: &Arc<AppState>, artifact: &str, earlier_id: &PackageId, result: &mut UpdateResult) {
    let shelf = state.shelf();
    let Some(new) = shelf
        .packages()
        .iter()
        .find(|p| p.format != PackageFormat::Folder && p.file_name.eq_ignore_ascii_case(artifact))
    else {
        result.failed = Some(format!(
            "{artifact} is in your mods folder, but the stone could not read it there, so nothing was swapped."
        ));
        return;
    };
    let Some(earlier) = shelf.package(earlier_id) else {
        result.failed = Some(
            "The earlier build is no longer in your mods folder, so nothing was swapped.".into(),
        );
        return;
    };
    result.earlier = Some(earlier.file_name.clone());
    let Some(installed) = installed_now(state) else {
        result.failed =
            Some("The stone could not read your game's installs, so nothing was swapped.".into());
        return;
    };
    let (swaps, stays) = swaps_of(earlier, new, &installed);
    result.stays = stays;
    let last = swaps.len().saturating_sub(1);
    for (i, swap) in swaps.into_iter().enumerate() {
        let put_away = if i == last {
            vec![earlier.id.clone()]
        } else {
            Vec::new()
        };
        match deploy::install_in_place(
            state,
            &swap.to_option_id,
            &Actor::Player,
            None,
            &[OptionId(swap.from_option_id.clone())],
            &put_away,
        ) {
            Ok(view) => {
                result.put_away.extend(view.put_away);
                result.put_away_failed = view.put_away_failed;
                result.swapped.push(swap);
            }
            Err(err) => {
                result.failed = Some(sentence(&err));
                return;
            }
        }
    }
    if result.swapped.is_empty() && result.stays.is_empty() {
        // Nothing of it was installed: the earlier zip goes away alone.
        let earlier_id = earlier.id.clone();
        match state.deploy(|_, journal| {
            Ok(versions::put_away_older(
                state,
                &shelf,
                &[earlier_id],
                |id| versions::has_install(journal, id),
            ))
        }) {
            Ok(put) => {
                result.put_away = put.moved;
                result.put_away_failed = put.failed;
            }
            Err(err) => result.put_away_failed = Some(sentence(&err)),
        }
    }
    if !result.put_away.is_empty() {
        state.request_scan();
    }
}

fn options(n: usize) -> String {
    if n == 1 {
        "1 option".into()
    } else {
        format!("{n} options")
    }
}

/// What Tamsin is told a press did, plainly (her tool answers carry no
/// persona): the facts, in the order they happened.
pub fn told(artifact: &str, done: &UpdateResult) -> String {
    let Some(earlier) = &done.earlier else {
        return match done.outcome {
            BringOutcome::Landed => format!(
                "added {artifact} to the mods folder; no earlier build of it was there, so nothing was installed. Plan and install it."
            ),
            BringOutcome::AlreadyHad => {
                format!("{artifact} was already in the mods folder; nothing changed.")
            }
        };
    };
    let mut said = vec![format!("added {artifact} in place of {earlier}")];
    if !done.swapped.is_empty() {
        said.push(format!("{} swapped", options(done.swapped.len())));
    }
    if let Some(refusal) = &done.failed {
        said.push(format!("then a swap was refused: {refusal}"));
    }
    if !done.stays.is_empty() {
        said.push(format!(
            "{} of {earlier} stay installed: this build has no option that is the same",
            options(done.stays.len())
        ));
    }
    if !done.put_away.is_empty() {
        said.push(format!(
            "{earlier} was put away in the older-versions folder"
        ));
    } else if let Some(failed) = &done.put_away_failed {
        said.push(format!("{earlier} could not be put away: {failed}"));
    }
    format!("{}. It loads at the game's next start.", said.join("; "))
}
