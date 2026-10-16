//! The home frame: `get_overview`, `rescan`, the settings, the troubled list.

use rift_core::{Package, ProblemKind, RiftError};
use rift_deploy::Journal;

use crate::commands::library::package_summary;
use crate::commands::{blocking, Spine};
use crate::dto::{
    self, path_text, BuildChanged, Check, CheckAction, CheckLevel, FluffySummary, GameSummary,
    LibraryRoot, LibraryStats, Overview, PackageSummary, ReframeworkSummary, Settings,
};
use crate::library::Shelf;
use crate::nexus::NexusStore;
use crate::state::{lock, read, write, AppState, JournalSlot, Snapshot};

/// The four trouble kinds, in the order that ranks the troubled list
/// (docs/WINDOW-CONTRACT.md `get_troubled_packages`; the window's
/// `TROUBLE_KINDS` in `src/services/voice.ts` speaks for the first a package
/// carries, in this same order).
pub const TROUBLE: [ProblemKind; 4] = [
    ProblemKind::Unreadable,
    ProblemKind::NothingToInstall,
    ProblemKind::OutdatedFormats,
    ProblemKind::FilesSkipped,
];

pub fn get_overview_impl(state: &AppState) -> Result<Overview, RiftError> {
    let snapshot = state.refresh_snapshot();
    let shelf = state.shelf();
    let (game, fluffy, installed_options, journal_problem) = {
        let world = state.world();
        let game = world.game.as_ref().map(|game| GameSummary {
            root: path_text(&game.root),
            build_id: game.build_id.clone(),
            source: game.source.into(),
            running: snapshot.running,
            build_changed: snapshot
                .game_state
                .as_ref()
                .and_then(|s| s.build_changed.as_ref())
                .map(|change| BuildChanged {
                    recorded: change.recorded.clone(),
                    now: change.now.clone(),
                }),
        });
        let adopted = world
            .journal
            .ready()
            .map(Journal::adopted_sections)
            .unwrap_or_default();
        let fluffy = world.fluffy.as_ref().map(|fluffy| FluffySummary {
            root: path_text(&fluffy.root),
            mods_dir: path_text(&fluffy.mods_dir),
            installed_count: snapshot.ledger.as_ref().map_or(0, |l| l.mods.len()),
            unadopted_count: snapshot
                .ledger
                .as_ref()
                .map_or(0, |l| l.unadopted(&adopted).count()),
        });
        let installed = world.journal.ready().map_or(0, |j| j.installs.len());
        let problem = match &world.journal {
            JournalSlot::Damaged(err) => Some(err.to_string()),
            _ => None,
        };
        (game, fluffy, installed, problem)
    };
    let nexus = crate::commands::nexus::summary(state, &shelf);
    let library = LibraryStats {
        packages: shelf.packages().len(),
        options: shelf.option_count(),
        entries: shelf.entry_count(),
        installed_options,
        updates_available: nexus.updates,
        conflicts: snapshot.conflicts.len(),
        last_scan_ms: *lock(&state.last_scan_ms),
    };
    let checks = checks(
        state,
        &snapshot,
        &shelf,
        game.as_ref(),
        journal_problem.as_deref(),
    );
    Ok(Overview {
        game,
        reframework: snapshot.ref_status.as_ref().map(ReframeworkSummary::from),
        fluffy,
        roots: state
            .roots_snapshot()
            .iter()
            .map(LibraryRoot::from)
            .collect(),
        library,
        scan: state.scan_state(),
        checks,
        nexus,
        recovered: lock(&state.recovered).clone(),
    })
}

/// rift-deploy's preflight plus the spine's own: the game, the journal, the
/// roots, the packages that would not open.
fn checks(
    state: &AppState,
    snapshot: &Snapshot,
    shelf: &Shelf,
    game: Option<&GameSummary>,
    journal_problem: Option<&str>,
) -> Vec<Check> {
    let mut checks = Vec::new();
    let mut add = |id: &str,
                   level: CheckLevel,
                   title: String,
                   detail: String,
                   action: Option<CheckAction>| {
        checks.push(Check {
            id: id.to_string(),
            level,
            title,
            detail,
            action,
        });
    };
    match game {
        None => add(
            "game",
            CheckLevel::Error,
            "Dragon's Dogma 2 was not found".into(),
            "Steam's library folders do not list it where the Riftstone looked. Point the Riftstone at the game folder (the one holding DD2.exe).".into(),
            Some(CheckAction::PickGameFolder),
        ),
        Some(game) => add(
            "game",
            CheckLevel::Ok,
            "Dragon's Dogma 2 found".into(),
            match &game.build_id {
                Some(build) => format!("Steam build {build}, at {}.", game.root),
                None => format!("At {}.", game.root),
            },
            None,
        ),
    }
    if let Some(problem) = journal_problem {
        add(
            "journal",
            CheckLevel::Error,
            "The install journal could not be read".into(),
            format!("{problem}. Installing and uninstalling wait until it is readable again; nothing in the game folder was touched."),
            None,
        );
    }
    let held = state.held_files();
    for file in &held {
        let (id, title, what) = match file.what {
            "roots" => (
                "rootsUnreadable",
                "Your mod folders could not be read",
                "the folders the Riftstone reads mods from",
            ),
            "nexus" => (
                "nexusUnreadable",
                "Your Nexus links could not be read",
                "which download is which Nexus mod",
            ),
            _ => (
                "settingsUnreadable",
                "Your settings could not be read",
                "your settings and the game folder you picked",
            ),
        };
        add(
            id,
            CheckLevel::Warn,
            title.into(),
            format!(
                "{} holds {what}, but it could not be opened ({}). The Riftstone started without it and leaves the file exactly as it is: changes made now are not saved. Check the file's permissions (or close the program holding it), then restart the Riftstone.",
                file.path.display(),
                file.why
            ),
            None,
        );
    }
    if state.roots_snapshot().is_empty() && !held.iter().any(|file| file.what == "roots") {
        add(
            "roots",
            CheckLevel::Info,
            "No mod folders yet".into(),
            "Add the folder your downloads live in. The Riftstone reads it and never writes to it."
                .into(),
            None,
        );
    }
    let count = |kind: ProblemKind| {
        shelf
            .packages()
            .iter()
            .filter(|p| p.problems.iter().any(|x| x.kind == kind))
            .count()
    };
    let unreadable = count(ProblemKind::Unreadable);
    if unreadable > 0 {
        add(
            "unreadable",
            CheckLevel::Warn,
            format!("{unreadable} download(s) could not be opened"),
            "They are listed in the Rift with the reason. A damaged or half-finished download usually needs downloading again.".into(),
            Some(CheckAction::Rescan),
        );
    }
    let empty = count(ProblemKind::NothingToInstall);
    if empty > 0 {
        add(
            "nothingToInstall",
            CheckLevel::Info,
            format!("{empty} download(s) hold nothing the game loads"),
            "They opened fine, but no file in them goes into natives/, reframework/, pak_mods/ or beside the game.".into(),
            None,
        );
    }
    if let Some(game_state) = &snapshot.game_state {
        // Under the ids the window knows; the faulty-file list is drawn from
        // `reframework.faultyFiles`, so its check would be a second row.
        checks.extend(
            rift_deploy::preflight(game_state)
                .iter()
                .filter(|check| !dto::SHOWN_ELSEWHERE.contains(&check.id.as_str()))
                .map(Check::from),
        );
    }
    checks.sort_by_key(|check| std::cmp::Reverse(check.level));
    checks
}

pub fn get_settings_impl(state: &AppState) -> Settings {
    read(&state.settings).window()
}

pub fn set_settings_impl(state: &AppState, settings: Settings) -> Settings {
    let window = {
        let mut stored = write(&state.settings);
        stored.apply(settings);
        stored.window()
    };
    state.save_settings();
    window
}

/// Where `package` ranks on the troubled list: the first of [`TROUBLE`] it
/// carries; `None` when it carries none.
pub fn trouble_rank(package: &Package) -> Option<usize> {
    TROUBLE
        .iter()
        .position(|kind| package.problems.iter().any(|p| p.kind == *kind))
}

/// The troubled list: every package carrying a trouble kind (those with zero
/// options too), by the first it carries, then library order. Its problems
/// are the catalog's, in the catalog's order: the card picks the one it
/// speaks for itself.
pub fn troubled(shelf: &Shelf, store: &NexusStore) -> Vec<PackageSummary> {
    let mut troubled: Vec<(usize, PackageSummary)> = shelf
        .packages()
        .iter()
        .filter_map(|package| Some((trouble_rank(package)?, package_summary(package, store))))
        .collect();
    troubled.sort_by_key(|(rank, _)| *rank);
    troubled.into_iter().map(|(_, summary)| summary).collect()
}

pub fn get_troubled_packages_impl(state: &AppState) -> Vec<PackageSummary> {
    let shelf = state.shelf();
    let store = lock(&state.nexus);
    troubled(&shelf, &store)
}

// -- wrappers ----------------------------------------------------------------

#[tauri::command]
pub async fn get_overview(state: Spine<'_>) -> Result<Overview, RiftError> {
    blocking(&state, |state| get_overview_impl(state)).await
}

#[tauri::command]
pub async fn rescan(state: Spine<'_>) -> Result<(), RiftError> {
    blocking(&state, |state| {
        state.start_scan();
        Ok(())
    })
    .await
}

#[tauri::command]
pub async fn get_settings(state: Spine<'_>) -> Result<Settings, RiftError> {
    blocking(&state, |state| Ok(get_settings_impl(state))).await
}

#[tauri::command]
pub async fn set_settings(state: Spine<'_>, settings: Settings) -> Result<Settings, RiftError> {
    blocking(&state, move |state| Ok(set_settings_impl(state, settings))).await
}

#[tauri::command]
pub async fn get_troubled_packages(state: Spine<'_>) -> Result<Vec<PackageSummary>, RiftError> {
    blocking(&state, |state| Ok(get_troubled_packages_impl(state))).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use rift_catalog::catalog::CachedPackage;
    use rift_catalog::CatalogCache;
    use rift_core::{
        DeployTarget, LibraryRoot as CoreRoot, ModOption, OptionId, PackageFormat, PackageId,
        PayloadFile, PayloadKinds, Problem, RootKind,
    };
    use std::path::PathBuf;

    fn option(package: &PackageId, root: &str, game_paths: &[&str]) -> ModOption {
        ModOption {
            id: OptionId::new(package, root),
            package_id: package.clone(),
            root: root.into(),
            name: root.into(),
            info: None,
            screenshot: None,
            payload: game_paths
                .iter()
                .map(|path| PayloadFile {
                    entry: format!("{root}/{path}"),
                    target: DeployTarget::Loose {
                        game_path: (*path).to_string(),
                    },
                    size: 1,
                })
                .collect(),
            skipped: Vec::new(),
            kinds: PayloadKinds::default(),
            parent: None,
            addon_for: None,
            bundle: None,
            menu_only: game_paths.is_empty(),
            trail: Vec::new(),
        }
    }

    fn package(file: &str, payload: &[&str], problems: &[ProblemKind]) -> Package {
        let id = PackageId::for_archive(file, 1);
        Package {
            id: id.clone(),
            root_id: "a".into(),
            path: PathBuf::from(format!("/library/{file}")),
            file_name: file.into(),
            display_name: file.trim_end_matches(".zip").into(),
            format: PackageFormat::Zip,
            size: 1,
            modified_ms: 1,
            entry_count: payload.len(),
            options: if payload.is_empty() {
                Vec::new()
            } else {
                vec![option(&id, "Mod", payload)]
            },
            nexus: None,
            problems: problems
                .iter()
                .map(|kind| Problem {
                    kind: *kind,
                    message: format!("{kind:?} in {file}"),
                })
                .collect(),
        }
    }

    // Built field by field: the cache's shape is rift-catalog's to grow.
    #[allow(clippy::field_reassign_with_default)]
    fn shelf(packages: Vec<Package>) -> Shelf {
        let mut cache = CatalogCache::default();
        cache.schema = CatalogCache::SCHEMA;
        cache.packages = packages
            .into_iter()
            .map(|package| CachedPackage {
                path: package.path.clone(),
                size: package.size,
                modified_ms: package.modified_ms,
                entries: Vec::new(),
                package,
            })
            .collect();
        let root = CoreRoot {
            id: "a".into(),
            path: PathBuf::from("/library"),
            label: "a".into(),
            kind: RootKind::Custom,
            dd2_shaped_only: false,
        };
        Shelf::from_cache(&[root], &cache)
    }

    /// The window's voice module, as it is built.
    const VOICE: &str = include_str!("../../../src/services/voice.ts");

    #[test]
    fn the_spine_ranks_troubles_in_the_order_the_window_speaks_for_them() {
        let start = VOICE
            .find("TROUBLE_KINDS: readonly ProblemKind[] = [")
            .expect("voice.ts declares TROUBLE_KINDS");
        let list = &VOICE[start..];
        let list = &list[list.find("= [").unwrap() + 3..list.find("];").unwrap()];
        let window: Vec<&str> = list
            .split(',')
            .map(|kind| kind.trim().trim_matches('"'))
            .filter(|kind| !kind.is_empty())
            .collect();
        let spine: Vec<String> = TROUBLE
            .iter()
            .map(|kind| {
                serde_json::to_value(dto::ProblemKind::from(*kind))
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_eq!(spine, window, "one order for the list and its cards");
    }

    #[test]
    fn every_package_with_a_trouble_kind_is_listed_by_the_first_it_carries() {
        let mesh = "natives/stm/cloak/ermine_cape.mesh.260421070";
        let shelf = shelf(vec![
            package("Skips.zip", &[mesh], &[ProblemKind::FilesSkipped]),
            package("Old.zip", &[mesh], &[ProblemKind::OutdatedFormats]),
            // Problems the list does not rank on: not troubled.
            package(
                "Garbled.zip",
                &[mesh],
                &[ProblemKind::ModinfoGarbled, ProblemKind::MissingParent],
            ),
            package(
                "Nothing.zip",
                &[],
                &[ProblemKind::FilesSkipped, ProblemKind::NothingToInstall],
            ),
            package("Broken.zip", &[], &[ProblemKind::Unreadable]),
            package("Also Skips.zip", &[mesh], &[ProblemKind::FilesSkipped]),
        ]);
        let list = troubled(&shelf, &NexusStore::new());
        let names: Vec<&str> = list.iter().map(|p| p.file_name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "Broken.zip",
                "Nothing.zip",
                "Old.zip",
                "Also Skips.zip",
                "Skips.zip"
            ],
            "by the first trouble kind carried, then library order"
        );
        let nothing: Vec<dto::ProblemKind> = list[1].problems.iter().map(|p| p.kind).collect();
        assert_eq!(
            nothing,
            vec![
                dto::ProblemKind::FilesSkipped,
                dto::ProblemKind::NothingToInstall
            ],
            "the problems stay in the catalog's order: the card picks its own lead"
        );
    }
}
