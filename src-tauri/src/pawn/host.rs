//! The pawn's host (RS-9.1): the WSL distro Tamsin runs in, read and made
//! ready before each summon, outside the fence.
//!
//! [`survey`] runs one small script in the distro and reads what the fence
//! needs: the user's home, the kernel, `bwrap`, `socat`, `python3` and Claude
//! Code. A missing piece refuses the summon (`noFence`), naming the piece and
//! the command that installs it; the stone never falls back to running a
//! pawn unfenced. [`install_files`] writes the relay folder's files through
//! `python3 -I -` on stdin, so no file's bytes ever pass through `wsl.exe`'s
//! argument quoting.
//!
//! On Windows the shell is `wsl.exe` with `WSLENV` emptied; on the Linux
//! bench it is `/bin/sh` itself, so the same steps run in the tests.

use std::io::{self, Write};
use std::process::{Command, Stdio};

use crate::pawn::fence::{FencePlan, Host};

/// What a script printed, and whether it succeeded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// A `/bin/sh` in the pawn's distro, outside the fence.
pub trait Shell: Send + Sync {
    fn run(&self, script: &str, stdin: &[u8]) -> io::Result<ShellOutput>;
}

fn run_command(mut command: Command, stdin: &[u8]) -> io::Result<ShellOutput> {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(mut pipe) = child.stdin.take() {
        pipe.write_all(stdin)?;
    }
    let output = child.wait_with_output()?;
    Ok(ShellOutput {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

/// `wsl.exe [-d <distro>] --cd / -e /bin/sh -c <script>`, `WSLENV` emptied.
pub struct Wsl {
    pub distro: Option<String>,
}

impl Shell for Wsl {
    fn run(&self, script: &str, stdin: &[u8]) -> io::Result<ShellOutput> {
        let mut command = Command::new("wsl.exe");
        if let Some(distro) = &self.distro {
            command.args(["-d", distro]);
        }
        command
            .args(["--cd", "/", "-e", "/bin/sh", "-c", script])
            .env("WSLENV", "");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        run_command(command, stdin)
    }
}

/// `/bin/sh -c <script>` on the Linux bench.
pub struct LocalSh;

impl Shell for LocalSh {
    fn run(&self, script: &str, stdin: &[u8]) -> io::Result<ShellOutput> {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", script]);
        run_command(command, stdin)
    }
}

/// The survey: one `key=value` line each. No double quote anywhere: `wsl.exe`
/// does not read `\"` back.
pub const SURVEY: &str = "echo distro=$WSL_DISTRO_NAME
echo home=$HOME
echo user=$(id -un)
echo kernel=$(uname -r)
echo bwrap=$(command -v bwrap)
echo socat=$(command -v socat)
echo python3=$(command -v python3)
echo claude=$($HOME/.local/bin/claude --version 2>/dev/null)
";

/// A piece the fence needs that the distro does not have (`noFence`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Missing {
    pub piece: String,
    /// The command that installs it, inside the distro.
    pub remedy: String,
}

impl Missing {
    pub fn sentence(&self) -> String {
        format!(
            "Tamsin cannot be summoned: {} is missing in WSL. Install it with `{}`, then summon her again.",
            self.piece, self.remedy
        )
    }
}

/// What the survey found: the host facts the ground names, and the home and
/// user the fence's plan needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Survey {
    pub host: Host,
    pub home: String,
    pub user: String,
}

/// Read the survey's lines. The first missing piece refuses.
pub fn parse_survey(stdout: &str, distro_setting: Option<&str>) -> Result<Survey, Missing> {
    let value = |key: &str| -> String {
        stdout
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{key}=")))
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let missing = |piece: &str, remedy: &str| Missing {
        piece: piece.into(),
        remedy: remedy.into(),
    };
    let home = value("home");
    let user = value("user");
    if home.is_empty() || user.is_empty() {
        return Err(missing("a WSL distro to run in", "wsl --install"));
    }
    for (key, piece, remedy) in [
        ("bwrap", "bubblewrap", "sudo apt install bubblewrap"),
        ("socat", "socat", "sudo apt install socat"),
        ("python3", "python3", "sudo apt install python3"),
    ] {
        if value(key).is_empty() {
            return Err(missing(piece, remedy));
        }
    }
    // `2.1.312 (Claude Code)` → `2.1.312`.
    let claude = value("claude");
    let claude_version = claude.split_whitespace().next().unwrap_or("").to_string();
    if claude_version.is_empty() || !claude_version.starts_with(|c: char| c.is_ascii_digit()) {
        return Err(missing(
            "Claude Code (in ~/.local/bin)",
            "curl -fsSL https://claude.ai/install.sh | bash",
        ));
    }
    let distro = match distro_setting {
        Some(named) => named.to_string(),
        None => value("distro"),
    };
    Ok(Survey {
        host: Host {
            distro,
            claude_version,
            kernel: value("kernel"),
        },
        home,
        user,
    })
}

/// Survey the distro through `shell`.
pub fn survey(shell: &dyn Shell, distro_setting: Option<&str>) -> Result<Survey, Missing> {
    match shell.run(SURVEY, b"") {
        Ok(output) if output.success => parse_survey(&output.stdout, distro_setting),
        _ => Err(Missing {
            piece: "a WSL distro to run in".into(),
            remedy: "wsl --install".into(),
        }),
    }
}

/// One file the stone writes into the distro: its absolute path, bytes and mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostFile {
    pub path: String,
    pub bytes: Vec<u8>,
    pub mode: u32,
}

/// The Python program `python3 -I -` reads on stdin: it makes each file's
/// folder (0700), writes the file through a `.tmp` and replaces it, and
/// makes every folder in `dirs` (0700). The files' bytes travel inside it as
/// base64, never through an argument.
pub fn install_program(files: &[HostFile], dirs: &[String]) -> String {
    use base64::Engine;
    let manifest: Vec<serde_json::Value> = files
        .iter()
        .map(|file| {
            serde_json::json!({
                "path": file.path,
                "mode": file.mode,
                "bytes": base64::engine::general_purpose::STANDARD.encode(&file.bytes),
            })
        })
        .collect();
    let payload = serde_json::json!({ "files": manifest, "dirs": dirs }).to_string();
    let encoded = base64::engine::general_purpose::STANDARD.encode(payload);
    format!(
        "import base64, json, os\n\
         plan = json.loads(base64.b64decode('{encoded}'))\n\
         for folder in plan['dirs']:\n    os.makedirs(folder, mode=0o700, exist_ok=True)\n\
         for item in plan['files']:\n    \
         os.makedirs(os.path.dirname(item['path']), mode=0o700, exist_ok=True)\n    \
         tmp = item['path'] + '.tmp'\n    \
         with open(tmp, 'wb') as out:\n        out.write(base64.b64decode(item['bytes']))\n    \
         os.chmod(tmp, item['mode'])\n    \
         os.replace(tmp, item['path'])\n\
         print('installed', len(plan['files']))\n"
    )
}

/// Write the relay folder's files and make Tamsin's config dir.
pub fn install_files(shell: &dyn Shell, files: &[HostFile], dirs: &[String]) -> Result<(), String> {
    let program = install_program(files, dirs);
    match shell.run("exec python3 -I -", program.as_bytes()) {
        Ok(output) if output.success => Ok(()),
        Ok(output) => Err(format!(
            "The stone could not write Tamsin's files in WSL: {}",
            output.stderr.trim()
        )),
        Err(err) => Err(format!("The stone could not reach WSL: {err}.")),
    }
}

/// The files a summon writes for `plan`: the relay, the settings, the MCP
/// config, the persona and the filter, all in the read-only relay folder.
pub fn relay_files(plan: &FencePlan, persona: &str) -> Vec<HostFile> {
    use crate::pawn::fence::{fence_filter, mcp_config, pawn_settings};
    let relay = plan.relay();
    let file = |name: &str, bytes: Vec<u8>| HostFile {
        path: format!("{relay}/{name}"),
        bytes,
        mode: 0o644,
    };
    vec![
        file("relay.py", crate::pawn::persona::RELAY.as_bytes().to_vec()),
        file("settings.json", pawn_settings()),
        file("mcp.json", mcp_config()),
        file("persona.md", persona.as_bytes().to_vec()),
        file("fence.bpf", fence_filter()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "distro=Ubuntu-24.04\nhome=/home/arisen\nuser=arisen\nkernel=6.18.33.2-microsoft-standard-WSL2\nbwrap=/usr/bin/bwrap\nsocat=/usr/bin/socat\npython3=/usr/bin/python3\nclaude=2.1.312 (Claude Code)\n";

    #[test]
    fn a_ready_distro_is_read_whole() {
        let survey = parse_survey(GOOD, None).unwrap();
        assert_eq!(survey.home, "/home/arisen");
        assert_eq!(survey.user, "arisen");
        assert_eq!(
            survey.host,
            Host {
                distro: "Ubuntu-24.04".into(),
                claude_version: "2.1.312".into(),
                kernel: "6.18.33.2-microsoft-standard-WSL2".into(),
            }
        );
        assert_eq!(
            parse_survey(GOOD, Some("Debian")).unwrap().host.distro,
            "Debian"
        );
    }

    #[test]
    fn each_missing_piece_names_itself_and_its_remedy() {
        for (key, piece, remedy) in [
            (
                "bwrap=/usr/bin/bwrap",
                "bubblewrap",
                "sudo apt install bubblewrap",
            ),
            ("socat=/usr/bin/socat", "socat", "sudo apt install socat"),
            (
                "python3=/usr/bin/python3",
                "python3",
                "sudo apt install python3",
            ),
            (
                "claude=2.1.312 (Claude Code)",
                "Claude Code (in ~/.local/bin)",
                "curl -fsSL https://claude.ai/install.sh | bash",
            ),
            (
                "home=/home/arisen",
                "a WSL distro to run in",
                "wsl --install",
            ),
        ] {
            let without = GOOD.replace(key, &format!("{}=", key.split('=').next().unwrap()));
            let refused = parse_survey(&without, None).unwrap_err();
            assert_eq!(refused.piece, piece);
            assert_eq!(refused.remedy, remedy);
            assert!(refused.sentence().contains(remedy));
        }
        assert!(parse_survey("", None).is_err());
    }

    #[test]
    fn the_survey_script_carries_no_double_quote() {
        assert!(!SURVEY.contains('"'));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_survey_runs_on_the_bench_shell() {
        let output = LocalSh.run(SURVEY, b"").unwrap();
        assert!(output.success);
        let home = std::env::var("HOME").unwrap_or_default();
        assert!(
            output.stdout.contains(&format!("home={home}")),
            "{}",
            output.stdout
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_files_go_through_stdin_whole_with_their_modes() {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        let base = root
            .path()
            .join("home with space/.local/share/riftstone/pawn");
        let relay = base.join("relay");
        let files = vec![
            HostFile {
                path: relay.join("relay.py").to_string_lossy().into_owned(),
                bytes: b"print('it\\'s \"quoted\"')\n".to_vec(),
                mode: 0o644,
            },
            HostFile {
                path: relay.join("fence.bpf").to_string_lossy().into_owned(),
                bytes: crate::pawn::fence::fence_filter(),
                mode: 0o644,
            },
        ];
        let claude = base.join("claude").to_string_lossy().into_owned();
        install_files(&LocalSh, &files, std::slice::from_ref(&claude)).unwrap();
        for file in &files {
            assert_eq!(
                std::fs::read(&file.path).unwrap(),
                file.bytes,
                "{}",
                file.path
            );
            let mode = std::fs::metadata(&file.path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o644);
        }
        let mode = std::fs::metadata(&claude).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "Tamsin's config dir is hers alone");
        // Written again, they are replaced, never doubled.
        install_files(&LocalSh, &files, &[claude]).unwrap();
        assert_eq!(
            std::fs::read_dir(&relay).unwrap().count(),
            2,
            "no .tmp left"
        );
    }

    #[test]
    fn a_summon_writes_the_five_relay_files() {
        let plan = FencePlan {
            home: "/home/arisen".into(),
            user: "arisen".into(),
            workshop: "/mnt/c/Other/The Rift/mods (workshop)".into(),
            run_dir: "/mnt/c/Users/A/AppData/Local/nl.zmuuzn.riftstone/pawn/runs/0b5e2c6e-4a8d-4d3e-9f4b-2a1c7d9e8f00".into(),
            run_id: "0b5e2c6e-4a8d-4d3e-9f4b-2a1c7d9e8f00".into(),
            inputs: Vec::new(),
        };
        let files = relay_files(&plan, "persona text");
        let names: Vec<&str> = files
            .iter()
            .map(|f| f.path.rsplit('/').next().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "relay.py",
                "settings.json",
                "mcp.json",
                "persona.md",
                "fence.bpf"
            ]
        );
        assert!(files.iter().all(|f| f
            .path
            .starts_with("/home/arisen/.local/share/riftstone/pawn/relay/")));
        assert_eq!(files[3].bytes, b"persona text");
    }
}
