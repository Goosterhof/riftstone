//! The Muster (v0.4): the save slots the stone found, the party of each, and the game's names.
//!
//! Read-only end to end (RS-8): a slot file is read into memory and parsed; nothing is written
//! to a save folder, and nothing of a save leaves the machine. The parsed parties are kept in
//! memory, keyed by file and its size and time; the game's names (the [`Codex`]) are cached
//! beside the stone's other caches, keyed by the game pak's size and time, so a game update
//! rebuilds them once.
//!
//! Privacy: a save folder's path holds the Steam account number, so no log line, error or event
//! names it. Logs name a slot by its file name and say counts; the window gets the party's
//! names in its view, and only there.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rift_core::{RiftError, SaveReason};
use rift_muster::saves::{self, SaveFolder, SlotFile};
use rift_muster::{Codex, Muster, MusterError, SlotError};
use serde::{Deserialize, Serialize};

use crate::dto::{
    MusterFolderProblem, MusterMember, MusterSlot, MusterSlotArisen, MusterSlotsView, MusterView,
    Named, NamesSource,
};
use crate::events;
use crate::state::{AppState, Discovery};
use crate::watch::{self, LibraryWatcher};

/// The game's own pak, in the game folder.
pub const GAME_PAK: &str = "re_chunk_000.pak";
/// The codex cache, in the stone's cache folder.
pub const CODEX_CACHE: &str = "muster-codex.json";
/// How long the stone waits before it reads a slot again whose signature disagreed (the game
/// writing it).
pub const REREAD_AFTER: Duration = Duration::from_millis(500);
/// Quiet time after the game writes a slot before the window hears of it.
pub const DEBOUNCE: Duration = Duration::from_secs(1);

const NO_GAME_NOTE: &str =
    "The stone found no Dragon's Dogma 2 install, and the names live in the game's own text.";
const NO_PAK_NOTE: &str =
    "The game's folder holds no re_chunk_000.pak, and the names live in the game's own text inside it.";
const UNREADABLE_NOTE: &str =
    "The stone could not read the game's own text in its pak, so each thing shows its number.";

/// The pak's size and modified time: what the codex cache is keyed by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct PakStamp {
    size: u64,
    secs: u64,
    nanos: u32,
}

impl PakStamp {
    fn of(path: &Path) -> Option<PakStamp> {
        let meta = fs::metadata(path).ok()?;
        let since = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
        Some(PakStamp {
            size: meta.len(),
            secs: since.as_secs(),
            nanos: since.subsec_nanos(),
        })
    }
}

#[derive(Serialize, Deserialize)]
struct CodexFile {
    stamp: PakStamp,
    codex: Codex,
}

/// What reading one slot file gave.
#[derive(Debug, Clone)]
enum SlotRead {
    Party(Arc<Muster>),
    /// The file is the game's system data: never listed.
    NotASlot,
    Problem(SaveReason),
}

#[derive(Debug, Clone)]
struct Cached {
    size: u64,
    modified: Option<SystemTime>,
    read: SlotRead,
}

/// The Muster's state in the spine. No lock on this state is held across a disk read; the
/// codex build's own gate (`building`) is held across the one read it guards.
#[derive(Default)]
pub struct MusterState {
    /// Tests and benches: exactly these save folders, nothing discovered.
    fixed: Mutex<Option<Vec<SaveFolder>>>,
    slots: Mutex<HashMap<PathBuf, Cached>>,
    /// The names built from a pak, or `None` when building them from it failed: held against
    /// the pak's size and time either way, so an unchanged pak is never read twice.
    codex: Mutex<Option<(PakStamp, Option<Arc<Codex>>)>>,
    watchers: Mutex<HashMap<PathBuf, LibraryWatcher>>,
    /// Held while one caller builds the names from the pak; the only lock held across a read.
    building: Mutex<()>,
    /// How many times the names were built from the game's pak since launch.
    builds: std::sync::atomic::AtomicUsize,
}

/// How many times the game's names were built from its pak since launch (the tests read it).
pub fn codex_builds(state: &AppState) -> usize {
    state
        .muster
        .builds
        .load(std::sync::atomic::Ordering::SeqCst)
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl MusterState {
    /// Use exactly these save folders (tests, benches).
    pub fn fix_folders(&self, folders: Vec<SaveFolder>) {
        *lock(&self.fixed) = Some(folders);
        lock(&self.slots).clear();
    }
}

/// The save folders, and whether a Steam folder was found at all.
fn folders(state: &AppState) -> (Vec<SaveFolder>, bool) {
    if let Some(fixed) = lock(&state.muster.fixed).clone() {
        let steam = !fixed.is_empty();
        return (fixed, steam);
    }
    match state.config.discovery {
        Discovery::Fixed { .. } => (Vec::new(), false),
        Discovery::System => {
            if let Some(folder) = saves::from_env() {
                return (vec![folder], true);
            }
            let roots: Vec<PathBuf> = rift_deploy::steam_roots()
                .into_iter()
                .filter(|r| r.is_dir())
                .collect();
            let steam = !roots.is_empty();
            (saves::discover(&roots), steam)
        }
    }
}

/// The same discovered save folders used by the Muster, for explicit raw backups.
pub(crate) fn backup_folders(state: &AppState) -> Vec<SaveFolder> {
    folders(state).0
}

fn modified(meta: &fs::Metadata) -> Option<SystemTime> {
    meta.modified().ok()
}

fn iso(time: Option<SystemTime>) -> String {
    let time = time.unwrap_or(UNIX_EPOCH);
    chrono::DateTime::<chrono::Utc>::from(time).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Why one attempt at a slot gave no party.
#[derive(Debug)]
enum Miss {
    /// The file is not there.
    Gone,
    /// The file is there and would not be read: another program holds it for a moment (the
    /// game, Steam Cloud, an antivirus), or this user may not read it.
    Held,
    Slot(SlotError),
}

impl Miss {
    /// A miss that a second look can heal: a held file, or one the game is writing (a cut file
    /// or a signature that disagrees).
    fn passing(&self) -> bool {
        matches!(
            self,
            Miss::Held
                | Miss::Slot(SlotError::Save(
                    rift_save::SaveError::Signature | rift_save::SaveError::Truncated
                ))
        )
    }
}

fn miss_of(err: &std::io::Error) -> Miss {
    if err.kind() == std::io::ErrorKind::NotFound {
        Miss::Gone
    } else {
        Miss::Held
    }
}

/// The file's bytes, read once and whole with the default sharing, holding nothing after. A
/// file larger than any save is never read: its size says so first.
fn slot_bytes(path: &Path) -> Result<Vec<u8>, Miss> {
    use std::io::Read;

    let file = fs::File::open(path).map_err(|e| miss_of(&e))?;
    let len = file.metadata().map_err(|e| miss_of(&e))?.len();
    let too_large = |len: u64| Miss::Slot(SlotError::Save(rift_save::SaveError::TooLarge(len)));
    if len > rift_save::MAX_FILE {
        return Err(too_large(len));
    }
    let mut bytes = Vec::with_capacity(len as usize);
    file.take(rift_save::MAX_FILE + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| miss_of(&e))?;
    if bytes.len() as u64 > rift_save::MAX_FILE {
        return Err(too_large(bytes.len() as u64));
    }
    Ok(bytes)
}

/// Read and parse one slot, trying the folder's own account first, then the others the stone
/// found. A miss a second look can heal is tried once more after [`REREAD_AFTER`]. A file that
/// still would not be read counts as being written, and so does a cut file or a disagreeing
/// signature whose file changed meanwhile; unchanged, those are damage. Only a file that is not
/// there is gone.
fn read_slot(path: &Path, account: u32, accounts: &[u32]) -> SlotRead {
    let attempt = || -> Result<Muster, Miss> {
        let bytes = slot_bytes(path)?;
        let mut last = SlotError::Save(rift_save::SaveError::WrongAccount);
        for id in std::iter::once(account).chain(accounts.iter().copied().filter(|&a| a != account))
        {
            match rift_muster::read_slot(&bytes, id) {
                Ok(m) => return Ok(m),
                Err(e @ SlotError::Save(rift_save::SaveError::WrongAccount)) => last = e,
                Err(other) => return Err(Miss::Slot(other)),
            }
        }
        Err(Miss::Slot(last))
    };
    let stat = || fs::metadata(path).ok().map(|m| (m.len(), modified(&m)));
    let before = stat();
    let result = match attempt() {
        Err(miss) if miss.passing() => {
            std::thread::sleep(REREAD_AFTER);
            match attempt() {
                Err(Miss::Held) => return SlotRead::Problem(SaveReason::BeingWritten),
                Err(miss) if miss.passing() => {
                    return SlotRead::Problem(if stat() == before {
                        SaveReason::Damaged
                    } else {
                        SaveReason::BeingWritten
                    });
                }
                other => other,
            }
        }
        other => other,
    };
    match result {
        Ok(m) => SlotRead::Party(Arc::new(m)),
        Err(Miss::Gone) => SlotRead::Problem(SaveReason::Gone),
        Err(Miss::Held) => SlotRead::Problem(SaveReason::BeingWritten),
        Err(Miss::Slot(SlotError::Save(rift_save::SaveError::WrongAccount))) => {
            SlotRead::Problem(SaveReason::OtherAccount)
        }
        Err(Miss::Slot(SlotError::Muster(MusterError::SystemData))) => SlotRead::NotASlot,
        Err(Miss::Slot(SlotError::Muster(MusterError::NoParty))) => {
            SlotRead::Problem(SaveReason::NoParty)
        }
        Err(Miss::Slot(_)) => SlotRead::Problem(SaveReason::Damaged),
    }
}

/// The slot's read, from the cache when the file has not changed since.
fn read_cached(
    state: &AppState,
    path: &Path,
    account: u32,
    accounts: &[u32],
) -> (Option<SystemTime>, SlotRead) {
    let meta = match fs::metadata(path) {
        Ok(meta) => meta,
        Err(err) => {
            return match miss_of(&err) {
                Miss::Gone => {
                    lock(&state.muster.slots).remove(path);
                    (None, SlotRead::Problem(SaveReason::Gone))
                }
                // Held for a moment: told as being written, which is never remembered.
                _ => (None, SlotRead::Problem(SaveReason::BeingWritten)),
            };
        }
    };
    let (size, time) = (meta.len(), modified(&meta));
    if let Some(hit) = lock(&state.muster.slots).get(path) {
        if hit.size == size && hit.modified == time {
            return (time, hit.read.clone());
        }
    }
    let read = read_slot(path, account, accounts);
    // A slot being written is read afresh next time; everything else waits for a change.
    if !matches!(read, SlotRead::Problem(SaveReason::BeingWritten)) {
        let fresh = fs::metadata(path).ok().map(|m| (m.len(), modified(&m)));
        if fresh == Some((size, time)) {
            lock(&state.muster.slots).insert(
                path.to_path_buf(),
                Cached {
                    size,
                    modified: time,
                    read: read.clone(),
                },
            );
        }
    }
    (time, read)
}

/// The names held for this pak, or the failure held for it: `None` when nothing is held for
/// this exact pak yet.
fn held_codex(state: &AppState, stamp: PakStamp) -> Option<Result<Arc<Codex>, &'static str>> {
    lock(&state.muster.codex)
        .as_ref()
        .filter(|(held, _)| *held == stamp)
        .map(|(_, codex)| codex.clone().ok_or(UNREADABLE_NOTE))
}

/// The game's names: from memory, the cache file, or the game's own pak. `Err` is the
/// sentence the window shows when they are missing. A build that fails is held against the
/// pak just as a success is, so the pak is not read again until it changes, and one build
/// runs at a time: a second caller waits for it.
pub fn codex(state: &AppState) -> Result<Arc<Codex>, &'static str> {
    let world = state.world();
    let game = world.game.as_ref().ok_or(NO_GAME_NOTE)?;
    let pak = child_ci(&game.root, GAME_PAK).ok_or(NO_PAK_NOTE)?;
    let stamp = PakStamp::of(&pak).ok_or(UNREADABLE_NOTE)?;
    if let Some(held) = held_codex(state, stamp) {
        return held;
    }
    let _building = lock(&state.muster.building);
    // Another caller may have built them while this one waited at the gate.
    if let Some(held) = held_codex(state, stamp) {
        return held;
    }
    let cache = state.config.dirs.cache.join(CODEX_CACHE);
    if let Some(file) = fs::read(&cache)
        .ok()
        .and_then(|b| serde_json::from_slice::<CodexFile>(&b).ok())
    {
        if file.stamp == stamp {
            let codex = Arc::new(file.codex);
            *lock(&state.muster.codex) = Some((stamp, Some(Arc::clone(&codex))));
            return Ok(codex);
        }
    }
    let started = std::time::Instant::now();
    state
        .muster
        .builds
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let built = rift_pak::game::GamePak::open(&pak)
        .map_err(|err| err.to_string())
        .and_then(|p| Codex::from_pak(&p).map_err(|err| err.to_string()));
    let codex = match built {
        Ok(codex) => Arc::new(codex),
        Err(err) => {
            log::warn!("the game's names could not be read from its pak: {err}; the stone tries again when the pak changes");
            *lock(&state.muster.codex) = Some((stamp, None));
            return Err(UNREADABLE_NOTE);
        }
    };
    let [items, vocations, inclinations, specializations, augments, skills] = codex.counts();
    log::info!(
        "the game's names read in {:?}: {items} items, {vocations} vocations, {inclinations} inclinations, \
         {specializations} specializations, {augments} augments, {skills} skills",
        started.elapsed()
    );
    let had_older = lock(&state.muster.codex)
        .replace((stamp, Some(Arc::clone(&codex))))
        .is_some();
    let file = CodexFile {
        stamp,
        codex: (*codex).clone(),
    };
    if let Ok(bytes) = serde_json::to_vec(&file) {
        let partial = cache.with_extension("json.part");
        let saved = fs::create_dir_all(&state.config.dirs.cache)
            .and_then(|()| fs::write(&partial, bytes))
            .and_then(|()| fs::rename(&partial, &cache));
        if let Err(err) = saved {
            log::warn!("the game's names could not be cached ({err}); they are read from the pak again next launch");
        }
    }
    if had_older {
        state.events.changed(events::MUSTER_CHANGED);
    }
    Ok(codex)
}

/// `dir`'s child called `name`, compared the way Windows compares names.
fn child_ci(dir: &Path, name: &str) -> Option<PathBuf> {
    let exact = dir.join(name);
    if exact.is_file() {
        return Some(exact);
    }
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name))
        .map(|e| e.path())
}

/// One slot as the list sees it, with where it lives.
struct Listed {
    folder: usize,
    file: SlotFile,
    path: PathBuf,
}

fn list(state: &AppState) -> (Vec<SaveFolder>, bool, Vec<Listed>) {
    let (folders, steam) = folders(state);
    // A folder found now (the first save of a new account) is watched from now on.
    arm_watchers(state, &folders);
    let mut listed = Vec::new();
    for (i, folder) in folders.iter().enumerate() {
        for file in saves::slot_files(&folder.path) {
            let path = folder.path.join(&file.file_name);
            listed.push(Listed {
                folder: i,
                file,
                path,
            });
        }
    }
    (folders, steam, listed)
}

/// The account numbers the window may see: 1-based, among the folders that hold slots, only
/// when there is more than one.
fn account_numbers(listed: &[Listed]) -> HashMap<usize, u32> {
    let mut with_slots: Vec<usize> = listed.iter().map(|l| l.folder).collect();
    with_slots.sort_unstable();
    with_slots.dedup();
    if with_slots.len() < 2 {
        return HashMap::new();
    }
    with_slots.into_iter().zip(1u32..).collect()
}

fn slot_id(file: &SlotFile, account: Option<u32>) -> String {
    match account {
        Some(n) => format!("{}@{n}", file.stem),
        None => file.stem.clone(),
    }
}

/// The character a slot belongs to (v0.5): FNV-1a 64 over the folder's account and the
/// Arisen's and the main pawn's names, as 16 hex digits. The save holds no playthrough id
/// (WINDOW-CONTRACT § v0.5), so the names tell characters apart. Made from private names, it is
/// private too: never logged.
fn character_of(account: u32, muster: &Muster) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for b in bytes {
            hash ^= u64::from(*b);
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
    };
    let name = |member: Option<&rift_muster::Member>| {
        member
            .and_then(|m| m.name.as_ref())
            .map(|n| n.expose().to_owned())
            .unwrap_or_default()
    };
    feed(&account.to_le_bytes());
    feed(&[0]);
    feed(name(muster.arisen.as_ref()).as_bytes());
    feed(&[0]);
    feed(name(muster.main_pawn.as_ref()).as_bytes());
    format!("{hash:016x}")
}

/// `get_muster_slots`: every slot that holds a party (or a problem), newest first.
pub fn slots_view(state: &AppState) -> MusterSlotsView {
    let (folders, steam, listed) = list(state);
    let accounts: Vec<u32> = folders.iter().map(|f| f.account_id).collect();
    let numbers = account_numbers(&listed);
    let names = codex(state).ok();
    let mut slots: Vec<(Option<SystemTime>, MusterSlot)> = Vec::new();
    for l in &listed {
        let (time, read) = read_cached(state, &l.path, folders[l.folder].account_id, &accounts);
        let account = numbers.get(&l.folder).copied();
        let (arisen, problem, slot, character, main_pawn_name) = match &read {
            SlotRead::NotASlot => continue,
            SlotRead::Problem(reason) => (None, Some(*reason), l.file.slot, None, None),
            SlotRead::Party(m) => (
                m.arisen.as_ref().map(|a| MusterSlotArisen {
                    name: a.name.as_ref().map(|n| n.expose().to_string()),
                    level: a.level,
                    vocation: Named::new(
                        a.vocation,
                        names.as_deref().and_then(|c| c.vocation(a.vocation)),
                    ),
                }),
                None,
                m.slot,
                Some(character_of(folders[l.folder].account_id, m)),
                m.main_pawn
                    .as_ref()
                    .and_then(|p| p.name.as_ref())
                    .map(|n| n.expose().to_string()),
            ),
        };
        slots.push((
            time,
            MusterSlot {
                slot_id: slot_id(&l.file, account),
                slot,
                saved_at: iso(time),
                account,
                arisen,
                problem,
                character,
                main_pawn_name,
            },
        ));
    }
    slots.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.slot_id.cmp(&b.1.slot_id)));
    let problem = if listed.is_empty() {
        Some(if folders.is_empty() && !steam {
            MusterFolderProblem::NoSteam
        } else {
            MusterFolderProblem::NoSaves
        })
    } else {
        None
    };
    log::info!(
        "the Muster lists {} slot(s) across {} save folder(s)",
        slots.len(),
        folders.len()
    );
    MusterSlotsView {
        problem,
        account_count: u32::try_from(numbers.len().max(usize::from(!listed.is_empty())))
            .unwrap_or(u32::MAX),
        slots: slots.into_iter().map(|(_, s)| s).collect(),
        names: if names.is_some() {
            NamesSource::Game
        } else {
            NamesSource::Missing
        },
    }
}

/// The listed slot behind `slot_id`, if it is still in its folder.
fn find(state: &AppState, slot_id: &str) -> Option<(Vec<SaveFolder>, Listed, Option<u32>)> {
    let (folders, _, listed) = list(state);
    let numbers = account_numbers(&listed);
    let found = listed
        .into_iter()
        .find(|l| slot_id_of(l, &numbers) == slot_id)?;
    let account = numbers.get(&found.folder).copied();
    Some((folders, found, account))
}

fn slot_id_of(l: &Listed, numbers: &HashMap<usize, u32>) -> String {
    slot_id(&l.file, numbers.get(&l.folder).copied())
}

fn file_name_of(slot_id: &str) -> String {
    format!("{}.bin", slot_id.split('@').next().unwrap_or(slot_id))
}

/// `get_muster`: the whole party of one slot, named.
pub fn view(state: &AppState, slot_id: &str) -> Result<MusterView, RiftError> {
    let Some((folders, l, _)) = find(state, slot_id) else {
        return Err(RiftError::save(
            SaveReason::Gone,
            format!("{} is no longer in the save folder.", file_name_of(slot_id)),
        ));
    };
    let accounts: Vec<u32> = folders.iter().map(|f| f.account_id).collect();
    let (time, read) = read_cached(state, &l.path, folders[l.folder].account_id, &accounts);
    let muster = match read {
        SlotRead::Party(m) => m,
        SlotRead::NotASlot => {
            return Err(RiftError::save(
                SaveReason::NoParty,
                format!(
                    "{} is the game's system data, not a save slot.",
                    l.file.file_name
                ),
            ))
        }
        SlotRead::Problem(reason) => {
            let sentence = match reason {
                SaveReason::OtherAccount => format!(
                    "{} belongs to a Steam account the stone did not find on this PC.",
                    l.file.file_name
                ),
                SaveReason::BeingWritten => format!(
                    "The game is writing {} right now, or another program is holding it for a moment.",
                    l.file.file_name
                ),
                SaveReason::Damaged => {
                    format!("{} is damaged inside and does not open.", l.file.file_name)
                }
                SaveReason::NoParty => {
                    format!("{} holds no Arisen and no pawns.", l.file.file_name)
                }
                SaveReason::Gone => {
                    format!("{} is no longer in the save folder.", l.file.file_name)
                }
            };
            return Err(RiftError::save(reason, sentence));
        }
    };
    let (codex, names, names_note) = match codex(state) {
        Ok(c) => (Some(c), NamesSource::Game, None),
        Err(note) => (None, NamesSource::Missing, Some(note.to_string())),
    };
    let c = codex.as_deref();
    Ok(MusterView {
        slot_id: slot_id.to_string(),
        slot: muster.slot,
        saved_at: iso(time),
        names,
        names_note,
        arisen: muster
            .arisen
            .as_ref()
            .map(|m| MusterMember::from_member(m, c)),
        main_pawn: muster
            .main_pawn
            .as_ref()
            .map(|m| MusterMember::from_member(m, c)),
        hired: muster
            .hired
            .iter()
            .map(|m| MusterMember::from_member(m, c))
            .collect(),
        gold: muster.gold,
        rift_crystals: muster.rift_crystals,
    })
}

/// `open_saves_folder`: the slot's folder in the file manager. The window never sees the path.
pub fn open_folder(state: &AppState, slot_id: &str) -> Result<(), RiftError> {
    let Some((folders, l, _)) = find(state, slot_id) else {
        return Err(RiftError::save(
            SaveReason::Gone,
            format!("{} is no longer in the save folder.", file_name_of(slot_id)),
        ));
    };
    // The launcher's own error names the folder, and this one's path holds the account
    // number: the window hears a sentence without it.
    state
        .config
        .launcher
        .show_folder(&folders[l.folder].path)
        .map_err(|_| RiftError::Io {
            detail: format!(
                "The stone could not open the save folder that holds {} in Explorer.",
                l.file.file_name
            ),
        })
}

/// Watch every save folder for the game's writes (each folder once). The window hears
/// `rift://muster-changed` a second after the last write of a burst.
pub fn arm_watchers(state: &AppState, folders: &[SaveFolder]) {
    let mut armed = lock(&state.muster.watchers);
    for (n, folder) in folders.iter().enumerate() {
        if armed.contains_key(&folder.path) || !folder.path.is_dir() {
            continue;
        }
        let sink = Arc::clone(&state.events);
        let label = format!("save folder {}", n + 1);
        if let Some(w) = watch::arm_labelled(&folder.path, &[], DEBOUNCE, &label, move || {
            sink.changed(events::MUSTER_CHANGED);
        }) {
            armed.insert(folder.path.clone(), w);
        }
    }
}

/// At wake: arm the save folders' watchers in the background (discovery reads a few folders).
pub fn wake(state: &Arc<AppState>) {
    let (folders, _) = folders(state);
    arm_watchers(state, &folders);
}
