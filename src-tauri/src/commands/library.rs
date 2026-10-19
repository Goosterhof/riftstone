//! The library: search, one option, one package, a thumbnail, the library
//! tree (v0.3), and the mods folder itself.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use rift_catalog::Nest;
use rift_core::{lower_key, ModOption, OptionId, Package, PackageId, RiftError};

use crate::commands::{blocking, not_found, Spine};
use crate::conflicts;
use crate::dto::{
    self, path_text, ArchiveEntry, ConflictView, InstallBadge, InstallOrigin, InstalledInfo,
    LibraryPackage, LibraryView, ModsFolderView, OptionDetail, OptionRef, PackageDetail,
    PackageSummary, PayloadKinds, SearchQuery, SearchResult, ThumbnailSize, TreeNode, TreeNodeKind,
};
use crate::family::{self, Families};
use crate::nexus::NexusStore;
use crate::state::{lock, AppState};
use crate::thumbs;

/// How much of a description a search card shows.
const SNIPPET_CHARS: usize = 180;

// ---------------------------------------------------------------------------
// search
// ---------------------------------------------------------------------------

/// The window's search: `limit: 0` reaches the catalog as "no cap" — every
/// match (v0.3).
pub fn search_impl(state: &AppState, query: &SearchQuery) -> Vec<SearchResult> {
    let shelf = state.shelf();
    let snapshot = state.snapshot();
    let installs = state.journal_installs();
    let is_installed = |id: &OptionId| installs.contains_key(id);
    let hits = shelf.search(&query.into(), &is_installed);
    let store = lock(&state.nexus);
    let mut badges: HashMap<&PackageId, Option<dto::UpdateBadge>> = HashMap::new();
    // v0.5: an older version's answer goes when the lead's same option answers this search too
    // (WINDOW-CONTRACT § v0.5, One entry); one only the older version answers stays.
    let families = Families::of_shelf(&shelf, &store);
    let answered: HashSet<&OptionId> = hits.iter().map(|hit| &hit.option_id).collect();
    hits.iter()
        .filter_map(|hit| {
            let (package, option) = shelf.option(&hit.option_id)?;
            let lead = families
                .lead(&package.id)
                .filter(|lead| **lead != package.id)
                .and_then(|lead| shelf.package(lead));
            if lead
                .and_then(|lead| family::lead_match(option, package, lead))
                .is_some_and(|same| answered.contains(&same.id))
            {
                return None;
            }
            let update = *badges
                .entry(&package.id)
                .or_insert_with(|| store.badge(package));
            let info = option.info.as_ref();
            // The package's Nexus labels, as the index holds them (v0.2).
            let labels = hit.labels.as_ref();
            Some(SearchResult {
                option_id: option.id.0.clone(),
                package_id: package.id.0.clone(),
                score: hit.score,
                name: option.name.clone(),
                trail: option.trail.clone(),
                bundle: option.bundle.clone(),
                package_name: package.display_name.clone(),
                package_format: package.format.into(),
                author: info.and_then(|i| i.author.clone()),
                version: info.and_then(|i| i.version.clone()),
                category: info.and_then(|i| i.category.clone()),
                description_snippet: info
                    .and_then(|i| i.description.as_deref())
                    .and_then(snippet),
                has_screenshot: option.screenshot.is_some(),
                kinds: option.kinds.into(),
                menu_only: option.menu_only,
                matched: hit.matched.iter().map(dto::MatchedField::from).collect(),
                file_matches: hit.file_matches.clone(),
                installed: install_badge(&installs, &option.id),
                update,
                conflicted: snapshot.conflicted.contains(&option.id.0),
                nexus_category: labels.and_then(|l| l.category.clone()),
                nexus_tags: labels.map(|l| l.tags.clone()).unwrap_or_default(),
                adult: labels.is_some_and(|l| l.adult),
            })
        })
        .collect()
}

/// The badge of an option the journal holds: the Riftstone's own install, or
/// an adopted one.
fn install_badge(
    installs: &HashMap<OptionId, (bool, String)>,
    id: &OptionId,
) -> Option<InstallBadge> {
    let (adopted, installed_at) = installs.get(id)?;
    Some(InstallBadge {
        origin: if *adopted {
            InstallOrigin::Adopted
        } else {
            InstallOrigin::Riftstone
        },
        installed_at: Some(installed_at.clone()),
    })
}

/// The first ~180 characters of a description, whitespace collapsed.
pub fn snippet(text: &str) -> Option<String> {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.is_empty() {
        return None;
    }
    if flat.chars().count() <= SNIPPET_CHARS {
        return Some(flat);
    }
    let cut: String = flat.chars().take(SNIPPET_CHARS - 1).collect();
    Some(format!("{}…", cut.trim_end()))
}

// ---------------------------------------------------------------------------
// get_option / get_package
// ---------------------------------------------------------------------------

/// A package as the window lists it, with the link the spine knows now.
pub fn package_summary(package: &Package, store: &NexusStore) -> PackageSummary {
    PackageSummary {
        id: package.id.0.clone(),
        path: path_text(&package.path),
        file_name: package.file_name.clone(),
        display_name: package.display_name.clone(),
        format: package.format.into(),
        size: package.size,
        modified_ms: package.modified_ms,
        entry_count: package.entry_count,
        option_count: package.options.len(),
        nexus: store.link_for(package).as_ref().map(dto::NexusLink::from),
        problems: package.problems.iter().map(dto::Problem::from).collect(),
    }
}

pub fn get_option_impl(state: &AppState, option_id: &str) -> Result<OptionDetail, RiftError> {
    let id = OptionId(option_id.to_string());
    let shelf = state.shelf();
    let (package, option) = shelf.option(&id).ok_or_else(|| not_found(option_id))?;
    // The game folder as it stands since the last change (shared, read once).
    let snapshot = state.snapshot();
    let installs = state.journal_installs();
    let installed_ids: HashSet<&OptionId> = installs.keys().collect();

    let children = shelf
        .options()
        .filter(|(_, child)| child.parent.as_ref() == Some(&id))
        .map(|(_, child)| option_ref(child, &installed_ids))
        .collect();
    let parent_name = option
        .parent
        .as_ref()
        .and_then(|parent| shelf.option(parent))
        .map(|(_, parent)| parent.name.clone());

    // A copy of the world: the package is read below (a pak's head and table
    // of contents) with no lock held, so nothing waits on this preview.
    let world = state.world();
    let (installed, conflicts) = match (&world.game, world.journal.ready()) {
        (Some(game), Some(journal)) => {
            let installed = journal.find(&id).map(InstalledInfo::from);
            let conflicts = if installed.is_some() {
                involving(&snapshot.conflicts, &id)
            } else {
                candidate_conflicts(state, &snapshot, &game.root, journal, package, option)
            };
            (installed, conflicts)
        }
        _ => (None, Vec::new()),
    };

    let store = lock(&state.nexus);
    Ok(OptionDetail {
        option: option.into(),
        package: package_summary(package, &store),
        parent_name,
        children,
        installed,
        conflicts,
        update: store.link_for(package).map(|_| store.row(package)),
        // The labels search filters by: the index's, so the two never differ.
        labels: shelf.labels(&package.id).map(dto::NexusLabels::from),
    })
}

fn option_ref(option: &ModOption, installed: &HashSet<&OptionId>) -> OptionRef {
    OptionRef {
        option_id: option.id.0.clone(),
        name: option.name.clone(),
        menu_only: option.menu_only,
        has_screenshot: option.screenshot.is_some(),
        installed: installed.contains(&option.id),
    }
}

fn involving(conflicts: &[ConflictView], id: &OptionId) -> Vec<ConflictView> {
    conflicts
        .iter()
        .filter(|c| c.layers.iter().any(|l| l.option_id == id.0))
        .cloned()
        .collect()
}

/// Whom `option` would override (and be overridden by) if it were installed
/// now: the game's layers plus this option on top, its paks read from the
/// package (head + table of contents only).
fn candidate_conflicts(
    state: &AppState,
    snapshot: &crate::state::Snapshot,
    game_root: &Path,
    journal: &rift_deploy::Journal,
    package: &Package,
    option: &ModOption,
) -> Vec<ConflictView> {
    let Some(game_state) = &snapshot.game_state else {
        return Vec::new();
    };
    let mut layers = conflicts::game_layers(game_root, journal, game_state, &state.paks);
    conflicts::add_candidate(&mut layers, option, |entry| {
        state.paks.in_archive(&package.id, entry, || {
            state
                .sources
                .with(package, |source| {
                    Ok(conflicts::read_archive_pak_hashes(source, entry))
                })
                .unwrap_or_else(|err| Err(err.to_string()))
        })
    });
    involving(&conflicts::detect(&layers), &option.id)
}

pub fn get_package_impl(state: &AppState, package_id: &str) -> Result<PackageDetail, RiftError> {
    let shelf = state.shelf();
    let package = shelf
        .package(&PackageId(package_id.to_string()))
        .ok_or_else(|| not_found(package_id))?;
    let snapshot = state.snapshot();
    let installs = state.journal_installs();
    let store = lock(&state.nexus);
    Ok(PackageDetail {
        package: package_summary(package, &store),
        tree: package_tree(
            package,
            &|id| installs.contains_key(id),
            &snapshot.conflicted,
        ),
        entries: shelf
            .entries(&package.id)
            .iter()
            .map(ArchiveEntry::from)
            .collect(),
    })
}

/// The package's options as their menus nest them (rift-catalog
/// `tree::nest`: `addonfor` children under their parent in the same package,
/// `NameAsBundle` siblings gathered under a bundle node, an option whose
/// parent lives elsewhere a top node here), each node told whether it is
/// installed and whether it takes part in a conflict. A bundle node is
/// installed or conflicted when any child is, menu-only when every child is,
/// and counts its children's kinds.
pub fn package_tree(
    package: &Package,
    is_installed: &dyn Fn(&OptionId) -> bool,
    conflicted: &HashSet<String>,
) -> Vec<TreeNode> {
    let options: HashMap<&OptionId, &ModOption> =
        package.options.iter().map(|o| (&o.id, o)).collect();
    rift_catalog::tree::nest(package)
        .iter()
        .filter_map(|node| tree_node(node, &options, is_installed, conflicted))
        .collect()
}

fn tree_node(
    node: &Nest,
    options: &HashMap<&OptionId, &ModOption>,
    is_installed: &dyn Fn(&OptionId) -> bool,
    conflicted: &HashSet<String>,
) -> Option<TreeNode> {
    let kids = |children: &[Nest]| -> Vec<TreeNode> {
        children
            .iter()
            .filter_map(|child| tree_node(child, options, is_installed, conflicted))
            .collect()
    };
    match node {
        Nest::Option { id, children } => {
            let option = options.get(id)?;
            Some(TreeNode {
                option_id: Some(option.id.0.clone()),
                label: option.name.clone(),
                kind: TreeNodeKind::Option,
                menu_only: option.menu_only,
                has_screenshot: option.screenshot.is_some(),
                installed: is_installed(&option.id),
                conflicted: conflicted.contains(&option.id.0),
                kinds: option.kinds.into(),
                children: kids(children),
            })
        }
        Nest::Bundle { label, children } => {
            let children = kids(children);
            let mut kinds = PayloadKinds::default();
            for child in &children {
                kinds.pak += child.kinds.pak;
                kinds.loose += child.kinds.loose;
                kinds.reframework += child.kinds.reframework;
                kinds.root += child.kinds.root;
            }
            Some(TreeNode {
                option_id: None,
                label: label.clone(),
                kind: TreeNodeKind::Bundle,
                menu_only: children.iter().all(|child| child.menu_only),
                has_screenshot: children.iter().any(|child| child.has_screenshot),
                installed: children.iter().any(|child| child.installed),
                conflicted: children.iter().any(|child| child.conflicted),
                kinds,
                children,
            })
        }
    }
}

// ---------------------------------------------------------------------------
// get_library (v0.3): the Rift's idle view
// ---------------------------------------------------------------------------

/// The mods folder as the window shows it.
pub fn mods_folder_view(state: &AppState) -> ModsFolderView {
    let folder = state.library_dir();
    ModsFolderView {
        path: path_text(folder),
        exists: folder.is_dir(),
    }
}

/// Every package of the mods folder with its option tree and no entry list:
/// its counts, update badge, labels and problems, by display name
/// (case-blind), then file name. Troubled packages are there too, carrying
/// their problems; one that yielded no option has an empty tree.
pub fn get_library_impl(state: &AppState) -> LibraryView {
    let shelf = state.shelf();
    let snapshot = state.snapshot();
    let installs = state.journal_installs();
    let is_installed = |id: &OptionId| installs.contains_key(id);
    let store = lock(&state.nexus);
    let families = Families::of_shelf(&shelf, &store);
    let mut questions = family::version_questions(&shelf, &store, &families);
    let mut packages: Vec<LibraryPackage> = shelf
        .packages()
        .iter()
        .map(|package| {
            let labels = shelf.labels(&package.id);
            LibraryPackage {
                package: package_summary(package, &store),
                installable: package.options.iter().filter(|o| !o.menu_only).count(),
                installed: package
                    .options
                    .iter()
                    .filter(|o| is_installed(&o.id))
                    .count(),
                update: store.badge(package),
                labels: labels.map(dto::NexusLabels::from),
                adult: labels.is_some_and(|l| l.adult),
                conflicted: package
                    .options
                    .iter()
                    .any(|o| snapshot.conflicted.contains(&o.id.0)),
                family: families.view(&shelf, &package.id, &is_installed),
                version_question: questions.remove(&package.id),
                tree: package_tree(package, &is_installed, &snapshot.conflicted),
            }
        })
        .collect();
    packages.sort_by(|a, b| {
        lower_key(&a.package.display_name)
            .cmp(&lower_key(&b.package.display_name))
            .then_with(|| lower_key(&a.package.file_name).cmp(&lower_key(&b.package.file_name)))
            .then_with(|| a.package.file_name.cmp(&b.package.file_name))
    });
    LibraryView {
        mods: mods_folder_view(state),
        packages,
    }
}

/// Open the mods folder in Explorer, Rust-side (the opener scope refuses
/// folders from the webview). A folder that is missing is made first, then
/// watched and read: a folder made again is a new folder, which the watcher
/// of the one that went never sees, so the watcher is armed afresh and a
/// scan follows. The same happens whenever nothing watches the folder: it
/// was not there at launch, or a scan found it gone and let its watcher go
/// (the player may have put it back by hand since).
pub fn open_mods_folder_impl(state: &Arc<AppState>) -> Result<(), RiftError> {
    let folder = state.library_dir().to_path_buf();
    let made = !folder.is_dir();
    std::fs::create_dir_all(&folder).map_err(|err| {
        RiftError::io(
            format!("could not make the mods folder {}", folder.display()),
            err,
        )
    })?;
    if made || state.library_unwatched() {
        state.arm_watcher();
        state.request_scan();
    }
    state.config.launcher.show_folder(&folder)
}

// ---------------------------------------------------------------------------
// get_thumbnail
// ---------------------------------------------------------------------------

pub fn get_thumbnail_impl(
    state: &AppState,
    option_id: &str,
    size: ThumbnailSize,
) -> Result<Option<String>, RiftError> {
    let shelf = state.shelf();
    let (package, option) = shelf
        .option(&OptionId(option_id.to_string()))
        .ok_or_else(|| not_found(option_id))?;
    let Some(entry) = option.screenshot.as_deref() else {
        return Ok(None);
    };
    let width = thumbs::width_for(size);
    let name = thumbs::cache_name(&package.id, entry, width);
    let dir = state.config.dirs.thumbs_dir();
    if let Some(jpeg) = thumbs::cached(&dir, &name) {
        return Ok(Some(thumbs::data_url(&jpeg)));
    }
    // A picture that could not be shown before is not read again (until a
    // new download of the package brings a new id).
    let failure = thumbs::failure_name(&package.id, entry);
    if thumbs::failed_before(&dir, &failure) {
        return Ok(None);
    }
    let cannot_show = |why: &str| {
        log::warn!(
            "the picture {entry} in {} could not be shown: {why}",
            package.file_name
        );
        thumbs::remember_failure(&dir, &failure);
        Ok(None)
    };
    let listed = shelf
        .entries(&package.id)
        .iter()
        .find(|listed| lower_key(&listed.path) == lower_key(entry))
        .map(|listed| listed.size);
    if listed.is_some_and(|size| size > thumbs::MAX_IMAGE_BYTES) {
        return cannot_show("it is larger than any screenshot");
    }
    // A read that fails (a locked or vanished file) is not remembered: it
    // may work next time.
    let bytes = state.sources.with(package, |source| {
        source.read_entry(entry, Some(thumbs::MAX_IMAGE_BYTES))
    })?;
    match thumbs::render(&bytes, width) {
        Ok(jpeg) => {
            thumbs::remember(&dir, &name, &jpeg);
            Ok(Some(thumbs::data_url(&jpeg)))
        }
        Err(why) => cannot_show(&why),
    }
}

// -- wrappers ----------------------------------------------------------------

#[tauri::command]
pub async fn search(state: Spine<'_>, query: SearchQuery) -> Result<Vec<SearchResult>, RiftError> {
    blocking(&state, move |state| Ok(search_impl(state, &query))).await
}

#[tauri::command]
pub async fn get_option(state: Spine<'_>, option_id: String) -> Result<OptionDetail, RiftError> {
    blocking(&state, move |state| get_option_impl(state, &option_id)).await
}

#[tauri::command]
pub async fn get_package(state: Spine<'_>, package_id: String) -> Result<PackageDetail, RiftError> {
    blocking(&state, move |state| get_package_impl(state, &package_id)).await
}

#[tauri::command]
pub async fn get_thumbnail(
    state: Spine<'_>,
    option_id: String,
    size: ThumbnailSize,
) -> Result<Option<String>, RiftError> {
    blocking(&state, move |state| {
        get_thumbnail_impl(state, &option_id, size)
    })
    .await
}

#[tauri::command]
pub async fn get_library(state: Spine<'_>) -> Result<LibraryView, RiftError> {
    blocking(&state, |state| Ok(get_library_impl(state))).await
}

#[tauri::command]
pub async fn open_mods_folder(state: Spine<'_>) -> Result<(), RiftError> {
    blocking(&state, open_mods_folder_impl).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use rift_core::{PackageFormat, PayloadKinds as CoreKinds};
    use std::path::PathBuf;

    fn option(
        package: &PackageId,
        root: &str,
        name: &str,
        parent: Option<&str>,
        bundle: Option<&str>,
    ) -> ModOption {
        ModOption {
            id: OptionId::new(package, root),
            package_id: package.clone(),
            root: root.into(),
            name: name.into(),
            info: None,
            screenshot: None,
            payload: vec![],
            skipped: vec![],
            kinds: CoreKinds {
                loose: 1,
                ..CoreKinds::default()
            },
            parent: parent.map(|p| OptionId::new(package, p)),
            addon_for: None,
            bundle: bundle.map(str::to_string),
            menu_only: false,
            trail: vec![],
        }
    }

    #[test]
    fn the_catalogs_nesting_is_decorated_for_the_window() {
        let id = PackageId("00000000000000aa".into());
        let mut cloaks = option(&id, "Menu", "Cloaks", None, None);
        cloaks.menu_only = true;
        cloaks.kinds = CoreKinds::default();
        let package = Package {
            id: id.clone(),
            path: PathBuf::from("/mods/Cloaks.zip"),
            file_name: "Cloaks.zip".into(),
            display_name: "Vernworth Cloaks Collection".into(),
            format: PackageFormat::Zip,
            size: 1,
            modified_ms: 1,
            entry_count: 4,
            options: vec![
                cloaks,
                option(&id, "Menu/A", "Ermine Cape", Some("Menu"), Some("Capes")),
                option(
                    &id,
                    "Menu/B",
                    "Ermine Cape - Variation 2",
                    Some("Menu"),
                    Some("capes"),
                ),
                option(&id, "Menu/C", "Wool Hood", Some("Menu"), None),
            ],
            nexus: None,
            problems: vec![],
        };
        let installed_id = OptionId::new(&id, "Menu/B");
        let conflicted = HashSet::from([OptionId::new(&id, "Menu/C").0]);
        let tree = package_tree(&package, &|id| id == &installed_id, &conflicted);
        assert_eq!(tree.len(), 1);
        let menu = &tree[0];
        assert_eq!(menu.label, "Cloaks");
        assert!(menu.menu_only);
        assert!(
            !menu.installed && !menu.conflicted,
            "a node speaks for itself"
        );
        assert_eq!(
            menu.children.len(),
            2,
            "one bundle node and one plain option"
        );
        let bundle = &menu.children[0];
        assert_eq!(bundle.kind, TreeNodeKind::Bundle);
        assert_eq!(bundle.option_id, None);
        assert_eq!(bundle.label, "Capes", "the first member's spelling");
        assert_eq!(bundle.children.len(), 2);
        assert!(bundle.installed, "any child installed");
        assert!(!bundle.conflicted);
        assert!(!bundle.menu_only);
        assert_eq!(bundle.kinds.loose, 2, "the children's kinds, summed");
        let hood = &menu.children[1];
        assert_eq!(hood.label, "Wool Hood");
        assert!(hood.conflicted);
        assert_eq!(hood.option_id.as_deref(), Some("00000000000000aa#Menu/C"));
    }

    #[test]
    fn snippets_collapse_whitespace_and_stop_near_180_characters() {
        assert_eq!(
            snippet("  A   lantern\n\tthat glows "),
            Some("A lantern that glows".into())
        );
        assert_eq!(snippet("   "), None);
        let long = "word ".repeat(100);
        let cut = snippet(&long).unwrap();
        assert!(cut.chars().count() <= SNIPPET_CHARS);
        assert!(cut.ends_with('…'));
    }
}
