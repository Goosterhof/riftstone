//! The home frame: `get_overview`, `rescan`, the settings, the troubled list.

use std::path::Path;

use rift_core::{Package, ProblemKind, RiftError};
use rift_deploy::GameInstall;

use crate::commands::library::{mods_folder_view, package_summary};
use crate::commands::{blocking, Spine};
use crate::dto::{
    self, path_text, BuildChanged, Check, CheckAction, CheckLevel, GameSummary, LaunchRoute,
    LibraryStats, Overview, PackageSummary, ReframeworkSummary, Settings,
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

/// The game as the window shows it: where it is, its build, whether it runs
/// (the game watcher's word) and since when, and how Play starts it.
pub fn game_summary(state: &AppState, game: &GameInstall, snapshot: &Snapshot) -> GameSummary {
    let run = state.game_run();
    GameSummary {
        root: path_text(&game.root),
        build_id: game.build_id.clone(),
        source: game.source.into(),
        running: run.running,
        started_ms: run.started_ms,
        launch: LaunchRoute::from(&rift_deploy::launch_route(game)),
        build_changed: snapshot
            .game_state
            .as_ref()
            .and_then(|s| s.build_changed.as_ref())
            .map(|change| BuildChanged {
                recorded: change.recorded.clone(),
                now: change.now.clone(),
            }),
    }
}

pub fn get_overview_impl(state: &AppState) -> Result<Overview, RiftError> {
    let snapshot = state.refresh_snapshot();
    let shelf = state.shelf();
    let (game, installed_options, journal_problem) = {
        let world = state.world();
        let game = world
            .game
            .as_ref()
            .map(|game| game_summary(state, game, &snapshot));
        let installed = world.journal.ready().map_or(0, |j| j.installs.len());
        let problem = match &world.journal {
            JournalSlot::Damaged(err) => Some(err.to_string()),
            _ => None,
        };
        (game, installed, problem)
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
        mods: mods_folder_view(state),
        library,
        scan: state.scan_state(),
        checks,
        nexus,
        recovered: lock(&state.recovered).clone(),
    })
}

/// What the mods folder is like right now (the `mods` check).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModsFolderState {
    /// It holds at least one package.
    Holds,
    /// It holds no package (a `desktop.ini`, a download in flight and a
    /// hidden name are no packages).
    Empty,
    /// It is there, but its list of files could not be read: why.
    Unlistable(String),
    /// It is not there: why it could not be made.
    Missing(String),
}

/// Look at the mods folder (one listing, no archive opened). `unmade` is
/// why it could not be made at launch, when it could not.
pub fn mods_folder_state(folder: &Path, unmade: Option<&str>) -> ModsFolderState {
    if !folder.is_dir() {
        return ModsFolderState::Missing(
            unmade
                .unwrap_or(
                    "it is not there any more; the Riftstone makes it again at its next start",
                )
                .to_string(),
        );
    }
    if let Err(err) = std::fs::read_dir(folder) {
        return ModsFolderState::Unlistable(err.to_string());
    }
    if rift_catalog::scan::list_library(folder).is_empty() {
        ModsFolderState::Empty
    } else {
        ModsFolderState::Holds
    }
}

/// rift-deploy's preflight plus the spine's own: the game, the journal, the
/// mods folder, the installs found again after a move, the packages that
/// would not open, the player's files that could not be read.
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
    let folder = state.library_dir();
    match mods_folder_state(folder, state.mods_unmade()) {
        ModsFolderState::Holds => {}
        ModsFolderState::Empty => add(
            "mods",
            CheckLevel::Info,
            "The mods folder is empty".into(),
            format!(
                "Put each mod's zip, 7z or rar, or its unpacked folder, directly in {}. The Riftstone reads it, never changes what is there, and sees new mods by itself.",
                folder.display()
            ),
            Some(CheckAction::OpenModsFolder),
        ),
        ModsFolderState::Unlistable(why) => add(
            "mods",
            CheckLevel::Warn,
            "The mods folder could not be read".into(),
            format!(
                "{} is there, but its list of files could not be read ({why}). Check the folder's permissions, then rescan.",
                folder.display()
            ),
            Some(CheckAction::Rescan),
        ),
        ModsFolderState::Missing(why) => add(
            "mods",
            CheckLevel::Error,
            "The mods folder is missing".into(),
            format!(
                "The Riftstone keeps your mods in {}, and that folder could not be made ({why}). Make it by hand, or install the Riftstone where it may write, then start it again.",
                folder.display()
            ),
            None,
        ),
    }
    let relinked = state.relinked_count();
    if relinked > 0 {
        add(
            "relinked",
            CheckLevel::Info,
            format!("{relinked} install(s) found again in the mods folder"),
            "Their mods had moved. Each was matched to the same option in the mods folder by its files, so it shows, updates and uninstalls as before. Nothing in the game folder changed.".into(),
            None,
        );
    }
    let held = state.held_files();
    for file in &held {
        let (id, title, what) = match file.what {
            "nexus" => (
                "nexusUnreadable",
                "Your Nexus links could not be read",
                "which mod is which Nexus mod",
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
            format!("{unreadable} mod(s) in the mods folder could not be opened"),
            "They are listed in the Rift with the reason. A damaged or half-finished download usually needs downloading again.".into(),
            Some(CheckAction::Rescan),
        );
    }
    let empty = count(ProblemKind::NothingToInstall);
    if empty > 0 {
        add(
            "nothingToInstall",
            CheckLevel::Info,
            format!("{empty} mod(s) in the mods folder hold nothing the game loads"),
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
        DeployTarget, ModOption, OptionId, PackageFormat, PackageId, PayloadFile, PayloadKinds,
        Problem,
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
            path: PathBuf::from(format!("/mods/{file}")),
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
        Shelf::from_cache(Path::new("/mods"), &cache)
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

    #[test]
    fn the_mods_folder_is_told_apart_missing_empty_or_holding() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        assert!(matches!(
            mods_folder_state(&mods, Some("access is denied")),
            ModsFolderState::Missing(why) if why == "access is denied"
        ));
        assert!(matches!(
            mods_folder_state(&mods, None),
            ModsFolderState::Missing(why) if why.contains("not there")
        ));
        std::fs::create_dir_all(&mods).unwrap();
        assert_eq!(mods_folder_state(&mods, None), ModsFolderState::Empty);
        // Windows' folder settings, a hidden name and a browser's download in
        // flight are no packages.
        std::fs::write(mods.join("desktop.ini"), b"[.ShellClassInfo]").unwrap();
        std::fs::write(mods.join(".riftstone-a1.part"), b"half").unwrap();
        std::fs::write(mods.join("Lantern Glow Recolor.zip.crdownload"), b"PK").unwrap();
        assert_eq!(mods_folder_state(&mods, None), ModsFolderState::Empty);
        std::fs::write(mods.join("Lantern Glow Recolor.zip"), b"PK").unwrap();
        assert_eq!(mods_folder_state(&mods, None), ModsFolderState::Holds);
    }
}
