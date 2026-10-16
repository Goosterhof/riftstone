#!/usr/bin/env python3
"""The privacy sweep — run before every push.

A player's mod library is their own business, and this repo is public. The
sweep collects every identifier a real library holds — archive and folder
names, the Nexus name shapes inside them (name, mod id, sqid), Fluffy's
installed.ini sections, mod names and pak names, every modinfo.ini name,
bundle, addonfor parent, author and homepage mod id — READ-ONLY, and greps the
repo for each. It prints where a hit is, never the identifier itself.

    python3 -I scripts/privacy-sweep.py \\
        --library  "/mnt/c/<fluffy>/Games/DragonsDogma2/Mods" \\
        --library  "/mnt/c/<fluffy>/Games/DragonsDogma2" \\
        --downloads "/mnt/c/Users/<you>/Downloads" \\
        --fluffy-ini "/mnt/c/<fluffy>/Games/DragonsDogma2/installed.ini"

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
import re
import sys
import zipfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
SKIP_DIRS = {"node_modules", "target", "dist", ".git", "gen"}
SKIP_FILES = {"Cargo.lock", "package-lock.json"}
BINARY = {".png", ".ico", ".icns", ".jpg", ".jpeg", ".webp", ".woff", ".woff2", ".ttf", ".bin", ".pak"}
GENERIC_OPTION = re.compile(r"(variation|variant|option|version|default|optional)\s*\d*", re.I)
NEXUS_2026 = re.compile(r"^(?P<name>.+?) (?P<mod>\d+) (?P<ver>\S+) \d{4}-\d{2}-\d{2}T\d{2}-\d{2}Z (?P<sqid>[A-Za-z0-9]+)$")
CLASSIC = re.compile(r"^(?P<name>.+?)-(?P<mod>\d{2,})-[\d-]+$")
DD2_SHAPED = re.compile(r"(^|/)(natives|reframework)/|\.pak$|(^|/)modinfo\.ini$|(^|/)dinput8\.dll$", re.I)
MIN_LEN = 4
# Fluffy's own folders beside a game's Mods folder: not mods, never identifiers.
FLUFFY_OWN = {"mods", "logo", "presets", "wallpapers"}


class Identifiers:
    def __init__(self, allow: set[str]):
        self.words: dict[str, str] = {}
        self.ids: dict[str, str] = {}
        self.allow = {a.lower() for a in allow}

    def word(self, text: str, kind: str) -> None:
        text = text.strip().strip('"')
        common = text.isalpha() and len(text) <= 6
        if len(text) >= MIN_LEN and not common and text.lower() not in self.allow and not GENERIC_OPTION.fullmatch(text):
            self.words.setdefault(text, kind)

    def number(self, digits: str, kind: str) -> None:
        if len(digits) >= 3:  # one- and two-digit ids are everywhere in code
            self.ids.setdefault(digits, kind)

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


def collect(args: argparse.Namespace) -> Identifiers:
    found = Identifiers(set(args.allow))
    for library in args.library:
        for child in sorted(Path(library).iterdir()):
            if child.name.lower() in FLUFFY_OWN or child.name.startswith("."):
                continue
            if child.is_dir() or child.suffix.lower() in (".zip", ".7z", ".rar"):
                found.name(child.name, "library")
                read_package(child, found)
    for downloads in args.downloads:
        for child in sorted(Path(downloads).iterdir()):
            if child.is_file() and looks_like_dd2(child):
                found.name(child.name, "download")
                read_package(child, found)
    for ini in args.fluffy_ini:
        for line in Path(ini).read_text(errors="replace").splitlines():
            line = line.strip()
            if line.startswith("[") and line.endswith("]"):
                found.word(line[1:-1], "Fluffy section")
                continue
            key, sep, value = line.partition("=")
            if not sep:
                continue
            key = key.strip().lower()
            if key == "modname":
                found.word(value, "Fluffy mod name")
            elif key.startswith("file") and value.strip().lower().endswith(".pak"):
                stem = re.sub(r"^x\d{4}-", "", Path(value.strip().replace("\\", "/")).stem)
                found.word(stem, "Fluffy pak name")
    return found


def repo_files() -> list[Path]:
    files = []
    for path in REPO.rglob("*"):
        relative = path.relative_to(REPO)
        if (
            path.is_file()
            and not (set(relative.parts) & SKIP_DIRS)
            and path.name not in SKIP_FILES
            and path.suffix.lower() not in BINARY
        ):
            files.append(path)
    return files


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--library", action="append", default=[], help="a library root (repeatable)")
    parser.add_argument("--downloads", action="append", default=[], help="a downloads folder (repeatable)")
    parser.add_argument("--fluffy-ini", action="append", default=[], help="Fluffy's installed.ini (repeatable)")
    parser.add_argument("--allow", action="append", default=["RE Framework", "REFramework", "ReShade"], help="a public identifier to skip")
    args = parser.parse_args()
    if not (args.library or args.downloads or args.fluffy_ini):
        parser.error("name at least one --library, --downloads or --fluffy-ini")

    found = collect(args)
    # Whole words only: a name is not a hit inside a longer word.
    words = [
        (re.compile(rf"(?<![A-Za-z0-9]){re.escape(word)}(?![A-Za-z0-9])", re.I), kind)
        for word, kind in found.words.items()
    ]
    numbers = [(re.compile(rf"(?<![\d.]){digits}(?![\d.])"), kind) for digits, kind in found.ids.items()]
    hits = 0
    for path in repo_files():
        try:
            text = path.read_text(errors="replace")
        except OSError:
            continue
        for number, line in enumerate(text.splitlines(), 1):
            for pattern, kind in words + numbers:
                if pattern.search(line):
                    hits += 1
                    print(f"{path.relative_to(REPO)}:{number}: holds a {kind}")
    print(
        f"privacy sweep: {len(found.words)} names and {len(found.ids)} ids from the library, "
        f"{len(repo_files())} files read, {hits} hit(s)"
    )
    return 1 if hits else 0


if __name__ == "__main__":
    sys.exit(main())
