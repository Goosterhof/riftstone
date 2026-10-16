# The Riftstone — Build Spec (v0.1)

*The single source of truth the builders work from. Each section names the
crate, the files it owns, the contract it must honour, and the tests that
prove it. Builders edit ONLY the files their section owns.*

## What the Riftstone is (one paragraph, for every builder)

A Windows desktop mod manager (Tauri v2, Rust + Vue 3) for **Dragon's Dogma 2**,
built for one player who outgrew Fluffy Mod Manager. Three asks, in the
investor's words: *"one specific for dragons dogma 2"*, *"search through the
nested directories/zip"*, and *"linked to nexus, so the modmanager can check if
there are updates"*. The evidence behind the second ask: one archive in the
investor's library (an armour-variants collection, 268 MB) holds **281
installable options** (each with its own `modinfo.ini`), 2,674 files, folders
17 deep, wired into Fluffy's nested `addonfor` menus — and the investor had
unpacked it by hand beside Fluffy to dig through it in Explorer. Fluffy's own
search covers mod titles only.

## The ground truth on the investor's machine (October 2026)

Measured, not assumed. Builders must not contradict these facts.

- **Game:** Steam app `2054970`, install `C:\Program Files (x86)\Steam\steamapps\common\Dragons Dogma 2`,
  `appmanifest_2054970.acf` `buildid` `25290080`. `re_chunk_000.pak` is 68.9 GB (2026-10-09 —
  the new RE Engine version); `re_chunk_000.pak.patch_001.pak` … `patch_015.pak` are 144-byte
  stubs (KPKA v4.1, flags 0x0008, **zero files**). Game paks are KPKA v4.2 with an encrypted
  table (flags 0x0068 / 0x0028).
- **Mod paks** (`pak_mods/x0000-*.pak`): KPKA **v4.0, flags 0x0000 → plain, readable
  table of contents** (177, 31 and 2 entries in the three installed).
- **REFramework** `v1.5.9.1+514` (`dinput8.dll` in the game root). Its config
  `re2_fw_config.txt` (`Key=value` lines) holds `IntegrityCheckBypass_LoadPakDirectory=true`
  (the "Alternate PAK Folder" — loads `pak_mods/`), `LooseFileLoader_Enabled=true`
  (loads `natives/`), `LooseTextureLoader_Enabled=true`, `FaultyFileDetector_Enabled=true`.
  Its log `re2_framework_log.txt` carries `Tag: v1.5.9.1`, `Commits past tag: 514`,
  `Build date: 11.10.2026`, `Game name: dd2`. Files `reframework_loose_files.txt`,
  `reframework_accessed_files.txt`, `reframework_faulty_files.txt` sit in the game root.
- **Game-folder layout of installed mods:** `pak_mods/xNNNN-<name>.pak` (Fluffy's priority
  prefix; *install order decides priority*), `natives/stm/...` loose files,
  `reframework/autorun/*.lua`, `reframework/data/...`, `reframework/plugins/`, `dinput8.dll`.
- **Fluffy Mod Manager**, portable, in whatever folder it was unpacked to (`<fluffy>\Modmanager.exe`). Its DD2 state:
  `Games\DragonsDogma2\installed.ini`, library `Games\DragonsDogma2\Mods\` (zip, rar,
  folders). `installed.ini` format, verbatim shape:

  ```ini
  GameInstallService=Steam
  GameInstallPath=C:\Program Files (x86)\Steam\steamapps\common\Dragons Dogma 2
  [RE Framework]
  ModID_v1=581683767515617555
  ShortModID_v1=16313623759508256203
  ModName=RE Framework
  installtype=normal
  file=dinput8.dll
  [LanternGlow-v2.1]
  ModName=Lantern Glow Recolor
  installtype=normal
  file=pak_mods\x0000-LanternGlow-v2.1.pak
  [Tops314-ErmineCape]
  ModName=Ermine Cape
  file=natives/stm/appsystem/TopsWb/mesh/tops_ErmineCape_wb_f.pfb.18
  file=reframework/data/WardrobeEnabler/database/tops/Female/Tops_314.json
  ```

  Section header = the mod's folder name in the archive; `ModName` = modinfo `name`;
  one `file=` per deployed file, relative to the game root, **either slash**. Fluffy
  writes `ModdedByFluffyModManager.txt` into the game root while mods are installed,
  and keeps `CrashReport_FMMBACKUP.exe` (it renames the game's crash reporter).
- **Fluffy skips** `modinfo.ini`, the screenshot and root-level `README*.txt` when
  installing (observed: a mod with `README.txt` + `screenshot.png` installed neither).
- **modinfo.ini** dialect (Fluffy readme + change log + observed files): `key=value`
  (also `key: value`), keys case-insensitive, CRLF. Keys: `name`, `description`,
  `author`, `version`, `screenshot`, `category`, `homepage`, `addonfor` (hide under the
  option NAMED so), `NameAsBundle` (group siblings into one bundle), `requires` /
  `requirement`, `DummyMod=True` (menu entry only), `MenuPriority` (int, higher first).
  Collections chain menus: `Tops` (a DummyMod) ← `Ermine Cape (Menu Option)`
  (`addonfor=Tops`, its description saying the entry is a menu that does nothing on
  its own) ← variants (`addonfor=Ermine Cape (Menu Option)`).
- **Archive formats in the wild here:** zip (deflate + stored dirs), **RAR5**
  (a REFramework helper mod), plain folders. 7z is common on Nexus generally.
- **A Nexus download** in `Downloads` named in the 2026 shape `<Name> <modId> <version> <YYYY-MM-DDTHH-MMZ> <token>.zip`
  (invented example: `Vernworth Cloaks 2207 8 2026-10-14T14-23Z Rk4vLmN2Q.zip`).

## Architecture contract (RS-1 … RS-7)

- **RS-1 — Rust does everything.** All reading, classification, hashing, deploying and
  Nexus traffic live in Rust. No sidecars, no shelling out.
- **RS-2 — The library is read-only.** The Riftstone never writes into a library root:
  no re-zipping, no extracting beside the archive, no renaming downloads.
- **RS-3 — Vanilla is sacred.** The deploy engine never writes `DD2.exe`,
  `re_chunk_000.pak*`, anything under `dlc/`, or any game file it did not put there
  itself — unless it first backs the original up under the journal, and it restores it
  on uninstall. A hard deny-list refuses the first three outright.
- **RS-4 — Write-ahead journal.** Every install/uninstall records its intent before the
  first byte moves and commits after the last. A crash mid-install is detected at the
  next launch and rolled back from the intent.
- **RS-5 — Fluffy is a neighbour, not a victim.** Fluffy's files are read, never
  written (`installed.ini` included). Files Fluffy deployed are known by owner; the
  Riftstone can ADOPT them (take them into its own journal) but never deletes a Fluffy
  file it has not adopted.
- **RS-6 — Nexus is optional.** Everything works offline. The API key lives in the
  Windows Credential Manager, never in a file or the webview. Requests carry the
  app's name and version; quota headers are honoured.
- **RS-7 — Vue 3 + UnoCSS**, `@unocss/reset` before `virtual:uno.css`, the
  `prefers-reduced-motion` floor in `uno.config.ts`.

## Workspace layout and ownership

```
Cargo.toml              workspace (owner: main session)
crates/rift-core/       the contract — DONE, owner: main session (read it first)
crates/rift-archive/    owner: archive builder
crates/rift-pak/        owner: pak builder
crates/rift-catalog/    owner: catalog builder
crates/rift-deploy/     owner: deploy builder
crates/rift-nexus/      owner: nexus builder
src-tauri/              owner: integration builder (after the crates land)
src/ tests/ uno.config.ts  owner: the Illusionist (after the Artisan's direction)
docs/                   owner: main session
```

**Rules for every builder:**

1. Read `crates/rift-core/src/*.rs` first — it is the contract. Do not change it. If
   the contract is genuinely wrong for your crate, say so in your final report with the
   exact change you need; do not edit it.
2. Keep the public signatures already stubbed in your crate (the integration builder
   codes against them). You MAY add public items. You MAY add private modules.
3. Your crate's `Cargo.toml` is yours: add dependencies there (prefer the
   `workspace = true` ones already declared in the root `Cargo.toml`). Do not edit the
   root `Cargo.toml`; if you need a new workspace dependency, add it to your own
   crate's `Cargo.toml` with an explicit version.
4. Build and test ONLY your crate, with your own target dir so builders do not block
   each other on cargo's lock:
   `CARGO_TARGET_DIR=target/<crate> cargo test -p <crate>` and
   `CARGO_TARGET_DIR=target/<crate> cargo clippy -p <crate> --all-targets -- -D warnings`.
   Both must be clean when you finish. `cargo fmt -p <crate>`.
5. Tests build their fixtures at run time (in a `tempfile` dir). Never commit a real
   mod, a real archive, or anything copied from the investor's library or game folder.
   No real mod names from the investor's library in fixtures, tests or docs — use
   invented ones (`Lantern Glow Recolor`, `Vernworth Cloaks Collection`).
6. Naming Doctrine: user-facing strings (error `reason`s, problem messages) are plain,
   specific sentences that say what happened and what to do. Module names may stay
   plain for mechanism; domain types already have their names in rift-core.
7. **Never touch the real game folder, Fluffy's folder, or the investor's Downloads from
   a test.** Real-data runs belong to the bench suite (`#[ignore]`d, env-gated), never to
   `cargo test`'s default set.
8. Do not run `git` commands that change history or the index. The main session commits.

---

## § rift-archive

**Owns:** `crates/rift-archive/**`.

**Contract:** `sniff(path)` and `open(path) -> Box<dyn PackageSource>` (stubbed), the
`PackageSource` trait in `rift-core/src/source.rs`.

**Behaviour:**

- `sniff` reads magic bytes: `PK\x03\x04` (also `PK\x05\x06` empty zip) → Zip;
  `7z\xBC\xAF\x27\x1C` → SevenZip; `Rar!\x1A\x07\x00` (RAR4) and `Rar!\x1A\x07\x01\x00`
  (RAR5) → Rar; a directory → Folder; anything else → `Ok(None)`. A file that cannot be
  opened → `Err(Unreadable)`.
- Four sources: `ZipSource` (crate `zip`), `SevenZipSource` (crate `sevenz-rust2`, BZip2 +
  PPMd coders on), `RarSource` (crate `unrar_sys` — the UnRAR C++ sources — through
  rift-archive's own packed `dll.hpp` mirrors, because the `unrar` crate cannot cancel a
  read mid-entry), `FolderSource` (crate `walkdir`; follows no symlinks;
  depth cap 64).
- `entries()`: files only, paths through `rift_core::normalize_entry_path`; unsafe paths
  dropped with `log::warn!`. Zip: honour the UTF-8 flag; without it, a name whose bytes are
  valid UTF-8 decodes as UTF-8 (tools that forget the flag) and anything else as CP437 —
  the `zip` crate's own behaviour. Encrypted entries → the package is `Err(Encrypted)` only when it
  is fully encrypted; a partially encrypted archive lists, and reading an encrypted entry
  → `Err(Encrypted)`.
- `read_entry(entry, limit)`: case-insensitive match; `limit` stops decompression early
  (the pak builder's TOC read needs the first `16 + 48 × n` bytes of a 2 GB pak — do not
  inflate the whole entry). 7z/rar have no random access: decompress sequentially up to
  the entry; acceptable.
- `extract(items, progress)`: ONE sequential pass for 7z and rar (iterate the archive in
  its own order, write the requested entries as they pass); zip and folders may go in
  item order. Create parent dirs. Write to `dest` exactly (the caller already chose a
  temporary name). Report progress after each file. Stop at the first error.
- Folder sources: `read_entry` and `extract` read/copy from disk; `extract` copies
  (never moves — RS-2).
- Every open/read error maps to a `RiftError` with the package path in it.

**Tests (in-crate, fixtures built at run time):** zip with nested dirs, a stored dir
entry, a backslash-separated entry name, a `../evil` entry (dropped), a CP437 name;
`read_entry` with and without `limit` (limit actually limits bytes read on a large
deflated entry — assert on the returned length); `extract` into a tempdir. 7z: build a
fixture with `sevenz-rust2`'s compress feature (enabled in the workspace dep). RAR cannot
be written by any crate — commit ONE tiny RAR5 fixture you generate by hand only if a
`rar` binary is available on the bench; otherwise test RAR through an env-gated
`#[ignore]` test that reads `RIFT_BENCH_RAR` (the investor's bench has
the RAR5 archive in `/mnt/c/<fluffy>/Games/DragonsDogma2/Mods/`; read-only
listing only, never extract it outside a tempdir). Folder source over a tempdir.

**Windows note:** `unrar` builds the UnRAR C++ sources with `cc`; it must compile with
MSVC (the investor builds on Windows with VS 2022 Build Tools) and with gcc on the Linux
bench. If `unrar` fails to build on Linux, report it — do not swap RAR support out.
*(As built: `build.rs` links `advapi32` on Windows; without it MSVC left 13 UnRAR symbols
unresolved.)*

**As built — opening a zip (main session, 2026-10-16).** `ZipSource::open` first reads
the central directory where an archive starting at byte 0 keeps it
(`Config { archive_offset: ArchiveOffset::Known(0) }`), and lets the `zip` crate search
for the start (`ZipArchive::new`, Detect) only when that fails, which is a zip behind a
program stub. When both fail, the search's error is reported. Detect scans every byte
before the end record, and behind a `BufReader` (whose seeks empty the buffer) it read
536 MB to open an 8 MB zip. The file therefore sits behind `SeekBuffered`, whose seeks
keep the buffer when the target lies inside it: listing a 3,000-entry directory read
394 MB through a plain `BufReader`. Measured on the investor's library over `/mnt/c`:
the cold scan went from 178 s to 0.21 s, with identical counts. Tests: a zip at byte 0
opens reading < 256 KiB of an 8 MB archive; a 3,000-entry directory lists reading about
the directory once; a stub-prefixed zip lists, reads and extracts; `SeekBuffered` lands
where it says, inside and outside the buffer.

---

## § rift-pak

**Owns:** `crates/rift-pak/**`.

**Contract:** stubbed in `crates/rift-pak/src/lib.rs`: `PathHash`, `PakEntry`, `PakToc`,
`PakError`, `HEADER_LEN`, `toc_len`, `read_toc`, `murmur3_32`, `path_hash`.

**Format (KPKA, RE Engine; verified on the investor's mod paks):** little-endian.
Header 16 bytes: `magic: [u8;4] = "KPKA"`, `major: u8` (4), `minor: u8` (0/1/2),
`feature_flags: u16`, `file_count: u32`, `fingerprint: u32`. Entry table follows the
header: `file_count` entries of **48 bytes** (v4): `hash_lower: u32`, `hash_upper: u32`,
`offset: u64`, `compressed_size: u64`, `decompressed_size: u64`, `attributes: u64`,
`checksum: u64`. When `feature_flags & 0x0008` the table is encrypted (the game's own
paks, the 144-byte stubs) — return `encrypted: true`, `entries: []`, never an error.
Accept major 4 with minor 0–2; other majors → `UnsupportedVersion`. (v2 paks with
24-byte entries exist in older RE titles; DD2 mod paks are v4 — supporting v2 is not
required.) `toc_len = 16 + 48 × file_count` (16 when encrypted). A plain table claiming more than
`MAX_ENTRIES` (4,194,304) files is refused as `TooManyEntries` by `toc_len`, `read_toc` and
`read_toc_from` alike. Flag bits outside `KNOWN_FLAGS` (0x007C) are read through and reported
by `PakToc::unknown_flags()`. *(Built 2026-10-16: the 128-byte key follows the 0x0010 / 0x0004
/ 0x0040 blocks, not the table directly — see the crate doc.)*

**Hash:** `murmur3_32` = MurmurHash3 x86_32. `path_hash(p)`: backslashes become forward
slashes; fold ASCII letters only (A–Z ↔ a–z, every other UTF-16 unit unchanged);
`lower = murmur3_32(utf16le(ascii_lower(p)), 0xFFFF_FFFF)`,
`upper = murmur3_32(utf16le(ascii_upper(p)), 0xFFFF_FFFF)`. The hashed path INCLUDES the
`natives/stm/` prefix. **Ruled 2026-10-16 (main session):** ASCII-only folding — it is
ree-pak-rs's rule (verified at commit 25562e6), REE.PAK.Tool's .NET culture folding and
Rust's full Unicode mapping disagree with each other on ß / Σ / İ, and every one of the
366,525 known DD2 paths is ASCII, so the choice changes no real hash. **Verify** against the
public reference implementation (REE.PAK.Tool / ree-pak-rs) and against the standard
murmur3 test vectors (`murmur3_32(b"", 0) == 0`, `murmur3_32(b"", 1) == 0x514E28B7`,
`murmur3_32(b"hello", 0) == 0x248BFA47`, `murmur3_32(b"Hello, world!", 1234) == 0xFAF6CDB3`).
Document in the module doc whether the hashed path includes the `natives/stm/` prefix,
citing the reference you checked.

**Tests:** header parse of a forged plain pak (2 entries) and a forged encrypted header;
truncated input → `Truncated`; bad magic → `NotAPak`; murmur3 vectors; `path_hash`
lower/upper differ and are case-insensitive in input.

---

## § rift-catalog

**Owns:** `crates/rift-catalog/**`.

**Contract:** the stubs in `crates/rift-catalog/src/*.rs`.

### modinfo
`parse(bytes)`: decode — UTF-8 BOM → UTF-8; UTF-16 LE/BE BOM → UTF-16; otherwise try
strict UTF-8, else Windows-1252 (`garbled = true` only when replacement characters
survive). Lines split on `\r\n`/`\n`; `;` and `#` start comments; blank lines skipped;
`[section]` lines ignored. Each line splits at the FIRST `=` or, failing that, the first
`:`; key trimmed and lower-cased, value trimmed. Empty values = absent. Known keys into
their fields (`requirement` and `requires` both append to `requires`; `dummymod` true
for `true/1/yes`, case-insensitive; `menupriority` parsed as i32, default 0); the rest
into `extra`. A repeated key: last wins (except `requires`).

### classify — `build_options(package_id, display_name, entries, modinfos)`
1. **Option roots with modinfo:** every directory in `modinfos` is an option root.
2. **Option roots without modinfo:** for files not under any modinfo root, find anchors:
   the first path segment (from the top) equal to `natives` or `reframework`
   (case-insensitive) makes the directory BEFORE it a root; a `.pak` file (anywhere not
   under natives/reframework) makes its parent dir a root; a `dinput8.dll` makes its
   parent a root. Files under no root (top-level docs, images) are documentation —
   they belong to no option. If a root found this way sits inside another such root,
   the outer one wins.
3. **Payload:** an option's payload = the files under its root that are not under a
   NESTED option root (nested roots are their own options). Classify each file by its
   path relative to the option root:
   - starts with `natives/` → `Loose { game_path: rel }`;
   - starts with `reframework/` → `Reframework { game_path: rel }`;
   - ends with `.pak` (anywhere else) → `Pak { file_name: <file name> }`;
   - directly in the root, and `modinfo.ini` → skipped `Modinfo`; an image
     (`jpg jpeg png gif bmp webp`) → skipped `Picture`; a document (`txt md pdf url
     nfo htm html rtf doc docx`) → skipped `Document`; anything else (`.dll .asi .ini
     .json` …) → `Root { game_path: rel }`;
   - anything in another subfolder → skipped `UnknownPlace` (and one `FilesSkipped`
     problem on the package, naming the count).
4. **Name:** modinfo `name`, else the root folder's last segment, else the package
   display name. `screenshot`: the modinfo `screenshot` file if it exists under the root
   (case-insensitive), else `screenshot.png|jpg|jpeg`, else the first root-level image.
5. `menu_only`: `DummyMod`, or an empty payload (it may still have addon children —
   tree resolves that). `bundle` from `NameAsBundle`; `addon_for` raw.
6. A package with zero options with payload and zero menu entries → one
   `NothingToInstall` problem.
7. Options sorted: `menu_priority` desc, then name (case-insensitive).
8. `is_dd2_shaped(entries)`: true when any entry path has a `natives/` or `reframework/`
   segment, or ends `.pak`, or is a `modinfo.ini`, or a `dinput8.dll`.

### tree — `resolve(&mut [Package])`
`addon_for` → the parent option with that `name` (case-insensitive, trimmed): same
package first, else any package (first by library order). Unresolved → keep `addon_for`,
`parent: None`, and add a `MissingParent` problem to the package once per distinct name.
Cycles → break at the option that closes the cycle (no parent). `trail` = names from the
package display name down to the parent, e.g. `["Vernworth Cloaks Collection", "Cloaks",
"Ermine Cape (Menu Option)"]`; bundled options insert the bundle name after the package.
An option that is `menu_only` because its payload is empty but has no children and is not
a DummyMod stays `menu_only` (it installs nothing either way).

### scan
`list_root`: the direct children of `root.path` — archive files by extension
(`zip 7z rar`, case-insensitive; `sniff` confirms during scan) and folders — skipping
dotfiles, `desktop.ini`, `*.crdownload`, `*.part`, `*.tmp`. Sorted by name.
`scan_package`: open → entries → read every `modinfo.ini` (`read_entry`, limit 64 KiB) →
`build_options`. `display_name`: strip the extension; then if
`rift_core::nexus_name::parse` (see § rift-core additions) recognises a Nexus download
name, use its `name` and set `package.nexus` (source `Filename`). Also set `nexus` from
any option's `homepage` that is a `nexusmods.com/dragonsdogma2/mods/<id>` URL when the
file name gave none (source `Homepage`). `size`/`modified_ms` from metadata (folder size
= sum of files). For a `dd2_shaped_only` root, non-DD2-shaped packages → `Ok(None)`.

### search — `SearchIndex`
Build per option a record: lower-cased name, trail (joined `›`), bundle, category,
author, description, package display + file name, version, and its payload + skipped
entry paths (relative to the package). Query: lower-case, split on whitespace; quoted
phrases (`"black necklace"`) stay one token. **Every token must match somewhere** (AND
across fields, substring match). Score per token = best field weight: name 10, trail 6,
bundle 5, package name 4, category 4, author 3, version 2, description 2, file 1; +50%
when the token is a prefix of a word in that field, +100% when it equals a whole word.
Hits sorted by score desc, then name. `matched` lists distinct fields best first;
`file_matches` up to 5 matching paths (prefer payload paths). Filters: `installed`,
`root_ids`, `include_menu_only`. Empty text → all options passing filters, library
order, limit applies. Target: < 5 ms for 300 options / 3,000 paths; < 50 ms for 10,000
options / 200,000 paths (add a `#[test]` that builds a synthetic 10k/200k index and
asserts a generous bound — 250 ms — in release-agnostic terms, or mark it `#[ignore]` if
debug builds are too slow, and run it with `--release` in your report).

### conflicts — `detect(layers)`
Index every layer's paths (lower-cased) AND their `rift_pak::path_hash` for loose paths
under `natives/`; index every pak hash. A key touched by ≥ 2 layers is a conflict.
Winner rules: among loose/reframework/root paths the LATER layer in the given slice wins
(the deploy engine passes layers in install order); a loose file beats any pak carrying
the same hash (REFramework's LooseFileLoader serves loose files ahead of pak contents);
among paks the higher `pak_order` wins. `layers` in the conflict: loser first, winner
last. Report each key once.

### catalog — `Catalog`
`scan`: for each root, `list_root`, then `scan_package` for each candidate in parallel
(rayon), reusing cached packages whose `(path, size, modified_ms)` match; then
`tree::resolve` over all packages (in root order, then name); then build the
`SearchIndex`. Emit `ScanProgress` after each package (from the calling thread — collect
results from rayon, then report; or report from a channel). A package that fails to open
still appears with an `Unreadable` problem and no options. `options_named`.

**Tests:** modinfo — every key, both separators, BOMs, 1252 fallback, comments. classify
— a synthetic collection shaped like the real one (a DummyMod menu, a Menu Option with
`addonfor`, 3 variants with screenshots and `natives/` payload, a bundle via
`NameAsBundle`, a root README), a plain pak mod (`Mod/Mod.pak` + `modinfo.ini` + image),
a REFramework script mod, RE Framework itself (`dinput8.dll` + `reframework_revision.txt`
at the archive root → `Root dinput8.dll`, revision txt skipped), a no-modinfo archive with
two variant folders each holding `natives/` (→ two options named after the folders), an
`UnknownPlace` file. tree — parent resolution same-package and cross-package, missing
parent problem, cycle break, trail contents. search — AND semantics, phrase, file-path
hit (`top_314` finds the option whose payload has that mesh), filters, ranking (a name
hit outranks a description hit). conflicts — loose vs loose, loose vs pak hash, pak vs pak
by order. catalog — cache reuse (a cached package with matching metadata is not
reopened: point its path at a deleted file and assert it still loads from cache).

**Bench (`tests/bench_real.rs`, `#[ignore]`, env `RIFT_BENCH_LIBRARY=<folder>`):** scan
the folder read-only, print packages / options / entries / ms, run three searches and
print the top hits. The main session runs it against the investor's Fluffy library.

### As built — rules added in wave B2 (2026-10-16; each one is a test)
- **classify, nested variants** (amends rule 3's "another subfolder → `UnknownPlace`"):
  a modinfo root with no game files of its own turns its direct subfolders that are
  anchor roots (holding `natives/`, `reframework/`, a `.pak` or `dinput8.dll`) into
  nested options. Each is named after its folder and inherits the parent's modinfo minus
  `name`, `screenshot` and `DummyMod`. It gets `addon_for` = the parent's name and
  `parent` = the modinfo option, and the parent becomes `menu_only`. A parent that still
  has root-level game files of its own keeps them as payload.
- **classify, `OutdatedFormats`:** counts loose payload files whose suffix
  `rift_core::dd2::outdated_suffix` flags (never reframework, root, pak or skipped
  files). It is one problem per package, naming the September 2026 engine update (TU 3.2)
  and DD2 Pak Bridge, and the sentence agrees in number.
- **tree:** among options of one package that share the parent's name, the one whose
  folder holds the child wins.
- **scan:** a `modinfo.ini` that cannot be decoded cleanly or read adds one
  `ModinfoGarbled` problem and keeps its option. A folder's cache key is the sum of its
  file sizes plus the newest time anywhere inside it.
- **scan, very old Nexus names (main-session ruling):** the timestamp-less classic shape
  (`Lantern Glow-123-1-2.zip`) is a link only when a `homepage=` line of the same
  package names that mod. Otherwise the package keeps its plain file stem as its name and
  gets no filename link. Alone, that shape reads just as well as a person's own
  `Cloaks Collection-12-3.zip`, and a wrong link aims update checks and `pin_link` at an
  unrelated mod. The MD5 identify still finds a genuine one.
- **catalog, copies:** the same archive in two roots (same package id) is kept once:
  the first readable copy, else the first copy. A reader panic marks that package
  `Unreadable`.
- **catalog, Downloads-style roots:** non-DD2 candidates are remembered by
  `(path, size, modified_ms)` in `CatalogCache.passed_over` and passed over unopened
  while that stamp holds. **A candidate there that cannot be opened is passed over
  unshown and tried again next scan** (main-session ruling). Nothing says it is a mod,
  and the folder holds the player's private files too, such as a password-protected zip
  of papers (`ScanStats.unreadable_passed_over`). `from_cache` drops whatever a
  `dd2_shaped_only` root would pass over, so the startup preview matches the scan.
- **conflicts:** one game file, one conflict. A hash conflict that names a `natives/`
  path and holds every layer of that path's conflict, with the same winner, replaces
  the path conflict.
- **cache:** `CatalogCache::SCHEMA` = 4 (2: nested variants, the new sentences and
  folder stamps; 3: the singular `OutdatedFormats` sentence; 4: the vouched very old
  names).

---

## § rift-core additions (owned by the catalog builder, small)

The catalog builder adds `crates/rift-core/src/nexus_name.rs` (and `pub mod nexus_name;`
in `rift-core/src/lib.rs` — the one permitted edit of the contract crate):

```rust
pub struct NexusFileName { pub name: String, pub mod_id: u64, pub version: Option<String>, pub uploaded_ms: Option<u64>, pub sqid: Option<String> }
pub fn parse(file_name: &str) -> Option<NexusFileName>
```

Two shapes (§ rift-nexus carries the researched description — authoritative):
- classic `<Name>-<modid>-<version with - for .>-<unix seconds>.<ext>`
  (`Lantern Glow-123-1-2-1711234567.zip` → `Lantern Glow`, 123, `1.2`, 1711234567000);
  very old names carry no timestamp (`Lantern Glow-123-1-2.zip`);
- 2026 `<Name> <modid> <version> <YYYY-MM-DDTHH-MMZ> <sqid>.<ext>`
  (`Vernworth Cloaks 2207 8 2026-10-14T14-23Z Rk4vLmN2Q.zip` → `Vernworth Cloaks`, 2207, `8`,
  2026-10-14T14:23Z, sqid `Rk4vLmN2Q`) — the timestamp is the file's UPLOAD time and the
  last token is the file's `sqid`, both used to pin the exact Nexus file;
- either shape may carry a browser duplicate suffix ` (1)` before the extension.
Anything else → `None`. Tests for every shape, plus names containing hyphens and digits.
*(Main session note: `NexusFileName` gains `pub sqid: Option<String>` — added after
wave A if the builder used the earlier draft.)*

---

## § rift-deploy

**Owns:** `crates/rift-deploy/**`. The most dangerous crate: it writes into the game
folder. Every rule below that says "refuse" is a test.

### Modules and API

```rust
// game.rs — find Dragon's Dogma 2 (RD: discovery, not configuration)
pub const DD2_STEAM_APP_ID: &str = "2054970";
pub struct GameInstall { pub root: PathBuf, pub build_id: Option<String>, pub source: GameSource }
pub enum GameSource { Steam, Env, Manual }
pub fn discover_game() -> Option<GameInstall>;
pub fn discover_game_from(steam_root: Option<&Path>) -> Option<GameInstall>; // testable core
pub fn validate_game_root(path: &Path) -> Result<GameInstall, RiftError>;  // manual pick: needs DD2.exe
pub fn read_build_id(game_root: &Path) -> Option<String>; // via the library's appmanifest_2054970.acf
pub mod vdf; // minimal Valve KeyValues text parser (quoted keys/values, braces, escapes) — libraryfolders.vdf + .acf
```
Discovery order: env `RIFTSTONE_GAME_ROOT` (bench + manual override) → on Windows the
registry `HKCU\Software\Valve\Steam\SteamPath`, then
`HKLM\SOFTWARE\WOW6432Node\Valve\Steam\InstallPath` → env `RIFTSTONE_STEAM_ROOT` (the
WSL bench: `/mnt/c/Program Files (x86)/Steam`) → each library in
`steamapps/libraryfolders.vdf` whose `apps` holds `2054970` → its
`steamapps/appmanifest_2054970.acf` `installdir` → `<lib>/steamapps/common/<installdir>`.
Valid root = contains `DD2.exe` (case-insensitive).

```rust
// reframework.rs — read REFramework's own files (never write them, except the one
// explicit switch action below)
pub struct RefStatus {
    pub installed: bool,                 // dinput8.dll present
    pub version: Option<String>,         // "v1.5.9.1+514" = log "Tag:" + "+" + "Commits past tag:"
    pub build_date: Option<String>,      // log "Build date:"
    pub loose_file_loader: Option<bool>, // re2_fw_config.txt LooseFileLoader_Enabled
    pub pak_directory: Option<bool>,     // re2_fw_config.txt IntegrityCheckBypass_LoadPakDirectory
    pub faulty_files: Vec<String>,       // reframework_faulty_files.txt, non-log lines
}
pub fn read_status(game_root: &Path) -> RefStatus;
/// Set one switch in re2_fw_config.txt (backup first, refuse while DD2.exe runs —
/// REFramework rewrites its config on exit). Keys limited to the two above.
pub fn set_switch(game_root: &Path, key: RefSwitch, on: bool, game_running: bool) -> Result<(), RiftError>;
pub enum RefSwitch { LooseFileLoader, PakDirectory }
```

```rust
// fluffy.rs — Fluffy is a neighbour (RS-5): read, never write
pub struct FluffyInstall { pub root: PathBuf, pub game_dir: PathBuf, pub mods_dir: PathBuf, pub installed_ini: Option<PathBuf> }
pub fn discover_fluffy() -> Option<FluffyInstall>;
pub fn discover_fluffy_in(candidates: &[PathBuf]) -> Option<FluffyInstall>; // testable core
pub struct FluffyLedger { pub game_install_path: Option<String>, pub mods: Vec<FluffyMod> }
pub struct FluffyMod { pub section: String, pub name: String, pub files: Vec<String> } // files normalized, '/' separators
pub fn read_ledger(installed_ini: &Path) -> Result<FluffyLedger, RiftError>;
```
Discovery: env `RIFTSTONE_FLUFFY_ROOT` → a bounded search for a folder holding
`Modmanager.exe` (case-insensitive) AND `Games/DragonsDogma2/`: every existing drive
root's children and grandchildren (skip `Windows`, `Program Files*`, `ProgramData`,
`$Recycle.Bin`, `System Volume Information`, `Users`), and the user's `Downloads`,
`Desktop`, `Documents`, `Games` to depth 3. Cap the whole search at 4,000 directories
visited. (Fluffy is portable: it runs from wherever its zip was unpacked, so there is no
one place to look.)

```rust
// journal.rs — the write-ahead journal (RS-4), JSON in the app data dir
pub struct Journal { pub schema: u32, pub game_root: PathBuf, pub installs: Vec<InstalledOption>,
                     pub pak_order: Vec<OptionId>, pub pending: Option<Intent>, pub next_seq: u64 }
pub struct InstalledOption { pub seq: u64, pub option_id: OptionId, pub name: String,
    pub package_file: String, pub package_path: PathBuf, pub package_size: u64,
    pub option_root: String, pub version: Option<String>, pub installed_at: String,
    pub game_build: Option<String>, pub origin: Origin, pub files: Vec<DeployedFile> }
pub enum Origin { Riftstone, AdoptedFromFluffy { section: String } }
pub struct DeployedFile { pub game_path: String, pub kind: DeployKind, pub size: u64,
    pub entry: Option<String>, pub below: Below }
pub enum DeployKind { Pak, Loose, Reframework, Root }
/// What sat at this path before this install wrote it.
pub enum Below { Nothing, Backup { store: String, owner: BelowOwner }, UnknownVanilla }
pub enum BelowOwner { Unowned, Fluffy { section: String }, Riftstone { option_id: OptionId } }
pub struct Intent { pub action: IntentAction, pub seq: u64, pub option_id: OptionId,
                    pub touched: Vec<String>, pub started_at: String }
impl Journal { pub fn load(dir: &Path, game_root: &Path) -> Result<Journal, RiftError>;
               pub fn save(&self, dir: &Path) -> Result<(), RiftError>; } // temp file + rename
```

```rust
// engine.rs — plan, install, uninstall, reorder, adopt, recover
pub struct DeployContext<'a> { pub game: &'a GameInstall, pub state_dir: &'a Path,
    pub ref_status: &'a RefStatus, pub fluffy: Option<&'a FluffyLedger>, pub game_running: bool }
pub fn plan_install(ctx: &DeployContext, journal: &Journal, package: &Package, option: &ModOption) -> InstallPlan;
pub fn install(ctx: &DeployContext, journal: &mut Journal, package: &Package, option: &ModOption,
               source: &mut dyn PackageSource, progress: &mut dyn FnMut(ExtractProgress)) -> Result<InstallReport, RiftError>;
pub fn plan_uninstall(ctx: &DeployContext, journal: &Journal, option_id: &OptionId) -> UninstallPlan;
pub fn uninstall(ctx: &DeployContext, journal: &mut Journal, option_id: &OptionId) -> Result<UninstallReport, RiftError>;
pub fn reorder_paks(ctx: &DeployContext, journal: &mut Journal, order: &[OptionId]) -> Result<(), RiftError>;
pub fn adopt_fluffy(ctx: &DeployContext, journal: &mut Journal, ledger: &FluffyLedger,
                    match_option: &dyn Fn(&FluffyMod) -> Option<(OptionId, String)>) -> Result<AdoptReport, RiftError>;
pub fn recover(ctx: &DeployContext, journal: &mut Journal) -> Result<Option<RecoveryReport>, RiftError>;
pub fn is_game_running() -> bool; // DD2.exe in the process list (Windows); false elsewhere
// state.rs — what is in the game right now
pub fn read_game_state(ctx: &DeployContext, journal: &Journal) -> GameState;
// preflight.rs
pub fn preflight(state: &GameState) -> Vec<Check>;
```
`InstallPlan`, `UninstallPlan`, `InstallReport`, `UninstallReport`, `AdoptReport`,
`RecoveryReport`, `GameState`, `Check` — serde camelCase structs the builder designs to
carry: every planned write (`entry → game_path`, kind, size, action = `create` /
`overwrite-riftstone {option name}` / `overwrite-fluffy {section}` / `overwrite-unowned`
/ `reinstall`), warnings (`pakFolderOff`, `looseLoaderOff`, `reframeworkMissing`,
`gameRunning`, `requiresMissing {name}`, `conflicts {option names}`), and a `refused`
reason when the plan cannot run. `GameState`: install + build, RefStatus, the journal's
installs, Fluffy's unadopted mods, every `pak_mods/*.pak` with its owner, strays (files in
`pak_mods/`, `natives/`, `reframework/autorun/`, `reframework/plugins/` owned by nobody),
files the journal says are installed but are gone, and `buildChanged` (the build id now vs
the one recorded at the newest install). `Check`: `{ id, level: ok|info|warn|error,
title, detail, action? }`.

### The rules (each one is a test)

1. **Target guard** (`guard.rs`, used by every write): game paths are normalized
   (`rift_core::normalize_entry_path`); the only writable places are `natives/**`,
   `reframework/**`, `pak_mods/*.pak`, and files directly in the game root. **Refused
   outright:** `DD2.exe`, any `*.exe` in the root, `re_chunk_000.pak` and every
   `re_chunk_000.pak.*`, everything under `dlc/`, `steam_api64.dll`, `steam_appid.txt`,
   `shader.cache2`, `config.ini`, `config_default.ini`. The joined path must stay inside
   the game root (check the canonical parent).
2. **Backups before overwrites.** A target that exists and is not this install's own:
   move (rename; copy+delete across volumes) the existing file into
   `<state_dir>/backups/<seq>/<game_path>` BEFORE the new bytes land; record `Below::Backup`
   with its owner (another Riftstone install, a Fluffy section per its ledger, or
   unowned). A target that does not exist → `Below::Nothing`.
3. **Write-ahead.** *(As built after the safety review, 2026-10-16: write-ahead → extract every
   part → fingerprint the parts (SHA-256 of size + first and last MiB) and save the
   fingerprints into the intent → per file move the original into backups/ and rename the
   part in. Undo deletes a file at a target only on proof — no part left AND size AND
   fingerprint match. A reinstall is one intent covering both halves.)*
   `install`/`uninstall` save the journal with `pending = Intent` first,
   then move bytes, then save with the install recorded and `pending = None`. Extraction
   writes `<dest>.riftstone-part` and renames into place per file. Any error: undo this
   intent's files (delete parts and finals it created, restore its backups), clear
   `pending`, save, return the error. `recover` does the same at launch for a stale
   `pending`, and reports what it rolled back.
4. **The overwrite stack.** Uninstalling option X from path P: if X is the top layer at P —
   `Below::Nothing` → delete P (only if its size still matches what X wrote; otherwise
   leave it and report "changed by something else"); `Below::Backup` → move the backup back
   into place; `UnknownVanilla` → leave P, report it. If X is NOT the top layer, the layer
   Y directly above X inherits X's `below` (X's backup file moves into Y's slot, Y's old
   backup — X's bytes — is deleted). Test: A, B, C over one path, uninstall in every order,
   assert the bytes at P and the store end clean. *(As built: a top-layer file changed by
   something else stays in place and the backup under it is PARKED on record —
   `Journal.parked`, a `parkedBackups` preflight check with `CheckAction::ReviewParked`,
   `restore_parked` / `discard_parked` — instead of being moved back over the change. A
   hand-down deletes the superseded slot only when it holds the lower layer's recorded size
   and fingerprint.)*
5. **Paks** go to `pak_mods/` (created on demand) as `<prefix>-<original file name>`; the
   prefix scheme and the load-order semantics are in § The DD2 load rules below. A pak
   never overwrites another pak: names are unique by prefix. `reorder_paks` renames
   files only (two-phase: to temporary names, then final names, journaled).
6. **Fluffy adoption.** For each `FluffyMod` not yet adopted: record an
   `InstalledOption` with `Origin::AdoptedFromFluffy`, its files as listed (skip and report
   files that no longer exist), `below = Nothing` for files under `natives/`,
   `reframework/`, `pak_mods/` and `dinput8.dll`, `UnknownVanilla` for any other root
   file. Adopted paks join `pak_order` in their current file-name order. Never write
   `installed.ini`. Report: adopted, already adopted, files missing.
7. **Never delete what you do not own.** Uninstall touches only files recorded in the
   journal. Strays are reported, never removed.
8. **Game running:** `plan_install` for a pak while DD2 runs → warning (it loads at next
   launch); `uninstall`/`reorder_paks` of paks while it runs → refused (the files are
   open). Loose files may install while it runs (REFramework's loose loader).
9. **Requirements:** `requires=` names (resolved by the caller to option ids or not) that
   are not installed → `requiresMissing` warning, never a refusal.

### § The DD2 load rules (researched 2026-10-16 in REFramework's source; confirmed in the investor's own log)

Constants live in `rift_core::dd2` — use them, never re-type the values.

- **pak_mods is loaded in file-name order.** REFramework iterates `pak_mods/` with
  `recursive_directory_iterator` (NTFS: alphabetical) and injects each `.pak` into the next
  free `re_chunk_000.pak.sub_000.pak.patch_NNN` slot; a later slot overrides an earlier
  one. The investor's log shows exactly that: `x0000-…` → `patch_001` (slot 103),
  `x0001-…` → `patch_002` (slot 104). **The file-name order IS the load order; the last
  name wins.**
- **Names the Riftstone writes:** `zrift-NNNN-<stem>.pak` (`dd2::PAK_PREFIX`; NNNN = a
  4-digit number above every `zrift-` pak already in `pak_mods/` or the journal, and at least
  the pak order's length — numbering by position would break load order after a gap;
  `reorder_paks` renumbers from 0000 — built 2026-10-16, F14; stem = the original pak file stem with any `xNNNN-` or
  `zrift-NNNN-` prefix stripped and Windows-illegal characters (`<>:"/\|?*`, controls)
  replaced by `_`, trimmed of trailing dots/spaces, capped at 120 chars). It sorts after
  Fluffy's `xNNNN-` names, so a new install wins over Fluffy's leftovers. Always lowercase
  `.pak` — REFramework's extension check is case-sensitive. Keep `pak_mods/` FLAT (the
  iterator recurses, so subfolders would load in an order nobody chose).
- `reorder_paks(order)`: the given order (lowest priority first) becomes `pak_order`;
  every managed pak (Riftstone-installed AND adopted) whose name does not match its
  target `zrift-<pos>-<stem>.pak` is renamed — two phases (all to
  `.riftstone-reorder-<n>` temporaries, then to finals), journaled as one intent.
- **Loose beats pak.** REFramework's LooseFileLoader answers the engine's path lookup
  for any path that exists on disk, so a loose `natives/` file wins over every pak,
  `pak_mods` included. (Already the conflicts rule.)
- **Restart required.** Both the pak list and the loose-file existence cache are read at
  game start. `InstallReport`/`UninstallReport` carry `restart_needed: bool` = the game
  is running.
- **Never** write `patch_NNN` paks into the game root, never touch `re_chunk_000.pak*`,
  the 144-byte stubs, `_storage_/` (REFramework's DLL cache) — all in
  `dd2::NEVER_WRITE`, which the guard enforces on top of rule 1.
- **Prerequisites:** pak payload → REFramework installed and `pak_directory != false`
  (default is true when the key is absent); loose payload → `loose_file_loader == true`
  (default false when absent). Missing → plan warnings (not refusals).
- **Pak count:** more than `dd2::PAK_COUNT_WARNING` paks in `pak_mods/` → a preflight
  warning (REFramework hands out slots without a bounds check; MH Wilds crashed at 130).
- **Did it load?** `reframework::read_pak_load_report(game_root) -> Option<PakLoadReport>`
  parses `re2_framework_log.txt`: lines `[IntegrityCheckBypass]: Cached custom pak with
  name: <file> at path: <path>` and `[IntegrityCheckBypass]: injecting <path> as
  native-form '<native>' (…, slot=<n>, …)`, plus any line containing `NONE were injected`.
  `PakLoadReport { log_started: Option<String> /* first line's timestamp */, cached:
  Vec<String>, injected: Vec<InjectedPak { file_name, native_name, slot }>, error:
  Option<String> }`. `GameState` carries it, and per managed pak: `loaded` (injected in
  the latest log), `pending` (installed after the log started — loads at next launch), or
  `not_loaded` (installed before the last launch, absent from the injected list).
- **Fluffy's crash-reporter rename:** `GameState.crash_reporter_renamed` = root has
  `CrashReport_FMMBACKUP.exe` and no `CrashReport.exe` → a preflight `info` check (Steam's
  file verify restores it; the Riftstone does not touch it).
- **Outdated assets:** the catalog flags loose files with `dd2::outdated_suffix` as an
  `OutdatedFormats` problem; `plan_install` surfaces it as a warning
  `outdatedFormats { count }`.

**Tests:** everything above against a FAKE game root in a tempdir (`DD2.exe` as an empty
file, `re_chunk_000.pak` as a small file, a `dlc/` folder) and a fake `PackageSource`
(in-memory map) — plus one round trip through `rift_archive::open` on a zip fixture.
Hostile cases: a payload targeting `../outside`, `DD2.exe`, `dlc/x.pak`,
`re_chunk_000.pak.patch_016.pak` via the root — all refused, nothing written. Crash
simulation: a source that fails on the 3rd file → nothing of the install remains, backups
restored, `pending` cleared. `recover` on a journal saved mid-intent. VDF parser on a real
`libraryfolders.vdf` shape and an `.acf`. Fluffy ledger parse of the documented shape
(both slash styles, a section with no files). RefStatus from fixture log/config files.
**Never point a test at the real game folder.**

**Bench (`#[ignore]`, env `RIFT_BENCH_GAME=<root>` and `RIFT_BENCH_FLUFFY=<root>`):**
READ-ONLY — `discover_game_from`, `read_status`, `read_ledger`, `read_game_state` against
a journal in a tempdir; print the result. No install, no uninstall, no switch, ever, in a
bench test.

## § rift-nexus

**Owns:** `crates/rift-nexus/**`.

**The research (2026-10-16, tested against the live API — cite as fact):**

- **Nexus's GraphQL v2 (`POST https://api.nexusmods.com/v2/graphql`) serves public mod
  data with NO API key.** Dragon's Dogma 2 is `gameDomain "dragonsdogma2"`, numeric
  `gameId 6234`. So update checks need no key and no account: the investor's ask works
  zero-config.
- Batched mod lookup (200 ids per call worked; results are NOT in request order — key by
  `modId`):
  ```graphql
  query($ids: [CompositeDomainWithIdInput!]!) {
    legacyModsByDomain(ids: $ids, count: 200) {
      totalCount nodes { modId name version updatedAt status } } }
  # variables: {"ids":[{"gameDomain":"dragonsdogma2","modId":2207}, …]}
  ```
- A mod's files (use GraphQL aliases to fetch several mods in one POST):
  ```graphql
  { m2207: modFiles(modId: 2207, gameId: 6234) {
      fileId groupId sqid name version category date primary sizeInBytes } }
  ```
  `category` enum: `MAIN, UPDATE, OPTIONAL, OLD_VERSION, MISCELLANEOUS, REMOVED, ARCHIVED`.
  `date` is unix seconds of the upload. **Every version of one logical file shares a
  `groupId`** (observed: one armour pack's v1…v8 share a group) — undocumented, so treat
  it as the primary signal with the fallbacks below.
- MD5 lookup, anonymous, across ALL games — filter to `gameId == 6234`:
  `fileHashes(md5s: [String!]!)` (and singular `fileHash(md5:)`). Ask the schema (an
  introspection query, or try a minimal selection and read the error) for the exact
  return fields; expect the file's `fileId`, `modId`, `gameId`, `version`, `name`.
  Several hits for one hash are normal (re-uploads).
- Send `Application-Name: Riftstone`, `Application-Version: <crate version>` and a
  descriptive `User-Agent` on every request (Nexus's acceptable-use policy). v2
  anonymous carries no documented rate limit: still batch, cache, never poll in a loop,
  and back off on HTTP 429 (honour `Retry-After` when present).
- **v1 REST** (`https://api.nexusmods.com/v1`, header `apikey`) is optional. Personal
  API keys are allowed for personal tools. v1 limits: 20,000/day then 500/hour, reported
  in `X-RL-Daily-Remaining` / `X-RL-Hourly-Remaining`. v1 gives `files.json` with a
  `file_updates` chain (`old_file_id → new_file_id`) — the fallback when `groupId`
  misleads. v1 is NOT required for v0.1; implement only `validate_key`
  (`GET /v1/users/validate.json`, free of the hourly limit) and `files_v1(mod_id)` behind
  an `Option<api_key>`.
- **Download names** (decoded live): 2026 shape
  `{name} {modId} {version} {YYYY-MM-DDTHH-MMZ} {sqid}.ext` — the timestamp is the FILE'S
  UPLOAD time, the last token is that file's `sqid` (not random). Classic shape
  `{name}-{modId}-{version, dots as hyphens}-{unix seconds}.ext`, and very old names
  without a timestamp (`Little Falinesti-23570-1-3.zip`). Browser duplicates append
  ` (1)` before the extension.

**API (all blocking — the window crate calls them from a worker thread):**

```rust
pub const GAME_DOMAIN: &str = "dragonsdogma2";
pub const GAME_ID: u64 = 6234;

pub struct NexusClient { /* reqwest::blocking::Client, endpoints (overridable for tests), app version, Option<api key> */ }
impl NexusClient {
    pub fn new(app_version: &str) -> Self;
    pub fn with_endpoints(self, graphql_url: &str, v1_base: &str) -> Self; // tests point at a local server
    pub fn with_api_key(self, key: Option<String>) -> Self;
    pub fn mods(&self, mod_ids: &[u64]) -> Result<Vec<NexusMod>, NexusError>;          // chunks of 200
    pub fn mod_files(&self, mod_ids: &[u64]) -> Result<BTreeMap<u64, Vec<NexusFile>>, NexusError>; // aliased, ≤ 20 mods per POST
    pub fn mod_files_report(&self, mod_ids: &[u64]) -> Result<ModFilesReport, NexusError>; // + per-mod refusals (built 2026-10-16)
    pub fn files_by_md5(&self, md5s: &[String]) -> Result<Vec<Md5Hit>, NexusError>;   // game 6234 only
    pub fn validate_key(&self) -> Result<NexusAccount, NexusError>;                    // v1, needs a key
}
pub struct NexusMod { pub mod_id: u64, pub name: String, pub version: Option<String>, pub updated_ms: Option<u64>, pub status: String }
pub struct NexusFile { pub file_id: u64, pub group_id: Option<u64>, pub sqid: Option<String>, pub name: String, pub version: Option<String>, pub category: FileCategory, pub uploaded_ms: u64, pub primary: bool, pub size_bytes: Option<u64> }
pub enum FileCategory { Main, Update, Optional, OldVersion, Miscellaneous, Removed, Archived, Unknown }
pub struct Md5Hit { pub mod_id: u64, pub file_id: u64, pub version: Option<String>, pub name: String, pub size_bytes: Option<u64> }
pub struct NexusAccount { pub name: String, pub is_premium: bool }
pub enum NexusError { Offline(String), RateLimited { retry_after_s: Option<u64> }, KeyRejected, BadResponse(String), Http(u16) }

/// md5 of a file, streamed (packages reach several GB). Hex lowercase.
pub fn md5_file(path: &Path, progress: &mut dyn FnMut(u64)) -> std::io::Result<String>;

/// Pin a package to its exact Nexus file. Exact identifiers first and final: sqid (when the
/// list carries sqids), then file_id (MD5 hits, pasted file URLs); a listed match pins (the
/// sqid wins a disagreement). An identifier the list no longer holds means the file is gone:
/// a link that already carries its file_id comes back UNCHANGED (update_state then reports it
/// retired via the remembered group_id); a sqid-only link gets None. Only links with no exact
/// identifier fall back to the upload timestamp (±60 s) and then the version string. Fills
/// file_id + group_id + uploaded_ms + version. None → the window offers MD5 or a pasted URL.
pub fn pin_link(link: &NexusLink, files: &[NexusFile]) -> Option<NexusLink>;

/// Decide one package's update state from its pinned link and the mod's current files.
pub fn update_state(link: &NexusLink, mod_info: Option<&NexusMod>, files: &[NexusFile]) -> UpdateState;
pub enum UpdateState {
    Current,                                   // no newer file in the group
    UpdateAvailable { newer: NexusFile },      // newest non-archived/removed/old file in the same group, later than the local one
    LocalArchived { newer: Option<NexusFile> },// retired by the author AND nothing in its group replaces it; newer = a cross-group suggestion
    Unpinned,                                  // we know the mod but not the exact file
    ModUnavailable { status: String },         // hidden / removed / under moderation
}

/// Parse a pasted URL: `https://www.nexusmods.com/dragonsdogma2/mods/2207?tab=files&file_id=2391`
/// → (mod 2207, file 2391); a bare number → mod id. Other games' URLs → None.
pub fn parse_mod_url(text: &str) -> Option<(u64, Option<u64>)>;
pub fn mod_page_url(mod_id: u64) -> String; // https://www.nexusmods.com/dragonsdogma2/mods/{id}
```

**Ruled 2026-10-16 (main session):** a retired local file (ARCHIVED / REMOVED / OLD_VERSION)
whose group holds a newer live file is `UpdateAvailable { newer: successor }` — that is the
answer the player needs ("a newer version of what you have exists"); `LocalArchived` is kept
for a retired file nothing in its group replaces.

Update rule (exact): within the files of the local file's `group_id`, ignore `REMOVED`,
`ARCHIVED`, `OLD_VERSION`; the newest by `uploaded_ms` that is newer than the local
file's `uploaded_ms` (or has a higher `file_id` when dates tie) is the update. No group
id → fall back to: same `name` (case-insensitive) as the local file. Never compare
mod-level version strings (the investor's own pack shows mod version "1" while its file
is "8").

**Tests:** spin a tiny local HTTP server in the test (std `TcpListener` on 127.0.0.1:0,
one thread, canned JSON responses recorded from the shapes above — write them by hand, do
not call the live API from `cargo test`); assert the request carries
`Application-Name`/`Application-Version`; batching (450 ids → 3 POSTs); unordered results
keyed correctly; aliased modFiles parse; md5 hits filtered to 6234; 429 →
`RateLimited`; pin_link by sqid / by timestamp / by version; update_state cases (newer in
group, newer only in another group = Current, local archived, mod hidden); parse_mod_url
variants; md5_file on a tempfile against a known digest.
**Bench (`#[ignore]`, env `RIFT_BENCH_NEXUS=1`):** ONE live `mods(&[id])` + `mod_files(&[id])` for the mod id in the env var
call, print the parsed result. The main session runs it once.

## § src-tauri (integration)

**Owns:** `src-tauri/**` (except `icons/`, `tauri.conf.json` window/bundle blocks may be
edited only for capability/plugin needs).

**Contract:** `docs/WINDOW-CONTRACT.md` (every command, event, the startup sequence) and
`src/types/rift.ts` (every shape, authoritative). Mirror the TS types in
`src-tauri/src/dto.rs` with `#[serde(rename_all = "camelCase")]` and tagged enums
(`#[serde(tag = "kind", rename_all = "camelCase")]` + per-variant `rename_all`) so the JSON
matches the TS unions EXACTLY. Add a `dto` round-trip test per union that serializes a
sample and asserts the JSON keys the TS side reads.

**Modules (suggested):** `state.rs` (AppState: `RwLock<Catalog>`, `Mutex<Journal>`,
game/ref/fluffy discovery results, settings, roots, nexus cache, scan flag), `persist.rs`
(JSON files in `app_data_dir()`: `settings.json`, `roots.json`, `catalog-cache.json`,
`nexus.json`; the journal and `backups/` belong to rift-deploy under the same dir;
`thumbs/` in `app_cache_dir()`; every write = temp file + rename), `commands/*.rs` (thin:
lock, call a crate, map to a DTO), `dto.rs`, `thumbs.rs` (decode with `image`, resize to
360 px / 1600 px wide max with `thumbnail`/`resize` Triangle, JPEG q≈82, base64
`data:image/jpeg;base64,` URL, disk cache keyed `<package id>-<fnv(entry)>-<size>.jpg`),
`watch.rs` (notify, recursive on each root, 1.5 s debounce → rescan), `conflicts.rs`
(a pak whose TOC read fails with `PakError::TooManyEntries` counts as unreadable; a pak with
`toc.unknown_flags() != 0` keeps its hashes but its conflicts are shown as unconfirmed)
(build `rift_catalog::Layer`s from the journal installs in install order + Fluffy's
unadopted mods; pak hashes read from `pak_mods/` files on disk with `rift_pak` (head +
TOC only; cache by file name + size + mtime); for a not-installed option in
`get_option`, read its pak TOCs from the archive with `read_entry(limit = toc_len)`),
`nexus.rs` (link store per package id: the filename/homepage link from the catalog,
pinned via `pin_link`, MD5 links, manual links; update results; last check; runs on a
worker thread), `keys.rs` (`keyring = "4"`, service `nl.zmuuzn.riftstone`, account
`nexus-api-key` — `Entry::new`, `set_password`, `get_password`, `delete_credential`,
`keyring::Error::NoEntry` = absent).

**Threading:** commands that touch disk heavily (`install`, `uninstall`, `reorder_paks`,
`adopt_fluffy`, `check_updates`, `identify_packages`, `get_thumbnail`, `get_package` of
a 7z/rar) are `async` commands that move their work into
`tauri::async_runtime::spawn_blocking`. Scans run on a `std::thread` and report through
events. Never hold the catalog write lock while opening archives: scan into a fresh
`Catalog`, then swap it in.

**Mappings that matter:**
- `SearchResult` = a `SearchHit` + the option's card fields + `installed` (journal →
  `riftstone` / `fluffy` origin; Fluffy ledger name match → `fluffyUnadopted`) +
  `update` (package link state: `available` / `current` / `unknown`; `null` unlinked) +
  `conflicted` (the option is a layer in any current conflict).
- Fluffy matching (adoption and badges): a `FluffyMod` matches the library option whose
  `name` equals its `name` (`ModName=`), else whose option-root last segment equals its
  `section`, case-insensitive; ties → the most recently modified package.
- `Overview.checks` = `rift_deploy::preflight(&state)` + the window crate's own (no game
  found → `pickGameFolder` action; scan problems summary).
- `remove_root` / `add_root` persist `roots.json`; default roots are created only on the
  first launch (Fluffy Mods folder when found; Downloads, `dd2_shaped_only = true`).
- `get_thumbnail` for `card` on an option without a screenshot → `null` (the window draws
  its own placeholder).
- Every command error → `RiftError` (serialize as rift-core does). Never `unwrap()` on a
  lock or a user path in a command.

**Env overrides for the WSL bench** (read once at startup, documented in the journal):
`RIFTSTONE_GAME_ROOT`, `RIFTSTONE_STEAM_ROOT`, `RIFTSTONE_FLUFFY_ROOT`,
`RIFTSTONE_DATA_HOME` (replaces `app_data_dir()` + `app_cache_dir()` — tests and the bench
never write into the real app data).

**Tests:** dto shape tests; a command-layer test harness that builds `AppState` over a
tempdir (fake game root, fake library with a zip collection) WITHOUT a Tauri runtime
(keep command bodies as plain functions `fn get_overview_impl(state: &AppState) -> …`
that the `#[tauri::command]` wrappers call) and drives: scan → search → plan_install →
install → get_installed → uninstall. Gates: `cargo clippy -p riftstone --all-targets -- -D
warnings`, `cargo test -p riftstone`, and `cargo build -p riftstone`.

## § The Window

*(the Artisan's direction, then the Illusionist)*

---

## § v0.2 — Nexus labels in the overview (investor ask, 2026-10-16)

*"is it also possible to use the tags from nexus for our overview? so that i can easily filter
on those tags?"* — yes. Measured live 2026-10-16 (anonymous v2): every `Mod` carries
`category: String!` (always present, e.g. `Armor`, `Visuals`, `Utilities`), `tags: [LegacyTag!]!`
(`{ name }`, often empty, e.g. `Utilities for Players`) and `adultContent: Boolean`. They arrive
in the SAME batched `legacyModsByDomain` call the update check already makes — zero extra
requests. Labels are per Nexus MOD: every option of a linked package inherits them.

- **rift-nexus:** the `mods()` selection adds `category tags { name } adultContent`;
  `NexusMod` gains `category: Option<String>`, `tags: Vec<String>`, `adult: bool`
  (`#[serde(default)]` so old caches load).
- **rift-catalog:** `SearchFilters` gains `categories: Vec<String>` (ANY-of — a mod has one
  category) and `tags: Vec<String>` (ALL-of — each tag narrows); `SearchIndex` gains
  per-package labels (`set_labels(HashMap<PackageId, PackageLabels { category, tags }>)`),
  searchable as a field `nexusLabel` (weight 4, between bundle and author) and used by the
  two filters; `MatchedField::NexusLabel`. Labels compare case-insensitively; display keeps
  Nexus's spelling.
- **spine:** the Nexus store keeps each linked mod's labels from the latest check (persisted in
  `nexus.json`), feeds them to the index after every check and scan, and serves
  `get_labels() -> LabelFacets { categories: LabelCount[], tags: LabelCount[], unlabeledPackages:
  number }` (`LabelCount { name, options }` — option counts, so a facet predicts the results).
  `SearchResult` gains `nexusCategory: string | null`, `nexusTags: string[]`, `adult: boolean`;
  `OptionDetail` shows them via `update.link` + the same fields.
- **window:** a NEXUS facet group in the stone below STATE — CATEGORY chips (any-of) and TAG
  chips (all-of) with counts; an unlinked-packages line with the one-click identify; the card
  carries the category as a quiet chip; the detail lists category, tags and the mod-page link.
  Settings: the veil gains a mode — `off` / `adult` (only mods Nexus flags adult) / `all`.

### v0.2 backlog beside the labels (from the deploy safety review, 2026-10-16)

- **Parked backups in the window:** `InstalledView.parked: ParkedFile[]` ({store, gamePath,
  owner, ownerKind, size | null, parkedAt, reason, occupied}); `UninstallResultView.parked` and
  `RecoveryNote.parked`; commands `restore_parked(store) -> ParkedRestoreReport` ({gamePath,
  restored, swappedOut | null, restartNeeded}) and `discard_parked(store)`, both followed by
  `rift://game-changed`; `CheckAction` gains `{ kind: "reviewParked" }`; `RecoveryNote.action`
  gains `"restoreParked"`.
- **rift-archive defence in depth:** the extractor opens destinations with
  `OpenOptions::create_new(true)` (the engine already pre-checks and re-guards part names; this
  closes the live race between that check and `File::create`).
