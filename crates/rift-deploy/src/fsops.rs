//! The small filesystem moves the deploy engine is built from.
//!
//! Three ideas live here. First, every move is a rename when it can be and a
//! copy-then-delete when the two paths sit on different volumes (the game on
//! `D:`, the app data on `C:`), and the copy lands under a temporary name so a
//! half-copied file never wears a final name. That temporary is always a NEW
//! file: a copy never follows a link or overwrites something already sitting
//! under the temporary name. Second, game paths are case-insensitive on
//! Windows: [`CaseResolver`] gives every path the spelling that is already on
//! disk, so the Linux bench behaves the way NTFS does. Third, the engine
//! removes only folders it created ([`missing_dirs`] before, [`prune_created`]
//! after).

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use rift_core::lower_key;
use rift_core::path::parent;
use sha2::{Digest, Sha256};

/// Suffix of a file the engine is still writing (extraction, cross-volume copies).
pub(crate) const PART_SUFFIX: &str = ".riftstone-part";
/// Marker of an uninstalled file waiting for the commit to delete it.
pub(crate) const TRASH_MARK: &str = ".riftstone-trash-";
/// Marker of a pak halfway through a reorder.
pub(crate) const REORDER_MARK: &str = ".riftstone-reorder-";

/// How much of each end of a file the fingerprint reads.
const FINGERPRINT_SPAN: u64 = 1 << 20;

/// Join a normalized game path (forward slashes) onto a root, one segment at
/// a time, so Windows never sees a mixed-separator path.
pub(crate) fn join_rel(root: &Path, rel: &str) -> PathBuf {
    let mut path = root.to_path_buf();
    for segment in rel.split('/').filter(|s| !s.is_empty()) {
        path.push(segment);
    }
    path
}

/// The same path with `suffix` appended to its last component.
pub(crate) fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut os = path.as_os_str().to_owned();
    os.push(suffix);
    PathBuf::from(os)
}

/// Is there anything at `path` (without following a final symlink)?
pub(crate) fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

/// Is there a plain file at `path` (a symlink does not count)?
pub(crate) fn is_file(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|m| m.is_file())
        .unwrap_or(false)
}

/// The size of the plain file at `path`.
pub(crate) fn file_len(path: &Path) -> Option<u64> {
    fs::symlink_metadata(path)
        .ok()
        .filter(|m| m.is_file())
        .map(|m| m.len())
}

/// Is there a real folder at `path` (a link to one does not count)?
pub(crate) fn is_real_dir(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|m| m.is_dir() && !is_link(&m))
        .unwrap_or(false)
}

/// Is this entry a symbolic link, a junction or any other reparse point —
/// something that leads somewhere else?
pub(crate) fn is_link(meta: &fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    false
}

/// Write `bytes` into a NEW file at `path`: fails when anything — a file, a
/// link, a folder — already sits there, so it can never write through a link
/// or over a file it did not create.
pub(crate) fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    if written.is_err() {
        drop(file);
        let _ = fs::remove_file(path);
    }
    written
}

/// Copy `from` into a NEW file at `to` (see [`write_new`]) and flush it.
fn copy_new(from: &Path, to: &Path) -> io::Result<()> {
    let mut source = fs::File::open(from)?;
    let mut out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(to)?;
    let copied = io::copy(&mut source, &mut out).and_then(|_| out.sync_all());
    if copied.is_err() {
        drop(out);
        let _ = fs::remove_file(to);
    }
    copied
}

/// A child of `dir` named `name`, compared case-insensitively. The exact
/// spelling is tried first (the only lookup a case-insensitive filesystem
/// needs); otherwise the folder is listed.
pub(crate) fn child_ci(dir: &Path, name: &str) -> Option<PathBuf> {
    let direct = dir.join(name);
    if exists(&direct) {
        return Some(direct);
    }
    let wanted = lower_key(name);
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .find(|entry| lower_key(&entry.file_name().to_string_lossy()) == wanted)
        .map(|entry| entry.path())
}

/// Rename within one volume. On Windows a file that just appeared is often
/// held for a moment by an antivirus scan; a sharing or lock violation is
/// retried a few times (about half a second in all) before it is an error.
pub(crate) fn rename(from: &Path, to: &Path) -> io::Result<()> {
    let mut attempt: u64 = 0;
    loop {
        match fs::rename(from, to) {
            Err(err) if attempt < 4 && is_transient_lock(&err) => {
                attempt += 1;
                std::thread::sleep(std::time::Duration::from_millis(50 * attempt));
            }
            other => return other,
        }
    }
}

fn is_transient_lock(err: &io::Error) -> bool {
    // ERROR_ACCESS_DENIED, ERROR_SHARING_VIOLATION, ERROR_LOCK_VIOLATION.
    cfg!(windows) && matches!(err.raw_os_error(), Some(5 | 32 | 33))
}

/// Move a file: rename, or copy-then-delete across volumes. Creates the
/// destination's parent folders. The destination must not exist.
pub(crate) fn move_file(from: &Path, to: &Path) -> io::Result<()> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    match rename(from, to) {
        Ok(()) => Ok(()),
        Err(err) if is_cross_device(&err) => copy_then_remove(from, to),
        Err(err) => Err(err),
    }
}

fn is_cross_device(err: &io::Error) -> bool {
    if err.kind() == io::ErrorKind::CrossesDevices {
        return true;
    }
    match err.raw_os_error() {
        #[cfg(windows)]
        Some(17) => true, // ERROR_NOT_SAME_DEVICE
        #[cfg(unix)]
        Some(18) => true, // EXDEV
        _ => false,
    }
}

fn copy_then_remove(from: &Path, to: &Path) -> io::Result<()> {
    let part = with_suffix(to, PART_SUFFIX);
    copy_new(from, &part)?;
    if let Err(err) = fs::rename(&part, to) {
        let _ = fs::remove_file(&part);
        return Err(err);
    }
    remove_file_if_exists(from).map(|_| ())
}

/// Move `from` over `to`, replacing what is there in one step — a rename on
/// one volume; across volumes a copy to `<to>.riftstone-part`, renamed over
/// `to`, then `from` deleted. `to` is never missing at any moment: when this
/// fails, `to` still holds what it held. Only for a `to` the caller has
/// proved is its own to replace.
pub(crate) fn replace_file(from: &Path, to: &Path) -> io::Result<()> {
    match rename(from, to) {
        Ok(()) => Ok(()),
        Err(err) if is_cross_device(&err) => {
            let part = with_suffix(to, PART_SUFFIX);
            copy_new(from, &part)?;
            if let Err(err) = fs::rename(&part, to) {
                let _ = fs::remove_file(&part);
                return Err(err);
            }
            remove_file_if_exists(from).map(|_| ())
        }
        Err(err) => Err(err),
    }
}

/// Delete a file if it is there. `Ok(true)` when something was deleted.
pub(crate) fn remove_file_if_exists(path: &Path) -> io::Result<bool> {
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        #[cfg(windows)]
        Err(err) if err.kind() == io::ErrorKind::PermissionDenied => {
            // A read-only attribute blocks deletion on Windows (renames are
            // unaffected). The file is ours to delete: clear it and retry.
            let mut permissions = fs::metadata(path)?.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            permissions.set_readonly(false);
            fs::set_permissions(path, permissions)?;
            fs::remove_file(path).map(|()| true)
        }
        Err(err) => Err(err),
    }
}

/// Remove empty folders from `dir` upwards, stopping at (and never removing)
/// `stop` or anything outside it. A folder that still holds anything stops
/// the climb.
pub(crate) fn prune_empty_dirs(dir: Option<&Path>, stop: &Path) {
    let mut current = dir;
    while let Some(folder) = current {
        if folder == stop || !folder.starts_with(stop) {
            break;
        }
        if fs::remove_dir(folder).is_err() {
            break;
        }
        current = folder.parent();
    }
}

/// The folders on the way to `game_path` (relative to `root`) that do not
/// exist yet, shallowest first, in the spelling given — the folders writing
/// `game_path` would create.
pub(crate) fn missing_dirs(root: &Path, game_path: &str) -> Vec<String> {
    let mut chain = Vec::new();
    let mut dir = parent(game_path);
    while !dir.is_empty() {
        chain.push(dir);
        dir = parent(dir);
    }
    chain.reverse();
    match chain.iter().position(|d| !exists(&join_rel(root, d))) {
        Some(first_missing) => chain[first_missing..]
            .iter()
            .map(|d| (*d).to_string())
            .collect(),
        None => Vec::new(),
    }
}

/// Remove `dir` (relative to `root`) and then its parents, for as long as
/// each one is empty AND listed in `created` — a folder the engine made. A
/// folder the player made, or one that still holds anything, stops the
/// climb. Pruned folders leave `created`.
pub(crate) fn prune_created(root: &Path, dir: &str, created: &mut Vec<String>) {
    let mut dir = dir;
    while !dir.is_empty() {
        let key = lower_key(dir);
        let Some(at) = created.iter().position(|d| lower_key(d) == key) else {
            break;
        };
        let path = join_rel(root, dir);
        if !is_real_dir(&path) || fs::remove_dir(&path).is_err() {
            break;
        }
        created.swap_remove(at);
        dir = parent(dir);
    }
}

/// Remove every empty folder inside `dir` (bottom-up), then `dir` itself if
/// it ended up empty. Files are never touched.
pub(crate) fn remove_empty_tree(dir: &Path) {
    if !is_real_dir(dir) {
        return;
    }
    for entry in walkdir::WalkDir::new(dir)
        .follow_links(false)
        .contents_first(true)
        .into_iter()
        .flatten()
    {
        if entry.file_type().is_dir() {
            let _ = fs::remove_dir(entry.path());
        }
    }
}

/// A cheap identity for a file's bytes: SHA-256 over its length, its first
/// MiB and its last MiB (the whole file below 2 MiB), 128 bits in hex. It
/// tells "the bytes this install wrote" from "something else wrote here
/// since" without reading a 2 GB pak end to end.
pub(crate) fn fingerprint(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let len = file.metadata()?.len();
    let mut hasher = Sha256::new();
    hasher.update(len.to_le_bytes());
    let head = len.min(FINGERPRINT_SPAN);
    hash_exact(&mut file, head, &mut hasher)?;
    if len > head {
        let tail = (len - head).min(FINGERPRINT_SPAN);
        file.seek(SeekFrom::Start(len - tail))?;
        hash_exact(&mut file, tail, &mut hasher)?;
    }
    let digest = hasher.finalize();
    Ok(digest.iter().take(16).map(|b| format!("{b:02x}")).collect())
}

fn hash_exact(file: &mut fs::File, count: u64, hasher: &mut Sha256) -> io::Result<()> {
    let mut buffer = vec![0u8; 64 * 1024];
    let mut left = count;
    while left > 0 {
        let want = usize::try_from(left.min(buffer.len() as u64)).unwrap_or(buffer.len());
        file.read_exact(&mut buffer[..want])?;
        hasher.update(&buffer[..want]);
        left -= want as u64;
    }
    Ok(())
}

/// Do two files hold the same bytes (by length and fingerprint)?
pub(crate) fn same_content(a: &Path, b: &Path) -> bool {
    match (file_len(a), file_len(b)) {
        (Some(la), Some(lb)) if la == lb => match (fingerprint(a), fingerprint(b)) {
            (Ok(fa), Ok(fb)) => fa == fb,
            _ => false,
        },
        _ => false,
    }
}

/// Gives a normalized game path the spelling already on disk, one segment at
/// a time — the way NTFS resolves `natives/STM` and `natives/stm` to one
/// folder. Segments that do not exist yet keep the spelling asked for, and
/// are remembered, so two paths of one operation that differ only in case
/// agree on one spelling. Listings are cached for the resolver's lifetime:
/// make one per operation.
pub(crate) struct CaseResolver {
    root: PathBuf,
    listings: HashMap<PathBuf, HashMap<String, String>>,
}

impl CaseResolver {
    pub(crate) fn new(root: &Path) -> Self {
        CaseResolver {
            root: root.to_path_buf(),
            listings: HashMap::new(),
        }
    }

    pub(crate) fn resolve(&mut self, rel: &str) -> String {
        let mut dir = self.root.clone();
        let mut spelled: Vec<String> = Vec::new();
        for segment in rel.split('/').filter(|s| !s.is_empty()) {
            let listing = self
                .listings
                .entry(dir.clone())
                .or_insert_with(|| read_listing(&dir));
            let actual = listing
                .entry(lower_key(segment))
                .or_insert_with(|| segment.to_string())
                .clone();
            dir.push(&actual);
            spelled.push(actual);
        }
        spelled.join("/")
    }
}

fn read_listing(dir: &Path) -> HashMap<String, String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    let mut listing = HashMap::new();
    for name in names {
        listing.entry(lower_key(&name)).or_insert(name);
    }
    listing
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolver_adopts_the_spelling_on_disk_and_remembers_new_ones() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("natives/STM/Mesh")).unwrap();
        fs::write(dir.path().join("natives/STM/Mesh/a.mesh"), b"x").unwrap();
        let mut resolver = CaseResolver::new(dir.path());
        assert_eq!(
            resolver.resolve("natives/stm/mesh/A.MESH"),
            "natives/STM/Mesh/a.mesh"
        );
        // A folder that does not exist keeps the first spelling asked for…
        assert_eq!(
            resolver.resolve("natives/stm/NewDir/b.mesh"),
            "natives/STM/NewDir/b.mesh"
        );
        // …and a second path of the same operation agrees with it.
        assert_eq!(
            resolver.resolve("natives/STM/newdir/c.mesh"),
            "natives/STM/NewDir/c.mesh"
        );
    }

    #[test]
    fn fingerprint_tells_same_size_files_apart() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        fs::write(&a, vec![1u8; 3 << 20]).unwrap();
        let mut other = vec![1u8; 3 << 20];
        other[(3 << 20) - 5] = 2; // differs inside the tail span
        fs::write(&b, other).unwrap();
        assert_ne!(fingerprint(&a).unwrap(), fingerprint(&b).unwrap());
        assert_eq!(fingerprint(&a).unwrap().len(), 32);
        assert!(same_content(&a, &a));
        assert!(!same_content(&a, &b));
    }

    #[test]
    fn move_file_creates_parents_and_prune_stops_at_the_root() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("a.txt");
        fs::write(&from, b"bytes").unwrap();
        let to = dir.path().join("x/y/z/a.txt");
        move_file(&from, &to).unwrap();
        assert!(!from.exists());
        assert_eq!(fs::read(&to).unwrap(), b"bytes");
        fs::remove_file(&to).unwrap();
        prune_empty_dirs(to.parent(), dir.path());
        assert!(!dir.path().join("x").exists());
        assert!(dir.path().exists());
    }

    #[test]
    fn new_files_never_land_on_something_already_there() {
        let dir = tempfile::tempdir().unwrap();
        let taken = dir.path().join("taken");
        fs::write(&taken, b"someone's").unwrap();
        assert!(write_new(&taken, b"mine").is_err());
        assert_eq!(fs::read(&taken).unwrap(), b"someone's");
        let fresh = dir.path().join("fresh");
        write_new(&fresh, b"mine").unwrap();
        assert_eq!(fs::read(&fresh).unwrap(), b"mine");
        #[cfg(unix)]
        {
            let victim = dir.path().join("victim");
            fs::write(&victim, b"victim").unwrap();
            let link = dir.path().join("link");
            std::os::unix::fs::symlink(&victim, &link).unwrap();
            assert!(write_new(&link, b"through").is_err());
            assert!(copy_new(&fresh, &link).is_err());
            assert_eq!(fs::read(&victim).unwrap(), b"victim");
        }
    }

    #[test]
    fn replace_file_swaps_in_one_step() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("backup");
        let to = dir.path().join("target");
        fs::write(&from, b"original").unwrap();
        fs::write(&to, b"ours").unwrap();
        replace_file(&from, &to).unwrap();
        assert_eq!(fs::read(&to).unwrap(), b"original");
        assert!(!from.exists());
        // A failed replace leaves the target as it was.
        assert!(replace_file(&dir.path().join("gone"), &to).is_err());
        assert_eq!(fs::read(&to).unwrap(), b"original");
    }

    #[test]
    fn only_folders_the_engine_made_are_pruned() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("natives/stm")).unwrap();
        assert_eq!(
            missing_dirs(dir.path(), "natives/stm/own/new/x.mesh"),
            ["natives/stm/own", "natives/stm/own/new"]
        );
        assert!(missing_dirs(dir.path(), "natives/stm/x.mesh").is_empty());
        assert!(missing_dirs(dir.path(), "dinput8.dll").is_empty());

        fs::create_dir_all(dir.path().join("natives/stm/own/new")).unwrap();
        let mut created = vec!["natives/stm/own/NEW".to_string()];
        prune_created(dir.path(), "natives/stm/own/new", &mut created);
        assert!(!dir.path().join("natives/stm/own/new").exists());
        assert!(
            dir.path().join("natives/stm/own").exists(),
            "own was not the engine's"
        );
        assert!(created.is_empty());
    }

    #[test]
    fn child_lookup_ignores_case() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("DD2.EXE"), b"").unwrap();
        assert!(child_ci(dir.path(), "dd2.exe").is_some());
        assert!(child_ci(dir.path(), "other.exe").is_none());
    }
}
