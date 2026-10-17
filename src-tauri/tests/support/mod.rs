//! The spine's test bench: a fake Dragon's Dogma 2 folder, the stone's one
//! mods folder (`<install dir>/mods`) holding an invented library — two zips
//! and a Nexus download, built at run time — and a stand-in for Nexus Mods on
//! a loopback port.
//!
//! Nothing here reads or writes the real game, the real mods folder, app data
//! or credential store, and nothing starts a program or opens a window: every
//! path is inside one tempdir, discovery is fixed to it, the key store lives
//! in memory, Play goes to a recording launcher and the process list is a
//! switch the test flips.

#![allow(dead_code)] // each test binary uses its own share of the bench

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Cursor, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use riftstone_lib::events::RecordedEvents;
use riftstone_lib::keys::MemoryKeyStore;
use riftstone_lib::persist::DataDirs;
use riftstone_lib::play::{GameCadence, RecordingLauncher, SwitchedGameWatch};
use riftstone_lib::protocol::MemoryRegistry;
use riftstone_lib::state::{AppState, Discovery, SpineConfig};

pub const ERMINE_MESH: &str = "natives/stm/appsystem/cloak/ermine_cape.mesh.260421070";
pub const WOOL_MESH: &str = "natives/stm/appsystem/hood/wool_hood.mesh.260421070";
pub const BANTER_LUA: &str = "reframework/autorun/pawn_banter.lua";
pub const BANTER_ZIP: &str = "Pawn Banter 1234 1 2026-10-14T14-23Z Ab3dE5fG7.zip";
/// 2026-10-14T14:23Z, the upload stamp in [`BANTER_ZIP`], in unix seconds.
pub const BANTER_UPLOADED_S: u64 = 1_791_987_780;

pub struct Bench {
    pub dir: tempfile::TempDir,
    pub game: PathBuf,
    /// The one library: the `mods` folder beside the stone's install.
    pub mods: PathBuf,
    /// The bench's desktop: what Play started, which folder was shown. A
    /// launch makes [`Bench::game_watch`] say the game runs.
    pub launcher: Arc<RecordingLauncher>,
    /// The bench's process list: the game runs when the test says so.
    pub game_watch: Arc<SwitchedGameWatch>,
    /// The bench's registry for nxm:// links: empty until a test fills it.
    pub registry: Arc<MemoryRegistry>,
    /// The stone's own exe on the bench (never run): what the nxm handler
    /// registers.
    pub exe: PathBuf,
}

impl Bench {
    pub fn new() -> Bench {
        let dir = tempfile::tempdir().expect("a tempdir");
        let root = dir.path();
        let game = root.join("Steam/steamapps/common/Dragons Dogma 2");
        let mods = root.join("The Riftstone/mods");
        for folder in [&game.join("dlc"), &mods] {
            fs::create_dir_all(folder).unwrap();
        }

        // The game: vanilla files the Riftstone must never touch.
        fs::write(game.join("DD2.exe"), b"").unwrap();
        fs::write(game.join("re_chunk_000.pak"), b"KPKA vanilla chunk").unwrap();
        fs::write(game.join("re_chunk_000.pak.patch_001.pak"), [0u8; 144]).unwrap();
        fs::write(game.join("dlc/re_dlc_stm_2581000.pak"), b"KPKA dlc").unwrap();

        // The library: a collection with a menu, a pak mod, a Nexus download.
        write_zip(
            &mods.join("Vernworth Cloaks Collection.zip"),
            &[
                ("Vernworth Cloaks Collection/README.txt", b"Read me by the fire."),
                (
                    "Vernworth Cloaks Collection/Cloaks/modinfo.ini",
                    b"name=Cloaks\r\nDummyMod=True\r\n",
                ),
                (
                    "Vernworth Cloaks Collection/Cloaks/Ermine Cape/modinfo.ini",
                    b"name=Ermine Cape\r\naddonfor=Cloaks\r\nauthor=Vern\r\nversion=2\r\ndescription=A white cape   with black tips,\tfor the cold roads north of Vernworth.\r\nscreenshot=screenshot.png\r\n",
                ),
                (
                    "Vernworth Cloaks Collection/Cloaks/Ermine Cape/screenshot.png",
                    &png(640, 360),
                ),
                (
                    &format!("Vernworth Cloaks Collection/Cloaks/Ermine Cape/{ERMINE_MESH}"),
                    b"ermine cape mesh bytes",
                ),
                (
                    "Vernworth Cloaks Collection/Cloaks/Wool Hood/modinfo.ini",
                    b"name=Wool Hood\r\naddonfor=Cloaks\r\n",
                ),
                (
                    &format!("Vernworth Cloaks Collection/Cloaks/Wool Hood/{WOOL_MESH}"),
                    b"wool hood mesh bytes",
                ),
            ],
        );
        let pak = pak(&[
            rift_pak::path_hash(ERMINE_MESH),
            rift_pak::path_hash("natives/stm/effect/lantern/lantern_glow.tex.251211553"),
        ]);
        write_zip(
            &mods.join("Lantern Glow Recolor.zip"),
            &[
                (
                    "Lantern Glow/modinfo.ini",
                    b"name=Lantern Glow Recolor\r\nversion=2.1\r\n",
                ),
                ("Lantern Glow/LanternGlow.pak", &pak),
            ],
        );
        write_zip(
            &mods.join(BANTER_ZIP),
            &[(&format!("Pawn Banter/{BANTER_LUA}"), b"-- pawns talk less")],
        );

        let game_watch = Arc::new(SwitchedGameWatch::default());
        let exe = root.join("The Riftstone/riftstone.exe");
        Bench {
            dir,
            game,
            mods,
            launcher: Arc::new(RecordingLauncher::starting(Arc::clone(&game_watch))),
            game_watch,
            registry: Arc::new(MemoryRegistry::default()),
            exe,
        }
    }

    /// The spine's configuration over this bench.
    pub fn config(&self, dirs: DataDirs) -> SpineConfig {
        SpineConfig {
            dirs,
            discovery: Discovery::Fixed {
                game_root: Some(self.game.clone()),
            },
            library_dir: self.mods.clone(),
            app_version: "0.3.0".into(),
            // Nothing listens there: no test reaches the real Nexus.
            nexus_endpoints: Some((dead_address(), dead_address())),
            watch_library: false,
            launcher: self.launcher.clone(),
            game_watch: self.game_watch.clone(),
            // A brisk watcher, so the tests that run it wait milliseconds.
            game_cadence: GameCadence {
                every: Duration::from_millis(40),
                after_launch: Duration::from_millis(10),
                after_launch_for: Duration::from_secs(120),
            },
            protocols: self.registry.clone(),
            exe: self.exe.clone(),
        }
    }

    /// The spine's configuration over this bench, its Nexus at `stand_in`.
    pub fn config_at(&self, stand_in: &StandIn) -> SpineConfig {
        let mut config = self.config(DataDirs::under(&self.data_home()));
        config.nexus_endpoints = Some((stand_in.graphql_url(), stand_in.v1_base()));
        config
    }

    /// Open the spine with a personal API key already in the credential
    /// store (an invented one).
    pub fn open_keyed(
        &self,
        config: SpineConfig,
        key: &str,
    ) -> (Arc<AppState>, Arc<RecordedEvents>) {
        let events = Arc::new(RecordedEvents::default());
        let state = AppState::open(
            config,
            events.clone(),
            Box::new(MemoryKeyStore::holding(key)),
        );
        (state, events)
    }

    /// The app data home inside the bench.
    pub fn data_home(&self) -> PathBuf {
        self.dir.path().join("riftstone-data")
    }

    /// Open the spine over this bench (startup steps 1–3).
    pub fn open(&self, config: SpineConfig) -> (Arc<AppState>, Arc<RecordedEvents>) {
        let events = Arc::new(RecordedEvents::default());
        let state = AppState::open(config, events.clone(), Box::new(MemoryKeyStore::default()));
        (state, events)
    }

    /// Write the game's journal by hand, as an older Riftstone left it.
    pub fn seed_journal(&self, dirs: &DataDirs, journal: &serde_json::Value) {
        fs::create_dir_all(dirs.state_dir()).unwrap();
        fs::write(
            rift_deploy::journal_path(dirs.state_dir(), &self.game),
            serde_json::to_vec_pretty(journal).unwrap(),
        )
        .unwrap();
    }

    /// The game's journal as it is on disk now, raw.
    pub fn journal_on_disk(&self, dirs: &DataDirs) -> serde_json::Value {
        let path = rift_deploy::journal_path(dirs.state_dir(), &self.game);
        serde_json::from_slice(&fs::read(path).expect("the journal is on disk")).unwrap()
    }

    /// REFramework in the game folder: its DLL, its config with both switches
    /// as given, and its faulty-file list.
    pub fn install_reframework(
        &self,
        loose_file_loader: bool,
        pak_directory: bool,
        faulty: &[&str],
    ) {
        fs::write(self.game.join("dinput8.dll"), b"MZ reframework").unwrap();
        fs::write(
            self.game.join("re2_fw_config.txt"),
            format!(
                "LooseFileLoader_Enabled={loose_file_loader}\r\nIntegrityCheckBypass_LoadPakDirectory={pak_directory}\r\nFaultyFileDetector_Enabled=true\r\n"
            ),
        )
        .unwrap();
        fs::write(
            self.game.join("reframework_faulty_files.txt"),
            faulty.join("\r\n"),
        )
        .unwrap();
    }

    /// Steam's manifest for the game, carrying `build`.
    pub fn set_steam_build(&self, build: &str) {
        let steamapps = self.game.parent().unwrap().parent().unwrap();
        fs::write(
            steamapps.join("appmanifest_2054970.acf"),
            format!(
                "\"AppState\"\n{{\n\t\"appid\"\t\t\"2054970\"\n\t\"buildid\"\t\t\"{build}\"\n}}\n"
            ),
        )
        .unwrap();
    }

    /// The ids `src/pages/PreflightPage.vue` draws rows for itself (its
    /// `OWN_CHECKS`): a check under one of them is skipped by the page.
    pub fn preflight_own_rows() -> Vec<String> {
        let page = fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/pages/PreflightPage.vue"),
        )
        .expect("the window's Preflight page");
        let start = page.find("OWN_CHECKS = new Set([").expect("OWN_CHECKS") + 22;
        let end = start + page[start..].find("])").unwrap();
        page[start..end]
            .split(',')
            .map(|id| id.trim().trim_matches('"').to_string())
            .filter(|id| !id.is_empty())
            .collect()
    }

    /// Every file under `dir` with its bytes — to prove a folder untouched.
    pub fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
        let mut files = BTreeMap::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(folder) = stack.pop() {
            for entry in fs::read_dir(&folder).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    let rel = path
                        .strip_prefix(dir)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/");
                    files.insert(rel, fs::read(&path).unwrap());
                }
            }
        }
        files
    }
}

pub fn write_zip(path: &Path, files: &[(&str, &[u8])]) {
    let file = fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in files {
        zip.start_file(*name, options).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
}

/// A real PNG, so thumbnails have something to decode.
pub fn png(width: u32, height: u32) -> Vec<u8> {
    let image = image::RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([(x % 256) as u8, (y % 256) as u8, 0x60])
    });
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgb8(image)
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .unwrap();
    bytes
}

/// A plain KPKA v4.0 table of contents carrying these hashes.
pub fn pak(hashes: &[rift_pak::PathHash]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"KPKA");
    bytes.extend_from_slice(&[4, 0, 0, 0]);
    bytes.extend_from_slice(&u32::try_from(hashes.len()).unwrap().to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    for hash in hashes {
        bytes.extend_from_slice(&hash.lower.to_le_bytes());
        bytes.extend_from_slice(&hash.upper.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 40]);
    }
    bytes
}

/// Every event recorded, and one event that holds its sender until the test
/// opens the gate — a scan stopped at its first progress report, say.
pub struct GatedEvents {
    pub recorded: RecordedEvents,
    gated: &'static str,
    open: Mutex<bool>,
    opened: std::sync::Condvar,
}

impl GatedEvents {
    pub fn closed_on(gated: &'static str) -> GatedEvents {
        GatedEvents {
            recorded: RecordedEvents::default(),
            gated,
            open: Mutex::new(false),
            opened: std::sync::Condvar::new(),
        }
    }

    pub fn open(&self) {
        *self.open.lock().unwrap() = true;
        self.opened.notify_all();
    }
}

impl riftstone_lib::events::EventSink for GatedEvents {
    fn emit_value(&self, event: &'static str, payload: serde_json::Value) {
        riftstone_lib::events::EventSink::emit_value(&self.recorded, event, payload);
        if event == self.gated {
            let mut open = self.open.lock().unwrap();
            while !*open {
                open = self.opened.wait(open).unwrap();
            }
        }
    }
}

/// A named pipe at `path`: whoever opens it to read waits until something
/// opens it to write — a file that takes as long to read as the test wants.
#[cfg(unix)]
pub fn mkfifo(path: &Path) {
    let status = std::process::Command::new("mkfifo")
        .arg(path)
        .status()
        .expect("mkfifo runs");
    assert!(status.success(), "mkfifo {}", path.display());
}

/// Let the reader of the pipe at `path` go: open it to write, write `bytes`,
/// close. Gives up after 10 s when nobody reads (the writer thread is left).
#[cfg(unix)]
pub fn feed_fifo(path: &Path, bytes: &[u8]) {
    let (path, bytes) = (path.to_path_buf(), bytes.to_vec());
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if let Ok(mut pipe) = fs::OpenOptions::new().write(true).open(&path) {
            let _ = pipe.write_all(&bytes);
        }
        let _ = tx.send(());
    });
    let _ = rx.recv_timeout(Duration::from_secs(10));
}

/// Run `work` on its own thread; `None` when it has not finished in `limit`.
pub fn within<T: Send + 'static>(
    limit: Duration,
    work: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(work());
    });
    rx.recv_timeout(limit).ok()
}

/// The option id of the library option named `name` (menu entries included).
pub fn option_named(state: &AppState, name: &str) -> String {
    riftstone_lib::commands::library::search_impl(
        state,
        &riftstone_lib::dto::SearchQuery {
            text: String::new(),
            filters: riftstone_lib::dto::SearchFilters {
                include_menu_only: true,
                ..riftstone_lib::dto::SearchFilters::default()
            },
            limit: 1_000,
        },
    )
    .into_iter()
    .find(|hit| hit.name == name)
    .unwrap_or_else(|| panic!("{name} is in the library"))
    .option_id
}

/// A loopback address nothing listens on.
pub fn dead_address() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    format!("http://{addr}")
}

// ---------------------------------------------------------------------------
// A stand-in for Nexus Mods
// ---------------------------------------------------------------------------

/// One request as the stand-in saw it.
#[derive(Debug, Clone)]
pub struct Seen {
    pub method: String,
    pub path: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Seen {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// What the stand-in answers: status, extra headers, JSON body.
pub struct Answer {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: serde_json::Value,
}

impl Answer {
    pub fn json(body: serde_json::Value) -> Answer {
        Answer {
            status: 200,
            headers: Vec::new(),
            body,
        }
    }
}

pub struct StandIn {
    addr: SocketAddr,
    seen: Arc<Mutex<Vec<Seen>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl StandIn {
    pub fn start(answer: impl Fn(&Seen) -> Answer + Send + 'static) -> StandIn {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let seen = Arc::clone(&seen);
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                    let Ok(read_half) = stream.try_clone() else {
                        continue;
                    };
                    let Some(request) = read_request(&mut BufReader::new(read_half)) else {
                        continue;
                    };
                    seen.lock().unwrap().push(request.clone());
                    write_answer(stream, &answer(&request));
                }
            })
        };
        StandIn {
            addr,
            seen,
            stop,
            thread: Some(thread),
        }
    }

    pub fn graphql_url(&self) -> String {
        format!("http://{}/v2/graphql", self.addr)
    }

    pub fn v1_base(&self) -> String {
        format!("http://{}/v1", self.addr)
    }

    pub fn seen(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }
}

impl Drop for StandIn {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn read_request(reader: &mut impl BufRead) -> Option<Seen> {
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_string();
    let path = parts.next()?.to_string();
    let mut headers = Vec::new();
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).ok()?;
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            headers.push((name.trim().to_string(), value.trim().to_string()));
        }
    }
    let length = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0; length];
    reader.read_exact(&mut body).ok()?;
    Some(Seen {
        method,
        path,
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

/// Wait until `done` says so; panics with `what` after `limit`.
pub fn wait_for(limit: Duration, what: &str, mut done: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + limit;
    while !done() {
        assert!(
            std::time::Instant::now() < deadline,
            "waited too long: {what}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

// ---------------------------------------------------------------------------
// A stand-in for Nexus's download servers (the CDN)
// ---------------------------------------------------------------------------

/// What the CDN stand-in sends: any bytes, optionally declaring another
/// length than it sends (a body cut off), optionally slowly.
#[derive(Clone)]
pub struct Delivery {
    pub status: u16,
    pub body: Vec<u8>,
    /// The `Content-Length` to declare; `None` declares the body's own.
    pub declared: Option<usize>,
    /// Send `.0` bytes, then wait `.1`, until the body is out.
    pub pace: Option<(usize, Duration)>,
}

impl Delivery {
    pub fn bytes(body: &[u8]) -> Delivery {
        Delivery {
            status: 200,
            body: body.to_vec(),
            declared: None,
            pace: None,
        }
    }
}

pub struct Cdn {
    addr: SocketAddr,
    seen: Arc<Mutex<Vec<Seen>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Cdn {
    pub fn start(delivery: impl Fn(&Seen) -> Delivery + Send + 'static) -> Cdn {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let seen = Arc::clone(&seen);
            let stop = Arc::clone(&stop);
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                    let Ok(read_half) = stream.try_clone() else {
                        continue;
                    };
                    let Some(request) = read_request(&mut BufReader::new(read_half)) else {
                        continue;
                    };
                    seen.lock().unwrap().push(request.clone());
                    deliver(stream, &delivery(&request));
                }
            })
        };
        Cdn {
            addr,
            seen,
            stop,
            thread: Some(thread),
        }
    }

    /// A download link on this stand-in, signed the way a CDN signs one.
    pub fn link(&self, path: &str) -> String {
        format!(
            "http://{}{path}?md5=SIGNED-BY-THE-CDN&expires=1791987830",
            self.addr
        )
    }

    pub fn seen(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }
}

impl Drop for Cdn {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn deliver(mut stream: TcpStream, delivery: &Delivery) {
    let declared = delivery.declared.unwrap_or(delivery.body.len());
    let head = format!(
        "HTTP/1.1 {} Delivery\r\nContent-Type: application/octet-stream\r\nContent-Length: {declared}\r\nConnection: close\r\n\r\n",
        delivery.status
    );
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }
    match delivery.pace {
        None => {
            let _ = stream.write_all(&delivery.body);
        }
        Some((step, pause)) => {
            for piece in delivery.body.chunks(step.max(1)) {
                if stream.write_all(piece).is_err() || stream.flush().is_err() {
                    return;
                }
                std::thread::sleep(pause);
            }
        }
    }
    let _ = stream.flush();
}

fn write_answer(mut stream: TcpStream, answer: &Answer) {
    let body = serde_json::to_vec(&answer.body).unwrap();
    let mut head = format!(
        "HTTP/1.1 {} Answer\r\nContent-Type: application/json\r\n",
        answer.status
    );
    for (name, value) in &answer.headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str(&format!(
        "Content-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    ));
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&body);
    let _ = stream.flush();
}
