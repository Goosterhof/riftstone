//! Reading one package off disk into a [`Package`].
//!
//! **Candidates** ([`list_root`]): the direct children of a library root —
//! archives by extension (`zip`, `7z`, `rar`, any case; the magic bytes decide
//! when the package is opened) and folders (each child of a root is one
//! package, the Fluffy convention). Dotfiles, `desktop.ini` and partial
//! downloads (`*.crdownload`, `*.part`, `*.tmp`) are never candidates.
//!
//! **One package** ([`scan_package`]): open → list → (a Downloads-style root
//! stops here for anything that does not look like a DD2 mod) → read every
//! `modinfo.ini` (at most 64 KiB each; rift-archive's sibling cache turns the
//! reads of a solid 7z or rar into one pass) → classify.
//!
//! **Names and Nexus links.** The display name is the file name without its
//! archive extension — unless the name is a Nexus download name
//! ([`rift_core::nexus_name`]): then the mod name it carries is the display
//! name, and its mod id, version, upload time and sqid become the package's
//! Nexus link (source `Filename`). The very old shape, which carries no
//! upload time (`Lantern Glow-123-1-2.zip`), is a link only when one of the
//! package's own `homepage=` lines names the same mod: alone it reads just as
//! well as a person's `Cloaks Collection-12-3.zip`, and a wrong link would aim
//! update checks at an unrelated mod. Unvouched, the package keeps its plain
//! name and no filename link (main-session ruling, 2026-10-16). A package
//! without a filename link is linked by its options' `homepage=` lines
//! instead (source `Homepage`): the `nexusmods.com/dragonsdogma2/mods/<id>`
//! most of them name, carrying the modinfo `version` beside it.
//!
//! **Size and time** — the cache key ([`crate::catalog`]). An archive's are
//! its file's own. A folder's size is the sum of its files, and its time the
//! newest of its own, its subfolders' and its files': a texture swapped deep
//! inside a folder package changes the folder's key, where the folder's own
//! timestamp would not.

use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use walkdir::WalkDir;

use rift_core::nexus_name;
use rift_core::path::{file_name, parent};
use rift_core::{
    lower_key, ArchiveEntry, LibraryRoot, LinkSource, ModInfo, ModOption, NexusLink, Package,
    PackageFormat, PackageId, PackageSource, Problem, ProblemKind, RiftError,
};

use crate::{classify, modinfo};

/// How much of one `modinfo.ini` is read. Real ones are a few hundred bytes;
/// the cap keeps a mislabelled giant from costing the scan anything.
pub const MODINFO_READ_LIMIT: u64 = 64 * 1024;

const MODINFO_FILE_NAME: &str = "modinfo.ini";
const ARCHIVE_EXTENSIONS: &[&str] = &["zip", "7z", "rar"];
/// Downloads still in flight (Chromium, Firefox, everyone else).
const PARTIAL_DOWNLOAD_SUFFIXES: &[&str] = &[".crdownload", ".part", ".tmp"];
/// How deep a folder's stamp walk goes: rift-archive's folder source lists no
/// deeper, so nothing below can change what the package holds.
const FOLDER_MAX_DEPTH: usize = 64;

/// A package plus its full file listing (the listing stays backend-side; the
/// window asks for it per package).
#[derive(Debug, Clone)]
pub struct ScannedPackage {
    pub package: Package,
    pub entries: Vec<ArchiveEntry>,
}

/// The candidate packages directly inside a root: every archive file and
/// every folder (one level deep — each child of a root is one package, the
/// Fluffy convention). Sorted by name (case-blind). A root that cannot be
/// listed yields nothing.
pub fn list_root(root: &LibraryRoot) -> Vec<PathBuf> {
    let reader = match fs::read_dir(&root.path) {
        Ok(reader) => reader,
        Err(err) => {
            log::warn!(
                "the library folder {} could not be listed: {err}",
                root.path.display()
            );
            return Vec::new();
        }
    };
    let mut candidates: Vec<(String, PathBuf)> = Vec::new();
    for item in reader {
        let item = match item {
            Ok(item) => item,
            Err(err) => {
                log::warn!(
                    "{}: skipped an unreadable entry: {err}",
                    root.path.display()
                );
                continue;
            }
        };
        let name = item.file_name().to_string_lossy().into_owned();
        if is_never_a_package(&name) {
            continue;
        }
        let path = item.path();
        let is_folder = match item.file_type() {
            // A link or junction counts as what it points at.
            Ok(kind) if !kind.is_symlink() => kind.is_dir(),
            _ => path.is_dir(),
        };
        if is_folder || has_archive_extension(&name) {
            candidates.push((lower_key(&name), path));
        }
    }
    candidates.sort_unstable_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    candidates.into_iter().map(|(_, path)| path).collect()
}

/// Open, list and classify one package. For a `dd2_shaped_only` root, a
/// package that is not DD2-shaped returns `Ok(None)`.
pub fn scan_package(root: &LibraryRoot, path: &Path) -> Result<Option<ScannedPackage>, RiftError> {
    let stamp = stamp(path).map_err(StampError::into_error)?;
    scan_stamped(root, path, stamp)
}

// ---------------------------------------------------------------------------
// Stamps: size + time, the cache key
// ---------------------------------------------------------------------------

/// A candidate's size and modification time (see the module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Stamp {
    pub(crate) size: u64,
    pub(crate) modified_ms: u64,
    pub(crate) is_folder: bool,
}

#[derive(Debug)]
pub(crate) enum StampError {
    /// The candidate is no longer there (deleted or moved since the listing).
    Gone(RiftError),
    /// It is there but could not be measured.
    Unreadable(RiftError),
}

impl StampError {
    pub(crate) fn into_error(self) -> RiftError {
        match self {
            StampError::Gone(err) | StampError::Unreadable(err) => err,
        }
    }
}

/// Measure a candidate. A folder is walked (no links followed, as deep as
/// rift-archive lists); any error inside it makes the whole folder unreadable,
/// as it does for the folder source.
pub(crate) fn stamp(path: &Path) -> Result<Stamp, StampError> {
    let metadata = fs::metadata(path).map_err(|err| {
        let gone = err.kind() == io::ErrorKind::NotFound;
        let err = RiftError::unreadable(path, err);
        if gone {
            StampError::Gone(err)
        } else {
            StampError::Unreadable(err)
        }
    })?;
    if !metadata.is_dir() {
        return Ok(Stamp {
            size: metadata.len(),
            modified_ms: modified_ms(metadata.modified()),
            is_folder: false,
        });
    }

    let mut size = 0u64;
    let mut newest = modified_ms(metadata.modified());
    let walk = WalkDir::new(path)
        .follow_links(false)
        .min_depth(1)
        .max_depth(FOLDER_MAX_DEPTH);
    for item in walk {
        let item = item.map_err(|err| {
            let at = err.path().unwrap_or(path).to_path_buf();
            StampError::Unreadable(RiftError::unreadable(&at, err))
        })?;
        let kind = item.file_type();
        if !kind.is_file() && !kind.is_dir() {
            continue; // links are never part of a package
        }
        let metadata = item
            .metadata()
            .map_err(|err| StampError::Unreadable(RiftError::unreadable(item.path(), err)))?;
        if kind.is_file() {
            size = size.saturating_add(metadata.len());
        }
        newest = newest.max(modified_ms(metadata.modified()));
    }
    Ok(Stamp {
        size,
        modified_ms: newest,
        is_folder: true,
    })
}

/// Milliseconds since the epoch; 0 when the platform keeps no time or the
/// time predates 1970.
fn modified_ms(modified: io::Result<SystemTime>) -> u64 {
    modified
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        })
}

// ---------------------------------------------------------------------------
// One package
// ---------------------------------------------------------------------------

/// [`scan_package`] with the stamp already taken (the catalog takes it first,
/// to decide whether the cache can answer).
pub(crate) fn scan_stamped(
    root: &LibraryRoot,
    path: &Path,
    stamp: Stamp,
) -> Result<Option<ScannedPackage>, RiftError> {
    let mut source = rift_archive::open(path)?;
    let format = source.format();
    let entries = source.entries()?;
    if root.dd2_shaped_only && !classify::is_dd2_shaped(&entries) {
        return Ok(None);
    }

    let is_folder = format == PackageFormat::Folder;
    let modinfos = read_modinfos(source.as_mut(), &entries);
    let naming = Naming::of(path, is_folder).vouched_by(modinfos.infos.values());
    let id = package_id(path, &naming.file_name, is_folder, stamp.size);
    let (options, mut problems) =
        classify::build_options(&id, &naming.display_name, &entries, &modinfos.infos);
    problems.extend(modinfos.problems);
    let nexus = naming.nexus.or_else(|| homepage_link(&options));

    Ok(Some(ScannedPackage {
        package: Package {
            id,
            root_id: root.id.clone(),
            path: path.to_path_buf(),
            file_name: naming.file_name,
            display_name: naming.display_name,
            format,
            size: stamp.size,
            modified_ms: stamp.modified_ms,
            entry_count: entries.len(),
            options,
            nexus,
            problems,
        },
        entries,
    }))
}

/// The package a candidate becomes when it cannot be opened: named and linked
/// like any other, no options, one `Unreadable` problem saying why.
pub(crate) fn unreadable_package(
    root: &LibraryRoot,
    path: &Path,
    stamp: Option<Stamp>,
    error: &RiftError,
) -> Package {
    let is_folder = stamp.map_or_else(|| path.is_dir(), |stamp| stamp.is_folder);
    let naming = Naming::of(path, is_folder);
    let size = stamp.map_or(0, |stamp| stamp.size);
    Package {
        id: package_id(path, &naming.file_name, is_folder, size),
        root_id: root.id.clone(),
        path: path.to_path_buf(),
        format: guess_format(&naming.file_name, is_folder),
        file_name: naming.file_name,
        display_name: naming.display_name,
        size,
        modified_ms: stamp.map_or(0, |stamp| stamp.modified_ms),
        entry_count: 0,
        options: Vec::new(),
        nexus: naming.nexus,
        problems: vec![unreadable_problem(error)],
    }
}

/// The `Unreadable` why for each way opening can fail. The window says the
/// verdict ("The stone couldn't open `x`:"), so this is only what follows its
/// colon (Chaos #00129 D3).
fn unreadable_problem(error: &RiftError) -> Problem {
    let message = match error {
        RiftError::Encrypted { .. } => "it is password-protected. Unpack it with its password and \
             put the unpacked folder in a library folder instead."
            .to_string(),
        RiftError::UnsupportedFormat { .. } => "it is not a zip, 7z or rar archive, whatever its \
             name says. It may be an incomplete or mislabelled download: download it again from \
             its mod page."
            .to_string(),
        RiftError::CorruptArchive { detail, .. } => {
            format!("it is damaged ({detail}). Download it again from its mod page.")
        }
        RiftError::Unreadable { detail, .. } => format!(
            "the disk or another program refused the read ({detail}). If another program holds \
             it open, close that program and rescan."
        ),
        other => format!("the reader stopped ({other}). Download it again from its mod page."),
    };
    Problem {
        kind: ProblemKind::Unreadable,
        message,
    }
}

/// What a package is called and what its file name links it to.
struct Naming {
    file_name: String,
    display_name: String,
    nexus: Option<NexusLink>,
    /// A very old Nexus name (no upload time), waiting for a `homepage=` line
    /// to vouch for it ([`Naming::vouched_by`]).
    unvouched: Option<nexus_name::NexusFileName>,
}

impl Naming {
    fn of(path: &Path, is_folder: bool) -> Self {
        let file_name = path.file_name().map_or_else(
            || path.to_string_lossy().into_owned(),
            |name| name.to_string_lossy().into_owned(),
        );
        let parsed = nexus_name::parse(&file_name);
        if let Some(parsed) = parsed
            .as_ref()
            .filter(|parsed| parsed.uploaded_ms.is_some())
        {
            return Naming {
                display_name: parsed.name.clone(),
                nexus: Some(parsed.link()),
                file_name,
                unvouched: None,
            };
        }
        let stem = if is_folder {
            file_name.as_str()
        } else {
            strip_archive_extension(&file_name)
        }
        .trim();
        Naming {
            display_name: if stem.is_empty() {
                file_name.clone()
            } else {
                stem.to_string()
            },
            nexus: None,
            file_name,
            unvouched: parsed,
        }
    }

    /// Trust a very old Nexus name when a `homepage=` line of the package
    /// names the same Dragon's Dogma 2 mod; otherwise drop it.
    fn vouched_by<'a>(mut self, infos: impl IntoIterator<Item = &'a ModInfo>) -> Self {
        if let Some(parsed) = self.unvouched.take() {
            let vouched = infos.into_iter().any(|info| {
                info.homepage
                    .as_deref()
                    .and_then(nexus_name::mod_id_from_url)
                    == Some(parsed.mod_id)
            });
            if vouched {
                self.display_name = parsed.name.clone();
                self.nexus = Some(parsed.link());
            }
        }
        self
    }
}

/// Archives by file name + size, folders by absolute path (rift-core's rule).
fn package_id(path: &Path, file_name: &str, is_folder: bool, size: u64) -> PackageId {
    if is_folder {
        let absolute = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        PackageId::for_folder(&absolute)
    } else {
        PackageId::for_archive(file_name, size)
    }
}

/// The format a package that never opened most likely has.
fn guess_format(file_name: &str, is_folder: bool) -> PackageFormat {
    if is_folder {
        return PackageFormat::Folder;
    }
    match file_name
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
    {
        Some(ext) if ext == "7z" => PackageFormat::SevenZip,
        Some(ext) if ext == "rar" => PackageFormat::Rar,
        _ => PackageFormat::Zip,
    }
}

// ---------------------------------------------------------------------------
// modinfo.ini
// ---------------------------------------------------------------------------

struct Modinfos {
    /// Folder (as spelled, `""` = the package root) → parsed modinfo.
    infos: BTreeMap<String, ModInfo>,
    problems: Vec<Problem>,
}

/// Read and parse every `modinfo.ini`. One that cannot be read still marks its
/// folder as an option root (with an empty modinfo); garbled and unreadable
/// files each earn one `ModinfoGarbled` problem naming the count.
fn read_modinfos(source: &mut dyn PackageSource, entries: &[ArchiveEntry]) -> Modinfos {
    let mut infos = BTreeMap::new();
    let mut garbled = 0usize;
    let mut unreadable: Vec<RiftError> = Vec::new();
    for entry in entries {
        if !file_name(&entry.path).eq_ignore_ascii_case(MODINFO_FILE_NAME) {
            continue;
        }
        let info = match source.read_entry(&entry.path, Some(MODINFO_READ_LIMIT)) {
            Ok(bytes) => {
                let parsed = modinfo::parse(&bytes);
                garbled += usize::from(parsed.garbled);
                parsed.info
            }
            Err(err) => {
                log::warn!("{}: {err}", entry.path);
                unreadable.push(err);
                ModInfo::default()
            }
        };
        infos.entry(parent(&entry.path).to_string()).or_insert(info);
    }

    let mut problems = Vec::new();
    if garbled > 0 {
        problems.push(Problem {
            kind: ProblemKind::ModinfoGarbled,
            message: if garbled == 1 {
                "1 modinfo.ini in this package could not be decoded cleanly, so its option's name \
                 or description may show stray characters. Saving that file as UTF-8 fixes it."
                    .to_string()
            } else {
                format!(
                    "{garbled} modinfo.ini files in this package could not be decoded cleanly, so \
                     their options' names or descriptions may show stray characters. Saving those \
                     files as UTF-8 fixes it."
                )
            },
        });
    }
    if let Some(first) = unreadable.first() {
        let reason = short_reason(first);
        problems.push(Problem {
            kind: ProblemKind::ModinfoGarbled,
            message: if unreadable.len() == 1 {
                format!(
                    "1 modinfo.ini in this package could not be read ({reason}), so its option \
                     shows its folder's name and no description. If the archive is damaged, \
                     download it again from its mod page."
                )
            } else {
                format!(
                    "{} modinfo.ini files in this package could not be read (the first: {reason}), \
                     so their options show their folders' names and no descriptions. If the \
                     archive is damaged, download it again from its mod page.",
                    unreadable.len()
                )
            },
        });
    }
    Modinfos { infos, problems }
}

/// Why one entry could not be read, without the package path the window
/// already shows.
fn short_reason(error: &RiftError) -> String {
    match error {
        RiftError::Encrypted { .. } => "it is password-protected".to_string(),
        RiftError::CorruptArchive { detail, .. } | RiftError::Unreadable { detail, .. } => {
            detail.clone()
        }
        RiftError::EntryNotFound { .. } => "it is missing from the archive".to_string(),
        other => other.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Nexus link from homepage= lines
// ---------------------------------------------------------------------------

/// The Dragon's Dogma 2 mod page the options' `homepage=` lines name: the id
/// most of them name; a tie goes to the page named nearest the package root,
/// then to the first named. The version is the modinfo `version` of the
/// option nearest the package root that names the page and has one.
fn homepage_link(options: &[ModOption]) -> Option<NexusLink> {
    struct Vote<'a> {
        mod_id: u64,
        count: usize,
        /// The shallowest option root naming the page.
        depth: usize,
        first: usize,
        /// The version of the shallowest option naming the page WITH one.
        version: Option<(usize, &'a str)>,
    }
    let mut votes: Vec<Vote<'_>> = Vec::new();
    for (index, option) in options.iter().enumerate() {
        let Some(info) = option.info.as_ref() else {
            continue;
        };
        let Some(mod_id) = info
            .homepage
            .as_deref()
            .and_then(nexus_name::mod_id_from_url)
        else {
            continue;
        };
        let depth = if option.root.is_empty() {
            0
        } else {
            option.root.split('/').count()
        };
        let version = info.version.as_deref().map(|version| (depth, version));
        match votes.iter_mut().find(|vote| vote.mod_id == mod_id) {
            Some(vote) => {
                vote.count += 1;
                vote.depth = vote.depth.min(depth);
                if version.is_some_and(|(at, _)| vote.version.is_none_or(|(best, _)| at < best)) {
                    vote.version = version;
                }
            }
            None => votes.push(Vote {
                mod_id,
                count: 1,
                depth,
                first: index,
                version,
            }),
        }
    }
    let best = votes
        .into_iter()
        .min_by_key(|vote| (Reverse(vote.count), vote.depth, vote.first))?;
    Some(NexusLink {
        mod_id: best.mod_id,
        file_id: None,
        group_id: None,
        sqid: None,
        version: best.version.map(|(_, version)| version.to_string()),
        uploaded_ms: None,
        source: LinkSource::Homepage,
    })
}

// ---------------------------------------------------------------------------
// Names
// ---------------------------------------------------------------------------

fn is_never_a_package(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    name.starts_with('.')
        || lower == "desktop.ini"
        || PARTIAL_DOWNLOAD_SUFFIXES
            .iter()
            .any(|suffix| lower.ends_with(suffix))
}

fn has_archive_extension(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(stem, ext)| {
        !stem.is_empty()
            && ARCHIVE_EXTENSIONS
                .iter()
                .any(|known| ext.eq_ignore_ascii_case(known))
    })
}

/// `Lantern Glow.ZIP` → `Lantern Glow`; any other name is kept whole.
fn strip_archive_extension(name: &str) -> &str {
    match name.rsplit_once('.') {
        Some((stem, ext))
            if !stem.is_empty()
                && ARCHIVE_EXTENSIONS
                    .iter()
                    .any(|known| ext.eq_ignore_ascii_case(known)) =>
        {
            stem
        }
        _ => name,
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use rift_archive::SevenZipSource;
    use rift_core::DeployTarget;

    use super::*;
    use crate::test_support::{
        folder_at, mark_zip_entry_encrypted, modinfo, root, set_mtime, solid_7z_at, zip_at,
    };

    const MESH: &[u8] = b"mesh bytes";
    /// 2026-10-14T14:23Z, the upload stamp of the invented 2026 name.
    const VERNWORTH_UPLOAD_MS: u64 = 1_791_987_780_000;

    fn names(paths: &[PathBuf]) -> Vec<String> {
        paths
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn list_root_keeps_archives_and_folders_by_name_and_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        for file in [
            "b Lantern Glow.zip",
            "A Vernworth Cape.7Z",
            "c Pawn Satchel.RAR",
            ".hidden.zip",
            "desktop.ini",
            "Vernworth Cloaks.zip.crdownload",
            "Lantern Glow.7z.part",
            "Pawn Satchel.rar.tmp",
            "readme.txt",
            "setup.exe",
            "notes.zip.txt",
            ".zip",
        ] {
            fs::write(dir.path().join(file), b"x").unwrap();
        }
        for folder in ["Lantern Glow Recolor", ".git", "half done.tmp"] {
            fs::create_dir(dir.path().join(folder)).unwrap();
        }
        let listed = list_root(&root("fluffy", dir.path(), false));
        assert_eq!(
            names(&listed),
            [
                "A Vernworth Cape.7Z",
                "b Lantern Glow.zip",
                "c Pawn Satchel.RAR",
                "Lantern Glow Recolor",
            ]
        );
        assert!(listed.iter().all(|path| path.starts_with(dir.path())));

        let missing = root("gone", &dir.path().join("no such folder"), false);
        assert!(list_root(&missing).is_empty());
    }

    #[test]
    fn a_zip_collection_with_a_2026_nexus_name() {
        let dir = tempfile::tempdir().unwrap();
        let file_name = "Vernworth Cloaks 2207 8 2026-10-14T14-23Z Rk4vLmN2Q.zip";
        let path = dir.path().join(file_name);
        let cloaks = "Vernworth Cloaks/Cloaks";
        let menu = modinfo(&[("name", "Cloaks"), ("DummyMod", "true")]);
        let white = modinfo(&[
            ("name", "Ermine Cape - White"),
            ("addonfor", "Cloaks"),
            (
                "homepage",
                "https://www.nexusmods.com/dragonsdogma2/mods/999",
            ),
        ]);
        zip_at(
            &path,
            &[
                ("Vernworth Cloaks/README.txt", b"read me"),
                (&format!("{cloaks}/modinfo.ini"), &menu),
                (&format!("{cloaks}/White/modinfo.ini"), &white),
                (&format!("{cloaks}/White/screenshot.png"), b"png"),
                (
                    &format!("{cloaks}/White/natives/stm/cape/cloak_301.mesh.260421070"),
                    MESH,
                ),
            ],
        );
        set_mtime(
            &path,
            SystemTime::UNIX_EPOCH + Duration::from_millis(1_700_000_000_000),
        );

        let scanned = scan_package(&root("fluffy", dir.path(), false), &path)
            .unwrap()
            .expect("a normal root keeps every package");
        let package = &scanned.package;
        let size = fs::metadata(&path).unwrap().len();
        assert_eq!(package.id, PackageId::for_archive(file_name, size));
        assert_eq!(package.root_id, "fluffy");
        assert_eq!(package.path, path);
        assert_eq!(package.file_name, file_name);
        assert_eq!(package.display_name, "Vernworth Cloaks");
        assert_eq!(package.format, PackageFormat::Zip);
        assert_eq!(
            (package.size, package.modified_ms),
            (size, 1_700_000_000_000)
        );
        assert_eq!(package.entry_count, 5);
        assert_eq!(scanned.entries.len(), 5);
        assert!(package.problems.is_empty(), "{:?}", package.problems);

        let nexus = package.nexus.as_ref().expect("the file name links it");
        assert_eq!(nexus.source, LinkSource::Filename);
        assert_eq!(nexus.mod_id, 2207, "the file name wins over homepage=");
        assert_eq!(nexus.version.as_deref(), Some("8"));
        assert_eq!(nexus.sqid.as_deref(), Some("Rk4vLmN2Q"));
        assert_eq!(nexus.uploaded_ms, Some(VERNWORTH_UPLOAD_MS));

        let names: Vec<&str> = package.options.iter().map(|o| o.name.as_str()).collect();
        assert_eq!(names, ["Cloaks", "Ermine Cape - White"]);
        let white = &package.options[1];
        assert_eq!(
            white.trail,
            ["Vernworth Cloaks"],
            "the tree is the catalog's job"
        );
        assert_eq!(
            white.payload[0].target,
            DeployTarget::Loose {
                game_path: "natives/stm/cape/cloak_301.mesh.260421070".into()
            }
        );
    }

    #[test]
    fn homepage_lines_link_a_package_whose_name_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Lantern Glow Recolor.ZIP");
        let page = |id: u32| format!("https://www.nexusmods.com/dragonsdogma2/mods/{id}?tab=files");
        let (p45, p321) = (page(45), page(321));
        zip_at(
            &path,
            &[
                (
                    "Lantern Glow/modinfo.ini",
                    &modinfo(&[
                        ("name", "Lantern Glow"),
                        ("homepage", &p321),
                        ("version", "1.0"),
                    ]),
                ),
                ("Lantern Glow/Lantern Glow.pak", b"KPKA"),
                (
                    "Lantern Glow/Ember/modinfo.ini",
                    &modinfo(&[("name", "Ember"), ("homepage", &p45), ("version", "2.0")]),
                ),
                ("Lantern Glow/Ember/natives/stm/ember.tex.251211553", MESH),
                (
                    "Lantern Glow/Azure/modinfo.ini",
                    &modinfo(&[("name", "Azure"), ("homepage", &p45)]),
                ),
                ("Lantern Glow/Azure/natives/stm/azure.tex.251211553", MESH),
            ],
        );
        let package = scan_package(&root("fluffy", dir.path(), false), &path)
            .unwrap()
            .unwrap()
            .package;
        assert_eq!(
            package.display_name, "Lantern Glow Recolor",
            "the extension goes, any case"
        );
        let nexus = package.nexus.expect("homepage= links it");
        assert_eq!(
            (nexus.mod_id, nexus.source),
            (45, LinkSource::Homepage),
            "the page most options name"
        );
        assert_eq!(
            nexus.version.as_deref(),
            Some("2.0"),
            "from an option naming that page"
        );
        assert_eq!(
            (nexus.file_id, nexus.uploaded_ms, nexus.sqid),
            (None, None, None)
        );

        // No Dragon's Dogma 2 page anywhere: no link.
        let other = dir.path().join("Pawn Satchel.zip");
        zip_at(
            &other,
            &[
                (
                    "modinfo.ini",
                    &modinfo(&[("homepage", "https://www.nexusmods.com/skyrim/mods/45")]),
                ),
                ("natives/stm/satchel.mesh.260421070", MESH),
            ],
        );
        let package = scan_package(&root("fluffy", dir.path(), false), &other)
            .unwrap()
            .unwrap()
            .package;
        assert_eq!(package.nexus, None);
        assert_eq!(package.options[0].name, "Pawn Satchel");
    }

    #[test]
    fn a_very_old_nexus_name_links_only_when_a_homepage_line_agrees() {
        let dir = tempfile::tempdir().unwrap();
        let fluffy = root("fluffy", dir.path(), false);
        let scan = |file_name: &str, homepage: Option<&str>| {
            let path = dir.path().join(file_name);
            let mut info = vec![("name", "Ermine Cape")];
            if let Some(page) = homepage {
                info.push(("homepage", page));
            }
            zip_at(
                &path,
                &[
                    ("modinfo.ini", &modinfo(&info)),
                    ("natives/stm/cape/cloak.mesh.260421070", MESH),
                ],
            );
            scan_package(&fluffy, &path).unwrap().unwrap().package
        };

        // Alone, a person's own name that happens to end in -<id>-<number>.
        let alone = scan("Vernworth Cloaks Collection-12-3.zip", None);
        assert_eq!(alone.nexus, None);
        assert_eq!(alone.display_name, "Vernworth Cloaks Collection-12-3");

        // A homepage= line naming the same mod vouches for it.
        let vouched = scan(
            "Lantern Glow-123-1-2.zip",
            Some("https://www.nexusmods.com/dragonsdogma2/mods/123"),
        );
        assert_eq!(vouched.display_name, "Lantern Glow");
        let link = vouched.nexus.expect("vouched for");
        assert_eq!(
            (link.mod_id, link.version.as_deref(), link.source),
            (123, Some("1.2"), LinkSource::Filename)
        );

        // A homepage= line naming another mod: that page links it, not the name.
        let other = scan(
            "Pawn Satchel-123-1-2.zip",
            Some("https://www.nexusmods.com/dragonsdogma2/mods/45"),
        );
        assert_eq!(other.display_name, "Pawn Satchel-123-1-2");
        let link = other.nexus.expect("the homepage link");
        assert_eq!((link.mod_id, link.source), (45, LinkSource::Homepage));

        // The classic shape with its upload time still links on its own.
        let classic = scan("Ember Lantern-123-1-2-1711234567.zip", None);
        assert_eq!(classic.display_name, "Ember Lantern");
        assert_eq!(classic.nexus.map(|link| link.mod_id), Some(123));
    }

    #[test]
    fn a_folder_package_with_a_classic_nexus_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Lantern Glow-123-1-2-1711234567");
        folder_at(
            &path,
            &[
                ("modinfo.ini", &modinfo(&[("name", "Lantern Glow")])),
                ("natives/stm/gui/glow.tex.251211553", b"0123456789"),
                ("natives/stm/gui/deeper/glow_n.tex.251211553", b"01234"),
            ],
        );
        let deep = path.join("natives/stm/gui/deeper/glow_n.tex.251211553");
        let newest = SystemTime::UNIX_EPOCH + Duration::from_millis(1_900_000_000_000);
        set_mtime(&deep, newest);

        let scanned = scan_package(&root("fluffy", dir.path(), false), &path)
            .unwrap()
            .unwrap();
        let package = &scanned.package;
        assert_eq!(package.format, PackageFormat::Folder);
        assert_eq!(
            package.id,
            PackageId::for_folder(&std::path::absolute(&path).unwrap())
        );
        let modinfo_len = modinfo(&[("name", "Lantern Glow")]).len() as u64;
        assert_eq!(
            package.size,
            modinfo_len + 15,
            "a folder's size is the sum of its files"
        );
        assert_eq!(
            package.modified_ms, 1_900_000_000_000,
            "the newest time anywhere inside"
        );
        assert_eq!(package.display_name, "Lantern Glow");
        let nexus = package.nexus.as_ref().unwrap();
        assert_eq!(
            (nexus.mod_id, nexus.version.as_deref(), nexus.uploaded_ms),
            (123, Some("1.2"), Some(1_711_234_567_000))
        );
        assert_eq!(package.options[0].kinds.loose, 2);
    }

    #[test]
    fn a_folder_stamp_sees_a_change_deep_inside() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Lantern Glow Recolor");
        folder_at(
            &path,
            &[
                ("natives/stm/gui/glow.tex.251211553", b"red"),
                ("natives/stm/gui/glow_n.tex.251211553", b"blue"),
            ],
        );
        let before = stamp(&path).unwrap();
        assert!(before.is_folder);
        assert_eq!(before.size, 7, "the sum of the files");

        // Same size, new bytes, deep inside: no folder's own entry list moves,
        // only the file's time does.
        let file = path.join("natives/stm/gui/glow.tex.251211553");
        fs::write(&file, b"RED").unwrap();
        let later = SystemTime::now() + Duration::from_secs(86_400);
        set_mtime(&file, later);
        let after = stamp(&path).unwrap();
        assert_eq!(after.size, before.size);
        assert_eq!(
            after.modified_ms,
            modified_ms(Ok(later)),
            "the newest time anywhere inside"
        );

        assert!(matches!(
            stamp(&dir.path().join("vanished.zip")),
            Err(StampError::Gone(_))
        ));
    }

    #[test]
    fn downloads_roots_pass_over_what_is_not_a_dd2_mod() {
        let dir = tempfile::tempdir().unwrap();
        let photos = dir.path().join("Holiday Photos.zip");
        zip_at(&photos, &[("beach.jpg", b"jpg"), ("notes.txt", b"txt")]);
        let downloads = root("downloads", dir.path(), true);
        assert!(scan_package(&downloads, &photos).unwrap().is_none());

        // In an ordinary root the same archive is a package with nothing to install.
        let package = scan_package(&root("custom", dir.path(), false), &photos)
            .unwrap()
            .unwrap()
            .package;
        assert!(package.options.is_empty());
        assert_eq!(package.problems[0].kind, ProblemKind::NothingToInstall);

        let glow = dir.path().join("Lantern Glow.zip");
        zip_at(&glow, &[("Lantern Glow/Lantern Glow.pak", b"KPKA")]);
        assert!(scan_package(&downloads, &glow).unwrap().is_some());
    }

    #[test]
    fn garbled_and_unreadable_modinfos_are_named_but_keep_their_options() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Vernworth Banners.zip");
        zip_at(
            &path,
            &[
                // Shift-JIS read as Windows-1252: C1 controls survive.
                (
                    "Banners/Red/modinfo.ini",
                    b"name=\x83\x89\x83\x93\x81\x40Red\r\n",
                ),
                ("Banners/Red/natives/stm/banner_red.tex.251211553", MESH),
                (
                    "Banners/Blue/modinfo.ini",
                    &modinfo(&[("name", "Blue Banner")]),
                ),
                ("Banners/Blue/natives/stm/banner_blue.tex.251211553", MESH),
                (
                    "Banners/Green/modinfo.ini",
                    &modinfo(&[("name", "Green Banner")]),
                ),
                ("Banners/Green/natives/stm/banner_green.tex.251211553", MESH),
            ],
        );
        mark_zip_entry_encrypted(&path, "Banners/Green/modinfo.ini");
        let package = scan_package(&root("fluffy", dir.path(), false), &path)
            .unwrap()
            .unwrap()
            .package;
        let names: Vec<&str> = package.options.iter().map(|o| o.name.as_str()).collect();
        assert_eq!(names.len(), 3, "{names:?}");
        assert!(names.contains(&"Blue Banner"));
        assert!(
            names.contains(&"Green"),
            "an unreadable modinfo still makes an option, named after its folder: {names:?}"
        );
        let kinds: Vec<ProblemKind> = package.problems.iter().map(|p| p.kind).collect();
        assert_eq!(
            kinds,
            [ProblemKind::ModinfoGarbled, ProblemKind::ModinfoGarbled]
        );
        assert!(
            package.problems[0]
                .message
                .starts_with("1 modinfo.ini in this package could not be decoded"),
            "{}",
            package.problems[0].message
        );
        assert!(
            package.problems[1].message.starts_with(
                "1 modinfo.ini in this package could not be read (it is password-protected)"
            ),
            "{}",
            package.problems[1].message
        );
    }

    #[test]
    fn every_modinfo_of_a_solid_7z_comes_out_of_one_pass() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Vernworth Cloaks Collection.7z");
        let mut owned: Vec<(String, Vec<u8>)> = Vec::new();
        for n in 0..40 {
            let variant = format!("Cloaks/Cape {n:02}");
            owned.push((
                format!("{variant}/modinfo.ini"),
                modinfo(&[("name", &format!("Cape {n:02}"))]),
            ));
            owned.push((
                format!("{variant}/natives/stm/cape_{n:02}.mesh.260421070"),
                vec![b'm'; 3000],
            ));
        }
        let files: Vec<(&str, &[u8])> = owned
            .iter()
            .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
            .collect();
        solid_7z_at(&path, &files);

        let mut source = SevenZipSource::open(&path).unwrap();
        let entries = source.entries().unwrap();
        let modinfos = read_modinfos(&mut source, &entries);
        assert_eq!(modinfos.infos.len(), 40);
        assert_eq!(
            modinfos.infos["Cloaks/Cape 07"].name.as_deref(),
            Some("Cape 07")
        );
        assert!(modinfos.problems.is_empty());
        assert_eq!(source.decode_passes(), 1, "40 modinfo.ini files, one pass");

        // And the whole package through scan_package.
        let package = scan_package(&root("fluffy", dir.path(), false), &path)
            .unwrap()
            .unwrap()
            .package;
        assert_eq!(package.format, PackageFormat::SevenZip);
        assert_eq!(package.options.len(), 40);
    }

    #[test]
    fn a_package_that_never_opened_keeps_its_name_and_link() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Lantern Glow-123-1-2-1711234567.7z");
        fs::write(&path, b"not an archive at all").unwrap();
        let fluffy = root("fluffy", dir.path(), false);
        let error = scan_package(&fluffy, &path).unwrap_err();
        assert!(
            matches!(error, RiftError::UnsupportedFormat { .. }),
            "{error:?}"
        );

        let stamped = stamp(&path).unwrap();
        let package = unreadable_package(&fluffy, &path, Some(stamped), &error);
        assert_eq!(
            package.id,
            PackageId::for_archive("Lantern Glow-123-1-2-1711234567.7z", 21)
        );
        assert_eq!(package.display_name, "Lantern Glow");
        assert_eq!(package.format, PackageFormat::SevenZip);
        assert_eq!(package.nexus.as_ref().map(|n| n.mod_id), Some(123));
        assert!(package.options.is_empty());
        assert_eq!(package.problems.len(), 1);
        assert_eq!(package.problems[0].kind, ProblemKind::Unreadable);
        assert!(
            package.problems[0]
                .message
                .contains("not a zip, 7z or rar archive"),
            "{}",
            package.problems[0].message
        );

        for (error, needle) in [
            (
                RiftError::Encrypted { path: "x".into() },
                "password-protected",
            ),
            (
                RiftError::corrupt("x", "the central directory is cut off"),
                "it is damaged (the central directory is cut off)",
            ),
            (
                RiftError::unreadable("x", "Permission denied"),
                "(Permission denied)",
            ),
        ] {
            let message = unreadable_problem(&error).message;
            assert!(message.contains(needle), "{message}");
            assert!(
                !message.contains("x:"),
                "no package path in the sentence: {message}"
            );
        }
    }

    /// Chaos #00129 D3: the window says "The stone couldn't open `x.7z`:" and
    /// this sentence follows the colon, so it is only the why.
    #[test]
    fn every_unreadable_message_is_a_why_the_window_can_follow() {
        for error in [
            RiftError::Encrypted { path: "x".into() },
            RiftError::UnsupportedFormat { path: "x".into() },
            RiftError::corrupt("x", "the central directory is cut off"),
            RiftError::unreadable("x", "Permission denied"),
            RiftError::EntryNotFound {
                entry: "a/modinfo.ini".into(),
            },
        ] {
            let message = unreadable_problem(&error).message;
            crate::test_support::assert_is_a_why(&message);
            assert!(
                !message.contains("the Riftstone"),
                "one narrator, and the window names it: {message}"
            );
        }
    }

    #[test]
    fn names_and_extensions() {
        assert!(has_archive_extension("a.ZIP") && has_archive_extension("b.7z"));
        assert!(!has_archive_extension(".rar") && !has_archive_extension("c.tar"));
        assert_eq!(
            strip_archive_extension("Lantern Glow v1.2.rar"),
            "Lantern Glow v1.2"
        );
        assert_eq!(
            strip_archive_extension("Lantern Glow v1.2"),
            "Lantern Glow v1.2"
        );
        assert!(is_never_a_package("Lantern.ZIP.CRDOWNLOAD"));
        assert!(is_never_a_package("Desktop.ini"));
        assert!(!is_never_a_package("Lantern Glow.zip"));
        assert_eq!(guess_format("x.RAR", false), PackageFormat::Rar);
        assert_eq!(guess_format("x.zip", true), PackageFormat::Folder);
        assert_eq!(modified_ms(Err(io::Error::other("no clock"))), 0);
    }
}
