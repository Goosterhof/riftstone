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
//!
//! ## Schema 2, and the journals older builds wrote
//!
//! Schema 2 knows two origins ([`Origin::Riftstone`], [`Origin::Adopted`])
//! and two backup owners ([`BelowOwner::Unowned`], [`BelowOwner::Riftstone`]).
//! A schema-1 journal is lifted in memory as it loads, on the raw JSON, by
//! rules that name no kind but the two that stay: an origin of any other kind
//! becomes `adopted` (labelled with its `section`, else the install's name);
//! an owner of any other kind becomes `unowned`; a stand-in option id — one
//! whose package half is not a package id's 16 lowercase hex digits — becomes
//! `adopted#<root half>` everywhere it is mentioned (`#2`, `#3` … when that
//! is taken). Loading never writes: the file on disk keeps schema 1 until the
//! next change saves the journal as schema 2.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use rift_core::error::RiftResult;
use rift_core::{fnv1a64, lower_key, DeployTarget, OptionId, PayloadKinds, RiftError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::fsops::with_suffix;

/// The journal format this build writes. It reads every format up to this
/// one: schema 1 (v0.1 and v0.2) is lifted as it loads.
pub const JOURNAL_SCHEMA: u32 = 2;

/// The package half every lifted stand-in id carries: an adopted install
/// that matched no library option is `adopted#<root half>`.
pub const STAND_IN_PACKAGE: &str = "adopted";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Journal {
    pub schema: u32,
    pub game_root: PathBuf,
    /// Everything installed, in install order.
    pub installs: Vec<InstalledOption>,
    /// Pak-bearing installs, lowest priority first — the last loads last and
    /// wins. The Riftstone's own installs and adopted ones alike.
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
    /// Who placed it (v0.7). A journal written before v0.7 has no field and
    /// reads as the player's; v0.6.2 reads this one and drops the mark at its
    /// next save (no `deny_unknown_fields`), so the schema stays 2.
    #[serde(default)]
    pub actor: Actor,
}

/// Who asked for an install (v0.7, BUILD-SPEC § v0.7 *The journal's actor*).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Actor {
    /// The player, at a press in the window. Every install before v0.7.
    #[default]
    Player,
    /// Tamsin, through her `install` tool, in the summoning `run_id` names.
    #[serde(rename_all = "camelCase")]
    Pawn { run_id: String },
}

/// The player, as a context's [`crate::DeployContext::actor`] borrows it.
pub static PLAYER: Actor = Actor::Player;

impl InstalledOption {
    /// Did the journal take this install over from files already in the
    /// game, rather than install it from a package?
    pub fn is_adopted(&self) -> bool {
        matches!(self.origin, Origin::Adopted { .. })
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

/// Where an install came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Origin {
    /// The Riftstone installed it from a package.
    Riftstone,
    /// The journal took it over from files already in the game folder: it
    /// has no package entries of its own (`DeployedFile::entry` is `None`)
    /// and uninstalls like any other install. `label` is the name it was
    /// known by when it was taken over.
    #[serde(rename_all = "camelCase")]
    Adopted { label: String },
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
    /// Unknown — possibly a game file (an adopted file directly in the game
    /// root). Uninstalling leaves the file in place.
    UnknownVanilla,
}

/// Whose bytes a backup holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BelowOwner {
    /// No install in the journal: "the file that was there before".
    Unowned,
    /// An install in the journal — the Riftstone's own or an adopted one. It
    /// sits directly below the layer holding the backup.
    #[serde(rename_all = "camelCase")]
    Riftstone { option_id: OptionId },
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
    /// A schema-1 journal is lifted in memory (see the module docs) and comes
    /// back as schema 2; nothing is written until the next save.
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
        let damaged = |err: serde_json::Error| {
            journal_error(format!(
                "{} is damaged ({err}). It was left untouched — it is the only record of which files in the game folder the Riftstone installed",
                path.display()
            ))
        };
        let mut raw: Value = serde_json::from_slice(&bytes).map_err(damaged)?;
        let schema = raw.get("schema").and_then(Value::as_u64);
        if let Some(newer) = schema.filter(|&s| s > u64::from(JOURNAL_SCHEMA)) {
            return Err(journal_error(format!(
                "{} was written by a newer Riftstone (format {newer}, this one reads formats up to {JOURNAL_SCHEMA}); update the Riftstone before managing mods",
                path.display()
            )));
        }
        if schema.is_some_and(|s| s < u64::from(JOURNAL_SCHEMA)) {
            lift_schema_1(&mut raw);
            if let Some(object) = raw.as_object_mut() {
                object.insert("schema".into(), JOURNAL_SCHEMA.into());
            }
        }
        let journal: Journal = serde_json::from_value(raw).map_err(damaged)?;
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

    /// Does the journal mention `option_id` anywhere — as an install, in the
    /// pak order, or as the owner of a backup or of a file kept aside? An id
    /// it holds can never be given to another install ([`Journal::rekey`]).
    pub fn holds(&self, option_id: &OptionId) -> bool {
        self.held_ids().contains(option_id)
    }

    /// The game build recorded by the newest install that recorded one.
    pub fn newest_game_build(&self) -> Option<&str> {
        self.installs
            .iter()
            .filter(|i| i.game_build.is_some())
            .max_by_key(|i| i.seq)
            .and_then(|i| i.game_build.as_deref())
    }

    /// Give installs new option ids: for every `from → to` in `map` whose
    /// `from` the journal holds, every mention of `from` — the install, the
    /// pak order, the owner of a backup or of a file kept aside — becomes
    /// `to`. (v0.3: a folder package's id follows its name now, and the
    /// relink pass finds installs whose package moved; see
    /// [`crate::relink`].) Entries for ids the journal does not hold, and
    /// entries onto themselves, change nothing.
    ///
    /// Refused — and then nothing changes — while an operation is pending,
    /// when a `to` is an id the journal already holds ([`Journal::holds`]),
    /// and when two installs would become one. Returns how many installs took
    /// a new id. Saving is the caller's ([`crate::engine::rekey`] does both).
    pub fn rekey(&mut self, map: &BTreeMap<OptionId, OptionId>) -> RiftResult<usize> {
        self.rekey_with_files(map, &BTreeMap::new())
    }

    /// [`Journal::rekey`], and each re-keyed install whose NEW id is in
    /// `package_files` takes that file name as its `package_file` (the name
    /// the installed list shows while the package is missing again).
    pub fn rekey_with_files(
        &mut self,
        map: &BTreeMap<OptionId, OptionId>,
        package_files: &BTreeMap<OptionId, String>,
    ) -> RiftResult<usize> {
        if self.pending.is_some() {
            return Err(RiftError::refused(
                "A change to the game folder is still recorded as unfinished, so no install was given a new id. Restart the Riftstone so it can put the game folder back first.",
            ));
        }
        let held = self.held_ids();
        let moves: BTreeMap<&OptionId, &OptionId> = map
            .iter()
            .filter(|(from, to)| from != to && held.contains(*from))
            .collect();
        let mut taken: HashMap<&OptionId, &OptionId> = HashMap::new();
        for (&from, &to) in &moves {
            if held.contains(to) {
                return Err(RiftError::refused(format!(
                    "The journal already holds \"{to}\", so \"{from}\" cannot take that id. Nothing was re-keyed."
                )));
            }
            if let Some(other) = taken.insert(to, from) {
                return Err(RiftError::refused(format!(
                    "\"{other}\" and \"{from}\" would both become \"{to}\". Nothing was re-keyed."
                )));
            }
        }
        if moves.is_empty() {
            return Ok(0);
        }
        let rename = |id: &mut OptionId| {
            if let Some(&to) = moves.get(&*id) {
                id.clone_from(to);
            }
        };
        let mut rekeyed = 0;
        for install in &mut self.installs {
            if let Some(&to) = moves.get(&install.option_id) {
                install.option_id.clone_from(to);
                if let Some(file) = package_files.get(to) {
                    install.package_file.clone_from(file);
                }
                rekeyed += 1;
            }
            for file in &mut install.files {
                if let Below::Backup {
                    owner: BelowOwner::Riftstone { option_id },
                    ..
                } = &mut file.below
                {
                    rename(option_id);
                }
            }
        }
        for id in &mut self.pak_order {
            rename(id);
        }
        for kept in &mut self.parked {
            if let BelowOwner::Riftstone { option_id } = &mut kept.owner {
                rename(option_id);
            }
        }
        Ok(rekeyed)
    }

    /// Every option id the journal mentions outside a pending intent.
    fn held_ids(&self) -> HashSet<&OptionId> {
        let owners = self
            .installs
            .iter()
            .flat_map(|install| &install.files)
            .filter_map(|file| match &file.below {
                Below::Backup {
                    owner: BelowOwner::Riftstone { option_id },
                    ..
                } => Some(option_id),
                _ => None,
            });
        let kept = self.parked.iter().filter_map(|kept| match &kept.owner {
            BelowOwner::Riftstone { option_id } => Some(option_id),
            BelowOwner::Unowned => None,
        });
        self.installs
            .iter()
            .map(|install| &install.option_id)
            .chain(&self.pak_order)
            .chain(owners)
            .chain(kept)
            .collect()
    }
}

/// Where the journal of `game_root` lives in `dir`.
pub fn journal_path(dir: &Path, game_root: &Path) -> PathBuf {
    let key = fnv1a64(root_key(game_root).as_bytes());
    dir.join(format!("journal-{key:016x}.json"))
}

/// Is `option_id` a stand-in rather than a library option's id: is its
/// package half something other than a package id's 16 lowercase hex digits?
/// (Every lifted stand-in reads `adopted#<root half>`.) The empty id of a
/// reorder's intent is no option at all, and no stand-in either.
pub fn is_stand_in(option_id: &OptionId) -> bool {
    stand_in_text(&option_id.0)
}

fn stand_in_text(id: &str) -> bool {
    if id.is_empty() {
        return false;
    }
    let package = id.split_once('#').map_or(id, |(package, _)| package);
    !(package.len() == 16
        && package
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
}

// ---------------------------------------------------------------------------
// The schema-1 lift
// ---------------------------------------------------------------------------

/// Lift a schema-1 journal to schema 2, on the raw JSON, by the rules in the
/// module docs. Anything the rules do not recognise (an origin or owner
/// without a `kind`, an id that is not a string) is left as it is, so the
/// strict read that follows calls the journal damaged instead of guessing.
fn lift_schema_1(raw: &mut Value) {
    for install in items_mut(raw, "installs") {
        lift_origin(install);
        for file in items_mut(install, "files") {
            if let Some(owner) = file
                .get_mut("below")
                .and_then(|below| below.get_mut("owner"))
            {
                lift_owner(owner);
            }
        }
    }
    for kept in items_mut(raw, "parked") {
        if let Some(owner) = kept.get_mut("owner") {
            lift_owner(owner);
        }
    }
    if let Some(pending) = raw.get_mut("pending") {
        for step in items_mut(pending, "steps") {
            if let Some(owner) = step.get_mut("owner") {
                lift_owner(owner);
            }
        }
    }
    rekey_stand_ins(raw);
}

/// The elements of the array at `key`, if there is one.
fn items_mut<'a>(value: &'a mut Value, key: &str) -> impl Iterator<Item = &'a mut Value> {
    value
        .get_mut(key)
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
}

/// An origin of any kind but `riftstone` → `adopted`, labelled with its
/// `section`, else the install's name.
fn lift_origin(install: &mut Value) {
    let name = install
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let Some(origin) = install.get_mut("origin") else {
        return;
    };
    match origin.get("kind").and_then(Value::as_str) {
        None | Some("riftstone") => {}
        Some(_) => {
            let label = origin
                .get("section")
                .and_then(Value::as_str)
                .filter(|section| !section.trim().is_empty())
                .map_or(name, str::to_string);
            *origin = json!({ "kind": "adopted", "label": label });
        }
    }
}

/// An owner of any kind but `unowned` or `riftstone` → `unowned`.
fn lift_owner(owner: &mut Value) {
    match owner.get("kind").and_then(Value::as_str) {
        None | Some("unowned" | "riftstone") => {}
        Some(_) => *owner = json!({ "kind": "unowned" }),
    }
}

/// Every stand-in id → `adopted#<root half>` (then `#2`, `#3` … while that
/// is taken), the same everywhere it is mentioned. Ids are handed out in the
/// order the journal mentions them, so the lift is the same at every load.
fn rekey_stand_ins(raw: &mut Value) {
    let mut mentioned: Vec<String> = Vec::new();
    for_each_id(raw, &mut |slot| {
        if let Some(id) = slot.as_str() {
            mentioned.push(id.to_string());
        }
    });
    let mut taken: HashSet<String> = mentioned
        .iter()
        .filter(|id| !id.is_empty() && !stand_in_text(id))
        .cloned()
        .collect();
    let mut renamed: HashMap<String, String> = HashMap::new();
    for id in &mentioned {
        if !stand_in_text(id) || renamed.contains_key(id) {
            continue;
        }
        let root = id.split_once('#').map_or("", |(_, root)| root);
        let base = format!("{STAND_IN_PACKAGE}#{root}");
        let mut candidate = base.clone();
        let mut n = 2u32;
        while taken.contains(&candidate) {
            candidate = format!("{base}#{n}");
            n += 1;
        }
        taken.insert(candidate.clone());
        renamed.insert(id.clone(), candidate);
    }
    if renamed.is_empty() {
        return;
    }
    for_each_id(raw, &mut |slot| {
        if let Some(new) = slot.as_str().and_then(|id| renamed.get(id)) {
            *slot = Value::String(new.clone());
        }
    });
}

/// Visit every place a journal names an option id, in a fixed order: the
/// installs, the pak order, the owners of backups, the owners of files kept
/// aside, the pending intent and the owners in its steps.
fn for_each_id(raw: &mut Value, visit: &mut dyn FnMut(&mut Value)) {
    for install in items_mut(raw, "installs") {
        if let Some(id) = install.get_mut("optionId") {
            visit(id);
        }
    }
    for id in items_mut(raw, "pakOrder") {
        visit(id);
    }
    for install in items_mut(raw, "installs") {
        for file in items_mut(install, "files") {
            if let Some(id) = file
                .get_mut("below")
                .and_then(|below| below.get_mut("owner"))
                .and_then(|owner| owner.get_mut("optionId"))
            {
                visit(id);
            }
        }
    }
    for kept in items_mut(raw, "parked") {
        if let Some(id) = kept
            .get_mut("owner")
            .and_then(|owner| owner.get_mut("optionId"))
        {
            visit(id);
        }
    }
    if let Some(pending) = raw.get_mut("pending") {
        if let Some(id) = pending.get_mut("optionId") {
            visit(id);
        }
        for step in items_mut(pending, "steps") {
            if let Some(id) = step
                .get_mut("owner")
                .and_then(|owner| owner.get_mut("optionId"))
            {
                visit(id);
            }
        }
    }
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
            package_path: PathBuf::from("/mods/Lantern Glow Recolor.zip"),
            package_size: 1234,
            option_root: "Glow".into(),
            version: Some("1.2".into()),
            installed_at: "2026-10-16T10:00:00Z".into(),
            game_build: Some("25290080".into()),
            actor: Actor::Player,
            origin: Origin::Riftstone,
            files: vec![DeployedFile {
                game_path: "natives/stm/a.mesh".into(),
                kind: DeployKind::Loose,
                size: 3,
                entry: Some("Glow/natives/stm/a.mesh".into()),
                below: Below::Backup {
                    store: "backups/1/natives/stm/a.mesh".into(),
                    owner: BelowOwner::Riftstone {
                        option_id: OptionId("adopted#Glow-v1".into()),
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
    fn a_journal_written_before_v0_7_reads_every_install_as_the_players() {
        let mut json = serde_json::to_value(sample(Path::new("/g"))).unwrap();
        json["installs"][0].as_object_mut().unwrap().remove("actor");
        let loaded: Journal = serde_json::from_value(json).unwrap();
        assert_eq!(loaded.installs[0].actor, Actor::Player);
    }

    #[test]
    fn a_pawns_install_round_trips_with_its_run() {
        let state = tempfile::tempdir().unwrap();
        let mut journal = sample(Path::new("/g"));
        journal.installs[0].actor = Actor::Pawn {
            run_id: "0b5e2c6e-4a8d-4d3e-9f4b-2a1c7d9e8f00".into(),
        };
        let json = serde_json::to_value(&journal).unwrap();
        assert_eq!(
            json["installs"][0]["actor"],
            serde_json::json!({ "kind": "pawn", "runId": "0b5e2c6e-4a8d-4d3e-9f4b-2a1c7d9e8f00" })
        );
        journal.save(state.path()).unwrap();
        assert_eq!(
            Journal::load(state.path(), Path::new("/g")).unwrap(),
            journal
        );
    }

    /// v0.6.2's install, field for field: no `actor`, and no
    /// `deny_unknown_fields` (the struct as it shipped in `ab79e87`).
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[allow(dead_code)]
    struct InstalledOptionV062 {
        seq: u64,
        option_id: OptionId,
        name: String,
        package_file: String,
        package_path: PathBuf,
        package_size: u64,
        option_root: String,
        version: Option<String>,
        installed_at: String,
        game_build: Option<String>,
        origin: Origin,
        files: Vec<DeployedFile>,
    }

    #[test]
    fn v0_6_2_reads_a_v0_7_install_and_only_drops_the_mark() {
        let mut journal = sample(Path::new("/g"));
        journal.installs[0].actor = Actor::Pawn {
            run_id: "0b5e2c6e-4a8d-4d3e-9f4b-2a1c7d9e8f00".into(),
        };
        let bytes = serde_json::to_vec(&journal.installs[0]).unwrap();
        let old: InstalledOptionV062 = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(old.option_id, journal.installs[0].option_id);
        assert_eq!(old.files, journal.installs[0].files);
    }

    #[test]
    fn the_wire_shape_is_camel_case_and_tagged() {
        let mut journal = sample(Path::new("/g"));
        let json = serde_json::to_value(&journal).unwrap();
        assert_eq!(json["schema"], 2);
        let file = &json["installs"][0]["files"][0];
        assert_eq!(file["gamePath"], "natives/stm/a.mesh");
        assert_eq!(file["below"]["kind"], "backup");
        assert_eq!(file["below"]["owner"]["kind"], "riftstone");
        assert_eq!(file["below"]["owner"]["optionId"], "adopted#Glow-v1");
        assert_eq!(json["installs"][0]["origin"]["kind"], "riftstone");
        assert_eq!(json["nextSeq"], 2);
        journal.installs[0].origin = Origin::Adopted {
            label: "Glow-v1".into(),
        };
        assert_eq!(
            serde_json::to_value(&journal).unwrap()["installs"][0]["origin"],
            serde_json::json!({ "kind": "adopted", "label": "Glow-v1" })
        );
        let unowned = serde_json::to_value(BelowOwner::Unowned).unwrap();
        assert_eq!(unowned, serde_json::json!({ "kind": "unowned" }));
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
    fn a_schema_2_journal_with_a_kind_it_does_not_know_is_damaged_not_lifted() {
        let state = tempfile::tempdir().unwrap();
        let root = Path::new("/g");
        let mut raw = serde_json::to_value(sample(root)).unwrap();
        raw["installs"][0]["origin"] = serde_json::json!({ "kind": "fromSomewhere" });
        let bytes = serde_json::to_vec(&raw).unwrap();
        fs::write(journal_path(state.path(), root), &bytes).unwrap();
        assert!(matches!(
            Journal::load(state.path(), root),
            Err(RiftError::Journal { .. })
        ));
        assert_eq!(fs::read(journal_path(state.path(), root)).unwrap(), bytes);
    }

    #[test]
    fn a_journal_from_a_newer_riftstone_is_refused() {
        let state = tempfile::tempdir().unwrap();
        let root = Path::new("/g");
        let mut journal = Journal::new(root);
        journal.schema = JOURNAL_SCHEMA + 1;
        journal.save(state.path()).unwrap();
        let err = Journal::load(state.path(), root).unwrap_err();
        assert!(
            matches!(&err, RiftError::Journal { detail } if detail.contains("newer")),
            "{err}"
        );
    }

    #[test]
    fn helpers_answer_from_the_installs() {
        let mut journal = sample(Path::new("/g"));
        let id = OptionId("00000000000000aa#Glow".into());
        assert!(journal.is_installed(&id));
        assert!(journal.holds(&id));
        assert!(
            journal.holds(&OptionId("adopted#Glow-v1".into())),
            "a backup's owner is held too"
        );
        assert!(!journal.holds(&OptionId("00000000000000bb#Glow".into())));
        assert_eq!(journal.newest_game_build(), Some("25290080"));
        assert_eq!(journal.installs[0].kinds().loose, 1);
        assert!(!journal.installs[0].is_adopted());
        journal.installs[0].origin = Origin::Adopted {
            label: "Glow-V1".into(),
        };
        assert!(journal.installs[0].is_adopted());
    }

    #[test]
    fn stand_ins_are_ids_without_a_package_id() {
        let id = |s: &str| OptionId(s.into());
        assert!(!is_stand_in(&id("0123456789abcdef#Glow")));
        assert!(!is_stand_in(&id("0123456789abcdef#")));
        assert!(!is_stand_in(&id("0123456789abcdef")));
        assert!(!is_stand_in(&id("")), "a reorder's empty id is no option");
        assert!(is_stand_in(&id("adopted#Glow")));
        assert!(is_stand_in(&id("elsewhere#Glow#2")));
        assert!(
            is_stand_in(&id("0123456789ABCDEF#Glow")),
            "package ids are lowercase"
        );
        assert!(is_stand_in(&id("0123456789abcde#Glow")), "15 digits");
        assert!(is_stand_in(&id("lonely")));
    }
}
