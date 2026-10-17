#!/usr/bin/env python3
"""The privacy sweep — run before every push.

A player's mod library is their own business, and this repo is public. The
sweep collects every identifier a real library holds — archive and folder
names, the Nexus name shapes inside them (name, mod id, sqid), every
modinfo.ini name, bundle, addonfor parent, author and homepage mod id, and
what the Riftstone's own journal remembers (install names and labels, package
files, option folders, pak names) — READ-ONLY, and greps the repo for each. It
prints where a hit is, never the identifier itself.

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
        --downloads "/mnt/c/Users/<you>/Downloads"

With no arguments it reads them from scripts/privacy-sweep.args, which git
ignores, so the real paths stay on the machine. One option per line, `#` for a
comment:

    --library /mnt/c/<install dir>/mods
    --journal /mnt/c/Users/<you>/AppData/Roaming/nl.zmuuzn.riftstone/journal-<key>.json
    --downloads /mnt/c/Users/<you>/Downloads
    --private-path C:\\<builds>\\riftstone-releases
    --private-word <your Nexus account name>
    --forbid <a word the tree must never carry>

`--library` is the mods folder (any other folder of mods counts too); every
archive and folder directly inside it is a package, except the names given with
`--skip-name` (a folder's own non-mod furniture). A library or downloads path
that is gone is noted and skipped, its location still swept. `--private-word`
holds a private word no folder holds, such as the Nexus account name.
`--forbid` names a word the working tree must never carry again (a retired
name): it is checked in the tree only, since the history keeps what it kept,
and it prints the place, never the word.

Exit 0 when the repo holds none of them, 1 when it does (each hit listed as
file:line and the identifier's kind), 2 on bad arguments. Zip archives are
read for their modinfo.ini files; 7z and rar archives contribute their names
only. Names in Downloads count only for archives that look like Dragon's
Dogma 2 mods.

`--allow` takes identifiers that are public on their own (a tool's name such
as REFramework or ReShade, which search_terms keeps whole); they are skipped. So are single words of six letters or
fewer (`Extras`, `Tops`): too common to identify anyone, and too common not to
appear in code.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import zipfile
from collections.abc import Iterator
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
ARGS_FILE = REPO / "scripts" / "privacy-sweep.args"
# Build output and caches are never pushed; a TRACKED file in one of these
# folders is (a committed .pyc once carried a path), so history reads those.
SKIP_DIRS = {"node_modules", "target", "dist", ".git", "gen", "__pycache__"}
SKIP_FILES = {"Cargo.lock", "package-lock.json", ARGS_FILE.name}
BINARY = {".png", ".ico", ".icns", ".jpg", ".jpeg", ".webp", ".woff", ".woff2", ".ttf", ".bin", ".pak"}
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

    def journal(self, raw: str) -> None:
        """The stone's own record of what it installed: every name and label an
        install carries, its package file, its option folder, its pak names."""
        try:
            record = json.loads(raw)
        except ValueError:
            return
        installs = record.get("installs") if isinstance(record, dict) else None
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
        for option_id in record.get("pakOrder") or [] if isinstance(record, dict) else []:
            if isinstance(option_id, str):
                self.option_id(option_id)

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


def collect(args: argparse.Namespace) -> Identifiers:
    found = Identifiers(set(args.allow))
    for given in [*args.library, *args.downloads, *args.journal]:
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
            found.journal(Path(journal).read_text(errors="replace"))
        except OSError:
            print("privacy sweep: a given journal could not be read, so it added no names")
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


def swept(relative: Path) -> bool:
    """A file the sweep reads: not a lockfile, not an image."""
    return relative.name not in SKIP_FILES and relative.suffix.lower() not in BINARY


def repo_files() -> list[Path]:
    """The working tree, pruned of build output and caches as it is walked."""
    files = []
    for folder, dirs, names in os.walk(REPO):
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
        files.extend(Path(folder, name) for name in names if swept(Path(folder, name).relative_to(REPO)))
    return files


def git(*args: str, stdin: bytes | None = None) -> bytes:
    return subprocess.run(["git", "-C", str(REPO), *args], input=stdin, capture_output=True, check=True).stdout


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
    out = git("cat-file", "--batch", stdin="\n".join(blobs).encode())
    versions, at = [], 0
    for sha in blobs:
        header_end = out.index(b"\n", at)
        size = int(out[at:header_end].split()[2])
        body = out[header_end + 1 : header_end + 1 + size]
        at = header_end + 1 + size + 1
        versions.append((sha, paths[sha], body.decode(errors="replace")))
    return versions


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
    text: str, patterns: list[tuple[re.Pattern[str], str]], any_of: re.Pattern[str]
) -> Iterator[tuple[int, str]]:
    if not any_of.search(text):
        return
    for number, line in enumerate(text.splitlines(), 1):
        if any_of.search(line):
            for pattern, kind in patterns:
                if pattern.search(line):
                    yield number, kind


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
    parser.add_argument("--skip-name", action="append", default=[], help="a library child that is not a package (repeatable)")
    parser.add_argument(
        "--private-path", action="append", default=[], help="another machine path to keep out, such as a build folder (repeatable)"
    )
    parser.add_argument("--private-word", action="append", default=[], help="a private word no folder holds (repeatable)")
    parser.add_argument("--forbid", action="append", default=[], help="a word the working tree must never carry (repeatable)")
    parser.add_argument("--allow", action="append", default=["RE Framework", "REFramework", "ReShade"], help="a public identifier to skip")
    argv = sys.argv[1:] or ([f"@{ARGS_FILE}"] if ARGS_FILE.is_file() else [])
    args = parser.parse_args(argv)
    if not (args.library or args.downloads or args.journal):
        parser.error(f"name at least one --library, --downloads or --journal (or write them to {ARGS_FILE.relative_to(REPO)})")

    found = collect(args)
    patterns = found.patterns()
    # One pass decides whether a text holds anything at all; most hold nothing.
    any_of = re.compile("|".join(f"(?:{pattern.pattern})" for pattern, _ in patterns) or r"(?!)", re.I)
    retired = forbidden_patterns(args.forbid)
    any_retired = re.compile("|".join(f"(?:{pattern.pattern})" for pattern, _ in retired) or r"(?!)", re.I)
    hits = 0
    files = repo_files()
    for path in files:
        try:
            text = path.read_text(errors="replace")
        except OSError:
            continue
        for number, kind in [*hits_in(text, patterns, any_of), *hits_in(text, retired, any_retired)]:
            hits += 1
            print(f"{path.relative_to(REPO)}:{number}: holds a {kind}")

    try:
        versions, messages = history_files(), commit_messages()
    except (OSError, subprocess.CalledProcessError):
        versions, messages = [], []
        print("privacy sweep: not a git checkout here, so the history and the commit messages were not read")
    for blob, path, text in versions:
        for number, kind in hits_in(text, patterns, any_of):
            hits += 1
            print(f"history {first_commit_with(blob)} {path}:{number}: holds a {kind}")
    for sha, body in messages:
        for number, kind in hits_in(body, patterns, any_of):
            hits += 1
            print(f"message {sha}:{number}: holds a {kind}")

    print(
        f"privacy sweep: {len(found.words)} names, {len(found.ids)} ids and {len(found.places)} locations from the library; "
        f"{len(retired)} retired word(s) checked in the tree; "
        f"{len(files)} files, {len(versions)} file versions in history and {len(messages)} commit messages read; {hits} hit(s)"
    )
    return 1 if hits else 0


if __name__ == "__main__":
    sys.exit(main())
