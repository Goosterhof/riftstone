//! The pawn's fence (RS-9, BUILD-SPEC § v0.7): pure functions only.
//!
//! Nothing here touches a disk, a process or a clock. The stone's summon and
//! its probe build the same [`FencePlan`], hand it to [`bwrap_argv`], and run
//! the result through [`wsl_args`]. So the fence the probe proves is the fence
//! the pawn lives in, word for word.
//!
//! The spike measured every piece on the investor's homestation (lab
//! `documents/design-systems/riftstone-pawn-workshop-2026-10-28/spike/`):
//! - reads by allowlist, so the saves, Steam, the game and the rest of `/mnt`
//!   are not in the namespace at all;
//! - `--remount-ro /`, so a write outside the workshop fails truthfully
//!   instead of landing in bwrap's throwaway root;
//! - [`fence_filter`], so WSL interop's socket (`AF_VSOCK`) and terminal
//!   injection (`TIOCSTI`) are refused for the whole process tree;
//! - `--clearenv` with [`ENV_ALLOWLIST`], so nothing the investor's
//!   environment carries crosses.
//!
//! Two validators guard the mounts. [`bwrap_argv`] refuses, on the WSL side,
//! any plan whose sources could reach `/init`, a drive root, `userdata`, a
//! `.claude` or the real home. [`check_mount_source`] refuses, on the Windows
//! side, a source whose final resolved path overlaps a protected root in
//! either direction. The resolving itself (`GetFinalPathNameByHandleW`) is
//! I/O and lives with the summon.

use sha2::{Digest, Sha256};

/// Where the stone's own read-only files appear inside the fence: the relay,
/// its MCP config, the settings, the persona and the filter.
pub const RELAY_MOUNT: &str = "/stone/relay";
/// Tamsin's own Claude config dir inside the fence (`CLAUDE_CONFIG_DIR`).
pub const CLAUDE_MOUNT: &str = "/stone/claude";
/// The live run's request folder (read-write) inside the fence.
pub const REQUESTS_MOUNT: &str = "/stone/mailbox/requests";
/// The live run's answer folder (read-only) inside the fence.
pub const ANSWERS_MOUNT: &str = "/stone/mailbox/answers";
/// Read-only inputs appear as `/inputs/<n>`.
pub const INPUTS_MOUNT: &str = "/inputs";

/// The tools Tamsin has, exactly (RS-9.5). The stone's own come through the
/// relay as MCP tools.
pub const PAWN_TOOLS: [&str; 6] = ["Bash", "Read", "Write", "Edit", "Glob", "Grep"];

/// The environment inside the fence is exactly these names (RS-9.5); the
/// values of `HOME` and `USER` come from the plan.
pub const ENV_ALLOWLIST: [&str; 9] = [
    "PATH",
    "HOME",
    "USER",
    "TERM",
    "COLORTERM",
    "LANG",
    "TMPDIR",
    "CLAUDE_CONFIG_DIR",
    "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC",
];

/// The descriptor bwrap reads the filter from.
pub const FILTER_FD: &str = "9";

// ---------------------------------------------------------------------------
// The filter
// ---------------------------------------------------------------------------

const BPF_LD_W_ABS: u16 = 0x20;
const BPF_JEQ_K: u16 = 0x15;
const BPF_JGE_K: u16 = 0x35;
const BPF_RET_K: u16 = 0x06;

const SECCOMP_RET_ALLOW: u32 = 0x7FFF_0000;
/// `SECCOMP_RET_ERRNO | EPERM`.
const SECCOMP_RET_EPERM: u32 = 0x0005_0001;
const AUDIT_ARCH_X86_64: u32 = 0xC000_003E;
const X32_SYSCALL_BIT: u32 = 0x4000_0000;
const NR_SOCKET: u32 = 41;
const NR_IOCTL: u32 = 16;
const AF_VSOCK: u32 = 40;
const TIOCSTI: u32 = 0x5412;

/// `struct seccomp_data` offsets: `nr`, `arch`, the low half of `args[0]` and
/// of `args[1]` (x86_64 is little-endian; `socket`'s family and `ioctl`'s
/// request are both 32-bit in the kernel's own signature).
const DATA_NR: u32 = 0;
const DATA_ARCH: u32 = 4;
const DATA_ARG0_LO: u32 = 16;
const DATA_ARG1_LO: u32 = 24;

/// One classic-BPF instruction: `(code, jt, jf, k)`.
pub type Instruction = (u16, u8, u8, u32);

/// The fence's 15 instructions (measured as `fence.bpf` in the spike): refuse
/// with `EPERM` any syscall that is not native x86_64, any x32 syscall,
/// `socket(AF_VSOCK, …)` and `ioctl(_, TIOCSTI, …)`; allow everything else.
pub fn filter_program() -> [Instruction; 15] {
    [
        (BPF_LD_W_ABS, 0, 0, DATA_ARCH),
        (BPF_JEQ_K, 1, 0, AUDIT_ARCH_X86_64),
        (BPF_RET_K, 0, 0, SECCOMP_RET_EPERM),
        (BPF_LD_W_ABS, 0, 0, DATA_NR),
        (BPF_JGE_K, 0, 1, X32_SYSCALL_BIT),
        (BPF_RET_K, 0, 0, SECCOMP_RET_EPERM),
        (BPF_JEQ_K, 0, 3, NR_SOCKET), // not socket → the ioctl test
        (BPF_LD_W_ABS, 0, 0, DATA_ARG0_LO),
        (BPF_JEQ_K, 0, 4, AF_VSOCK), // another family → allow
        (BPF_RET_K, 0, 0, SECCOMP_RET_EPERM),
        (BPF_JEQ_K, 0, 2, NR_IOCTL), // neither → allow
        (BPF_LD_W_ABS, 0, 0, DATA_ARG1_LO),
        (BPF_JEQ_K, 1, 0, TIOCSTI),
        (BPF_RET_K, 0, 0, SECCOMP_RET_ALLOW),
        (BPF_RET_K, 0, 0, SECCOMP_RET_EPERM),
    ]
}

/// [`filter_program`] as the bytes bwrap's `--seccomp` reads: each
/// instruction `struct sock_filter`, little-endian, 8 bytes.
pub fn fence_filter() -> Vec<u8> {
    let mut bytes = Vec::with_capacity(15 * 8);
    for (code, jt, jf, k) in filter_program() {
        bytes.extend_from_slice(&code.to_le_bytes());
        bytes.push(jt);
        bytes.push(jf);
        bytes.extend_from_slice(&k.to_le_bytes());
    }
    bytes
}

// ---------------------------------------------------------------------------
// The plan and its argv
// ---------------------------------------------------------------------------

/// Everything one fence mounts, as WSL paths. The stone's own paths under the
/// distro user's home ([`FencePlan::relay`], [`FencePlan::claude_config`]) are
/// derived from `home`, never chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FencePlan {
    /// The distro user's real home, e.g. `/home/arisen`. Inside the fence it
    /// is a fresh tmpfs with only Claude Code's binary tree mounted in it.
    pub home: String,
    pub user: String,
    /// `/mnt/<drive>/…/<mods> (workshop)`, read-write, at its own path.
    pub workshop: String,
    /// `/mnt/<drive>/…/pawn/runs/<run_id>`: its `requests` and `answers` are
    /// mounted, never the run folder itself.
    pub run_dir: String,
    pub run_id: String,
    /// Read-only inputs, each already validated on the Windows side
    /// ([`check_mount_source`]), mounted at `/inputs/<n>` in this order.
    pub inputs: Vec<String>,
}

impl FencePlan {
    /// `~/.local/share/riftstone/pawn`, the stone's WSL-side folder.
    pub fn pawn_dir(&self) -> String {
        format!("{}/.local/share/riftstone/pawn", self.home)
    }

    pub fn relay(&self) -> String {
        format!("{}/relay", self.pawn_dir())
    }

    pub fn claude_config(&self) -> String {
        format!("{}/claude", self.pawn_dir())
    }

    /// The filter's file, written by the stone into the relay folder.
    pub fn filter_file(&self) -> String {
        format!("{}/fence.bpf", self.relay())
    }

    pub fn claude_bin(&self) -> String {
        format!("{}/.local/bin/claude", self.home)
    }
}

/// What runs inside the fence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FenceCommand {
    /// Tamsin: Claude Code, restricted, with the relay as its only MCP server.
    Pawn,
    /// One probe step: `/bin/sh -c <script>` in the identical fence.
    Probe { script: String },
}

/// Why a plan cannot be fenced. The sentence never carries a path under
/// `userdata`, the saves or the investor's home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FenceRefusal(pub String);

impl std::fmt::Display for FenceRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn refuse(what: &str, why: &str) -> FenceRefusal {
    FenceRefusal(format!("The fence will not mount {what}: {why}."))
}

/// An absolute WSL path with no empty, `.` or `..` component, no trailing
/// slash and no control character, as components.
fn components(path: &str) -> Option<Vec<&str>> {
    let rest = path.strip_prefix('/')?;
    if rest.is_empty() || path.chars().any(char::is_control) {
        return None;
    }
    let parts: Vec<&str> = rest.split('/').collect();
    if parts
        .iter()
        .any(|p| p.is_empty() || *p == "." || *p == "..")
    {
        return None;
    }
    Some(parts)
}

/// Is `inner` `outer` itself or inside it? drvfs is case-insensitive, so the
/// `/mnt` paths compare without case.
fn within(inner: &[&str], outer: &[&str]) -> bool {
    inner.len() >= outer.len()
        && inner
            .iter()
            .zip(outer)
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
}

fn overlap(a: &[&str], b: &[&str]) -> bool {
    within(a, b) || within(b, a)
}

/// A Windows drive's mount: `/mnt/<one letter>`.
fn is_drive(parts: &[&str]) -> bool {
    parts.len() >= 2
        && parts[0] == "mnt"
        && parts[1].len() == 1
        && parts[1].as_bytes()[0].is_ascii_alphabetic()
}

/// The components no mount source may carry, at any depth, in any case.
const FORBIDDEN_COMPONENTS: [&str; 3] = ["userdata", ".claude", ".claude.json"];

fn forbidden_component(parts: &[&str]) -> bool {
    parts.iter().any(|p| {
        FORBIDDEN_COMPONENTS
            .iter()
            .any(|bad| p.eq_ignore_ascii_case(bad))
    })
}

fn is_uuid(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => *b == b'-',
            _ => b.is_ascii_digit() || (b'a'..=b'f').contains(b),
        })
}

/// A Windows source, as WSL sees it: under a drive, at least two folders
/// deep (never a drive root or a top folder like `/mnt/c/Users`).
fn windows_source<'a>(what: &str, path: &'a str) -> Result<Vec<&'a str>, FenceRefusal> {
    let parts = components(path).ok_or_else(|| refuse(what, "its path is not plain"))?;
    if !is_drive(&parts) || parts.len() < 4 {
        return Err(refuse(what, "it is not a folder on a Windows drive"));
    }
    if forbidden_component(&parts) {
        return Err(refuse(what, "it reaches Steam's userdata or a .claude"));
    }
    Ok(parts)
}

/// Check every source the plan names, before one word of argv is written.
fn validate(plan: &FencePlan) -> Result<(), FenceRefusal> {
    let home = components(&plan.home).ok_or_else(|| refuse("the home", "it is not plain"))?;
    if plan.home.starts_with("/mnt") || forbidden_component(&home) {
        return Err(refuse("the home", "it is not the distro user's home"));
    }
    if plan.user.is_empty()
        || !plan
            .user
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || b == b'.')
    {
        return Err(refuse("the user", "its name is not plain"));
    }
    let workshop = windows_source("the workshop", &plan.workshop)?;
    if !workshop
        .last()
        .is_some_and(|last| last.ends_with(" (workshop)"))
    {
        return Err(refuse(
            "the workshop",
            "it is not the mods folder's workshop",
        ));
    }
    if !is_uuid(&plan.run_id) {
        return Err(refuse("the mailbox", "the run has no proper id"));
    }
    let run = windows_source("the mailbox", &plan.run_dir)?;
    let n = run.len();
    if run[n - 1] != plan.run_id || !run[n - 2].eq_ignore_ascii_case("runs") {
        return Err(refuse("the mailbox", "it is not this run's folder"));
    }
    if overlap(&workshop, &run) {
        return Err(refuse("the mailbox", "it overlaps the workshop"));
    }
    for (i, input) in plan.inputs.iter().enumerate() {
        let what = format!("input {i}");
        let parts = windows_source(&what, input)?;
        if overlap(&parts, &workshop) {
            return Err(refuse(&what, "it overlaps the workshop"));
        }
        if overlap(&parts, &run[..n - 2]) {
            return Err(refuse(&what, "it overlaps the stone's mailbox"));
        }
    }
    Ok(())
}

fn push(argv: &mut Vec<String>, words: &[&str]) {
    argv.extend(words.iter().map(|w| (*w).to_string()));
}

/// The fence's argv, `bwrap` first: the allowlist, the cleared environment,
/// the read-only root and the filter, then `--` and the command. A plan that
/// could mount anything outside the allowlist is refused whole.
pub fn bwrap_argv(plan: &FencePlan, command: &FenceCommand) -> Result<Vec<String>, FenceRefusal> {
    validate(plan)?;
    let home = plan.home.as_str();
    let relay = plan.relay();
    let claude_config = plan.claude_config();
    let requests = format!("{}/requests", plan.run_dir);
    let answers = format!("{}/answers", plan.run_dir);
    let claude_share = format!("{home}/.local/share/claude");
    let local_bin = format!("{home}/.local/bin");

    let mut argv = vec!["bwrap".to_string()];
    push(
        &mut argv,
        &[
            "--die-with-parent",
            "--unshare-pid",
            "--unshare-ipc",
            "--clearenv",
        ],
    );
    for (name, value) in [
        ("PATH", "/usr/local/bin:/usr/bin:/bin"),
        ("HOME", home),
        ("USER", plan.user.as_str()),
        ("TERM", "xterm-256color"),
        ("COLORTERM", "truecolor"),
        ("LANG", "C.UTF-8"),
        ("TMPDIR", "/tmp"),
        ("CLAUDE_CONFIG_DIR", CLAUDE_MOUNT),
        ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1"),
    ] {
        push(&mut argv, &["--setenv", name, value]);
    }
    push(
        &mut argv,
        &[
            "--ro-bind",
            "/usr",
            "/usr",
            "--symlink",
            "usr/bin",
            "/bin",
            "--symlink",
            "usr/lib",
            "/lib",
            "--symlink",
            "usr/lib64",
            "/lib64",
            "--symlink",
            "usr/sbin",
            "/sbin",
            "--ro-bind",
            "/etc",
            "/etc",
            "--ro-bind",
            "/mnt/wsl/resolv.conf",
            "/mnt/wsl/resolv.conf",
            "--dev",
            "/dev",
            "--proc",
            "/proc",
            "--tmpfs",
            "/tmp",
            "--tmpfs",
            home,
            "--ro-bind",
            &claude_share,
            &claude_share,
            "--ro-bind",
            &local_bin,
            &local_bin,
            "--ro-bind",
            &relay,
            RELAY_MOUNT,
            "--bind",
            &claude_config,
            CLAUDE_MOUNT,
            "--bind",
            &requests,
            REQUESTS_MOUNT,
            "--ro-bind",
            &answers,
            ANSWERS_MOUNT,
        ],
    );
    for (i, input) in plan.inputs.iter().enumerate() {
        push(
            &mut argv,
            &["--ro-bind", input, &format!("{INPUTS_MOUNT}/{i}")],
        );
    }
    push(
        &mut argv,
        &[
            "--bind",
            &plan.workshop,
            &plan.workshop,
            "--remount-ro",
            "/",
            "--chdir",
            &plan.workshop,
            "--seccomp",
            FILTER_FD,
            "--",
        ],
    );
    match command {
        FenceCommand::Pawn => {
            argv.push(plan.claude_bin());
            argv.push("--restricted".into());
            argv.push("--tools".into());
            argv.extend(PAWN_TOOLS.iter().map(|t| (*t).to_string()));
            push(
                &mut argv,
                &[
                    "--settings",
                    &format!("{RELAY_MOUNT}/settings.json"),
                    "--strict-mcp-config",
                    "--mcp-config",
                    &format!("{RELAY_MOUNT}/mcp.json"),
                    "--append-system-prompt-file",
                    &format!("{RELAY_MOUNT}/persona.md"),
                    "--permission-mode",
                    "default",
                ],
            );
            // `--restricted` keeps the file tools in the working dirs: each
            // input is named so Tamsin can read it (the fence keeps it
            // read-only).
            if !plan.inputs.is_empty() {
                argv.push("--add-dir".into());
                argv.extend((0..plan.inputs.len()).map(|i| format!("{INPUTS_MOUNT}/{i}")));
            }
        }
        FenceCommand::Probe { script } => {
            push(&mut argv, &["/bin/sh", "-c", script]);
        }
    }
    Ok(argv)
}

// ---------------------------------------------------------------------------
// wsl.exe
// ---------------------------------------------------------------------------

/// One word for `sh`, single-quoted. The script never carries a double quote:
/// `wsl.exe` does not read `\"` back (the lab's Tauri record).
pub fn sh_quote(word: &str) -> String {
    format!("'{}'", word.replace('\'', r"'\''"))
}

/// The arguments for `wsl.exe` (RS-9.1): the distro, `/` as the working
/// folder, and `/bin/sh -c 'exec bwrap … 9<filter'`. The stone starts it with
/// `WSLENV` emptied. A distro name that is not plain is refused.
pub fn wsl_args(
    distro: &str,
    plan: &FencePlan,
    argv: &[String],
) -> Result<Vec<String>, FenceRefusal> {
    if distro.is_empty()
        || !distro
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
    {
        return Err(refuse("the distro", "its name is not plain"));
    }
    let words: Vec<String> = argv.iter().map(|w| sh_quote(w)).collect();
    let script = format!(
        "exec {} {FILTER_FD}<{}",
        words.join(" "),
        sh_quote(&plan.filter_file())
    );
    Ok(vec![
        "-d".into(),
        distro.into(),
        "--cd".into(),
        "/".into(),
        "-e".into(),
        "/bin/sh".into(),
        "-c".into(),
        script,
    ])
}

// ---------------------------------------------------------------------------
// The stone's files in the relay folder
// ---------------------------------------------------------------------------

/// The settings Tamsin's Claude Code loads (RS-9.4, 9.6): its own sandbox on
/// and fail-closed inside the fence, approvals on, no bypass, no hooks.
pub fn pawn_settings() -> Vec<u8> {
    let settings = serde_json::json!({
        "sandbox": {
            "enabled": true,
            "failIfUnavailable": true,
            "allowUnsandboxedCommands": false,
            "autoAllowBashIfSandboxed": true
        },
        "permissions": {
            "defaultMode": "default",
            "disableBypassPermissionsMode": "disable"
        },
        "disableAllHooks": true
    });
    let mut bytes = serde_json::to_vec_pretty(&settings).unwrap_or_default();
    bytes.push(b'\n');
    bytes
}

/// The relay as Claude Code's only MCP server.
pub fn mcp_config() -> Vec<u8> {
    let config = serde_json::json!({
        "mcpServers": {
            "riftstone": {
                "command": "/usr/bin/python3",
                "args": ["-I", format!("{RELAY_MOUNT}/relay.py")]
            }
        }
    });
    let mut bytes = serde_json::to_vec_pretty(&config).unwrap_or_default();
    bytes.push(b'\n');
    bytes
}

// ---------------------------------------------------------------------------
// The ground
// ---------------------------------------------------------------------------

/// What a probe record holds for (#00015 R20.14). A record proved on another
/// ground is `UNPROBED`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ground {
    pub distro: String,
    pub claude_version: String,
    pub kernel: String,
    /// SHA-256 over the pawn's argv (the run's id normalised), the settings,
    /// the MCP config, the filter and the relay.
    pub digest: String,
}

/// The facts of the host the stone reads before a summon or a probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Host {
    pub distro: String,
    pub claude_version: String,
    pub kernel: String,
}

/// Name the ground: the pawn's argv for this plan, with the run's id written
/// as `<run>` (every run has its own), and the bytes of every file the stone
/// writes into the relay folder.
pub fn ground(plan: &FencePlan, host: &Host, relay: &[u8]) -> Result<Ground, FenceRefusal> {
    let argv = bwrap_argv(plan, &FenceCommand::Pawn)?;
    let mut hash = Sha256::new();
    let mut part = |bytes: &[u8]| {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    };
    for word in &argv {
        part(word.replace(&plan.run_id, "<run>").as_bytes());
    }
    part(b"--");
    part(&pawn_settings());
    part(&mcp_config());
    part(&fence_filter());
    part(relay);
    let digest = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    Ok(Ground {
        distro: host.distro.clone(),
        claude_version: host.claude_version.clone(),
        kernel: host.kernel.clone(),
        digest,
    })
}

// ---------------------------------------------------------------------------
// Windows sources: the protected roots
// ---------------------------------------------------------------------------

/// A folder no mount may overlap, by its final resolved path, and the name a
/// refusal gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectedRoot {
    pub name: String,
    pub path: String,
}

/// A Windows path, normalised for comparison: the `\\?\` prefix dropped,
/// `/` read as `\`, the drive letter upper-case, no trailing separator. A
/// path that is not on a drive, or carries `.`, `..` or an empty component,
/// is `None`.
pub fn normalize_windows(path: &str) -> Option<Vec<String>> {
    let path = path.replace('/', "\\");
    let path = path
        .strip_prefix(r"\\?\")
        .or_else(|| path.strip_prefix(r"\??\"))
        .unwrap_or(&path);
    let bytes = path.as_bytes();
    if bytes.len() < 2 || !bytes[0].is_ascii_alphabetic() || bytes[1] != b':' {
        return None;
    }
    if path.chars().any(char::is_control) {
        return None;
    }
    let mut parts = vec![format!("{}:", (bytes[0] as char).to_ascii_uppercase())];
    let rest = path[2..].trim_start_matches('\\').trim_end_matches('\\');
    if !rest.is_empty() {
        for part in rest.split('\\') {
            if part.is_empty() || part == "." || part == ".." {
                return None;
            }
            parts.push(part.to_string());
        }
    }
    Some(parts)
}

fn windows_within(inner: &[String], outer: &[String]) -> bool {
    inner.len() >= outer.len()
        && inner
            .iter()
            .zip(outer)
            .all(|(a, b)| a.to_lowercase() == b.to_lowercase())
}

/// The WSL path of a Windows one: `C:\Other\mods` → `/mnt/c/Other/mods`.
pub fn windows_to_wsl(path: &str) -> Option<String> {
    let parts = normalize_windows(path)?;
    let drive = parts[0][..1].to_ascii_lowercase();
    let mut out = format!("/mnt/{drive}");
    for part in &parts[1..] {
        out.push('/');
        out.push_str(part);
    }
    Some(out)
}

/// The refusal a mount source earns (WINDOW-CONTRACT § v0.7, `inputRefused`):
/// it names the root, never the path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MountRefusal {
    /// Not a plain path on a drive.
    NotPlain,
    /// Equal to, inside, or containing the named protected root.
    Overlaps(String),
    /// A trusted mount that is not exactly the stone-made folder it names.
    NotTheStones(String),
}

impl std::fmt::Display for MountRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotPlain => f.write_str("That folder's path is not a plain folder on a drive."),
            Self::Overlaps(root) => write!(
                f,
                "That folder overlaps {root}, which Tamsin may never see. Choose a folder outside it."
            ),
            Self::NotTheStones(what) => write!(
                f,
                "{what} is not the folder the stone made for it, so the fence will not mount it."
            ),
        }
    }
}

/// What a mount source is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MountKind<'a> {
    /// A read-only input the investor named in Settings.
    Input,
    /// One of the declared trusted mounts (the workshop, the live run's two
    /// mailbox folders, Tamsin's config dir): it must resolve to exactly
    /// `expected`, the stone-made folder, and is then excepted from the bans.
    Trusted { name: &'a str, expected: &'a str },
}

/// Validate one mount source by its **final resolved path** (RS-9.2). An
/// input is refused when it overlaps any protected root in either direction:
/// equal, inside it, or containing it. A trusted mount must be exactly its
/// own folder.
pub fn check_mount_source(
    final_path: &str,
    kind: &MountKind<'_>,
    protected: &[ProtectedRoot],
) -> Result<(), MountRefusal> {
    let source = normalize_windows(final_path).ok_or(MountRefusal::NotPlain)?;
    match kind {
        MountKind::Trusted { name, expected } => {
            let expected = normalize_windows(expected).ok_or(MountRefusal::NotPlain)?;
            if source.len() == expected.len() && windows_within(&source, &expected) {
                Ok(())
            } else {
                Err(MountRefusal::NotTheStones((*name).to_string()))
            }
        }
        MountKind::Input => {
            if source.len() < 2 {
                return Err(MountRefusal::Overlaps("a whole drive".into()));
            }
            for root in protected {
                // A root that cannot be read as a path protects everything:
                // refuse rather than guess.
                let Some(root_parts) = normalize_windows(&root.path) else {
                    return Err(MountRefusal::Overlaps(root.name.clone()));
                };
                if windows_within(&source, &root_parts) || windows_within(&root_parts, &source) {
                    return Err(MountRefusal::Overlaps(root.name.clone()));
                }
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUN: &str = "0b5e2c6e-4a8d-4d3e-9f4b-2a1c7d9e8f00";

    fn plan() -> FencePlan {
        FencePlan {
            home: "/home/arisen".into(),
            user: "arisen".into(),
            workshop: "/mnt/c/Other/The Rift/mods (workshop)".into(),
            run_dir: format!(
                "/mnt/c/Users/Arisen/AppData/Local/nl.zmuuzn.riftstone/pawn/runs/{RUN}"
            ),
            run_id: RUN.into(),
            inputs: vec!["/mnt/d/Extracts/dd2-vanilla".into()],
        }
    }

    /// Every `(flag, source, destination)` the argv mounts or links.
    fn mounts(argv: &[String]) -> Vec<(String, String, String)> {
        let end = argv.iter().position(|w| w == "--").unwrap();
        let mut out = Vec::new();
        let mut i = 0;
        while i < end {
            match argv[i].as_str() {
                "--bind" | "--ro-bind" | "--symlink" | "--bind-try" | "--ro-bind-try"
                | "--dev-bind" | "--dev-bind-try" => {
                    out.push((argv[i].clone(), argv[i + 1].clone(), argv[i + 2].clone()));
                    i += 3;
                }
                "--setenv" => i += 3,
                "--tmpfs" | "--dev" | "--proc" | "--remount-ro" | "--chdir" | "--seccomp" => {
                    out.push((argv[i].clone(), argv[i + 1].clone(), String::new()));
                    i += 2;
                }
                _ => i += 1,
            }
        }
        out
    }

    #[test]
    fn the_filter_is_the_fifteen_instructions_the_spike_measured() {
        // `fence.bpf` from the spike (runs F–H and the TIOCSTI run), byte for byte.
        let measured: [u8; 120] = [
            0x20, 0, 0, 0, 0x04, 0, 0, 0, 0x15, 0, 0x01, 0, 0x3e, 0, 0, 0xc0, //
            0x06, 0, 0, 0, 0x01, 0, 0x05, 0, 0x20, 0, 0, 0, 0, 0, 0, 0, //
            0x35, 0, 0, 0x01, 0, 0, 0, 0x40, 0x06, 0, 0, 0, 0x01, 0, 0x05, 0, //
            0x15, 0, 0, 0x03, 0x29, 0, 0, 0, 0x20, 0, 0, 0, 0x10, 0, 0, 0, //
            0x15, 0, 0, 0x04, 0x28, 0, 0, 0, 0x06, 0, 0, 0, 0x01, 0, 0x05, 0, //
            0x15, 0, 0, 0x02, 0x10, 0, 0, 0, 0x20, 0, 0, 0, 0x18, 0, 0, 0, //
            0x15, 0, 0x01, 0, 0x12, 0x54, 0, 0, 0x06, 0, 0, 0, 0, 0, 0xff, 0x7f, //
            0x06, 0, 0, 0, 0x01, 0, 0x05, 0,
        ];
        assert_eq!(fence_filter(), measured.to_vec());
    }

    /// Run the program the way the kernel does, on one invented syscall.
    fn run_filter(arch: u32, nr: u32, arg0: u32, arg1: u32) -> u32 {
        let program = filter_program();
        let mut acc = 0u32;
        let mut pc = 0usize;
        loop {
            let (code, jt, jf, k) = program[pc];
            match code {
                BPF_LD_W_ABS => {
                    acc = match k {
                        DATA_NR => nr,
                        DATA_ARCH => arch,
                        DATA_ARG0_LO => arg0,
                        DATA_ARG1_LO => arg1,
                        other => panic!("an unexpected load at {other}"),
                    };
                    pc += 1;
                }
                BPF_JEQ_K => pc += 1 + usize::from(if acc == k { jt } else { jf }),
                BPF_JGE_K => pc += 1 + usize::from(if acc >= k { jt } else { jf }),
                BPF_RET_K => return k,
                other => panic!("an unexpected instruction {other:#x}"),
            }
        }
    }

    #[test]
    fn the_filter_refuses_exactly_vsock_tiocsti_and_foreign_syscalls() {
        let x86 = AUDIT_ARCH_X86_64;
        // The refusals.
        assert_eq!(run_filter(x86, NR_SOCKET, AF_VSOCK, 1), SECCOMP_RET_EPERM);
        assert_eq!(run_filter(x86, NR_IOCTL, 0, TIOCSTI), SECCOMP_RET_EPERM);
        assert_eq!(run_filter(0x4000_0003, 1, 0, 0), SECCOMP_RET_EPERM, "i386");
        assert_eq!(
            run_filter(x86, X32_SYSCALL_BIT | 1, 0, 0),
            SECCOMP_RET_EPERM,
            "x32"
        );
        // What the pawn needs: IPv4 and IPv6 and Unix sockets, the window size,
        // reads and writes.
        for family in [1, 2, 10] {
            assert_eq!(run_filter(x86, NR_SOCKET, family, 1), SECCOMP_RET_ALLOW);
        }
        assert_eq!(
            run_filter(x86, NR_IOCTL, 0, 0x5413),
            SECCOMP_RET_ALLOW,
            "TIOCGWINSZ"
        );
        assert_eq!(run_filter(x86, 0, 0, 0), SECCOMP_RET_ALLOW, "read");
        assert_eq!(run_filter(x86, 1, 0, 0), SECCOMP_RET_ALLOW, "write");
        // A family of 40 on another syscall is not a socket.
        assert_eq!(run_filter(x86, 2, AF_VSOCK, 0), SECCOMP_RET_ALLOW);
    }

    #[test]
    fn the_argv_mounts_exactly_the_allowlist() {
        let p = plan();
        let argv = bwrap_argv(&p, &FenceCommand::Pawn).unwrap();
        let run = &p.run_dir;
        let s = |x: &str| x.to_string();
        let expected = vec![
            (s("--ro-bind"), s("/usr"), s("/usr")),
            (s("--symlink"), s("usr/bin"), s("/bin")),
            (s("--symlink"), s("usr/lib"), s("/lib")),
            (s("--symlink"), s("usr/lib64"), s("/lib64")),
            (s("--symlink"), s("usr/sbin"), s("/sbin")),
            (s("--ro-bind"), s("/etc"), s("/etc")),
            (
                s("--ro-bind"),
                s("/mnt/wsl/resolv.conf"),
                s("/mnt/wsl/resolv.conf"),
            ),
            (s("--dev"), s("/dev"), s("")),
            (s("--proc"), s("/proc"), s("")),
            (s("--tmpfs"), s("/tmp"), s("")),
            (s("--tmpfs"), s("/home/arisen"), s("")),
            (
                s("--ro-bind"),
                s("/home/arisen/.local/share/claude"),
                s("/home/arisen/.local/share/claude"),
            ),
            (
                s("--ro-bind"),
                s("/home/arisen/.local/bin"),
                s("/home/arisen/.local/bin"),
            ),
            (
                s("--ro-bind"),
                s("/home/arisen/.local/share/riftstone/pawn/relay"),
                s("/stone/relay"),
            ),
            (
                s("--bind"),
                s("/home/arisen/.local/share/riftstone/pawn/claude"),
                s("/stone/claude"),
            ),
            (
                s("--bind"),
                format!("{run}/requests"),
                s("/stone/mailbox/requests"),
            ),
            (
                s("--ro-bind"),
                format!("{run}/answers"),
                s("/stone/mailbox/answers"),
            ),
            (
                s("--ro-bind"),
                s("/mnt/d/Extracts/dd2-vanilla"),
                s("/inputs/0"),
            ),
            (s("--bind"), p.workshop.clone(), p.workshop.clone()),
            (s("--remount-ro"), s("/"), s("")),
            (s("--chdir"), p.workshop.clone(), s("")),
            (s("--seccomp"), s("9"), s("")),
        ];
        assert_eq!(mounts(&argv), expected);
        assert_eq!(argv[0], "bwrap");
        for flag in [
            "--die-with-parent",
            "--unshare-pid",
            "--unshare-ipc",
            "--clearenv",
        ] {
            assert!(
                argv[..10].contains(&flag.to_string()),
                "{flag} leads the argv"
            );
        }
        assert!(
            !argv.iter().any(|w| w == "--new-session"),
            "the TUI keeps its terminal"
        );
        assert!(
            !argv.iter().any(|w| w == "--unshare-net"),
            "the pawn reaches Anthropic"
        );
    }

    #[test]
    fn the_environment_is_exactly_the_allowlist_after_clearenv() {
        let argv = bwrap_argv(&plan(), &FenceCommand::Pawn).unwrap();
        let clear = argv.iter().position(|w| w == "--clearenv").unwrap();
        let mut names = Vec::new();
        for (i, word) in argv.iter().enumerate() {
            if word == "--setenv" {
                assert!(i > clear, "every variable is set after the clear");
                names.push(argv[i + 1].as_str());
            }
            assert_ne!(word, "--unsetenv");
        }
        assert_eq!(names, ENV_ALLOWLIST.to_vec());
        let value = |name: &str| {
            let i = argv.iter().position(|w| w == name).unwrap();
            argv[i + 1].clone()
        };
        assert_eq!(value("CLAUDE_CONFIG_DIR"), "/stone/claude");
        assert_eq!(value("HOME"), "/home/arisen");
        assert_eq!(value("PATH"), "/usr/local/bin:/usr/bin:/bin");
    }

    #[test]
    fn the_pawn_runs_restricted_with_its_six_tools_and_the_relay_only() {
        let argv = bwrap_argv(&plan(), &FenceCommand::Pawn).unwrap();
        let tail: Vec<&str> = argv
            .iter()
            .skip_while(|w| *w != "--")
            .skip(1)
            .map(String::as_str)
            .collect();
        assert_eq!(
            tail,
            [
                "/home/arisen/.local/bin/claude",
                "--restricted",
                "--tools",
                "Bash",
                "Read",
                "Write",
                "Edit",
                "Glob",
                "Grep",
                "--settings",
                "/stone/relay/settings.json",
                "--strict-mcp-config",
                "--mcp-config",
                "/stone/relay/mcp.json",
                "--append-system-prompt-file",
                "/stone/relay/persona.md",
                "--permission-mode",
                "default",
                "--add-dir",
                "/inputs/0",
            ]
        );
        for banned in [
            "bypassPermissions",
            "dontAsk",
            "--dangerously-skip-permissions",
            "acceptEdits",
        ] {
            assert!(!argv.iter().any(|w| w == banned), "{banned}");
        }
    }

    #[test]
    fn the_probe_runs_in_the_identical_fence() {
        let p = plan();
        let pawn = bwrap_argv(&p, &FenceCommand::Pawn).unwrap();
        let probe = bwrap_argv(
            &p,
            &FenceCommand::Probe {
                script: "echo x > probe/control.txt".into(),
            },
        )
        .unwrap();
        let fence =
            |argv: &[String]| argv[..=argv.iter().position(|w| w == "--").unwrap()].to_vec();
        assert_eq!(fence(&pawn), fence(&probe));
        assert_eq!(
            probe[probe.len() - 3..],
            ["/bin/sh", "-c", "echo x > probe/control.txt"]
        );
    }

    /// One hostile change to a good plan.
    type Change = Box<dyn Fn(&mut FencePlan)>;

    #[test]
    fn hostile_plans_are_refused_whole() {
        let cases: Vec<(&str, Change)> = vec![
            (
                "a drive root input",
                Box::new(|p| p.inputs = vec!["/mnt/c".into()]),
            ),
            (
                "a top folder input",
                Box::new(|p| p.inputs = vec!["/mnt/c/Users".into()]),
            ),
            ("the init", Box::new(|p| p.inputs = vec!["/init".into()])),
            ("the root", Box::new(|p| p.inputs = vec!["/".into()])),
            (
                "userdata",
                Box::new(|p| p.inputs = vec!["/mnt/c/Program Files (x86)/Steam/userdata/1".into()]),
            ),
            (
                "UserData in another case",
                Box::new(|p| p.inputs = vec!["/mnt/c/Games/Steam/UserData".into()]),
            ),
            (
                "a .claude",
                Box::new(|p| p.inputs = vec!["/mnt/c/Users/Arisen/.claude".into()]),
            ),
            (
                "a climb",
                Box::new(|p| p.inputs = vec!["/mnt/c/Other/../Users/x".into()]),
            ),
            (
                "a trailing slash",
                Box::new(|p| p.inputs = vec!["/mnt/c/Other/x/".into()]),
            ),
            (
                "a double slash",
                Box::new(|p| p.inputs = vec!["/mnt/c//Other/x".into()]),
            ),
            (
                "a relative path",
                Box::new(|p| p.inputs = vec!["mnt/c/Other/x".into()]),
            ),
            (
                "a WSL path input",
                Box::new(|p| p.inputs = vec!["/home/arisen/.ssh".into()]),
            ),
            (
                "an input inside the workshop",
                Box::new(|p| p.inputs = vec!["/mnt/c/Other/The Rift/MODS (workshop)/dist".into()]),
            ),
            (
                "an input holding the workshop",
                Box::new(|p| p.inputs = vec!["/mnt/c/Other/The Rift".into()]),
            ),
            (
                "an input holding the mailbox",
                Box::new(|p| {
                    p.inputs = vec!["/mnt/c/Users/Arisen/AppData/Local/nl.zmuuzn.riftstone".into()]
                }),
            ),
            (
                "a workshop that is not one",
                Box::new(|p| p.workshop = "/mnt/c/Other/mods".into()),
            ),
            (
                "a run with another id",
                Box::new(|p| p.run_id = RUN.replace('0', "1")),
            ),
            (
                "a run id not a uuid",
                Box::new(|p| p.run_id = "../../x".into()),
            ),
            (
                "a home on a drive",
                Box::new(|p| p.home = "/mnt/c/Users/Arisen".into()),
            ),
            (
                "a home that is a .claude",
                Box::new(|p| p.home = "/home/a/.claude".into()),
            ),
            ("a user with a space", Box::new(|p| p.user = "a b".into())),
        ];
        for (what, change) in cases {
            let mut p = plan();
            change(&mut p);
            assert!(
                bwrap_argv(&p, &FenceCommand::Pawn).is_err(),
                "{what} must be refused"
            );
        }
    }

    /// An independent oracle: may this source ever appear in a fence?
    fn source_is_safe(source: &str, home: &str) -> bool {
        let lower = source.to_ascii_lowercase();
        let segs: Vec<&str> = lower.split('/').collect();
        if source == "/" || source == "/init" || lower == "/mnt" || source.contains("..") {
            return false;
        }
        if segs
            .iter()
            .any(|s| *s == "userdata" || *s == ".claude" || *s == ".claude.json")
        {
            return false;
        }
        // A drive root or a top folder of a drive.
        if lower.starts_with("/mnt/") && lower != "/mnt/wsl/resolv.conf" && segs.len() < 5 {
            return false;
        }
        // The home itself is only ever a tmpfs.
        source != home
    }

    /// A small deterministic generator, so the property test needs no crate.
    struct Draw(u64);

    impl Draw {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn pick<'a>(&mut self, from: &[&'a str]) -> &'a str {
            from[(self.next() % from.len() as u64) as usize]
        }
    }

    #[test]
    fn no_drawn_plan_mounts_init_a_drive_userdata_or_a_claude() {
        let pieces = [
            "",
            "/",
            "/init",
            "/mnt",
            "/mnt/c",
            "/mnt/C",
            "/mnt/c/Users",
            "/mnt/wsl",
            "/mnt/c/Program Files (x86)/Steam/userdata/7",
            "/mnt/c/Users/A/.claude",
            "/mnt/c/Users/A/.claude.json",
            "/home/u",
            "/home/u/.claude",
            "/root",
            "/etc",
            "/mnt/c/Other/x",
            "/mnt/d/Extracts/vanilla",
            "/mnt/c/Other/x/../../Users",
            "/mnt/c/Other/ok (workshop)",
            "/mnt/c/A/B (workshop)",
            "/mnt/c/Users/A/AppData",
            "/mnt/c/Users/A/AppData/Local/nl.zmuuzn.riftstone/pawn/runs",
            "/mnt/e/Mods/USERDATA",
            "/mnt/c/Games/Steam/steamapps/common/Dragons Dogma 2",
            "/proc",
            "/dev",
        ];
        let ids = [
            RUN,
            RUN,
            RUN,
            "not-a-run",
            "0B5E2C6E-4A8D-4D3E-9F4B-2A1C7D9E8F00",
        ];
        let homes = [
            "/home/u",
            "/home/u",
            "/home/u",
            "/root",
            "/",
            "/mnt/c/Users/A",
            "/home/u/.claude",
            "home",
        ];
        // Each field draws from the hostile pieces and from its own plausible
        // values, so the generator reaches plans that pass as well as plans
        // that must not.
        let workshops = [
            "/mnt/c/Other/ok (workshop)",
            "/mnt/c/A/B (workshop)",
            "/mnt/c/Users/A/.claude/x (workshop)",
            "/mnt/c/Steam/userdata/x (workshop)",
            "/mnt/c/x (workshop)",
        ];
        let run_bases = [
            "/mnt/c/Users/A/AppData/Local/nl.zmuuzn.riftstone/pawn",
            "/mnt/c/Other/ok (workshop)/pawn",
            "/mnt/c/Users/A/.claude/pawn",
        ];
        let mut draw = Draw(0x5eed_7a45_1e00_0001);
        let mut accepted = 0;
        for _ in 0..20_000 {
            let run_id = draw.pick(&ids).to_string();
            let run_base = if draw.next() % 4 == 0 {
                draw.pick(&pieces)
            } else {
                draw.pick(&run_bases)
            };
            let workshop = if draw.next() % 4 == 0 {
                draw.pick(&pieces)
            } else {
                draw.pick(&workshops)
            };
            let plan = FencePlan {
                home: draw.pick(&homes).into(),
                user: "u".into(),
                workshop: workshop.into(),
                run_dir: if draw.next() % 2 == 0 {
                    format!("{run_base}/runs/{run_id}")
                } else {
                    run_base.into()
                },
                run_id,
                inputs: (0..draw.next() % 3)
                    .map(|_| draw.pick(&pieces).to_string())
                    .collect(),
            };
            let Ok(argv) = bwrap_argv(&plan, &FenceCommand::Pawn) else {
                continue;
            };
            accepted += 1;
            assert!(!argv.iter().any(|w| w == "/init"), "{plan:?}");
            for (flag, source, _) in mounts(&argv) {
                if flag == "--bind" || flag == "--ro-bind" {
                    assert!(
                        source_is_safe(&source, &plan.home),
                        "{flag} {source} from {plan:?}"
                    );
                }
            }
        }
        assert!(
            accepted > 100,
            "the generator must reach plans that pass ({accepted})"
        );
    }

    #[test]
    fn the_wsl_command_carries_no_double_quote_and_quotes_every_word() {
        let p = plan();
        let argv = bwrap_argv(&p, &FenceCommand::Pawn).unwrap();
        let args = wsl_args("Ubuntu-24.04", &p, &argv).unwrap();
        assert_eq!(
            args[..7],
            ["-d", "Ubuntu-24.04", "--cd", "/", "-e", "/bin/sh", "-c"]
        );
        let script = &args[7];
        assert!(!script.contains('"'));
        assert!(script.starts_with("exec 'bwrap' '--die-with-parent'"));
        assert!(script.ends_with(" 9<'/home/arisen/.local/share/riftstone/pawn/relay/fence.bpf'"));
        assert!(script.contains("'/mnt/c/Other/The Rift/mods (workshop)'"));
        assert_eq!(sh_quote("it's"), r"'it'\''s'");
        assert!(wsl_args("Ubuntu; rm -rf /", &p, &argv).is_err());
        assert!(wsl_args("", &p, &argv).is_err());
    }

    #[test]
    fn the_settings_keep_the_inner_sandbox_closed_and_approvals_on() {
        let settings: serde_json::Value = serde_json::from_slice(&pawn_settings()).unwrap();
        assert_eq!(settings["sandbox"]["enabled"], true);
        assert_eq!(settings["sandbox"]["failIfUnavailable"], true);
        assert_eq!(settings["sandbox"]["allowUnsandboxedCommands"], false);
        assert_eq!(settings["permissions"]["defaultMode"], "default");
        assert_eq!(
            settings["permissions"]["disableBypassPermissionsMode"],
            "disable"
        );
        assert_eq!(settings["disableAllHooks"], true);
        let mcp: serde_json::Value = serde_json::from_slice(&mcp_config()).unwrap();
        let servers = mcp["mcpServers"].as_object().unwrap();
        assert_eq!(servers.len(), 1, "the relay is the only MCP server");
        assert_eq!(servers["riftstone"]["args"][1], "/stone/relay/relay.py");
    }

    #[test]
    fn the_ground_ignores_the_run_and_changes_with_everything_else() {
        let host = Host {
            distro: "Ubuntu".into(),
            claude_version: "2.1.312".into(),
            kernel: "6.18.33.2-microsoft-standard-WSL2".into(),
        };
        let p = plan();
        let base = ground(&p, &host, b"relay").unwrap();
        assert_eq!(base.digest.len(), 64);
        let mut another_run = p.clone();
        another_run.run_id = "11111111-2222-4333-8444-555555555555".into();
        another_run.run_dir = p.run_dir.replace(RUN, &another_run.run_id);
        assert_eq!(ground(&another_run, &host, b"relay").unwrap(), base);
        assert_ne!(ground(&p, &host, b"relay 2").unwrap().digest, base.digest);
        let mut more_inputs = p.clone();
        more_inputs.inputs.push("/mnt/d/Extracts/more".into());
        assert_ne!(
            ground(&more_inputs, &host, b"relay").unwrap().digest,
            base.digest
        );
        let mut other_host = host.clone();
        other_host.claude_version = "2.1.320".into();
        let other = ground(&p, &other_host, b"relay").unwrap();
        assert_eq!(other.digest, base.digest, "the version is named apart");
        assert_ne!(other, base);
    }

    fn roots() -> Vec<ProtectedRoot> {
        [
            ("Steam", r"C:\Program Files (x86)\Steam"),
            (
                "the game folder",
                r"D:\SteamLibrary\steamapps\common\Dragons Dogma 2",
            ),
            ("the mods folder", r"C:\Other\The Rift\mods"),
            (
                "the older versions folder",
                r"C:\Other\The Rift\mods (older versions)",
            ),
            ("the workshop", r"C:\Other\The Rift\mods (workshop)"),
            (
                "the stone's app data",
                r"C:\Users\Arisen\AppData\Roaming\nl.zmuuzn.riftstone",
            ),
            (
                "the stone's local data",
                r"C:\Users\Arisen\AppData\Local\nl.zmuuzn.riftstone",
            ),
            ("your .claude", r"C:\Users\Arisen\.claude"),
            ("your profile", r"C:\Users\Arisen"),
            ("a save backup", r"E:\Backups\DD2 saves"),
        ]
        .into_iter()
        .map(|(name, path)| ProtectedRoot {
            name: name.into(),
            path: path.into(),
        })
        .collect()
    }

    #[test]
    fn an_input_overlapping_a_protected_root_either_way_is_refused_by_name() {
        let roots = roots();
        for root in &roots {
            let equal = root.path.clone();
            let child = format!(r"{}\inner\deeper", root.path);
            let parent = {
                let mut parts = normalize_windows(&root.path).unwrap();
                parts.pop();
                parts.join("\\")
            };
            let aliases = [
                format!(r"\\?\{}", root.path),
                root.path.to_uppercase(),
                root.path.replace('\\', "/"),
                format!(r"{}\", root.path),
                format!(r"{}\\", root.path.to_lowercase()),
            ];
            for path in [equal, child, parent].iter().chain(aliases.iter()) {
                let refusal = check_mount_source(path, &MountKind::Input, &roots).unwrap_err();
                let MountRefusal::Overlaps(name) = &refusal else {
                    panic!("{path}: {refusal:?}");
                };
                assert!(!name.contains('\\'), "a refusal names a root, never a path");
            }
        }
        // A sibling of every root is a fine input.
        for sibling in [
            r"C:\Other\The Rift\vanilla",
            r"D:\SteamLibrary\steamapps\common\Elden Ring",
            r"E:\Backups\Other",
            r"C:\Users\Arisenne\Extracts",
            r"C:\Other\The Rift\mods (pinned inputs)",
        ] {
            assert_eq!(
                check_mount_source(sibling, &MountKind::Input, &roots),
                Ok(()),
                "{sibling}"
            );
        }
        // A drive, a relative path, a climb, a UNC share.
        for bad in [
            r"D:\",
            "D:",
            r"Other\x",
            r"C:\Other\..\Users",
            r"\\server\share\x",
        ] {
            assert!(
                check_mount_source(bad, &MountKind::Input, &roots).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn a_trusted_mount_is_exactly_the_stones_own_folder() {
        let roots = roots();
        let workshop = r"C:\Other\The Rift\mods (workshop)";
        let kind = MountKind::Trusted {
            name: "The workshop",
            expected: workshop,
        };
        assert_eq!(check_mount_source(workshop, &kind, &roots), Ok(()));
        assert_eq!(
            check_mount_source(r"\\?\c:\other\the rift\MODS (workshop)\", &kind, &roots),
            Ok(())
        );
        for other in [
            r"C:\Other\The Rift\mods (workshop)\dist",
            r"C:\Other\The Rift",
            r"C:\Program Files (x86)\Steam",
        ] {
            assert_eq!(
                check_mount_source(other, &kind, &roots),
                Err(MountRefusal::NotTheStones("The workshop".into())),
                "{other}"
            );
        }
    }

    #[test]
    fn windows_paths_reach_wsl_by_their_drive() {
        assert_eq!(
            windows_to_wsl(r"C:\Other\The Rift\mods (workshop)").as_deref(),
            Some("/mnt/c/Other/The Rift/mods (workshop)")
        );
        assert_eq!(windows_to_wsl(r"\\?\D:\x\").as_deref(), Some("/mnt/d/x"));
        assert_eq!(windows_to_wsl(r"\\server\share"), None);
        assert_eq!(windows_to_wsl(r"C:\a\..\b"), None);
    }
}

#[cfg(test)]
mod bench {
    use super::*;

    /// Prints the `wsl.exe` arguments for an invented plan, as JSON, for a
    /// bench run by hand (`RIFT_BENCH_FENCE_PLAN=<plan.json>`).
    #[test]
    #[ignore = "a bench aid: prints a fence for a hand run"]
    fn print_a_fence_for_the_bench() {
        let path = std::env::var("RIFT_BENCH_FENCE_PLAN").expect("RIFT_BENCH_FENCE_PLAN");
        let raw: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let s = |k: &str| raw[k].as_str().unwrap().to_string();
        let plan = FencePlan {
            home: s("home"),
            user: s("user"),
            workshop: s("workshop"),
            run_dir: s("runDir"),
            run_id: s("runId"),
            inputs: raw["inputs"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect(),
        };
        let command = match raw["script"].as_str() {
            Some(script) => FenceCommand::Probe {
                script: script.into(),
            },
            None => FenceCommand::Pawn,
        };
        let argv = bwrap_argv(&plan, &command).unwrap();
        let args = wsl_args(&s("distro"), &plan, &argv).unwrap();
        std::fs::write(plan.filter_file(), fence_filter()).unwrap();
        println!("WSLARGS {}", serde_json::to_string(&args).unwrap());
    }
}
