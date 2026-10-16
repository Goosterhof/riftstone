//! The write-ahead journal (RS-4): what the Riftstone put in the game folder,
//! what sat there before, and what it was in the middle of doing.
//!
//! One JSON file per game folder in the app data dir
//! (`journal-<16 hex>.json`, the hex an FNV-1a of the folder's lower-cased
//! path), replaced atomically on every save (temp file, fsync, rename). A
//! damaged journal is an error, never silently reset: it is the only record
//! of which files in the game folder are the Riftstone's to remove.
//!
//! Every install and uninstall saves the journal with [`Journal::pending`]
//! set BEFORE the first byte moves, and saves it again with the work recorded
//! and `pending` cleared AFTER the last. The intent's [`IntentStep`]s say
//! exactly which paths were in play and where their backups go, so a crash
//! anywhere in between can be undone from what is on disk.
//!
//! Two lists keep the engine honest about what it does not own. A file that
//! should have gone back into the game folder but could not — because
//! something else had changed the file at its path meanwhile — is never
//! deleted and never forgotten: it stays in the backup store as a
//! [`ParkedBackup`] until the player puts it back or discards it. And the
//! engine removes only folders it created itself ([`Journal::created_dirs`]),
//! never one the player made.

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use rift_core::{fnv1a64, lower_key, DeployTarget, OptionId, PayloadKinds, RiftError};
use serde::{Deserialize, Serialize};

use crate::fsops::with_suffix;

/// The journal format this build reads and writes.
pub const JOURNAL_SCHEMA: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Journal {
    pub schema: u32,
    pub game_root: PathBuf,
    /// Everything installed, in install order.
    pub installs: Vec<InstalledOption>,
    /// Pak-bearing installs, lowest priority first — the last loads last and
    /// wins. Riftstone installs and adopted Fluffy installs alike.
    pub pak_order: Vec<OptionId>,
    /// The operation in flight. `Some` on disk after a crash.
    pub pending: Option<Intent>,
    pub next_seq: u64,
    /// Files a committed operation still has to delete (its trash, superseded
    /// backups). Only ever names files the Riftstone itself created.
    #[serde(default)]
    pub cleanup: Vec<Leftover>,
    /// Files kept aside in the backup store because putting them back would
    /// have overwritten something the Riftstone does not own. Listed until
    /// the player puts each one back or discards it.
    #[serde(default)]
    pub parked: Vec<ParkedBackup>,
    /// Folders in the game folder the Riftstone created (relative to the game
    /// root, the spelling on disk). The only folders it ever removes again,
    /// and only once they are empty.
    #[serde(default)]
    pub created_dirs: Vec<String>,
}

/// A file kept aside in the backup store: it belongs at `game_path`, but
/// something else sits there now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParkedBackup {
    /// Relative to the state dir — always under `backups/`.
    pub store: String,
    /// Where it belongs, relative to the game root.
    pub game_path: String,
    /// Whose bytes it holds. `Riftstone { option_id }` means that install's
    /// own bytes, buried under a file something else wrote: uninstalling it
    /// hands what was below it to this record, as to any layer above.
    pub owner: BelowOwner,
    /// RFC 3339, UTC.
    pub parked_at: String,
    /// One plain sentence: why it could not go back.
    pub reason: String,
}

/// One installed option.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledOption {
    pub seq: u64,
    pub option_id: OptionId,
    pub name: String,
    pub package_file: String,
    pub package_path: PathBuf,
    pub package_size: u64,
    pub option_root: String,
    pub version: Option<String>,
    /// RFC 3339, UTC.
    pub installed_at: String,
    pub game_build: Option<String>,
    pub origin: Origin,
    pub files: Vec<DeployedFile>,
}

impl InstalledOption {
    pub fn is_adopted(&self) -> bool {
        matches!(self.origin, Origin::AdoptedFromFluffy { .. })
    }

    pub fn pak_files(&self) -> impl Iterator<Item = &DeployedFile> {
        self.files.iter().filter(|f| f.kind == DeployKind::Pak)
    }

    pub fn has_paks(&self) -> bool {
        self.pak_files().next().is_some()
    }

    /// Counts per deploy kind, for the badge row.
    pub fn kinds(&self) -> PayloadKinds {
        let mut kinds = PayloadKinds::default();
        for file in &self.files {
            match file.kind {
                DeployKind::Pak => kinds.pak += 1,
                DeployKind::Loose => kinds.loose += 1,
                DeployKind::Reframework => kinds.reframework += 1,
                DeployKind::Root => kinds.root += 1,
            }
        }
        kinds
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Origin {
    Riftstone,
    #[serde(rename_all = "camelCase")]
    AdoptedFromFluffy {
        section: String,
    },
}

/// One file an install put in the game folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployedFile {
    /// Relative to the game root, `/` separators, the spelling on disk.
    pub game_path: String,
    pub kind: DeployKind,
    /// Bytes written.
    pub size: u64,
    /// The package entry it came from (`None` for adopted files).
    pub entry: Option<String>,
    /// What sat at this path before this install wrote it.
    pub below: Below,
    /// A fingerprint of the bytes written (see the engine): together with
    /// `size` it decides whether the file is still this install's to remove.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeployKind {
    Pak,
    Loose,
    Reframework,
    Root,
}

impl From<&DeployTarget> for DeployKind {
    fn from(target: &DeployTarget) -> Self {
        match target {
            DeployTarget::Pak { .. } => DeployKind::Pak,
            DeployTarget::Loose { .. } => DeployKind::Loose,
            DeployTarget::Reframework { .. } => DeployKind::Reframework,
            DeployTarget::Root { .. } => DeployKind::Root,
        }
    }
}

/// What sat at a path before an install wrote it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Below {
    /// Nothing: uninstalling deletes the file.
    Nothing,
    /// A file, moved into the backup store (`store` is relative to the state
    /// dir: `backups/<seq>/<game path>`); uninstalling moves it back.
    #[serde(rename_all = "camelCase")]
    Backup { store: String, owner: BelowOwner },
    /// Unknown — possibly a game file (an adopted Fluffy file in the game
    /// root). Uninstalling leaves the file in place.
    UnknownVanilla,
}

/// Whose bytes a backup holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BelowOwner {
    Unowned,
    #[serde(rename_all = "camelCase")]
    Fluffy {
        section: String,
    },
    /// Another install: it sits directly below the one holding the backup.
    #[serde(rename_all = "camelCase")]
    Riftstone {
        option_id: OptionId,
    },
}

/// An operation in flight, saved before its first byte moves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Intent {
    pub action: IntentAction,
    pub seq: u64,
    /// The option installed or uninstalled (empty for a reorder).
    pub option_id: OptionId,
    /// Every game path the operation may change.
    pub touched: Vec<String>,
    /// RFC 3339, UTC.
    pub started_at: String,
    #[serde(default)]
    pub option_name: String,
    /// The precise moves, in order — what recovery undoes.
    #[serde(default)]
    pub steps: Vec<IntentStep>,
    /// Reorder only: 0 while paks move to temporary names, 1 once every pak
    /// has one and they move to their final names.
    #[serde(default)]
    pub phase: u8,
    /// Install: the folders the operation creates in the game folder (they
    /// did not exist when it was planned), shallowest first. An undo removes
    /// them again once empty; the commit adds them to
    /// [`Journal::created_dirs`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dirs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IntentAction {
    /// An install — or a reinstall, whose intent first lists the uninstall
    /// of the old copy, so a failure puts the old copy back.
    Install,
    Uninstall,
    Reorder,
    /// Putting a file kept aside back into the game folder.
    RestoreParked,
}

/// One planned move of an operation. Game paths are relative to the game
/// root, store paths to the state dir.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum IntentStep {
    /// Install: put `size` bytes at `game_path` (via `<game_path>.riftstone-part`);
    /// when `backup` is set, the file there moves into the store first.
    #[serde(rename_all = "camelCase")]
    Place {
        game_path: String,
        size: u64,
        backup: Option<String>,
        /// Whose bytes the backup holds — what the record says should the
        /// backup have to stay in the store.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        owner: Option<BelowOwner>,
        /// The fingerprint of the extracted part, saved before the first
        /// rename. From then on a file at `game_path` with `size` bytes and
        /// this fingerprint is this install's own. `None`: nothing was
        /// renamed into place yet, so nothing at `game_path` is this
        /// install's — an undo never deletes it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fingerprint: Option<String>,
        /// The extraction stopped before it wrote this file's part, so
        /// whatever sits at `<game_path>.riftstone-part` is not this
        /// install's (the extractor refused a name something else took after
        /// the check, or never got that far): an undo leaves it alone. Set
        /// only when an extraction fails; `false` otherwise, and after a
        /// crash, when a part at a name the install checked free is its own.
        #[serde(default, skip_serializing_if = "is_false")]
        unextracted: bool,
    },
    /// Uninstall: `game_path` moves to `trash` (deleted after the commit).
    #[serde(rename_all = "camelCase")]
    Remove { game_path: String, trash: String },
    /// Uninstall: `game_path` (when present) moves to `trash`, then the
    /// backup moves back into place.
    #[serde(rename_all = "camelCase")]
    Restore {
        game_path: String,
        trash: Option<String>,
        backup: String,
    },
    /// Reorder: `from` → `temp` (phase 0) → `to` (phase 1).
    #[serde(rename_all = "camelCase")]
    Rename {
        from: String,
        temp: String,
        to: String,
    },
    /// Putting a kept file back over a file nobody owns: the file at
    /// `game_path` moves into the store at `store` (and is kept aside in
    /// turn).
    #[serde(rename_all = "camelCase")]
    Park { game_path: String, store: String },
}

/// A file a committed operation still has to delete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "place", rename_all = "camelCase")]
pub enum Leftover {
    /// Relative to the game root — only ever a `.riftstone-trash-` file.
    Game { path: String },
    /// Relative to the state dir — only ever under `backups/`.
    Store { path: String },
}

impl Journal {
    /// An empty journal for `game_root`.
    pub fn new(game_root: &Path) -> Journal {
        Journal {
            schema: JOURNAL_SCHEMA,
            game_root: game_root.to_path_buf(),
            installs: Vec::new(),
            pak_order: Vec::new(),
            pending: None,
            next_seq: 1,
            cleanup: Vec::new(),
            parked: Vec::new(),
            created_dirs: Vec::new(),
        }
    }

    /// The journal for `game_root` in `dir`, or a fresh one when none exists.
    /// A file that cannot be read or parsed is an error — it is never reset.
    pub fn load(dir: &Path, game_root: &Path) -> Result<Journal, RiftError> {
        let path = journal_path(dir, game_root);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Journal::new(game_root))
            }
            Err(err) => {
                return Err(journal_error(format!(
                    "could not read {}: {err}",
                    path.display()
                )))
            }
        };
        let journal: Journal = serde_json::from_slice(&bytes).map_err(|err| {
            journal_error(format!(
                "{} is damaged ({err}). It was left untouched — it is the only record of which files in the game folder the Riftstone installed",
                path.display()
            ))
        })?;
        if journal.schema > JOURNAL_SCHEMA {
            return Err(journal_error(format!(
                "{} was written by a newer Riftstone (format {}, this one reads {JOURNAL_SCHEMA}); update the Riftstone before managing mods",
                path.display(),
                journal.schema
            )));
        }
        if !journal.belongs_to(game_root) {
            return Err(journal_error(format!(
                "{} records the game folder {}, not {}",
                path.display(),
                journal.game_root.display(),
                game_root.display()
            )));
        }
        Ok(journal)
    }

    /// Write the journal atomically: a temp file beside it, flushed to disk,
    /// renamed over the old one.
    pub fn save(&self, dir: &Path) -> Result<(), RiftError> {
        fs::create_dir_all(dir)
            .map_err(|err| journal_error(format!("could not create {}: {err}", dir.display())))?;
        let path = journal_path(dir, &self.game_root);
        let temp = with_suffix(&path, ".tmp");
        let json = serde_json::to_vec_pretty(self)
            .map_err(|err| journal_error(format!("could not encode the journal: {err}")))?;
        let written = fs::File::create(&temp)
            .and_then(|mut file| {
                file.write_all(&json)?;
                file.sync_all()
            })
            .and_then(|()| fs::rename(&temp, &path));
        if let Err(err) = written {
            let _ = fs::remove_file(&temp);
            return Err(journal_error(format!(
                "could not write {}: {err}",
                path.display()
            )));
        }
        Ok(())
    }

    /// Is this the journal of `game_root` (compared case-insensitively)?
    pub fn belongs_to(&self, game_root: &Path) -> bool {
        root_key(&self.game_root) == root_key(game_root)
    }

    pub fn find(&self, option_id: &OptionId) -> Option<&InstalledOption> {
        self.installs.iter().find(|i| &i.option_id == option_id)
    }

    pub fn position(&self, option_id: &OptionId) -> Option<usize> {
        self.installs.iter().position(|i| &i.option_id == option_id)
    }

    pub fn is_installed(&self, option_id: &OptionId) -> bool {
        self.position(option_id).is_some()
    }

    /// Lower-cased Fluffy sections already adopted.
    pub fn adopted_sections(&self) -> HashSet<String> {
        self.installs
            .iter()
            .filter_map(|i| match &i.origin {
                Origin::AdoptedFromFluffy { section } => Some(lower_key(section)),
                Origin::Riftstone => None,
            })
            .collect()
    }

    /// The game build recorded by the newest install that recorded one.
    pub fn newest_game_build(&self) -> Option<&str> {
        self.installs
            .iter()
            .filter(|i| i.game_build.is_some())
            .max_by_key(|i| i.seq)
            .and_then(|i| i.game_build.as_deref())
    }
}

/// Where the journal of `game_root` lives in `dir`.
pub fn journal_path(dir: &Path, game_root: &Path) -> PathBuf {
    let key = fnv1a64(root_key(game_root).as_bytes());
    dir.join(format!("journal-{key:016x}.json"))
}

/// A game root as a comparison key: `/` separators, no trailing separator,
/// lower-cased (Windows paths are case-insensitive).
fn root_key(game_root: &Path) -> String {
    let unified = game_root.to_string_lossy().replace('\\', "/");
    lower_key(unified.trim_end_matches('/'))
}

fn journal_error(detail: String) -> RiftError {
    RiftError::Journal { detail }
}

/// Keeps a flag that is off out of the journal file (serde's
/// `skip_serializing_if` hands it a reference).
fn is_false(flag: &bool) -> bool {
    !*flag
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(game_root: &Path) -> Journal {
        let mut journal = Journal::new(game_root);
        journal.installs.push(InstalledOption {
            seq: 1,
            option_id: OptionId("00000000000000aa#Glow".into()),
            name: "Lantern Glow Recolor".into(),
            package_file: "Lantern Glow Recolor.zip".into(),
            package_path: PathBuf::from("/library/Lantern Glow Recolor.zip"),
            package_size: 1234,
            option_root: "Glow".into(),
            version: Some("1.2".into()),
            installed_at: "2026-10-16T10:00:00Z".into(),
            game_build: Some("25290080".into()),
            origin: Origin::Riftstone,
            files: vec![DeployedFile {
                game_path: "natives/stm/a.mesh".into(),
                kind: DeployKind::Loose,
                size: 3,
                entry: Some("Glow/natives/stm/a.mesh".into()),
                below: Below::Backup {
                    store: "backups/1/natives/stm/a.mesh".into(),
                    owner: BelowOwner::Fluffy {
                        section: "Glow-v1".into(),
                    },
                },
                fingerprint: Some("00".repeat(16)),
            }],
        });
        journal.next_seq = 2;
        journal
    }

    #[test]
    fn saves_and_loads_atomically_per_game_root() {
        let state = tempfile::tempdir().unwrap();
        let root = Path::new("/games/Dragons Dogma 2");
        let journal = sample(root);
        journal.save(state.path()).unwrap();
        let loaded = Journal::load(state.path(), Path::new("/GAMES/dragons dogma 2/")).unwrap();
        assert_eq!(loaded, journal);
        // No temp file left behind; one journal file named after the root.
        let names: Vec<String> = fs::read_dir(state.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names.len(), 1);
        assert!(names[0].starts_with("journal-") && names[0].ends_with(".json"));
        // Another game folder gets its own, empty journal.
        let other = Journal::load(state.path(), Path::new("/elsewhere/DD2")).unwrap();
        assert!(other.installs.is_empty());
    }

    #[test]
    fn the_wire_shape_is_camel_case_and_tagged() {
        let journal = sample(Path::new("/g"));
        let json = serde_json::to_value(&journal).unwrap();
        let file = &json["installs"][0]["files"][0];
        assert_eq!(file["gamePath"], "natives/stm/a.mesh");
        assert_eq!(file["below"]["kind"], "backup");
        assert_eq!(file["below"]["owner"]["kind"], "fluffy");
        assert_eq!(json["installs"][0]["origin"]["kind"], "riftstone");
        assert_eq!(json["nextSeq"], 2);
        let step = serde_json::to_value(IntentStep::Place {
            game_path: "a".into(),
            size: 1,
            backup: None,
            owner: None,
            fingerprint: None,
            unextracted: false,
        })
        .unwrap();
        assert_eq!(step["kind"], "place");
        assert_eq!(step["gamePath"], "a");
        assert!(
            step.get("fingerprint").is_none(),
            "an unset fingerprint stays off the wire"
        );
        assert!(
            step.get("unextracted").is_none(),
            "so does a part nobody doubts"
        );
        let doubted = serde_json::json!({ "kind": "place", "gamePath": "a", "size": 1,
                                          "backup": null, "unextracted": true });
        let parsed: IntentStep = serde_json::from_value(doubted.clone()).unwrap();
        assert!(matches!(
            parsed,
            IntentStep::Place {
                unextracted: true,
                ..
            }
        ));
        assert_eq!(serde_json::to_value(&parsed).unwrap(), doubted);
        // A step written before the flag existed reads as an extracted part.
        let older: IntentStep = serde_json::from_value(
            serde_json::json!({ "kind": "place", "gamePath": "a", "size": 1, "backup": null }),
        )
        .unwrap();
        assert!(matches!(
            older,
            IntentStep::Place {
                unextracted: false,
                ..
            }
        ));
    }

    #[test]
    fn a_journal_from_before_parking_still_loads() {
        let state = tempfile::tempdir().unwrap();
        let root = Path::new("/g");
        let mut old = serde_json::to_value(sample(root)).unwrap();
        let object = old.as_object_mut().unwrap();
        object.remove("parked");
        object.remove("createdDirs");
        object.remove("cleanup");
        fs::write(
            journal_path(state.path(), root),
            serde_json::to_vec(&old).unwrap(),
        )
        .unwrap();
        let loaded = Journal::load(state.path(), root).unwrap();
        assert!(loaded.parked.is_empty() && loaded.created_dirs.is_empty());
        assert_eq!(loaded.installs, sample(root).installs);
    }

    #[test]
    fn a_missing_journal_is_fresh_and_a_damaged_one_is_an_error() {
        let state = tempfile::tempdir().unwrap();
        let root = Path::new("/g");
        let fresh = Journal::load(state.path(), root).unwrap();
        assert_eq!(fresh, Journal::new(root));
        fs::write(journal_path(state.path(), root), b"{ not json").unwrap();
        let err = Journal::load(state.path(), root).unwrap_err();
        assert!(matches!(err, RiftError::Journal { .. }));
        // The damaged file is left exactly as it was.
        assert_eq!(
            fs::read(journal_path(state.path(), root)).unwrap(),
            b"{ not json"
        );
    }

    #[test]
    fn a_journal_from_a_newer_riftstone_is_refused() {
        let state = tempfile::tempdir().unwrap();
        let root = Path::new("/g");
        let mut journal = Journal::new(root);
        journal.schema = JOURNAL_SCHEMA + 1;
        journal.save(state.path()).unwrap();
        assert!(matches!(
            Journal::load(state.path(), root),
            Err(RiftError::Journal { .. })
        ));
    }

    #[test]
    fn helpers_answer_from_the_installs() {
        let mut journal = sample(Path::new("/g"));
        let id = OptionId("00000000000000aa#Glow".into());
        assert!(journal.is_installed(&id));
        assert_eq!(journal.newest_game_build(), Some("25290080"));
        assert_eq!(journal.installs[0].kinds().loose, 1);
        journal.installs[0].origin = Origin::AdoptedFromFluffy {
            section: "Glow-V1".into(),
        };
        assert!(journal.adopted_sections().contains("glow-v1"));
    }
}
