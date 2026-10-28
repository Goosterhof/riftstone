//! A summoning (BUILD-SPEC § v0.7, Modules): Tamsin's terminal and her
//! mailbox, for one run.
//!
//! [`summon`] starts the fence's command in a PTY made for this pawn alone
//! (`portable-pty`, as the Mezzanine does), pumps its output in order, and
//! runs the mailbox loop. Every 100 ms it takes each `<uuid>.json` from the
//! run's `requests\` by the pinned handle ([`MailFolder::take`]), hands it to
//! the run's [`Broker`] on its own thread, and writes the answer into
//! `answers\` ([`MailFolder::write_new`]). A request refused unread (a link,
//! a pipe, one byte past 64 KiB) is answered `refused` under its id when the
//! name is a proper one, so the relay never waits out its 120 s for nothing.
//!
//! [`Summoning::depart`] ends it in the contract's order: the pawn's process
//! is killed (a forced kill after 3 s), the loop stops reading, every call
//! already taken is finished and answered, and the run retires.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

use crate::pawn::fence::{bwrap_argv, wsl_args, FenceCommand, FencePlan, FenceRefusal};
use crate::pawn::folder::MailFolder;
use crate::pawn::mailbox::{request_id_of, Answer, Broker, Handled, Outcome, Retired};

/// How often the mailbox loop looks for requests.
const MAILBOX_POLL: Duration = Duration::from_millis(100);
/// How long a departing pawn has before a forced kill.
const DEPART_GRACE: Duration = Duration::from_secs(3);

/// The process a summoning starts in its PTY.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    pub program: String,
    pub args: Vec<String>,
    /// Set on the launched process (`WSLENV` emptied on Windows).
    pub env: Vec<(String, String)>,
}

impl Launch {
    /// The stone's launch on Windows: `wsl.exe … /bin/sh -c 'exec bwrap …'`,
    /// with `WSLENV` emptied (RS-9.1, RS-9.5).
    pub fn through_wsl(distro: &str, plan: &FencePlan) -> Result<Launch, FenceRefusal> {
        let argv = bwrap_argv(plan, &FenceCommand::Pawn)?;
        Ok(Launch {
            program: "wsl.exe".into(),
            args: wsl_args(distro, plan, &argv)?,
            env: vec![("WSLENV".into(), String::new())],
        })
    }

    /// The same fence on the Linux bench: the very script `wsl.exe` would be
    /// handed, run by `/bin/sh` itself.
    pub fn on_bench(plan: &FencePlan) -> Result<Launch, FenceRefusal> {
        let argv = bwrap_argv(plan, &FenceCommand::Pawn)?;
        let mut args = wsl_args("bench", plan, &argv)?;
        let script = args.pop().unwrap_or_default();
        Ok(Launch {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), script],
            env: Vec::new(),
        })
    }
}

/// What a summoning tells the window (the room's events, wired by the
/// commands slice): each read of the terminal in order, each answered call,
/// and the end.
pub trait Watcher: Send + Sync {
    fn output(&self, seq: u64, bytes: &[u8]);
    fn hand(&self, handled: &Handled);
    fn ended(&self, exit_code: Option<u32>);
}

type SharedChild = Arc<Mutex<Box<dyn Child + Send + Sync>>>;

/// One pawn, summoned.
pub struct Summoning {
    run_id: String,
    broker: Arc<Broker>,
    writer: Mutex<Box<dyn Write + Send>>,
    master: Mutex<Box<dyn MasterPty + Send>>,
    child: SharedChild,
    stop: Arc<AtomicBool>,
    mailbox: Mutex<Option<JoinHandle<()>>>,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The run a summoning serves: its id and its two mailbox folders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunFolders {
    pub run_id: String,
    pub requests: std::path::PathBuf,
    pub answers: std::path::PathBuf,
}

/// Start the pawn: the PTY (`cols` × `rows`), its reader, and the mailbox
/// loop over the run's two folders (opened once, by handle, refused if
/// either is a link).
pub fn summon(
    launch: &Launch,
    (cols, rows): (u16, u16),
    broker: Arc<Broker>,
    run: &RunFolders,
    watcher: Arc<dyn Watcher>,
) -> Result<Summoning, String> {
    let requests = MailFolder::open(&run.requests)
        .map_err(|err| format!("The run's request folder could not be opened: {err}."))?;
    let answers = MailFolder::open(&run.answers)
        .map_err(|err| format!("The run's answer folder could not be opened: {err}."))?;

    let pty = native_pty_system()
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|err| format!("The stone could not make Tamsin's terminal: {err}."))?;
    let mut command = CommandBuilder::new(&launch.program);
    command.args(&launch.args);
    for (key, value) in &launch.env {
        command.env(key, value);
    }
    let child = pty
        .slave
        .spawn_command(command)
        .map_err(|err| format!("The stone could not start Tamsin: {err}."))?;
    drop(pty.slave);
    let writer = pty
        .master
        .take_writer()
        .map_err(|err| format!("The stone could not reach Tamsin's terminal: {err}."))?;
    let reader = pty
        .master
        .try_clone_reader()
        .map_err(|err| format!("The stone could not read Tamsin's terminal: {err}."))?;
    let child: SharedChild = Arc::new(Mutex::new(child));

    let pump_child = Arc::clone(&child);
    let pump_watcher = Arc::clone(&watcher);
    std::thread::spawn(move || pump(reader, &pump_child, pump_watcher.as_ref()));

    let stop = Arc::new(AtomicBool::new(false));
    let loop_stop = Arc::clone(&stop);
    let loop_broker = Arc::clone(&broker);
    let mailbox = std::thread::spawn(move || {
        mailbox_loop(&requests, &answers, &loop_broker, watcher, &loop_stop);
    });

    Ok(Summoning {
        run_id: run.run_id.clone(),
        broker,
        writer: Mutex::new(writer),
        master: Mutex::new(pty.master),
        child,
        stop,
        mailbox: Mutex::new(Some(mailbox)),
    })
}

/// Read the terminal until it closes, then report the exit.
fn pump(mut reader: Box<dyn Read + Send>, child: &SharedChild, watcher: &dyn Watcher) {
    let mut buffer = [0u8; 8192];
    let mut seq = 0u64;
    loop {
        match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                watcher.output(seq, &buffer[..n]);
                seq += 1;
            }
        }
    }
    let code = lock(child).wait().ok().map(|status| status.exit_code());
    watcher.ended(code);
}

/// The answer a request refused unread gets, when its name is a proper one.
fn unread_answer(name: &str, sentence: String) -> Option<Answer> {
    request_id_of(name).map(|id| Answer {
        id: id.to_string(),
        outcome: Outcome::Refused,
        text: sentence,
        data: serde_json::Value::Null,
    })
}

fn mailbox_loop(
    requests: &MailFolder,
    answers: &MailFolder,
    broker: &Arc<Broker>,
    watcher: Arc<dyn Watcher>,
    stop: &AtomicBool,
) {
    let answers = Arc::new(answers);
    std::thread::scope(|scope| {
        while !stop.load(Ordering::SeqCst) {
            let names = requests.names().unwrap_or_default();
            for name in names.into_iter().filter(|name| name.ends_with(".json")) {
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                match requests.take(&name) {
                    Ok(bytes) => {
                        let broker = Arc::clone(broker);
                        let watcher = Arc::clone(&watcher);
                        let answers = Arc::clone(&answers);
                        // Each call on its own thread: the broker reserves
                        // its id and caps the work in flight at four.
                        scope.spawn(move || {
                            if let Some(handled) = broker.handle(&name, &bytes) {
                                write_answer(&answers, &name, &handled.answer);
                                watcher.hand(&handled);
                            }
                        });
                    }
                    Err(refusal) => {
                        log::warn!("a pawn request was refused unread: {}", refusal.sentence());
                        if let Some(answer) = unread_answer(&name, refusal.sentence()) {
                            write_answer(&answers, &name, &answer);
                        }
                    }
                }
            }
            std::thread::sleep(MAILBOX_POLL);
        }
        // Leaving the scope finishes every call already taken: its answer
        // is written, where no relay may be left to read it.
    });
}

fn write_answer(answers: &MailFolder, name: &str, answer: &Answer) {
    if let Err(err) = answers.write_new(name, &answer.to_bytes()) {
        log::warn!("a pawn answer could not be written: {err}");
    }
}

impl Summoning {
    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    pub fn broker(&self) -> &Arc<Broker> {
        &self.broker
    }

    /// Keystrokes and pastes, to the terminal.
    pub fn write(&self, bytes: &[u8]) -> std::io::Result<()> {
        let mut writer = lock(&self.writer);
        writer.write_all(bytes)?;
        writer.flush()
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), String> {
        lock(&self.master)
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|err| err.to_string())
    }

    /// Is the pawn's process still running?
    pub fn is_here(&self) -> bool {
        matches!(lock(&self.child).try_wait(), Ok(None))
    }

    /// End the summoning: the process is killed (forced after 3 s), the
    /// mailbox stops reading, every call already taken is finished and
    /// answered, and the run retires.
    pub fn depart(&self) -> Retired {
        {
            let mut child = lock(&self.child);
            let _ = child.kill();
            let deadline = Instant::now() + DEPART_GRACE;
            while matches!(child.try_wait(), Ok(None)) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(50));
            }
            if matches!(child.try_wait(), Ok(None)) {
                let _ = child.kill();
            }
        }
        self.stop.store(true, Ordering::SeqCst);
        if let Some(mailbox) = lock(&self.mailbox).take() {
            let _ = mailbox.join();
        }
        self.broker.retire()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pawn::mailbox::{Done, Errands, LedgerSink, PlannedInstall, RunLedger};
    use rift_deploy::Actor;
    use serde_json::Value;
    use std::sync::atomic::AtomicUsize;

    const RUN: &str = "0b5e2c6e-4a8d-4d3e-9f4b-2a1c7d9e8f00";

    #[derive(Default)]
    struct Ears {
        output: Mutex<Vec<u8>>,
        seqs: Mutex<Vec<u64>>,
        hands: Mutex<Vec<Handled>>,
        ended: Mutex<Option<Option<u32>>>,
    }

    impl Watcher for Ears {
        fn output(&self, seq: u64, bytes: &[u8]) {
            lock(&self.seqs).push(seq);
            lock(&self.output).extend_from_slice(bytes);
        }
        fn hand(&self, handled: &Handled) {
            lock(&self.hands).push(handled.clone());
        }
        fn ended(&self, exit_code: Option<u32>) {
            *lock(&self.ended) = Some(exit_code);
        }
    }

    impl Ears {
        fn wait_for(&self, what: &str, done: impl Fn(&Ears) -> bool) {
            let deadline = Instant::now() + Duration::from_secs(10);
            while !done(self) {
                assert!(Instant::now() < deadline, "{what} never came");
                std::thread::sleep(Duration::from_millis(20));
            }
        }

        fn said(&self) -> String {
            String::from_utf8_lossy(&lock(&self.output)).into_owned()
        }
    }

    #[derive(Default)]
    struct Quiet {
        listed: AtomicUsize,
    }

    impl Errands for Quiet {
        fn search_library(&self, _: &str, _: u64) -> Result<Value, String> {
            Ok(Value::Null)
        }
        fn get_option(&self, _: &str) -> Result<Value, String> {
            Ok(Value::Null)
        }
        fn list_installed(&self) -> Result<Value, String> {
            self.listed.fetch_add(1, Ordering::SeqCst);
            Ok(serde_json::json!({ "installs": [] }))
        }
        fn plan_install(&self, _: &str) -> Result<PlannedInstall, String> {
            Err("no".into())
        }
        fn install(&self, _: &str, _: &Actor, _: &PlannedInstall) -> Result<Done, String> {
            Err("no".into())
        }
        fn uninstall(&self, _: &str, _: &Actor) -> Result<Done, String> {
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

    #[derive(Default)]
    struct Kept(Mutex<Vec<RunLedger>>);

    impl LedgerSink for Kept {
        fn record(&self, ledger: &RunLedger) -> Result<(), String> {
            lock(&self.0).push(ledger.clone());
            Ok(())
        }
    }

    struct Run {
        _root: tempfile::TempDir,
        requests: std::path::PathBuf,
        answers: std::path::PathBuf,
        errands: Arc<Quiet>,
        broker: Arc<Broker>,
        ears: Arc<Ears>,
    }

    fn run() -> Run {
        let root = tempfile::tempdir().unwrap();
        let requests = root.path().join("requests");
        let answers = root.path().join("answers");
        std::fs::create_dir_all(&requests).unwrap();
        std::fs::create_dir_all(&answers).unwrap();
        let errands = Arc::new(Quiet::default());
        let broker = Arc::new(Broker::new(
            RUN,
            errands.clone(),
            Arc::new(Kept::default()),
            crate::pawn::mailbox::now,
        ));
        Run {
            _root: root,
            requests,
            answers,
            errands,
            broker,
            ears: Arc::new(Ears::default()),
        }
    }

    fn sh(script: &str) -> Launch {
        Launch {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), script.into()],
            env: Vec::new(),
        }
    }

    fn start(run: &Run, launch: &Launch) -> Summoning {
        summon(
            launch,
            (80, 24),
            Arc::clone(&run.broker),
            &RunFolders {
                run_id: RUN.into(),
                requests: run.requests.clone(),
                answers: run.answers.clone(),
            },
            run.ears.clone(),
        )
        .unwrap()
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_terminal_speaks_listens_and_reports_its_end() {
        let run = run();
        let pawn = start(
            &run,
            &sh("printf 'ready\\n'; read line; printf 'got:%s\\n' \"$line\"; exit 3"),
        );
        run.ears
            .wait_for("ready", |ears| ears.said().contains("ready"));
        pawn.write(b"yes\n").unwrap();
        run.ears
            .wait_for("the end", |ears| lock(&ears.ended).is_some());
        assert!(run.ears.said().contains("got:yes"), "{}", run.ears.said());
        assert_eq!(*lock(&run.ears.ended), Some(Some(3)));
        let seqs = lock(&run.ears.seqs).clone();
        assert!(
            seqs.windows(2).all(|pair| pair[1] == pair[0] + 1),
            "in order, none dropped"
        );
        assert!(pawn.resize(120, 40).is_ok() || !pawn.is_here());
        pawn.depart();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_request_left_in_the_mailbox_is_answered_and_heard() {
        let run = run();
        let pawn = start(&run, &sh("sleep 30"));
        let id = "11111111-2222-4333-8444-555555555555";
        let body = serde_json::json!({ "id": id, "tool": "list_installed", "args": {} });
        std::fs::write(run.requests.join(format!("{id}.json")), body.to_string()).unwrap();
        let answer_path = run.answers.join(format!("{id}.json"));
        run.ears.wait_for("the answer", |_| answer_path.exists());
        let answer: Value = serde_json::from_slice(&std::fs::read(&answer_path).unwrap()).unwrap();
        assert_eq!(answer["outcome"], "done");
        assert_eq!(answer["data"]["play"]["waiting"], Value::Null);
        assert!(!run.requests.join(format!("{id}.json")).exists(), "taken");
        run.ears
            .wait_for("the hand", |ears| !lock(&ears.hands).is_empty());
        assert_eq!(run.errands.listed.load(Ordering::SeqCst), 1);
        let retired = pawn.depart();
        assert!(retired.persisted);
        run.ears
            .wait_for("the end", |ears| lock(&ears.ended).is_some());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_linked_request_is_answered_refused_without_being_read() {
        let run = run();
        let pawn = start(&run, &sh("sleep 30"));
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret"), b"canary").unwrap();
        let id = "22222222-2222-4333-8444-555555555555";
        std::os::unix::fs::symlink(
            outside.path().join("secret"),
            run.requests.join(format!("{id}.json")),
        )
        .unwrap();
        let answer_path = run.answers.join(format!("{id}.json"));
        run.ears.wait_for("the refusal", |_| answer_path.exists());
        let answer: Value = serde_json::from_slice(&std::fs::read(&answer_path).unwrap()).unwrap();
        assert_eq!(answer["outcome"], "refused");
        assert_eq!(
            std::fs::read(outside.path().join("secret")).unwrap(),
            b"canary"
        );
        pawn.depart();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn departing_ends_a_pawn_that_will_not_stop_and_retires_the_run() {
        let run = run();
        let pawn = start(&run, &sh("trap '' HUP TERM; while :; do sleep 1; done"));
        assert!(pawn.is_here());
        let began = Instant::now();
        let retired = pawn.depart();
        assert!(began.elapsed() < Duration::from_secs(8));
        assert!(!pawn.is_here());
        assert!(retired.ledger.ended_at.is_some());
        run.ears
            .wait_for("the end", |ears| lock(&ears.ended).is_some());
    }

    #[test]
    fn the_bench_launch_is_the_script_wsl_would_be_handed() {
        let plan = FencePlan {
            home: "/home/arisen".into(),
            user: "arisen".into(),
            workshop: "/mnt/c/Other/The Rift/mods (workshop)".into(),
            run_dir: format!("/mnt/c/Users/A/AppData/Local/nl.zmuuzn.riftstone/pawn/runs/{RUN}"),
            run_id: RUN.into(),
            inputs: Vec::new(),
        };
        let wsl = Launch::through_wsl("Ubuntu-24.04", &plan).unwrap();
        let bench = Launch::on_bench(&plan).unwrap();
        assert_eq!(wsl.program, "wsl.exe");
        assert_eq!(wsl.env, [("WSLENV".to_string(), String::new())]);
        assert_eq!(wsl.args.last(), bench.args.last(), "the same fence script");
        assert_eq!(bench.program, "/bin/sh");
        assert!(bench.args[1].starts_with("exec 'bwrap'"));
    }
}
