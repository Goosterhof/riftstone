//! The bench every scenario runs on: a FAKE Dragon's Dogma 2 folder in a
//! tempdir (an empty `DD2.exe`, a small `re_chunk_000.pak`, a 144-byte patch
//! stub, a `dlc/` pak, `steam_api64.dll`), a state dir for the journal and
//! the backup store, an in-memory package source, and invented mods.
//! Nothing here ever touches a real game folder.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use rift_core::{
    ArchiveEntry, DeployTarget, ExtractItem, ExtractProgress, ModInfo, ModOption, OptionId,
    Package, PackageFormat, PackageId, PackageSource, PayloadFile, PayloadKinds, RiftError,
};
use rift_deploy::{
    install, uninstall, Below, DeployContext, DeployKind, DeployedFile, GameInstall, GameSource,
    InstallReport, InstalledOption, Journal, Origin, RefStatus, UninstallReport,
};

pub struct Bench {
    pub dir: tempfile::TempDir,
    pub state_dir: tempfile::TempDir,
    pub game: GameInstall,
    pub reframework: RefStatus,
}

impl Bench {
    pub fn new() -> Bench {
        let dir = tempfile::tempdir().unwrap();
        // The game root sits one level down so "../outside" stays inside the tempdir.
        let root = dir.path().join("Dragons Dogma 2");
        fs::create_dir_all(root.join("dlc")).unwrap();
        fs::write(root.join("DD2.exe"), b"").unwrap();
        fs::write(root.join("re_chunk_000.pak"), b"KPKA vanilla chunk").unwrap();
        fs::write(root.join("re_chunk_000.pak.patch_001.pak"), [0u8; 144]).unwrap();
        fs::write(root.join("dlc/re_dlc_stm_2581000.pak"), b"KPKA dlc").unwrap();
        fs::write(root.join("steam_api64.dll"), b"MZ steam").unwrap();
        Bench {
            game: GameInstall {
                root,
                build_id: Some("25290080".into()),
                source: GameSource::Manual,
            },
            reframework: RefStatus {
                installed: true,
                version: Some("v1.5.9.1+514".into()),
                build_date: Some("11.10.2026".into()),
                loose_file_loader: Some(true),
                pak_directory: Some(true),
                faulty_files: Vec::new(),
            },
            state_dir: tempfile::tempdir().unwrap(),
            dir,
        }
    }

    pub fn root(&self) -> &Path {
        &self.game.root
    }

    pub fn state(&self) -> &Path {
        self.state_dir.path()
    }

    pub fn ctx(&self) -> DeployContext<'_> {
        self.ctx_with(false)
    }

    /// The context with the game running (`true`) or not.
    pub fn ctx_with(&self, running: bool) -> DeployContext<'_> {
        DeployContext {
            game: &self.game,
            state_dir: self.state_dir.path(),
            ref_status: &self.reframework,
            game_running: running,
        }
    }

    /// The journal as it is ON DISK.
    pub fn journal(&self) -> Journal {
        Journal::load(self.state(), self.root()).unwrap()
    }

    pub fn write(&self, rel: &str, bytes: &[u8]) {
        let path = self.root().join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    pub fn read(&self, rel: &str) -> Option<Vec<u8>> {
        fs::read(self.root().join(rel)).ok()
    }

    pub fn exists(&self, rel: &str) -> bool {
        self.root().join(rel).exists()
    }

    /// Every file and folder in the game folder (folders map to `None`).
    pub fn snapshot(&self) -> BTreeMap<String, Option<Vec<u8>>> {
        snapshot(self.root())
    }

    /// Every FILE left in the backup store.
    pub fn store_files(&self) -> Vec<String> {
        let backups = self.state().join("backups");
        snapshot(&backups)
            .into_iter()
            .filter(|(_, bytes)| bytes.is_some())
            .map(|(path, _)| path)
            .collect()
    }

    /// Names directly inside a game folder (sorted).
    pub fn names_in(&self, rel: &str) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.root().join(rel))
            .map(|entries| {
                entries
                    .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }
}

pub fn snapshot(root: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
    let mut out = BTreeMap::new();
    if !root.exists() {
        return out;
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let kind = entry.file_type().unwrap();
            if kind.is_symlink() {
                // A link is recorded by where it points, never followed.
                let to = fs::read_link(&path).unwrap();
                out.insert(rel, Some(format!("link -> {}", to.display()).into_bytes()));
            } else if kind.is_dir() {
                out.insert(rel, None);
                stack.push(path);
            } else {
                out.insert(rel, Some(fs::read(&path).unwrap()));
            }
        }
    }
    out
}

/// Something a test does while an extraction runs (after the last file is
/// written, before the engine looks at the results).
pub type AfterExtract = Box<dyn FnMut() + Send>;

/// An in-memory package. It can fail (an error) or crash (a panic) when it
/// is about to write the n-th file of an extraction (1-based), and it can
/// run a hook once every file is written (another program acting while the
/// Riftstone installs).
pub struct FakeSource {
    pub files: Vec<(String, Vec<u8>)>,
    pub fail_at: Option<usize>,
    pub panic_at: Option<usize>,
    pub after: Option<AfterExtract>,
}

impl FakeSource {
    pub fn new(files: Vec<(String, Vec<u8>)>) -> FakeSource {
        FakeSource {
            files,
            fail_at: None,
            panic_at: None,
            after: None,
        }
    }

    pub fn failing_at(mut self, n: usize) -> FakeSource {
        self.fail_at = Some(n);
        self
    }

    pub fn panicking_at(mut self, n: usize) -> FakeSource {
        self.panic_at = Some(n);
        self
    }

    pub fn after_extract(mut self, hook: impl FnMut() + Send + 'static) -> FakeSource {
        self.after = Some(Box::new(hook));
        self
    }

    fn find(&self, entry: &str) -> Option<&Vec<u8>> {
        self.files
            .iter()
            .find(|(path, _)| path.eq_ignore_ascii_case(entry))
            .map(|(_, bytes)| bytes)
    }
}

impl PackageSource for FakeSource {
    fn format(&self) -> PackageFormat {
        PackageFormat::Folder
    }

    fn entries(&mut self) -> Result<Vec<ArchiveEntry>, RiftError> {
        Ok(self
            .files
            .iter()
            .map(|(path, bytes)| ArchiveEntry {
                path: path.clone(),
                size: bytes.len() as u64,
            })
            .collect())
    }

    fn read_entry(&mut self, entry: &str, limit: Option<u64>) -> Result<Vec<u8>, RiftError> {
        let bytes = self.find(entry).ok_or_else(|| RiftError::EntryNotFound {
            entry: entry.to_string(),
        })?;
        let cut = limit.map_or(bytes.len(), |l| bytes.len().min(l as usize));
        Ok(bytes[..cut].to_vec())
    }

    fn extract(
        &mut self,
        items: &[ExtractItem],
        progress: &mut dyn FnMut(ExtractProgress),
    ) -> Result<(), RiftError> {
        let bytes_total: u64 = items
            .iter()
            .filter_map(|i| self.find(&i.entry))
            .map(|b| b.len() as u64)
            .sum();
        let mut bytes_done = 0;
        for (n, item) in items.iter().enumerate() {
            if self.fail_at == Some(n + 1) {
                return Err(RiftError::corrupt(
                    "/mods/fake.zip",
                    format!("{} is damaged", item.entry),
                ));
            }
            if self.panic_at == Some(n + 1) {
                panic!("simulated crash while extracting {}", item.entry);
            }
            let bytes =
                self.find(&item.entry)
                    .cloned()
                    .ok_or_else(|| RiftError::EntryNotFound {
                        entry: item.entry.clone(),
                    })?;
            fs::create_dir_all(item.dest.parent().unwrap()).unwrap();
            fs::write(&item.dest, &bytes).unwrap();
            bytes_done += bytes.len() as u64;
            progress(ExtractProgress {
                files_done: n + 1,
                files_total: items.len(),
                bytes_done,
                bytes_total,
                item: n,
            });
        }
        if let Some(hook) = self.after.as_mut() {
            hook();
        }
        Ok(())
    }
}

/// The journal file as it is on disk, as text (to ask whether it still
/// mentions a store path).
pub fn journal_text(bench: &Bench) -> String {
    fs::read_to_string(rift_deploy::journal_path(bench.state(), bench.root())).unwrap_or_default()
}

/// Make a folder (and, with `deep`, everything under it) read-only, or
/// writable again. Unix only: the permission tests are skipped elsewhere.
#[cfg(unix)]
pub fn set_read_only(path: &Path, read_only: bool, deep: bool) {
    use std::os::unix::fs::PermissionsExt;
    let mode = if read_only { 0o555 } else { 0o755 };
    if deep {
        for entry in walkdir_all(path) {
            if entry.is_dir() {
                fs::set_permissions(&entry, fs::Permissions::from_mode(mode)).unwrap();
            }
        }
    }
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

#[cfg(unix)]
fn walkdir_all(path: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if entry.file_type().is_ok_and(|t| t.is_dir()) {
                    stack.push(p.clone());
                }
                out.push(p);
            }
        }
    }
    out
}

/// Can this process be stopped by a read-only folder? (Not when it runs as
/// root — then the permission tests have nothing to prove.)
#[cfg(unix)]
pub fn permissions_bite() -> bool {
    let dir = tempfile::tempdir().unwrap();
    set_read_only(dir.path(), true, false);
    let blocked = fs::write(dir.path().join("probe"), b"x").is_err();
    set_read_only(dir.path(), false, false);
    blocked
}

/// An invented mod: one package, one option, its payload and its bytes.
#[derive(Clone)]
pub struct TestMod {
    pub package: Package,
    pub option: ModOption,
    pub files: Vec<(String, Vec<u8>)>,
}

impl TestMod {
    pub fn new(name: &str) -> TestMod {
        let file_name = format!("{name}.zip");
        let id = PackageId::for_archive(&file_name, 4096);
        let package = Package {
            id: id.clone(),
            path: PathBuf::from(format!("/mods/{file_name}")),
            file_name,
            display_name: name.into(),
            format: PackageFormat::Zip,
            size: 4096,
            modified_ms: 0,
            entry_count: 0,
            options: Vec::new(),
            nexus: None,
            problems: Vec::new(),
        };
        let option = ModOption {
            id: OptionId::new(&id, name),
            package_id: id,
            root: name.into(),
            name: name.into(),
            info: None,
            screenshot: None,
            payload: Vec::new(),
            skipped: Vec::new(),
            kinds: PayloadKinds::default(),
            parent: None,
            addon_for: None,
            bundle: None,
            menu_only: false,
            trail: Vec::new(),
        };
        TestMod {
            package,
            option,
            files: Vec::new(),
        }
    }

    pub fn id(&self) -> OptionId {
        self.option.id.clone()
    }

    /// Any target, with the entry placed under the option root.
    pub fn target(mut self, entry_rel: &str, target: DeployTarget, bytes: &[u8]) -> TestMod {
        let entry = format!("{}/{entry_rel}", self.option.root);
        self.option.payload.push(PayloadFile {
            entry: entry.clone(),
            target,
            size: bytes.len() as u64,
        });
        self.option.kinds = PayloadKinds::tally(&self.option.payload);
        self.files.push((entry, bytes.to_vec()));
        self
    }

    pub fn loose(self, game_path: &str, bytes: &[u8]) -> TestMod {
        self.target(
            game_path,
            DeployTarget::Loose {
                game_path: game_path.into(),
            },
            bytes,
        )
    }

    pub fn script(self, game_path: &str, bytes: &[u8]) -> TestMod {
        self.target(
            game_path,
            DeployTarget::Reframework {
                game_path: game_path.into(),
            },
            bytes,
        )
    }

    pub fn root_file(self, game_path: &str, bytes: &[u8]) -> TestMod {
        self.target(
            game_path,
            DeployTarget::Root {
                game_path: game_path.into(),
            },
            bytes,
        )
    }

    /// A pak at `entry_rel` inside the option (its file name is the last segment).
    pub fn pak(self, entry_rel: &str, bytes: &[u8]) -> TestMod {
        let file_name = entry_rel.rsplit('/').next().unwrap().to_string();
        self.target(entry_rel, DeployTarget::Pak { file_name }, bytes)
    }

    pub fn requires(mut self, names: &[&str]) -> TestMod {
        let info = self.option.info.get_or_insert_with(ModInfo::default);
        info.requires = names.iter().map(|n| n.to_string()).collect();
        self
    }

    pub fn source(&self) -> FakeSource {
        FakeSource::new(self.files.clone())
    }

    pub fn install_with(
        &self,
        ctx: &DeployContext,
        journal: &mut Journal,
    ) -> Result<InstallReport, RiftError> {
        install(
            ctx,
            journal,
            &self.package,
            &self.option,
            &mut self.source(),
            &mut |_| {},
        )
    }

    pub fn install(&self, bench: &Bench, journal: &mut Journal) -> InstallReport {
        self.install_with(&bench.ctx(), journal)
            .unwrap_or_else(|err| panic!("installing {} failed: {err}", self.option.name))
    }

    pub fn uninstall(&self, bench: &Bench, journal: &mut Journal) -> UninstallReport {
        uninstall(&bench.ctx(), journal, &self.option.id)
            .unwrap_or_else(|err| panic!("uninstalling {} failed: {err}", self.option.name))
    }
}

/// An install the journal took over from files already in the game, built by
/// hand the way an older build's journal recorded one: no package, no entries,
/// no fingerprints (the size alone speaks for each file), each file's `below`
/// as given. Its seq is the journal's next; the caller pushes it and saves.
pub fn adopted(
    journal: &mut Journal,
    option_id: &str,
    name: &str,
    label: &str,
    files: &[(&str, DeployKind, u64, Below)],
) -> InstalledOption {
    let seq = journal.next_seq;
    journal.next_seq += 1;
    InstalledOption {
        seq,
        option_id: OptionId(option_id.into()),
        name: name.into(),
        package_file: String::new(),
        package_path: PathBuf::new(),
        package_size: 0,
        option_root: OptionId(option_id.into()).root().to_string(),
        version: None,
        installed_at: "2026-10-01T08:00:00Z".into(),
        game_build: Some("25290080".into()),
        origin: Origin::Adopted {
            label: label.into(),
        },
        files: files
            .iter()
            .map(|(game_path, kind, size, below)| DeployedFile {
                game_path: (*game_path).into(),
                kind: *kind,
                size: *size,
                entry: None,
                below: below.clone(),
                fingerprint: None,
            })
            .collect(),
    }
}

/// No file or folder name in the game folder carries the Riftstone's
/// temporary suffixes.
pub fn assert_no_temporaries(bench: &Bench) {
    for path in bench.snapshot().keys() {
        assert!(
            !path.contains(".riftstone-"),
            "a temporary file was left behind: {path}"
        );
    }
}
