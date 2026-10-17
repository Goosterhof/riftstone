//! The deploy engine: plan, install, uninstall, reorder, re-key, recover.
//!
//! ## How an install moves bytes
//!
//! 1. Plan: every payload file gets its target (paks a `zrift-NNNN-` name in
//!    `pak_mods/`), passes the [`guard`], takes the spelling already on disk,
//!    and learns what sits there now: nothing; another install's file (only
//!    while the bytes there are still that install's, by size and
//!    fingerprint); a file no install owns. Its temporary name must be free.
//! 2. Write-ahead: the journal is saved with an [`Intent`] whose steps name
//!    every path, every backup slot and every folder the install creates.
//! 3. The package extracts every file to `<target>.riftstone-part` in one
//!    pass (solid archives decompress once), each part created new and
//!    reported by name once written. Nothing else has moved yet.
//! 4. Each part is size-checked and fingerprinted, and the fingerprints are
//!    saved into the intent before the first rename.
//! 5. File by file: what sits at the target moves into
//!    `<state_dir>/backups/<seq>/<game path>`, then the part is renamed into
//!    place.
//! 6. Commit: the install is recorded, `pending` cleared, the journal saved.
//!
//! Any failure undoes the intent from what is on disk, every step even when
//! one of them fails, and never deletes what it cannot prove it wrote: a part
//! is the install's own (its name was free, and the extractor never writes
//! over anything) — but when the extraction itself stopped, only the parts it
//! reported written are, and a name something else took meanwhile stays
//! with whoever took it; a file at a target only when no part is left for it
//! and its size and fingerprint match the ones saved in step 4. A backup
//! that cannot go back because something else now sits at its path stays in
//! the store, kept aside on record ([`ParkedBackup`]). A crash leaves
//! `pending` on disk; [`recover`] does the same undo at the next launch
//! (after a crash, which parts the extraction wrote is not known: a part at
//! a name the install checked free counts as its own).
//!
//! Reinstalling an installed option is ONE intent: the old copy's uninstall
//! steps first, then the new copy's placements. A failure anywhere undoes
//! both halves, so the option stays installed as it was.
//!
//! ## How an uninstall moves bytes
//!
//! Files to delete are renamed to `<path>.riftstone-trash-<seq>` and backups
//! move back into place, under an intent; only after the commit are the
//! trash files (and superseded backups) deleted, through the journal's
//! `cleanup` list. Until the commit everything is reversible. A backup whose
//! path something else has changed since stays in the store, kept aside on
//! record; a superseded backup is deleted only when it really holds the bytes
//! the uninstalled layer recorded.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rift_core::dd2::{self, PAK_MODS_DIR};
use rift_core::error::RiftResult;
use rift_core::path::{file_name, parent};
use rift_core::{
    lower_key, normalize_entry_path, DeployTarget, ExtractItem, ExtractProgress, ModOption,
    OptionId, Package, PackageSource, PayloadFile, RiftError,
};

use crate::fsops::{
    self, join_rel, with_suffix, CaseResolver, PART_SUFFIX, REORDER_MARK, TRASH_MARK,
};
use crate::game::GameInstall;
use crate::guard;
use crate::journal::{
    Below, BelowOwner, DeployKind, DeployedFile, InstalledOption, Intent, IntentAction, IntentStep,
    Journal, Leftover, Origin, ParkedBackup,
};
use crate::paks;
use crate::plan::{
    InstallPlan, InstallReport, OwnerKind, ParkedRestoreReport, PlanWarning, PlannedRemoval,
    PlannedWrite, RecoveryReport, RemovalAction, UninstallPlan, UninstallReport, WriteAction,
};
use crate::reframework::RefStatus;
use crate::stack::{LayerRef, PathIndex};

/// Everything an operation needs to know about the world.
#[derive(Debug, Clone, Copy)]
pub struct DeployContext<'a> {
    pub game: &'a GameInstall,
    /// The app data dir: the journal and `backups/` live here.
    pub state_dir: &'a Path,
    pub ref_status: &'a RefStatus,
    /// Is DD2.exe running? ([`is_game_running`])
    pub game_running: bool,
}

/// Is Dragon's Dogma 2 running? (`DD2.exe` in the Windows process list;
/// always `false` elsewhere.)
pub fn is_game_running() -> bool {
    #[cfg(windows)]
    {
        process_list::is_running(crate::game::GAME_EXE)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

// ---------------------------------------------------------------------------
// Install
// ---------------------------------------------------------------------------

/// One payload file, fully planned.
#[derive(Debug, Clone)]
struct PlannedFile {
    entry: String,
    /// Relative to the game root, in the spelling on disk.
    game_path: String,
    kind: DeployKind,
    size: u64,
    below: Below,
    /// Store path (relative to the state dir) the existing file moves to.
    backup: Option<String>,
    /// A top layer whose file at this path is gone: its claim retires and
    /// this install inherits what was below it.
    retire: Option<LayerRef>,
}

impl PlannedFile {
    /// The write-ahead step for this file (its fingerprint follows once the
    /// part is extracted).
    fn step(&self) -> IntentStep {
        IntentStep::Place {
            game_path: self.game_path.clone(),
            size: self.size,
            backup: self.backup.clone(),
            owner: match (&self.backup, &self.below) {
                (Some(_), Below::Backup { owner, .. }) => Some(owner.clone()),
                _ => None,
            },
            fingerprint: None,
            unextracted: false,
        }
    }
}

/// An install, planned: what the window sees, the files, and the folders it
/// creates.
struct PreparedInstall {
    plan: InstallPlan,
    files: Vec<PlannedFile>,
    dirs: Vec<String>,
}

/// What installing `option` would do. Pure: nothing is written.
pub fn plan_install(
    ctx: &DeployContext,
    journal: &Journal,
    package: &Package,
    option: &ModOption,
) -> InstallPlan {
    prepare_install(ctx, journal, package, option, journal.next_seq).plan
}

/// Install `option` from `source` (the opened `package`). Installing an
/// option that is already installed reinstalls it — the old copy is
/// uninstalled and the option comes back as the newest layer — as one intent:
/// if anything fails, the old copy is put back as it was.
pub fn install(
    ctx: &DeployContext,
    journal: &mut Journal,
    package: &Package,
    option: &ModOption,
    source: &mut dyn PackageSource,
    progress: &mut dyn FnMut(ExtractProgress),
) -> Result<InstallReport, RiftError> {
    let started = Instant::now();
    let preview = plan_install(ctx, journal, package, option);
    if let Some(reason) = preview.refused {
        return Err(RiftError::refused(reason));
    }
    let root = &ctx.game.root;
    let seq = allocate_seq(journal, ctx.state_dir);
    let before = journal.clone();
    // The journal as the commit will write it: for a reinstall, without the
    // old copy, whose uninstall is the first half of this intent.
    let mut post = journal.clone();
    post.next_seq = seq + 1;
    let mut removals: Vec<IntentStep> = Vec::new();
    let reinstall = journal.position(&option.id);
    if let Some(position) = reinstall {
        let (plan, work) = prepare_uninstall(ctx, journal, &option.id);
        if let Some(reason) = plan.refused {
            return Err(RiftError::refused(reason));
        }
        removals = removal_steps(&work, seq);
        temporaries_free(root, &removals)?;
        apply_uninstall(&mut post, position, &work, &removals);
        let installed = &journal.installs[position];
        journal.pending = Some(Intent {
            action: IntentAction::Install,
            seq,
            option_id: option.id.clone(),
            touched: installed
                .files
                .iter()
                .map(|f| f.game_path.clone())
                .collect(),
            started_at: now(),
            option_name: option.name.clone(),
            steps: removals.clone(),
            phase: 0,
            dirs: Vec::new(),
        });
        journal.next_seq = seq + 1;
        if let Err(err) = journal.save(ctx.state_dir) {
            *journal = before;
            return Err(err);
        }
        if let Err(err) = execute_removals(ctx, &removals) {
            return Err(fail_and_roll_back(ctx, journal, &before, err));
        }
    }

    let PreparedInstall { plan, files, dirs } = prepare_install(ctx, &post, package, option, seq);
    if let Some(reason) = plan.refused {
        let err = RiftError::refused(reason);
        return Err(match reinstall {
            Some(_) => fail_and_roll_back(ctx, journal, &before, err),
            None => err,
        });
    }
    let intent = journal.pending.get_or_insert_with(|| Intent {
        action: IntentAction::Install,
        seq,
        option_id: option.id.clone(),
        touched: Vec::new(),
        started_at: now(),
        option_name: option.name.clone(),
        steps: Vec::new(),
        phase: 0,
        dirs: Vec::new(),
    });
    intent
        .touched
        .extend(files.iter().map(|f| f.game_path.clone()));
    intent.steps.extend(files.iter().map(PlannedFile::step));
    intent.dirs.clone_from(&dirs);
    journal.next_seq = seq + 1;
    if let Err(err) = journal.save(ctx.state_dir) {
        if reinstall.is_some() {
            return Err(fail_and_roll_back(ctx, journal, &before, err));
        }
        *journal = before;
        return Err(err);
    }

    let mut extracted = vec![false; files.len()];
    let fingerprints = match extract_parts(ctx, package, &files, source, progress, &mut extracted) {
        Ok(fingerprints) => fingerprints,
        Err(err) => {
            note_unextracted(ctx, journal, &extracted);
            return Err(fail_and_roll_back(ctx, journal, &before, err));
        }
    };
    // From here on a file at a target with its size and fingerprint is this
    // install's own: say so before the first rename.
    if let Some(intent) = journal.pending.as_mut() {
        let places = intent.steps.iter_mut().filter_map(|step| match step {
            IntentStep::Place { fingerprint, .. } => Some(fingerprint),
            _ => None,
        });
        for (recorded, fingerprint) in places.zip(&fingerprints) {
            *recorded = Some(fingerprint.clone());
        }
    }
    if let Err(err) = journal.save(ctx.state_dir) {
        return Err(fail_and_roll_back(ctx, journal, &before, err));
    }
    if let Err(err) = place_parts(ctx, &files) {
        return Err(fail_and_roll_back(ctx, journal, &before, err));
    }
    let record = match build_record(ctx, package, option, seq, &files, &fingerprints) {
        Ok(record) => record,
        Err(err) => return Err(fail_and_roll_back(ctx, journal, &before, err)),
    };

    let mut committed = post;
    for file in &files {
        if let Some(retired) = file.retire {
            let key = lower_key(&file.game_path);
            committed.installs[retired.install]
                .files
                .retain(|f| lower_key(&f.game_path) != key);
        }
    }
    let has_paks = record.has_paks();
    committed.installs.push(record);
    if has_paks {
        committed.pak_order.retain(|id| id != &option.id);
        committed.pak_order.push(option.id.clone());
    }
    note_created_dirs(&mut committed.created_dirs, root, &dirs);
    committed.pending = None;
    if let Err(err) = committed.save(ctx.state_dir) {
        return Err(fail_and_roll_back(ctx, journal, &before, err));
    }
    *journal = committed;
    process_cleanup(ctx, journal);
    prune_restored_slots(ctx, &removals);

    Ok(InstallReport {
        option_id: option.id.clone(),
        files_written: files.len(),
        bytes_written: files.iter().map(|f| f.size).sum(),
        backups: files.iter().filter(|f| f.backup.is_some()).count(),
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        restart_needed: ctx.game_running,
    })
}

/// Plan an install against `journal`. When `journal` still holds the option
/// (the preview of a reinstall), its own files and pak names do not count as
/// someone else's; the real reinstall plans against the journal without it.
fn prepare_install(
    ctx: &DeployContext,
    journal: &Journal,
    package: &Package,
    option: &ModOption,
    seq: u64,
) -> PreparedInstall {
    let plan = InstallPlan {
        option_id: option.id.clone(),
        name: option.name.clone(),
        writes: Vec::new(),
        total_bytes: 0,
        warnings: Vec::new(),
        refused: None,
    };
    let refuse = |mut plan: InstallPlan, reason: String| {
        plan.writes.clear();
        plan.refused = Some(reason);
        PreparedInstall {
            plan,
            files: Vec::new(),
            dirs: Vec::new(),
        }
    };
    if let Some(reason) = common_refusal(ctx, journal) {
        return refuse(plan, reason);
    }
    if option.package_id != package.id {
        return refuse(
            plan,
            format!(
                "\"{}\" does not come from the package {}; rescan the library and try again.",
                option.name, package.file_name
            ),
        );
    }
    if option.payload.is_empty() {
        return refuse(
            plan,
            format!(
                "\"{}\" is a menu entry with nothing to install. Pick one of the options under it.",
                option.name
            ),
        );
    }
    let reinstall = journal.find(&option.id);
    if let Some(installed) = reinstall {
        if ctx.game_running && installed.has_paks() {
            return refuse(
                plan,
                format!(
                    "Dragon's Dogma 2 is running and holds \"{}\"'s paks open, so it cannot be reinstalled now. Close the game, then try again.",
                    installed.name
                ),
            );
        }
    }

    let mut plan = plan;
    let root = &ctx.game.root;
    let index = PathIndex::build(journal);
    let mut resolver = CaseResolver::new(root);
    let payload = dedupe_payload(&option.payload);

    let has_paks = payload
        .iter()
        .any(|p| matches!(p.target, DeployTarget::Pak { .. }));
    let pak_number = if has_paks {
        next_pak_number(&mut resolver, root, journal, reinstall)
    } else {
        0
    };
    if pak_number > paks::MAX_PAK_POSITION {
        return refuse(
            plan,
            "The Riftstone has run out of pak numbers (zrift-9999 is taken). Reorder the paks once to renumber them from 0000 — and move any zrift- pak the Riftstone does not manage out of pak_mods — then install again.".to_string(),
        );
    }
    let pak_dir = resolver.resolve(PAK_MODS_DIR);
    let mut used_stems = HashSet::new();
    let mut files = Vec::with_capacity(payload.len());

    for item in payload {
        let kind = DeployKind::from(&item.target);
        let wanted = match &item.target {
            DeployTarget::Pak { file_name } => {
                let stem = paks::unique_stem(paks::pak_stem(file_name), &mut used_stems);
                format!("{pak_dir}/{}", paks::managed_pak_name(pak_number, &stem))
            }
            DeployTarget::Loose { game_path }
            | DeployTarget::Reframework { game_path }
            | DeployTarget::Root { game_path } => game_path.clone(),
        };
        let normalized = match guard::check_game_path(&wanted)
            .and_then(|n| guard::check_kind(kind, &n).map(|()| n))
        {
            Ok(normalized) => normalized,
            Err(err) => return refuse(plan, with_entry(err, &item.entry)),
        };
        let game_path = resolver.resolve(&normalized);
        let target = match guard::guard_target(root, &game_path) {
            Ok(target) => target,
            Err(err) => return refuse(plan, with_entry(err, &item.entry)),
        };
        if fsops::exists(&with_suffix(&target, PART_SUFFIX)) {
            return refuse(plan, part_taken(&game_path));
        }
        let key = lower_key(&game_path);
        let present = fsops::is_file(&target);
        let store = format!("backups/{seq}/{game_path}");

        let (mut action, below, backup, retire) = match index.top(journal, &key) {
            Some(top) if top.install(journal).option_id == option.id => {
                (WriteAction::Reinstall, Below::Nothing, None, None)
            }
            Some(top) if present && still_ours(&target, top.file(journal)) => {
                let owner = top.install(journal);
                (
                    WriteAction::OverwriteRiftstone {
                        option_name: owner.name.clone(),
                        option_id: owner.option_id.clone(),
                    },
                    Below::Backup {
                        store: store.clone(),
                        owner: BelowOwner::Riftstone {
                            option_id: owner.option_id.clone(),
                        },
                    },
                    Some(store),
                    None,
                )
            }
            // Something replaced the top layer's file since it was installed:
            // a break in the stack. The bytes there belong to whoever wrote
            // them — never to that layer, whose uninstall would otherwise
            // hand them down into the bin.
            Some(_) if present => unowned_below(store),
            Some(top) => (
                WriteAction::Create,
                top.file(journal).below.clone(),
                None,
                Some(top),
            ),
            None if present => unowned_below(store),
            None => (WriteAction::Create, Below::Nothing, None, None),
        };
        if reinstall.is_some() && kind == DeployKind::Pak {
            // The option's own pak goes and comes back under this name.
            action = WriteAction::Reinstall;
        }
        plan.writes.push(PlannedWrite {
            entry: item.entry.clone(),
            game_path: game_path.clone(),
            kind,
            size: item.size,
            action,
        });
        files.push(PlannedFile {
            entry: item.entry.clone(),
            game_path,
            kind,
            size: item.size,
            below,
            backup,
            retire,
        });
    }

    // One path cannot be a file and the folder of another file at once.
    let file_keys: HashSet<String> = files.iter().map(|f| lower_key(&f.game_path)).collect();
    for file in &files {
        let mut dir = parent(&file.game_path);
        while !dir.is_empty() {
            if file_keys.contains(&lower_key(dir)) {
                return refuse(
                    plan,
                    format!(
                        "\"{dir}\" is both a file and a folder in \"{}\" ({} sits inside it). The archive is broken, so the Riftstone refused it.",
                        option.name, file.game_path
                    ),
                );
            }
            dir = parent(dir);
        }
    }

    let mut dirs = Vec::new();
    let mut seen_dirs = HashSet::new();
    for file in &files {
        for dir in fsops::missing_dirs(root, &file.game_path) {
            if seen_dirs.insert(lower_key(&dir)) {
                dirs.push(dir);
            }
        }
    }
    plan.total_bytes = files.iter().map(|f| f.size).sum();
    plan.warnings = install_warnings(ctx, journal, option, &plan.writes);
    PreparedInstall { plan, files, dirs }
}

/// A file no install owns sits at a target: it is backed up as "the file
/// that was there before", and comes back when this install leaves.
fn unowned_below(store: String) -> (WriteAction, Below, Option<String>, Option<LayerRef>) {
    (
        WriteAction::OverwriteUnowned,
        Below::Backup {
            store: store.clone(),
            owner: BelowOwner::Unowned,
        },
        Some(store),
        None,
    )
}

/// The refusal for a temporary name that is already taken.
fn part_taken(game_path: &str) -> String {
    format!(
        "\"{game_path}{PART_SUFFIX}\" already sits in the game folder. The Riftstone writes \"{game_path}\" under that temporary name first, and it never writes through or over something it did not put there. Move it out of the game folder, then install again."
    )
}

/// The payload with later duplicates of one game path (any case) replacing
/// earlier ones — the way an archive extracted over itself would end up.
/// Paks are never duplicates: two of one name both install.
fn dedupe_payload(payload: &[PayloadFile]) -> Vec<&PayloadFile> {
    let mut out: Vec<&PayloadFile> = Vec::with_capacity(payload.len());
    let mut slot: HashMap<String, usize> = HashMap::new();
    for item in payload {
        match item.target.game_path() {
            Some(path) => {
                let key = lower_key(path);
                if let Some(&at) = slot.get(&key) {
                    log::warn!(
                        "{} and {} both target {path}; the later one wins",
                        out[at].entry,
                        item.entry
                    );
                    out[at] = item;
                } else {
                    slot.insert(key, out.len());
                    out.push(item);
                }
            }
            None => out.push(item),
        }
    }
    out
}

/// The next free pak number: above every `zrift-NNNN-` pak in `pak_mods/` or
/// the journal, and at least the pak order's length, so a new pak always
/// sorts after (and wins over) everything already there — strays included.
/// The engine's own temporaries (a pak mid-uninstall, mid-reorder) hold no
/// place, and neither does `exclude`, the option a preview reinstalls.
fn next_pak_number(
    resolver: &mut CaseResolver,
    root: &Path,
    journal: &Journal,
    exclude: Option<&InstalledOption>,
) -> u32 {
    let excluded_id = exclude.map(|i| &i.option_id);
    let excluded_names: HashSet<String> = exclude
        .map(|i| {
            i.pak_files()
                .map(|f| lower_key(file_name(&f.game_path)))
                .collect()
        })
        .unwrap_or_default();
    let mut next = u32::try_from(
        journal
            .pak_order
            .iter()
            .filter(|id| Some(*id) != excluded_id)
            .count(),
    )
    .unwrap_or(u32::MAX);
    let pak_dir = join_rel(root, &resolver.resolve(PAK_MODS_DIR));
    if let Ok(entries) = fs::read_dir(&pak_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let lower = lower_key(&name);
            if !lower.ends_with(paks::PAK_EXTENSION)
                || lower.contains(guard::RESERVED_MARKER)
                || excluded_names.contains(&lower)
            {
                continue;
            }
            if let Some(n) = paks::managed_number(&name) {
                next = next.max(n.saturating_add(1));
            }
        }
    }
    for install in &journal.installs {
        if Some(&install.option_id) == excluded_id {
            continue;
        }
        for file in install.pak_files() {
            if let Some(n) = paks::managed_number(file_name(&file.game_path)) {
                next = next.max(n.saturating_add(1));
            }
        }
    }
    next
}

fn install_warnings(
    ctx: &DeployContext,
    journal: &Journal,
    option: &ModOption,
    writes: &[PlannedWrite],
) -> Vec<PlanWarning> {
    let has = |kind: DeployKind| writes.iter().any(|w| w.kind == kind);
    let (has_paks, has_loose, has_scripts) = (
        has(DeployKind::Pak),
        has(DeployKind::Loose),
        has(DeployKind::Reframework),
    );
    let ships_reframework = writes.iter().any(|w| {
        w.kind == DeployKind::Root && lower_key(&w.game_path) == crate::reframework::REFRAMEWORK_DLL
    });
    let status = ctx.ref_status;
    let mut warnings = Vec::new();
    if ctx.game_running {
        warnings.push(PlanWarning::GameRunning);
    }
    if (has_paks || has_loose || has_scripts) && !status.installed && !ships_reframework {
        warnings.push(PlanWarning::ReframeworkMissing);
    }
    if has_paks && status.pak_directory == Some(false) {
        warnings.push(PlanWarning::PakFolderOff);
    }
    if has_loose && status.loose_file_loader != Some(true) {
        warnings.push(PlanWarning::LooseLoaderOff);
    }
    if let Some(info) = &option.info {
        let mut seen = HashSet::new();
        for name in &info.requires {
            let name = name.trim();
            if name.is_empty() || !seen.insert(name_key(name)) {
                continue;
            }
            if !requirement_installed(ctx.ref_status, journal, &option.id, name) {
                warnings.push(PlanWarning::RequiresMissing {
                    name: name.to_string(),
                });
            }
        }
    }
    let mut names: Vec<String> = Vec::new();
    let mut paths = 0usize;
    for write in writes {
        let WriteAction::OverwriteRiftstone { option_name, .. } = &write.action else {
            continue;
        };
        let name = option_name.clone();
        paths += 1;
        if !names.contains(&name) {
            names.push(name);
        }
    }
    if paths > 0 {
        warnings.push(PlanWarning::Conflicts {
            option_names: names,
            paths,
        });
    }
    let outdated = writes
        .iter()
        .filter(|w| w.kind == DeployKind::Loose && dd2::outdated_suffix(&w.game_path).is_some())
        .count();
    if outdated > 0 {
        warnings.push(PlanWarning::OutdatedFormats { count: outdated });
    }
    warnings
}

/// A name for comparing `requires=` values: lower-case letters and digits
/// only, so `RE Framework` and `REFramework` are one name.
fn name_key(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Is a `requires=` name met: REFramework itself, or any other install in
/// the journal (an adopted one too) by its name — or, for an adopted
/// install, by the label it was taken over under?
fn requirement_installed(
    ref_status: &RefStatus,
    journal: &Journal,
    self_id: &OptionId,
    name: &str,
) -> bool {
    let wanted = name_key(name);
    if wanted == "reframework" && ref_status.installed {
        return true;
    }
    journal.installs.iter().any(|i| {
        &i.option_id != self_id
            && (name_key(&i.name) == wanted
                || matches!(&i.origin, Origin::Adopted { label } if name_key(label) == wanted))
    })
}

/// Steps 3 and 4 of an install: one extraction pass into the temporary
/// names, then every part size-checked and fingerprinted. Nothing in the game
/// folder has moved yet. `extracted[n]` turns true as the extractor reports
/// file `n` written: when this fails, those are the only parts this install
/// made ([`note_unextracted`]).
fn extract_parts(
    ctx: &DeployContext,
    package: &Package,
    files: &[PlannedFile],
    source: &mut dyn PackageSource,
    progress: &mut dyn FnMut(ExtractProgress),
    extracted: &mut [bool],
) -> Result<Vec<String>, RiftError> {
    let root = &ctx.game.root;
    let parts: Vec<PathBuf> = files
        .iter()
        .map(|f| with_suffix(&join_rel(root, &f.game_path), PART_SUFFIX))
        .collect();
    // The extractor creates each part and its missing folders: the folders
    // must still pass the guard, and the names must still be free (the
    // extractor refuses a taken one too — this says so in the engine's words).
    for (file, part) in files.iter().zip(&parts) {
        guard::guard_target(root, &file.game_path)?;
        if fsops::exists(part) {
            return Err(RiftError::refused(part_taken(&file.game_path)));
        }
    }
    let items: Vec<ExtractItem> = files
        .iter()
        .zip(&parts)
        .map(|(f, part)| ExtractItem {
            entry: f.entry.clone(),
            dest: part.clone(),
        })
        .collect();
    source.extract(&items, &mut |report: ExtractProgress| {
        if let Some(written) = extracted.get_mut(report.item) {
            *written = true;
        }
        progress(report);
    })?;

    let mut fingerprints = Vec::with_capacity(files.len());
    for (file, part) in files.iter().zip(&parts) {
        let written = fsops::file_len(part).ok_or_else(|| {
            RiftError::corrupt(
                &package.path,
                format!("{} did not come out of the archive", file.entry),
            )
        })?;
        if written != file.size {
            return Err(RiftError::corrupt(
                &package.path,
                format!(
                    "{} unpacked to {written} bytes instead of the {} the archive lists",
                    file.entry, file.size
                ),
            ));
        }
        let fingerprint = fsops::fingerprint(part).map_err(|err| {
            RiftError::io(
                format!("could not read back {} after unpacking it", file.game_path),
                err,
            )
        })?;
        fingerprints.push(fingerprint);
    }
    Ok(fingerprints)
}

/// An extraction stopped: mark every `Place` step whose part the extractor
/// never reported written, so the rollback leaves whatever sits at that name
/// alone — something else took it after the check (the extractor refused to
/// write over it), or nothing is there. The marks are saved too, so a crash
/// during the rollback cannot turn the next launch's recovery against them.
/// Places are the intent's only `Place` steps, in `files` order.
fn note_unextracted(ctx: &DeployContext, journal: &mut Journal, extracted: &[bool]) {
    let Some(intent) = journal.pending.as_mut() else {
        return;
    };
    let places = intent.steps.iter_mut().filter_map(|step| match step {
        IntentStep::Place { unextracted, .. } => Some(unextracted),
        _ => None,
    });
    let mut marked = false;
    for (unextracted, &written) in places.zip(extracted) {
        if !written {
            *unextracted = true;
            marked = true;
        }
    }
    if marked {
        if let Err(err) = journal.save(ctx.state_dir) {
            log::warn!(
                "could not record which parts the stopped extraction wrote ({err}); this rollback still leaves the others alone"
            );
        }
    }
}

/// Step 5 of an install: file by file, what sits at the target moves into
/// the backup store, then the part is renamed into place.
fn place_parts(ctx: &DeployContext, files: &[PlannedFile]) -> Result<(), RiftError> {
    let root = &ctx.game.root;
    for file in files {
        let target = guard::guard_target(root, &file.game_path)?;
        match &file.backup {
            Some(store) => {
                fsops::move_file(&target, &join_rel(ctx.state_dir, store)).map_err(|err| {
                    RiftError::io(
                        format!(
                            "could not move {} aside into the backup store",
                            file.game_path
                        ),
                        err,
                    )
                })?;
            }
            None if fsops::exists(&target) => {
                return Err(RiftError::refused(format!(
                    "\"{}\" appeared in the game folder while the Riftstone was installing. It was left alone and the install was rolled back; try again.",
                    file.game_path
                )));
            }
            None => {}
        }
        fsops::rename(&with_suffix(&target, PART_SUFFIX), &target).map_err(|err| {
            RiftError::io(format!("could not move {} into place", file.game_path), err)
        })?;
    }
    Ok(())
}

fn build_record(
    ctx: &DeployContext,
    package: &Package,
    option: &ModOption,
    seq: u64,
    files: &[PlannedFile],
    fingerprints: &[String],
) -> Result<InstalledOption, RiftError> {
    let mut deployed = Vec::with_capacity(files.len());
    for (file, fingerprint) in files.iter().zip(fingerprints) {
        let path = join_rel(&ctx.game.root, &file.game_path);
        let size = fsops::file_len(&path).ok_or_else(|| {
            RiftError::io(
                format!("{} vanished right after it was installed", file.game_path),
                "not found",
            )
        })?;
        deployed.push(DeployedFile {
            game_path: file.game_path.clone(),
            kind: file.kind,
            size,
            entry: Some(file.entry.clone()),
            below: file.below.clone(),
            fingerprint: Some(fingerprint.clone()),
        });
    }
    Ok(InstalledOption {
        seq,
        option_id: option.id.clone(),
        name: option.name.clone(),
        package_file: package.file_name.clone(),
        package_path: package.path.clone(),
        package_size: package.size,
        option_root: option.root.clone(),
        version: option.info.as_ref().and_then(|i| i.version.clone()),
        installed_at: now(),
        game_build: ctx.game.build_id.clone(),
        origin: Origin::Riftstone,
        files: deployed,
    })
}

/// Remember the folders an operation created (those that exist now), so
/// they — and only they — can be pruned once empty.
fn note_created_dirs(created: &mut Vec<String>, root: &Path, dirs: &[String]) {
    for dir in dirs {
        let key = lower_key(dir);
        if fsops::is_real_dir(&join_rel(root, dir)) && !created.iter().any(|d| lower_key(d) == key)
        {
            created.push(dir.clone());
        }
    }
}

// ---------------------------------------------------------------------------
// Uninstall
// ---------------------------------------------------------------------------

/// What a backup slot above a layer holds, compared with that layer's record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SlotState {
    /// Exactly the bytes the layer recorded: superseded once it leaves.
    Ours,
    /// Nothing: the slot is gone from the store.
    Missing,
    /// Something else: never deleted.
    Foreign,
}

#[derive(Debug, Clone)]
enum RemovalWork {
    Delete {
        game_path: String,
    },
    Restore {
        game_path: String,
        backup: String,
        present: bool,
    },
    /// A layer above holds this install's bytes in its slot: it inherits
    /// what was below this install.
    HandDown {
        above: LayerRef,
        slot: SlotState,
    },
    /// A file kept aside holds this install's bytes: it inherits what was
    /// below this install.
    HandDownParked {
        parked: usize,
        slot: SlotState,
    },
    /// Something else changed the file since: it stays, and the backup below
    /// this install is kept aside on record.
    Park {
        backup: ParkedBackup,
    },
    Keep,
}

/// What uninstalling `option_id` would do. Pure: nothing is written.
pub fn plan_uninstall(
    ctx: &DeployContext,
    journal: &Journal,
    option_id: &OptionId,
) -> UninstallPlan {
    prepare_uninstall(ctx, journal, option_id).0
}

fn prepare_uninstall(
    ctx: &DeployContext,
    journal: &Journal,
    option_id: &OptionId,
) -> (UninstallPlan, Vec<(usize, RemovalWork)>) {
    let mut plan = UninstallPlan {
        option_id: option_id.clone(),
        name: String::new(),
        removals: Vec::new(),
        refused: None,
    };
    let refuse = |mut plan: UninstallPlan, reason: String| {
        plan.removals.clear();
        plan.refused = Some(reason);
        (plan, Vec::new())
    };
    if let Some(reason) = common_refusal(ctx, journal) {
        return refuse(plan, reason);
    }
    let Some(position) = journal.position(option_id) else {
        return refuse(
            plan,
            format!("\"{option_id}\" is not installed by the Riftstone, so there is nothing to uninstall."),
        );
    };
    let installed = &journal.installs[position];
    plan.name = installed.name.clone();
    if ctx.game_running && installed.has_paks() {
        return refuse(
            plan,
            format!(
                "Dragon's Dogma 2 is running and holds \"{}\"'s paks open. Close the game, then uninstall.",
                installed.name
            ),
        );
    }

    let root = &ctx.game.root;
    let index = PathIndex::build(journal);
    let mut resolver = CaseResolver::new(root);
    let mut work = Vec::with_capacity(installed.files.len());
    let mut seen = HashSet::new();
    for (file_index, file) in installed.files.iter().enumerate() {
        let key = lower_key(&file.game_path);
        if !seen.insert(key.clone()) {
            // An older journal may list one path twice for one install (an
            // adopted install recorded it in two spellings): the first entry
            // speaks for it.
            continue;
        }
        if let Some(above) = index.above(journal, &key, option_id) {
            let upper = above.install(journal);
            let slot = match &above.file(journal).below {
                Below::Backup { store, .. } => slot_state(ctx.state_dir, store, file),
                _ => SlotState::Missing,
            };
            plan.removals.push(PlannedRemoval {
                game_path: file.game_path.clone(),
                kind: file.kind,
                action: RemovalAction::HandDown {
                    to_option_name: upper.name.clone(),
                    to_option_id: upper.option_id.clone(),
                },
            });
            work.push((file_index, RemovalWork::HandDown { above, slot }));
            continue;
        }
        if let Some(parked) = parked_above(journal, &key, option_id) {
            // Its bytes sit in a kept-aside file, under something else's
            // change: the file in the game folder stays as it is.
            let slot = slot_state(ctx.state_dir, &journal.parked[parked].store, file);
            plan.removals.push(PlannedRemoval {
                game_path: file.game_path.clone(),
                kind: file.kind,
                action: RemovalAction::KeepChanged,
            });
            work.push((file_index, RemovalWork::HandDownParked { parked, slot }));
            continue;
        }
        let game_path = resolver.resolve(&file.game_path);
        if let Err(err) = guard::guard_target(root, &game_path) {
            return refuse(plan, reason_of(err));
        }
        let path = join_rel(root, &game_path);
        let present = fsops::is_file(&path);
        let unchanged = present && still_ours(&path, file);
        let (action, removal) = match &file.below {
            Below::Nothing if !present => (RemovalAction::AlreadyGone, RemovalWork::Keep),
            Below::Nothing if unchanged => {
                (RemovalAction::Delete, RemovalWork::Delete { game_path })
            }
            Below::Nothing => (RemovalAction::KeepChanged, RemovalWork::Keep),
            // Something else changed the file since: it stays, and what was
            // below — which cannot go back without overwriting that change —
            // is kept aside on record.
            Below::Backup { store, owner } if present && !unchanged => {
                let removal = if fsops::is_file(&join_rel(ctx.state_dir, store)) {
                    RemovalWork::Park {
                        backup: ParkedBackup {
                            store: store.clone(),
                            game_path: game_path.clone(),
                            owner: owner.clone(),
                            parked_at: now(),
                            reason: format!(
                                "Kept aside when \"{}\" was uninstalled: something else had changed the file at this path since it was installed, so the file that was there before could not go back without overwriting that change.",
                                installed.name
                            ),
                        },
                    }
                } else {
                    RemovalWork::Keep
                };
                (RemovalAction::KeepChanged, removal)
            }
            Below::Backup { store, owner } if fsops::is_file(&join_rel(ctx.state_dir, store)) => {
                let (label, kind) = owner_label(journal, owner);
                (
                    RemovalAction::RestoreBackup {
                        owner: label,
                        owner_kind: kind,
                    },
                    RemovalWork::Restore {
                        game_path,
                        backup: store.clone(),
                        present,
                    },
                )
            }
            // The backup itself is gone from the store.
            Below::Backup { .. } if !present => (RemovalAction::AlreadyGone, RemovalWork::Keep),
            Below::Backup { .. } if file.kind == DeployKind::Root => {
                // Never strip a game-root file whose original is lost.
                (RemovalAction::KeepVanilla, RemovalWork::Keep)
            }
            Below::Backup { .. } => (RemovalAction::Delete, RemovalWork::Delete { game_path }),
            Below::UnknownVanilla => (RemovalAction::KeepVanilla, RemovalWork::Keep),
        };
        plan.removals.push(PlannedRemoval {
            game_path: file.game_path.clone(),
            kind: file.kind,
            action,
        });
        work.push((file_index, removal));
    }
    (plan, work)
}

/// Is the file at `path` still the bytes `file` recorded?
fn still_ours(path: &Path, file: &DeployedFile) -> bool {
    fsops::file_len(path) == Some(file.size)
        && file
            .fingerprint
            .as_ref()
            .is_none_or(|fp| fsops::fingerprint(path).ok().as_ref() == Some(fp))
}

/// What the slot `store` holds compared with `file`'s record.
fn slot_state(state_dir: &Path, store: &str, file: &DeployedFile) -> SlotState {
    let slot = join_rel(state_dir, store);
    if !fsops::exists(&slot) {
        SlotState::Missing
    } else if still_ours(&slot, file) {
        SlotState::Ours
    } else {
        SlotState::Foreign
    }
}

/// The kept-aside file at this path that holds `option_id`'s bytes.
fn parked_above(journal: &Journal, key: &str, option_id: &OptionId) -> Option<usize> {
    journal.parked.iter().position(|kept| {
        lower_key(&kept.game_path) == key
            && matches!(&kept.owner, BelowOwner::Riftstone { option_id: under } if under == option_id)
    })
}

/// A backup owner as the window names it: "the file that was there
/// before", or the owning install's name (adopted, or the Riftstone's own).
pub(crate) fn owner_label(journal: &Journal, owner: &BelowOwner) -> (String, OwnerKind) {
    match owner {
        BelowOwner::Unowned => (
            "the file that was there before".to_string(),
            OwnerKind::Unowned,
        ),
        BelowOwner::Riftstone { option_id } => match journal.find(option_id) {
            Some(install) if install.is_adopted() => (install.name.clone(), OwnerKind::Adopted),
            Some(install) => (install.name.clone(), OwnerKind::Riftstone),
            None => (option_id.to_string(), OwnerKind::Riftstone),
        },
    }
}

/// Uninstall `option_id`. Only files the journal records are touched; what
/// was below comes back; files something else changed since are left alone,
/// and the backups under them are kept aside on record.
pub fn uninstall(
    ctx: &DeployContext,
    journal: &mut Journal,
    option_id: &OptionId,
) -> Result<UninstallReport, RiftError> {
    let (plan, work) = prepare_uninstall(ctx, journal, option_id);
    if let Some(reason) = plan.refused {
        return Err(RiftError::refused(reason));
    }
    let position = journal
        .position(option_id)
        .ok_or_else(|| RiftError::refused("the install vanished from the journal"))?;
    let seq = allocate_seq(journal, ctx.state_dir);
    let steps = removal_steps(&work, seq);
    temporaries_free(&ctx.game.root, &steps)?;

    let before = journal.clone();
    let installed = &journal.installs[position];
    journal.pending = Some(Intent {
        action: IntentAction::Uninstall,
        seq,
        option_id: option_id.clone(),
        touched: installed
            .files
            .iter()
            .map(|f| f.game_path.clone())
            .collect(),
        started_at: now(),
        option_name: installed.name.clone(),
        steps: steps.clone(),
        phase: 0,
        dirs: Vec::new(),
    });
    journal.next_seq = seq + 1;
    if let Err(err) = journal.save(ctx.state_dir) {
        *journal = before;
        return Err(err);
    }
    if let Err(err) = execute_removals(ctx, &steps) {
        return Err(fail_and_roll_back(ctx, journal, &before, err));
    }

    let mut committed = journal.clone();
    let parked = apply_uninstall(&mut committed, position, &work, &steps);
    committed.pending = None;
    if let Err(err) = committed.save(ctx.state_dir) {
        return Err(fail_and_roll_back(ctx, journal, &before, err));
    }
    *journal = committed;
    process_cleanup(ctx, journal);
    prune_restored_slots(ctx, &steps);

    let count =
        |f: fn(&RemovalAction) -> bool| plan.removals.iter().filter(|r| f(&r.action)).count();
    Ok(UninstallReport {
        option_id: option_id.clone(),
        files_removed: count(|a| matches!(a, RemovalAction::Delete)),
        files_restored: count(|a| matches!(a, RemovalAction::RestoreBackup { .. })),
        files_kept: count(|a| matches!(a, RemovalAction::KeepChanged | RemovalAction::KeepVanilla)),
        files_handed_down: count(|a| matches!(a, RemovalAction::HandDown { .. })),
        restart_needed: ctx.game_running,
        parked,
    })
}

/// The moves an uninstall makes on disk, as intent steps.
fn removal_steps(work: &[(usize, RemovalWork)], seq: u64) -> Vec<IntentStep> {
    let trash = |path: &str| format!("{path}{TRASH_MARK}{seq}");
    work.iter()
        .filter_map(|(_, removal)| match removal {
            RemovalWork::Delete { game_path } => Some(IntentStep::Remove {
                game_path: game_path.clone(),
                trash: trash(game_path),
            }),
            RemovalWork::Restore {
                game_path,
                backup,
                present,
            } => Some(IntentStep::Restore {
                game_path: game_path.clone(),
                trash: present.then(|| trash(game_path)),
                backup: backup.clone(),
            }),
            _ => None,
        })
        .collect()
}

/// The names an uninstall moves files through must be free: a rename onto a
/// taken name would replace what sits there, and a copy would write through
/// a link.
fn temporaries_free(root: &Path, steps: &[IntentStep]) -> Result<(), RiftError> {
    let taken = |name: &str| {
        RiftError::refused(format!(
            "\"{name}\" already sits in the game folder. The Riftstone moves files through that name and never over something it did not put there; move it out of the game folder, then try again."
        ))
    };
    for step in steps {
        match step {
            IntentStep::Remove { trash, .. } => {
                if fsops::exists(&join_rel(root, trash)) {
                    return Err(taken(trash));
                }
            }
            IntentStep::Restore {
                game_path, trash, ..
            } => {
                if let Some(trash) = trash {
                    if fsops::exists(&join_rel(root, trash)) {
                        return Err(taken(trash));
                    }
                }
                let part = format!("{game_path}{PART_SUFFIX}");
                if fsops::exists(&join_rel(root, &part)) {
                    return Err(taken(&part));
                }
            }
            IntentStep::Place { .. } | IntentStep::Rename { .. } | IntentStep::Park { .. } => {}
        }
    }
    Ok(())
}

/// Record an uninstall in `journal`: the layers (and kept files) above its
/// files inherit what was below them, backups that cannot go back are kept
/// aside on record, the install leaves, and what the commit owes joins the
/// cleanup list. Returns the files this uninstall kept aside.
fn apply_uninstall(
    journal: &mut Journal,
    position: usize,
    work: &[(usize, RemovalWork)],
    steps: &[IntentStep],
) -> Vec<ParkedBackup> {
    let own = journal.installs[position].files.clone();
    let name = journal.installs[position].name.clone();
    let option_id = journal.installs[position].option_id.clone();
    let mut cleanup: Vec<Leftover> = steps
        .iter()
        .filter_map(|step| match step {
            IntentStep::Remove { trash, .. }
            | IntentStep::Restore {
                trash: Some(trash), ..
            } => Some(Leftover::Game {
                path: trash.clone(),
            }),
            _ => None,
        })
        .collect();
    let mut parked: Vec<ParkedBackup> = Vec::new();
    let mut dropped: Vec<usize> = Vec::new();
    let mismatch = || {
        format!(
            "Kept aside when \"{name}\" was uninstalled: the backup above it no longer held that install's bytes, so neither could be thrown away."
        )
    };
    for (file_index, removal) in work {
        let file = &own[*file_index];
        match removal {
            RemovalWork::HandDown { above, slot } => {
                let upper = &mut journal.installs[above.install].files[above.file];
                match slot {
                    SlotState::Ours | SlotState::Missing => {
                        let superseded = std::mem::replace(&mut upper.below, file.below.clone());
                        if *slot == SlotState::Ours {
                            if let Below::Backup { store, .. } = superseded {
                                cleanup.push(Leftover::Store { path: store });
                            }
                        }
                    }
                    SlotState::Foreign => {
                        // The slot holds bytes that are not this install's:
                        // the layer above keeps them as its own below…
                        if let Below::Backup { owner, .. } = &mut upper.below {
                            *owner = BelowOwner::Unowned;
                        }
                        // …and what was below this install is kept on record.
                        if let Below::Backup { store, owner } = &file.below {
                            parked.push(ParkedBackup {
                                store: store.clone(),
                                game_path: file.game_path.clone(),
                                owner: owner.clone(),
                                parked_at: now(),
                                reason: mismatch(),
                            });
                        }
                    }
                }
            }
            RemovalWork::HandDownParked { parked: at, slot } => {
                let entry = &mut journal.parked[*at];
                match (slot, &file.below) {
                    (SlotState::Foreign, below) => {
                        entry.owner = BelowOwner::Unowned;
                        if let Below::Backup { store, owner } = below {
                            parked.push(ParkedBackup {
                                store: store.clone(),
                                game_path: file.game_path.clone(),
                                owner: owner.clone(),
                                parked_at: now(),
                                reason: mismatch(),
                            });
                        }
                    }
                    (_, Below::Backup { store, owner }) => {
                        let superseded = std::mem::replace(&mut entry.store, store.clone());
                        entry.owner = owner.clone();
                        if *slot == SlotState::Ours {
                            cleanup.push(Leftover::Store { path: superseded });
                        }
                    }
                    // What was below may be the game's own file: keep the bytes.
                    (SlotState::Ours, Below::UnknownVanilla) => entry.owner = BelowOwner::Unowned,
                    (SlotState::Ours, Below::Nothing) => {
                        cleanup.push(Leftover::Store {
                            path: entry.store.clone(),
                        });
                        dropped.push(*at);
                    }
                    (SlotState::Missing, _) => dropped.push(*at),
                }
            }
            RemovalWork::Park { backup } => parked.push(backup.clone()),
            RemovalWork::Delete { .. } | RemovalWork::Restore { .. } | RemovalWork::Keep => {}
        }
    }
    dropped.sort_unstable();
    dropped.dedup();
    for at in dropped.into_iter().rev() {
        journal.parked.remove(at);
    }
    journal.parked.extend(parked.iter().cloned());
    journal.installs.remove(position);
    journal.pak_order.retain(|id| id != &option_id);
    journal.cleanup.extend(cleanup);
    parked
}

fn execute_removals(ctx: &DeployContext, steps: &[IntentStep]) -> Result<(), RiftError> {
    let root = &ctx.game.root;
    let in_use = |path: &str| {
        format!("could not remove {path} (is the game or another program holding it?)")
    };
    for step in steps {
        match step {
            IntentStep::Remove { game_path, trash } => {
                fsops::rename(&join_rel(root, game_path), &join_rel(root, trash))
                    .map_err(|err| RiftError::io(in_use(game_path), err))?;
            }
            IntentStep::Restore {
                game_path,
                trash,
                backup,
            } => {
                let target = join_rel(root, game_path);
                if let Some(trash) = trash {
                    fsops::rename(&target, &join_rel(root, trash))
                        .map_err(|err| RiftError::io(in_use(game_path), err))?;
                }
                fsops::move_file(&join_rel(ctx.state_dir, backup), &target).map_err(|err| {
                    RiftError::io(format!("could not put the backup of {game_path} back"), err)
                })?;
            }
            IntentStep::Place { .. } | IntentStep::Rename { .. } | IntentStep::Park { .. } => {}
        }
    }
    Ok(())
}

/// Remove the store folders emptied by backups that went back into place.
fn prune_restored_slots(ctx: &DeployContext, steps: &[IntentStep]) {
    let backups = ctx.state_dir.join("backups");
    for step in steps {
        if let IntentStep::Restore { backup, .. } = step {
            fsops::prune_empty_dirs(join_rel(ctx.state_dir, backup).parent(), &backups);
        }
    }
}

// ---------------------------------------------------------------------------
// Files kept aside
// ---------------------------------------------------------------------------

/// Put a file kept aside back where it belongs (`store` is its place in the
/// backup store, as [`crate::GameState::parked`] lists it). A free path
/// simply takes it back. A file nobody owns at the path — usually the change
/// that made the Riftstone keep it — swaps with it: that file is kept aside
/// in its turn, so nothing is lost and the swap can be reversed the same way.
/// Refused while an install in the journal (an adopted one too) owns the
/// file at the path: uninstall that first. A running game does not stop it —
/// the file loads at the next launch (`restart_needed`); one the game holds
/// open fails to move, and the swap is rolled back. One journaled intent,
/// rolled back on failure.
pub fn restore_parked(
    ctx: &DeployContext,
    journal: &mut Journal,
    store: &str,
) -> Result<ParkedRestoreReport, RiftError> {
    if let Some(reason) = common_refusal(ctx, journal) {
        return Err(RiftError::refused(reason));
    }
    let Some(at) = journal.parked.iter().position(|kept| kept.store == store) else {
        return Err(stale_parked(store));
    };
    let entry = journal.parked[at].clone();
    if !safe_store(&entry.store) || !safe_rel(&entry.game_path) {
        return Err(RiftError::refused(
            "The journal's record of this kept file names a path outside the game folder or the backup store, so the Riftstone will not act on it.",
        ));
    }
    let root = &ctx.game.root;
    let slot = join_rel(ctx.state_dir, &entry.store);
    if !fsops::is_file(&slot) {
        return Err(RiftError::refused(format!(
            "The kept file for \"{}\" is no longer in the backup store ({}). There is nothing left to put back; discard the entry.",
            entry.game_path,
            slot.display()
        )));
    }
    let mut resolver = CaseResolver::new(root);
    let game_path = resolver.resolve(&entry.game_path);
    let target = guard::guard_target(root, &game_path)?;
    let key = lower_key(&game_path);
    let occupied = fsops::is_file(&target);
    if occupied {
        let index = PathIndex::build(journal);
        if let Some(top) = index.top(journal, &key) {
            if still_ours(&target, top.file(journal)) {
                return Err(RiftError::refused(format!(
                    "\"{}\" is installed over {game_path}, so the kept file cannot go back there now. Uninstall it first, then put the kept file back.",
                    top.install(journal).name
                )));
            }
        }
    }

    let seq = allocate_seq(journal, ctx.state_dir);
    let swap_store = format!("backups/{seq}/{game_path}");
    let mut steps = Vec::new();
    if occupied {
        steps.push(IntentStep::Park {
            game_path: game_path.clone(),
            store: swap_store.clone(),
        });
    }
    steps.push(IntentStep::Restore {
        game_path: game_path.clone(),
        trash: None,
        backup: entry.store.clone(),
    });
    temporaries_free(root, &steps)?;
    let dirs = fsops::missing_dirs(root, &game_path);

    let before = journal.clone();
    journal.pending = Some(Intent {
        action: IntentAction::RestoreParked,
        seq,
        option_id: OptionId(String::new()),
        touched: vec![game_path.clone()],
        started_at: now(),
        option_name: format!("the file kept aside for {game_path}"),
        steps,
        phase: 0,
        dirs: dirs.clone(),
    });
    journal.next_seq = seq + 1;
    if let Err(err) = journal.save(ctx.state_dir) {
        *journal = before;
        return Err(err);
    }
    let moved = (|| {
        if occupied {
            fsops::move_file(&target, &join_rel(ctx.state_dir, &swap_store)).map_err(|err| {
                RiftError::io(
                    format!("could not move {game_path} aside into the backup store"),
                    err,
                )
            })?;
        }
        fsops::move_file(&slot, &target).map_err(|err| {
            RiftError::io(
                format!("could not put the kept file back at {game_path}"),
                err,
            )
        })
    })();
    if let Err(err) = moved {
        return Err(fail_and_roll_back(ctx, journal, &before, err));
    }

    let mut committed = journal.clone();
    committed.parked.remove(at);
    let swapped_out = occupied.then(|| ParkedBackup {
        store: swap_store,
        game_path: game_path.clone(),
        owner: BelowOwner::Unowned,
        parked_at: now(),
        reason: format!("Kept aside when the file kept for {game_path} was put back in its place."),
    });
    committed.parked.extend(swapped_out.clone());
    note_created_dirs(&mut committed.created_dirs, root, &dirs);
    committed.pending = None;
    if let Err(err) = committed.save(ctx.state_dir) {
        return Err(fail_and_roll_back(ctx, journal, &before, err));
    }
    *journal = committed;
    fsops::prune_empty_dirs(slot.parent(), &ctx.state_dir.join("backups"));
    Ok(ParkedRestoreReport {
        game_path,
        restored: entry,
        swapped_out,
        restart_needed: ctx.game_running,
    })
}

/// Throw away a file kept aside — the player decided it is not needed. Only
/// the kept copy in the backup store is deleted; the game folder is not
/// touched.
pub fn discard_parked(
    ctx: &DeployContext,
    journal: &mut Journal,
    store: &str,
) -> Result<(), RiftError> {
    if let Some(reason) = common_refusal(ctx, journal) {
        return Err(RiftError::refused(reason));
    }
    let Some(at) = journal.parked.iter().position(|kept| kept.store == store) else {
        return Err(stale_parked(store));
    };
    let mut committed = journal.clone();
    let entry = committed.parked.remove(at);
    if safe_store(&entry.store) {
        committed
            .cleanup
            .push(Leftover::Store { path: entry.store });
    }
    committed.save(ctx.state_dir)?;
    *journal = committed;
    process_cleanup(ctx, journal);
    Ok(())
}

/// The refusal for a handle the journal no longer lists: the window's list
/// of kept files was read before that file went back, was discarded, or
/// moved to another entry. It names the game path the handle stands for,
/// never the handle — an inner path of the backup store means nothing to the
/// player — and asks for what the window offers: a fresh look at PARKED,
/// which the window reads again after any refusal.
fn stale_parked(store: &str) -> RiftError {
    let entry = parked_game_path(store).map_or_else(
        || "that entry".to_string(),
        |path| format!("the entry for {path}"),
    );
    RiftError::refused(format!(
        "The list of kept files was out of date ({entry} changed after it was shown), so nothing was touched. Look again at PARKED."
    ))
}

/// The game path a store handle stands for: every kept file sits at
/// `backups/<seq>/<game path>`. `None` for anything else.
fn parked_game_path(store: &str) -> Option<&str> {
    if !safe_store(store) {
        return None;
    }
    let (seq, game_path) = store.get("backups/".len()..)?.split_once('/')?;
    (!seq.is_empty() && seq.bytes().all(|b| b.is_ascii_digit()) && safe_rel(game_path))
        .then_some(game_path)
}

// ---------------------------------------------------------------------------
// Pak order
// ---------------------------------------------------------------------------

/// Make `order` (option ids, lowest priority first) the pak load order:
/// every managed pak — Riftstone's and adopted ones — whose name does not
/// match its position is renamed, in two journaled phases (all to
/// temporaries, then all to their final names). Refused while DD2 runs.
pub fn reorder_paks(
    ctx: &DeployContext,
    journal: &mut Journal,
    order: &[OptionId],
) -> Result<(), RiftError> {
    if let Some(reason) = common_refusal(ctx, journal) {
        return Err(RiftError::refused(reason));
    }
    if ctx.game_running {
        return Err(RiftError::refused(
            "Dragon's Dogma 2 is running and holds its paks open. Close the game, then change the load order.",
        ));
    }
    let managed: Vec<usize> = journal
        .installs
        .iter()
        .enumerate()
        .filter(|(_, i)| i.has_paks())
        .map(|(n, _)| n)
        .collect();
    let mut seen = HashSet::new();
    for id in order {
        if !seen.insert(id) {
            return Err(RiftError::refused(format!(
                "The new load order lists \"{id}\" twice; send every pak-bearing install exactly once."
            )));
        }
        if !managed
            .iter()
            .any(|&n| &journal.installs[n].option_id == id)
        {
            return Err(RiftError::refused(format!(
                "\"{id}\" has no paks installed by the Riftstone, so it has no place in the load order."
            )));
        }
    }
    if let Some(&left_out) = managed
        .iter()
        .find(|&&n| !seen.contains(&journal.installs[n].option_id))
    {
        return Err(RiftError::refused(format!(
            "The new load order leaves out \"{}\"; send every pak-bearing install exactly once.",
            journal.installs[left_out].name
        )));
    }
    if order.len() > (paks::MAX_PAK_POSITION as usize) + 1 {
        return Err(RiftError::refused(
            "More than 10,000 pak installs cannot be numbered.",
        ));
    }

    let root = &ctx.game.root;
    let mut resolver = CaseResolver::new(root);
    let pak_dir = resolver.resolve(PAK_MODS_DIR);
    // (install, file, from, to)
    let mut renames: Vec<(usize, usize, String, String)> = Vec::new();
    for (position, id) in order.iter().enumerate() {
        let install = journal.position(id).unwrap_or_default();
        let mut used = HashSet::new();
        for (file_index, file) in journal.installs[install].files.iter().enumerate() {
            if file.kind != DeployKind::Pak {
                continue;
            }
            let stem = paks::unique_stem(paks::pak_stem(file_name(&file.game_path)), &mut used);
            let position = u32::try_from(position).unwrap_or(paks::MAX_PAK_POSITION);
            let target = format!("{pak_dir}/{}", paks::managed_pak_name(position, &stem));
            let from = resolver.resolve(&file.game_path);
            if file_name(&from) != file_name(&target) {
                renames.push((install, file_index, from, target));
            }
        }
    }
    if renames.is_empty() {
        let mut committed = journal.clone();
        committed.pak_order = order.to_vec();
        committed.save(ctx.state_dir)?;
        *journal = committed;
        return Ok(());
    }
    let sources: HashSet<String> = renames.iter().map(|r| lower_key(&r.2)).collect();
    for (_, _, from, to) in &renames {
        guard::guard_target(root, to)?;
        if !fsops::is_file(&join_rel(root, from)) {
            return Err(RiftError::refused(format!(
                "\"{from}\" is missing from pak_mods, so the order cannot be applied. Reinstall or uninstall that mod first."
            )));
        }
        if fsops::exists(&join_rel(root, to)) && !sources.contains(&lower_key(to)) {
            return Err(RiftError::refused(format!(
                "A pak the Riftstone does not manage is already named \"{}\" in pak_mods. Move it out of pak_mods, then reorder again.",
                file_name(to)
            )));
        }
    }

    let seq = allocate_seq(journal, ctx.state_dir);
    let steps: Vec<IntentStep> = renames
        .iter()
        .enumerate()
        .map(|(n, (_, _, from, to))| IntentStep::Rename {
            from: from.clone(),
            temp: format!("{from}{REORDER_MARK}{n}"),
            to: to.clone(),
        })
        .collect();
    for step in &steps {
        if let IntentStep::Rename { temp, .. } = step {
            if fsops::exists(&join_rel(root, temp)) {
                return Err(RiftError::refused(format!(
                    "\"{temp}\" already sits in pak_mods. The Riftstone renames paks through that name and never over something it did not put there; move it out of pak_mods, then reorder again."
                )));
            }
        }
    }
    let before = journal.clone();
    journal.pending = Some(Intent {
        action: IntentAction::Reorder,
        seq,
        option_id: OptionId(String::new()),
        touched: renames
            .iter()
            .flat_map(|r| [r.2.clone(), r.3.clone()])
            .collect(),
        started_at: now(),
        option_name: "the pak load order".to_string(),
        steps: steps.clone(),
        phase: 0,
        dirs: Vec::new(),
    });
    journal.next_seq = seq + 1;
    if let Err(err) = journal.save(ctx.state_dir) {
        *journal = before;
        return Err(err);
    }
    let rename = |from: &str, to: &str| {
        fsops::rename(&join_rel(root, from), &join_rel(root, to))
            .map_err(|err| RiftError::io(format!("could not rename {from} to {to}"), err))
    };
    for step in &steps {
        if let IntentStep::Rename { from, temp, .. } = step {
            if let Err(err) = rename(from, temp) {
                return Err(fail_and_roll_back(ctx, journal, &before, err));
            }
        }
    }
    if let Some(pending) = journal.pending.as_mut() {
        pending.phase = 1;
    }
    if let Err(err) = journal.save(ctx.state_dir) {
        return Err(fail_and_roll_back(ctx, journal, &before, err));
    }
    for step in &steps {
        if let IntentStep::Rename { temp, to, .. } = step {
            if fsops::exists(&join_rel(root, to)) {
                let err = RiftError::refused(format!(
                    "\"{to}\" appeared in pak_mods during the reorder; the old order was put back."
                ));
                return Err(fail_and_roll_back(ctx, journal, &before, err));
            }
            if let Err(err) = rename(temp, to) {
                return Err(fail_and_roll_back(ctx, journal, &before, err));
            }
        }
    }
    let mut committed = journal.clone();
    for (install, file, _, to) in &renames {
        committed.installs[*install].files[*file]
            .game_path
            .clone_from(to);
    }
    committed.pak_order = order.to_vec();
    committed.pending = None;
    if let Err(err) = committed.save(ctx.state_dir) {
        return Err(fail_and_roll_back(ctx, journal, &before, err));
    }
    *journal = committed;
    Ok(())
}

// ---------------------------------------------------------------------------
// Re-keying installs (v0.3: folder ids follow the name; the relink pass)
// ---------------------------------------------------------------------------

/// Give installs new option ids ([`Journal::rekey`]) and save the journal
/// once, atomically. A refusal or a failed save changes nothing, on disk or
/// in memory; a map that changes nothing saves nothing. Nothing in the game
/// folder moves. Returns how many installs took a new id.
pub fn rekey(
    ctx: &DeployContext,
    journal: &mut Journal,
    map: &BTreeMap<OptionId, OptionId>,
) -> RiftResult<usize> {
    rekey_with_files(ctx, journal, map, &BTreeMap::new())
}

/// [`rekey`], and each re-keyed install whose NEW id is in `package_files`
/// takes that file name as its `package_file` ([`Journal::rekey_with_files`]).
pub fn rekey_with_files(
    ctx: &DeployContext,
    journal: &mut Journal,
    map: &BTreeMap<OptionId, OptionId>,
    package_files: &BTreeMap<OptionId, String>,
) -> RiftResult<usize> {
    if let Some(reason) = common_refusal(ctx, journal) {
        return Err(RiftError::refused(reason));
    }
    let mut committed = journal.clone();
    let rekeyed = committed.rekey_with_files(map, package_files)?;
    if committed != *journal {
        committed.save(ctx.state_dir)?;
        *journal = committed;
    }
    Ok(rekeyed)
}

// ---------------------------------------------------------------------------
// Recovery
// ---------------------------------------------------------------------------

/// At launch: roll back an operation a crash left in the journal, then
/// finish any deletions a committed operation still owes. `None` when there
/// was nothing to roll back. When the roll-back cannot finish, the intent
/// stays recorded and the next launch tries again.
pub fn recover(
    ctx: &DeployContext,
    journal: &mut Journal,
) -> Result<Option<RecoveryReport>, RiftError> {
    if !journal.belongs_to(&ctx.game.root) {
        return Err(RiftError::Journal {
            detail: format!(
                "the journal belongs to {}, not {}",
                journal.game_root.display(),
                ctx.game.root.display()
            ),
        });
    }
    let mut report = None;
    if let Some(intent) = journal.pending.clone() {
        let mut rolled = journal.clone();
        rolled.pending = None;
        let what = action_words(intent.action);
        let outcome = roll_back(ctx, journal, rolled).map_err(|failure| match failure {
            RollbackFailure::Undo(detail) => RiftError::Io {
                detail: format!(
                    "could not finish putting the game folder back after the interrupted {what} ({detail}); it stays recorded, and the Riftstone tries again at its next start"
                ),
            },
            RollbackFailure::Journal(err) => RiftError::Io {
                detail: format!(
                    "the game folder was put back after the interrupted {what}, but the journal could not record that ({err}); the Riftstone checks again at its next start"
                ),
            },
        })?;
        report = Some(RecoveryReport {
            action: intent.action,
            option_id: intent.option_id,
            option_name: intent.option_name,
            files_rolled_back: outcome.rolled_back,
            files_kept: outcome.kept,
            parked: outcome.parked,
        });
    }
    process_cleanup(ctx, journal);
    Ok(report)
}

fn action_words(action: IntentAction) -> &'static str {
    match action {
        IntentAction::Install => "install",
        IntentAction::Uninstall => "uninstall",
        IntentAction::Reorder => "change of the pak order",
        IntentAction::RestoreParked => "return of a kept file",
    }
}

/// What undoing an intent did.
#[derive(Debug, Default)]
struct UndoOutcome {
    rolled_back: usize,
    kept: usize,
    /// Backups that could not go back: kept aside, on record once saved.
    parked: Vec<ParkedBackup>,
    /// Steps that could not be undone (yet).
    errors: Vec<String>,
    /// Game paths (lower-cased) whose `.riftstone-part` a stopped extraction
    /// never wrote and so left to whoever holds it: an earlier step at the
    /// same path (a reinstall's restore of the old copy) leaves it too.
    spared_parts: HashSet<String>,
}

/// Why a roll-back did not finish.
enum RollbackFailure {
    /// A step could not be undone; the intent stays pending.
    Undo(String),
    /// Everything was undone, but the journal could not record it; the
    /// intent stays pending (in memory as on disk).
    Journal(RiftError),
}

/// Undo the pending intent on disk, then save `rolled` — the journal as it
/// should read once the intent is gone — with what the undo kept aside.
/// Until that save succeeds, the journal in memory keeps its pending intent,
/// exactly as the disk does.
fn roll_back(
    ctx: &DeployContext,
    journal: &mut Journal,
    mut rolled: Journal,
) -> Result<UndoOutcome, RollbackFailure> {
    let Some(intent) = journal.pending.clone() else {
        return Ok(UndoOutcome::default());
    };
    let outcome = undo_intent(ctx, &intent);
    if !outcome.errors.is_empty() {
        return Err(RollbackFailure::Undo(outcome.errors.join("; ")));
    }
    rolled.pending = None;
    rolled.next_seq = rolled.next_seq.max(intent.seq + 1);
    rolled.parked.extend(outcome.parked.iter().cloned());
    rolled
        .save(ctx.state_dir)
        .map_err(RollbackFailure::Journal)?;
    *journal = rolled;
    Ok(outcome)
}

/// Roll the pending intent back after `err`, returning the error to report:
/// `err` itself when the game folder and the journal are back, otherwise
/// `err` plus exactly what could not be finished.
fn fail_and_roll_back(
    ctx: &DeployContext,
    journal: &mut Journal,
    before: &Journal,
    err: RiftError,
) -> RiftError {
    match roll_back(ctx, journal, before.clone()) {
        Ok(_) => err,
        Err(RollbackFailure::Undo(detail)) => RiftError::Io {
            detail: format!(
                "{err} — and putting the game folder back failed too ({detail}); the Riftstone will finish undoing it at its next start"
            ),
        },
        Err(RollbackFailure::Journal(save)) => RiftError::Io {
            detail: format!(
                "{err} — the game folder was put back, but the journal could not record that ({save}); the Riftstone checks again at its next start"
            ),
        },
    }
}

/// Undo every step of `intent` that happened, judged from what is on disk,
/// in reverse — every step, even when one of them fails. Idempotent: undoing
/// an undone intent changes nothing.
fn undo_intent(ctx: &DeployContext, intent: &Intent) -> UndoOutcome {
    let root = &ctx.game.root;
    let mut outcome = UndoOutcome::default();
    let steps: Vec<&IntentStep> = intent
        .steps
        .iter()
        .filter(|step| {
            let safe = step_is_safe(step);
            if !safe {
                log::warn!(
                    "the journal records a step with an unsafe path; it was not acted on: {step:?}"
                );
            }
            safe
        })
        .collect();
    if intent.action == IntentAction::Reorder {
        undo_reorder(root, &steps, intent.phase, &mut outcome);
    } else {
        // A reinstall's intent can touch one path twice (the old copy's
        // removal, then the new copy's placement). Once a step at a path
        // fails, or finds something there that is not the operation's own,
        // the earlier steps at that path wait for the next attempt rather
        // than act on a file whose story this pass no longer knows.
        let mut hands_off: HashSet<String> = HashSet::new();
        for step in steps.iter().rev() {
            let key = lower_key(step_game_path(step));
            if hands_off.contains(&key) {
                log::warn!(
                    "{} was left as it is: a later step at that path could not be undone cleanly",
                    step_game_path(step)
                );
                continue;
            }
            let kept_before = outcome.kept;
            let undone = match step {
                IntentStep::Place {
                    game_path,
                    size,
                    backup,
                    owner,
                    fingerprint,
                    unextracted,
                } => undo_place(
                    ctx,
                    intent,
                    &PlaceUndo {
                        game_path,
                        size: *size,
                        backup: backup.as_deref(),
                        owner: owner.as_ref(),
                        fingerprint: fingerprint.as_deref(),
                        unextracted: *unextracted,
                    },
                    &mut outcome,
                )
                .map_err(|err| format!("could not undo the install of {game_path}: {err}")),
                IntentStep::Remove { game_path, trash } => {
                    undo_remove(root, game_path, trash, &mut outcome)
                        .map_err(|err| format!("could not put {game_path} back: {err}"))
                }
                IntentStep::Restore {
                    game_path,
                    trash,
                    backup,
                } => {
                    let partial_spared = outcome.spared_parts.contains(&key);
                    undo_restore(
                        root,
                        ctx.state_dir,
                        &RestoreUndo {
                            game_path,
                            trash: trash.as_deref(),
                            backup,
                            partial_spared,
                        },
                        &mut outcome,
                    )
                    .map_err(|err| format!("could not put {game_path} back: {err}"))
                }
                IntentStep::Park { game_path, store } => {
                    undo_park(ctx, game_path, store, &mut outcome)
                        .map_err(|err| format!("could not put {game_path} back: {err}"))
                }
                IntentStep::Rename { .. } => Ok(()),
            };
            if let Err(error) = undone {
                log::warn!("{error}");
                outcome.errors.push(error);
                hands_off.insert(key);
            } else if outcome.kept > kept_before {
                hands_off.insert(key);
            }
        }
    }
    // The folders this operation created, deepest first, once empty again.
    let mut dirs: Vec<&String> = intent.dirs.iter().filter(|d| safe_rel(d)).collect();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.matches('/').count()));
    for dir in dirs {
        let path = join_rel(root, dir);
        if fsops::is_real_dir(&path) {
            let _ = fs::remove_dir(&path);
        }
    }
    fsops::remove_empty_tree(&ctx.state_dir.join("backups").join(intent.seq.to_string()));
    outcome
}

/// One `Place` step, borrowed.
struct PlaceUndo<'a> {
    game_path: &'a str,
    size: u64,
    backup: Option<&'a str>,
    owner: Option<&'a BelowOwner>,
    fingerprint: Option<&'a str>,
    unextracted: bool,
}

fn undo_place(
    ctx: &DeployContext,
    intent: &Intent,
    step: &PlaceUndo<'_>,
    outcome: &mut UndoOutcome,
) -> io::Result<()> {
    let root = &ctx.game.root;
    let target = join_rel(root, step.game_path);
    let part = with_suffix(&target, PART_SUFFIX);
    let mut undone = false;
    // The part is this install's own: its name was free when the install
    // began, and the extractor only ever creates files new. Unless the
    // stopped extraction never reported writing it (`unextracted`): then
    // whatever holds the name is someone else's, and it stays. It is no
    // file at the target either, so it does not keep the path's other steps
    // from being undone.
    let part_left = fsops::exists(&part);
    if part_left && !fsops::is_real_dir(&part) {
        if step.unextracted {
            log::warn!(
                "{}{PART_SUFFIX} is not a part the install wrote (its extraction stopped before it); it was left alone",
                step.game_path
            );
            outcome.spared_parts.insert(lower_key(step.game_path));
        } else {
            fsops::remove_file_if_exists(&part)?;
            undone = true;
        }
    }
    let slot = step.backup.map(|backup| join_rel(ctx.state_dir, backup));
    if let Some(slot) = &slot {
        // Half a cross-volume copy into the store.
        fsops::remove_file_if_exists(&with_suffix(slot, PART_SUFFIX))?;
    }
    // A folder at the target can only be empty folders of this install's
    // extraction — or something else's, which then holds files and stays.
    if fsops::is_real_dir(&target) {
        fsops::remove_empty_tree(&target);
    }
    let occupied = fsops::exists(&target);
    // A file at the target is this install's only once its part was renamed
    // (none is left) and its size and fingerprint match the ones saved
    // before the first rename. Without that proof it is never deleted.
    let ours = !part_left
        && occupied
        && step.fingerprint.is_some_and(|fingerprint| {
            fsops::file_len(&target) == Some(step.size)
                && fsops::fingerprint(&target).ok().as_deref() == Some(fingerprint)
        });
    let writable = guard::guard_target(root, step.game_path).is_ok();
    match slot.as_ref().filter(|slot| fsops::is_file(slot)) {
        Some(slot) => {
            if ours && writable {
                // One step: the path never stands empty, even if this fails.
                fsops::replace_file(slot, &target)?;
                undone = true;
            } else if !occupied && writable {
                fsops::move_file(slot, &target)?;
                undone = true;
            } else if occupied && !ours && fsops::same_content(&target, slot) {
                // A cross-volume move into the store stopped before deleting
                // its source: the file never left.
                fsops::remove_file_if_exists(slot)?;
                undone = true;
            } else {
                log::warn!(
                    "{} holds a file the install did not write; it was left alone and the backup was kept aside",
                    step.game_path
                );
                outcome.parked.push(ParkedBackup {
                    store: step.backup.unwrap_or_default().to_string(),
                    game_path: step.game_path.to_string(),
                    owner: step.owner.cloned().unwrap_or(BelowOwner::Unowned),
                    parked_at: now(),
                    reason: format!(
                        "Kept aside when the install of \"{}\" was rolled back: something else had appeared at this path meanwhile, so the file that was there before could not go back without overwriting it.",
                        intent.option_name
                    ),
                });
                outcome.kept += 1;
            }
            if undone {
                fsops::prune_empty_dirs(slot.parent(), &ctx.state_dir.join("backups"));
            }
        }
        None if ours && writable => {
            fsops::remove_file_if_exists(&target)?;
            undone = true;
        }
        None if ours || (occupied && step.backup.is_none()) => {
            log::warn!(
                "{} is not a file the install wrote; it was left alone",
                step.game_path
            );
            outcome.kept += 1;
        }
        // The original is in place: it never moved, or is already back.
        None => {}
    }
    if undone {
        outcome.rolled_back += 1;
    }
    Ok(())
}

fn undo_remove(
    root: &Path,
    game_path: &str,
    trash: &str,
    outcome: &mut UndoOutcome,
) -> io::Result<()> {
    // A Remove is a rename inside one folder: no copy, so no partial file.
    let target = join_rel(root, game_path);
    let trashed = join_rel(root, trash);
    if fsops::exists(&trashed) {
        if fsops::exists(&target) {
            log::warn!("{game_path} reappeared during the uninstall; the uninstalled copy stays at {trash}");
            outcome.kept += 1;
        } else {
            fsops::rename(&trashed, &target)?;
            outcome.rolled_back += 1;
        }
    }
    Ok(())
}

/// One `Restore` step, borrowed.
struct RestoreUndo<'a> {
    game_path: &'a str,
    trash: Option<&'a str>,
    backup: &'a str,
    /// A reinstall's new copy left `<game_path>.riftstone-part` to whoever
    /// holds it ([`UndoOutcome::spared_parts`]): this restore finished
    /// before that extraction began, so the name is not its partial copy.
    partial_spared: bool,
}

fn undo_restore(
    root: &Path,
    store: &Path,
    step: &RestoreUndo<'_>,
    outcome: &mut UndoOutcome,
) -> io::Result<()> {
    let (game_path, trash, backup) = (step.game_path, step.trash, step.backup);
    let target = join_rel(root, game_path);
    let slot = join_rel(store, backup);
    // A cross-volume restore that stopped half-way leaves a partial copy
    // beside the target (its name was checked free before the uninstall)…
    let partial = with_suffix(&target, PART_SUFFIX);
    if !step.partial_spared && !fsops::is_real_dir(&partial) {
        fsops::remove_file_if_exists(&partial)?;
    }
    // …and a move back into the store that stopped half-way, one there.
    fsops::remove_file_if_exists(&with_suffix(&slot, PART_SUFFIX))?;
    match trash {
        Some(trash) => {
            let trashed = join_rel(root, trash);
            if !fsops::exists(&trashed) {
                // The install's file never moved: nothing happened here.
                return Ok(());
            }
            if fsops::exists(&target) {
                if fsops::is_file(&slot) && fsops::same_content(&target, &slot) {
                    // The backup's cross-volume move stopped before deleting
                    // its source: the copy in place is redundant.
                    fsops::remove_file_if_exists(&target)?;
                } else if !fsops::is_file(&slot) {
                    // The backup was restored: send it back to the store.
                    fsops::move_file(&target, &slot)?;
                } else {
                    log::warn!("{game_path} was changed by something else during the uninstall; it was left alone");
                    outcome.kept += 1;
                    return Ok(());
                }
            }
            fsops::rename(&trashed, &target)?;
            outcome.rolled_back += 1;
        }
        None => {
            if fsops::exists(&target) {
                if !fsops::is_file(&slot) {
                    fsops::move_file(&target, &slot)?;
                    outcome.rolled_back += 1;
                } else if fsops::same_content(&target, &slot) {
                    fsops::remove_file_if_exists(&target)?;
                    outcome.rolled_back += 1;
                }
            }
        }
    }
    Ok(())
}

fn undo_park(
    ctx: &DeployContext,
    game_path: &str,
    store: &str,
    outcome: &mut UndoOutcome,
) -> io::Result<()> {
    let target = join_rel(&ctx.game.root, game_path);
    let slot = join_rel(ctx.state_dir, store);
    // Half a cross-volume copy into the store.
    fsops::remove_file_if_exists(&with_suffix(&slot, PART_SUFFIX))?;
    if !fsops::is_file(&slot) {
        // It never moved.
        return Ok(());
    }
    if !fsops::exists(&target) {
        fsops::move_file(&slot, &target)?;
        outcome.rolled_back += 1;
    } else if fsops::same_content(&target, &slot) {
        fsops::remove_file_if_exists(&slot)?;
        outcome.rolled_back += 1;
    } else {
        outcome.parked.push(ParkedBackup {
            store: store.to_string(),
            game_path: game_path.to_string(),
            owner: BelowOwner::Unowned,
            parked_at: now(),
            reason: format!(
                "Kept aside when putting a file back at {game_path} was rolled back: the path was not free again, so this file could not return without overwriting what sits there."
            ),
        });
        outcome.kept += 1;
    }
    Ok(())
}

fn undo_reorder(root: &Path, steps: &[&IntentStep], phase: u8, outcome: &mut UndoOutcome) {
    let renames: Vec<(&str, &str, &str)> = steps
        .iter()
        .filter_map(|step| match step {
            IntentStep::Rename { from, temp, to } => {
                Some((from.as_str(), temp.as_str(), to.as_str()))
            }
            _ => None,
        })
        .collect();
    if phase >= 1 {
        for (_, temp, to) in renames.iter().rev() {
            let temp_path = join_rel(root, temp);
            let to_path = join_rel(root, to);
            if !fsops::exists(&temp_path) && fsops::exists(&to_path) {
                if let Err(err) = fsops::rename(&to_path, &temp_path) {
                    outcome
                        .errors
                        .push(format!("could not rename {to} back to {temp}: {err}"));
                }
            }
        }
    }
    for (from, temp, _) in renames.iter().rev() {
        let temp_path = join_rel(root, temp);
        let from_path = join_rel(root, from);
        if fsops::exists(&temp_path) && !fsops::exists(&from_path) {
            match fsops::rename(&temp_path, &from_path) {
                Ok(()) => outcome.rolled_back += 1,
                Err(err) => outcome
                    .errors
                    .push(format!("could not restore the pak name {from}: {err}")),
            }
        }
    }
}

/// Delete what committed operations left in `journal.cleanup` (their trash
/// and superseded backups), and the folders the Riftstone created that this
/// empties — never a folder it did not create. Failures stay listed for the
/// next try.
fn process_cleanup(ctx: &DeployContext, journal: &mut Journal) {
    if journal.cleanup.is_empty() {
        return;
    }
    let root = &ctx.game.root;
    let backups = ctx.state_dir.join("backups");
    let mut remaining = Vec::new();
    for item in std::mem::take(&mut journal.cleanup) {
        let path = match &item {
            Leftover::Game { path } if safe_rel(path) && path.contains(TRASH_MARK) => {
                join_rel(root, path)
            }
            Leftover::Store { path } if safe_store(path) => join_rel(ctx.state_dir, path),
            other => {
                log::warn!("ignored a cleanup entry that is not the Riftstone's own: {other:?}");
                continue;
            }
        };
        match fsops::remove_file_if_exists(&path) {
            Ok(_) => match &item {
                Leftover::Game { path: rel } => {
                    fsops::prune_created(root, parent(rel), &mut journal.created_dirs);
                }
                Leftover::Store { .. } => fsops::prune_empty_dirs(path.parent(), &backups),
            },
            Err(err) => {
                log::warn!("could not delete {}: {err}; will retry", path.display());
                remaining.push(item);
            }
        }
    }
    journal.cleanup = remaining;
    if let Err(err) = journal.save(ctx.state_dir) {
        log::warn!("could not save the journal after cleaning up: {err}");
    }
}

// ---------------------------------------------------------------------------
// Shared
// ---------------------------------------------------------------------------

/// Is `rel` a normalized relative path that stays inside its root — the only
/// shape the engine ever records? A journal entry of any other shape (a
/// `..`, an absolute path, a stray separator) is never acted on.
fn safe_rel(rel: &str) -> bool {
    !rel.is_empty() && normalize_entry_path(rel).is_ok_and(|normalized| normalized == rel)
}

/// A path in the backup store: safe, and under `backups/`.
fn safe_store(rel: &str) -> bool {
    safe_rel(rel) && lower_key(rel).starts_with("backups/")
}

/// The game path a step acts on.
fn step_game_path(step: &IntentStep) -> &str {
    match step {
        IntentStep::Place { game_path, .. }
        | IntentStep::Remove { game_path, .. }
        | IntentStep::Restore { game_path, .. }
        | IntentStep::Park { game_path, .. } => game_path,
        IntentStep::Rename { from, .. } => from,
    }
}

fn step_is_safe(step: &IntentStep) -> bool {
    match step {
        IntentStep::Place {
            game_path, backup, ..
        } => safe_rel(game_path) && backup.as_deref().is_none_or(safe_store),
        IntentStep::Remove { game_path, trash } => safe_rel(game_path) && safe_rel(trash),
        IntentStep::Restore {
            game_path,
            trash,
            backup,
        } => safe_rel(game_path) && trash.as_deref().is_none_or(safe_rel) && safe_store(backup),
        IntentStep::Rename { from, temp, to } => safe_rel(from) && safe_rel(temp) && safe_rel(to),
        IntentStep::Park { game_path, store } => safe_rel(game_path) && safe_store(store),
    }
}

fn common_refusal(ctx: &DeployContext, journal: &Journal) -> Option<String> {
    if journal.pending.is_some() {
        return Some(
            "An earlier change to the game folder was interrupted and has not been rolled back yet. Restart the Riftstone so it can put things back, then try again.".to_string(),
        );
    }
    if !journal.belongs_to(&ctx.game.root) {
        return Some(format!(
            "This journal belongs to the game folder at {}, not {}. Reopen the Riftstone so it loads the right one.",
            journal.game_root.display(),
            ctx.game.root.display()
        ));
    }
    None
}

/// A sequence number no install and no backup folder uses yet.
fn allocate_seq(journal: &Journal, state_dir: &Path) -> u64 {
    let used: HashSet<u64> = journal.installs.iter().map(|i| i.seq).collect();
    let mut seq = journal.next_seq.max(1);
    while used.contains(&seq) || state_dir.join("backups").join(seq.to_string()).exists() {
        seq += 1;
    }
    seq
}

fn with_entry(err: RiftError, entry: &str) -> String {
    match err {
        RiftError::Refused { reason } => format!("{reason} (Package file: {entry}.)"),
        other => other.to_string(),
    }
}

/// The sentence inside a refusal (other errors as they print).
fn reason_of(err: RiftError) -> String {
    match err {
        RiftError::Refused { reason } => reason,
        other => other.to_string(),
    }
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(windows)]
mod process_list {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    /// Is a process with this executable name (any case) running?
    pub(super) fn is_running(exe: &str) -> bool {
        // SAFETY: a ToolHelp snapshot is walked with a correctly sized,
        // zero-initialised PROCESSENTRY32W and closed before returning; no
        // pointer outlives the call.
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snapshot == INVALID_HANDLE_VALUE {
                return false;
            }
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut found = false;
            let mut more = Process32FirstW(snapshot, &mut entry) != 0;
            while more {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                if String::from_utf16_lossy(&entry.szExeFile[..len]).eq_ignore_ascii_case(exe) {
                    found = true;
                    break;
                }
                more = Process32NextW(snapshot, &mut entry) != 0;
            }
            CloseHandle(snapshot);
            found
        }
    }
}
