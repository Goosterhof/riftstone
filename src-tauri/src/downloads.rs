//! Downloads into the mods folder (v0.3, docs/WINDOW-CONTRACT.md § v0.3
//! Downloads): the one kind of file the stone ever adds there (RS-2).
//!
//! **The list** is this session's, in memory only. A quit forgets it; the
//! partial file of a download a quit cut short is removed at the next launch
//! (`.riftstone-<id>.part`, the only name the stone writes under while bytes
//! arrive).
//!
//! **One worker** (`riftstone-download`) takes the queued downloads one at a
//! time, in order. It holds the spine weakly while it waits, so it never
//! keeps a closing stone alive, and it never takes the deploy lock or the
//! update check's turn: a download never waits for an install or a check,
//! and neither ever waits for a download.
//!
//! **A premium account's** download is queued at once. **A free account's**
//! waits (`waiting`, with the file's page on Nexus for the window to open)
//! until the site's Mod Manager Download button hands the stone an nxm://
//! link for the same mod and file ([`crate::nxm`]): the link's grant is what
//! lets v1 serve a free account the download link.
//!
//! **The run.** `linking`: the file's facts (anonymous v2, when the start did
//! not ask already), the archive's own name and size (one v1 `files.json`)
//! and the download link (v1 `download_link.json`, Nexus's preferred mirror
//! first). `downloading`: rift-nexus's downloader, which never holds the API
//! key, streams into `.riftstone-<id>.part` with the MD5 computed on the
//! way; the window hears the bytes at most every 200 ms, plus the last.
//! `verifying`: the byte count against Nexus's size (v2's, else v1's) and the
//! server's length (the downloader; a body nobody sized stops at 32 GiB),
//! then the MD5 through the anonymous `fileHashes` lookup, which must name
//! this mod and file (`verify`): a hash Nexus does not know yet, or a
//! lookup that cannot be asked (Nexus out of reach, its 429 cooldown), passes
//! on a stated size alone; with no size stated anywhere, only a hash Nexus
//! knows for this file passes. `landed`: the file takes its Nexus name,
//! cleaned for Windows, never over an existing file (the same bytes there
//! already is `alreadyHad`; another file of that name makes it ` (2)`, ` (3)`
//! …); an archive is linked in `nexus.json` to the exact file (source
//! `download`) and the library is scanned, while a file that is no archive
//! (a raw `.pak`, an `.exe`) lands with no package id and no link, since the
//! scan never lists it. Failure or cancel: the partial file is deleted and
//! nothing else in the mods folder is touched. The stone never installs a
//! download by itself.
//!
//! **Secrets.** The API key stays in the client. An nxm grant's key lives in
//! its job until the worker takes it, never in the view. The download link
//! lives only in the worker's hands. None of them is logged, put in an error
//! or sent to the window.

use std::collections::VecDeque;
use std::fs;
use std::io::{self, Read};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError, Weak};
use std::time::{Duration, Instant};

use rift_core::{lower_key, LinkSource, NexusLink, PackageId, RiftError};
use rift_nexus::{Downloaded, NexusClient, NexusError, NxmGrant, NxmLink};

use crate::account;
use crate::dto::{DownloadPhase, DownloadProgressEvent, DownloadView};
use crate::events;
use crate::landing::{occupant, rename_new};
use crate::persist;
use crate::state::{lock, read, AppState};

/// The quickest the window hears a download's bytes (plus the last).
pub const PROGRESS_EVERY: Duration = Duration::from_millis(200);

/// The longest name the stone gives an archive, in UTF-16 units (what
/// Windows counts): Windows allows 255 per name, and the rest leaves room
/// for ` (2)` and a deep install folder.
const LONGEST_NAME: usize = 180;

/// The most ` (n)` names tried before the landing gives up.
const MOST_SIBLINGS: u32 = 999;

const NOT_LISTED: &str =
    "That download is not in this session's list any more. Look again at the downloads.";
const OVER: &str =
    "That download is already over (landed, failed or cancelled), so there is nothing to cancel.";
const LANDING: &str = "That download is being put in the mods folder right now and can no longer be stopped. It lands in a moment; remove the archive in Explorer if you do not want it.";
const STOPPED: &str = "The download stopped unexpectedly, and nothing was left in the mods folder. Try the download again; if it keeps happening, restart the Riftstone.";

/// The name a download is written under while its bytes arrive.
pub fn part_name(id: &str) -> String {
    format!(".riftstone-{id}.part")
}

/// The downloads of this session (`AppState.downloads`).
pub struct Downloads {
    shared: Arc<Shared>,
    worker: AtomicBool,
    /// Makes ids unique across launches, so a partial name never meets a
    /// leftover of an earlier session.
    session: String,
}

/// What the worker shares with the spine: the list and the queue, and the
/// bell it waits on.
#[derive(Default)]
struct Shared {
    book: Mutex<Book>,
    ready: Condvar,
}

#[derive(Default)]
struct Book {
    next: u64,
    /// Oldest first.
    jobs: Vec<Job>,
    /// Queued ids, in the order they run.
    queue: VecDeque<String>,
    /// The spine is gone: the worker stops.
    closed: bool,
}

struct Job {
    seq: u64,
    view: DownloadView,
    cancel: Arc<AtomicBool>,
    /// An nxm link's grant: a secret, never in the view.
    grant: Option<NxmGrant>,
    facts: Option<FileFacts>,
    /// The worker is putting the archive in place: no cancel any more.
    committed: bool,
}

/// What Nexus says about the file (anonymous v2 `modFiles`).
#[derive(Debug, Clone, PartialEq, Eq)]
struct FileFacts {
    /// The file's name on its page.
    name: String,
    version: Option<String>,
    size: Option<u64>,
    group_id: Option<u64>,
    sqid: Option<String>,
    uploaded_ms: u64,
}

/// What the worker takes from a queued job.
struct Running {
    id: String,
    mod_id: u64,
    file_id: u64,
    cancel: Arc<AtomicBool>,
    grant: Option<NxmGrant>,
    facts: Option<FileFacts>,
}

/// Why a run stopped short.
enum Stop {
    Cancelled,
    Failed(RiftError),
}

impl From<NexusError> for Stop {
    fn from(err: NexusError) -> Stop {
        match err {
            NexusError::Cancelled => Stop::Cancelled,
            other => Stop::Failed(other.into()),
        }
    }
}

/// How a run ended short.
enum Finish {
    Cancelled,
    Failed(RiftError),
}

impl Downloads {
    /// An empty list; `session_ms` (the launch's moment) makes its ids unique.
    pub fn new(session_ms: u64) -> Downloads {
        Downloads {
            shared: Arc::default(),
            worker: AtomicBool::new(false),
            session: format!("{session_ms:x}"),
        }
    }

    fn book(&self) -> MutexGuard<'_, Book> {
        self.shared
            .book
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// This session's downloads, newest first (`get_downloads`).
    pub fn views(&self) -> Vec<DownloadView> {
        let book = self.book();
        let mut jobs: Vec<&Job> = book.jobs.iter().collect();
        jobs.sort_by_key(|job| std::cmp::Reverse(job.seq));
        jobs.into_iter().map(|job| job.view.clone()).collect()
    }

    /// One download's row, if the list still holds it.
    pub fn view(&self, id: &str) -> Option<DownloadView> {
        self.book()
            .jobs
            .iter()
            .find(|job| job.view.id == id)
            .map(|job| job.view.clone())
    }

    /// Add a download (queued, or waiting for the site's link) unless one of
    /// the same file is still on its way: that one answers instead, with
    /// `false` in the second place.
    #[allow(clippy::too_many_arguments)]
    fn add(
        &self,
        mod_id: u64,
        file_id: u64,
        for_package_id: Option<String>,
        facts: Option<FileFacts>,
        mod_name: Option<String>,
        grant: Option<NxmGrant>,
        queued: bool,
        now_ms: u64,
    ) -> (DownloadView, bool) {
        let mut guard = self.book();
        let book = &mut *guard;
        if let Some(job) = book
            .jobs
            .iter()
            .find(|job| job.same_file(mod_id, file_id) && !job.view.phase.is_over())
        {
            return (job.view.clone(), false);
        }
        book.next += 1;
        let seq = book.next;
        let id = format!("{}-{seq}", self.session);
        let view = DownloadView {
            id: id.clone(),
            mod_id,
            file_id,
            mod_name,
            file_name: facts.as_ref().map(|facts| facts.name.clone()),
            version: facts.as_ref().and_then(|facts| facts.version.clone()),
            for_package_id,
            phase: if queued {
                DownloadPhase::Queued
            } else {
                DownloadPhase::Waiting
            },
            bytes_done: 0,
            bytes_total: facts.as_ref().and_then(|facts| facts.size),
            landed_as: None,
            package_id: None,
            already_had: false,
            failure: None,
            nexus_page_url: (!queued).then(|| rift_nexus::mod_manager_page_url(mod_id, file_id)),
            started_ms: now_ms,
            finished_ms: None,
        };
        book.jobs.push(Job {
            seq,
            view: view.clone(),
            cancel: Arc::new(AtomicBool::new(false)),
            grant,
            facts,
            committed: false,
        });
        if queued {
            book.queue.push_back(id);
        }
        (view, true)
    }

    /// The worker takes a queued job: it is `linking` from here.
    fn begin(&self, id: &str) -> Option<Running> {
        let mut book = self.book();
        let job = book.jobs.iter_mut().find(|job| job.view.id == id)?;
        if job.view.phase != DownloadPhase::Queued {
            return None;
        }
        job.view.phase = DownloadPhase::Linking;
        Some(Running {
            id: id.to_string(),
            mod_id: job.view.mod_id,
            file_id: job.view.file_id,
            cancel: Arc::clone(&job.cancel),
            grant: job.grant.take(),
            facts: job.facts.clone(),
        })
    }

    /// Change a job still under way; `false` when it is over (cancelled
    /// meanwhile, say) or gone from the list.
    fn change(&self, id: &str, change: impl FnOnce(&mut Job)) -> bool {
        let mut book = self.book();
        match book
            .jobs
            .iter_mut()
            .find(|job| job.view.id == id && !job.view.phase.is_over())
        {
            Some(job) => {
                change(job);
                true
            }
            None => false,
        }
    }

    /// The worker is about to put the archive in place: from here a cancel is
    /// refused. `false` when it was cancelled before that.
    fn commit(&self, id: &str) -> bool {
        let mut book = self.book();
        match book
            .jobs
            .iter_mut()
            .find(|job| job.view.id == id && !job.view.phase.is_over())
        {
            Some(job) if !job.cancel.load(Ordering::SeqCst) => {
                job.committed = true;
                true
            }
            _ => false,
        }
    }

    /// End a run that stopped short. `true` when the row changed.
    fn finish(&self, id: &str, how: Finish) -> bool {
        let now = persist::now_ms();
        self.change(id, |job| {
            job.view.finished_ms = Some(now);
            job.view.nexus_page_url = None;
            match how {
                Finish::Cancelled => job.view.phase = DownloadPhase::Cancelled,
                Finish::Failed(err) => {
                    job.view.phase = DownloadPhase::Failed;
                    job.view.failure = Some(err);
                }
            }
        })
    }
}

impl Job {
    fn same_file(&self, mod_id: u64, file_id: u64) -> bool {
        self.view.mod_id == mod_id && self.view.file_id == file_id
    }
}

impl Drop for Downloads {
    /// The spine is closing: the worker stops at its next look.
    fn drop(&mut self) {
        let mut book = self.book();
        book.closed = true;
        for job in &book.jobs {
            job.cancel.store(true, Ordering::SeqCst);
        }
        drop(book);
        self.shared.ready.notify_all();
    }
}

// ---------------------------------------------------------------------------
// The commands' side
// ---------------------------------------------------------------------------

/// `start_download`: no key → `noKey`. Asks Nexus (anonymously) for the
/// file's facts, so the row names it from the start, and validates the key
/// when the session has not yet (a download needs to know whether the account
/// is premium). Premium → queued; free → waiting, with the file's page on
/// Nexus. A download of the same file still on its way answers instead of a
/// second one.
pub fn start(
    state: &Arc<AppState>,
    mod_id: u64,
    file_id: u64,
    for_package_id: Option<String>,
) -> Result<DownloadView, RiftError> {
    if !state.has_api_key.load(Ordering::SeqCst) {
        return Err(NexusError::NoApiKey.into());
    }
    if let Some(view) = in_flight(state, mod_id, file_id) {
        return Ok(view);
    }
    let account = account::account_now(state)?;
    let client = read(&state.nexus_client).clone();
    let (facts, mod_name) = file_facts(&client, mod_id, file_id)?;
    let (view, added) = state.downloads.add(
        mod_id,
        file_id,
        for_package_id,
        Some(facts),
        mod_name,
        None,
        account.is_premium,
        persist::now_ms(),
    );
    if added {
        log::info!(
            "a download of mod {mod_id} file {file_id} {}",
            if account.is_premium {
                "is queued"
            } else {
                "waits for the site's Mod Manager Download link"
            }
        );
        state.events.changed(events::DOWNLOADS_CHANGED);
        if view.phase == DownloadPhase::Queued {
            ensure_worker(state);
        }
    }
    Ok(view)
}

/// A download of this file still on its way, if there is one.
fn in_flight(state: &AppState, mod_id: u64, file_id: u64) -> Option<DownloadView> {
    state
        .downloads
        .book()
        .jobs
        .iter()
        .find(|job| job.same_file(mod_id, file_id) && !job.view.phase.is_over())
        .map(|job| job.view.clone())
}

/// An accepted nxm link: it resumes the waiting download of the same mod and
/// file, or names the one of that file already on its way, or starts a new
/// one (queued: the worker asks Nexus for the file's facts). The id of the
/// download it went to.
pub fn take_link(state: &Arc<AppState>, link: NxmLink) -> String {
    let NxmLink {
        mod_id,
        file_id,
        mut grant,
        ..
    } = link;
    let (id, changed) = {
        let mut guard = state.downloads.book();
        let book = &mut *guard;
        let waiting = book
            .jobs
            .iter_mut()
            .find(|job| job.same_file(mod_id, file_id) && job.view.phase == DownloadPhase::Waiting);
        if let Some(job) = waiting {
            job.grant = grant.take();
            job.view.phase = DownloadPhase::Queued;
            job.view.nexus_page_url = None;
            let id = job.view.id.clone();
            book.queue.push_back(id.clone());
            (Some(id), true)
        } else if let Some(job) = book
            .jobs
            .iter_mut()
            .find(|job| job.same_file(mod_id, file_id) && !job.view.phase.is_over())
        {
            if job.view.phase == DownloadPhase::Queued && grant.is_some() {
                job.grant = grant.take();
            }
            (Some(job.view.id.clone()), false)
        } else {
            (None, true)
        }
    };
    let id = match id {
        Some(id) => id,
        None => {
            let (view, _) = state.downloads.add(
                mod_id,
                file_id,
                None,
                None,
                None,
                grant,
                true,
                persist::now_ms(),
            );
            view.id
        }
    };
    if changed {
        state.events.changed(events::DOWNLOADS_CHANGED);
    }
    ensure_worker(state);
    id
}

/// `cancel_download`: a download still on its way is cancelled at once (the
/// worker removes its partial file at its next look, between two reads);
/// one that is over, or is landing right now, is refused.
pub fn cancel(state: &AppState, id: &str) -> Result<(), RiftError> {
    {
        let mut guard = state.downloads.book();
        let book = &mut *guard;
        let Some(job) = book.jobs.iter_mut().find(|job| job.view.id == id) else {
            return Err(RiftError::refused(NOT_LISTED));
        };
        if job.view.phase.is_over() {
            return Err(RiftError::refused(OVER));
        }
        if job.committed {
            return Err(RiftError::refused(LANDING));
        }
        job.cancel.store(true, Ordering::SeqCst);
        job.grant = None;
        job.view.phase = DownloadPhase::Cancelled;
        job.view.finished_ms = Some(persist::now_ms());
        job.view.nexus_page_url = None;
        book.queue.retain(|queued| queued != id);
    }
    state.events.changed(events::DOWNLOADS_CHANGED);
    Ok(())
}

/// `clear_downloads`: forget the landed, failed and cancelled rows.
pub fn clear(state: &AppState) {
    let removed = {
        let mut book = state.downloads.book();
        let before = book.jobs.len();
        book.jobs.retain(|job| !job.view.phase.is_over());
        book.jobs.len() != before
    };
    if removed {
        state.events.changed(events::DOWNLOADS_CHANGED);
    }
}

// ---------------------------------------------------------------------------
// The worker
// ---------------------------------------------------------------------------

/// Start the worker if it is not running, and ring for it.
pub fn ensure_worker(state: &Arc<AppState>) {
    let downloads = &state.downloads;
    if !downloads.worker.swap(true, Ordering::SeqCst) {
        let weak = Arc::downgrade(state);
        let shared = Arc::clone(&downloads.shared);
        let spawned = std::thread::Builder::new()
            .name("riftstone-download".into())
            .spawn(move || work(&weak, &shared));
        if let Err(err) = spawned {
            log::error!("the download worker could not start ({err}); queued downloads wait for the next one");
            downloads.worker.store(false, Ordering::SeqCst);
            return;
        }
    }
    downloads.shared.ready.notify_all();
}

/// Take the queue in order until the spine is gone.
fn work(weak: &Weak<AppState>, shared: &Shared) {
    loop {
        let id = {
            let mut book = shared.book.lock().unwrap_or_else(PoisonError::into_inner);
            loop {
                if book.closed {
                    return;
                }
                if let Some(id) = book.queue.pop_front() {
                    break id;
                }
                book = shared
                    .ready
                    .wait(book)
                    .unwrap_or_else(PoisonError::into_inner);
            }
        };
        let Some(state) = weak.upgrade() else {
            return;
        };
        run(&state, &id);
    }
}

fn run(state: &Arc<AppState>, id: &str) {
    let Some(job) = state.downloads.begin(id) else {
        return;
    };
    state.events.changed(events::DOWNLOADS_CHANGED);
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| fetch(state, &job)));
    let changed = match outcome {
        Ok(Ok(())) => false,
        Ok(Err(Stop::Cancelled)) => state.downloads.finish(id, Finish::Cancelled),
        Ok(Err(Stop::Failed(err))) => {
            log::warn!(
                "the download of mod {} file {} did not land: {err}",
                job.mod_id,
                job.file_id
            );
            state.downloads.finish(id, Finish::Failed(err))
        }
        Err(_) => {
            log::error!(
                "the download of mod {} file {} stopped unexpectedly",
                job.mod_id,
                job.file_id
            );
            state.downloads.finish(
                id,
                Finish::Failed(RiftError::Io {
                    detail: STOPPED.into(),
                }),
            )
        }
    };
    if changed {
        state.events.changed(events::DOWNLOADS_CHANGED);
    }
}

/// One run, from `linking` to `landed`.
fn fetch(state: &Arc<AppState>, job: &Running) -> Result<(), Stop> {
    let still_wanted = || {
        if job.cancel.load(Ordering::SeqCst) {
            Err(Stop::Cancelled)
        } else {
            Ok(())
        }
    };
    let client = read(&state.nexus_client).clone();
    if !client.has_api_key() {
        return Err(NexusError::NoApiKey.into());
    }
    let facts = match &job.facts {
        Some(facts) => facts.clone(),
        None => {
            let (facts, mod_name) =
                file_facts(&client, job.mod_id, job.file_id).map_err(Stop::Failed)?;
            state.downloads.change(&job.id, |row| {
                row.view.file_name = Some(facts.name.clone());
                row.view.version.clone_from(&facts.version);
                row.view.bytes_total = facts.size;
                if mod_name.is_some() {
                    row.view.mod_name = mod_name;
                }
                row.facts = Some(facts.clone());
            });
            facts
        }
    };
    still_wanted()?;
    let listed = v1_listing(&client, job.mod_id, job.file_id);
    // Nexus's size: v2's, else v1's (`size_in_bytes`, never the KB fields).
    let expected = facts.size.or(listed.size);
    still_wanted()?;
    let mirror = client
        .download_links(job.mod_id, job.file_id, job.grant.as_ref())?
        .into_iter()
        .next()
        .ok_or_else(|| {
            Stop::Failed(RiftError::nexus(
                "Nexus Mods offered no download link for this file. Try again later.",
            ))
        })?;
    // The link's last path segment is the second choice of name; the link
    // itself never leaves this run.
    let link_name = link_file_name(&mirror.uri);
    still_wanted()?;
    if !state.downloads.change(&job.id, |row| {
        row.view.phase = DownloadPhase::Downloading;
        row.view.bytes_total = expected.or(row.view.bytes_total);
    }) {
        return Err(Stop::Cancelled);
    }
    state.events.changed(events::DOWNLOADS_CHANGED);

    let part = state.library_dir().join(part_name(&job.id));
    let mut meter = Meter::new(state, &job.id, expected);
    let downloaded = client.downloader().download_to(
        &mirror.uri,
        &part,
        expected,
        &mut |done, total| meter.tick(done, total),
        &job.cancel,
    )?;
    drop(mirror);
    meter.last(downloaded.bytes);
    // From here the partial file is the run's to remove on every way out.
    let part = PartGuard(part);

    if !state
        .downloads
        .change(&job.id, |row| row.view.phase = DownloadPhase::Verifying)
    {
        return Err(Stop::Cancelled);
    }
    state.events.changed(events::DOWNLOADS_CHANGED);
    verify(&client, &downloaded, job)?;
    if !state.downloads.commit(&job.id) {
        return Err(Stop::Cancelled);
    }

    let wanted = landing_name(
        listed.name.as_deref(),
        link_name.as_deref(),
        job.mod_id,
        job.file_id,
        &part.0,
    );
    let landing = land(
        state.library_dir(),
        &part.0,
        &wanted,
        downloaded.bytes,
        &downloaded.md5,
    )
    .map_err(Stop::Failed)?;
    drop(part);
    // Only an archive becomes a package: a raw `.pak` or an `.exe` lands
    // under its own name, but the scan never lists it, so no package id
    // and no link are made for it.
    let package_id = archive_extension(landing.name())
        .map(|_| PackageId::for_archive(landing.name(), downloaded.bytes));
    if let Some(package_id) = &package_id {
        pre_link(state, package_id, job, &facts);
    }
    let landed_ms = persist::now_ms();
    let already_had = matches!(landing, Landing::AlreadyHad(_));
    state.downloads.change(&job.id, |row| {
        row.view.phase = DownloadPhase::Landed;
        row.view.landed_as = Some(landing.name().to_string());
        row.view.package_id = package_id.as_ref().map(|id| id.0.clone());
        row.view.already_had = already_had;
        row.view.bytes_done = downloaded.bytes;
        row.view.bytes_total = Some(downloaded.bytes);
        row.view.finished_ms = Some(landed_ms);
    });
    log::info!(
        "the download of mod {} file {} landed in the mods folder{}",
        job.mod_id,
        job.file_id,
        match (already_had, package_id.is_some()) {
            (true, _) => " (the same file was there already)",
            (false, false) => " (no archive, so no package: the scan does not list it)",
            (false, true) => "",
        }
    );
    state.events.changed(events::DOWNLOADS_CHANGED);
    if !already_had && package_id.is_some() {
        state.request_scan();
    }
    Ok(())
}

/// The verifying step's MD5 half (the byte count was checked as the bytes
/// arrived): rift-nexus's `verify_download`. Where a stated size vouched for
/// the bytes, only a lookup that names another file (`notTheFile`) lets them
/// go: a lookup that could not be asked (Nexus out of reach, its 429
/// cooldown, an answer it could not use) lands the archive on its size
/// alone, as a hash Nexus does not know yet does. With no size stated at
/// all, only a hash Nexus knows for this file vouches: a lookup that could
/// not answer, or that does not know the hash, fails the download.
fn verify(client: &NexusClient, downloaded: &Downloaded, job: &Running) -> Result<(), Stop> {
    match client.verify_download(downloaded, job.mod_id, job.file_id) {
        Ok(_) => Ok(()),
        Err(NexusError::NotTheFile) => Err(NexusError::NotTheFile.into()),
        Err(
            err @ (NexusError::Offline(_)
            | NexusError::RateLimited { .. }
            | NexusError::Http(_)
            | NexusError::BadResponse(_)),
        ) if downloaded.size_vouched => {
            log::warn!(
                "the download of mod {} file {} lands on its size alone: Nexus Mods could not be asked about its MD5 ({err})",
                job.mod_id,
                job.file_id
            );
            Ok(())
        }
        Err(err) => Err(err.into()),
    }
}

/// What Nexus says about one file (anonymous v2), and the mod's name (best
/// effort: a name that could not be read is no reason to fail).
fn file_facts(
    client: &NexusClient,
    mod_id: u64,
    file_id: u64,
) -> Result<(FileFacts, Option<String>), RiftError> {
    let detail = client
        .mod_file_details(mod_id)?
        .into_iter()
        .find(|detail| detail.file.file_id == file_id)
        .ok_or_else(|| RiftError::from(NexusError::FileGone))?;
    let mod_name = match client.mods(&[mod_id]) {
        Ok(mods) => mods
            .into_iter()
            .find(|found| found.mod_id == mod_id)
            .map(|found| found.name),
        Err(err) => {
            log::warn!("the name of mod {mod_id} could not be read: {err}");
            None
        }
    };
    let file = detail.file;
    Ok((
        FileFacts {
            name: file.name,
            version: file.version,
            size: file.size_bytes,
            group_id: file.group_id,
            sqid: file.sqid,
            uploaded_ms: file.uploaded_ms,
        },
        mod_name,
    ))
}

/// What v1 `files.json` says about one file: the archive's own name as
/// uploaded, and its size in bytes. Each is `None` when Nexus does not say
/// or could not be asked (the download link names the archive then, and
/// v2's size or the server's has to do).
#[derive(Debug, Default)]
struct V1Listing {
    name: Option<String>,
    size: Option<u64>,
}

/// One v1 `files.json` read for both: the name and the size.
fn v1_listing(client: &NexusClient, mod_id: u64, file_id: u64) -> V1Listing {
    match client.files_v1(mod_id) {
        Ok(list) => V1Listing {
            name: list.file_name(file_id).map(str::to_string),
            size: list.size_of(file_id),
        },
        Err(err) => {
            log::warn!("v1 could not say the archive name and size of mod {mod_id} file {file_id} ({err}); the download link names it instead");
            V1Listing::default()
        }
    }
}

/// Link the landed archive's package to the exact file it is, before the
/// scan finds it, and tell the window (`rift://updates-changed`).
fn pre_link(state: &AppState, package_id: &PackageId, job: &Running, facts: &FileFacts) {
    let link = NexusLink {
        mod_id: job.mod_id,
        file_id: Some(job.file_id),
        group_id: facts.group_id,
        sqid: facts.sqid.clone(),
        version: facts.version.clone(),
        uploaded_ms: Some(facts.uploaded_ms).filter(|ms| *ms > 0),
        source: LinkSource::Download,
    };
    lock(&state.nexus).set_link_for(package_id, link);
    state.save_nexus();
    state.relabel();
    state.events.changed(events::UPDATES_CHANGED);
}

/// The window hears a download's bytes at most every [`PROGRESS_EVERY`],
/// plus the last; the row in the list follows each time.
struct Meter<'a> {
    state: &'a AppState,
    id: &'a str,
    total: Option<u64>,
    told_at: Option<Instant>,
    told_bytes: Option<u64>,
}

impl<'a> Meter<'a> {
    fn new(state: &'a AppState, id: &'a str, total: Option<u64>) -> Meter<'a> {
        Meter {
            state,
            id,
            total,
            told_at: None,
            told_bytes: None,
        }
    }

    fn tick(&mut self, done: u64, total: Option<u64>) {
        self.total = total.or(self.total);
        let whole = self.total.is_some_and(|total| done >= total);
        let due = self.told_at.is_none_or(|at| at.elapsed() >= PROGRESS_EVERY);
        if due || whole {
            self.tell(done);
        }
    }

    /// The last count, unless it was just told.
    fn last(&mut self, done: u64) {
        if self.told_bytes != Some(done) {
            self.tell(done);
        }
    }

    fn tell(&mut self, done: u64) {
        let total = self.total;
        let live = self.state.downloads.change(self.id, |row| {
            row.view.bytes_done = done;
            row.view.bytes_total = total.or(row.view.bytes_total);
        });
        self.told_at = Some(Instant::now());
        self.told_bytes = Some(done);
        if live {
            self.state.events.emit(
                events::DOWNLOAD_PROGRESS,
                &DownloadProgressEvent {
                    id: self.id.to_string(),
                    phase: DownloadPhase::Downloading,
                    bytes_done: done,
                    bytes_total: total,
                },
            );
        }
    }
}

/// The partial file, removed on every way out of the run after the
/// downloader handed it over whole.
struct PartGuard(PathBuf);

impl Drop for PartGuard {
    fn drop(&mut self) {
        match fs::remove_file(&self.0) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => log::warn!(
                "a partial download could not be removed now ({err}); the next launch removes it"
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// The name rule, the collision rule
// ---------------------------------------------------------------------------

/// Where a download landed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Landing {
    /// Written under this name.
    Written(String),
    /// A file with the same bytes was there already under this name: nothing
    /// was written.
    AlreadyHad(String),
}

impl Landing {
    pub fn name(&self) -> &str {
        match self {
            Landing::Written(name) | Landing::AlreadyHad(name) => name,
        }
    }
}

/// The name a download lands under: Nexus's own archive name (`files.json`),
/// else the download link's last path segment ([`link_file_name`]), cleaned
/// for Windows; else a name of the stone's own. A name without a zip, 7z or
/// rar extension takes the one its bytes say, so the scan sees the package.
pub fn landing_name(
    archive: Option<&str>,
    link_name: Option<&str>,
    mod_id: u64,
    file_id: u64,
    part: &Path,
) -> String {
    let name = archive
        .and_then(clean_file_name)
        .or_else(|| link_name.and_then(clean_file_name))
        .unwrap_or_else(|| format!("Nexus mod {mod_id} file {file_id}"));
    if archive_extension(&name).is_some() {
        return name;
    }
    match sniff_archive(part) {
        Some(extension) => fit(&format!("{name}.{extension}")),
        None => name,
    }
}

/// A file name Windows accepts and the scan lists: only the last path
/// component of `raw`; control characters, `<>:"|?*` and the invisible
/// Unicode format characters (`hidden_in_a_name`) become `_`; no leading or
/// trailing dots or spaces (a dot-name is hidden from the scan, a trailing
/// dot or space is dropped by Windows); at most 180 UTF-16 units, the
/// extension kept; and a reserved device name (`CON`, `NUL`, `COM1`,
/// `CONIN$` …, with an extension or not) takes a leading `_`, counted in
/// those 180. `None` when nothing is left.
pub fn clean_file_name(raw: &str) -> Option<String> {
    let last = raw.rsplit(['/', '\\']).next().unwrap_or(raw);
    let replaced: String = last
        .chars()
        .map(|c| {
            if c.is_control()
                || matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*')
                || hidden_in_a_name(c)
            {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = replaced.trim_matches(|c: char| c == '.' || c.is_whitespace());
    if trimmed.is_empty() {
        return None;
    }
    Some(fit(trimmed))
}

/// The invisible characters of Unicode's format category (Cf), and the line
/// and paragraph separators: they show nothing, and some reorder what a name
/// shows (a right-to-left override makes `piz.exe` read `exe.zip`), so a
/// name from Nexus could pass for something it is not in Explorer and in the
/// window.
fn hidden_in_a_name(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'
            | '\u{0600}'..='\u{0605}'
            | '\u{061C}'
            | '\u{06DD}'
            | '\u{070F}'
            | '\u{0890}'..='\u{0891}'
            | '\u{08E2}'
            | '\u{180E}'
            | '\u{200B}'..='\u{200F}'
            | '\u{2028}'..='\u{202E}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206F}'
            | '\u{FEFF}'
            | '\u{FFF9}'..='\u{FFFB}'
            | '\u{110BD}'
            | '\u{110CD}'
            | '\u{13430}'..='\u{1343F}'
            | '\u{1BCA0}'..='\u{1BCA3}'
            | '\u{1D173}'..='\u{1D17A}'
            | '\u{E0001}'
            | '\u{E0020}'..='\u{E007F}'
    )
}

/// What Windows counts in a name: UTF-16 units.
fn units(text: &str) -> usize {
    text.encode_utf16().count()
}

/// [`clamp`], then a leading `_` for a reserved device name, the `_` counted:
/// a cut can leave a device name behind (`CON` and spaces, cut to `CON`).
fn fit(name: &str) -> String {
    let name = clamp(name);
    if is_reserved(&name) {
        clamp(&format!("_{name}"))
    } else {
        name
    }
}

/// At most [`LONGEST_NAME`] UTF-16 units, the extension kept, never a
/// character cut in half.
fn clamp(name: &str) -> String {
    if units(name) <= LONGEST_NAME {
        return name.to_string();
    }
    let extension = name
        .rfind('.')
        .map(|at| &name[at..])
        .filter(|extension| units(extension) <= 10)
        .unwrap_or("");
    let stem = &name[..name.len() - extension.len()];
    let mut room = LONGEST_NAME - units(extension);
    let mut cut = String::new();
    for c in stem.chars() {
        let width = c.len_utf16();
        if width > room {
            break;
        }
        room -= width;
        cut.push(c);
    }
    format!(
        "{}{extension}",
        cut.trim_end_matches(|c: char| c == '.' || c.is_whitespace())
    )
}

/// Is this a name Windows keeps for a device (`CON`, `PRN`, `AUX`, `NUL`,
/// `CONIN$`, `CONOUT$`, `COM0`–`COM9`, `LPT0`–`LPT9`, and the superscript
/// ones), whatever its extension?
fn is_reserved(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or(name)
        .trim_end()
        .to_uppercase();
    if ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].contains(&stem.as_str()) {
        return true;
    }
    let mut chars = stem.chars();
    let prefix: String = chars.by_ref().take(3).collect();
    let rest: Vec<char> = chars.collect();
    (prefix == "COM" || prefix == "LPT")
        && rest.len() == 1
        && (rest[0].is_ascii_digit() || matches!(rest[0], '¹' | '²' | '³'))
}

/// The last path segment of a download link, percent-decoded.
pub fn link_file_name(link: &str) -> Option<String> {
    let rest = link.split_once("://").map_or(link, |(_, rest)| rest);
    let path = &rest[rest.find('/')?..];
    let path = path.split(['?', '#']).next().unwrap_or(path);
    let segment = path.rsplit('/').next()?;
    percent_decode(segment)
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            let hex = text.get(at + 1..at + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// `zip`, `7z` or `rar` when the name ends in it (any case).
fn archive_extension(name: &str) -> Option<&'static str> {
    let lower = lower_key(name);
    ["zip", "7z", "rar"]
        .into_iter()
        .find(|extension| lower.ends_with(&format!(".{extension}")))
}

/// What the first bytes say the archive is.
fn sniff_archive(path: &Path) -> Option<&'static str> {
    let mut head = [0u8; 8];
    let read = fs::File::open(path).ok()?.read(&mut head).ok()?;
    let head = &head[..read];
    if head.starts_with(b"PK\x03\x04") || head.starts_with(b"PK\x05\x06") {
        Some("zip")
    } else if head.starts_with(b"7z\xBC\xAF\x27\x1C") {
        Some("7z")
    } else if head.starts_with(b"Rar!\x1A\x07") {
        Some("rar")
    } else {
        None
    }
}

/// Put the whole, checked download in place under `wanted`, never over an
/// existing file: a file of that name (any case) with the same size and MD5
/// is `AlreadyHad` (nothing is written, the partial file stays for the
/// caller to remove); another file of that name makes it ` (2)`, ` (3)` …
/// before the extension.
pub fn land(
    folder: &Path,
    part: &Path,
    wanted: &str,
    bytes: u64,
    md5: &str,
) -> Result<Landing, RiftError> {
    let (stem, extension) = match wanted.rfind('.') {
        Some(at) if at > 0 => (&wanted[..at], &wanted[at..]),
        _ => (wanted, ""),
    };
    for n in 1..=MOST_SIBLINGS {
        let name = if n == 1 {
            wanted.to_string()
        } else {
            format!("{stem} ({n}){extension}")
        };
        if let Some((there, path)) = occupant(folder, &name) {
            if same_bytes(&path, bytes, md5) {
                return Ok(Landing::AlreadyHad(there));
            }
            continue;
        }
        match rename_new(part, &folder.join(&name)) {
            Ok(()) => return Ok(Landing::Written(name)),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(RiftError::Io {
                    detail: format!(
                        "The download arrived whole, but it could not be put in the mods folder ({err}). Nothing was left behind; try the download again."
                    ),
                })
            }
        }
    }
    Err(RiftError::Io {
        detail: format!(
            "The mods folder already holds {MOST_SIBLINGS} files named like this download, so it was not kept. Tidy the folder in Explorer, then try again."
        ),
    })
}

/// Does the file at `path` hold exactly these bytes (size, then MD5)?
fn same_bytes(path: &Path, bytes: u64, md5: &str) -> bool {
    fs::metadata(path).is_ok_and(|meta| meta.is_file() && meta.len() == bytes)
        && rift_nexus::md5_file(path, &mut |_| {}).is_ok_and(|hash| hash == md5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_partial_name_is_the_one_the_launch_cleanup_removes() {
        let name = part_name("18f3a2b4c5d-1");
        assert_eq!(name, ".riftstone-18f3a2b4c5d-1.part");
        assert!(crate::state::is_partial_download(&name));
    }

    #[test]
    fn names_are_cleaned_for_windows() {
        let clean = |raw: &str| clean_file_name(raw);
        assert_eq!(
            clean("Lantern Glow Recolor-2207-2-2-1791987780.zip").as_deref(),
            Some("Lantern Glow Recolor-2207-2-2-1791987780.zip")
        );
        // No separators: the last component only, so nothing climbs out.
        assert_eq!(clean("../../Lantern.zip").as_deref(), Some("Lantern.zip"));
        assert_eq!(clean(r"..\..\Lantern.zip").as_deref(), Some("Lantern.zip"));
        assert_eq!(
            clean("Glow: <amber> \"v2\"?.zip").as_deref(),
            Some("Glow_ _amber_ _v2__.zip")
        );
        // No leading dots (hidden from the scan), no trailing dots or spaces.
        assert_eq!(clean("  .Lantern.zip. . ").as_deref(), Some("Lantern.zip"));
        assert_eq!(
            clean("Lantern\u{7}Glow.zip").as_deref(),
            Some("Lantern_Glow.zip")
        );
        // Reserved device names, with or without an extension.
        assert_eq!(clean("CON").as_deref(), Some("_CON"));
        assert_eq!(clean("nul.zip").as_deref(), Some("_nul.zip"));
        assert_eq!(clean("Com7 .7z").as_deref(), Some("_Com7 .7z"));
        assert_eq!(clean("LPT¹.rar").as_deref(), Some("_LPT¹.rar"));
        assert_eq!(clean("CONSOLE.zip").as_deref(), Some("CONSOLE.zip"));
        assert_eq!(clean("COM10.zip").as_deref(), Some("COM10.zip"));
        // Nothing left.
        for empty in ["", "..", "/", " . ", "a/"] {
            assert_eq!(clean(empty), None, "{empty:?}");
        }
        // Long names keep their extension.
        let long = format!("{}.zip", "Vernworth Cloaks ".repeat(20));
        let cut = clean(&long).unwrap();
        assert!(
            cut.ends_with(".zip") && cut.chars().count() <= LONGEST_NAME,
            "{cut}"
        );
        assert!(
            !cut.contains(" .zip"),
            "no space before the extension: {cut}"
        );
    }

    #[test]
    fn invisible_reordering_characters_never_reach_a_name() {
        let clean = |raw: &str| clean_file_name(raw);
        // A right-to-left override makes "Lantern<RLO>piz.exe" read as
        // "Lanternexe.zip" in Explorer and in the window.
        assert_eq!(
            clean("Lantern\u{202e}piz.exe").as_deref(),
            Some("Lantern_piz.exe")
        );
        for hidden in [
            '\u{200b}', '\u{200e}', '\u{202a}', '\u{2066}', '\u{2069}', '\u{2060}', '\u{feff}',
            '\u{00ad}', '\u{2028}',
        ] {
            let cleaned = clean(&format!("Lantern{hidden}Glow.zip")).unwrap();
            assert_eq!(cleaned, "Lantern_Glow.zip", "U+{:04X}", u32::from(hidden));
        }
    }

    #[test]
    fn every_device_name_is_kept_from_a_name() {
        let clean = |raw: &str| clean_file_name(raw);
        assert_eq!(clean("CONIN$").as_deref(), Some("_CONIN$"));
        assert_eq!(clean("conout$.zip").as_deref(), Some("_conout$.zip"));
        assert_eq!(clean("CONINS.zip").as_deref(), Some("CONINS.zip"));
        // A name the cut leaves as a device name takes its "_" after the cut.
        let cut = clean(&format!("CON{}z.zip", " ".repeat(200))).unwrap();
        assert_eq!(cut, "_CON.zip");
    }

    #[test]
    fn names_are_measured_the_way_windows_measures_them() {
        let clean = |raw: &str| clean_file_name(raw);
        // The "_" of a device name counts too.
        let device = clean(&format!("CON.{}", "z".repeat(400))).unwrap();
        assert!(device.starts_with("_CON."), "{device}");
        assert!(units(&device) <= LONGEST_NAME, "{}", units(&device));
        // Characters outside the basic plane are two UTF-16 units each.
        let lanterns = clean(&format!("{}.zip", "\u{1F3EE}".repeat(300))).unwrap();
        assert!(lanterns.ends_with(".zip"), "{lanterns}");
        assert!(
            units(&lanterns) <= LONGEST_NAME,
            "{} units",
            units(&lanterns)
        );
        assert!(
            lanterns
                .trim_end_matches(".zip")
                .chars()
                .all(|c| c == '\u{1F3EE}'),
            "no lantern cut in half"
        );
        // An extension the bytes add is measured with the name.
        let dir = tempfile::tempdir().unwrap();
        let part = dir.path().join(part_name("n-1"));
        fs::write(&part, b"PK\x03\x04 a zip").unwrap();
        let named = landing_name(Some(&"\u{1F3EE}".repeat(300)), None, 2207, 9001, &part);
        assert!(
            named.ends_with(".zip") && units(&named) <= LONGEST_NAME,
            "{named}"
        );
    }

    #[test]
    fn the_link_names_the_archive_when_nexus_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let part = dir.path().join(part_name("a-1"));
        fs::write(&part, b"PK\x03\x04 a zip").unwrap();
        let link = link_file_name(
            "https://cdn.example.invalid/6234/2207/Lantern%20Glow%20Recolor-2207-2-2.zip?md5=SIGNED&expires=9",
        );
        assert_eq!(
            link.as_deref(),
            Some("Lantern Glow Recolor-2207-2-2.zip"),
            "the last segment, decoded, without the signature"
        );
        let link = link.as_deref();
        assert_eq!(
            landing_name(
                Some("Lantern Glow Recolor-2207-2-2-1791987780.zip"),
                link,
                2207,
                9001,
                &part
            ),
            "Lantern Glow Recolor-2207-2-2-1791987780.zip",
            "files.json first"
        );
        assert_eq!(
            landing_name(None, link, 2207, 9001, &part),
            "Lantern Glow Recolor-2207-2-2.zip",
            "else the link's last segment"
        );
        assert_eq!(
            landing_name(Some("  "), Some(""), 2207, 9001, &part),
            "Nexus mod 2207 file 9001.zip",
            "else a name of the stone's own, with the extension the bytes say"
        );
        fs::write(&part, b"7z\xBC\xAF\x27\x1C\x00\x04").unwrap();
        assert_eq!(
            landing_name(Some("Lantern Glow"), link, 2207, 9001, &part),
            "Lantern Glow.7z"
        );
        fs::write(&part, b"plain text").unwrap();
        assert_eq!(
            landing_name(Some("Lantern Glow"), link, 2207, 9001, &part),
            "Lantern Glow",
            "bytes that say nothing add nothing"
        );
        assert_eq!(link_file_name("https://cdn.example.invalid"), None);
        assert_eq!(link_file_name("https://cdn.example.invalid/a%ZZ.zip"), None);
    }

    fn md5_of(bytes: &[u8]) -> String {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x");
        fs::write(&path, bytes).unwrap();
        rift_nexus::md5_file(&path, &mut |_| {}).unwrap()
    }

    #[test]
    fn a_landing_never_writes_over_a_file_and_knows_the_same_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path();
        let bytes = b"PK\x03\x04 the lantern archive";
        let md5 = md5_of(bytes);
        let size = bytes.len() as u64;
        let part = |id: &str| {
            let path = mods.join(part_name(id));
            fs::write(&path, bytes).unwrap();
            path
        };

        // Free name: written, the partial name gone.
        let first = part("a-1");
        let landed = land(mods, &first, "Lantern Glow.zip", size, &md5).unwrap();
        assert_eq!(landed, Landing::Written("Lantern Glow.zip".into()));
        assert!(!first.exists());
        assert_eq!(fs::read(mods.join("Lantern Glow.zip")).unwrap(), bytes);

        // The same bytes under that name (any case): already had, nothing written.
        let again = part("a-2");
        let had = land(mods, &again, "LANTERN GLOW.zip", size, &md5).unwrap();
        assert_eq!(had, Landing::AlreadyHad("Lantern Glow.zip".into()));
        assert!(again.exists(), "the partial file is the caller's to remove");
        fs::remove_file(&again).unwrap();

        // Another file of that name: " (2)", then " (3)".
        fs::write(mods.join("Pawn Banter.zip"), b"someone else's").unwrap();
        let third = part("a-3");
        assert_eq!(
            land(mods, &third, "Pawn Banter.zip", size, &md5).unwrap(),
            Landing::Written("Pawn Banter (2).zip".into())
        );
        fs::write(mods.join("Cloaks (2).zip"), b"x").unwrap();
        fs::write(mods.join("Cloaks.zip"), b"y").unwrap();
        let fourth = part("a-4");
        assert_eq!(
            land(mods, &fourth, "Cloaks.zip", size, &md5).unwrap(),
            Landing::Written("Cloaks (3).zip".into())
        );
        assert_eq!(
            fs::read(mods.join("Pawn Banter.zip")).unwrap(),
            b"someone else's"
        );
        assert_eq!(fs::read(mods.join("Cloaks.zip")).unwrap(), b"y");
        assert_eq!(fs::read(mods.join("Cloaks (2).zip")).unwrap(), b"x");

        // A folder of that name is no file with the same bytes.
        fs::create_dir_all(mods.join("Banter Folder.zip")).unwrap();
        let fifth = part("a-5");
        assert_eq!(
            land(mods, &fifth, "Banter Folder.zip", size, &md5).unwrap(),
            Landing::Written("Banter Folder (2).zip".into())
        );
        // A name without an extension counts on.
        let sixth = part("a-6");
        fs::write(mods.join("Lantern"), b"z").unwrap();
        assert_eq!(
            land(mods, &sixth, "Lantern", size, &md5).unwrap(),
            Landing::Written("Lantern (2)".into())
        );
    }
}
