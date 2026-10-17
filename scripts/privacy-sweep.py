#!/usr/bin/env python3
"""The privacy sweep — run before every push.

A player's mod library is their own business, and this repo is public. The
sweep collects every identifier a real library holds — archive and folder
names, the Nexus name shapes inside them (name, mod id, sqid), every
modinfo.ini name, bundle, addonfor parent, author and homepage mod id, what
the Riftstone's own journal remembers (install names and labels, package
files, option folders, pak names) and what its nexus.json remembers (each
link's mod, file and group id and sqid, the mod names and newer files the
checks brought home) — READ-ONLY, and greps the repo for each. It prints where
a hit is, never the identifier itself.

The folders it is pointed at are private too: where a player keeps their mods
is their own disk layout. Each path's folder chain, from the drive to the first
folder no stock install has, is swept as a "library location" with either
slash, so the same folder in a doc, a mock and a test all count.

A push publishes more than the working tree, so the sweep reads three places:
the working tree, every file version any ref reaches in the history, and every
commit message. A leak scrubbed from the tree but alive in history still counts.

    python3 -I scripts/privacy-sweep.py \\
        --library  "/mnt/c/<install dir>/mods" \\
        --journal  "/mnt/c/Users/<you>/AppData/Roaming/nl.zmuuzn.riftstone/journal-<key>.json" \\
        --nexus    "/mnt/c/Users/<you>/AppData/Roaming/nl.zmuuzn.riftstone/nexus.json" \\
        --downloads "/mnt/c/Users/<you>/Downloads"

With no arguments it reads them from scripts/privacy-sweep.args, which git
ignores, so the real paths stay on the machine. One option per line, `#` for a
comment:

    --library /mnt/c/<install dir>/mods
    --journal /mnt/c/Users/<you>/AppData/Roaming/nl.zmuuzn.riftstone/journal-<key>.json
    --nexus /mnt/c/Users/<you>/AppData/Roaming/nl.zmuuzn.riftstone/nexus.json
    --downloads /mnt/c/Users/<you>/Downloads
    --private-path C:\\<builds>\\riftstone-releases
    --private-word <your Nexus account name>
    --forbid <a word the tree must never carry>

`--library` is the mods folder (any other folder of mods counts too); every
archive and folder directly inside it is a package, except the names given with
`--skip-name` (a folder's own non-mod furniture). A library or downloads path
that is gone is noted and skipped, its location still swept. `--nexus` names
the stone's nexus.json; the nexus.json beside each `--journal` is read too (the
stone keeps both in its app data folder), so a forgotten line never drops the
Nexus ids. A journal or nexus.json that cannot be read or parsed is noted, never
skipped in silence. `--private-word` holds a private word no folder holds, such
as the Nexus account name.

`--forbid` names a word the tree must never carry again (a retired name). It is
checked in the working tree and in the tip a push publishes: every tracked file
at HEAD, wherever the working tree holds different text (an edit not yet
committed does not clean the tip), and HEAD's own message. The older history
keeps what it kept and is not read for it (BUILD-SPEC § v0.3, ruling 1). It
prints the place, never the word.

Pictures carry their words in pixels, and no text gate reads them: the sweep
counts the pictures in the tree by folder, so whoever pushes looks at them.

Exit 0 when the repo holds none of them, 1 when it does (each hit listed as
file:line and the identifier's kind), 2 on bad arguments. Zip archives are
read for their modinfo.ini files; 7z and rar archives contribute their names
only. Names in Downloads count only for archives that look like Dragon's
Dogma 2 mods. A version is never collected: alone it names nothing, and it
would trip on every dependency's version.

`--allow` takes identifiers that are public on their own (a tool's name such
as REFramework or ReShade, which search_terms keeps whole); they are skipped. So are single words of six letters or
fewer (`Extras`, `Tops`): too common to identify anyone, and too common not to
appear in code.

`python3 -I scripts/privacy-sweep.py --self-test` runs the sweep's own checks
on invented data; it reads no library and no repo.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import zipfile
from collections.abc import Iterable, Iterator
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
ARGS_FILE = REPO / "scripts" / "privacy-sweep.args"
# Build output and caches are never pushed; a TRACKED file in one of these
# folders is (a committed .pyc once carried a path), so history reads those.
SKIP_DIRS = {"node_modules", "target", "dist", ".git", "gen", "__pycache__"}
SKIP_FILES = {"Cargo.lock", "package-lock.json", ARGS_FILE.name}
# Pictures: their words are pixels, which no text gate can read.
PICTURES = {".png", ".ico", ".icns", ".jpg", ".jpeg", ".webp", ".gif", ".bmp"}
BINARY = PICTURES | {".woff", ".woff2", ".ttf", ".bin", ".pak"}
GENERIC_OPTION = re.compile(r"(variation|variant|option|version|default|optional)\s*\d*", re.I)
NEXUS_2026 = re.compile(r"^(?P<name>.+?) (?P<mod>\d+) (?P<ver>\S+) \d{4}-\d{2}-\d{2}T\d{2}-\d{2}Z (?P<sqid>[A-Za-z0-9]+)$")
CLASSIC = re.compile(r"^(?P<name>.+?)-(?P<mod>\d{2,})-[\d-]+$")
DD2_SHAPED = re.compile(r"(^|/)(natives|reframework)/|\.pak$|(^|/)modinfo\.ini$|(^|/)dinput8\.dll$", re.I)
MIN_LEN = 4
# A pak name as an install records it: a numbered priority prefix (the stone's
# zrift-NNNN- or another tool's xNNNN-) carries nothing private.
PAK_PREFIX = re.compile(r"^(?:zrift-\d{4}|x\d{4})-", re.I)
STAND_IN_PACKAGE = re.compile(r"^[0-9a-f]{16}$")
# Folders every Windows or Steam install has, and the mods folder's own name: a
# chain made only of these is nobody's layout (a default Steam library is no secret).
STOCK_FOLDERS = {
    "users", "program files", "program files (x86)", "programdata", "steam", "steamlibrary",
    "steamapps", "common", "games", "mods", "downloads", "documents", "desktop",
    "dragonsdogma2", "dragons dogma 2", "dragon's dogma 2",
}


class Identifiers:
    def __init__(self, allow: set[str]):
        self.words: dict[str, str] = {}
        self.ids: dict[str, str] = {}
        self.places: dict[tuple[str, ...], str] = {}
        self.allow = {a.lower() for a in allow}

    def word(self, text: str, kind: str) -> None:
        text = text.strip().strip('"')
        common = text.isalpha() and len(text) <= 6
        if len(text) >= MIN_LEN and not common and text.lower() not in self.allow and not GENERIC_OPTION.fullmatch(text):
            self.words.setdefault(text, kind)

    def number(self, digits: str, kind: str) -> None:
        if len(digits) >= 3:  # one- and two-digit ids are everywhere in code
            self.ids.setdefault(digits, kind)

    def nexus_number(self, value: object, kind: str) -> None:
        """An id as JSON carries it: a number, or digits in a string (a map key)."""
        if isinstance(value, bool):
            return
        if isinstance(value, int):
            self.number(str(value), kind)
        elif isinstance(value, str) and value.isascii() and value.isdigit():
            self.number(value, kind)

    def location(self, path: str, kind: str = "library location") -> None:
        if chain := private_chain(path):
            self.places.setdefault(chain, kind)

    def name(self, file_name: str, kind: str) -> None:
        stem = re.sub(r"\.(zip|7z|rar)$", "", file_name, flags=re.I)
        self.word(stem, kind + " name")
        if m := NEXUS_2026.match(stem):
            self.word(m["name"], kind + " Nexus name")
            self.number(m["mod"], kind + " Nexus mod id")
            self.word(m["sqid"], kind + " Nexus sqid")
        elif m := CLASSIC.match(stem):
            self.word(m["name"], kind + " Nexus name")
            self.number(m["mod"], kind + " Nexus mod id")

    def modinfo(self, raw: bytes) -> None:
        text = ""
        for encoding in ("utf-8-sig", "utf-16", "cp1252"):
            try:
                text = raw.decode(encoding)
                break
            except UnicodeDecodeError:
                continue
        for line in text.splitlines():
            key, sep, value = line.partition("=")
            if not sep:
                continue
            key = key.strip().lower()
            if key in ("name", "nameasbundle", "addonfor", "author"):
                self.word(value, f"modinfo {key}")
            elif key == "homepage" and (m := re.search(r"dragonsdogma2/mods/(\d+)", value, re.I)):
                self.number(m[1], "modinfo homepage mod id")

    def journal(self, raw: str) -> bool:
        """The stone's own record of what it installed: every name and label an
        install carries, its package file, its option folder, its pak names.
        False when the text is not a journal at all."""
        try:
            record = json.loads(raw)
        except ValueError:
            return False
        if not isinstance(record, dict):
            return False
        installs = record.get("installs")
        for install in installs if isinstance(installs, list) else []:
            if not isinstance(install, dict):
                continue
            if isinstance(install.get("name"), str):
                self.word(install["name"], "install name")
            origin = install.get("origin")
            if isinstance(origin, dict):
                for key, value in origin.items():
                    if key != "kind" and isinstance(value, str):
                        self.word(value, "install label")
            if isinstance(install.get("packageFile"), str) and install["packageFile"]:
                self.name(Path(install["packageFile"].replace("\\", "/")).name, "install package")
            if isinstance(install.get("optionId"), str):
                self.option_id(install["optionId"])
            for file in install.get("files") or []:
                path = file.get("gamePath") if isinstance(file, dict) else None
                if isinstance(path, str) and path.lower().endswith(".pak"):
                    self.word(PAK_PREFIX.sub("", Path(path.replace("\\", "/")).stem), "pak name")
        pak_order = record.get("pakOrder")
        for option_id in pak_order if isinstance(pak_order, list) else []:
            if isinstance(option_id, str):
                self.option_id(option_id)
        return True

    def nexus(self, raw: str) -> bool:
        """The stone's nexus.json: each link's mod, file and group id and its
        sqid, the mod a verdict was for and the newer file it found, the mod
        names the checks brought home, and every mod whose labels it keeps.
        False when the text is not a nexus.json at all."""
        try:
            record = json.loads(raw)
        except ValueError:
            return False
        if not isinstance(record, dict):
            return False
        packages = record.get("packages")
        for entry in packages.values() if isinstance(packages, dict) else []:
            if not isinstance(entry, dict):
                continue
            link = entry.get("link")
            if isinstance(link, dict):
                self.nexus_number(link.get("modId"), "Nexus mod id")
                self.nexus_number(link.get("fileId"), "Nexus file id")
                self.nexus_number(link.get("groupId"), "Nexus group id")
                if isinstance(link.get("sqid"), str):
                    self.word(link["sqid"], "Nexus sqid")
            self.nexus_number(entry.get("stateModId"), "Nexus mod id")
            if isinstance(entry.get("modName"), str):
                self.word(entry["modName"], "Nexus mod name")
            state = entry.get("state")
            newer = state.get("newer") if isinstance(state, dict) else None
            if isinstance(newer, dict):
                self.nexus_number(newer.get("fileId"), "Nexus file id")
                if isinstance(newer.get("name"), str):
                    self.word(newer["name"], "Nexus file name")
        labels = record.get("modLabels")
        for mod_id in labels if isinstance(labels, dict) else []:
            self.nexus_number(mod_id, "Nexus mod id")
        return True

    def option_id(self, option_id: str) -> None:
        package, _, root = option_id.partition("#")
        if not STAND_IN_PACKAGE.fullmatch(package):
            self.word(root.split("#")[0], "install label")
        for segment in re.split(r"[\\/#]+", root):
            self.word(segment, "option folder")

    def patterns(self) -> list[tuple[re.Pattern[str], str]]:
        # Whole words only: a name is not a hit inside a longer word.
        words = [
            (re.compile(rf"(?<![A-Za-z0-9]){re.escape(word)}(?![A-Za-z0-9])", re.I), kind)
            for word, kind in self.words.items()
        ]
        numbers = [(re.compile(rf"(?<![\d.]){digits}(?![\d.])"), kind) for digits, kind in self.ids.items()]
        return words + numbers + [(chain_pattern(chain), kind) for chain, kind in self.places.items()]


def private_chain(path: str) -> tuple[str, ...] | None:
    """The folders that give a path's owner away: from the drive to the first
    folder no stock install has, at least two deep when the path allows it.
    `/mnt/c/<a>/<b>/Games/…` gives (<a>, <b>); `C:\\Users\\<name>\\Downloads`
    gives (Users, <name>); a path of stock folders only gives None."""
    parts = [p for p in re.split(r"[\\/]+", path.strip()) if p]
    if len(parts) >= 2 and parts[0].lower() == "mnt" and len(parts[1]) == 1:
        parts = parts[2:]
    elif parts and re.fullmatch(r"[A-Za-z]:", parts[0]):
        parts = parts[1:]
    for index, part in enumerate(parts):
        if part.lower() not in STOCK_FOLDERS and not part.startswith("<"):
            return tuple(parts[: max(index + 1, min(2, len(parts)))])
    return None


def chain_pattern(chain: tuple[str, ...]) -> re.Pattern[str]:
    """A chain with either slash (or a doubled backslash, as source code writes
    one) between its folders; a one-folder chain only right after a drive."""
    body = r"[\\/]+".join(re.escape(part) for part in chain)
    lead = r"(?:\b[A-Za-z]:|/mnt/[A-Za-z])[\\/]+" if len(chain) == 1 else r"(?<![A-Za-z0-9])"
    return re.compile(rf"{lead}{body}(?![A-Za-z0-9])", re.I)


def read_package(path: Path, found: Identifiers) -> None:
    if path.is_dir():
        for modinfo in path.rglob("*"):
            if modinfo.name.lower() == "modinfo.ini" and modinfo.is_file():
                found.modinfo(modinfo.read_bytes()[:65536])
    elif path.suffix.lower() == ".zip":
        try:
            with zipfile.ZipFile(path) as archive:
                for entry in archive.namelist():
                    if entry.replace("\\", "/").lower().endswith("modinfo.ini"):
                        found.modinfo(archive.read(entry)[:65536])
        except (zipfile.BadZipFile, OSError, RuntimeError, NotImplementedError):
            pass


def looks_like_dd2(path: Path) -> bool:
    if path.suffix.lower() != ".zip":
        return False
    try:
        with zipfile.ZipFile(path) as archive:
            return any(DD2_SHAPED.search(n.replace("\\", "/")) for n in archive.namelist())
    except (zipfile.BadZipFile, OSError, RuntimeError):
        return False


def children(folder: str) -> list[Path]:
    """A folder's direct children, or nothing (noted) when it is gone."""
    try:
        return sorted(Path(folder).iterdir())
    except OSError:
        print(f"privacy sweep: a given folder could not be listed ({kind_of(folder)}), so it added no names")
        return []


def kind_of(folder: str) -> str:
    return "its location is still swept" if private_chain(folder) else "a stock path"


def nexus_files(nexus: list[str], journals: list[str]) -> list[Path]:
    """Every --nexus, then the nexus.json beside each --journal (the stone keeps
    both in its app data folder), each once."""
    named = [Path(given) for given in nexus]
    beside = [Path(journal).with_name("nexus.json") for journal in journals]
    seen: set[str] = set()
    files = []
    for path in [*named, *(path for path in beside if path.is_file())]:
        key = os.path.normcase(os.path.abspath(path))
        if key not in seen:
            seen.add(key)
            files.append(path)
    return files


def collect(args: argparse.Namespace) -> Identifiers:
    found = Identifiers(set(args.allow))
    for given in [*args.library, *args.downloads, *args.journal, *args.nexus]:
        found.location(given)
    for given in args.private_path:
        found.location(given, "machine path")
    for word in args.private_word:
        found.word(word, "private word")
    skip = {name.lower() for name in args.skip_name}
    for library in args.library:
        for child in children(library):
            if child.name.lower() in skip or child.name.startswith("."):
                continue
            if child.is_dir() or child.suffix.lower() in (".zip", ".7z", ".rar"):
                found.name(child.name, "library")
                read_package(child, found)
    for downloads in args.downloads:
        for child in children(downloads):
            if child.is_file() and looks_like_dd2(child):
                found.name(child.name, "download")
                read_package(child, found)
    for journal in args.journal:
        try:
            raw = Path(journal).read_text(errors="replace")
        except OSError:
            print("privacy sweep: a given journal could not be read, so it added no names")
            continue
        if not found.journal(raw):
            print("privacy sweep: a given journal is not one the sweep can parse, so it added no names")
    for nexus in nexus_files(args.nexus, args.journal):
        try:
            raw = nexus.read_text(errors="replace")
        except OSError:
            print("privacy sweep: a given nexus.json could not be read, so it added no ids")
            continue
        if not found.nexus(raw):
            print("privacy sweep: a nexus.json is not one the sweep can parse, so it added no ids")
    return found


def forbidden_patterns(words: list[str]) -> list[tuple[re.Pattern[str], str]]:
    """Retired words. Five letters or more match anywhere (a retired name hides
    inside identifiers like `find_<name>`); shorter ones only as whole words,
    or embedded data would trip them by chance."""
    patterns = []
    for word in (w.strip() for w in words):
        if not word:
            continue
        body = re.escape(word)
        if len(word) < 5:
            body = rf"(?<![A-Za-z0-9]){body}(?![A-Za-z0-9])"
        patterns.append((re.compile(body, re.I), "retired word"))
    return patterns


def any_of(patterns: list[tuple[re.Pattern[str], str]]) -> re.Pattern[str]:
    """One pattern that decides whether a text holds anything at all; most hold nothing."""
    return re.compile("|".join(f"(?:{pattern.pattern})" for pattern, _ in patterns) or r"(?!)", re.I)


def swept(relative: Path) -> bool:
    """A file the sweep reads: not a lockfile, not a picture or other binary."""
    return relative.name not in SKIP_FILES and relative.suffix.lower() not in BINARY


def repo_files() -> list[Path]:
    """The working tree, pruned of build output and caches as it is walked;
    pictures included (they are counted, not read)."""
    files = []
    for folder, dirs, names in os.walk(REPO):
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
        files.extend(Path(folder, name) for name in names)
    return files


def picture_count(relatives: Iterable[Path]) -> str | None:
    """The pictures no text gate reads, counted by folder: `docs/images 3, …`."""
    by_folder: dict[str, int] = {}
    for relative in relatives:
        if relative.suffix.lower() in PICTURES:
            folder = relative.parent.as_posix()
            by_folder[folder] = by_folder.get(folder, 0) + 1
    if not by_folder:
        return None
    listing = ", ".join(f"{folder} {count}" for folder, count in sorted(by_folder.items()))
    return f"{sum(by_folder.values())} picture(s) that no text gate reads ({listing})"


def git(*args: str, stdin: bytes | None = None) -> bytes:
    return subprocess.run(["git", "-C", str(REPO), *args], input=stdin, capture_output=True, check=True).stdout


def read_blobs(shas: list[str]) -> list[str]:
    """Each blob's text, in the order asked."""
    if not shas:
        return []
    out = git("cat-file", "--batch", stdin="\n".join(shas).encode())
    texts, at = [], 0
    for _ in shas:
        header_end = out.index(b"\n", at)
        size = int(out[at:header_end].split()[2])
        texts.append(out[header_end + 1 : header_end + 1 + size].decode(errors="replace"))
        at = header_end + 1 + size + 1
    return texts


def history_files() -> list[tuple[str, str, str]]:
    """Every file version any ref reaches, once each: (blob, path, text)."""
    paths: dict[str, str] = {}
    for line in git("rev-list", "--objects", "--all").decode(errors="replace").splitlines():
        sha, _, path = line.partition(" ")
        if path and sha not in paths:
            paths[sha] = path
    if not paths:
        return []
    kinds = git("cat-file", "--batch-check=%(objectname) %(objecttype)", stdin="\n".join(paths).encode())
    blobs = [
        sha
        for sha, kind in (row.split() for row in kinds.decode().splitlines())
        if kind == "blob" and swept(Path(paths[sha]))
    ]
    return [(sha, paths[sha], text) for sha, text in zip(blobs, read_blobs(blobs))]


def tip_files() -> tuple[str, list[tuple[str, str]], str]:
    """What a push publishes as its branch's tip: every tracked file at HEAD
    that the sweep reads, as (path, text), and HEAD's own message.
    ("", [], "") before the first commit."""
    try:
        head = git("rev-parse", "--short", "--verify", "HEAD^{commit}").decode().strip()
    except subprocess.CalledProcessError:
        return "", [], ""
    entries = []
    for row in git("ls-tree", "-r", "-z", "HEAD").decode(errors="replace").split("\0"):
        meta, _, path = row.partition("\t")
        fields = meta.split()
        if len(fields) == 3 and fields[1] == "blob" and swept(Path(path)):
            entries.append((fields[2], path))
    texts = read_blobs([sha for sha, _ in entries])
    message = git("log", "-1", "--format=%B", "HEAD").decode(errors="replace")
    return head, [(path, text) for (_, path), text in zip(entries, texts)], message


def tip_hits(
    tip: list[tuple[str, str]], tree: dict[str, str], retired: list[tuple[re.Pattern[str], str]]
) -> Iterator[tuple[str, int, str]]:
    """Retired words in the tip's files, skipping each file whose text the
    working tree holds unchanged (the tree pass counted it already)."""
    anything = any_of(retired)
    for path, text in tip:
        if tree.get(path) == text:
            continue
        for number, kind in hits_in(text, retired, anything):
            yield path, number, kind


def commit_messages() -> list[tuple[str, str]]:
    raw = git("log", "--all", "--format=%h%x00%B%x01").decode(errors="replace")
    messages = []
    for record in raw.split("\x01"):
        sha, sep, body = record.lstrip("\n").partition("\x00")
        if sep:
            messages.append((sha, body))
    return messages


def first_commit_with(blob: str) -> str:
    shas = git("log", "--all", "--format=%h", f"--find-object={blob}").decode().split()
    return shas[-1] if shas else blob[:9]


def hits_in(
    text: str, patterns: list[tuple[re.Pattern[str], str]], anything: re.Pattern[str]
) -> Iterator[tuple[int, str]]:
    if not anything.search(text):
        return
    for number, line in enumerate(text.splitlines(), 1):
        if anything.search(line):
            for pattern, kind in patterns:
                if pattern.search(line):
                    yield number, kind


def self_test() -> int:
    """The sweep's own checks, on invented data only (the lab's fixture names
    and ids); no library, journal or repo is read."""
    failures: list[str] = []

    def check(what: str, held: bool) -> None:
        if not held:
            failures.append(what)

    # nexus.json: every id, sqid and name that links and verdicts carry
    found = Identifiers(set())
    store = {
        "schema": 1,
        "packages": {
            "0123456789abcdef": {
                "link": {"modId": 2207, "fileId": 2391, "groupId": 220700, "sqid": "Rk4vLmN2Q", "version": "2.4.1-beta",
                         "uploadedMs": 1760451780000, "source": "download"},
                "modName": "Vernworth Cloaks Collection",
                "state": {"kind": "updateAvailable",
                          "newer": {"fileId": 2210, "name": "Lantern Glow Recolor", "version": "2026.10.14", "uploadedMs": 1}},
                "stateModId": 2207,
            },
            "fedcba9876543210": {"unlinked": True},
        },
        "modLabels": {"7140": {"category": "Armour", "tags": ["Fashion"], "adult": False}},
    }
    check("nexus.json parses", found.nexus(json.dumps(store)))
    for digits, kind in (("2207", "Nexus mod id"), ("2391", "Nexus file id"), ("220700", "Nexus group id"),
                         ("2210", "Nexus file id"), ("7140", "Nexus mod id")):
        check(f"nexus.json gives {kind} {digits}", found.ids.get(digits) == kind)
    for text, kind in (("Rk4vLmN2Q", "Nexus sqid"), ("Vernworth Cloaks Collection", "Nexus mod name"),
                       ("Lantern Glow Recolor", "Nexus file name")):
        check(f"nexus.json gives the {kind}", found.words.get(text) == kind)
    check("a version is never an identifier", not {"2.4.1-beta", "2026.10.14"} & set(found.words))
    check("an upload time is never an identifier", "1760451780000" not in found.ids)
    check("public labels are never identifiers", "Armour" not in found.words and "Fashion" not in found.words)
    check("a damaged nexus.json says so", not Identifiers(set()).nexus('{"packages": '))
    check("a damaged journal says so", not Identifiers(set()).journal("[1, 2"))
    leaked = "see mod 2207, file 2391 (Rk4vLmN2Q)"
    check("a leaked sqid and ids are hits", len(list(hits_in(leaked, found.patterns(), any_of(found.patterns())))) == 3)

    # the tip: a retired word committed at HEAD but edited away on disk still counts
    retired = forbidden_patterns(["Quillborne", "qbm"])
    tip = [("docs/a.md", "kept\nthe Quillborne ledger\n"), ("docs/b.md", "the qbm folder\n"), ("docs/c.md", "fine\n")]
    tree = {"docs/a.md": "kept\nthe stone's journal\n", "docs/b.md": "the qbm folder\n", "docs/c.md": "fine\n"}
    check("the tip's retired word counts while the tree is clean", list(tip_hits(tip, tree, retired)) == [("docs/a.md", 2, "retired word")])
    check("a file the tree holds unchanged is counted once, by the tree pass", all(path != "docs/b.md" for path, _, _ in tip_hits(tip, tree, retired)))
    check("the tip's message is read too", list(hits_in("fix: drop the Quillborne ledger\n", retired, any_of(retired))) == [(1, "retired word")])
    check("a long retired word hides inside identifiers", any(p.search("readQuillborneLedger") for p, _ in retired))
    check("a short retired word counts only on its own", not any(p.search("offqbmap") for p, _ in retired))

    # pictures: counted by folder, never read
    count = picture_count([Path("docs/images/a.png"), Path("docs/images/b.png"), Path("src-tauri/icons/icon.ico"), Path("README.md")])
    check("pictures are counted by folder", count == "3 picture(s) that no text gate reads (docs/images 2, src-tauri/icons 1)")
    check("a picture is never read as text", not swept(Path("docs/images/a.png")))

    for failure in failures:
        print(f"privacy sweep self-test FAILED: {failure}")
    print(f"privacy sweep self-test: {'failed' if failures else 'every check held'}")
    return 1 if failures else 0


class ArgsFileParser(argparse.ArgumentParser):
    """`@file` holds one `--option value` per line; blank lines and `#` lines skip."""

    def convert_arg_line_to_args(self, arg_line: str) -> list[str]:
        line = arg_line.strip()
        if not line or line.startswith("#"):
            return []
        option, _, value = line.partition(" ")
        return [option, value.strip()] if value.strip() else [option]


def main() -> int:
    parser = ArgsFileParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter, fromfile_prefix_chars="@"
    )
    parser.add_argument("--library", action="append", default=[], help="the mods folder, or another folder of mods (repeatable)")
    parser.add_argument("--downloads", action="append", default=[], help="a downloads folder (repeatable)")
    parser.add_argument("--journal", action="append", default=[], help="the stone's journal-<key>.json (repeatable)")
    parser.add_argument(
        "--nexus", action="append", default=[], help="the stone's nexus.json (repeatable; the one beside each --journal is read too)"
    )
    parser.add_argument("--skip-name", action="append", default=[], help="a library child that is not a package (repeatable)")
    parser.add_argument(
        "--private-path", action="append", default=[], help="another machine path to keep out, such as a build folder (repeatable)"
    )
    parser.add_argument("--private-word", action="append", default=[], help="a private word no folder holds (repeatable)")
    parser.add_argument(
        "--forbid", action="append", default=[], help="a word the working tree and the tip must never carry (repeatable)"
    )
    parser.add_argument("--allow", action="append", default=["RE Framework", "REFramework", "ReShade"], help="a public identifier to skip")
    parser.add_argument("--self-test", action="store_true", help="run the sweep's own checks on invented data, then stop")
    argv = sys.argv[1:] or ([f"@{ARGS_FILE}"] if ARGS_FILE.is_file() else [])
    args = parser.parse_args(argv)
    if args.self_test:
        return self_test()
    if not (args.library or args.downloads or args.journal or args.nexus):
        parser.error(
            f"name at least one --library, --downloads, --journal or --nexus (or write them to {ARGS_FILE.relative_to(REPO)})"
        )

    found = collect(args)
    patterns = found.patterns()
    anything = any_of(patterns)
    retired = forbidden_patterns(args.forbid)
    any_retired = any_of(retired)
    hits = 0
    files = repo_files()
    tree: dict[str, str] = {}
    for path in files:
        relative = path.relative_to(REPO)
        if not swept(relative):
            continue
        try:
            text = path.read_bytes().decode(errors="replace")
        except OSError:
            continue
        tree[relative.as_posix()] = text
        for number, kind in [*hits_in(text, patterns, anything), *hits_in(text, retired, any_retired)]:
            hits += 1
            print(f"{relative}:{number}: holds a {kind}")

    try:
        versions, messages = history_files(), commit_messages()
    except (OSError, subprocess.CalledProcessError):
        versions, messages = [], []
        print("privacy sweep: not a git checkout here, so the history and the commit messages were not read")
    try:
        tip, tip_versions, tip_message = tip_files() if retired else ("", [], "")
    except (OSError, subprocess.CalledProcessError):
        tip, tip_versions, tip_message = "", [], ""
        print("privacy sweep: the tip could not be read, so the retired words were checked in the working tree only")
    for blob, path, text in versions:
        for number, kind in hits_in(text, patterns, anything):
            hits += 1
            print(f"history {first_commit_with(blob)} {path}:{number}: holds a {kind}")
    for sha, body in messages:
        for number, kind in hits_in(body, patterns, anything):
            hits += 1
            print(f"message {sha}:{number}: holds a {kind}")
    for path, number, kind in tip_hits(tip_versions, tree, retired):
        hits += 1
        print(f"tip {tip} {path}:{number}: holds a {kind}")
    for number, kind in hits_in(tip_message, retired, any_retired):
        hits += 1
        print(f"tip message {tip}:{number}: holds a {kind}")

    if pictures := picture_count(path.relative_to(REPO) for path in files):
        print(f"privacy sweep: {pictures}: look at every one that shows the window before a push")
    where = f"in the working tree and at the tip {tip}" if tip else "in the working tree only"
    print(
        f"privacy sweep: {len(found.words)} names, {len(found.ids)} ids and {len(found.places)} locations from the given sources; "
        f"{len(retired)} retired word(s) checked {where}; "
        f"{len(tree)} files, {len(versions)} file versions in history and {len(messages)} commit messages read; {hits} hit(s)"
    )
    return 1 if hits else 0


if __name__ == "__main__":
    sys.exit(main())
