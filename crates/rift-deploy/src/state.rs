//! What is in the game folder right now: the journal's installs, every pak
//! in `pak_mods/` with its owner and whether REFramework loaded it, Fluffy's
//! un-adopted installs, files nobody owns, and files the journal expects but
//! the disk no longer has. Read-only.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use chrono::{DateTime, Local, NaiveDateTime};
use rift_core::dd2::{self, PAK_MODS_DIR};
use rift_core::path::file_name;
use rift_core::{lower_key, OptionId, PayloadKinds};
use serde::Serialize;

use crate::engine::{owner_label, DeployContext};
use crate::fsops::{self, child_ci, join_rel, CaseResolver};
use crate::game::GameInstall;
use crate::journal::{Below, DeployKind, Journal, Origin};
use crate::paks::{self, PAK_EXTENSION};
use crate::plan::OwnerKind;
use crate::reframework::{self, PakLoadReport, RefStatus};
use crate::stack::PathIndex;

/// Folders whose unowned files count as strays.
const STRAY_PLACES: &[&str] = &["natives", "reframework/autorun", "reframework/plugins"];

/// Deep enough for any real `natives/` tree, shallow enough to stay bounded.
const WALK_DEPTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameState {
    pub game: GameInstall,
    pub running: bool,
    /// The build id now differs from the one recorded at the newest install.
    pub build_changed: Option<BuildChange>,
    pub reframework: RefStatus,
    pub installs: Vec<InstallSummary>,
    /// Pak-bearing installs, lowest priority first.
    pub pak_order: Vec<OptionId>,
    /// Every `.pak` in `pak_mods/` (any case, any depth), in load order.
    pub paks: Vec<PakFile>,
    pub fluffy_unadopted: Vec<FluffyModState>,
    /// Files in `pak_mods/`, `natives/`, `reframework/autorun/` and
    /// `reframework/plugins/` that nobody owns. Reported, never removed.
    pub strays: Vec<StrayFile>,
    /// Files the journal says are installed that are gone.
    pub missing: Vec<MissingFile>,
    /// Files kept aside in the backup store because something else had
    /// changed the file at their path (see [`crate::restore_parked`] and
    /// [`crate::discard_parked`]).
    pub parked: Vec<ParkedFile>,
    pub pak_load: Option<PakLoadReport>,
    /// Fluffy renamed the game's crash reporter (`CrashReport_FMMBACKUP.exe`
    /// present, `CrashReport.exe` absent).
    pub crash_reporter_renamed: bool,
    /// `.pak` files REFramework would load from `pak_mods/`.
    pub pak_count: usize,
    pub pak_count_warning: usize,
    /// The journal holds an operation that was interrupted.
    pub interrupted: bool,
    /// Installed loose files that carry pre-TU 3.2 asset suffixes.
    pub outdated_files: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildChange {
    pub recorded: String,
    pub now: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallSummary {
    pub option_id: OptionId,
    pub name: String,
    pub package_file: String,
    pub version: Option<String>,
    pub installed_at: String,
    pub game_build: Option<String>,
    pub origin: Origin,
    pub kinds: PayloadKinds,
    pub file_count: usize,
    /// Its paks, in load order.
    pub paks: Vec<PakView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PakView {
    pub file_name: String,
    pub size: u64,
    pub load: PakLoadState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PakFile {
    pub file_name: String,
    pub game_path: String,
    pub size: u64,
    pub owner: PakOwner,
    pub load: PakLoadState,
    /// Sits in a subfolder of `pak_mods/` (REFramework still loads it, in an
    /// order nobody chose).
    pub nested: bool,
    /// Ends in exactly `.pak` — REFramework skips `.PAK`.
    pub loads: bool,
    /// Its place in the order REFramework loads `pak_mods/` (0 first);
    /// `None` when it does not load.
    pub load_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PakOwner {
    #[serde(rename_all = "camelCase")]
    Riftstone {
        option_id: OptionId,
        name: String,
        adopted: bool,
    },
    #[serde(rename_all = "camelCase")]
    Fluffy {
        section: String,
        name: String,
    },
    Unowned,
}

/// What REFramework's log says about one pak.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PakLoadState {
    /// Injected at the last launch.
    #[serde(rename_all = "camelCase")]
    Loaded { slot: u32, native_name: String },
    /// Placed (or renamed) after the last launch began: loads next time.
    Pending,
    /// Placed before the last launch, yet REFramework never injected it.
    NotLoaded,
    /// No REFramework log to read.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StrayFile {
    pub game_path: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingFile {
    pub option_id: OptionId,
    pub option_name: String,
    pub game_path: String,
    /// The bytes that are gone sat in another install's backup slot.
    pub in_backup: bool,
}

/// One file kept aside in the backup store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParkedFile {
    /// Its place in the backup store (relative to the app data dir) — the
    /// handle [`crate::restore_parked`] and [`crate::discard_parked`] take.
    pub store: String,
    /// Where it belongs in the game folder.
    pub game_path: String,
    /// Whose bytes it holds, for the window: "the file that was there
    /// before", a Fluffy mod's name, or an install's name.
    pub owner: String,
    pub owner_kind: OwnerKind,
    /// Its size in the store; `None` when the kept file is gone from it.
    pub size: Option<u64>,
    pub parked_at: String,
    pub reason: String,
    /// Something sits at `game_path` now: putting the kept file back swaps
    /// it out (or is refused while an install or Fluffy owns it).
    pub occupied: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FluffyModState {
    pub section: String,
    pub name: String,
    pub file_count: usize,
    /// How many of its files are still on disk.
    pub present_files: usize,
}

/// Read what is in the game right now. Never writes, never fails: what
/// cannot be read is simply not reported.
pub fn read_game_state(ctx: &DeployContext, journal: &Journal) -> GameState {
    let root = &ctx.game.root;
    let mut resolver = CaseResolver::new(root);
    let index = PathIndex::build(journal);
    let pak_load = reframework::read_pak_load_report(root);
    let log_started = pak_load
        .as_ref()
        .and_then(|r| r.log_started.as_deref())
        .and_then(parse_log_time);

    // Who owns which path: any journal layer, else Fluffy (un-adopted).
    let adopted = journal.adopted_sections();
    let mut fluffy_owner: HashMap<String, (&str, &str)> = HashMap::new();
    if let Some(ledger) = ctx.fluffy {
        for fluffy_mod in ledger.unadopted(&adopted) {
            for file in &fluffy_mod.files {
                fluffy_owner.insert(
                    lower_key(file),
                    (fluffy_mod.section.as_str(), fluffy_mod.name.as_str()),
                );
            }
        }
    }
    let owned = |key: &str| index.claimed(key) || fluffy_owner.contains_key(key);

    // pak_mods/
    let pak_dir = resolver.resolve(PAK_MODS_DIR);
    let mut paks_found: Vec<PakFile> = Vec::new();
    let mut strays: Vec<StrayFile> = Vec::new();
    for (game_path, size, depth) in walk_files(root, &pak_dir) {
        let key = lower_key(&game_path);
        let name = file_name(&game_path).to_string();
        if !lower_key(&name).ends_with(PAK_EXTENSION) {
            if !owned(&key) {
                strays.push(StrayFile { game_path, size });
            }
            continue;
        }
        let owner = if let Some(top) = index.top(journal, &key) {
            let install = top.install(journal);
            PakOwner::Riftstone {
                option_id: install.option_id.clone(),
                name: install.name.clone(),
                adopted: install.is_adopted(),
            }
        } else if let Some((section, name)) = fluffy_owner.get(&key) {
            PakOwner::Fluffy {
                section: (*section).to_string(),
                name: (*name).to_string(),
            }
        } else {
            strays.push(StrayFile {
                game_path: game_path.clone(),
                size,
            });
            PakOwner::Unowned
        };
        let placed = match &owner {
            PakOwner::Riftstone { option_id, .. } => journal
                .find(option_id)
                .and_then(|i| parse_rfc3339_local(&i.installed_at)),
            _ => fs::metadata(join_rel(root, &game_path))
                .and_then(|m| m.modified())
                .ok()
                .map(|t| DateTime::<Local>::from(t).naive_local()),
        };
        paks_found.push(PakFile {
            load: load_state(pak_load.as_ref(), &name, placed, log_started),
            loads: name.ends_with(PAK_EXTENSION),
            nested: depth > 1,
            file_name: name,
            game_path,
            size,
            owner,
            load_index: None,
        });
    }
    paks_found.sort_by_cached_key(|p| paks::ntfs_order_key(&p.game_path));
    let mut next_index = 0usize;
    for pak in &mut paks_found {
        if pak.loads {
            pak.load_index = Some(next_index);
            next_index += 1;
        }
    }

    // natives/ and REFramework's script and plugin folders.
    for place in STRAY_PLACES {
        let dir = resolver.resolve(place);
        for (game_path, size, _) in walk_files(root, &dir) {
            if !owned(&lower_key(&game_path)) {
                strays.push(StrayFile { game_path, size });
            }
        }
    }
    strays.sort_by(|a, b| a.game_path.cmp(&b.game_path));

    // Files the journal expects.
    let mut missing = Vec::new();
    for install in &journal.installs {
        for file in &install.files {
            let key = lower_key(&file.game_path);
            let gone = match index.above(journal, &key, &install.option_id) {
                // Someone sits on top: these bytes live in its backup slot.
                Some(above) => match &above.file(journal).below {
                    Below::Backup { store, .. } => !fsops::is_file(&join_rel(ctx.state_dir, store)),
                    _ => false,
                },
                None => !fsops::is_file(&join_rel(root, &resolver.resolve(&file.game_path))),
            };
            if gone {
                missing.push(MissingFile {
                    option_id: install.option_id.clone(),
                    option_name: install.name.clone(),
                    game_path: file.game_path.clone(),
                    in_backup: index.above(journal, &key, &install.option_id).is_some(),
                });
            }
        }
    }

    // Files kept aside.
    let parked = journal
        .parked
        .iter()
        .map(|kept| {
            let (owner, owner_kind) = owner_label(journal, ctx.fluffy, &kept.owner);
            let in_store = |rel: &str| {
                rift_core::normalize_entry_path(rel).is_ok_and(|n| n == rel)
                    && lower_key(rel).starts_with("backups/")
            };
            ParkedFile {
                store: kept.store.clone(),
                game_path: kept.game_path.clone(),
                owner,
                owner_kind,
                size: in_store(&kept.store)
                    .then(|| fsops::file_len(&join_rel(ctx.state_dir, &kept.store)))
                    .flatten(),
                parked_at: kept.parked_at.clone(),
                reason: kept.reason.clone(),
                occupied: fsops::exists(&join_rel(root, &resolver.resolve(&kept.game_path))),
            }
        })
        .collect();

    let installs = journal
        .installs
        .iter()
        .map(|install| {
            let mut own_paks: Vec<&PakFile> = paks_found
                .iter()
                .filter(|p| matches!(&p.owner, PakOwner::Riftstone { option_id, .. } if option_id == &install.option_id))
                .collect();
            own_paks.sort_by_key(|p| p.load_index);
            InstallSummary {
                option_id: install.option_id.clone(),
                name: install.name.clone(),
                package_file: install.package_file.clone(),
                version: install.version.clone(),
                installed_at: install.installed_at.clone(),
                game_build: install.game_build.clone(),
                origin: install.origin.clone(),
                kinds: install.kinds(),
                file_count: install.files.len(),
                paks: own_paks
                    .into_iter()
                    .map(|p| PakView {
                        file_name: p.file_name.clone(),
                        size: p.size,
                        load: p.load.clone(),
                    })
                    .collect(),
            }
        })
        .collect();

    let fluffy_unadopted = ctx
        .fluffy
        .map(|ledger| {
            ledger
                .unadopted(&adopted)
                .map(|m| FluffyModState {
                    section: m.section.clone(),
                    name: m.name.clone(),
                    file_count: m.files.len(),
                    present_files: m
                        .files
                        .iter()
                        .filter(|f| fsops::is_file(&join_rel(root, &resolver.resolve(f))))
                        .count(),
                })
                .collect()
        })
        .unwrap_or_default();

    let build_changed = match (journal.newest_game_build(), ctx.game.build_id.as_deref()) {
        (Some(recorded), Some(now)) if recorded != now => Some(BuildChange {
            recorded: recorded.to_string(),
            now: now.to_string(),
        }),
        _ => None,
    };
    let crash_reporter_renamed = child_ci(root, "CrashReport_FMMBACKUP.exe").is_some()
        && child_ci(root, "CrashReport.exe").is_none();
    let outdated_files = journal
        .installs
        .iter()
        .flat_map(|i| &i.files)
        .filter(|f| f.kind == DeployKind::Loose && dd2::outdated_suffix(&f.game_path).is_some())
        .count();

    GameState {
        game: ctx.game.clone(),
        running: ctx.game_running,
        build_changed,
        reframework: ctx.ref_status.clone(),
        installs,
        pak_order: journal.pak_order.clone(),
        pak_count: paks_found.iter().filter(|p| p.loads).count(),
        paks: paks_found,
        fluffy_unadopted,
        strays,
        missing,
        parked,
        pak_load,
        crash_reporter_renamed,
        pak_count_warning: dd2::PAK_COUNT_WARNING,
        interrupted: journal.pending.is_some(),
        outdated_files,
    }
}

/// Whether REFramework loaded a pak at the last launch, judged from its log.
fn load_state(
    report: Option<&PakLoadReport>,
    file_name: &str,
    placed: Option<NaiveDateTime>,
    log_started: Option<NaiveDateTime>,
) -> PakLoadState {
    let Some(report) = report else {
        return PakLoadState::Unknown;
    };
    if let Some(injected) = report
        .injected
        .iter()
        .find(|i| i.file_name.eq_ignore_ascii_case(file_name))
    {
        return PakLoadState::Loaded {
            slot: injected.slot,
            native_name: injected.native_name.clone(),
        };
    }
    if let (Some(placed), Some(started)) = (placed, log_started) {
        if placed > started {
            return PakLoadState::Pending;
        }
    }
    // Loaded at the last launch under its old name: a reorder since.
    let stem = lower_key(&paks::pak_stem(file_name));
    if report
        .injected
        .iter()
        .any(|i| lower_key(&paks::pak_stem(&i.file_name)) == stem)
    {
        return PakLoadState::Pending;
    }
    PakLoadState::NotLoaded
}

/// Files under `root/rel_dir` (no links followed): (game path, size, depth
/// below `rel_dir`, 1 = directly inside).
fn walk_files(root: &Path, rel_dir: &str) -> Vec<(String, u64, usize)> {
    let base = join_rel(root, rel_dir);
    if !base.is_dir() {
        return Vec::new();
    }
    walkdir::WalkDir::new(&base)
        .follow_links(false)
        .max_depth(WALK_DEPTH)
        .into_iter()
        .flatten()
        .filter(|entry| entry.file_type().is_file())
        .filter_map(|entry| {
            let relative = entry.path().strip_prefix(root).ok()?;
            let game_path = relative
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            Some((game_path, size, entry.depth()))
        })
        .collect()
}

/// REFramework's log time (`2026-10-14 21:34:14.970`, local time).
fn parse_log_time(stamp: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(stamp, "%Y-%m-%d %H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(stamp, "%Y-%m-%d %H:%M:%S"))
        .ok()
}

/// A journal timestamp (RFC 3339) in local time, comparable with the log's.
fn parse_rfc3339_local(stamp: &str) -> Option<NaiveDateTime> {
    DateTime::parse_from_rfc3339(stamp)
        .ok()
        .map(|t| t.with_timezone(&Local).naive_local())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reframework::InjectedPak;

    fn report() -> PakLoadReport {
        PakLoadReport {
            log_started: Some("2026-10-14 21:34:14.970".into()),
            cached: vec![],
            injected: vec![InjectedPak {
                file_name: "zrift-0000-Lantern Glow.pak".into(),
                native_name: "re_chunk_000.pak.sub_000.pak.patch_001.pak".into(),
                slot: 103,
            }],
            error: None,
        }
    }

    #[test]
    fn load_states_follow_the_log() {
        let started = parse_log_time("2026-10-14 21:34:14.970");
        let before = parse_log_time("2026-10-14 20:00:00");
        let after = parse_log_time("2026-10-14 22:00:00.5");
        assert!(matches!(
            load_state(
                Some(&report()),
                "ZRIFT-0000-lantern glow.pak",
                before,
                started
            ),
            PakLoadState::Loaded { slot: 103, .. }
        ));
        assert_eq!(
            load_state(
                Some(&report()),
                "zrift-0001-Vernworth Cloaks.pak",
                after,
                started
            ),
            PakLoadState::Pending
        );
        assert_eq!(
            load_state(
                Some(&report()),
                "zrift-0001-Vernworth Cloaks.pak",
                before,
                started
            ),
            PakLoadState::NotLoaded
        );
        // Renamed by a reorder since the launch: loads next time.
        assert_eq!(
            load_state(
                Some(&report()),
                "zrift-0003-Lantern Glow.pak",
                before,
                started
            ),
            PakLoadState::Pending
        );
        assert_eq!(
            load_state(None, "x.pak", before, started),
            PakLoadState::Unknown
        );
    }

    #[test]
    fn journal_times_compare_with_log_times() {
        use chrono::TimeZone;
        let local = parse_rfc3339_local("2026-10-14T19:00:00Z").unwrap();
        let utc = DateTime::parse_from_rfc3339("2026-10-14T19:00:00Z")
            .unwrap()
            .naive_utc();
        let offset = Local.offset_from_utc_datetime(&utc).local_minus_utc();
        assert_eq!((local - utc).num_seconds(), i64::from(offset));
    }
}
