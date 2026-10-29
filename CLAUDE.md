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

Since v0.5 the stone knows **versions of one file**. Two packages are one Nexus file's versions
when Nexus proves it: the same mod and file group, or a check that named the other's file as
newer; names and sizes never do. Where Nexus cannot prove it (an unpacked folder, a link by mod id
alone), **the player's word** can: when their file names make it likely, the stone asks whether a
package is a version of a pinned file of the same mod, and a yes joins it (ruling 10, #00015
§18.18). The Rift and THE FAR WORLD show one entry per file, the newest
leading with the older ones inside it. Installing a version replaces the other versions' install
of the same option in one journaled step, and then the older versions left without an install move
whole to `mods (older versions)` beside the mods folder (THE UNDERCROFT), from which they come
back at a press. *Keep only the newest* cleans up versions installed one over another before
v0.5. THE MUSTER opens **by character** (the Arisen with their main pawn, within one Steam
account), remembers the one picked last, follows their newest save, and keeps their other saves in
a slot menu (#00015 §18).

The metaphor (design system #00015, *Rift-Lit Basalt*, ruled 2026-10-16, amended for v0.3 on
2026-10-17): DD2's Riftstone is the monolith you touch to summon the pawns of other Arisen. Mods are
creations from other worlds; the Rift holds them; installing pulls one through into your world.
Core nouns stay plain (mod, option, install, update, load order, play, download) — *generic gives
meaning faster* — and the voice lives in the frame, the states and the two signature interactions.

- **Experiment log:** `zmuuzn/documents/experiment-logs/00068-the-riftstone.md`
- **Design system:** `zmuuzn/documents/design-systems/00015-system-riftstone.md` (A · The Monolith; §16 is v0.3, §17 the Muster, §18 v0.5); audition `docs/design/audition.html`; tokens `docs/design/tokens.uno.ts`
- **Build spec (the contract every crate was built to):** `docs/BUILD-SPEC.md` (§ v0.5 for this version; § v0.3 for the one folder, § v0.4 for THE MUSTER)
- **Window contract (every command, event, shape):** `docs/WINDOW-CONTRACT.md` + `src/types/rift.ts`

## The Architecture Contract (RS-1 … RS-7)

- **RS-1 — Rust does everything.** Reading archives, classifying options, hashing, deploying, launching, Nexus traffic, downloads.
- **RS-2 — The mods folder is the player's.** The stone never changes, renames or deletes a file in it. It adds exactly one kind of file: a download the player asked for, written under a hidden partial name (`.riftstone-<id>.part`, which the stone removes again when the download fails or is cancelled, and at the next launch) and renamed into place only when whole and checked, never over an existing file. **Amended in v0.5, one exception:** the stone moves an older version of a file, whole, to `<mods folder> (older versions)` beside the mods folder (THE UNDERCROFT), and back at the player's word. It moves one only after the install that superseded it committed, or at the player's own press. A move is never a copy, a delete or an edit, and never lands on an existing name (the download landing's rules, `src-tauri/src/landing.rs`).
- **RS-3 — Vanilla is sacred.** `rift_core::dd2::NEVER_WRITE` (`DD2.exe`, `re_chunk_000.pak*`, `dlc/`, `_storage_/`, `steam_api64.dll` …) is refused outright; any other existing file is backed up before it is overwritten and restored on uninstall.
- **RS-4 — Write-ahead journal.** Intent saved before the first byte moves, committed after the last; a crash mid-install is rolled back at the next launch. Schema 2 since v0.3; an older journal is lifted in memory and saved as schema 2 at the next change.
- **RS-5 — One folder.** The `mods` folder beside `riftstone.exe` is the only library (a bench may point `RIFTSTONE_MODS_DIR` elsewhere). Every archive and folder directly inside it is one package; nothing else is scanned. Folder packages are known by their name, so moving the whole install keeps every link.
- **RS-6 — Nexus is optional.** Update checks, labels, Find, browsing and a mod's files use the anonymous GraphQL v2 API and need no key. The optional personal API key adds the key's extras through v1: the account, downloads, tracking and endorsing. It lives in Windows Credential Manager, never in a file or the webview; a download URL and an nxm link's key are secrets like it (never logged, never in an error, never in the window). Every request names the Riftstone (`Application-Name`, `Application-Version`), and v1's quota headers are honoured.
- **RS-7 — Vue 3 + UnoCSS**, `@unocss/reset` before `virtual:uno.css`, the `prefers-reduced-motion` floor.
- **RS-8 — The saves are the player's (v0.4; backup exception, investor 2026-10-27).** The stone never changes, renames, re-signs or uploads an original save, and keeps no decrypted copy on disk. At the player's explicit **Back up all saves…** press it may copy the original encrypted files to a separate chosen destination. It verifies every copy and unchanged sources before publishing a new snapshot, refuses while the game runs and refuses a destination in Steam's saves, the game, mod library or app data. No restore is included. The local backup manifest retains account mapping for manual recovery; source paths/account numbers never enter logs, errors or events. It carries no way to encrypt a save, and neither does the rest of the repo outside test builds: the only sealer is `rift-save`'s `fixtures` code (which `rift-muster`'s fixtures build on), and only test builds compile it. The Sentinel measures that on every push: the graph the shipped exe is built from switches on no `fixtures` feature. The privacy sweep's Python opens saves and seals none. The game's pak stays read-only (RS-3).
- **RS-9 / RS-10 and RS-2's second exception (v0.7, THE WORKSHOP, contracted and being built 2026-10-28).** RS-9: the pawn (Tamsin) is launched only in WSL, inside the stone's whole-process fence: a `bwrap` allowlist, a read-only root, and a seccomp filter refusing `AF_VSOCK` and `TIOCSTI`, a cleared environment, and mounts validated against protected roots in both directions, with Claude Code's own sandbox nested inside. The tools travel through a per-summoning mailbox the stone reads by handle. Only the deterministic probe may name what the fence holds. RS-10: the `pawn` cargo feature and `VITE_RIFT_PAWN` keep every line of it out of the public build, measured by the Sentinel. RS-2 gains *bring it through*: one built artifact, by the digest the player was shown. All three are in `docs/BUILD-SPEC.md` § v0.7 and `docs/WINDOW-CONTRACT.md` § v0.7. **Built and merged (PRs #6–#8):** `src-tauri/src/pawn/`, which holds the fence (`fence.rs`), the mailbox broker and the held Play's desk (`mailbox.rs`), the folders by handle (`folder.rs`), bring-through and Put back, the stone's errands with the install checked in its committing turn (`errands.rs`), the relay, persona and starter (`persona.rs`, `resources/pawn/`), the host survey (`host.rs`) and the summoning's PTY (`summon.rs`). **On `feat/v0.7-commands` (PR #9):** the `pawn`/`bring` error kinds, the desk (`desk.rs`: `pawn.json`, the live summoning, the room's events, the backup inventory as a record) and the fifteen commands (`commands.rs`). **On `feat/v0.7-probe`:** the fence's probe (`probe.rs` and the desk's runner): thirteen steps in the identical fence, read from outside, `pawn-fence.json` and the chip; natively on the homestation it reads `contained`. The Heretic's Assay of the probe (WP1–WP4) made every row carry a verdict (held, breached, inconclusive), a record hold only when whole, the chip read the ground now, and the broker checks run with each probe; natively every row held. **On `feat/v0.7-room`:** the room, in `src/workshop/` behind `VITE_RIFT_PAWN` (the Sentinel greps the public `dist/` with a positive control): the hollow and THE PARTING, the fence's fold, the bench's hands and held Play, the bring-through card and Put back. Still to come: the installed build's first sign-in and the native swap test the contract's unmeasured list names. Every Heretic finding so far is closed (lab `documents/assessments/2026-10-28-riftstone-workshop-*.md`); the native Windows runs and spike run K are in log #00068.

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

## THE FRAY — the party screen (v0.6, in progress on `feat/fray`)

A second window, `fray`, for the investor's portrait second monitor: the party's damage per
member while a fight runs, what it was made of (skills first, then the seven damage parts), the
fight's record when it settles, and the last five fights (#00015 §19, ruled in three passes). Its
numbers come from **the eye**, a read-only REFramework script that writes every hit, death, foe
sample and heartbeat as one JSON line under `<game>\reframework\data\riftstone\probe\`. Today
the eye is the lab's frozen party-screen probe (final-6, md5 `889b3ca5…`, kept in the private lab
repo, not here); a slimmer production eye follows once a real session has been read.
`crates/rift-fray` credits each hit by the reviewed analyzer's rule and cuts fights;
`src-tauri/src/fray.rs` follows the newest session and answers `fray_frame` (WINDOW-CONTRACT
§ v0.6). With a second monitor, `open_fray` gives THE FRAY one whole, borderless and sized to it
(the War Tent's Second Screen), remembered by name; with one monitor it opens as a window. The desk names vocations and custom skills from THE MUSTER's Codex: the eye's `sk` is the id
the save carries (measured on the first real session, every skill a member used fell in its own
vocation's block, and the game's text named all of them). The eye's folder is read; the autorun
folder is never listed.

## Workspace

```
crates/rift-core      the contract: model, path discipline + zip-slip guard, dd2 constants, errors, Nexus file names
crates/rift-archive   zip / 7z / rar (UnRAR) / folder readers behind rift_core::PackageSource
crates/rift-pak       RE Engine KPKA table of contents + the engine's murmur3 path hash; v0.4: the game's own pak (its encrypted table, chunked/zstd/deflate entries) and its GMSG text files
crates/rift-save      v0.4: a save file, read-only: the DSSS container, its Mandarin cipher, the self-describing class stream
crates/rift-muster    v0.4: the party in a save, the save folders and slot files, and the Codex (the game's names, by id)
crates/rift-fray      v0.6 (in progress): THE FRAY's numbers — the eye's NDJSON tail, credit per member, fights, the frame; `fray-read` prints a session
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
# v0.7: THE WORKSHOP compiles only with the pawn feature (RS-10), so it is gated on its own
cargo clippy -p riftstone --all-targets --features pawn -- -D warnings
cargo test -p riftstone --features pawn

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
- *Window*: `npm ci`, then typecheck, lint, test, format:check and build. RS-10's window step
  (v0.7) greps the public `dist/` for `THE WORKSHOP` and `summon_pawn` and fails on a hit; then
  it builds again with `VITE_RIFT_PAWN=1` and fails unless the same grep finds both (the
  positive control).
- *Crates*: fmt, clippy with warnings denied, RS-8 measured, and the workspace's tests. RS-8's
  step reads `cargo tree -p riftstone -e normal,build` and fails when any `fixtures` feature
  reaches the graph the shipped exe is built from. The graph is its own denominator: the step
  also fails when `cargo tree` fails or lists no `riftstone`, so it cannot pass on nothing. It
  is pinned to Rust
  1.98.0, the toolchain both benches verified with, because a newer stable's lints can turn
  `-D warnings` red overnight. Moving the pin is a deliberate commit, run on both benches first.
  RS-10's step (v0.7) reads the same graph and fails when the default build of `riftstone`
  switches on `pawn`, with a positive control that must see `pawn` under `--features pawn`.
  Clippy and the tests then run again with the feature on, since the pawn's code compiles in no
  default build. `tests/spine_rs10.rs` runs in both: without `pawn` every one of WINDOW-CONTRACT
  § v0.7's commands answers "not found" through Tauri's own IPC dispatch, and with it none does
  (a made-up name and a public command are its controls).
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
repo's own text. The same holds for a Nexus id the public tree already carried where the sweep
last passed clean (`SWEPT_CLEAN`, 5a01aa5, v0.4.1): it was the repo's own number before any link
could have put it there, so a mod linked since that shares it is a collision, not a leak. It is
set aside and counted; a Steam account's numbers never are. A hit on an untouched file, the day
a new link joins the sources, is the shape of a collision: check `git diff` against the last
clean commit before calling it a leak. Two of the game's record types carry a `Name` that is the
game's own id; their strings never count (2,767 of them on the bench's eight saves).

The sweep reads the live save folder, not a copy, on purpose. A copy goes stale the day a new
pawn is hired, and a stale copy would pass a leak of exactly the newest names. The bench rule
below (a COPY, never the real folder) guards against bench runs of the stone itself, an
unreleased build with watchers and repeated reads. The sweep reads each slot once, whole and
read-only, and keeps nothing open. It checks a slot the way `rift-save` does: header, signature,
then a length no save exceeds, before it decrypts anything. A slot that does not open is read
once more a second later, since the game may have been writing it. One that still does not
open is counted in the output, never a traceback that stops the gate.

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
RE-CARVING. v0.5 adds the characters' scenes: `musterchars` (Odalric & Wren and Aldith & Corvin
across five slots, two that won't open), `musterpinned`, `musterfollow`, and `mustergone` (a
remembered character no slot carries). `window.riftMock.musterMock.saveSlot("data008Slot")` fires
a newer save in any slot; `saveSlot(id, true)` makes it answer as being written once. On the
bench, `RIFTSTONE_SAVE_DIR` points the stone at one save folder (a COPY of one, never the real
folder: a bench run must not race the game), with the account read from the folder three levels
up, or from `RIFTSTONE_SAVE_ACCOUNT`.

The versions' scenes (v0.5, `src/services/mock/versions.ts`): `family` (Lantern Glow Recolor v1.3
over v1.2, both installed, as v0.4 stacked them), `familyfresh` (v1.3 downloaded, v1.2 installed),
`familythree` (and a v1.1 with nothing installed), `putaway` (installing v1.3 puts v1.2 away),
`putawaytaken` (it lands as ` (2)`), `putawayfail` (the move fails after the install committed),
`putawayhalf` (with v1.1 too, and Try again moves v1.2 while v1.1 still stays),
`undercroft` (v1.2 already put away), `undercroftempty` and `undercroftnone` (the folder empty, or
never made). The player's word (#00015 §18.18): `question` (the unpacked Lantern Glow Recolor folder,
linked by mod id, beside the pinned v1.3, and the stone asks), `questioninstalled`, `questionjoined`,
`questionmanual` (no question, two pinned files to pick from) and `questionrefused` (the join refused once).

THE FRAY's scenes (v0.6, `src/services/mock/fray.ts`, whose header lists all 52) open at
`fray.html?mock=<scene>`: `fray` (FIGHT 7 at 1:36), `fraysettle` (the last six seconds, then THE
SETTLING), `fraysettled`, `fraymember`, `fraytypes`, `frayworld`, `frayleft`, `frayshelf`,
`frayidle`, `fraystale`, `fraysilent`, `frayasleep` and more. The pillar's THE FRAY door opens
`fray.html?mock=fray` on the mock.

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
2. The Rust gates natively on Windows, because the `#[cfg(windows)]` code compiles nowhere else:
   `cargo test --workspace -j 4` and `cargo clippy`, then `npm run tauri build` (the only build of
   the uninstall hook). Keep the `-j 4`: compiling every test binary at once has run out of Windows
   commit memory (`os error 1455`).
3. The presses no test makes, on a Windows desktop (Known Limitations).
4. The README frames, when the window changed what they show: shot again from the mock
   (`npm run dev`, invented names only), then each one looked at.
5. The privacy sweep, last, on the commit that is pushed.

## Local usability follow-up — 2026-10-20

The Heretic's focused usability follow-up builds on `86cb5ab`; native installation and
Windows acceptance remain pending. The room inscriptions retain their names with short descriptions;
the populated library exposes Open mods folder and Scan; search has a clear button;
active filters show their scope and have a reset that keeps the typed query. Empty state
filters explain why nothing appears. Screenshot keyboard events stay inside the viewer.
Option preview, detail and completion responses belong to the selection that requested
them, so a delayed plan cannot land under a different option. The spine-side retryPutAway
from the General's review remains unchanged.

The exact rebased frontend passed 470 tests, typecheck, lint without warnings, formatting
and production build. Browser mock inspection is separate from native Windows acceptance.
The new regressions are `tests/selectionLifetime.spec.ts` and `tests/libraryUsability.spec.ts`.

## Manual save backups — investor request, 27 October 2026

v0.6.2 adds **Back up all saves…** to THE MUSTER. The manual action keeps the
destination and timing under the player's control before experiments such as
character-height trials; automatic backups on Play or installation would require retention and
failure decisions beyond this request. This is the explicit RS-8 copy exception.

`save_backup.rs` inventories the full discovered folders, streams original encrypted
bytes into an exclusive staging folder, syncs and SHA-256 checks the copies, then
compares the sources again before a no-replace publication. The command holds the
game turn and repeatedly checks the process watcher. It protects Steam saves, the
game, both mod shelves, app data and the app's install folder. The window gets only
destination/counts, never the source path or account identifiers; the local manifest
keeps the account mapping for manual recovery. Backup is independent of party parsing.

The colleague's review caught the older-versions shelf destination; it is protected
and exercised through Tauri's actual IPC dispatcher. Synthetic tests also cover all
accounts and system files, changing/added source files, game start, symlinks, repeat
backup, picker cancellation, repeated presses and receipt/reveal. The two Muster
README frames were refreshed from invented data and inspected. The window contract
owns the command and interruption behavior. Native build and installed acceptance
must be recorded separately from source checks.

Linux validation before the fullscreen integration: workspace **1,017 passed, 0 failed, 11 ignored**;
window **589 passed**; clippy with warnings denied, Rust/Vue formatting, typecheck,
production build, privacy self-test and full private-data sweep pass. Browser mock
checks at 1320×860 and 1024×680 keep the backup door visible without horizontal
overflow. The colleague's source review and older-shelf correction are complete;
native Windows validation and installed acceptance remain pending at this entry.

The integrated v0.6.2 candidate `a5c8285` contains the approved backup revision
`5262298` and the released fullscreen merge `2e256c7`. Its Linux workspace passes
**1,022 tests, 0 failed, 11 ignored**, and all **589 window tests**, strict clippy,
lint, formatting, typecheck, production build and full privacy sweep pass. Native
Windows Rust 1.98.0 MSVC tests at that exact candidate pass **1,007 tests, 0 failed,
11 ignored** with `RUSTFLAGS=-D warnings`; native
`cargo clippy --workspace --all-targets -j 4 -- -D warnings` also passes without
warnings. The installer build uses that candidate; installed button presses and
native display acceptance remain separate. Later journal/frame-only edits do not
change executable source.

## Known Limitations

- **Manual save backups (v0.6.2)** cover every discovered account's original encrypted
  `win64_save` files, including system/creator data, independently of readable party slots.
  The destination must be outside Steam saves, the game, both mod shelves and app data.
  A crash can leave a hidden `.riftstone-save-backup-*` folder in the chosen destination;
  it is incomplete. Only a published `DD2 saves …` folder with its manifest is a completed
  backup. Restore, scheduling and automatic cleanup/retention are not included.
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
  the installer was built and the probe passed again (§ The third native Windows run). For 0.4.0
  they ran there at f653d8b (913 passed, 0 failed), the installer was built, and a probe of THE
  MUSTER on a copy of a real save folder named 80 of 80 equipped things (BUILD-SPEC § v0.4, As
  built). For 0.4.1, after the General's review round, they ran at e24a73a (920 passed, 0
  failed), and the installer and the probe passed again. For 0.5.0 they ran first at c372344
  (953 passed, 0 failed), where the probe found no family on the investor's library and led to
  ruling 10, and again at 2f410ea with the player's word (959 passed, 0 failed); the installer
  was built and the probe asked about exactly the package the investor reported, a folder whose
  file names are all in the pinned download.
  On 2026-10-27 the matching Windows Clippy component was installed. The colleague's
  first clean run at `5906e79` was peer-reported; the Heretic independently ran
  `cargo clippy --workspace --all-targets -j 4 -- -D warnings` at `a5c8285`, exit 0
  with no warnings. Still owed: every press on a Windows desktop: Play by each
  route; Open mods folder, also after deleting it while the stone runs; the nxm switch on and
  off, also over another program's key; a download with each account type, one cancelled, one
  into a FAT32 or exFAT folder; uninstall with the switch on; an upgrade over an installed v0.3
  (BUILD-SPEC § v0.3 § src-tauri, Open, lists each press in full).
- No profiles yet.
- THE WORKSHOP's terminal (0.7.1) opens only an `https` link the player Ctrl+clicks and copies,
  pastes and interrupts by the chords in `src/workshop/hollowHands.ts` (WINDOW-CONTRACT § v0.7,
  The room as built). It does not honour OSC 52, so Claude Code's `c to copy` copies nothing:
  Ctrl+click the link instead. Native sign-in on the installed build is the first proof.
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
- **Versions (v0.5)** are known only when Nexus proves them: both packages linked to the same mod
  and pinned to one file group (an update check or a download pins a link), or a check that named
  the other's file as newer, or by the player's word (ruling 10). A package linked by name alone
  joins a family only when the player says so; the stone asks only when one package's file names
  are all among the other's and they share two or more (or each holds just the one same name),
  so a version that renamed or dropped a file is never asked about
  (the option detail's *This is a version of…* joins it all the same). An unlinked package joins
  nothing: link it first. Without a join, its install stacks over another version's as in v0.4,
  and both keep an entry. A modder who uploads a
  new version as a new file in another group, never marked as the old one's update, gets two
  entries too.
- The older-versions folder sits outside every watcher: an archive moved into it by hand shows
  at the window's next look (entering the Rift, opening THE UNDERCROFT, a put-away or a bring
  back). A mods folder at the root of a drive has no folder beside it, so nothing is put away
  there (the install still stands and says why). Nor is anything put away when the mods folder is
  a link to another drive: the older-versions folder sits beside the link, and a move across
  drives is refused rather than turned into a copy.
- A put-away or bring-back that has to take another name (` (2)` …) saves the record under that
  name to `nexus.json` BEFORE the move (RS-A2, closed in v0.6): a save that fails refuses the
  move with nothing moved. A crash between that save and the move leaves the record under both
  names, and a restart loads both, so a leftover can outlive every later save. The stone's own
  moves are safe from it: before a move takes a name it writes over whatever record that name
  holds (the Heretic's counterexample on PR #3), and it passes over a name whose id another
  archive or folder on disk already carries, since ids go by name and not by folder. A package
  the PLAYER later puts in the mods folder under exactly a leftover's name would still inherit
  it. While `nexus.json` is held (unreadable at launch), nothing is saved, so the move goes ahead
  as before and the window already names the file held.
- THE FRAY on a monitor of its own has no title bar, so it has no close button. Close it from
  the taskbar, or click it and press Alt+F4; the pillar's door opens it again on the same
  monitor. A plain window (one monitor at the press) stays plain when a second monitor is
  plugged in until the door is pressed again. The fit and the no-focus rule rest on tao 0.37's
  Windows code as read, not on a native run.
- THE MUSTER tells characters apart by the Arisen's and the main pawn's names within one Steam
  account: the saves carry no playthrough id. Two playthroughs begun with the same two names read
  as one character, and the slot menu tells their saves apart by level and time.
