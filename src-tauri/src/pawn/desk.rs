//! THE WORKSHOP's desk (WINDOW-CONTRACT § v0.7): the one live summoning, the
//! pawn's settings (`pawn.json`: the voice, the read-only inputs, the
//! distro), and the commands' work, off the async runtime.
//!
//! The desk is Tauri-managed state of its own, beside the spine, so neither
//! `AppState` nor the window's `Settings` carry anything of the pawn in a
//! public build (RS-10).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};

use base64::Engine;
use rift_core::{PawnReason, RiftError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::events::EventSink;
use crate::pawn::errands::{RunsFile, StoneErrands};
use crate::pawn::fence::{
    bwrap_argv, check_mount_source, ground, sh_quote, FenceCommand, FencePlan, Ground, Host,
    MountKind, ProtectedRoot,
};
use crate::pawn::host::{install_files, relay_files, survey, Shell};
use crate::pawn::mailbox::{now, Broker, Handled, Outcome, PlayRefusal};
use crate::pawn::persona::{persona, Voice, RELAY, WORKSHOP_STARTER};
use crate::pawn::probe::{self, FenceRecord};
use crate::pawn::summon::{summon, Launch, RunFolders, Summoning, Watcher};
use crate::persist;
use crate::state::AppState;

pub const PAWN_OUTPUT: &str = "rift://pawn-output";
pub const PAWN_STATE: &str = "rift://pawn-state";
pub const PAWN_HANDS: &str = "rift://pawn-hands";
pub const WORKSHOP_CHANGED: &str = "rift://workshop-changed";

/// The pawn's own settings, in `pawn.json` beside the spine's.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PawnSettings {
    pub voice: Voice,
    /// Windows folders mounted read-only at `/inputs/<n>`, validated on set
    /// and again at every summon.
    pub inputs: Vec<String>,
    /// The WSL distro, when not the default one.
    pub distro: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostView {
    pub distro: String,
    pub claude_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeldView {
    pub held_request_id: String,
    pub since: String,
}

/// `PawnStateView` (WINDOW-CONTRACT § v0.7, Types).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PawnStateView {
    pub run_id: Option<String>,
    /// `absent`, `summoning`, `here`, `departed` or `torn`.
    pub state: &'static str,
    pub since: Option<String>,
    pub exit_code: Option<u32>,
    pub host: Option<HostView>,
    pub held: Option<HeldView>,
}

impl PawnStateView {
    fn absent() -> Self {
        Self {
            run_id: None,
            state: "absent",
            since: None,
            exit_code: None,
            host: None,
            held: None,
        }
    }
}

/// What `answer_pawn_play` did: the decision, and whether it reached the
/// run's ledger on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayAnsweredView {
    pub decision: crate::pawn::mailbox::Decision,
    pub persisted: bool,
}

/// `PawnHand` (WINDOW-CONTRACT § v0.7, Types): one answered call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PawnHand {
    pub run_id: String,
    pub seq: u64,
    pub at: String,
    pub tool: Option<String>,
    pub outcome: &'static str,
    pub summary: String,
    pub install_id: Option<String>,
    pub refusal: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct OutputEvent {
    seq: u64,
    data: String,
}

struct Live {
    summoning: Arc<Summoning>,
    host: Host,
    since: String,
}

/// What `prepare` made ready for a summon or a probe.
struct Prepared {
    shell: Box<dyn Shell>,
    survey: crate::pawn::host::Survey,
    plan: FencePlan,
    workshop: PathBuf,
    run_dir: PathBuf,
    run_id: String,
    ground: Ground,
}

/// `FenceView` (WINDOW-CONTRACT § v0.7, Types).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FenceView {
    pub chip: &'static str,
    pub ground: Option<Ground>,
    pub proved_at: Option<String>,
    pub steps: Vec<FenceStepView>,
    pub changed: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FenceStepView {
    pub step: String,
    pub expected: probe::Expect,
    pub observed: String,
    pub ok: bool,
}

#[derive(Default)]
struct DeskState {
    live: Option<Live>,
    last: Option<PawnStateView>,
}

/// The Workshop's desk: one per stone.
pub struct Desk {
    settings_path: PathBuf,
    record_path: PathBuf,
    runs_dir: PathBuf,
    /// The ground the last summon or probe was prepared on.
    ground: Mutex<Option<Ground>>,
    runs_file: Arc<RunsFile>,
    settings: Mutex<PawnSettings>,
    state: Mutex<DeskState>,
    hand_seq: AtomicU64,
    me: Weak<Desk>,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// A fresh run id: a version-4-shaped UUID from the clock, the process and
/// a counter (unique, not secret; the fence checks only its shape).
pub fn new_run_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let mut hash = Sha256::new();
    hash.update(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
            .to_le_bytes(),
    );
    hash.update(std::process::id().to_le_bytes());
    hash.update(COUNTER.fetch_add(1, Ordering::SeqCst).to_le_bytes());
    let mut bytes: [u8; 16] = hash.finalize()[..16].try_into().unwrap_or([0; 16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// The workshop: `<mods folder> (workshop)`, beside it.
pub fn workshop_dir(mods: &Path) -> Option<PathBuf> {
    let mut name = mods.file_name()?.to_os_string();
    name.push(" (workshop)");
    Some(mods.parent()?.join(name))
}

/// A folder's final resolved path (junctions and links followed), as text.
#[cfg(windows)]
pub fn final_path(path: &Path) -> std::io::Result<String> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::os::windows::io::{FromRawHandle, OwnedHandle};
    use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, GetFinalPathNameByHandleW, FILE_FLAG_BACKUP_SEMANTICS, FILE_NAME_NORMALIZED,
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, VOLUME_NAME_DOS,
    };
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);
    // SAFETY: a NUL-terminated path; the handle is checked.
    let raw = unsafe {
        CreateFileW(
            wide.as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            std::ptr::null_mut(),
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: a handle just opened, owned by nothing else.
    let handle = unsafe { OwnedHandle::from_raw_handle(raw as _) };
    let mut buffer = vec![0u16; 1024];
    loop {
        // SAFETY: a live handle and a buffer of the stated length.
        let len = unsafe {
            GetFinalPathNameByHandleW(
                std::os::windows::io::AsRawHandle::as_raw_handle(&handle) as _,
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                FILE_NAME_NORMALIZED | VOLUME_NAME_DOS,
            )
        } as usize;
        if len == 0 {
            return Err(std::io::Error::last_os_error());
        }
        if len < buffer.len() {
            let text = std::ffi::OsString::from_wide(&buffer[..len]);
            return Ok(text.to_string_lossy().into_owned());
        }
        buffer.resize(len + 1, 0);
    }
}

/// A folder's final resolved path, on the Linux bench.
#[cfg(not(windows))]
pub fn final_path(path: &Path) -> std::io::Result<String> {
    Ok(std::fs::canonicalize(path)?.to_string_lossy().into_owned())
}

/// How a Windows folder is named inside the fence.
#[cfg(windows)]
fn fence_path(final_path: &str) -> Option<String> {
    crate::pawn::fence::windows_to_wsl(final_path)
}

#[cfg(not(windows))]
fn fence_path(final_path: &str) -> Option<String> {
    Some(final_path.to_string())
}

#[cfg(windows)]
fn shell(distro: Option<String>) -> Box<dyn Shell> {
    Box::new(crate::pawn::host::Wsl { distro })
}

#[cfg(not(windows))]
fn shell(_distro: Option<String>) -> Box<dyn Shell> {
    Box::new(crate::pawn::host::LocalSh)
}

#[cfg(windows)]
fn launch(distro: &str, plan: &FencePlan) -> Result<Launch, crate::pawn::fence::FenceRefusal> {
    Launch::through_wsl(distro, plan)
}

#[cfg(not(windows))]
fn launch(_distro: &str, plan: &FencePlan) -> Result<Launch, crate::pawn::fence::FenceRefusal> {
    Launch::on_bench(plan)
}

fn no_fence(detail: impl Into<String>) -> RiftError {
    RiftError::pawn(PawnReason::NoFence, detail)
}

/// The backup inventory's file name, in the stone's app data.
const INVENTORY_FILE: &str = "save-backup-destinations.json";

/// Where the stone keeps the save-backup destinations it has written to.
fn backup_destinations_file(data_dir: &Path) -> PathBuf {
    data_dir.join(INVENTORY_FILE)
}

/// One writer of the backup inventory at a time, in this process.
static INVENTORY: Mutex<()> = Mutex::new(());

/// The save-backup folders the fence keeps out of reach, read with
/// integrity, never as a cache (the Heretic's WD3): no file is an empty
/// list, but a file that cannot be read or parsed refuses (it is left as it
/// is, never set aside or overwritten), so known coverage is never lost
/// silently.
pub fn backup_destinations(data_dir: &Path) -> Result<Vec<String>, RiftError> {
    let file = backup_destinations_file(data_dir);
    match std::fs::read(&file) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        // The sentence names the file, never its path: the app data sits
        // under the investor's home (WINDOW-CONTRACT § v0.7, Errors).
        Err(err) => Err(no_fence(format!(
            "The list of save backup folders ({INVENTORY_FILE} in the stone's app data) could not be read ({}), so Tamsin's fence cannot be sure of them. Fix or restore that file.",
            err.kind()
        ))),
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| {
            no_fence(format!(
                "The list of save backup folders ({INVENTORY_FILE} in the stone's app data) is damaged, so Tamsin's fence cannot be sure of them. Fix or restore that file; the stone leaves it as it is."
            ))
        }),
    }
}

/// Register a folder a save backup is about to be written into, durably,
/// BEFORE the backup is made: a destination that cannot be registered is no
/// destination (the backup refuses), so every backup the pawn's build makes
/// is a protected root from its first byte.
pub fn remember_backup_destination(data_dir: &Path, destination: &Path) -> Result<(), RiftError> {
    let _one = lock(&INVENTORY);
    let mut known = backup_destinations(data_dir)?;
    let text =
        final_path(destination).unwrap_or_else(|_| destination.to_string_lossy().into_owned());
    if !known.contains(&text) {
        known.push(text);
        persist::write_json(&backup_destinations_file(data_dir), &known, true).map_err(|err| {
            // persist's own sentence may carry the path; this one does not.
            log::warn!("the backup inventory could not be written: {err}");
            RiftError::refused(
                "The backup folder could not be recorded for Tamsin's fence, so no backup was made.",
            )
        })?;
    }
    Ok(())
}

/// The roots no input may overlap, by their final paths where they exist
/// (RS-9.2): Steam's install (and so `userdata`), the game, the mods folder
/// and its siblings, the stone's app data, the profile and its `.claude`, and
/// every save-backup folder registered (an unreadable list refuses).
pub fn protected_roots(state: &AppState) -> Result<Vec<ProtectedRoot>, RiftError> {
    let remembered = backup_destinations(&state.config.dirs.data)?;
    let mut roots = Vec::new();
    let mut add = |name: &str, path: Option<PathBuf>| {
        if let Some(path) = path {
            let text = final_path(&path).unwrap_or_else(|_| path.to_string_lossy().into_owned());
            roots.push(ProtectedRoot {
                name: name.into(),
                path: text,
            });
        }
    };
    let mods = state.library_dir().to_path_buf();
    let game = state.world().game.map(|game| game.root);
    // Steam's library is the folder above `steamapps`, wherever the game sits.
    let steam = game.as_ref().and_then(|root| {
        root.ancestors()
            .find(|dir| {
                dir.file_name()
                    .is_some_and(|n| n.eq_ignore_ascii_case("steamapps"))
            })
            .and_then(Path::parent)
            .map(Path::to_path_buf)
    });
    add("Steam's library", steam);
    // Every Steam install the stone's own discovery knows (the registry, the
    // environment), and the default one (the Heretic's WD3).
    for root in rift_deploy::steam_roots() {
        add("Steam", Some(root));
    }
    add(
        "Steam",
        std::env::var_os("ProgramFiles(x86)").map(|p| PathBuf::from(p).join("Steam")),
    );
    for destination in remembered {
        add("a save backup folder", Some(PathBuf::from(destination)));
    }
    add("the game folder", game);
    add(
        "the older versions folder",
        crate::family::older_folder(&mods),
    );
    add("the workshop", workshop_dir(&mods));
    add("the mods folder", Some(mods));
    add("the stone's app data", Some(state.config.dirs.data.clone()));
    add("the stone's cache", Some(state.config.dirs.cache.clone()));
    let profile = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from);
    add("your .claude", profile.as_ref().map(|p| p.join(".claude")));
    add("your profile", profile);
    Ok(roots)
}

impl Desk {
    pub fn new(data_dir: &Path) -> Arc<Desk> {
        let settings_path = data_dir.join("pawn.json");
        let settings = persist::read_json(&settings_path).unwrap_or_default();
        Arc::new_cyclic(|me| Desk {
            runs_dir: data_dir.join("pawn").join("runs"),
            record_path: data_dir.join("pawn-fence.json"),
            ground: Mutex::new(None),
            runs_file: Arc::new(RunsFile::new(data_dir.join(RunsFile::NAME))),
            settings_path,
            settings: Mutex::new(settings),
            state: Mutex::new(DeskState::default()),
            hand_seq: AtomicU64::new(0),
            me: me.clone(),
        })
    }

    pub fn settings(&self) -> PawnSettings {
        lock(&self.settings).clone()
    }

    fn save_settings(&self, settings: PawnSettings) -> Result<(), RiftError> {
        persist::write_json(&self.settings_path, &settings, true)?;
        *lock(&self.settings) = settings;
        Ok(())
    }

    /// `set_pawn_voice`: saved now, in effect at the next summon.
    pub fn set_voice(&self, voice: Voice) -> Result<(), RiftError> {
        let mut settings = self.settings();
        settings.voice = voice;
        self.save_settings(settings)
    }

    /// The read-only inputs, each refused when it overlaps a protected root
    /// in either direction (`inputRefused`, naming the root).
    pub fn set_inputs(&self, state: &AppState, inputs: Vec<String>) -> Result<(), RiftError> {
        let roots = protected_roots(state)?;
        for input in &inputs {
            validate_input(input, &roots)?;
        }
        let mut settings = self.settings();
        settings.inputs = inputs;
        self.save_settings(settings)
    }

    /// The last probe's record, `pawn-fence.json`.
    pub fn record(&self) -> Option<FenceRecord> {
        persist::read_json(&self.record_path)
    }

    /// The chip, judged against the ground the last summon or probe was
    /// prepared on. Before either since the stone started, a holding record
    /// cannot be judged and reads `unprobed`; a breach reads `breached`.
    pub fn fence_view(&self) -> FenceView {
        let record = self.record();
        let current = lock(&self.ground).clone();
        let (chip, changed) = match (&record, &current) {
            (Some(record), _) if record.breached() => ("breached", None),
            (Some(_), None) => (
                "unprobed",
                Some("not checked against this host since the stone started".to_string()),
            ),
            (record, Some(current)) => {
                let (chip, changed) = probe::chip(record.as_ref(), current);
                (chip, changed)
            }
            (None, None) => ("unprobed", None),
        };
        FenceView {
            chip,
            ground: record.as_ref().map(|r| r.ground.clone()),
            proved_at: record.as_ref().map(|r| r.proved_at.clone()),
            steps: record
                .map(|r| {
                    r.steps
                        .into_iter()
                        .map(|step| FenceStepView {
                            step: step.step,
                            expected: step.expected,
                            observed: step.observed,
                            ok: step.ok,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            changed,
        }
    }

    pub fn runs(&self) -> Arc<RunsFile> {
        Arc::clone(&self.runs_file)
    }

    /// The pawn's state now.
    pub fn pawn_state(&self) -> PawnStateView {
        let desk = lock(&self.state);
        match &desk.live {
            Some(live) => {
                let held = live.summoning.broker().ledger().held.map(|held| HeldView {
                    held_request_id: held.held_request_id,
                    since: held.since,
                });
                PawnStateView {
                    run_id: Some(live.summoning.run_id().to_string()),
                    state: if live.summoning.is_here() {
                        "here"
                    } else {
                        "torn"
                    },
                    since: Some(live.since.clone()),
                    exit_code: None,
                    host: Some(HostView {
                        distro: live.host.distro.clone(),
                        claude_version: live.host.claude_version.clone(),
                    }),
                    held,
                }
            }
            None => desk.last.clone().unwrap_or_else(PawnStateView::absent),
        }
    }

    fn live(&self) -> Option<Arc<Summoning>> {
        lock(&self.state)
            .live
            .as_ref()
            .map(|live| Arc::clone(&live.summoning))
    }

    /// Everything a summon and a probe share: survey the host, make the
    /// workshop and a run's mailbox, validate every mounted Windows folder by
    /// its final path, and write the relay folder.
    fn prepare(&self, state: &AppState) -> Result<Prepared, RiftError> {
        let settings = self.settings();
        let shell = shell(settings.distro.clone());
        let survey = survey(shell.as_ref(), settings.distro.as_deref())
            .map_err(|missing| no_fence(missing.sentence()))?;

        // The workshop, made on first use, with its starter and dist.
        let mods = state.library_dir().to_path_buf();
        let workshop = workshop_dir(&mods)
            .ok_or_else(|| no_fence("The mods folder has no folder around it for a workshop."))?;
        std::fs::create_dir_all(workshop.join("dist"))
            .map_err(|err| RiftError::io("could not make the workshop", err))?;
        if let Ok(mut starter) = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(workshop.join("CLAUDE.md"))
        {
            use std::io::Write;
            let _ = starter.write_all(WORKSHOP_STARTER.as_bytes());
        }

        // The run's mailbox.
        let run_id = new_run_id();
        let run_dir = self.runs_dir.join(&run_id);
        for folder in ["requests", "answers"] {
            std::fs::create_dir_all(run_dir.join(folder))
                .map_err(|err| RiftError::io("could not make the run's mailbox", err))?;
        }

        // Every mounted Windows object, by its final path (RS-9.2).
        let roots = protected_roots(state)?;
        // A trusted mount must resolve to exactly the folder the stone made:
        // a junction or link in its place is refused.
        let trusted = |name: &str, path: &Path| -> Result<String, RiftError> {
            let resolved = final_path(path)
                .map_err(|err| no_fence(format!("{name} could not be resolved: {err}.")))?;
            same_folder(name, &resolved, path, &roots)?;
            fence_path(&resolved).ok_or_else(|| no_fence(format!("{name} is not on a drive.")))
        };
        let workshop_fence = trusted("The workshop", &workshop)?;
        let run_fence = trusted("The run's mailbox", &run_dir)?;
        let mut inputs = Vec::new();
        for input in &settings.inputs {
            let resolved = validate_input(input, &roots)?;
            inputs.push(fence_path(&resolved).ok_or_else(|| {
                RiftError::pawn(PawnReason::InputRefused, "An input is not on a drive.")
            })?);
        }
        let plan = FencePlan {
            home: survey.home.clone(),
            user: survey.user.clone(),
            workshop: workshop_fence.clone(),
            run_dir: run_fence,
            run_id: run_id.clone(),
            inputs,
        };
        install_files(
            shell.as_ref(),
            &relay_files(&plan, &persona(settings.voice, &workshop_fence)),
            &[plan.claude_config()],
        )
        .map_err(no_fence)?;
        let ground =
            ground(&plan, &survey.host, RELAY.as_bytes()).map_err(|refusal| no_fence(refusal.0))?;
        *lock(&self.ground) = Some(ground.clone());
        Ok(Prepared {
            shell,
            survey,
            plan,
            workshop,
            run_dir,
            run_id,
            ground,
        })
    }

    /// `summon_pawn`: prepare, refuse a breached fence, and start the pawn.
    pub fn summon(
        &self,
        state: &Arc<AppState>,
        events: Arc<dyn EventSink>,
        cols: u16,
        rows: u16,
    ) -> Result<PawnStateView, RiftError> {
        if self.live().is_some_and(|pawn| pawn.is_here()) {
            return Err(RiftError::pawn(
                PawnReason::AlreadyHere,
                "Tamsin is already here.",
            ));
        }
        if self.record().is_some_and(|record| record.breached()) {
            return Err(RiftError::pawn(
                PawnReason::Breached,
                "The fence's last probe let something through, so Tamsin stays away until a probe holds.",
            ));
        }
        let Prepared {
            survey,
            plan,
            run_dir,
            run_id,
            ..
        } = self.prepare(state)?;
        let launch = launch(&survey.host.distro, &plan).map_err(|refusal| no_fence(refusal.0))?;

        let errands = Arc::new(StoneErrands::new(Arc::clone(state)));
        let broker = Arc::new(Broker::new(&run_id, errands, self.runs(), now));
        let ears = Arc::new(Ears {
            desk: self.me.clone(),
            events,
            run_id: run_id.clone(),
        });
        let summoning = summon(
            &launch,
            (cols, rows),
            broker,
            &RunFolders {
                run_id: run_id.clone(),
                requests: run_dir.join("requests"),
                answers: run_dir.join("answers"),
            },
            ears.clone(),
        )
        .map_err(no_fence)?;
        lock(&self.state).live = Some(Live {
            summoning: Arc::new(summoning),
            host: survey.host,
            since: now(),
        });
        let view = self.pawn_state();
        ears.events
            .emit_value(PAWN_STATE, serde_json::to_value(&view).unwrap_or_default());
        Ok(view)
    }

    /// `run_fence_probe` (BUILD-SPEC § v0.7, the probe): each step in the
    /// identical fence, read from outside; anything that landed where it
    /// must not is removed (the stone's own probe file, never anything
    /// else); the record is written, and the chip returned.
    pub fn run_probe(&self, state: &AppState) -> Result<FenceView, RiftError> {
        if self.live().is_some_and(|pawn| pawn.is_here()) {
            return Err(RiftError::pawn(
                PawnReason::PawnHere,
                "The fence cannot be probed while Tamsin is here. Let her depart first.",
            ));
        }
        let prepared = self.prepare(state)?;
        let nonce = new_run_id();
        let secret = new_run_id();
        let pawn_base = self
            .runs_dir
            .parent()
            .unwrap_or(&self.runs_dir)
            .to_path_buf();
        let io = |what: &str, err: std::io::Error| RiftError::io(what.to_string(), err);

        // The canary, and the probe-only input with a link to its folder.
        let canary_dir = pawn_base.join("probe-canary");
        std::fs::create_dir_all(&canary_dir).map_err(|e| io("could not make the canary", e))?;
        let canary = canary_dir.join("probe-canary.txt");
        std::fs::write(&canary, &nonce).map_err(|e| io("could not write the canary", e))?;
        let probe_input = pawn_base.join("probe-input");
        std::fs::create_dir_all(&probe_input)
            .map_err(|e| io("could not make the probe input", e))?;
        make_link(&probe_input.join("link"), &canary_dir)
            .map_err(|e| io("could not make the probe's link", e))?;
        let probe_dir = prepared.workshop.join("probe");
        std::fs::create_dir_all(&probe_dir)
            .map_err(|e| io("could not make the probe folder", e))?;
        plant_cmd(&probe_dir.join("planted.exe"));

        let fence_of = |path: &Path| -> String {
            final_path(path)
                .ok()
                .and_then(|resolved| fence_path(&resolved))
                .unwrap_or_else(|| path.to_string_lossy().into_owned())
        };
        let mods = state.library_dir().to_path_buf();
        let game = state.world().game.map(|game| game.root);
        let userdata = steam_userdata(game.as_deref());
        let windows_of = |path: &Path| -> String {
            final_path(path)
                .unwrap_or_else(|_| path.to_string_lossy().into_owned())
                .trim_start_matches(r"\\?\")
                .to_string()
        };
        let targets = probe::Targets {
            nonce: nonce.clone(),
            mods: fence_of(&mods),
            game: game.as_deref().map(fence_of),
            userdata: userdata.as_deref().map(fence_of),
            canary: fence_of(&canary),
            home: prepared.survey.home.clone(),
            claude_bin: prepared.plan.claude_bin(),
            mods_windows: windows_of(&mods),
            cmd_exe: "/mnt/c/Windows/System32/cmd.exe".into(),
        };
        let ctx = probe::Context {
            nonce: nonce.clone(),
            claude_version: prepared.survey.host.claude_version.clone(),
            secret: secret.clone(),
            userdata_names: userdata
                .as_deref()
                .and_then(|dir| std::fs::read_dir(dir).ok())
                .map(|entries| {
                    entries
                        .flatten()
                        .map(|e| e.file_name().to_string_lossy().into_owned())
                        .filter(|name| name.len() > 3)
                        .collect()
                })
                .unwrap_or_default(),
        };
        let probe_input_fence = fence_of(&probe_input);
        let answers = prepared.run_dir.join("answers");
        let control_file = probe_dir.join("control.txt");
        let pe_file = mods.join(format!("probe-pe-{nonce}.txt"));
        let home_claude = format!("{}/.claude/riftstone-probe-{nonce}", prepared.survey.home);

        let mut control = false;
        let mut records = Vec::new();
        for step in probe::steps(&targets) {
            let argv = bwrap_argv(
                &prepared.plan,
                &FenceCommand::Probe {
                    script: step.script.clone(),
                    probe_input: Some(probe_input_fence.clone()),
                },
            )
            .map_err(|refusal| no_fence(refusal.0))?;
            let (exit_ok, output) =
                run_step(&prepared.survey.host.distro, &prepared.plan, &argv, &secret);
            // What landed on the real disk, read outside the fence; a file
            // that landed where it must not is the stone's own, removed now.
            let landed_at = |path: &Path| -> Option<bool> {
                let landed = path.exists();
                if landed {
                    let _ = std::fs::remove_file(path);
                }
                Some(landed)
            };
            let landed = match step.id {
                "C" => Some(std::fs::read_to_string(&control_file).is_ok_and(|text| text == nonce)),
                "1" => landed_at(&mods.join(format!("probe-{nonce}.txt"))),
                "2" => game.as_ref().map_or(Some(false), |game| {
                    landed_at(&game.join(format!("probe-{nonce}.txt")))
                }),
                "5" => {
                    let asked = prepared
                        .shell
                        .run(
                            &format!(
                                "test -e {0} && rm -f {0} && echo landed",
                                sh_quote(&home_claude)
                            ),
                            b"",
                        )
                        .map(|out| out.stdout.contains("landed"))
                        .unwrap_or(false);
                    Some(asked)
                }
                "6" | "7" => landed_at(&pe_file),
                "11" => Some(
                    answers.join("x.json").symlink_metadata().is_ok()
                        || answers.join("y.json").exists(),
                ),
                _ => None,
            };
            let record = probe::judge(
                &step,
                &probe::Seen {
                    exit_ok,
                    output,
                    landed,
                },
                &ctx,
            );
            if step.id == "C" {
                control = record.ok;
            } else {
                records.push(record);
            }
        }
        let _ = std::fs::remove_file(&control_file);
        let _ = std::fs::remove_file(probe_dir.join("planted.exe"));
        let record = FenceRecord {
            ground: prepared.ground,
            proved_at: now(),
            control,
            steps: records,
        };
        persist::write_json(&self.record_path, &record, true)?;
        Ok(self.fence_view())
    }

    fn here(&self) -> Result<Arc<Summoning>, RiftError> {
        self.live()
            .ok_or_else(|| RiftError::pawn(PawnReason::NotHere, "Tamsin is not here."))
    }

    pub fn write(&self, data: &str) -> Result<(), RiftError> {
        self.here()?
            .write(data.as_bytes())
            .map_err(|err| RiftError::io("could not reach Tamsin's terminal", err))
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        if let Some(pawn) = self.live() {
            let _ = pawn.resize(cols, rows);
        }
    }

    /// `depart_pawn`: the summoning ends and its run retires. Her installs
    /// stay (Put back offers them).
    pub fn depart(&self, events: &dyn EventSink) -> Result<PawnStateView, RiftError> {
        // Taken out of `live` first, so the process's own end (which runs as
        // the kill lands) finds nothing left to depart.
        let pawn = lock(&self.state)
            .live
            .take()
            .map(|live| live.summoning)
            .ok_or_else(|| RiftError::pawn(PawnReason::NotHere, "Tamsin is not here."))?;
        let retired = pawn.depart();
        let view = PawnStateView {
            run_id: Some(pawn.run_id().to_string()),
            state: "departed",
            since: Some(now()),
            exit_code: None,
            host: None,
            held: None,
        };
        {
            let mut desk = lock(&self.state);
            desk.live = None;
            desk.last = Some(view.clone());
        }
        if !retired.persisted {
            log::warn!("the departed run's ledger is not whole on disk");
        }
        events.emit_value(PAWN_STATE, serde_json::to_value(&view).unwrap_or_default());
        Ok(view)
    }

    /// `answer_pawn_play`: the Arisen's press on the held Play, for the run
    /// and request the window was shown.
    pub fn answer_play(
        &self,
        state: &AppState,
        run_id: &str,
        held_request_id: &str,
        accept: bool,
    ) -> Result<PlayAnsweredView, RiftError> {
        let pawn = self
            .here()
            .map_err(|_| RiftError::pawn(PawnReason::StaleHeld, "Tamsin's summoning has ended."))?;
        let launch = || {
            crate::commands::play::launch_game_impl(state)
                .map(|_| ())
                .map_err(|err| match err {
                    RiftError::Refused { reason } => reason,
                    other => other.to_string(),
                })
        };
        match pawn
            .broker()
            .answer_play(run_id, held_request_id, accept, launch)
        {
            Ok(answered) => {
                if !answered.persisted {
                    log::warn!("the Arisen's answer to a held Play could not be recorded");
                }
                // The window says when the decision is not on disk (WD5).
                Ok(PlayAnsweredView {
                    decision: answered.decision,
                    persisted: answered.persisted,
                })
            }
            Err(PlayRefusal::NothingHeld) => Err(RiftError::pawn(
                PawnReason::NothingHeld,
                "No Play is waiting.",
            )),
            Err(PlayRefusal::StaleHeld) => Err(RiftError::pawn(
                PawnReason::StaleHeld,
                "That Play request is not the one waiting now.",
            )),
            Err(PlayRefusal::LedgerDown) => Err(RiftError::pawn(
                PawnReason::LedgerDown,
                "Tamsin's ledger cannot be written, so nothing more changes in this summoning.",
            )),
            Err(PlayRefusal::Launch(reason)) => Err(RiftError::refused(reason)),
        }
    }

    fn hand(&self, run_id: &str, handled: &Handled) -> PawnHand {
        let answer = &handled.answer;
        let outcome = if handled.repeated {
            "repeated"
        } else {
            match answer.outcome {
                Outcome::Done => "done",
                Outcome::Refused => "refused",
                Outcome::Held => "held",
                Outcome::Unknown => "unknown",
            }
        };
        PawnHand {
            run_id: run_id.to_string(),
            seq: self.hand_seq.fetch_add(1, Ordering::SeqCst),
            at: now(),
            tool: handled.tool.clone(),
            outcome,
            summary: answer.text.clone(),
            install_id: handled.install_id.clone(),
            refusal: (answer.outcome == Outcome::Refused).then(|| answer.text.clone()),
        }
    }

    /// The pawn's process ended by itself (she exited, or the terminal
    /// closed): the summoning is taken out of `live` and departed in full, so
    /// intake closes, calls already taken finish, and the run retires (the
    /// Heretic's WD2). Its final state stays readable.
    fn ended(&self, run_id: &str, exit_code: Option<u32>) -> Option<PawnStateView> {
        let pawn = {
            let mut desk = lock(&self.state);
            let ours = desk
                .live
                .as_ref()
                .is_some_and(|live| live.summoning.run_id() == run_id);
            if !ours {
                return None; // a departure already recorded it
            }
            desk.live.take().map(|live| live.summoning)
        };
        if let Some(pawn) = pawn {
            let retired = pawn.depart();
            if !retired.persisted {
                log::warn!("the ended run's ledger is not whole on disk");
            }
        }
        let mut desk = lock(&self.state);
        let view = PawnStateView {
            run_id: Some(run_id.to_string()),
            state: if exit_code == Some(0) {
                "departed"
            } else {
                "torn"
            },
            since: Some(now()),
            exit_code,
            host: None,
            held: None,
        };
        desk.last = Some(view.clone());
        Some(view)
    }
}

/// Is `resolved` exactly the folder the stone made at `made`?
#[cfg(windows)]
fn same_folder(
    name: &str,
    resolved: &str,
    made: &Path,
    roots: &[ProtectedRoot],
) -> Result<(), RiftError> {
    let made = made.to_string_lossy();
    check_mount_source(
        resolved,
        &MountKind::Trusted {
            name,
            expected: &made,
        },
        roots,
    )
    .map_err(|refusal| no_fence(refusal.to_string()))
}

/// The bench's POSIX paths: the canonical path is the path the stone made.
#[cfg(not(windows))]
fn same_folder(
    name: &str,
    resolved: &str,
    made: &Path,
    _roots: &[ProtectedRoot],
) -> Result<(), RiftError> {
    if Path::new(resolved) == made {
        Ok(())
    } else {
        Err(no_fence(format!(
            "{name} is not the folder the stone made for it, so the fence will not mount it."
        )))
    }
}

/// Run one probe step in the fence and collect what it said, with a 30 s
/// bound. The stone's secret is set on its own side and in `WSLENV`, so step
/// 10 sees whether anything carries it across.
fn run_step(distro: &str, plan: &FencePlan, argv: &[String], secret: &str) -> (bool, String) {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let Ok(args) = crate::pawn::fence::wsl_args(distro, plan, argv) else {
        return (false, "the step's command could not be built".into());
    };
    #[cfg(windows)]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let mut command = Command::new("wsl.exe");
        command
            .args(&args)
            .env("RIFTSTONE_PROBE_SECRET", secret)
            .env("WSLENV", "RIFTSTONE_PROBE_SECRET")
            .creation_flags(0x0800_0000);
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", &args[args.len() - 1]])
            .env("RIFTSTONE_PROBE_SECRET", secret);
        command
    };
    let Ok(mut child) = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    else {
        return (false, "the step could not start".into());
    };
    let pipe = |mut reader: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = reader.read_to_string(&mut text);
            text
        })
    };
    let out = child.stdout.take().map(|o| pipe(Box::new(o)));
    let err = child.stderr.take().map(|e| pipe(Box::new(e)));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            _ => {
                let _ = child.kill();
                break None;
            }
        }
    };
    let mut output = String::new();
    for reader in [out, err].into_iter().flatten() {
        output.push_str(&reader.join().unwrap_or_default());
    }
    (status.is_some_and(|s| s.success()), output)
}

/// A link from `link` to `target`: an NTFS junction on Windows, a symlink on
/// the bench. One left by an earlier probe is replaced.
fn make_link(link: &Path, target: &Path) -> std::io::Result<()> {
    if link.symlink_metadata().is_ok() {
        let _ = std::fs::remove_dir(link);
        let _ = std::fs::remove_file(link);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let made = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .creation_flags(0x0800_0000)
            .output()?;
        if !made.status.success() {
            return Err(std::io::Error::other("mklink /J failed"));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        std::os::unix::fs::symlink(target, link)
    }
}

/// A copy of Windows' own `cmd.exe` in the workshop's probe folder (step 6).
fn plant_cmd(at: &Path) {
    #[cfg(windows)]
    {
        let system = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        let _ = std::fs::copy(system.join("System32").join("cmd.exe"), at);
    }
    #[cfg(not(windows))]
    let _ = at;
}

/// Steam's `userdata`, beside the library that holds the game, or in the
/// default install.
fn steam_userdata(game: Option<&Path>) -> Option<PathBuf> {
    let from_game = game.and_then(|root| {
        root.ancestors()
            .find(|dir| {
                dir.file_name()
                    .is_some_and(|n| n.eq_ignore_ascii_case("steamapps"))
            })
            .and_then(Path::parent)
            .map(|steam| steam.join("userdata"))
    });
    let default = std::env::var_os("ProgramFiles(x86)")
        .map(|p| PathBuf::from(p).join("Steam").join("userdata"));
    [from_game, default]
        .into_iter()
        .flatten()
        .find(|dir| dir.is_dir())
}

/// An input's final path, refused when it overlaps a protected root.
fn validate_input(input: &str, roots: &[ProtectedRoot]) -> Result<String, RiftError> {
    let resolved = final_path(Path::new(input)).map_err(|_| {
        RiftError::pawn(
            PawnReason::InputRefused,
            "That folder could not be found, so it cannot be an input.",
        )
    })?;
    check_mount_source(&resolved, &MountKind::Input, roots)
        .map_err(|refusal| RiftError::pawn(PawnReason::InputRefused, refusal.to_string()))?;
    Ok(resolved)
}

/// What a summoning tells the window, as the room's events.
struct Ears {
    desk: Weak<Desk>,
    events: Arc<dyn EventSink>,
    run_id: String,
}

impl Watcher for Ears {
    fn output(&self, seq: u64, bytes: &[u8]) {
        let event = OutputEvent {
            seq,
            data: base64::engine::general_purpose::STANDARD.encode(bytes),
        };
        self.events.emit_value(
            PAWN_OUTPUT,
            serde_json::to_value(&event).unwrap_or_default(),
        );
    }

    fn hand(&self, handled: &Handled) {
        if let Some(desk) = self.desk.upgrade() {
            let hand = desk.hand(&self.run_id, handled);
            self.events
                .emit_value(PAWN_HANDS, serde_json::to_value(&hand).unwrap_or_default());
        }
    }

    fn ended(&self, exit_code: Option<u32>) {
        if let Some(view) = self
            .desk
            .upgrade()
            .and_then(|desk| desk.ended(&self.run_id, exit_code))
        {
            self.events
                .emit_value(PAWN_STATE, serde_json::to_value(&view).unwrap_or_default());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_run_id_is_a_proper_uuid_and_never_repeats() {
        let a = new_run_id();
        let b = new_run_id();
        assert!(crate::pawn::fence::is_uuid(&a), "{a}");
        assert_ne!(a, b);
        assert_eq!(&a[14..15], "4", "version 4");
    }

    #[test]
    fn the_workshop_sits_beside_the_mods_folder() {
        assert_eq!(
            workshop_dir(Path::new("/games/The Rift/mods")),
            Some(PathBuf::from("/games/The Rift/mods (workshop)"))
        );
        assert_eq!(workshop_dir(Path::new("/")), None);
    }

    #[test]
    fn the_settings_keep_their_voice_and_inputs() {
        let dir = tempfile::tempdir().unwrap();
        let desk = Desk::new(dir.path());
        assert_eq!(desk.settings().voice, Voice::Sparing);
        desk.set_voice(Voice::Full).unwrap();
        let again = Desk::new(dir.path());
        assert_eq!(again.settings().voice, Voice::Full, "saved in pawn.json");
        assert_eq!(again.pawn_state().state, "absent");
    }

    /// The Heretic's WD2: a pawn whose process exits by itself is departed
    /// in full. Intake closes, the run retires, and a request first posted
    /// after the exit is never acted on.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_pawn_that_exits_by_itself_is_departed_and_its_run_retired() {
        use crate::pawn::mailbox::{Done, Errands, PlannedInstall};
        use crate::pawn::summon::Launch;
        use rift_deploy::Actor;
        use serde_json::Value;

        #[derive(Default)]
        struct Counting(std::sync::atomic::AtomicUsize);
        impl Errands for Counting {
            fn search_library(&self, _: &str, _: u64) -> Result<Value, String> {
                Ok(Value::Null)
            }
            fn get_option(&self, _: &str) -> Result<Value, String> {
                Ok(Value::Null)
            }
            fn list_installed(&self) -> Result<Value, String> {
                Ok(Value::Null)
            }
            fn plan_install(&self, _: &str) -> Result<PlannedInstall, String> {
                Err("no".into())
            }
            fn install(&self, _: &str, _: &Actor, _: &PlannedInstall) -> Result<Done, String> {
                Err("no".into())
            }
            fn uninstall(&self, _: &str, _: &Actor) -> Result<Done, String> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Err("no".into())
            }
            fn hold_play(&self, _: &str, _: &str) -> Result<(), String> {
                Ok(())
            }
            fn drop_held_play(&self, _: &str) {}
            fn read_reframework_log(&self, _: u64) -> Result<Value, String> {
                Ok(Value::Null)
            }
        }

        let dir = tempfile::tempdir().unwrap();
        let desk = Desk::new(dir.path());
        let run_id = new_run_id();
        let run_dir = dir.path().join("run");
        for folder in ["requests", "answers"] {
            std::fs::create_dir_all(run_dir.join(folder)).unwrap();
        }
        let errands = Arc::new(Counting::default());
        let broker = Arc::new(Broker::new(&run_id, errands.clone(), desk.runs(), now));
        let events: Arc<dyn EventSink> = Arc::new(crate::events::RecordedEvents::default());
        let ears = Arc::new(Ears {
            desk: Arc::downgrade(&desk),
            events,
            run_id: run_id.clone(),
        });
        // A pawn that waits a moment, then leaves by itself, cleanly.
        let launch = Launch {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), "sleep 0.3; exit 0".into()],
            env: Vec::new(),
        };
        let summoning = summon(
            &launch,
            (80, 24),
            broker,
            &RunFolders {
                run_id: run_id.clone(),
                requests: run_dir.join("requests"),
                answers: run_dir.join("answers"),
            },
            ears,
        )
        .unwrap();
        lock(&desk.state).live = Some(Live {
            summoning: Arc::new(summoning),
            host: Host {
                distro: "bench".into(),
                claude_version: "0".into(),
                kernel: "0".into(),
            },
            since: now(),
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while desk.pawn_state().state != "departed" {
            assert!(
                std::time::Instant::now() < deadline,
                "never departed: {:?}",
                desk.pawn_state()
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(desk.pawn_state().exit_code, Some(0));
        assert!(desk.live().is_none(), "nothing is left live");
        let ledger = desk
            .runs()
            .runs()
            .into_iter()
            .find(|r| r.run_id == run_id)
            .unwrap();
        assert!(ledger.ended_at.is_some(), "the run retired");
        // A request first posted after the exit stays unread and unacted.
        let id = "55555555-2222-4333-8444-555555555555";
        let request = run_dir.join("requests").join(format!("{id}.json"));
        std::fs::write(
            &request,
            serde_json::json!({ "id": id, "tool": "uninstall", "args": { "installId": "a#b" } })
                .to_string(),
        )
        .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(400));
        assert!(request.exists(), "never taken");
        assert_eq!(errands.0.load(Ordering::SeqCst), 0, "never acted on");
    }
}
