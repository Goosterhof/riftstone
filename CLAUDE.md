# CLAUDE.md — The Riftstone

*The lab journal for `gadgets/riftstone/`. Read this before picking up the scalpel.*

## What This Is

**The Riftstone** is a Tauri v2 Windows desktop mod manager for **Dragon's Dogma 2**, built
to replace Fluffy Mod Manager for one player. It reads every mod you downloaded — zip, 7z,
rar or a plain folder, nested as deep as the modder packed it — and searches all of it at
once: option names, descriptions, the menu trail a variant hides behind, and every file path
inside the archives. It installs and uninstalls the way the 2026 engine expects
(`pak_mods/`, loose `natives/`, REFramework scripts, root DLLs) under a write-ahead journal
that remembers every byte it moved, and it asks Nexus Mods — anonymously, no key — whether a
newer file of each download exists. Since v0.2 the same check brings each linked mod's Nexus
category and tags home, and they filter the search (categories any-of, tags all-of).

The metaphor (design system #00015, *Rift-Lit Basalt*, ruled 2026-10-16): DD2's Riftstone is
the monolith you touch to summon the pawns of other Arisen. Mods are creations from other
worlds; the Rift holds them; installing pulls one through into your world. Core nouns stay
plain (mod, option, install, update, load order) — *generic gives meaning faster* — and the
voice lives in the frame, the states and the two signature interactions.

- **Experiment log:** `zmuuzn/documents/experiment-logs/00068-the-riftstone.md`
- **Design system:** `zmuuzn/documents/design-systems/00015-system-riftstone.md` (A · The Monolith); audition `docs/design/audition.html`; tokens `docs/design/tokens.uno.ts`
- **Build spec (the contract every crate was built to):** `docs/BUILD-SPEC.md`
- **Window contract (every command, event, shape):** `docs/WINDOW-CONTRACT.md` + `src/types/rift.ts`

## The Architecture Contract (RS-1 … RS-7)

- **RS-1 — Rust does everything.** Reading archives, classifying options, hashing, deploying, Nexus traffic.
- **RS-2 — The library is read-only.** No write, rename or delete ever touches a library root.
- **RS-3 — Vanilla is sacred.** `rift_core::dd2::NEVER_WRITE` (`DD2.exe`, `re_chunk_000.pak*`, `dlc/`, `_storage_/`, `steam_api64.dll` …) is refused outright; any other existing file is backed up before it is overwritten and restored on uninstall.
- **RS-4 — Write-ahead journal.** Intent saved before the first byte moves, committed after the last; a crash mid-install is rolled back at the next launch.
- **RS-5 — Fluffy is a neighbour.** Its files and `installed.ini` are read, never written. Its installs can be adopted into the journal; nothing un-adopted is ever deleted.
- **RS-6 — Nexus is optional.** Everything works offline. Update checks use the anonymous GraphQL v2 API; an optional personal API key lives in Windows Credential Manager, never in a file or the webview.
- **RS-7 — Vue 3 + UnoCSS**, `@unocss/reset` before `virtual:uno.css`, the `prefers-reduced-motion` floor.

## The DD2 Load Rules (researched 2026-10-16; constants in `crates/rift-core/src/dd2.rs`)

- REFramework's `IntegrityCheckBypass_LoadPakDirectory` loads `pak_mods/` in **file-name
  order** (NTFS = alphabetical) into `re_chunk_000.pak.sub_000.pak.patch_NNN` slots — the
  last name wins. Confirmed in the investor's own `re2_framework_log.txt`. The `.pak`
  extension check is case-sensitive.
- The Riftstone names its paks `zrift-NNNN-<stem>.pak` — sorting after Fluffy's `xNNNN-`, so
  a new install wins over Fluffy's leftovers. `pak_mods/` stays flat.
- LooseFileLoader serves any `natives/` path that exists on disk ahead of every pak.
- Both lists are read at game start: an install while DD2 runs loads at the next launch.
- TU 3.2 (2026-09-01/02) changed asset suffixes (`.mesh.240423143` → `.mesh.260421070`,
  `.tex.760230703` → `.tex.251211553` …); loose files with old suffixes are flagged.
- More than 100 paks in `pak_mods/` → a warning (REFramework's slot table has no bounds check).

## Workspace

```
crates/rift-core      the contract: model, path discipline + zip-slip guard, dd2 constants, errors, Nexus file names
crates/rift-archive   zip / 7z / rar (UnRAR) / folder readers behind rift_core::PackageSource
crates/rift-pak       RE Engine KPKA table of contents + the engine's murmur3 path hash
crates/rift-catalog   modinfo.ini, option discovery, the addon tree, search, conflicts, the scan cache
crates/rift-deploy    game/REFramework/Fluffy discovery, the journal, install/uninstall/reorder/adopt, preflight
crates/rift-nexus     anonymous Nexus v2 GraphQL, MD5 lookup, file pinning, update state
src-tauri/            the spine: state, persistence, commands, events, thumbnails, the library watcher
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
npm test            # vitest
npm run format:check
npm run build
```

Bench suites (`#[ignore]`, env-gated, READ-ONLY against the investor's real data) — see each
crate's `tests/bench_real.rs`. They never install, uninstall or flip a switch, and they print
counts only. rift-nexus's live name-search bench
(`RIFT_BENCH_NEXUS_FIND=1 cargo test -p rift-nexus --test bench_real -- --ignored --nocapture`)
sends only its two fixed phrases (`lantern glow`, `armor`), never a name from the library.

## Privacy — the library is the investor's own

This repo is public, and the investor's mod library is private. **No
real mod name, file name, option name, author, Fluffy section, Nexus mod id, file id or sqid from
it ever enters this repo**: not in fixtures, tests, docs, comments or bench output. Fixtures use
invented names (*Vernworth Cloaks Collection*, *Lantern Glow Recolor*, mod `2207`, sqid
`Rk4vLmN2Q`). A bench that needs a real identifier takes it from the environment (the live Nexus
bench reads `RIFT_BENCH_NEXUS_MOD`). **Before every push**, run the sweep. It reads the library
read-only and greps the tree for every identifier it holds, printing locations, never names:

```sh
python3 -I scripts/privacy-sweep.py \
  --library "/mnt/c/<fluffy>/Games/DragonsDogma2/Mods" \
  --library "/mnt/c/<fluffy>/Games/DragonsDogma2" \
  --downloads "/mnt/c/Users/<you>/Downloads" \
  --fluffy-ini "/mnt/c/<fluffy>/Games/DragonsDogma2/installed.ini"
```

*(Written 2026-10-16, after the first sweep found a real mod id in 30 places in rift-nexus's
tests and a real option name in BUILD-SPEC, both from before the scrub.)*

## Running It

```sh
npm run dev            # the window in a browser on the mock backend (?mock=empty|nogame|scanning|refoff|patched|offline|fresh|firstrun|key|quotalow, comma-separated)
npm run tauri dev      # the real app (WSL bench: set RIFTSTONE_GAME_ROOT / RIFTSTONE_FLUFFY_ROOT / RIFTSTONE_DATA_HOME)
npm run tauri build    # on Windows: target/release/riftstone.exe (+ an unsigned NSIS installer)
```

No release pipeline and no updater: the investor ruled *"i do not really need a signed
release for this"*. Build on the Windows side and run the exe.

## Known Limitations (v0.2)

- Nexus labels reach linked packages only. MD5 identify cannot link an unpacked folder (there is no
  archive to hash) or a repack (its bytes are new). Find on Nexus searches those by name (the
  package's, else its first option's bundle or modinfo name) and offers up to five candidates;
  the player links. A name unlike the mod's Nexus name finds the wrong mods or none, and a link
  by mod id stays unpinned (labels yes, "is my copy the newest" no) until the file's URL is pasted.
- The veil's `adult` mode also blurs packages Nexus has not labelled yet, on purpose (a veil protects).
- Windows-only (discovery reads the Windows registry; env overrides serve the WSL bench).
- No `nxm://` handler — it would fight Vortex and MO2 for the protocol.
- No profiles yet.
- Conflicts cover files the game addresses by path; two REFramework scripts can still clash in behaviour.
