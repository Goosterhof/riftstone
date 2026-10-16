//! The library: search, one option, one package, a thumbnail, the roots.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rift_core::{fnv1a64, lower_key, ModOption, OptionId, Package, PackageId, RiftError, RootKind};

use crate::commands::{blocking, not_found, Spine};
use crate::conflicts;
use crate::dto::{
    self, path_text, ArchiveEntry, ConflictView, InstallBadge, InstallOrigin, InstalledInfo,
    LibraryRoot, OptionDetail, OptionRef, PackageDetail, PackageSummary, PayloadKinds, SearchQuery,
    SearchResult, ThumbnailSize, TreeNode, TreeNodeKind,
};
use crate::events;
use crate::nexus::NexusStore;
use crate::state::{downloads_root, fluffy_root, lock, read, same_folder, write, AppState};
use crate::thumbs;
use crate::watch;

/// How much of a description a search card shows.
const SNIPPET_CHARS: usize = 180;

// ---------------------------------------------------------------------------
// search
// ---------------------------------------------------------------------------

pub fn search_impl(state: &AppState, query: &SearchQuery) -> Vec<SearchResult> {
    let shelf = state.shelf();
    let snapshot = state.snapshot();
    let installs = state.journal_installs();
    let is_installed =
        |id: &OptionId| installs.contains_key(id) || snapshot.fluffy_installed.contains(id);
    let hits = shelf.search(&query.into(), &is_installed);
    let store = lock(&state.nexus);
    let mut badges: HashMap<&PackageId, Option<dto::UpdateBadge>> = HashMap::new();
    hits.iter()
        .filter_map(|hit| {
            let (package, option) = shelf.option(&hit.option_id)?;
            let update = *badges
                .entry(&package.id)
                .or_insert_with(|| store.badge(package));
            let info = option.info.as_ref();
            Some(SearchResult {
                option_id: option.id.0.clone(),
                package_id: package.id.0.clone(),
                score: hit.score,
                name: option.name.clone(),
                trail: option.trail.clone(),
                bundle: option.bundle.clone(),
                package_name: package.display_name.clone(),
                package_format: package.format.into(),
                root_id: package.root_id.clone(),
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
                installed: install_badge(&installs, &snapshot.fluffy_installed, &option.id),
                update,
                conflicted: snapshot.conflicted.contains(&option.id.0),
            })
        })
        .collect()
}

fn install_badge(
    installs: &HashMap<OptionId, (bool, String)>,
    fluffy_installed: &HashSet<OptionId>,
    id: &OptionId,
) -> Option<InstallBadge> {
    if let Some((adopted, installed_at)) = installs.get(id) {
        return Some(InstallBadge {
            origin: if *adopted {
                InstallOrigin::Fluffy
            } else {
                InstallOrigin::Riftstone
            },
            installed_at: Some(installed_at.clone()),
        });
    }
    fluffy_installed.contains(id).then_some(InstallBadge {
        origin: InstallOrigin::FluffyUnadopted,
        installed_at: None,
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
        root_id: package.root_id.clone(),
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
        fluffy_installed: snapshot.fluffy_installed.contains(&id),
        conflicts,
        update: store.link_for(package).map(|_| store.row(package)),
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
    let mut layers = conflicts::game_layers(
        game_root,
        journal,
        game_state,
        snapshot.ledger.as_ref(),
        &snapshot.fluffy_matches,
        &state.paks,
    );
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
    let installs = state.journal_installs();
    let installed: HashSet<&OptionId> = installs.keys().collect();
    let store = lock(&state.nexus);
    Ok(PackageDetail {
        package: package_summary(package, &store),
        tree: package_tree(package, &installed),
        entries: shelf
            .entries(&package.id)
            .iter()
            .map(ArchiveEntry::from)
            .collect(),
    })
}

/// The package's options as Fluffy's menus nest them: `addonfor` children
/// under their parent, `NameAsBundle` siblings gathered under a bundle node.
/// An option whose parent lives in another package is a top node here.
pub fn package_tree(package: &Package, installed: &HashSet<&OptionId>) -> Vec<TreeNode> {
    let own: HashSet<&OptionId> = package.options.iter().map(|o| &o.id).collect();
    let mut children: HashMap<&OptionId, Vec<&ModOption>> = HashMap::new();
    let mut top = Vec::new();
    for option in &package.options {
        match option.parent.as_ref().filter(|parent| own.contains(parent)) {
            Some(parent) => children.entry(parent).or_default().push(option),
            None => top.push(option),
        }
    }
    let mut visited = HashSet::new();
    tree_level(&top, &children, installed, &mut visited)
}

fn tree_level<'a>(
    options: &[&'a ModOption],
    children: &HashMap<&OptionId, Vec<&'a ModOption>>,
    installed: &HashSet<&OptionId>,
    visited: &mut HashSet<&'a OptionId>,
) -> Vec<TreeNode> {
    enum Slot<'a> {
        Option(&'a ModOption),
        Bundle(usize),
    }
    let mut slots: Vec<Slot> = Vec::new();
    let mut bundles: Vec<(String, Vec<&ModOption>)> = Vec::new();
    for option in options {
        match &option.bundle {
            Some(bundle) => match bundles
                .iter()
                .position(|(name, _)| lower_key(name) == lower_key(bundle))
            {
                Some(at) => bundles[at].1.push(option),
                None => {
                    slots.push(Slot::Bundle(bundles.len()));
                    bundles.push((bundle.clone(), vec![option]));
                }
            },
            None => slots.push(Slot::Option(option)),
        }
    }
    let mut nodes = Vec::new();
    for slot in slots {
        match slot {
            Slot::Option(option) => {
                if let Some(node) = option_node(option, children, installed, visited) {
                    nodes.push(node);
                }
            }
            Slot::Bundle(at) => {
                let (name, members) = &bundles[at];
                let kids: Vec<TreeNode> = members
                    .iter()
                    .filter_map(|option| option_node(option, children, installed, visited))
                    .collect();
                let mut kinds = PayloadKinds::default();
                for kid in &kids {
                    kinds.pak += kid.kinds.pak;
                    kinds.loose += kid.kinds.loose;
                    kinds.reframework += kid.kinds.reframework;
                    kinds.root += kid.kinds.root;
                }
                nodes.push(TreeNode {
                    option_id: None,
                    label: name.clone(),
                    kind: TreeNodeKind::Bundle,
                    menu_only: kids.iter().all(|k| k.menu_only),
                    has_screenshot: kids.iter().any(|k| k.has_screenshot),
                    installed: kids.iter().any(|k| k.installed),
                    kinds,
                    children: kids,
                });
            }
        }
    }
    nodes
}

fn option_node<'a>(
    option: &'a ModOption,
    children: &HashMap<&OptionId, Vec<&'a ModOption>>,
    installed: &HashSet<&OptionId>,
    visited: &mut HashSet<&'a OptionId>,
) -> Option<TreeNode> {
    if !visited.insert(&option.id) {
        return None; // a loop the tree resolver let through: shown once
    }
    let kids = children
        .get(&option.id)
        .map(|kids| tree_level(kids, children, installed, visited))
        .unwrap_or_default();
    Some(TreeNode {
        option_id: Some(option.id.0.clone()),
        label: option.name.clone(),
        kind: TreeNodeKind::Option,
        menu_only: option.menu_only,
        has_screenshot: option.screenshot.is_some(),
        installed: installed.contains(&option.id),
        kinds: option.kinds.into(),
        children: kids,
    })
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

// ---------------------------------------------------------------------------
// roots
// ---------------------------------------------------------------------------

/// Add a folder to read mods from. Fluffy's Mods folder and Downloads come
/// back as their built-in roots; any other folder is a custom root. Adding a
/// folder that is already a root returns that root.
pub fn add_root_impl(state: &Arc<AppState>, path: &str) -> Result<dto::LibraryRoot, RiftError> {
    let folder = PathBuf::from(path.trim());
    if path.trim().is_empty() || !folder.is_dir() {
        return Err(RiftError::unreadable(
            &folder,
            "the folder does not exist or is not a folder",
        ));
    }
    // A root holding the stone's own folders would rescan itself after every
    // scan (each one writes the cache there); one holding the game, or inside
    // it, would list the very files the stone installs.
    let dirs = &state.config.dirs;
    if watch::within(&dirs.data, &folder) || watch::within(&dirs.cache, &folder) {
        return Err(RiftError::refused(
            "That folder holds the Riftstone's own data, so it can't also be a library folder. Choose the folder your mod downloads land in.",
        ));
    }
    let game_root = read(&state.world).game.as_ref().map(|g| g.root.clone());
    if game_root.is_some_and(|game| watch::within(&game, &folder) || watch::within(&folder, &game))
    {
        return Err(RiftError::refused(
            "That folder holds the game itself, so it can't be a library folder: the stone would list the very files it installs there. Choose the folder your mod downloads land in.",
        ));
    }
    let fluffy_mods = read(&state.world)
        .fluffy
        .as_ref()
        .map(|f| f.mods_dir.clone());
    let root = {
        let mut roots = write(&state.roots);
        if let Some(existing) = roots.iter().find(|r| same_folder(&r.path, &folder)) {
            return Ok(LibraryRoot::from(existing));
        }
        let taken = |id: &str| roots.iter().any(|r| r.id == id);
        let root =
            if fluffy_mods.is_some_and(|mods| same_folder(&mods, &folder)) && !taken("fluffy") {
                fluffy_root(&folder)
            } else if state
                .config
                .downloads_dir
                .as_ref()
                .is_some_and(|dl| same_folder(dl, &folder))
                && !taken("downloads")
            {
                downloads_root(&folder)
            } else {
                custom_root(&folder, &roots)
            };
        roots.push(root.clone());
        root
    };
    state.save_roots();
    state.arm_watcher();
    state.start_scan();
    Ok(LibraryRoot::from(&root))
}

fn custom_root(folder: &Path, roots: &[rift_core::LibraryRoot]) -> rift_core::LibraryRoot {
    let key = lower_key(&folder.to_string_lossy().replace('\\', "/"));
    let mut id = format!("custom-{:016x}", fnv1a64(key.as_bytes()));
    let mut n = 2;
    while roots.iter().any(|r| r.id == id) {
        id = format!("custom-{:016x}-{n}", fnv1a64(key.as_bytes()));
        n += 1;
    }
    let label = folder
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| folder.to_string_lossy().into_owned());
    rift_core::LibraryRoot {
        id,
        path: folder.to_path_buf(),
        label,
        kind: RootKind::Custom,
        dd2_shaped_only: false,
    }
}

/// Stop reading a folder. Its packages leave the library at once (rebuilt
/// from the catalog cache — nothing is opened); built-in roots come back only
/// through `add_root`.
pub fn remove_root_impl(state: &Arc<AppState>, root_id: &str) -> Result<(), RiftError> {
    let removed = {
        let mut roots = write(&state.roots);
        let before = roots.len();
        roots.retain(|r| r.id != root_id);
        roots.len() != before
    };
    if removed {
        state.save_roots();
        state.reshelve_from_cache();
        state.arm_watcher();
        state.events.changed(events::LIBRARY_CHANGED);
    }
    Ok(())
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
pub async fn add_root(state: Spine<'_>, path: String) -> Result<dto::LibraryRoot, RiftError> {
    blocking(&state, move |state| add_root_impl(state, &path)).await
}

#[tauri::command]
pub async fn remove_root(state: Spine<'_>, root_id: String) -> Result<(), RiftError> {
    blocking(&state, move |state| remove_root_impl(state, &root_id)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use rift_core::{PackageFormat, PayloadKinds as CoreKinds};

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
    fn the_tree_nests_addons_and_gathers_bundles() {
        let id = PackageId("00000000000000aa".into());
        let package = Package {
            id: id.clone(),
            root_id: "fluffy".into(),
            path: PathBuf::from("/m/Cloaks.zip"),
            file_name: "Cloaks.zip".into(),
            display_name: "Vernworth Cloaks Collection".into(),
            format: PackageFormat::Zip,
            size: 1,
            modified_ms: 1,
            entry_count: 4,
            options: vec![
                option(&id, "Menu", "Cloaks", None, None),
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
        let installed = HashSet::from([&installed_id]);
        let tree = package_tree(&package, &installed);
        assert_eq!(tree.len(), 1);
        let menu = &tree[0];
        assert_eq!(menu.label, "Cloaks");
        assert_eq!(
            menu.children.len(),
            2,
            "one bundle node and one plain option"
        );
        let bundle = &menu.children[0];
        assert_eq!(bundle.kind, TreeNodeKind::Bundle);
        assert_eq!(bundle.option_id, None);
        assert_eq!(bundle.children.len(), 2);
        assert!(bundle.installed);
        assert_eq!(bundle.kinds.loose, 2);
        assert_eq!(menu.children[1].label, "Wool Hood");
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
