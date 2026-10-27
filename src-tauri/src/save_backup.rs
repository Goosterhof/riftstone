//! Player-requested, verified copies of the original encrypted save files.
//! Sources are never written or decrypted. A backup is published only after
//! every copied file and a second source inventory agree with the first.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use rift_core::RiftError;
use rift_muster::saves::SaveFolder;
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Only the user-chosen destination reaches the window; source paths never do.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveBackupReport {
    pub path: String,
    pub folder_name: String,
    pub created_at: String,
    pub accounts: usize,
    pub files: usize,
    pub bytes: u64,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
struct Stamp {
    bytes: u64,
    #[serde(skip)]
    modified: Option<SystemTime>,
    sha256: String,
}

#[derive(Serialize)]
struct AccountManifest {
    account: u32,
    folder: String,
    files: BTreeMap<String, Stamp>,
}

#[derive(Serialize)]
struct Manifest {
    schema: u8,
    game: &'static str,
    created_at: String,
    original_encrypted_files: bool,
    accounts: Vec<AccountManifest>,
}

fn failed(detail: &str) -> RiftError {
    RiftError::Io {
        detail: detail.into(),
    }
}

fn changed() -> RiftError {
    RiftError::refused("The saves changed while the backup was being made. No completed backup was kept. Wait for Steam to finish syncing, then try again.")
}

fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

fn stopped(running: &impl Fn() -> bool) -> Result<(), RiftError> {
    if running() {
        return Err(RiftError::refused(
            "Close Dragon's Dogma 2 before backing up its saves, then try again.",
        ));
    }
    Ok(())
}

fn fingerprint(path: &Path) -> Result<Stamp, RiftError> {
    let mut file = File::open(path).map_err(|_| {
        failed("A save file could not be read. Wait for Steam to finish syncing, then try again.")
    })?;
    let before = file.metadata().map_err(|_| changed())?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    let mut bytes = 0;
    loop {
        let n = file.read(&mut buffer).map_err(|_| changed())?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
        bytes += n as u64;
    }
    let after = fs::metadata(path).map_err(|_| changed())?;
    if before.len() != bytes
        || after.len() != bytes
        || before.modified().ok() != after.modified().ok()
    {
        return Err(changed());
    }
    Ok(Stamp {
        bytes,
        modified: after.modified().ok(),
        sha256: hex(hash.finalize()),
    })
}

fn inventory(folder: &Path) -> Result<BTreeMap<String, Stamp>, RiftError> {
    fn visit(root: &Path, at: &Path, files: &mut BTreeMap<String, Stamp>) -> Result<(), RiftError> {
        let entries = fs::read_dir(at).map_err(|_| {
            failed(
                "The save folder could not be read. Check that Steam can open it, then try again.",
            )
        })?;
        for entry in entries {
            let entry = entry.map_err(|_| changed())?;
            let kind = entry.file_type().map_err(|_| changed())?;
            if kind.is_symlink() || (!kind.is_file() && !kind.is_dir()) {
                return Err(RiftError::refused("The save folder contains a link or special file. No backup was made; choose a normal save folder before trying again."));
            }
            if kind.is_dir() {
                visit(root, &entry.path(), files)?;
            } else {
                let path = entry.path();
                let relative = path.strip_prefix(root).map_err(|_| changed())?;
                let name = relative.to_str().ok_or_else(|| failed("A save file has a name the backup cannot preserve. No completed backup was kept."))?.replace('\\', "/");
                files.insert(name, fingerprint(&path)?);
            }
        }
        Ok(())
    }
    let mut files = BTreeMap::new();
    visit(folder, folder, &mut files)?;
    Ok(files)
}

fn key(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_lowercase()
}

fn inside(path: &Path, root: &Path) -> bool {
    let (path, root) = (key(path), key(root));
    path == root || path.starts_with(&(root + "/"))
}

/// The app's Steam Cloud subtree; a bench's arbitrary save folder stays itself.
fn cloud_root(source: &Path) -> &Path {
    let parents: Vec<_> = source.ancestors().take(5).collect();
    if parents.len() == 5
        && parents[0]
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("win64_save"))
        && parents[1]
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("remote"))
        && parents[2].file_name().is_some_and(|n| n == "2054970")
    {
        parents[4]
    } else {
        source
    }
}

/// An exclusively created staging directory; failure removes only this attempt.
struct Pending(Option<PathBuf>);
impl Drop for Pending {
    fn drop(&mut self) {
        if let Some(path) = &self.0 {
            let _ = fs::remove_dir_all(path);
        }
    }
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), RiftError> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)
        .map_err(|_| failed("The backup folder would not accept a new file. Choose another folder and try again."))?;
    file.write_all(bytes).and_then(|()| file.sync_all())
        .map_err(|_| failed("The backup could not be finished. Check free space or choose another folder, then try again."))
}

fn copy_new(source: &Path, target: &Path, expected: &Stamp) -> Result<(), RiftError> {
    let mut input = File::open(source).map_err(|_| changed())?;
    let mut output = OpenOptions::new().write(true).create_new(true).open(target)
        .map_err(|_| failed("The backup folder would not accept a new file. Choose another folder and try again."))?;
    // Stream the encrypted bytes: an unexpectedly large file does not need to
    // fit in memory, and create_new never replaces an earlier copied file.
    let bytes = std::io::copy(&mut input, &mut output)
        .map_err(|_| failed("The backup could not be finished. Check free space or choose another folder, then try again."))?;
    output.sync_all().map_err(|_| failed("The backup could not be finished. Check free space or choose another folder, then try again."))?;
    if bytes != expected.bytes || fingerprint(target)?.sha256 != expected.sha256 {
        return Err(changed());
    }
    Ok(())
}

/// `protected` includes the game, mod library and app data. The caller holds the
/// game-folder turn, so Play cannot launch through the stone during this copy.
pub fn create(
    folders: &[SaveFolder],
    destination: &Path,
    protected: &[PathBuf],
    running: impl Fn() -> bool,
) -> Result<SaveBackupReport, RiftError> {
    stopped(&running)?;
    if folders.is_empty() {
        return Err(RiftError::refused("No Dragon's Dogma 2 save folder was found. Start the game and save once, then look again."));
    }
    let destination = fs::canonicalize(destination).map_err(|_| {
        failed(
            "The backup destination could not be opened. Choose an existing folder and try again.",
        )
    })?;
    if !destination.is_dir() {
        return Err(RiftError::refused("Choose a folder for the backup."));
    }
    let mut sources = Vec::new();
    for folder in folders {
        let path = fs::canonicalize(&folder.path).map_err(|_| changed())?;
        if inside(&destination, cloud_root(&path)) || inside(&destination, &path) {
            return Err(RiftError::refused("Keep the backup outside Steam's save folders. Choose a separate folder, such as Documents."));
        }
        let files = inventory(&path)?;
        if files.is_empty() {
            return Err(RiftError::refused(
                "A save folder is empty. No backup was made; look again after saving in the game.",
            ));
        }
        sources.push((folder.account_id, path, files));
    }
    for path in protected {
        if let Ok(path) = fs::canonicalize(path) {
            if inside(&destination, &path) {
                return Err(RiftError::refused("Keep save backups outside the game, mod library and Riftstone app data. Choose a separate folder, such as Documents."));
            }
        }
    }
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let now = chrono::Utc::now();
    let created_at = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let base = format!("DD2 saves {}", now.format("%Y-%m-%d_%H-%M-%S-%3fZ"));
    let stage = destination.join(format!(
        ".riftstone-save-backup-{}-{}-{}",
        now.timestamp_millis(),
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&stage).map_err(|_| {
        failed("The backup folder could not be created. Choose another destination and try again.")
    })?;
    let mut pending = Pending(Some(stage.clone()));
    let mut manifest = Manifest {
        schema: 1,
        game: "2054970",
        created_at: created_at.clone(),
        original_encrypted_files: true,
        accounts: Vec::new(),
    };
    let mut files_count = 0;
    let mut bytes = 0;
    for (index, (account, source, files)) in sources.iter().enumerate() {
        let folder = format!("account-{}/win64_save", index + 1);
        for (name, expected) in files {
            stopped(&running)?;
            let target = stage.join(&folder).join(name);
            fs::create_dir_all(target.parent().ok_or_else(changed)?)
                .map_err(|_| failed("A backup subfolder could not be created. Choose another destination and try again."))?;
            // Read encrypted bytes only. Neither the parser nor a sealer is called.
            copy_new(&source.join(name), &target, expected)?;
            files_count += 1;
            bytes += expected.bytes;
        }
        manifest.accounts.push(AccountManifest {
            account: *account,
            folder,
            files: files.clone(),
        });
    }
    for (_, source, before) in &sources {
        if inventory(source)? != *before {
            return Err(changed());
        }
    }
    stopped(&running)?;
    let encoded = serde_json::to_vec_pretty(&manifest)
        .map_err(|_| failed("The backup receipt could not be prepared. Try again."))?;
    write_new(&stage.join("manifest.json"), &encoded)?;
    write_new(&stage.join("RESTORE.txt"), b"DD2 SAVE BACKUP\n\nOriginal encrypted files, copied without changing the live saves.\nmanifest.json maps each account folder and lists file hashes.\nKeep Dragon's Dogma 2 closed when restoring manually.\nRestore is not performed by this backup action.\n")?;
    for n in 1..=999 {
        let name = if n == 1 {
            base.clone()
        } else {
            format!("{base} ({n})")
        };
        let target = destination.join(&name);
        match crate::landing::move_new(&stage, &target) {
            Ok(()) => {
                pending.0 = None;
                return Ok(SaveBackupReport { path: target.to_string_lossy().into_owned(), folder_name: name, created_at, accounts: sources.len(), files: files_count, bytes });
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(failed("The verified backup could not be put in place. Choose another destination and try again.")),
        }
    }
    Err(failed(
        "Every backup name was taken. Choose another destination and try again.",
    ))
}
