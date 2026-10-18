# CLAUDE.md — The Riftstone

*The lab journal for `gadgets/riftstone/`. Read this before picking up the scalpel.*

## What This Is

**The Riftstone** is a Tauri v2 Windows desktop mod manager for **Dragon's Dogma 2**. It reads
every mod in its one library, the `mods` folder beside the installed `riftstone.exe`. A mod is a
zip, 7z or rar or a plain folder, nested as deep as the modder packed it. The stone opens on a
classic tree of those packages, and a search reaches all of it at once: option names,
descriptions, the menu trail a variant hides behind, and every file path inside the archives. It
installs and uninstalls the way the 2026 engine expects (`pak_mods/`, loose `natives/`, REFramework
scripts, root DLLs) under a write-ahead journal that remembers every byte it moved, and a Play
button starts the game. It asks Nexus Mods, anonymously, whether a newer file of each download
exists, and brings each linked mod's category and tags home to filter the search. With a personal
Nexus key it also shows the account, downloads files straight into the mods folder (premium in the
stone; free accounts through the site's Mod Manager Download button, once the player turns the
stone's nxm handler on), and tracks and endorses mods.

Since v0.4, **THE MUSTER** shows the party of any save slot, read-only, from the player's own saves:
the Arisen, the main pawn and the hired pawns, each with level, vocation, the experience in every
vocation, the stats the levels gave, everything worn by slot, the augments and skills set for the
vocation in use, a pawn's inclination and specialization, and the Arisen's gold and Rift
Crystals. It decrypts each slot in memory (keyed by the Steam account the save folder belongs
to) and names what the party carries from the game's own text, read from the player's own
`re_chunk_000.pak` at run time and never bundled. When the game saves, the page reads the slot
again and marks what changed (THE RE-CARVING, #00015 R17.25).

The metaphor (design system #00015, *Rift-Lit Basalt*, ruled 2026-10-16, amended for v0.3 on
2026-10-17): DD2's Riftstone is the monolith you touch to summon the pawns of other Arisen. Mods are
creations from other worlds; the Rift holds them; installing pulls one through into your world.
Core nouns stay plain (mod, option, install, update, load order, play, download) — *generic gives
meaning faster* — and the voice lives in the frame, the states and the two signature interactions.

- **Experiment log:** `zmuuzn/documents/experiment-logs/00068-the-riftstone.md`
- **Design system:** `zmuuzn/documents/design-systems/00015-system-riftstone.md` (A · The Monolith; §16 is v0.3, §17 the Muster); audition `docs/design/audition.html`; tokens `docs/design/tokens.uno.ts`
- **Build spec (the contract every crate was built to):** `docs/BUILD-SPEC.md` (§ v0.3 for this version)
- **Window contract (every command, event, shape):** `docs/WINDOW-CONTRACT.md` + `src/types/rift.ts`

## The Architecture Contract (RS-1 … RS-7)

- **RS-1 — Rust does everything.** Reading archives, classifying options, hashing, deploying, launching, Nexus traffic, downloads.
- **RS-2 — The mods folder is the player's.** The stone never changes, renames or deletes a file in it. It adds exactly one kind of file: a download the player asked for, written under a hidden partial name (`.riftstone-<id>.part`, which the stone removes again when the download fails or is cancelled, and at the next launch) and renamed into place only when whole and checked, never over an existing file.
- **RS-3 — Vanilla is sacred.** `rift_core::dd2::NEVER_WRITE` (`DD2.exe`, `re_chunk_000.pak*`, `dlc/`, `_storage_/`, `steam_api64.dll` …) is refused outright; any other existing file is backed up before it is overwritten and restored on uninstall.
- **RS-4 — Write-ahead journal.** Intent saved before the first byte moves, committed after the last; a crash mid-install is rolled back at the next launch. Schema 2 since v0.3; an older journal is lifted in memory and saved as schema 2 at the next change.
- **RS-5 — One folder.** The `mods` folder beside `riftstone.exe` is the only library (a bench may point `RIFTSTONE_MODS_DIR` elsewhere). Every archive and folder directly inside it is one package; nothing else is scanned. Folder packages are known by their name, so moving the whole install keeps every link.
- **RS-6 — Nexus is optional.** Update checks, labels, Find, browsing and a mod's files use the anonymous GraphQL v2 API and need no key. The optional personal API key adds the key's extras through v1: the account, downloads, tracking and endorsing. It lives in Windows Credential Manager, never in a file or the webview; a download URL and an nxm link's key are secrets like it (never logged, never in an error, never in the window). Every request names the Riftstone (`Application-Name`, `Application-Version`), and v1's quota headers are honoured.
- **RS-7 — Vue 3 + UnoCSS**, `@unocss/reset` before `virtual:uno.css`, the `prefers-reduced-motion` floor.
- **RS-8 — The saves are the player's (v0.4).** A save is read into memory and nothing else: the stone never writes, renames, re-signs or copies a save, never sends one, and keeps no decrypted copy on disk. It carries no way to encrypt a save: the test fixtures are sealed by code that only test builds compile (`rift-save`'s and `rift-muster`'s `fixtures` features). The game's pak is read the same way, read-only (RS-3 still forbids writing it). A save folder's path holds the Steam account number, so no log line, error or event names it.

## The DD2 Load Rules (researched 2026-10-16; constants in `crates/rift-core/src/dd2.rs`)

- REFramework's `IntegrityCheckBypass_LoadPakDirectory` loads `pak_mods/` in **file-name
  order** (NTFS = alphabetical) into `re_chunk_000.pak.sub_000.pak.patch_NNN` slots — the
  last name wins. Confirmed in the investor's own `re2_framework_log.txt`. The `.pak`
  extension check is case-sensitive.
- The Riftstone names its paks `zrift-NNNN-<stem>.pak`. `z` sorts after the numbered prefixes
  other tools use (`xNNNN-`), so a new install wins over anything already in `pak_mods/`. Adopted
  paks keep their old name until the first reorder renames them. `pak_mods/` stays flat.
- LooseFileLoader serves any `natives/` path that exists on disk ahead of every pak.
- Both lists are read at game start: an install while DD2 runs loads at the next launch.
- TU 3.2 (2026-09-01/02) changed asset suffixes (`.mesh.240423143` → `.mesh.260421070`,
  `.tex.760230703` → `.tex.251211553` …); loose files with old suffixes are flagged.
- More than 100 paks in `pak_mods/` → a warning (REFramework's slot table has no bounds check).
- Play opens `steam://rungameid/2054970` when the game was found through Steam (Steam starts if
  needed and applies the player's launch options); otherwise it starts `DD2.exe` in the game folder.
  REFramework's `dinput8.dll` loads from the game folder either way.

## Workspace

```
crates/rift-core      the contract: model, path discipline + zip-slip guard, dd2 constants, errors, Nexus file names
crates/rift-archive   zip / 7z / rar (UnRAR) / folder readers behind rift_core::PackageSource
crates/rift-pak       RE Engine KPKA table of contents + the engine's murmur3 path hash; v0.4: the game's own pak (its encrypted table, chunked/zstd/deflate entries) and its GMSG text files
crates/rift-save      v0.4: a save file, read-only: the DSSS container, its Mandarin cipher, the self-describing class stream
crates/rift-muster    v0.4: the party in a save, the save folders and slot files, and the Codex (the game's names, by id)
crates/rift-catalog   modinfo.ini, option discovery, the addon tree and its nesting, search, conflicts, the scan cache
crates/rift-deploy    game/REFramework discovery, Play's plan, the journal (schema 2) and its relink, install/uninstall/reorder, preflight
crates/rift-nexus     anonymous Nexus v2 GraphQL (updates, labels, Find, browse, a mod's files), v1 with the key (account, downloads, tracking, endorsing), the downloader, nxm links
src-tauri/            the spine: state, persistence, commands, events, thumbnails, the mods-folder watcher, the game watcher, downloads, the nxm handler (and windows/hooks.nsh, the uninstaller's give-back); v0.4: muster.rs (slots, parties, the codex cache, the save folders' watchers)
src/                  the window (Vue 3 + UnoCSS) — services/ is the only door to Tauri; services/mock/ runs it in a browser
scripts/              privacy-sweep.py — the pre-push gate below; dd2_saves.py, the save reader it loads (v0.4)
```

## Containment Protocols (the gates on disk)

```sh
# Rust (workspace root)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Frontend
npm run typecheck   # vue-tsc, strict
npm run lint        # oxlint --type-aware (war-room canonical config; src/services/mock/** relaxed — dev tooling)
npm test            # vitest (the stands-alone spec, tests/standsAlone.spec.ts, reads src/, tests/, docs/, README, CLAUDE.md, scripts/ and every .rs file for the forbidden words, never spelling them)
npm run format:check
npm run build

# The privacy sweep's own checks (invented data; reads no library)
python3 -I scripts/privacy-sweep.py --self-test
```

**The Sentinel** (`.github/workflows/sentinel.yml`, armed with the first push on 2026-10-18) runs
those gates on every push to `main` and every pull request against it, in three jobs:
- *Window*: `npm ci`, then typecheck, lint, test, format:check and build.
- *Crates*: fmt, clippy with warnings denied, and the workspace's tests. It is pinned to Rust
  1.98.0, the toolchain both benches verified with, because a newer stable's lints can turn
  `-D warnings` red overnight. Moving the pin is a deliberate commit, run on both benches first.
- *Privacy*: the sweep's `--self-test`.

CI never sees the library, the journal, the saves or the game, so the full sweep stays a bench
step before each push. `main` is protected behind the three checks: changes land by pull request,
merged with merge commits only (the lab's Decision 025).

`#[cfg(windows)]` code (the registry for Steam and the nxm handler, the process list, Play's
`CreateProcessW`, a download's no-overwrite `MoveFileExW` landing, and a few file-system checks)
never compiles on the Linux bench: build, test and lint it natively on Windows before a release.
The NSIS uninstall hook (`src-tauri/windows/hooks.nsh`) is built only by `npm run tauri build`.
v0.3's native runs (the tests on 2026-10-17 and again on 2026-10-18, with the installer's first
build, and once more for 0.3.1) and what is still owed on Windows are recorded in BUILD-SPEC
§ v0.3 § The first, § The second and § The third native Windows run.

Bench suites (`#[ignore]`, env-gated, READ-ONLY against the investor's real data) — see each
crate's `tests/bench_real.rs` (rift-pak's is `tests/bench_reference_hashes.rs`). They never
install, uninstall, download, launch or flip a switch, and they print counts only. rift-nexus's
live name-search bench
(`RIFT_BENCH_NEXUS_FIND=1 cargo test -p rift-nexus --test bench_real -- --ignored --nocapture`)
sends only its two fixed phrases (`lantern glow`, `armor`), never a name from the library.

## Privacy — the library is the investor's own

This repo is public, and the investor's mod library is private. **No real mod name, file name,
option name, author, install label, Nexus mod id, file id, sqid or Nexus account name from it ever
enters this repo**: not in fixtures, tests, docs, comments or bench output. Fixtures use invented
names (*Vernworth Cloaks Collection*, *Lantern Glow Recolor*, mod `2207`, sqid `Rk4vLmN2Q`, account
*Arisen of Vernworth*). A bench that needs a real identifier takes it from the environment (the
live Nexus bench reads `RIFT_BENCH_NEXUS_MOD`). The investor's install folder is private too: docs
write `<install dir>\mods`. **Before every push**, run the sweep. It reads the library, the
stone's own journal and its `nexus.json` read-only and greps the working tree, every file version
in the history and every commit message for each identifier it holds, and for each private folder
chain. It prints locations, never names:

```sh
python3 -I scripts/privacy-sweep.py   # reads scripts/privacy-sweep.args, which git ignores
```

The args file keeps this machine's real paths off the public tree, one option per line:

```
--library /mnt/c/<install dir>/mods
--journal /mnt/c/Users/<you>/AppData/Roaming/nl.zmuuzn.riftstone/journal-<key>.json
--nexus /mnt/c/Users/<you>/AppData/Roaming/nl.zmuuzn.riftstone/nexus.json
--downloads /mnt/c/Users/<you>/Downloads
--private-path C:\<builds>\riftstone-releases
--private-word <the Nexus account name>
--forbid <a word the repo must never carry>
--saves /mnt/c/Program Files (x86)/Steam/userdata/<account>/2054970/remote/win64_save
```

`--saves` (v0.4) names a Dragon's Dogma 2 save folder. The sweep decrypts each save read-only
(`scripts/dd2_saves.py`) and collects what a person chose: the Arisen's and the pawns' names and
nicknames, the hired pawns' creators' names, and the pawns' Rift search ids. It also collects
the Steam account number the folder's path holds, that account's SteamID64, and the folder's own
chain. A character's name counts even when it is a short word, which the library's words do not;
a short name matches in its own case only, because the same letters in another case are a code
word far more often than anyone's name. A name the repo already held as a word before it read a
save (`BEFORE_SAVES`, 830fdf0) is set aside and counted: a hit on it could not be told from the
repo's own text. Two of the game's record types carry a `Name` that is the game's own id; their
strings never count (2,767 of them on the bench's eight saves).

`--nexus` names the stone's `nexus.json`, where every link keeps its mod id, file id, group id
and sqid and every check keeps the mod's Nexus name and the newer file it found; the sweep also
reads the `nexus.json` beside each `--journal`, so a missing line never drops those ids.
`--private-path` names any other folder of this machine's that must not appear, such as where
builds land; the sweep reads it as a folder chain, with either slash. `--skip-name` names a
library child that is not a package. `--forbid` holds words the repo must never carry again,
printed as places, never as words: the args file is where those words live, so the repo never
has to spell them. A word of five letters or more matches anywhere, inside identifiers too; a
shorter one matches only as a whole word. The sweep never reads the base64 payload of a `data:`
URI (the fonts embedded in `docs/design/audition.html`): what such data spells is chance, not a
name or an id. A forbidden word is checked in the working tree and at the tip a push
publishes (every tracked file at HEAD whose text differs from the working tree, and HEAD's own
message), so a clean-up that is not committed yet never passes for a clean push. The older
history keeps what it kept (BUILD-SPEC § v0.3, ruling 1), and a commit between the last push
and the tip is not read for them either: a word committed and taken out again before one push
still goes out in that push's history. So run the sweep before each commit you will push, and
once more before the push, with the commit you push checked out. The committed gate, the
stands-alone spec (`tests/standsAlone.spec.ts`), guards the sources, specs, docs, scripts and
Rust on every clone at each `npm test`, in the working tree only.

**Pictures are the sweeps' blind spot.** Every gate here reads text, and a screenshot carries
whatever the window drew. The sweep counts the pictures in the tree by folder on every run.
README frames are shot from the mock only (`npm run dev`, invented names), shot again whenever
the window changes what they show, and looked at, one by one, before a push.

*(Written 2026-10-16, after the first sweep found a real mod id in 30 places in rift-nexus's
tests and a real option name in BUILD-SPEC, both from before the scrub. Reworked 2026-10-17 for
v0.3: install names now come from the stone's own journal, and the Nexus ids from its
`nexus.json`. The tip check and the picture count followed that night, after a review found
a forbidden word still committed at HEAD in three files while the sweep, reading only the
uncommitted working tree, reported none, and the README's frames still showing one in their
pixels. On 2026-10-18 the committed spec began reading the docs, the scripts and the Rust too.
The same day the reworked sweep's first full run, the first to read the journal's and
`nexus.json`'s ids, found one more leak: a real Nexus file id in an example URL since v0.1, beside
the invented mod id that had replaced its real one in the first scrub. The history was
rewritten once more before the first push, so every commit carries the invented file 2391 in
its place.)*

## Running It

The Muster's mock scenes: `?mock=muster`, `musternosteam`, `musternosaves`, `musternames` (the
names missing), `musterslow` (a read that takes 2 s) and `musterwriting` (a slot the game is writing
once). In `npm run dev`, `window.riftMock.gameSaved()` saves Slot 9 a level later, for THE
RE-CARVING. On the bench, `RIFTSTONE_SAVE_DIR` points the stone at one save folder (a COPY of
one, never the real folder: a bench run must not race the game), with the account read from the
folder three levels up, or from `RIFTSTONE_SAVE_ACCOUNT`.

```sh
npm run dev            # the window in a browser on the mock backend (?mock=empty|emptymods|nofloor|gonemods|nogame|scanning|refoff|patched|offline|fresh|firstrun|key|premium|rejected|quotalow|running|nxmother|nocreations, comma-separated)
npm run tauri dev      # the real app (WSL bench: set RIFTSTONE_GAME_ROOT / RIFTSTONE_MODS_DIR / RIFTSTONE_DATA_HOME)
npm run tauri build    # on Windows: target/release/riftstone.exe (+ an unsigned NSIS installer)
```

`RIFTSTONE_MODS_DIR` moves the library; without it the stone reads `<folder of riftstone.exe>\mods`
(under `npm run tauri dev` that is `target/debug/mods`). Never point a bench at the investor's real
mods folder with a download in mind: RS-2 lets the stone add files there. The mock's `emptymods`
scene empties the folder after the installs were made, `nofloor` gives a mods folder that could
not be made at launch (the spine's Error, THE RIFT HAS NO FLOOR), and `gonemods` one that went
while the stone ran (the Gone Warn, THE RIFT LOST ITS FLOOR; its Open mods folder makes the
folder again, empty, and scans). `nxmother` has another program answer Nexus's links,
`Wagonmaster.exe` (#00015 R16.24), so the nxm switch shows that program's line.

No release pipeline and no updater: the investor ruled *"i do not really need a signed
release for this"*. Build on the Windows side and run the installer. Installing a new version over
an old one, or uninstalling, leaves the `mods` folder alone: Tauri's NSIS uninstaller deletes only
the files it installed and removes the install folder only when it is empty. Its "Delete the
application data" box (unticked by default; ignored on an upgrade) removes
`%APPDATA%\nl.zmuuzn.riftstone` and `%LOCALAPPDATA%\nl.zmuuzn.riftstone` whole: the journal and its
backups, `nexus.json`, the settings, caches and logs. Uninstall mods from the stone before ticking
it, or their files stay in the game folder with no journal to take them out.

Before it removes a file, the uninstaller gives the Mod Manager Download links back:
`NSIS_HOOK_PREUNINSTALL` in `src-tauri/windows/hooks.nsh` (wired in through `tauri.conf.json`'s
`installerHooks`) runs `riftstone.exe --give-back-nxm`, which does what the nxm switch's off does
(the user's registry key goes back as the stone found it, WINDOW-CONTRACT § The nxm handler)
and, where it cannot put the key back while it still starts this exe, removes the key whole,
because the exe is about to go. The hook is skipped on an updater's `/UPDATE` (there is no
updater). An upgrade, whose installer uninstalls the old version first by default, gives the
links back too: turn the switch on again after it (WINDOW-CONTRACT § v0.3 The one folder).

**Before a release**, in this order:
1. Every gate above, on the Linux bench.
2. The Rust gates natively on Windows: `cargo test --workspace` and `cargo clippy` (the
   `#[cfg(windows)]` code compiles nowhere else), then `npm run tauri build` (the only build of
   the uninstall hook).
3. The presses no test makes, on a Windows desktop (Known Limitations).
4. The README frames, when the window changed what they show: shot again from the mock
   (`npm run dev`, invented names only), then each one looked at.
5. The privacy sweep, last, on the commit that is pushed.

## Known Limitations

- Downloads in the stone need a premium account; a free account goes through the file's page and
  the site's Mod Manager Download button, which reaches the stone only while its nxm handler is on.
  Vortex, when installed with "Handle Nexus Links" on, takes the handler back at each of its starts.
  An upgrade of the stone gives the handler back (by default its installer runs the old
  uninstaller first, whose hook gives the links back): turn the switch on again after one.
- A download is not resumed across a restart: its partial file is removed at the next launch.
- A Nexus file that is not a zip, 7z or rar (a raw `.pak`, an `.exe`) lands in the mods folder under
  its own name, but the scan never lists it: only archives and folders are packages. It gets no
  link and no scan, and the window offers Show in folder instead of Show in the Rift.
- A relink reads at most 4 GiB of candidate entries per scan (their listed sizes): installs
  whose checks do not fit show `packageMissing` until a later scan (each launch or mods-folder
  change) confirms them, and one check over 4 GiB alone is never read, so that install keeps its
  id. A solid 7z or RAR may decode more than the listed sizes to reach the entries.
- An archive overwritten in place by another version with the same file name and the same size
  keeps its package id (name + size), so no relink is proposed: the install shows on the new
  archive's option while the game folder holds the old bytes.
- A relink confirms each file by the engine's fingerprint (its size and its first and last MiB),
  not by every byte. A newer version of the same size whose changes all lie between the first and
  last MiB of a file over 2 MiB still takes over the older install, and the Rift then shows the
  newer option installed while the game folder holds the older bytes.
- Nexus labels reach linked packages only. MD5 identify cannot link an unpacked folder (there is no
  archive to hash) or a repack (its bytes are new). Find on Nexus searches those by name (the
  package's, else its first option's bundle or modinfo name) and offers up to five candidates;
  the player links. A name unlike the mod's Nexus name finds the wrong mods or none, and a link
  by mod id stays unpinned (labels yes, "is my copy the newest" no) until the file's URL is pasted.
- The veil's `adult` mode also blurs packages Nexus has not labelled yet, on purpose (a veil protects).
- Windows-only (discovery reads the Windows registry; env overrides serve the WSL bench).
- The Windows-only code compiles and its tests pass natively on Windows. On 2026-10-18
  `cargo test --workspace` ran there (MSVC, Rust 1.98.0, `RUSTFLAGS=-D warnings`) at 31d26bf:
  861 passed, 0 failed. So every `#[cfg(windows)]` line up to 80b1ca8 compiles there without a
  warning, including `mod win`'s `user_key`, `restore_user`, `raw_value`, `put_value` and
  `remove_if_empty` (the nxm handler's key memory in `protocol.rs`), `downloads.rs`'s
  `move_new` and `copy_new`'s `cfg_attr`. `npm run tauri build` at 85ba93c built
  `riftstone.exe` and the NSIS installer with the uninstall hook (5.88 MiB), and a read-only
  bench probe of that build, on a copy of real app data, passed (BUILD-SPEC § v0.3 § The second
  native Windows run). For 0.3.1 the tests ran there again at d68df79 (864 passed, 0 failed),
  the installer was built and the probe passed again (§ The third native Windows run).
  `cargo clippy` is not installed on the Windows bench and has never run
  there. Still owed: `cargo clippy` natively, and every press on a Windows desktop: Play by each
  route; Open mods folder, also after deleting it while the stone runs; the nxm switch on and
  off, also over another program's key; a download with each account type, one cancelled, one
  into a FAT32 or exFAT folder; uninstall with the switch on; an upgrade over an installed v0.3
  (BUILD-SPEC § v0.3 § src-tauri, Open, lists each press in full).
- No profiles yet.
- Conflicts cover files the game addresses by path; two REFramework scripts can still clash in behaviour.
- **The Muster (v0.4)** shows what the levels gave, not the totals the game's own sheet shows with
  gear on: those need the game's parameter tables, a format the stone does not read yet. It shows
  each vocation's experience, not its rank, for the same reason.
- The Muster names augments and skills by id rules measured on one party (BUILD-SPEC § v0.4).
  An augment or skill a later title update adds, beyond those rules, stays unnamed: the window
  shows its number. Item names come from the game's three item text files; an item from a
  source the stone does not read shows its number too.
- The Muster reads English names only (the text files hold fourteen languages; language 1 is
  English).
- The Muster finds saves in the Steam folders Play finds the game through (the registry,
  `RIFTSTONE_STEAM_ROOT`, the default folders). A Steam folder elsewhere that the registry does not
  name is not found, and the bench's `RIFTSTONE_SAVE_DIR` is the way in.
- A slot whose signature disagrees is read once more after 500 ms. If it still disagrees and
  the file changed meanwhile, the window says the game is saving; if it did not change, the
  window says the slot is damaged.
