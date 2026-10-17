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

The metaphor (design system #00015, *Rift-Lit Basalt*, ruled 2026-10-16, amended for v0.3 on
2026-10-17): DD2's Riftstone is the monolith you touch to summon the pawns of other Arisen. Mods are
creations from other worlds; the Rift holds them; installing pulls one through into your world.
Core nouns stay plain (mod, option, install, update, load order, play, download) — *generic gives
meaning faster* — and the voice lives in the frame, the states and the two signature interactions.

- **Experiment log:** `zmuuzn/documents/experiment-logs/00068-the-riftstone.md`
- **Design system:** `zmuuzn/documents/design-systems/00015-system-riftstone.md` (A · The Monolith; §16 is v0.3); audition `docs/design/audition.html`; tokens `docs/design/tokens.uno.ts`
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
crates/rift-pak       RE Engine KPKA table of contents + the engine's murmur3 path hash
crates/rift-catalog   modinfo.ini, option discovery, the addon tree and its nesting, search, conflicts, the scan cache
crates/rift-deploy    game/REFramework discovery, Play's plan, the journal (schema 2) and its relink, install/uninstall/reorder, preflight
crates/rift-nexus     anonymous Nexus v2 GraphQL (updates, labels, Find, browse, a mod's files), v1 with the key (account, downloads, tracking, endorsing), the downloader, nxm links
src-tauri/            the spine: state, persistence, commands, events, thumbnails, the mods-folder watcher, the game watcher, downloads, the nxm handler
src/                  the window (Vue 3 + UnoCSS) — services/ is the only door to Tauri; services/mock/ runs it in a browser
scripts/              privacy-sweep.py — the pre-push gate below
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
npm test            # vitest (tests/noOldManager.spec.ts sweeps src/ and tests/ for the retired name)
npm run format:check
npm run build

# The privacy sweep's own checks (invented data; reads no library)
python3 -I scripts/privacy-sweep.py --self-test
```

`#[cfg(windows)]` code (the registry for Steam and the nxm handler, the process list, Play's
`CreateProcessW`, and a few file-system checks) never compiles on the Linux bench: build, test and
lint it natively on Windows before a release. v0.3's first native run is recorded in BUILD-SPEC
§ v0.3 § The first native Windows run.

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
--forbid <a retired word>
```

`--nexus` names the stone's `nexus.json`, where every link keeps its mod id, file id, group id
and sqid and every check keeps the mod's Nexus name and the newer file it found; the sweep also
reads the `nexus.json` beside each `--journal`, so a missing line never drops those ids.
`--private-path` names any other folder of this machine's that must not appear, such as where
builds land; the sweep reads it as a folder chain, with either slash. `--skip-name` names a
library child that is not a package. `--forbid` holds words the repo must never carry again,
printed as places, never as words: the args file is where the retired name lives, so the repo
never has to spell it. A forbidden word is checked in the working tree and at the tip a push
publishes (every tracked file at HEAD whose text differs from the working tree, and HEAD's own
message), so a clean-up that is not committed yet never passes for a clean push. The older
history keeps what it kept (BUILD-SPEC § v0.3, ruling 1), so run the sweep before each push,
with the commit you push checked out.

**Pictures are the sweeps' blind spot.** Every gate here reads text, and a screenshot carries
whatever the window drew. The sweep counts the pictures in the tree by folder on every run.
README frames are shot from the mock only (`npm run dev`, invented names), shot again whenever
the window changes what they show, and looked at, one by one, before a push.

*(Written 2026-10-16, after the first sweep found a real mod id in 30 places in rift-nexus's
tests and a real option name in BUILD-SPEC, both from before the scrub. Reworked 2026-10-17 for
v0.3: install names now come from the stone's own journal, and the Nexus ids from its
`nexus.json`. The tip check and the picture count followed that night, after a review found
the retired name still committed at HEAD in three files while the sweep, reading only the
uncommitted working tree, reported none, and the README's frames still showing it in their
pixels.)*

## Running It

```sh
npm run dev            # the window in a browser on the mock backend (?mock=empty|emptymods|nogame|scanning|refoff|patched|offline|fresh|firstrun|key|premium|rejected|quotalow|running|nxmother|nocreations, comma-separated)
npm run tauri dev      # the real app (WSL bench: set RIFTSTONE_GAME_ROOT / RIFTSTONE_MODS_DIR / RIFTSTONE_DATA_HOME)
npm run tauri build    # on Windows: target/release/riftstone.exe (+ an unsigned NSIS installer)
```

`RIFTSTONE_MODS_DIR` moves the library; without it the stone reads `<folder of riftstone.exe>\mods`
(under `npm run tauri dev` that is `target/debug/mods`). Never point a bench at the investor's real
mods folder with a download in mind: RS-2 lets the stone add files there.

No release pipeline and no updater: the investor ruled *"i do not really need a signed
release for this"*. Build on the Windows side and run the installer. Installing a new version over
an old one, or uninstalling, leaves the `mods` folder alone: Tauri's NSIS uninstaller deletes only
the files it installed and removes the install folder only when it is empty. Its "Delete the
application data" box (unticked by default; ignored on an upgrade) removes
`%APPDATA%\nl.zmuuzn.riftstone` and `%LOCALAPPDATA%\nl.zmuuzn.riftstone` whole: the journal and its
backups, `nexus.json`, the settings, caches and logs. Uninstall mods from the stone before ticking
it, or their files stay in the game folder with no journal to take them out.

**Before a release**, in this order:
1. Every gate above, on the Linux bench.
2. The Rust gates natively on Windows: `cargo test --workspace` and `cargo clippy` (the
   `#[cfg(windows)]` code compiles nowhere else).
3. The presses no test makes, on a Windows desktop (Known Limitations).
4. The README frames, when the window changed what they show: shot again from the mock
   (`npm run dev`, invented names only), then each one looked at.
5. The privacy sweep, last, on the commit that is pushed.

## Known Limitations

- Downloads in the stone need a premium account; a free account goes through the file's page and
  the site's Mod Manager Download button, which reaches the stone only while its nxm handler is on.
  Vortex, when installed with "Handle Nexus Links" on, takes the handler back at each of its starts.
- A download is not resumed across a restart: its partial file is removed at the next launch.
- A Nexus file that is not a zip, 7z or rar (a raw `.pak`, an `.exe`) lands in the mods folder under
  its own name, but the scan never lists it: only archives and folders are packages.
- Nexus labels reach linked packages only. MD5 identify cannot link an unpacked folder (there is no
  archive to hash) or a repack (its bytes are new). Find on Nexus searches those by name (the
  package's, else its first option's bundle or modinfo name) and offers up to five candidates;
  the player links. A name unlike the mod's Nexus name finds the wrong mods or none, and a link
  by mod id stays unpinned (labels yes, "is my copy the newest" no) until the file's URL is pasted.
- The veil's `adult` mode also blurs packages Nexus has not labelled yet, on purpose (a veil protects).
- Windows-only (discovery reads the Windows registry; env overrides serve the WSL bench).
- v0.3's Windows-only code has compiled and its tests have run natively on Windows (BUILD-SPEC
  § v0.3 § The first native Windows run), but nothing has been pressed on a Windows desktop yet.
  Owed: press Play by each route, use Open mods folder, turn the nxm handler on and off (with and
  without another program holding it), and download once with each account type, cancelling one
  mid-download (BUILD-SPEC § v0.3 § src-tauri, Open); also a Windows test run after 0b34ac4 and
  `cargo clippy` there.
- No profiles yet.
- Conflicts cover files the game addresses by path; two REFramework scripts can still clash in behaviour.
