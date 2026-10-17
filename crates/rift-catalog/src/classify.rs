//! Option discovery and payload classification for ONE package.
//!
//! **Option roots.** Every folder holding a `modinfo.ini` (the `modinfos`
//! keys) is an option root. Files under no modinfo root look for an anchor: the
//! first folder (from the top) named `natives` or `reframework` makes the folder
//! BEFORE it a root; a `.pak` elsewhere makes its own folder a root; so does a
//! `dinput8.dll`. When an anchored root sits inside another anchored root, the
//! outer one wins. Files under no root at all (a README beside the mod folders)
//! are documentation and belong to no option.
//!
//! **Nested variants.** A modinfo root with no game files of its own — nothing
//! under its own `natives/` or `reframework/`, no `.pak` outside the folders
//! below — whose direct subfolders are anchor roots themselves (each holds
//! `natives/`, `reframework/`, a `.pak` or a `dinput8.dll`) is a menu over
//! those subfolders: each becomes an option named after its folder, hanging
//! under the modinfo option (`addonfor` = its name, `parent` = its id) and
//! carrying the modinfo option's info minus its `name` and `screenshot` (and
//! minus `DummyMod`, or the variants would be menus too). The modinfo option
//! keeps what is left — usually nothing, so it is `menu_only`. Without this
//! rule every file of such a pack sat in "a folder the game never reads".
//!
//! **Payload.** A file belongs to its NEAREST root (nested roots are their own
//! options) and is placed by its path relative to that root:
//!
//! | relative path                         | becomes                          |
//! |---------------------------------------|----------------------------------|
//! | `natives/…`                           | `Loose`                          |
//! | `reframework/…`                       | `Reframework`                    |
//! | `….pak` anywhere else                 | `Pak` (file name only)           |
//! | `modinfo.ini` at the root             | skipped `Modinfo`                |
//! | an image at the root                  | skipped `Picture`                |
//! | a document at the root                | skipped `Document`               |
//! | any other file at the root            | `Root` (`dinput8.dll`, `.asi`…)  |
//! | anything in another subfolder         | skipped `UnknownPlace`           |
//!
//! Three refinements keep junk out of the game folder and keep skip counts
//! honest: the option's own preview image is a skipped `Picture` wherever it
//! sits (a `screenshot=` may point into a subfolder); operating-system litter
//! (`Thumbs.db`, `desktop.ini`, `.DS_Store`, `._*`) and extension-less
//! `README` / `LICENSE` / `CHANGELOG` files at the root are skipped `Document`s;
//! programs at the root (`.exe`, `.msi`, `.bat`, …) are skipped `UnknownPlace`
//! — the deploy guard refuses every `*.exe` in the game root, so listing one as
//! payload would make the whole option uninstallable; and so are archives at the
//! root (`.zip`, `.7z`, `.rar`): the game never opens one, so copying it into the
//! game folder would only leave litter there.
//!
//! **Problems.** One `FilesSkipped` naming how many files sit where the game
//! never looks; one `OutdatedFormats` counting the loose payload files whose
//! asset suffix predates the September 2026 engine update, TU 3.2
//! ([`rift_core::dd2::outdated_suffix`]), and naming DD2 Pak Bridge as the
//! likely fix; one `NothingToInstall` when no option has a payload and none is
//! a menu (a `DummyMod`, or an empty option that another option of the package
//! hangs under).

use std::borrow::Cow;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use rift_core::dd2::outdated_suffix;
use rift_core::path::{file_name, parent};
use rift_core::{
    lower_key, normalize_entry_path, ArchiveEntry, DeployTarget, ModInfo, ModOption, OptionId,
    PackageId, PayloadFile, PayloadKinds, Problem, ProblemKind, SkipReason, SkippedFile,
};

const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "bmp", "webp"];
const DOCUMENT_EXTENSIONS: &[&str] = &[
    "txt", "md", "pdf", "url", "nfo", "htm", "html", "rtf", "doc", "docx",
];
/// Litter operating systems leave beside files; never mod content.
const SYSTEM_LITTER: &[&str] = &["thumbs.db", "desktop.ini", ".ds_store"];
/// Documentation by convention even without an extension.
const DOCUMENT_NAMES: &[&str] = &[
    "readme",
    "license",
    "licence",
    "changelog",
    "credits",
    "copying",
];
/// Programs: the deploy guard refuses executables in the game root.
const PROGRAM_EXTENSIONS: &[&str] = &["exe", "msi", "bat", "cmd", "ps1", "vbs", "com", "scr"];
/// Archives inside a mod: packaging, never payload. The game never opens one.
const ARCHIVE_EXTENSIONS: &[&str] = &["zip", "7z", "rar"];
/// Preview names looked for when modinfo names none, in order.
const SCREENSHOT_NAMES: &[&str] = &["screenshot.png", "screenshot.jpg", "screenshot.jpeg"];
/// Folders the game root must keep as folders.
const GAME_FOLDER_NAMES: &[&str] = &["natives", "reframework", "pak_mods"];

/// Find every option in a package and classify its payload.
///
/// `modinfos` maps a normalized directory inside the package (the folder that
/// holds a `modinfo.ini`, `""` for the package root) to its parsed ModInfo.
/// Parents (`addonfor`) are NOT resolved here — [`crate::tree::resolve`] does
/// that across the whole library; `trail` starts as the package name (plus the
/// bundle) and the tree completes it.
pub fn build_options(
    package_id: &PackageId,
    package_display_name: &str,
    entries: &[ArchiveEntry],
    modinfos: &BTreeMap<String, ModInfo>,
) -> (Vec<ModOption>, Vec<Problem>) {
    // One file per Windows path: the first spelling wins, as a case-blind
    // `read_entry` would find it.
    let mut seen = HashSet::with_capacity(entries.len());
    let files: Vec<(&ArchiveEntry, String)> = entries
        .iter()
        .filter_map(|entry| {
            let key = lower_key(&entry.path);
            seen.insert(key.clone()).then_some((entry, key))
        })
        .collect();

    let roots = discover_roots(&files, modinfos, package_display_name);

    let mut members: HashMap<&str, Vec<usize>> = HashMap::with_capacity(roots.len());
    for (index, (_, lower)) in files.iter().enumerate() {
        if let Some(root) = nearest_root(lower, &roots) {
            members.entry(root).or_default().push(index);
        }
    }

    let lower_index: HashMap<&str, usize> = files
        .iter()
        .enumerate()
        .map(|(index, (_, lower))| (lower.as_str(), index))
        .collect();

    let mut options = Vec::with_capacity(roots.len());
    for (key, root) in &roots {
        let indices = members.get(key.as_str()).map(Vec::as_slice).unwrap_or(&[]);
        options.push(build_option(
            package_id,
            package_display_name,
            root,
            indices,
            &files,
            &lower_index,
        ));
    }

    options.sort_by_cached_key(|option| {
        (
            Reverse(option.info.as_ref().map_or(0, |info| info.menu_priority)),
            lower_key(&option.name),
            lower_key(&option.root),
        )
    });

    let problems = package_problems(&options);
    (options, problems)
}

// ---------------------------------------------------------------------------
// Roots
// ---------------------------------------------------------------------------

struct Root<'a> {
    /// The folder as spelled in the package (`""` = the package root).
    spelled: String,
    /// How many path segments the folder has.
    depth: usize,
    /// The folder's own modinfo, or — for a nested variant — the one it
    /// inherits from the modinfo option above it.
    info: Option<Cow<'a, ModInfo>>,
    /// A nested variant's parent: the modinfo option root (as spelled) it
    /// hangs under.
    parent: Option<String>,
}

impl<'a> Root<'a> {
    fn new(spelled: String, info: Option<Cow<'a, ModInfo>>) -> Self {
        let depth = segment_count(&spelled);
        Root {
            spelled,
            depth,
            info,
            parent: None,
        }
    }
}

/// Every option root, keyed by its lower-cased path.
fn discover_roots<'a>(
    files: &[(&ArchiveEntry, String)],
    modinfos: &'a BTreeMap<String, ModInfo>,
    package_display_name: &str,
) -> BTreeMap<String, Root<'a>> {
    let mut roots: BTreeMap<String, Root<'a>> = BTreeMap::new();
    for (dir, info) in modinfos {
        match normalize_entry_path(dir) {
            Ok(spelled) => {
                roots
                    .entry(lower_key(&spelled))
                    .or_insert_with(|| Root::new(spelled, Some(Cow::Borrowed(info))));
            }
            Err(err) => log::warn!("ignoring a modinfo.ini in an unsafe folder: {err}"),
        }
    }

    let nested = nested_variants(files, &roots, package_display_name);

    // Anchored roots, from the files no modinfo root claims.
    let mut anchors: HashMap<String, String> = HashMap::new();
    for (entry, lower) in files {
        if nearest_root(lower, &roots).is_some() {
            continue;
        }
        if let Some(dir) = anchor_dir(&entry.path) {
            anchors
                .entry(lower_key(dir))
                .or_insert_with(|| dir.to_string());
        }
    }
    for (key, spelled) in &anchors {
        let inner = !key.is_empty() && ancestor_dirs(key).any(|outer| anchors.contains_key(outer));
        if !inner {
            roots
                .entry(key.clone())
                .or_insert_with(|| Root::new(spelled.clone(), None));
        }
    }

    for (key, root) in nested {
        roots.entry(key).or_insert(root);
    }
    roots
}

/// The nested variants under modinfo roots that hold no game files of their
/// own (see the module docs), keyed by lower-cased path. `modinfo_roots`
/// holds the modinfo roots only.
fn nested_variants<'a>(
    files: &[(&ArchiveEntry, String)],
    modinfo_roots: &BTreeMap<String, Root<'a>>,
    package_display_name: &str,
) -> Vec<(String, Root<'a>)> {
    let mut owned: HashMap<&str, Vec<usize>> = HashMap::new();
    for (index, (_, lower)) in files.iter().enumerate() {
        if let Some(root) = nearest_root(lower, modinfo_roots) {
            owned.entry(root).or_default().push(index);
        }
    }

    let mut nested = Vec::new();
    for (key, root) in modinfo_roots {
        let (Some(indices), Some(info)) = (owned.get(key.as_str()), root.info.as_deref()) else {
            continue;
        };

        // Direct subfolders that are anchor roots themselves: lower-cased
        // path (as the files spell it, lower-cased) → the folder's own name.
        let mut variants: BTreeMap<&str, &str> = BTreeMap::new();
        for &index in indices {
            let (entry, lower) = &files[index];
            let rel = strip_segments(&entry.path, root.depth);
            if let Some(dir) = anchor_dir(rel) {
                if !dir.is_empty() && !dir.contains('/') {
                    variants
                        .entry(prefix_segments(lower, root.depth + 1))
                        .or_insert(dir);
                }
            }
        }
        if variants.is_empty() {
            continue;
        }

        // Game files of the root's own, outside those subfolders, keep the
        // root an ordinary option (its other folders stay skipped).
        let has_own_payload = indices.iter().any(|&index| {
            let (entry, lower) = &files[index];
            let rel = strip_segments(&entry.path, root.depth);
            let in_variant =
                rel.contains('/') && variants.contains_key(prefix_segments(lower, root.depth + 1));
            !in_variant
                && matches!(
                    place(rel),
                    Placement::Deploy(
                        DeployTarget::Loose { .. }
                            | DeployTarget::Reframework { .. }
                            | DeployTarget::Pak { .. }
                    )
                )
        });
        if has_own_payload {
            continue;
        }

        let parent_name = option_name(Some(info), &root.spelled, package_display_name);
        for (variant_key, dir) in variants {
            let spelled = if root.spelled.is_empty() {
                dir.to_string()
            } else {
                format!("{}/{dir}", root.spelled)
            };
            let mut inherited = info.clone();
            inherited.name = None;
            inherited.screenshot = None;
            inherited.dummy = false;
            inherited.addon_for = Some(parent_name.clone());
            let mut variant = Root::new(spelled, Some(Cow::Owned(inherited)));
            variant.parent = Some(root.spelled.clone());
            nested.push((variant_key.to_string(), variant));
        }
    }
    nested
}

/// The deepest root containing `lower_path` (a lower-cased file path).
fn nearest_root<'k, V>(lower_path: &str, roots: &'k BTreeMap<String, V>) -> Option<&'k str> {
    ancestor_dirs(lower_path).find_map(|dir| roots.get_key_value(dir).map(|(key, _)| key.as_str()))
}

/// The folders containing `path`, deepest first, ending with `""`
/// (`a/b/c.pak` → `a/b`, `a`, ``). For a folder path these are its proper
/// ancestors.
fn ancestor_dirs(path: &str) -> impl Iterator<Item = &str> + '_ {
    path.rmatch_indices('/')
        .map(move |(at, _)| &path[..at])
        .chain(std::iter::once(""))
}

/// The root a file without a modinfo would anchor, if any.
fn anchor_dir(path: &str) -> Option<&str> {
    let mut offset = 0;
    let mut segments = path.split('/').peekable();
    while let Some(segment) = segments.next() {
        if segments.peek().is_none() {
            break; // the file name itself is never an anchor folder
        }
        if segment.eq_ignore_ascii_case("natives") || segment.eq_ignore_ascii_case("reframework") {
            return Some(if offset == 0 { "" } else { &path[..offset - 1] });
        }
        offset += segment.len() + 1;
    }
    let name = file_name(path);
    (has_extension(name, "pak") || name.eq_ignore_ascii_case("dinput8.dll")).then(|| parent(path))
}

// ---------------------------------------------------------------------------
// One option
// ---------------------------------------------------------------------------

enum Placement {
    Deploy(DeployTarget),
    Skip(SkipReason),
}

fn build_option(
    package_id: &PackageId,
    package_display_name: &str,
    root: &Root<'_>,
    indices: &[usize],
    files: &[(&ArchiveEntry, String)],
    lower_index: &HashMap<&str, usize>,
) -> ModOption {
    let screenshot = pick_screenshot(root, indices, files, lower_index);

    let mut payload = Vec::new();
    let mut skipped = Vec::new();
    for &index in indices {
        let entry = files[index].0;
        let placement = match place(strip_segments(&entry.path, root.depth)) {
            // The preview is a picture wherever it sits — but never steals payload.
            Placement::Skip(_) if screenshot.as_deref() == Some(entry.path.as_str()) => {
                Placement::Skip(SkipReason::Picture)
            }
            placement => placement,
        };
        match placement {
            Placement::Deploy(target) => payload.push(PayloadFile {
                entry: entry.path.clone(),
                target,
                size: entry.size,
            }),
            Placement::Skip(reason) => skipped.push(SkippedFile {
                entry: entry.path.clone(),
                reason,
            }),
        }
    }

    let info = root.info.as_deref();
    let name = option_name(info, &root.spelled, package_display_name);
    let bundle = info.and_then(|info| info.name_as_bundle.clone());
    let menu_only = info.is_some_and(|info| info.dummy) || payload.is_empty();

    let mut trail = Vec::with_capacity(2);
    let package_crumb = package_display_name.trim();
    if !package_crumb.is_empty() {
        trail.push(package_crumb.to_string());
    }
    if let Some(bundle) = bundle.as_deref().map(str::trim).filter(|b| !b.is_empty()) {
        if lower_key(bundle) != lower_key(package_crumb) {
            trail.push(bundle.to_string());
        }
    }

    ModOption {
        id: OptionId::new(package_id, &root.spelled),
        package_id: package_id.clone(),
        root: root.spelled.clone(),
        name,
        info: info.cloned(),
        screenshot,
        kinds: PayloadKinds::tally(&payload),
        payload,
        skipped,
        // Only nested variants know their parent here; every other `addonfor`
        // is resolved by name across the library by `tree::resolve`.
        parent: root
            .parent
            .as_deref()
            .map(|dir| OptionId::new(package_id, dir)),
        addon_for: info.and_then(|info| info.addon_for.clone()),
        bundle,
        menu_only,
        trail,
    }
}

/// An option's display name: modinfo `name`, else the root folder's own name,
/// else the package's display name.
fn option_name(info: Option<&ModInfo>, spelled_root: &str, package_display_name: &str) -> String {
    info.and_then(|info| info.name.as_deref())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .or_else(|| (!spelled_root.is_empty()).then(|| file_name(spelled_root).to_string()))
        .unwrap_or_else(|| package_display_name.to_string())
}

/// Where a file lands, by its path relative to its option root.
fn place(rel: &str) -> Placement {
    if starts_with_folder(rel, "natives") {
        return Placement::Deploy(DeployTarget::Loose {
            game_path: rel.to_string(),
        });
    }
    if starts_with_folder(rel, "reframework") {
        return Placement::Deploy(DeployTarget::Reframework {
            game_path: rel.to_string(),
        });
    }
    if has_extension(rel, "pak") {
        return Placement::Deploy(DeployTarget::Pak {
            file_name: file_name(rel).to_string(),
        });
    }
    if rel.contains('/') {
        return Placement::Skip(SkipReason::UnknownPlace);
    }

    let lower = rel.to_lowercase();
    let extension = lower.rsplit_once('.').map_or("", |(_, ext)| ext);
    if lower == "modinfo.ini" {
        Placement::Skip(SkipReason::Modinfo)
    } else if GAME_FOLDER_NAMES.contains(&lower.as_str()) {
        // A FILE named like a game folder would block the folder itself.
        Placement::Skip(SkipReason::UnknownPlace)
    } else if IMAGE_EXTENSIONS.contains(&extension) {
        Placement::Skip(SkipReason::Picture)
    } else if DOCUMENT_EXTENSIONS.contains(&extension)
        || SYSTEM_LITTER.contains(&lower.as_str())
        || DOCUMENT_NAMES.contains(&lower.as_str())
        || lower.starts_with("._")
    {
        Placement::Skip(SkipReason::Document)
    } else if PROGRAM_EXTENSIONS.contains(&extension) || ARCHIVE_EXTENSIONS.contains(&extension) {
        Placement::Skip(SkipReason::UnknownPlace)
    } else {
        Placement::Deploy(DeployTarget::Root {
            game_path: rel.to_string(),
        })
    }
}

/// modinfo `screenshot=` when that file exists under the root, else
/// `screenshot.png|jpg|jpeg` at the root, else the first image at the root.
fn pick_screenshot(
    root: &Root<'_>,
    indices: &[usize],
    files: &[(&ArchiveEntry, String)],
    lower_index: &HashMap<&str, usize>,
) -> Option<String> {
    let lookup = |rel: &str| {
        let key = if root.spelled.is_empty() {
            lower_key(rel)
        } else {
            lower_key(&format!("{}/{rel}", root.spelled))
        };
        lower_index
            .get(key.as_str())
            .map(|&index| files[index].0.path.clone())
    };

    let named = root
        .info
        .as_deref()
        .and_then(|info| info.screenshot.as_deref())
        .and_then(|shot| normalize_entry_path(shot).ok())
        .filter(|rel| is_image(rel))
        .and_then(|rel| lookup(&rel));
    named
        .or_else(|| SCREENSHOT_NAMES.iter().find_map(|name| lookup(name)))
        .or_else(|| {
            indices
                .iter()
                .map(|&index| files[index].0)
                .find_map(|entry| {
                    let rel = strip_segments(&entry.path, root.depth);
                    (!rel.contains('/') && is_image(rel)).then(|| entry.path.clone())
                })
        })
}

// ---------------------------------------------------------------------------
// Problems
// ---------------------------------------------------------------------------

fn package_problems(options: &[ModOption]) -> Vec<Problem> {
    let mut problems = Vec::new();

    let has_payload = options.iter().any(|option| !option.payload.is_empty());
    let addon_targets: HashSet<String> = options
        .iter()
        .filter_map(|option| option.addon_for.as_deref())
        .map(|name| lower_key(name.trim()))
        .collect();
    let has_menu = options.iter().any(|option| {
        option.info.as_ref().is_some_and(|info| info.dummy)
            || (option.payload.is_empty() && addon_targets.contains(&lower_key(option.name.trim())))
    });
    if !has_payload && !has_menu {
        problems.push(Problem {
            kind: ProblemKind::NothingToInstall,
            // only the why: the window's card already says "holds nothing the game loads"
            message: "no natives/ or reframework/ folder, no .pak file and no file meant for the game \
                      folder. If it should be a Dragon's Dogma 2 mod, check that the download is complete, \
                      or look for a repacked version on its mod page."
                .to_string(),
        });
    }

    // UnknownPlace files: archives inside the mod (counted on their own: the
    // advice differs), files deeper than the option root (a folder the game
    // never reads), or at the root itself (a program, or a file named like a
    // folder).
    let (mut in_folders, mut at_root, mut archives) = (0, 0, 0);
    for option in options {
        let depth = segment_count(&option.root);
        for file in &option.skipped {
            if file.reason != SkipReason::UnknownPlace {
                continue;
            }
            if is_archive(&file.entry) {
                archives += 1;
            } else if strip_segments(&file.entry, depth).contains('/') {
                in_folders += 1;
            } else {
                at_root += 1;
            }
        }
    }
    if in_folders + at_root > 0 {
        problems.push(Problem {
            kind: ProblemKind::FilesSkipped,
            message: files_skipped_message(in_folders, at_root),
        });
    }
    if archives > 0 {
        problems.push(Problem {
            kind: ProblemKind::FilesSkipped,
            message: archives_inside_message(archives),
        });
    }

    let mut outdated = 0;
    let mut assets = BTreeSet::new();
    for file in options.iter().flat_map(|option| &option.payload) {
        if let DeployTarget::Loose { game_path } = &file.target {
            if let Some(change) = outdated_suffix(game_path) {
                outdated += 1;
                assets.insert(change.asset);
            }
        }
    }
    if outdated > 0 {
        problems.push(Problem {
            kind: ProblemKind::OutdatedFormats,
            message: outdated_formats_message(outdated, &assets),
        });
    }

    problems
}

/// The `OutdatedFormats` why: how many loose files, which asset types, the
/// engine update that orphaned them, and the likely fix. The verdict ("part of
/// it won't load") is the window's to say (Chaos #00129 D3).
fn outdated_formats_message(outdated: usize, assets: &BTreeSet<&str>) -> String {
    let verb = if outdated == 1 { "was" } else { "were" };
    format!(
        "{} ({}) {verb} made for Dragon's Dogma 2 before the September 2026 engine update (TU 3.2). \
         DD2 Pak Bridge is the likely fix; otherwise look for an updated version of this mod.",
        count(outdated, "loose file", "loose files"),
        assets.iter().copied().collect::<Vec<_>>().join(", ")
    )
}

/// The `FilesSkipped` why: what keeps each file out, counted by cause. The
/// verdict ("some of its files stay out") is the window's to say (Chaos #00129 D3).
fn files_skipped_message(in_folders: usize, at_root: usize) -> String {
    let total = in_folders + at_root;
    let programs = |n: usize| {
        if n == 1 {
            "is a program (or named like a game folder)"
        } else {
            "are programs (or named like game folders)"
        }
    };
    let why = match (in_folders, at_root) {
        (1, 0) => "1 file sits in a folder the game never reads".to_string(),
        (_, 0) => format!("{in_folders} files sit in folders the game never reads"),
        (0, _) => format!(
            "{} {}, and those never go into the game folder",
            count(at_root, "file", "files"),
            programs(at_root)
        ),
        _ => format!(
            "{} in folders the game never reads, and {at_root} {}",
            count_verb(in_folders, "file sits", "files sit"),
            programs(at_root)
        ),
    };
    let (subject, object) = if total == 1 {
        ("it was", "it")
    } else {
        ("they were", "them")
    };
    format!(
        "{why}. The options list {object} under skipped files; if {subject} meant for the game, \
         the mod needs repacking."
    )
}

/// An entry whose name ends in a zip, 7z or rar extension (any case).
fn is_archive(entry: &str) -> bool {
    let lower = file_name(entry).to_lowercase();
    lower
        .rsplit_once('.')
        .is_some_and(|(stem, ext)| !stem.is_empty() && ARCHIVE_EXTENSIONS.contains(&ext))
}

/// The `FilesSkipped` why for archives an option carries: the game never opens
/// one, so it is listed as skipped and the player is told where it belongs.
fn archives_inside_message(archives: usize) -> String {
    if archives == 1 {
        "1 archive sits inside the mod, and the game never opens archives. The options list it \
         under skipped files; to install what it holds, put that archive directly in the mods folder."
            .to_string()
    } else {
        format!(
            "{archives} archives sit inside the mod, and the game never opens archives. The options \
             list them under skipped files; to install what they hold, put each archive directly in \
             the mods folder."
        )
    }
}

fn count(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

/// `1 sits` / `3 sit`.
fn count_verb(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn is_image(path: &str) -> bool {
    IMAGE_EXTENSIONS.iter().any(|ext| has_extension(path, ext))
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// How many segments a normalized folder path has (`""` has none).
fn segment_count(path: &str) -> usize {
    if path.is_empty() {
        0
    } else {
        path.split('/').count()
    }
}

/// The first `count` segments of `path` (`a/b/c.pak`, 2 → `a/b`); the whole
/// path when it has no more.
fn prefix_segments(path: &str, count: usize) -> &str {
    if count == 0 {
        return "";
    }
    path.match_indices('/')
        .nth(count - 1)
        .map_or(path, |(at, _)| &path[..at])
}

/// `path` without its first `depth` segments (the option root).
fn strip_segments(path: &str, depth: usize) -> &str {
    if depth == 0 {
        return path;
    }
    path.match_indices('/')
        .nth(depth - 1)
        .map_or("", |(at, _)| &path[at + 1..])
}

/// `natives/…` (any case) — the folder and a slash, never a file named so.
fn starts_with_folder(rel: &str, folder: &str) -> bool {
    rel.get(..folder.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(folder))
        && rel.as_bytes().get(folder.len()) == Some(&b'/')
}

/// Case-blind extension test on a path or file name.
fn has_extension(path: &str, extension: &str) -> bool {
    file_name(path)
        .rsplit_once('.')
        .is_some_and(|(stem, ext)| !stem.is_empty() && ext.eq_ignore_ascii_case(extension))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listing(paths: &[&str]) -> Vec<ArchiveEntry> {
        paths
            .iter()
            .map(|path| ArchiveEntry {
                path: (*path).to_string(),
                size: 100,
            })
            .collect()
    }

    fn info(lines: &[(&str, &str)]) -> ModInfo {
        let text: String = lines.iter().map(|(k, v)| format!("{k}={v}\r\n")).collect();
        crate::modinfo::parse(text.as_bytes()).info
    }

    fn modinfos(items: &[(&str, ModInfo)]) -> BTreeMap<String, ModInfo> {
        items
            .iter()
            .map(|(dir, info)| ((*dir).to_string(), info.clone()))
            .collect()
    }

    fn by_name<'a>(options: &'a [ModOption], name: &str) -> &'a ModOption {
        options
            .iter()
            .find(|option| option.name == name)
            .unwrap_or_else(|| {
                panic!(
                    "no option named {name:?} in {:?}",
                    options.iter().map(|o| &o.name).collect::<Vec<_>>()
                )
            })
    }

    fn package() -> PackageId {
        PackageId::for_archive("Vernworth Cloaks Collection.zip", 4096)
    }

    const MENU_OPTION_TEXT: &str = "Menu Option. Enabling/disabling does nothing.";

    /// A collection shaped like the real 281-option pack: a DummyMod menu, a
    /// Menu Option hanging under it, three variants under that.
    fn collection() -> (Vec<ArchiveEntry>, BTreeMap<String, ModInfo>) {
        let base = "Vernworth Cloaks Collection";
        let cape = format!("{base}/Cloaks/Ermine Cape");
        let paths = [
            format!("{base}/README.txt"),
            format!("{base}/Cloaks/modinfo.ini"),
            format!("{cape}/modinfo.ini"),
            format!("{cape}/screenshot.png"),
            format!("{cape}/White/modinfo.ini"),
            format!("{cape}/White/screenshot.jpg"),
            format!(
                "{cape}/White/natives/stm/appsystem/cloakwb/mesh/cloak_301_wb_f.mesh.260421070"
            ),
            format!("{cape}/White/natives/stm/appsystem/cloakwb/tex/cloak_301_alba.tex.251211553"),
            format!("{cape}/Black/modinfo.ini"),
            format!("{cape}/Black/preview/black.png"),
            format!(
                "{cape}/Black/natives/stm/appsystem/cloakwb/mesh/cloak_302_wb_f.mesh.260421070"
            ),
            format!("{cape}/Grey/modinfo.ini"),
            format!("{cape}/Grey/Grey Preview.webp"),
            format!("{cape}/Grey/natives/stm/appsystem/cloakwb/mesh/cloak_303_wb_f.mesh.240423143"),
            format!("{cape}/Grey/extras/notes/alt.bin"),
        ];
        let entries = paths
            .iter()
            .map(|path| ArchiveEntry {
                path: path.clone(),
                size: 100,
            })
            .collect();
        let infos = modinfos(&[
            (
                &format!("{base}/Cloaks"),
                info(&[("name", "Cloaks"), ("DummyMod", "true")]),
            ),
            (
                &cape,
                info(&[
                    ("name", "Ermine Cape (Menu Option)"),
                    ("addonfor", "Cloaks"),
                    ("description", MENU_OPTION_TEXT),
                ]),
            ),
            (
                &format!("{cape}/White"),
                info(&[
                    ("name", "Ermine Cape - White"),
                    ("addonfor", "Ermine Cape (Menu Option)"),
                    ("NameAsBundle", "Ermine Capes"),
                    ("MenuPriority", "1"),
                ]),
            ),
            (
                &format!("{cape}/Black"),
                info(&[
                    ("name", "Ermine Cape - Black"),
                    ("addonfor", "Ermine Cape (Menu Option)"),
                    ("NameAsBundle", "Ermine Capes"),
                    ("screenshot", r"preview\black.png"),
                ]),
            ),
            (
                &format!("{cape}/Grey"),
                info(&[
                    ("name", "Ermine Cape - Grey"),
                    ("addonfor", "Ermine Cape (Menu Option)"),
                ]),
            ),
        ]);
        (entries, infos)
    }

    #[test]
    fn a_collection_becomes_menus_and_variants() {
        let (entries, infos) = collection();
        let (options, problems) =
            build_options(&package(), "Vernworth Cloaks Collection", &entries, &infos);

        let names: Vec<&str> = options.iter().map(|o| o.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Ermine Cape - White", // MenuPriority 1 sorts first
                "Cloaks",
                "Ermine Cape (Menu Option)",
                "Ermine Cape - Black",
                "Ermine Cape - Grey",
            ]
        );

        let menu = by_name(&options, "Cloaks");
        assert!(menu.menu_only && menu.payload.is_empty());
        assert_eq!(menu.root, "Vernworth Cloaks Collection/Cloaks");
        assert_eq!(
            menu.id,
            OptionId::new(&package(), "Vernworth Cloaks Collection/Cloaks")
        );

        let menu_option = by_name(&options, "Ermine Cape (Menu Option)");
        assert!(menu_option.menu_only, "an empty payload is a menu entry");
        assert_eq!(menu_option.addon_for.as_deref(), Some("Cloaks"));
        assert_eq!(menu_option.parent, None, "parents are the tree's job");
        assert_eq!(
            menu_option.screenshot.as_deref(),
            Some("Vernworth Cloaks Collection/Cloaks/Ermine Cape/screenshot.png")
        );
        assert!(
            menu_option
                .payload
                .iter()
                .all(|f| !f.entry.contains("/White/")),
            "nested option roots keep their own files"
        );

        let white = by_name(&options, "Ermine Cape - White");
        assert!(!white.menu_only);
        assert_eq!(white.bundle.as_deref(), Some("Ermine Capes"));
        assert_eq!(white.trail, ["Vernworth Cloaks Collection", "Ermine Capes"]);
        assert_eq!(
            white.kinds,
            PayloadKinds {
                pak: 0,
                loose: 2,
                reframework: 0,
                root: 0
            }
        );
        assert_eq!(
            white.payload[0].target,
            DeployTarget::Loose {
                game_path: "natives/stm/appsystem/cloakwb/mesh/cloak_301_wb_f.mesh.260421070"
                    .into()
            }
        );
        assert!(white
            .screenshot
            .as_deref()
            .is_some_and(|s| s.ends_with("White/screenshot.jpg")));
        let reasons: Vec<SkipReason> = white.skipped.iter().map(|s| s.reason).collect();
        assert_eq!(reasons, [SkipReason::Modinfo, SkipReason::Picture]);

        let black = by_name(&options, "Ermine Cape - Black");
        assert_eq!(
            black.screenshot.as_deref(),
            Some("Vernworth Cloaks Collection/Cloaks/Ermine Cape/Black/preview/black.png"),
            "modinfo screenshot=, backslashes and all"
        );
        assert!(
            black
                .skipped
                .iter()
                .any(|s| s.entry.ends_with("black.png") && s.reason == SkipReason::Picture),
            "the preview in a subfolder is a picture, not a lost file"
        );

        let grey = by_name(&options, "Ermine Cape - Grey");
        assert!(grey
            .screenshot
            .as_deref()
            .is_some_and(|s| s.ends_with("Grey/Grey Preview.webp")));
        assert!(grey
            .skipped
            .iter()
            .any(|s| s.entry.ends_with("extras/notes/alt.bin")
                && s.reason == SkipReason::UnknownPlace));

        assert!(
            options.iter().all(
                |o| o.payload.iter().all(|f| !f.entry.ends_with("README.txt"))
                    && o.skipped.iter().all(|s| !s.entry.ends_with("README.txt"))
            ),
            "the top-level README belongs to no option"
        );

        let kinds: Vec<ProblemKind> = problems.iter().map(|p| p.kind).collect();
        assert_eq!(
            kinds,
            [ProblemKind::FilesSkipped, ProblemKind::OutdatedFormats]
        );
        assert!(
            problems[0].message.starts_with("1 file "),
            "{}",
            problems[0].message
        );
        assert!(
            problems[1].message.starts_with("1 loose file (mesh)"),
            "{}",
            problems[1].message
        );
    }

    #[test]
    fn a_plain_pak_mod() {
        let entries = listing(&[
            "Lantern Glow Recolor/Lantern Glow Recolor.pak",
            "Lantern Glow Recolor/modinfo.ini",
            "Lantern Glow Recolor/cover.jpg",
        ]);
        let infos = modinfos(&[(
            "Lantern Glow Recolor",
            info(&[("name", "Lantern Glow Recolor")]),
        )]);
        let (options, problems) =
            build_options(&package(), "Lantern Glow Recolor", &entries, &infos);
        assert_eq!(options.len(), 1);
        let option = &options[0];
        assert_eq!(
            option.payload[0].target,
            DeployTarget::Pak {
                file_name: "Lantern Glow Recolor.pak".into()
            }
        );
        assert_eq!(
            option.screenshot.as_deref(),
            Some("Lantern Glow Recolor/cover.jpg")
        );
        assert_eq!(option.kinds.pak, 1);
        let reasons: Vec<SkipReason> = option.skipped.iter().map(|s| s.reason).collect();
        assert_eq!(reasons, [SkipReason::Modinfo, SkipReason::Picture]);
        assert!(problems.is_empty(), "{problems:?}");
    }

    #[test]
    fn a_reframework_script_mod_without_modinfo() {
        let entries = listing(&[
            "Lantern Timer/reframework/autorun/lantern_timer.lua",
            "Lantern Timer/reframework/data/lantern_timer/config.json",
            "Lantern Timer/README.md",
        ]);
        let (options, problems) =
            build_options(&package(), "Lantern Timer v2", &entries, &BTreeMap::new());
        assert_eq!(options.len(), 1);
        let option = &options[0];
        assert_eq!(option.name, "Lantern Timer", "named after its folder");
        assert_eq!(option.root, "Lantern Timer");
        assert_eq!(option.kinds.reframework, 2);
        assert_eq!(
            option.payload[0].target,
            DeployTarget::Reframework {
                game_path: "reframework/autorun/lantern_timer.lua".into()
            }
        );
        assert_eq!(option.skipped[0].reason, SkipReason::Document);
        assert!(option.info.is_none());
        assert!(problems.is_empty());

        // At the top of the archive the option is the package itself.
        let entries = entries_at_top();
        let (options, _) =
            build_options(&package(), "Lantern Timer v2", &entries, &BTreeMap::new());
        assert_eq!(
            (options[0].root.as_str(), options[0].name.as_str()),
            ("", "Lantern Timer v2")
        );
    }

    fn entries_at_top() -> Vec<ArchiveEntry> {
        listing(&["reframework/autorun/lantern_timer.lua"])
    }

    #[test]
    fn re_framework_itself() {
        let entries = listing(&["dinput8.dll", "reframework_revision.txt"]);
        let (options, problems) =
            build_options(&package(), "RE Framework", &entries, &BTreeMap::new());
        assert_eq!(options.len(), 1);
        let option = &options[0];
        assert_eq!(option.name, "RE Framework");
        assert_eq!(
            option.payload,
            [PayloadFile {
                entry: "dinput8.dll".into(),
                target: DeployTarget::Root {
                    game_path: "dinput8.dll".into()
                },
                size: 100,
            }]
        );
        assert_eq!(
            option.skipped,
            [SkippedFile {
                entry: "reframework_revision.txt".into(),
                reason: SkipReason::Document,
            }]
        );
        assert!(problems.is_empty());
    }

    #[test]
    fn variant_folders_without_modinfo_become_options_named_after_them() {
        let entries = listing(&[
            "Vernworth Banners/Read me.txt",
            "Vernworth Banners/Red/natives/stm/gui/banner_red.tex.251211553",
            "Vernworth Banners/Blue/natives/stm/gui/banner_blue.tex.251211553",
            "Vernworth Banners/Blue/natives/stm/gui/banner_blue_n.tex.251211553",
        ]);
        let (options, problems) =
            build_options(&package(), "Vernworth Banners", &entries, &BTreeMap::new());
        let names: Vec<&str> = options.iter().map(|o| o.name.as_str()).collect();
        assert_eq!(names, ["Blue", "Red"]);
        assert_eq!(by_name(&options, "Blue").kinds.loose, 2);
        assert_eq!(by_name(&options, "Red").root, "Vernworth Banners/Red");
        assert!(problems.is_empty());
    }

    #[test]
    fn the_outer_anchor_wins() {
        let entries = listing(&[
            "Mod/natives/stm/a.mesh.260421070",
            "Mod/Extra/natives/stm/b.mesh.260421070",
            "Mod/Mod.pak",
            "Mod/Optional/Optional.pak",
        ]);
        let (options, problems) = build_options(&package(), "Mod", &entries, &BTreeMap::new());
        assert_eq!(options.len(), 1);
        let option = &options[0];
        assert_eq!(option.root, "Mod");
        assert_eq!(
            (option.kinds.loose, option.kinds.pak),
            (1, 2),
            "a pak anywhere is still a pak"
        );
        assert_eq!(option.skipped.len(), 1);
        assert_eq!(option.skipped[0].reason, SkipReason::UnknownPlace);
        assert_eq!(problems[0].kind, ProblemKind::FilesSkipped);
    }

    #[test]
    fn a_modinfo_root_holds_an_anchored_folder_and_a_nested_modinfo() {
        let entries = listing(&[
            "natives/stm/a.mesh.260421070",
            "Optional/modinfo.ini",
            "Optional/natives/stm/b.mesh.260421070",
        ]);
        let infos = modinfos(&[("Optional", info(&[("name", "Lantern Glow - Optional")]))]);
        let (options, _) = build_options(&package(), "Lantern Glow", &entries, &infos);
        assert_eq!(options.len(), 2);
        assert_eq!(by_name(&options, "Lantern Glow").payload.len(), 1);
        assert_eq!(
            by_name(&options, "Lantern Glow - Optional").payload[0].target,
            DeployTarget::Loose {
                game_path: "natives/stm/b.mesh.260421070".into()
            }
        );
    }

    #[test]
    fn nothing_to_install_is_reported_once_and_only_when_true() {
        let (options, problems) = build_options(
            &package(),
            "Pictures",
            &listing(&["Docs/readme.txt", "image.png"]),
            &BTreeMap::new(),
        );
        assert!(options.is_empty());
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].kind, ProblemKind::NothingToInstall);

        // A modinfo with nothing beside it installs nothing either.
        let entries = listing(&["Lantern/modinfo.ini", "Lantern/screenshot.png"]);
        let infos = modinfos(&[("Lantern", info(&[("name", "Lantern")]))]);
        let (options, problems) = build_options(&package(), "Lantern", &entries, &infos);
        assert!(options[0].menu_only);
        assert_eq!(
            problems.iter().map(|p| p.kind).collect::<Vec<_>>(),
            [ProblemKind::NothingToInstall]
        );

        // A DummyMod is a deliberate menu, not an empty package.
        let infos = modinfos(&[("Lantern", info(&[("name", "Lantern"), ("DummyMod", "yes")]))]);
        let (_, problems) = build_options(&package(), "Lantern", &entries, &infos);
        assert!(problems.is_empty(), "{problems:?}");

        // So is an empty option another option of the package hangs under.
        let entries = listing(&["Menu/modinfo.ini", "Menu/Child/modinfo.ini"]);
        let infos = modinfos(&[
            ("Menu", info(&[("name", "Lantern Menu")])),
            (
                "Menu/Child",
                info(&[("name", "Child"), ("addonfor", " lantern menu ")]),
            ),
        ]);
        let (_, problems) = build_options(&package(), "Lantern", &entries, &infos);
        assert!(problems.is_empty(), "{problems:?}");
    }

    #[test]
    fn junk_and_programs_never_reach_the_game_root() {
        let entries = listing(&[
            "Mod/natives/stm/a.mesh.260421070",
            "Mod/Thumbs.db",
            "Mod/desktop.ini",
            "Mod/._a.mesh",
            "Mod/LICENSE",
            "Mod/Installer.exe",
            "Mod/lantern.asi",
            "Mod/lantern.ini",
        ]);
        let (options, problems) = build_options(&package(), "Mod", &entries, &BTreeMap::new());
        let option = &options[0];
        let roots: Vec<&str> = option
            .payload
            .iter()
            .filter_map(|f| match &f.target {
                DeployTarget::Root { game_path } => Some(game_path.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(roots, ["lantern.asi", "lantern.ini"]);
        let skipped: Vec<(&str, SkipReason)> = option
            .skipped
            .iter()
            .map(|s| (file_name(&s.entry), s.reason))
            .collect();
        assert_eq!(
            skipped,
            [
                ("Thumbs.db", SkipReason::Document),
                ("desktop.ini", SkipReason::Document),
                ("._a.mesh", SkipReason::Document),
                ("LICENSE", SkipReason::Document),
                ("Installer.exe", SkipReason::UnknownPlace),
            ]
        );
        assert_eq!(problems[0].kind, ProblemKind::FilesSkipped);
        assert_eq!(
            problems[0].message,
            "1 file is a program (or named like a game folder), and those never go into the game \
             folder. The options list it under skipped files; if it was meant for the game, the mod \
             needs repacking."
        );
    }

    #[test]
    fn an_archive_inside_a_mod_is_never_copied_into_the_game_root() {
        let entries = listing(&[
            "Mod/natives/stm/a.mesh.260421070",
            "Mod/modinfo.ini",
            "Mod/lantern.asi",
            "Mod/Older Lantern.zip",
            "Mod/extras/spare.7z",
        ]);
        let infos = modinfos(&[("Mod", info(&[("name", "Lantern")]))]);
        let (options, problems) = build_options(&package(), "Lantern", &entries, &infos);
        let option = &options[0];
        assert!(
            option
                .payload
                .iter()
                .all(|f| !f.entry.to_lowercase().ends_with(".zip")),
            "an archive at the option root is not payload"
        );
        let skipped: Vec<(&str, SkipReason)> = option
            .skipped
            .iter()
            .map(|s| (file_name(&s.entry), s.reason))
            .collect();
        assert!(skipped.contains(&("Older Lantern.zip", SkipReason::UnknownPlace)));
        assert!(skipped.contains(&("spare.7z", SkipReason::UnknownPlace)));
        assert_eq!(
            problems
                .iter()
                .filter(|p| p.kind == ProblemKind::FilesSkipped)
                .map(|p| p.message.as_str())
                .collect::<Vec<_>>(),
            ["2 archives sit inside the mod, and the game never opens archives. The options list \
              them under skipped files; to install what they hold, put each archive directly in the \
              mods folder."],
            "archives are counted on their own, never as programs or as folder litter"
        );
    }

    #[test]
    fn skipped_files_are_counted_by_cause() {
        let entries = listing(&[
            "Mod/natives/stm/a.mesh.260421070",
            "Mod/extras/one.bin",
            "Mod/extras/two.bin",
            "Mod/setup.exe",
            "Mod/natives",
        ]);
        let (options, problems) = build_options(&package(), "Mod", &entries, &BTreeMap::new());
        assert_eq!(
            options[0].payload.len(),
            1,
            "a FILE named natives is never deployed"
        );
        assert_eq!(
            problems[0].message,
            "2 files sit in folders the game never reads, and 2 are programs (or named like game \
             folders). The options list them under skipped files; if they were meant for the game, \
             the mod needs repacking."
        );
    }

    #[test]
    fn a_screenshot_never_steals_payload_and_must_be_an_image() {
        let entries = listing(&[
            "Mod/modinfo.ini",
            "Mod/notes.txt",
            "Mod/natives/stm/gui/preview.png",
            "Mod/shot.jpg",
        ]);
        let infos = modinfos(&[(
            "Mod",
            info(&[
                ("name", "Lantern"),
                ("screenshot", "natives/stm/gui/preview.png"),
            ]),
        )]);
        let (options, _) = build_options(&package(), "Mod", &entries, &infos);
        let option = &options[0];
        assert_eq!(
            option.screenshot.as_deref(),
            Some("Mod/natives/stm/gui/preview.png")
        );
        assert_eq!(
            option.payload.len(),
            1,
            "the preview inside natives/ still installs"
        );

        let infos = modinfos(&[(
            "Mod",
            info(&[("name", "Lantern"), ("screenshot", "notes.txt")]),
        )]);
        let (options, _) = build_options(&package(), "Mod", &entries, &infos);
        assert_eq!(
            options[0].screenshot.as_deref(),
            Some("Mod/shot.jpg"),
            "a screenshot= that is not an image falls back to the first root image"
        );
    }

    #[test]
    fn roots_and_duplicates_are_case_blind() {
        let entries = listing(&[
            "Lantern/natives/STM/a.mesh.260421070",
            "LANTERN/NATIVES/stm/A.MESH.260421070",
            "Lantern/Modinfo.INI",
        ]);
        let infos = modinfos(&[("lantern", info(&[("name", "Lantern")]))]);
        let (options, _) = build_options(&package(), "Lantern", &entries, &infos);
        assert_eq!(options.len(), 1);
        let option = &options[0];
        assert_eq!(option.root, "lantern", "the modinfo folder's own spelling");
        assert_eq!(option.payload.len(), 1, "one file per Windows path");
        assert_eq!(
            option.payload[0].target,
            DeployTarget::Loose {
                game_path: "natives/STM/a.mesh.260421070".into()
            },
            "the archive's own spelling"
        );
        assert_eq!(option.skipped[0].reason, SkipReason::Modinfo);
    }

    #[test]
    fn a_modinfo_at_the_package_root_takes_the_display_name_when_unnamed() {
        let entries = listing(&["modinfo.ini", "natives/stm/a.mesh.260421070"]);
        let infos = modinfos(&[("", info(&[("author", "Someone")]))]);
        let (options, _) = build_options(&package(), "Lantern Glow", &entries, &infos);
        assert_eq!(
            (options[0].name.as_str(), options[0].root.as_str()),
            ("Lantern Glow", "")
        );
        assert_eq!(options[0].trail, ["Lantern Glow"]);
    }

    #[test]
    fn strip_segments_and_folder_prefixes() {
        assert_eq!(strip_segments("a/b/c/d.pak", 2), "c/d.pak");
        assert_eq!(strip_segments("a/b.pak", 0), "a/b.pak");
        assert!(starts_with_folder("NATIVES/stm", "natives"));
        assert!(!starts_with_folder("natives", "natives"));
        assert!(!starts_with_folder("nativesx/a", "natives"));
        assert!(has_extension("a/B.PAK", "pak"));
        assert!(!has_extension(".pak", "pak"));
        assert_eq!(anchor_dir("A/B/natives/c/natives/d"), Some("A/B"));
        assert_eq!(anchor_dir("A/x.pak"), Some("A"));
        assert_eq!(
            anchor_dir("A/natives"),
            None,
            "a file named natives anchors nothing"
        );
        assert_eq!(prefix_segments("a/b/c.pak", 2), "a/b");
        assert_eq!(prefix_segments("a/b", 5), "a/b");
        assert_eq!(prefix_segments("a/b", 0), "");
        assert_eq!((segment_count(""), segment_count("a/b")), (0, 2));
    }

    // -----------------------------------------------------------------------
    // Nested variants
    // -----------------------------------------------------------------------

    /// A modinfo menu with no game files of its own over four subfolders:
    /// three variants (loose, REFramework, pak) and one folder of sketches.
    fn hood_pack() -> (Vec<ArchiveEntry>, BTreeMap<String, ModInfo>) {
        let hoods = "Vernworth Cloaks Collection/Hoods";
        let entries = listing(&[
            &format!("{hoods}/modinfo.ini"),
            &format!("{hoods}/hoods.png"),
            &format!("{hoods}/Read me.txt"),
            &format!(
                "{hoods}/Ashen Hood/natives/stm/appsystem/hoodwb/hood_301_wb_f.mesh.260421070"
            ),
            &format!("{hoods}/Ashen Hood/preview.jpg"),
            &format!("{hoods}/Amber Hood/reframework/autorun/amber_hood.lua"),
            &format!("{hoods}/Azure Hood/Azure Hood.pak"),
            &format!("{hoods}/Azure Hood/extras/notes.bin"),
            &format!("{hoods}/Sketches/hood.psd"),
        ]);
        let infos = modinfos(&[(
            hoods,
            info(&[
                ("name", "Hoods"),
                ("author", "Vernworth Tailors"),
                ("description", "Pick one hood."),
                ("addonfor", "Wardrobe"),
                ("NameAsBundle", "Headwear"),
                ("screenshot", "hoods.png"),
                ("DummyMod", "true"),
                ("MenuPriority", "2"),
                ("requires", "RE Framework"),
            ]),
        )]);
        (entries, infos)
    }

    #[test]
    fn subfolders_of_a_payload_less_modinfo_root_become_nested_variants() {
        let (entries, infos) = hood_pack();
        let (options, problems) =
            build_options(&package(), "Vernworth Cloaks Collection", &entries, &infos);

        let names: Vec<&str> = options.iter().map(|o| o.name.as_str()).collect();
        assert_eq!(
            names,
            ["Amber Hood", "Ashen Hood", "Azure Hood", "Hoods"],
            "named after their folders; the inherited MenuPriority ties them, so by name"
        );

        let hoods = by_name(&options, "Hoods");
        assert!(hoods.menu_only && hoods.payload.is_empty());
        assert_eq!(hoods.parent, None);
        assert_eq!(hoods.addon_for.as_deref(), Some("Wardrobe"));
        let hoods_skipped: Vec<(&str, SkipReason)> = hoods
            .skipped
            .iter()
            .map(|s| (file_name(&s.entry), s.reason))
            .collect();
        assert_eq!(
            hoods_skipped,
            [
                ("modinfo.ini", SkipReason::Modinfo),
                ("hoods.png", SkipReason::Picture),
                ("Read me.txt", SkipReason::Document),
                ("hood.psd", SkipReason::UnknownPlace),
            ],
            "a subfolder that anchors nothing stays skipped"
        );

        let ashen = by_name(&options, "Ashen Hood");
        assert_eq!(ashen.root, "Vernworth Cloaks Collection/Hoods/Ashen Hood");
        assert_eq!(
            ashen.parent.as_ref(),
            Some(&hoods.id),
            "parent = the modinfo option"
        );
        assert_eq!(ashen.addon_for.as_deref(), Some("Hoods"));
        assert!(!ashen.menu_only, "DummyMod is never inherited");
        assert_eq!(
            ashen.payload[0].target,
            DeployTarget::Loose {
                game_path: "natives/stm/appsystem/hoodwb/hood_301_wb_f.mesh.260421070".into()
            }
        );
        assert_eq!(
            ashen.screenshot.as_deref(),
            Some("Vernworth Cloaks Collection/Hoods/Ashen Hood/preview.jpg"),
            "its own picture, not the menu's"
        );
        let inherited = ashen.info.as_ref().expect("inherited info");
        assert_eq!(inherited.name, None);
        assert_eq!(inherited.screenshot, None);
        assert!(!inherited.dummy);
        assert_eq!(inherited.addon_for.as_deref(), Some("Hoods"));
        assert_eq!(inherited.author.as_deref(), Some("Vernworth Tailors"));
        assert_eq!(inherited.description.as_deref(), Some("Pick one hood."));
        assert_eq!(inherited.requires, ["RE Framework"]);
        assert_eq!(inherited.menu_priority, 2);
        assert_eq!(ashen.bundle.as_deref(), Some("Headwear"));

        let amber = by_name(&options, "Amber Hood");
        assert_eq!(amber.kinds.reframework, 1);
        assert_eq!(amber.parent.as_ref(), Some(&hoods.id));
        assert_eq!(amber.screenshot, None, "no picture of its own");

        let azure = by_name(&options, "Azure Hood");
        assert_eq!(
            azure.payload[0].target,
            DeployTarget::Pak {
                file_name: "Azure Hood.pak".into()
            }
        );
        assert_eq!(azure.skipped.len(), 1);
        assert_eq!(azure.skipped[0].reason, SkipReason::UnknownPlace);

        assert_eq!(
            problems.iter().map(|p| p.kind).collect::<Vec<_>>(),
            [ProblemKind::FilesSkipped]
        );
        assert!(
            problems[0].message.starts_with("2 files sit in folders"),
            "only the sketch and the variant's own stray file: {}",
            problems[0].message
        );
    }

    #[test]
    fn pak_variants_stop_installing_together() {
        let entries = listing(&[
            "Lantern Glow/modinfo.ini",
            "Lantern Glow/Red/Lantern Glow Red.pak",
            "Lantern Glow/Blue/Lantern Glow Blue.pak",
        ]);
        let infos = modinfos(&[("Lantern Glow", info(&[("name", "Lantern Glow")]))]);
        let (options, problems) = build_options(&package(), "Lantern Glow", &entries, &infos);
        let names: Vec<&str> = options.iter().map(|o| o.name.as_str()).collect();
        assert_eq!(names, ["Blue", "Lantern Glow", "Red"]);
        assert!(by_name(&options, "Lantern Glow").menu_only);
        for colour in ["Red", "Blue"] {
            let variant = by_name(&options, colour);
            assert_eq!(variant.kinds.pak, 1, "{colour}");
            assert_eq!(variant.addon_for.as_deref(), Some("Lantern Glow"));
        }
        assert!(problems.is_empty(), "{problems:?}");
    }

    #[test]
    fn a_package_root_modinfo_hangs_its_variants_under_the_package_name() {
        let entries = listing(&[
            "modinfo.ini",
            "Red/natives/stm/gui/glow_red.tex.251211553",
            "Blue/natives/stm/gui/glow_blue.tex.251211553",
        ]);
        let infos = modinfos(&[("", info(&[("author", "Someone")]))]);
        let (options, _) = build_options(&package(), "Lantern Glow", &entries, &infos);
        let menu = by_name(&options, "Lantern Glow");
        assert!(menu.menu_only);
        assert_eq!(menu.root, "");
        let red = by_name(&options, "Red");
        assert_eq!(red.root, "Red");
        assert_eq!(red.parent, Some(OptionId::new(&package(), "")));
        assert_eq!(red.addon_for.as_deref(), Some("Lantern Glow"));
        assert_eq!(
            red.info.as_ref().and_then(|i| i.author.as_deref()),
            Some("Someone")
        );
    }

    #[test]
    fn nested_variants_need_a_root_without_game_files_and_direct_subfolders() {
        // Game files of its own: the subfolder stays skipped, as before.
        let entries = listing(&[
            "Mod/modinfo.ini",
            "Mod/natives/stm/a.mesh.260421070",
            "Mod/Optional/natives/stm/b.mesh.260421070",
        ]);
        let infos = modinfos(&[("Mod", info(&[("name", "Lantern Glow")]))]);
        let (options, problems) = build_options(&package(), "Mod", &entries, &infos);
        assert_eq!(options.len(), 1);
        assert_eq!(options[0].payload.len(), 1);
        assert_eq!(options[0].skipped[1].reason, SkipReason::UnknownPlace);
        assert_eq!(problems[0].kind, ProblemKind::FilesSkipped);

        // A pak directly in the root is a game file of its own too.
        let entries = listing(&[
            "Mod/modinfo.ini",
            "Mod/Lantern Glow.pak",
            "Mod/Optional/natives/stm/b.mesh.260421070",
        ]);
        let (options, _) = build_options(&package(), "Mod", &entries, &infos);
        assert_eq!(options.len(), 1);
        assert_eq!(options[0].kinds.pak, 1);

        // An anchor two folders down is not a direct subfolder.
        let entries = listing(&[
            "Mod/modinfo.ini",
            "Mod/Variants/Red/natives/stm/c.mesh.260421070",
        ]);
        let (options, problems) = build_options(&package(), "Mod", &entries, &infos);
        assert_eq!(options.len(), 1);
        assert!(options[0].menu_only);
        assert_eq!(options[0].skipped[1].reason, SkipReason::UnknownPlace);
        assert_eq!(
            problems.iter().map(|p| p.kind).collect::<Vec<_>>(),
            [ProblemKind::NothingToInstall, ProblemKind::FilesSkipped]
        );

        // A subfolder with its own modinfo is an ordinary option, never a variant.
        let entries = listing(&[
            "Mod/modinfo.ini",
            "Mod/Red/modinfo.ini",
            "Mod/Red/natives/stm/c.mesh.260421070",
        ]);
        let infos = modinfos(&[
            ("Mod", info(&[("name", "Lantern Glow")])),
            ("Mod/Red", info(&[("name", "Lantern Glow - Red")])),
        ]);
        let (options, _) = build_options(&package(), "Mod", &entries, &infos);
        let red = by_name(&options, "Lantern Glow - Red");
        assert_eq!(red.parent, None);
        assert_eq!(red.addon_for, None);
    }

    #[test]
    fn nested_variants_find_their_parent_through_the_tree_even_beside_a_namesake() {
        // Another option of the same package is also called "Hoods" and sorts
        // first; the variants still hang under the menu whose folder holds them.
        let (mut entries, mut infos) = hood_pack();
        entries.extend(listing(&[
            "Vernworth Cloaks Collection/Archive/modinfo.ini",
            "Vernworth Cloaks Collection/Archive/natives/stm/old_hood.mesh.260421070",
        ]));
        infos.insert(
            "Vernworth Cloaks Collection/Archive".to_string(),
            info(&[("name", "Hoods"), ("MenuPriority", "9")]),
        );
        let (options, problems) =
            build_options(&package(), "Vernworth Cloaks Collection", &entries, &infos);
        let menu_id = OptionId::new(&package(), "Vernworth Cloaks Collection/Hoods");
        let mut packages = vec![rift_core::Package {
            id: package(),
            path: "/mods/Vernworth Cloaks Collection.zip".into(),
            file_name: "Vernworth Cloaks Collection.zip".into(),
            display_name: "Vernworth Cloaks Collection".into(),
            format: rift_core::PackageFormat::Zip,
            size: 4096,
            modified_ms: 0,
            entry_count: entries.len(),
            options,
            nexus: None,
            problems,
        }];
        crate::tree::resolve(&mut packages);
        let options = &packages[0].options;
        assert_eq!(options[0].root, "Vernworth Cloaks Collection/Archive");
        let ashen = by_name(options, "Ashen Hood");
        assert_eq!(ashen.parent.as_ref(), Some(&menu_id));
        assert_eq!(
            ashen.trail,
            ["Vernworth Cloaks Collection", "Headwear", "Hoods"]
        );
    }

    // -----------------------------------------------------------------------
    // Outdated formats
    // -----------------------------------------------------------------------

    #[test]
    fn outdated_loose_files_are_counted_once_per_package_and_point_at_the_fix() {
        let entries = listing(&[
            "Old Capes/Red/natives/stm/cape/cape_301.mesh.240423143",
            "Old Capes/Red/natives/stm/cape/cape_301_alba.tex.760230703",
            "Old Capes/Blue/natives/stm/cape/cape_302.mesh.231011879",
            "Old Capes/Blue/natives/stm/cape/cape_302_alba.tex.251211553",
            "Old Capes/Blue/reframework/data/cape_302.mesh.240423143",
        ]);
        let (options, problems) =
            build_options(&package(), "Old Capes", &entries, &BTreeMap::new());
        assert_eq!(options.len(), 2);
        assert_eq!(
            problems.iter().map(|p| p.kind).collect::<Vec<_>>(),
            [ProblemKind::OutdatedFormats],
            "one problem for the whole package"
        );
        let message = &problems[0].message;
        assert!(
            message.starts_with("3 loose files (mesh, tex) were made for Dragon's Dogma 2"),
            "only loose payload counts, not the REFramework copy: {message}"
        );
        for needle in ["September 2026 engine update", "TU 3.2", "DD2 Pak Bridge"] {
            assert!(
                message.contains(needle),
                "{needle:?} missing from {message}"
            );
        }
    }

    #[test]
    fn one_outdated_file_reads_in_the_singular_and_more_in_the_plural() {
        let one = listing(&["Old Cape/natives/stm/cape/cape_301.mesh.240423143"]);
        let (_, problems) = build_options(&package(), "Old Cape", &one, &BTreeMap::new());
        assert_eq!(
            problems[0].message,
            "1 loose file (mesh) was made for Dragon's Dogma 2 before the September 2026 engine \
             update (TU 3.2). DD2 Pak Bridge is the likely fix; otherwise look for an updated \
             version of this mod."
        );

        let two = listing(&[
            "Old Cape/natives/stm/cape/cape_301.mesh.240423143",
            "Old Cape/natives/stm/cape/cape_301_alba.tex.760230703",
        ]);
        let (_, problems) = build_options(&package(), "Old Cape", &two, &BTreeMap::new());
        assert_eq!(
            problems[0].message,
            "2 loose files (mesh, tex) were made for Dragon's Dogma 2 before the September 2026 \
             engine update (TU 3.2). DD2 Pak Bridge is the likely fix; otherwise look for an \
             updated version of this mod."
        );
    }

    /// Chaos #00129 D3: the window opens each trouble card with its own verdict
    /// ("opened, but holds nothing the game loads: …"), so every message here
    /// is only the why that follows its colon.
    #[test]
    fn every_trouble_message_is_a_why_the_window_can_follow() {
        let (_, problems) = build_options(
            &package(),
            "Pictures",
            &listing(&["Docs/readme.txt", "image.png"]),
            &BTreeMap::new(),
        );
        assert_eq!(problems[0].kind, ProblemKind::NothingToInstall);
        crate::test_support::assert_is_a_why(&problems[0].message);
        assert_eq!(
            problems[0].message,
            "no natives/ or reframework/ folder, no .pak file and no file meant for the game \
             folder. If it should be a Dragon's Dogma 2 mod, check that the download is complete, \
             or look for a repacked version on its mod page."
        );

        for (in_folders, at_root) in [(1, 0), (3, 0), (0, 1), (0, 2), (2, 1), (2, 2)] {
            crate::test_support::assert_is_a_why(&files_skipped_message(in_folders, at_root));
        }
        assert_eq!(
            files_skipped_message(1, 0),
            "1 file sits in a folder the game never reads. The options list it under skipped \
             files; if it was meant for the game, the mod needs repacking."
        );
        assert_eq!(
            files_skipped_message(3, 0),
            "3 files sit in folders the game never reads. The options list them under skipped \
             files; if they were meant for the game, the mod needs repacking."
        );
        assert_eq!(
            files_skipped_message(2, 1),
            "2 files sit in folders the game never reads, and 1 is a program (or named like a \
             game folder). The options list them under skipped files; if they were meant for the \
             game, the mod needs repacking."
        );

        let assets: BTreeSet<&str> = ["mesh", "tex"].into_iter().collect();
        for outdated in [1, 4] {
            crate::test_support::assert_is_a_why(&outdated_formats_message(outdated, &assets));
        }
    }
}
