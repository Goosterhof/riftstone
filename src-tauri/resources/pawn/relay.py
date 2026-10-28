"""The Riftstone's relay: Tamsin's only MCP server (BUILD-SPEC § v0.7, *The tools*).

It speaks MCP (JSON-RPC 2.0, one message per line) to Claude Code on stdin and
stdout, and nothing else. Each tool call becomes one request in the run's
mailbox: an exclusive create of `requests/<uuid>.tmp`, then a rename to
`requests/<uuid>.json`. The stone answers in `answers/<uuid>.json`, which is read
only here; this relay polls it every 100 ms (inotify does not see Windows-side
writes to drvfs) for up to 120 s.

A call that timed out is answered `unknown`. So is one whose mailbox read or
write failed after the request was published, or while its id was already out
(the Heretic's WS2): the stone may have it. Calling the same tool with the same
arguments again reuses that id, so the stone answers it from its ledger and
repeats nothing. Only a failure proven to come before publication (the rename)
is `refused`: nothing was asked.

The stone writes this file at summon and mounts it read-only. Run:
    python3 -I relay.py [mailbox] [wait seconds]   (default /stone/mailbox, 120)
Standard library only.
"""

import json
import os
import sys
import time
import uuid

VERSION = "0.7.0"
POLL_SECONDS = 0.1
WAIT_SECONDS = 120.0
TIMEOUT_TEXT = (
    "unknown: the stone may still be doing this. Call list_installed, "
    "or call the same tool again, which repeats nothing."
)
TRANSPORT_TEXT = (
    "unknown: the mailbox failed after the request was sent ({}). The stone may "
    "still be doing this. Call list_installed, or call the same tool again, which "
    "repeats nothing."
)


def schema(properties, required):
    return {
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": False,
    }


OPTION_ID = {"type": "string", "minLength": 1, "maxLength": 512}

TOOLS = [
    {
        "name": "search_library",
        "description": "Search the Arisen's mod library: every option, description, menu trail and file path. Returns option ids, names and install state.",
        "inputSchema": schema(
            {
                "query": {"type": "string", "maxLength": 200},
                "limit": {"type": "integer", "minimum": 1, "maximum": 50},
            },
            ["query", "limit"],
        ),
    },
    {
        "name": "get_option",
        "description": "One option's detail, its files included.",
        "inputSchema": schema({"optionId": OPTION_ID}, ["optionId"]),
    },
    {
        "name": "list_installed",
        "description": "What is installed now, with ids, origins and who placed each; also whether a Play is waiting and the Arisen's last answer to one.",
        "inputSchema": schema({}, []),
    },
    {
        "name": "plan_install",
        "description": "Plan installing one option: what it writes and any warning. Returns a planToken that install needs.",
        "inputSchema": schema({"optionId": OPTION_ID}, ["optionId"]),
    },
    {
        "name": "install",
        "description": "Install one option exactly as its plan said. Refused as stalePlan if the library or the game changed since that plan.",
        "inputSchema": schema(
            {
                "optionId": OPTION_ID,
                "planToken": {"type": "string", "pattern": "^[0-9a-f]{64}$"},
            },
            ["optionId", "planToken"],
        ),
    },
    {
        "name": "uninstall",
        "description": "Uninstall one install, by its id.",
        "inputSchema": schema({"installId": OPTION_ID}, ["installId"]),
    },
    {
        "name": "play",
        "description": "Ask the Arisen to start the game. It is held until the Arisen answers; a later call, or list_installed, tells you the answer.",
        "inputSchema": schema({}, []),
    },
    {
        "name": "read_reframework_log",
        "description": "The last lines of REFramework's log from the game's last start, read only.",
        "inputSchema": schema(
            {"tailLines": {"type": "integer", "minimum": 1, "maximum": 2000}},
            ["tailLines"],
        ),
    },
]
TOOL_NAMES = {tool["name"] for tool in TOOLS}


class Mailbox:
    def __init__(self, root, wait=WAIT_SECONDS):
        self.wait = wait
        self.requests = os.path.join(root, "requests")
        self.answers = os.path.join(root, "answers")
        # (tool, arguments) of calls that timed out, to the id they used.
        self.timed_out = {}

    def post(self, request_id, tool, args):
        body = json.dumps({"id": request_id, "tool": tool, "args": args}).encode("utf-8")
        tmp = os.path.join(self.requests, request_id + ".tmp")
        try:
            os.unlink(tmp)  # this relay's own half-written try of the same id
        except FileNotFoundError:
            pass
        fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        try:
            os.write(fd, body)
            os.fsync(fd)
        finally:
            os.close(fd)
        os.rename(tmp, os.path.join(self.requests, request_id + ".json"))

    def answer(self, request_id):
        path = os.path.join(self.answers, request_id + ".json")
        try:
            with open(path, "rb") as handle:
                return json.loads(handle.read().decode("utf-8"))
        except FileNotFoundError:
            return None
        except ValueError:
            return None  # still being renamed into place: read it again

    def call(self, tool, args):
        key = json.dumps([tool, args], sort_keys=True)
        # An id already out (an earlier call timed out or failed) may be with
        # the stone: any failure now leaves the outcome unknown.
        published = key in self.timed_out
        request_id = self.timed_out.get(key) or str(uuid.uuid4())
        try:
            answered = self.answer(request_id)
            if answered is None:
                self.post(request_id, tool, args)
                published = True
                deadline = time.monotonic() + self.wait
                while answered is None and time.monotonic() < deadline:
                    time.sleep(POLL_SECONDS)
                    answered = self.answer(request_id)
        except OSError as err:
            if not published:
                raise  # before publication: nothing was asked
            self.timed_out[key] = request_id
            return {"outcome": "unknown", "text": TRANSPORT_TEXT.format(err), "data": None}
        if answered is None:
            self.timed_out[key] = request_id
            return {"outcome": "unknown", "text": TIMEOUT_TEXT, "data": None}
        self.timed_out.pop(key, None)
        return answered


def tool_result(answer):
    text = answer.get("text", "")
    data = answer.get("data")
    body = {"outcome": answer.get("outcome"), "text": text}
    if data is not None:
        body["data"] = data
    return {
        "content": [{"type": "text", "text": json.dumps(body, ensure_ascii=False)}],
        "isError": answer.get("outcome") == "refused",
    }


def handle(mailbox, message):
    method = message.get("method")
    params = message.get("params") or {}
    if method == "initialize":
        return {
            "protocolVersion": params.get("protocolVersion", "2025-06-18"),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "riftstone", "version": VERSION},
        }
    if method == "ping":
        return {}
    if method == "tools/list":
        return {"tools": TOOLS}
    if method == "tools/call":
        name = params.get("name")
        args = params.get("arguments") or {}
        if name not in TOOL_NAMES or not isinstance(args, dict):
            raise LookupError("The stone has no tool named " + str(name) + ".")
        return tool_result(mailbox.call(name, args))
    raise NotImplementedError(method)


def serve(mailbox, reader, writer):
    for line in reader:
        line = line.strip()
        if not line:
            continue
        try:
            message = json.loads(line)
        except ValueError:
            reply = {"jsonrpc": "2.0", "id": None, "error": {"code": -32700, "message": "Parse error"}}
            writer.write(json.dumps(reply) + "\n")
            writer.flush()
            continue
        if "id" not in message:
            continue  # a notification: nothing to answer
        reply = {"jsonrpc": "2.0", "id": message["id"]}
        try:
            reply["result"] = handle(mailbox, message)
        except NotImplementedError:
            reply["error"] = {"code": -32601, "message": "Method not found"}
        except LookupError as err:
            reply["error"] = {"code": -32602, "message": str(err)}
        except OSError as err:
            reply["result"] = tool_result(
                {"outcome": "refused", "text": "The mailbox could not be written: " + str(err) + "."}
            )
        writer.write(json.dumps(reply, ensure_ascii=False) + "\n")
        writer.flush()


if __name__ == "__main__":
    root = sys.argv[1] if len(sys.argv) > 1 else "/stone/mailbox"
    wait = float(sys.argv[2]) if len(sys.argv) > 2 else WAIT_SECONDS
    serve(Mailbox(root, wait), sys.stdin, sys.stdout)
