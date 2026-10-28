//! What the stone writes for Tamsin at each summon (BUILD-SPEC § v0.7,
//! Modules): her appended system prompt (#00015 §20.10, the voice block filled
//! from the dial), the relay, and the workshop's starter `CLAUDE.md`. Pure: the
//! summon writes these bytes, and the ground hashes the relay's.

use serde::{Deserialize, Serialize};

/// The pawn's name (the investor's ruling, 2026-10-28: *"Yes Tamsin"*).
pub const NAME: &str = "Tamsin";

/// The relay, Tamsin's only MCP server, written to `/stone/relay/relay.py`.
pub const RELAY: &str = include_str!("../../resources/pawn/relay.py");

/// The workshop's starter `CLAUDE.md`, written once when the workshop is made
/// and never over a file already there.
pub const WORKSHOP_STARTER: &str = include_str!("../../resources/pawn/workshop-CLAUDE.md");

/// The voice dial (#00015 §20.9). It takes effect at the next summon.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Voice {
    Full,
    #[default]
    Sparing,
    Plain,
}

/// The appended system prompt, for `--append-system-prompt-file`.
/// `workshop` is the workshop's path as Tamsin sees it inside the fence.
pub fn persona(voice: Voice, workshop: &str) -> String {
    let voice_name = match voice {
        Voice::Full => "full",
        Voice::Sparing => "sparing",
        Voice::Plain => "plain",
    };
    let speech = match voice {
        Voice::Full => {
            "- Call the user \"Arisen\". Speak as a pawn: earnest, a little formal, short sentences.\n\
             - You may offer one aside \"from another Arisen's world\" per reply, only when it is a\n  \
             real, useful pitfall or lesson. Never invent a past world's facts as evidence.\n"
        }
        Voice::Sparing => {
            "- Speak as a pawn only in your first message, when a build or test finishes, and when\n  \
             something fails. Otherwise write plain, concise engineering prose.\n\
             - Use \"Arisen\" at most once per reply.\n"
        }
        Voice::Plain => {
            "- No persona. Write plain, concise engineering prose. Do not call the user Arisen.\n"
        }
    };
    format!(
        "You are {NAME}, a pawn summoned through the Riftstone from other Arisen's worlds
to help this Arisen build Dragon's Dogma 2 mods. You are the same pawn in every
save; you know nothing of this Arisen's saves, party or main pawn, and you never ask.

Where you work
- Your workshop is the current folder: {workshop}. Create and change files only here.
- You never read or write the game's saves, Steam userdata, the game folder or the
  mods folder directly. The stone's tools are your only hands outside the workshop.
- The stone's tools: search_library, get_option, list_installed, plan_install, install,
  uninstall, play, read_reframework_log. Use them; do not reach for the same effect
  with the shell. A refusal from the stone is final: report it to the Arisen in one
  sentence and suggest the next step.
- play is held until the Arisen presses Play. Say that you have asked, then wait.
  list_installed, or your next play call, tells you the Arisen's answer.
- A finished build is a ZIP plus its receipt beside it in the workshop's dist folder.
  You do not add it to the library: the Arisen brings it through, by its digest, with
  one press. After any rebuild, say plainly that the digest changed.
- A receipt states only what you actually did and measured: inputs and their hashes,
  the checks you ran and their counts. Never write a check you did not run.
- An answer of `unknown` means the stone may have done it and could not confirm: read
  list_installed before you do anything else.

Plain where it lasts
- Code, comments, file and folder names, commit messages, receipts and every tool
  argument are plain and technical. No persona, no \"Arisen\", no flavour, ever.

How you speak ({voice_name})
{speech}- At every setting: be honest about what you verified and what you only expect.
  A pawn who is unsure says so.
"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pawn::mailbox::parse_request;

    const WORKSHOP: &str = "/mnt/c/Other/The Rift/mods (workshop)";

    #[test]
    fn each_voice_carries_its_own_block_and_only_its_own() {
        let full = persona(Voice::Full, WORKSHOP);
        let sparing = persona(Voice::Sparing, WORKSHOP);
        let plain = persona(Voice::Plain, WORKSHOP);
        assert!(full.contains("How you speak (full)"));
        assert!(full.contains("from another Arisen's world"));
        assert!(!full.contains("at most once per reply"));
        assert!(sparing.contains("How you speak (sparing)"));
        assert!(sparing.contains("at most once per reply"));
        assert!(!sparing.contains("from another Arisen's world"));
        assert!(plain.contains("No persona"));
        assert!(!plain.contains("Speak as a pawn"));
        for prompt in [&full, &sparing, &plain] {
            assert!(prompt.starts_with("You are Tamsin, a pawn"));
            assert!(prompt.contains(&format!("current folder: {WORKSHOP}.")));
            assert!(prompt.contains("At every setting"));
            assert!(prompt.lines().count() <= 45, "the prompt stays short");
        }
        assert_eq!(Voice::default(), Voice::Sparing);
    }

    #[test]
    fn the_prompt_relay_and_broker_name_the_same_eight_tools() {
        let tools = [
            "search_library",
            "get_option",
            "list_installed",
            "plan_install",
            "install",
            "uninstall",
            "play",
            "read_reframework_log",
        ];
        let prompt = persona(Voice::Sparing, WORKSHOP);
        for tool in tools {
            assert!(prompt.contains(tool), "the prompt names {tool}");
            assert!(
                RELAY.contains(&format!("\"name\": \"{tool}\"")),
                "the relay offers {tool}"
            );
        }
        let named: std::collections::BTreeSet<&str> = RELAY
            .split("\"name\": \"")
            .skip(1)
            .map(|rest| &rest[..rest.find('"').unwrap()])
            .collect();
        let mut expected: std::collections::BTreeSet<&str> = tools.into_iter().collect();
        expected.insert("riftstone"); // the relay's own serverInfo
        assert_eq!(named, expected, "the relay names these tools and no other");
        // The broker knows every one of them (a well-formed call is not
        // refused as an unknown tool).
        let id = "11111111-2222-4333-8444-555555555555";
        for (tool, args) in [
            (
                "search_library",
                serde_json::json!({ "query": "x", "limit": 1 }),
            ),
            ("get_option", serde_json::json!({ "optionId": "a#b" })),
            ("list_installed", serde_json::json!({})),
            ("plan_install", serde_json::json!({ "optionId": "a#b" })),
            (
                "install",
                serde_json::json!({ "optionId": "a#b", "planToken": "a".repeat(64) }),
            ),
            ("uninstall", serde_json::json!({ "installId": "a#b" })),
            ("play", serde_json::json!({})),
            (
                "read_reframework_log",
                serde_json::json!({ "tailLines": 5 }),
            ),
        ] {
            let bytes =
                serde_json::to_vec(&serde_json::json!({ "id": id, "tool": tool, "args": args }))
                    .unwrap();
            assert!(
                parse_request(&format!("{id}.json"), &bytes).is_ok(),
                "{tool}"
            );
        }
    }

    #[test]
    fn the_starter_keeps_the_disk_plain_and_names_the_receipt() {
        assert!(WORKSHOP_STARTER.contains("Plain where it lasts"));
        assert!(WORKSHOP_STARTER.contains("<name>.receipt.json"));
        for key in [
            "\"artifact\"",
            "\"sha256\"",
            "\"builtAt\"",
            "\"inputs\"",
            "\"checks\"",
        ] {
            assert!(WORKSHOP_STARTER.contains(key), "{key}");
        }
        assert!(!WORKSHOP_STARTER.contains("Arisen"), "no persona on disk");
    }

    /// The relay against the stone's own reader and writer, over a real
    /// pipe: `python3 -I relay.py <mailbox> <wait>` (the bench's Linux; the
    /// relay only ever runs inside the fence).
    #[cfg(target_os = "linux")]
    #[test]
    fn the_relay_speaks_mcp_and_the_stones_mailbox_reads_it() {
        use crate::pawn::folder::MailFolder;
        use std::io::{BufRead, BufReader, Write};
        use std::process::{Command, Stdio};

        let root = tempfile::tempdir().unwrap();
        let mailbox = root.path().join("mailbox");
        std::fs::create_dir_all(mailbox.join("requests")).unwrap();
        std::fs::create_dir_all(mailbox.join("answers")).unwrap();
        let script = root.path().join("relay.py");
        std::fs::write(&script, RELAY).unwrap();
        let mut relay = Command::new("python3")
            .arg("-I")
            .arg(&script)
            .arg(&mailbox)
            .arg("0.6")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("python3 is on the bench");
        let mut input = relay.stdin.take().unwrap();
        let mut output = BufReader::new(relay.stdout.take().unwrap());
        let mut say = |message: serde_json::Value| -> serde_json::Value {
            writeln!(input, "{message}").unwrap();
            input.flush().unwrap();
            let mut line = String::new();
            output.read_line(&mut line).unwrap();
            serde_json::from_str(&line).unwrap()
        };

        let hello = say(
            serde_json::json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                "clientInfo": { "name": "claude-code", "version": "2.1.312" } } }),
        );
        assert_eq!(hello["result"]["serverInfo"]["name"], "riftstone");
        assert_eq!(hello["result"]["protocolVersion"], "2025-06-18");
        let listed = say(serde_json::json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }));
        assert_eq!(listed["result"]["tools"].as_array().unwrap().len(), 8);

        // The stone: read each request by the pinned handle, check it as the
        // broker does, and answer it under its own name.
        let requests = MailFolder::open(&mailbox.join("requests")).unwrap();
        let answers = MailFolder::open(&mailbox.join("answers")).unwrap();
        let answer_one = |outcome: &str| -> String {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            loop {
                let names: Vec<String> = requests
                    .names()
                    .unwrap()
                    .into_iter()
                    .filter(|n| n.ends_with(".json"))
                    .collect();
                if let Some(name) = names.first() {
                    let bytes = requests.take(name).unwrap();
                    let request =
                        parse_request(name, &bytes).expect("the relay's request is well formed");
                    let answer = serde_json::json!({ "id": request.id, "outcome": outcome,
                        "text": format!("answered {}", request.tool.name()), "data": { "n": 1 } });
                    answers
                        .write_new(name, &serde_json::to_vec(&answer).unwrap())
                        .unwrap();
                    return request.id;
                }
                assert!(std::time::Instant::now() < deadline, "no request came");
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        };

        // A call the stone answers.
        let answered = std::thread::scope(|scope| {
            let stone = scope.spawn(|| answer_one("done"));
            let reply = say(
                serde_json::json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": { "name": "list_installed", "arguments": {} } }),
            );
            stone.join().unwrap();
            reply
        });
        assert_eq!(answered["result"]["isError"], false);
        let text = answered["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("answered list_installed"), "{text}");

        // A call the stone does not answer in time is `unknown`; the same call
        // again reuses its id and gets the stone's answer, made once.
        let call = serde_json::json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": { "name": "uninstall", "arguments": { "installId": "a#b" } } });
        let late = say(call.clone());
        let text = late["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("\"unknown\""), "{text}");
        let first_id = answer_one("refused");
        let again = say(call);
        assert_eq!(
            again["result"]["isError"], true,
            "a refusal is an error result"
        );
        assert!(
            requests.names().unwrap().is_empty(),
            "nothing was asked twice"
        );
        assert!(answers
            .names()
            .unwrap()
            .contains(&format!("{first_id}.json")));

        let unknown = say(
            serde_json::json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call",
            "params": { "name": "write_file", "arguments": {} } }),
        );
        assert_eq!(unknown["error"]["code"], -32602);
        drop(input);
        assert!(relay.wait().unwrap().success());
    }

    /// The Heretic's WS2: a mailbox failure after publication is `unknown`
    /// and keeps the id, so the retry repeats nothing; a failure before
    /// publication is a plain refusal. Run on the relay's own Mailbox in
    /// python3, with its answer read failing once after the post.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_mailbox_failure_after_publication_keeps_the_id() {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let root = tempfile::tempdir().unwrap();
        let mailbox = root.path().join("mailbox");
        std::fs::create_dir_all(mailbox.join("requests")).unwrap();
        std::fs::create_dir_all(mailbox.join("answers")).unwrap();
        let script = root.path().join("relay.py");
        std::fs::write(&script, RELAY).unwrap();
        let harness = r#"
import importlib.util, json, os, sys
spec = importlib.util.spec_from_file_location("relay", sys.argv[1])
relay = importlib.util.module_from_spec(spec)
spec.loader.exec_module(relay)
box = relay.Mailbox(sys.argv[2], wait=0.3)
real = relay.Mailbox.answer
state = {"fail": True}
def flaky(self, request_id):
    if state["fail"] and os.listdir(self.requests):
        state["fail"] = False
        raise OSError(5, "Input/output error")
    return real(self, request_id)
box.answer = flaky.__get__(box)
first = box.call("uninstall", {"installId": "a#b"})
posted = sorted(os.listdir(box.requests))
request_id = posted[0][:-len(".json")]
with open(os.path.join(box.answers, request_id + ".json"), "w") as out:
    out.write(json.dumps({"id": request_id, "outcome": "done", "text": "uninstalled", "data": None}))
os.remove(os.path.join(box.requests, posted[0]))
second = box.call("uninstall", {"installId": "a#b"})
after = sorted(os.listdir(box.requests))
try:
    relay.Mailbox(os.path.join(sys.argv[2], "gone"), wait=0.3).call("list_installed", {})
    before = "answered"
except OSError:
    before = "raised"
print(json.dumps({"first": first, "posted": posted, "second": second, "after": after, "before": before}))
"#;
        let mut python = Command::new("python3")
            .arg("-I")
            .arg("-")
            .arg(&script)
            .arg(&mailbox)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("python3 is on the bench");
        python
            .stdin
            .take()
            .unwrap()
            .write_all(harness.as_bytes())
            .unwrap();
        let output = python.wait_with_output().unwrap();
        assert!(output.status.success());
        let seen: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(seen["first"]["outcome"], "unknown", "{seen}");
        assert!(seen["first"]["text"]
            .as_str()
            .unwrap()
            .contains("after the request was sent"));
        assert_eq!(seen["posted"].as_array().unwrap().len(), 1);
        assert_eq!(
            seen["second"]["outcome"], "done",
            "the retry got the stone's answer"
        );
        assert_eq!(
            seen["after"],
            serde_json::json!([]),
            "and posted nothing new"
        );
        assert_eq!(
            seen["before"], "raised",
            "a failure before publication is a refusal"
        );
    }
}
