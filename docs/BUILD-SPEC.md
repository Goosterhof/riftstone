# The Riftstone — Build Spec

*The single source of truth the builders work from. Each section names the
crate, the files it owns, the contract it must honour, and the tests that
prove it. Builders edit ONLY the files their section owns. The base sections
(from "What the Riftstone is" through § The Window) describe the system as built
through v0.3. § v0.2 and § v0.3 keep each version's own build record: its
rulings, its builders, and what they reported.*

## What the Riftstone is (one paragraph, for every builder)

A Windows desktop mod manager (Tauri v2, Rust + Vue 3) for **Dragon's Dogma 2**,
built for one player. It began with three asks, in the investor's words: *"one
specific for dragons dogma 2"*, *"search through the nested directories/zip"*,
and *"linked to nexus, so the [mod manager] can check if there are updates"*.
The evidence behind the second ask: one archive in the investor's library (an
armour-variants collection, 268 MB) holds **281 installable options** (each with
its own `modinfo.ini`), 2,674 files, folders 17 deep, wired into nested
`addonfor` menus. The investor had unpacked it by hand to dig through it in
Explorer. Since v0.3 the library is one folder, `<install dir>\mods`, beside the
installed `riftstone.exe`. The Rift opens on a tree of its packages, and a
search shows every match. A Play button starts the game. A personal Nexus key
shows the account, downloads into the mods folder (premium in the stone; free
through the site's Mod Manager Download link and an opt-in nxm handler), and
tracks and endorses mods.

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
- **Game-folder layout of installed mods:** `pak_mods/<name>.pak` (other tools give a pak a
  numbered priority prefix, `xNNNN-<name>.pak`; *the file-name order is the load order*),
  `natives/stm/...` loose files, `reframework/autorun/*.lua`, `reframework/data/...`,
  `reframework/plugins/`, `dinput8.dll`.
- **The mods folder** (v0.3): `<install dir>\mods`, beside the installed `riftstone.exe`. The
  investor moved the mods there (the v0.3 ask, 2026-10-17). It is the stone's only library.
- **Packaging, not payload:** a mod's `modinfo.ini`, its screenshot and root-level
  `README*.txt` never go into the game folder (classify rule 3 skips them).
- **modinfo.ini** dialect (the format's own readme and change log, and observed files): `key=value`
  (also `key: value`), keys case-insensitive, CRLF. Keys: `name`, `description`,
  `author`, `version`, `screenshot`, `category`, `homepage`, `addonfor` (hide under the
  option NAMED so), `NameAsBundle` (group siblings into one bundle), `requires` /
  `requirement`, `DummyMod=True` (menu entry only), `MenuPriority` (int, higher first).
  Collections chain menus: `Tops` (a DummyMod) ← `Ermine Cape (Menu Option)`
  (`addonfor=Tops`, its description saying the entry is a menu that does nothing on
  its own) ← variants (`addonfor=Ermine Cape (Menu Option)`).
- **Archive formats in the wild here:** zip (deflate + stored dirs), **RAR5**
  (a REFramework helper mod), plain folders. 7z is common on Nexus generally.
- **A Nexus download** is named in the 2026 shape `<Name> <modId> <version> <YYYY-MM-DDTHH-MMZ> <token>.zip`
  (invented example: `Vernworth Cloaks 2207 8 2026-10-14T14-23Z Rk4vLmN2Q.zip`).

## Architecture contract (RS-1 … RS-7)

- **RS-1 — Rust does everything.** Reading archives, classifying options, hashing, deploying, launching, Nexus traffic, downloads.
- **RS-2 — The mods folder is the player's.** The stone never changes, renames or deletes a file in it. It adds exactly one kind of file: a download the player asked for, written under a hidden partial name (`.riftstone-<id>.part`, which the stone removes again when the download fails or is cancelled, and at the next launch) and renamed into place only when whole and checked, never over an existing file.
- **RS-3 — Vanilla is sacred.** `rift_core::dd2::NEVER_WRITE` (`DD2.exe`, `re_chunk_000.pak*`, `dlc/`, `_storage_/`, `steam_api64.dll` …) is refused outright; any other existing file is backed up before it is overwritten and restored on uninstall.
- **RS-4 — Write-ahead journal.** Intent saved before the first byte moves, committed after the last; a crash mid-install is rolled back at the next launch. Schema 2 since v0.3; an older journal is lifted in memory and saved as schema 2 at the next change.
- **RS-5 — One folder.** The `mods` folder beside `riftstone.exe` is the only library (a bench may point `RIFTSTONE_MODS_DIR` elsewhere). Every archive and folder directly inside it is one package; nothing else is scanned. Folder packages are known by their name, so moving the whole install keeps every link.
- **RS-6 — Nexus is optional.** Update checks, labels, Find, browsing and a mod's files use the anonymous GraphQL v2 API and need no key. The optional personal API key adds the key's extras through v1: the account, downloads, tracking and endorsing. It lives in Windows Credential Manager, never in a file or the webview; a download URL and an nxm link's key are secrets like it (never logged, never in an error, never in the window). Every request names the Riftstone (`Application-Name`, `Application-Version`), and v1's quota headers are honoured.
- **RS-7 — Vue 3 + UnoCSS**, `@unocss/reset` before `virtual:uno.css`, the `prefers-reduced-motion` floor.

## Workspace layout and ownership

*(v0.1's build. § v0.3 "Ownership in the build" has the rows the v0.3 builders worked in.)*

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
   No real mod name, file name, option name, author, Nexus id or account name from
   the investor's library in fixtures, tests or docs — use invented ones (`Lantern
   Glow Recolor`, `Vernworth Cloaks Collection`, mod `2207`, sqid `Rk4vLmN2Q`,
   account *Arisen of Vernworth*), and write the install folder as `<install dir>`.
6. Naming Doctrine: user-facing strings (error `reason`s, problem messages) are plain,
   specific sentences that say what happened and what to do. Module names may stay
   plain for mechanism; domain types already have their names in rift-core.
7. **Never touch the real game folder, the investor's mods folder, the stone's app data
   or the investor's Downloads from a test.** Real-data runs belong to the bench suite
   (`#[ignore]`d, env-gated, read-only, counts only), never to `cargo test`'s default set.
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
`#[ignore]` test that reads `RIFT_BENCH_RAR` (the RAR5 archive in the investor's
mods folder: `RIFT_BENCH_RAR='/mnt/c/<install dir>/mods/<a mod>.rar'`; read-only
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

*(v0.1 also had an `is_dd2_shaped` filter for a Downloads-style root. v0.3 removed it with
the roots: every archive and folder in the mods folder is a package.)*

### tree — `resolve(&mut [Package])` and `nest(&Package)`
`addon_for` → the parent option with that `name` (case-insensitive, trimmed): same
package first, else any package (first by library order). Unresolved → keep `addon_for`,
`parent: None`, and add a `MissingParent` problem to the package once per distinct name.
Cycles → break at the option that closes the cycle (no parent). `trail` = names from the
package display name down to the parent, e.g. `["Vernworth Cloaks Collection", "Cloaks",
"Ermine Cape (Menu Option)"]`; bundled options insert the bundle name after the package.
An option that is `menu_only` because its payload is empty but has no children and is not
a DummyMod stays `menu_only` (it installs nothing either way).
`nest(package) -> Vec<Nest>` (v0.3, pure; the shape of the window's library tree and of
`get_package`'s tree): `Nest::Option { id, children }` or `Nest::Bundle { label, children }`.
An option whose parent is in its own package hangs under it; one whose parent is in
another package, or that has none, is a top node of its own package. At each level, two
or more siblings sharing a `bundle` (case-blind, trimmed) gather under one `Bundle` at the
first member's place, labelled with its spelling. The package's option order holds, and
every option appears exactly once (a loop is shown once, from its first member).

### scan
`list_library(library)`: the direct children of the mods folder — archive files by
extension (`zip 7z rar`, case-insensitive; `sniff` confirms when the package opens) and
folders (a link or junction counts as what it points at) — skipping names that start
with `.` (the stone's own `.riftstone-<id>.part` downloads among them), `desktop.ini`,
`*.crdownload`, `*.part`, `*.tmp`. Sorted by name (case-blind), then path. Nothing else is
held back: an archive that holds no mod is a package with `nothingToInstall`.
`scan_package(path) -> Result<ScannedPackage, RiftError>`: open → entries → read every
`modinfo.ini` (`read_entry`, limit 64 KiB) → `build_options`. `display_name`: strip the
extension; then if `rift_core::nexus_name::parse` (see § rift-core additions) recognises a
Nexus download name, use its `name` and set `package.nexus` (source `Filename`). Also set
`nexus` from any option's `homepage` that is a `nexusmods.com/dragonsdogma2/mods/<id>` URL
when the file name gave none (source `Homepage`). `size`/`modified_ms` from metadata
(folder size = sum of files). Ids: `PackageId::for_archive(file name, size)` and (v0.3)
`PackageId::for_folder(folder name)`, so moving the whole mods folder keeps every id. A
folder package holding zip/7z/rar files that no option installs gets one `FilesSkipped`
problem that counts them, never names them: "it holds N archives of its own; put each
archive directly in the mods folder." When the folder installs nothing at all, its
`NothingToInstall` problem says the same.

### search — `SearchIndex`
Build per option a record: lower-cased name, trail (joined `›`), bundle, category,
author, description, package display + file name, version, and its payload + skipped
entry paths (relative to the package). Query: lower-case, split on whitespace; quoted
phrases (`"black necklace"`) stay one token. **Every token must match somewhere** (AND
across fields, substring match). Score per token = best field weight: name 10, trail 6,
bundle 5, package name 4, category 4, author 3, version 2, description 2, file 1; +50%
when the token is a prefix of a word in that field, +100% when it equals a whole word.
Hits sorted by score desc, then name, then library order. `matched` lists distinct fields
best first; `file_matches` up to 5 matching paths (prefer payload paths). Filters:
`installed`, `include_menu_only`, and (v0.2) the Nexus `categories` (any-of) and `tags`
(all-of). Empty text → all options passing filters, library order. `limit` 0 means no
cap (v0.3); a positive limit keeps the best. *(As built: a term written `-cape`, or
`-"ermine cape"`, must match nowhere and removes options; a term holding `/` matches
across folders, and a backslash reads as `/`.)* Target: < 5 ms for 300 options / 3,000
paths; < 50 ms for 10,000 options / 200,000 paths (add a `#[test]` that builds a
synthetic 10k/200k index and asserts a generous bound — 250 ms — in release-agnostic
terms, or mark it `#[ignore]` if debug builds are too slow, and run it with `--release`
in your report).

### conflicts — `detect(layers)`
Index every layer's paths (lower-cased) AND their `rift_pak::path_hash` for loose paths
under `natives/`; index every pak hash. A key touched by ≥ 2 layers is a conflict.
Winner rules: among loose/reframework/root paths the LATER layer in the given slice wins
(the deploy engine passes layers in install order); a loose file beats any pak carrying
the same hash (REFramework's LooseFileLoader serves loose files ahead of pak contents);
among paks the higher `pak_order` wins. `layers` in the conflict: loser first, winner
last. Report each key once.

### catalog — `Catalog`
`scan(library, cache, progress) -> (Catalog, CatalogCache)`: `list_library`, then
`scan_package` for each candidate in parallel (rayon, at most `MAX_SCAN_THREADS` = 8
threads), reusing cached packages whose `(path, size, modified_ms)` match; then
`tree::resolve` over all packages (library order); then build the `SearchIndex`. Emit
`ScanProgress` after each package, from the calling thread. A package that fails to open
still appears with an `Unreadable` problem and no options, and is never cached.
`from_cache(library, cache)`: the same catalog without the disk (the cached packages that
sit directly in `library`, in scan order), so the startup preview shows what the scan
will. A cache of another schema answers nothing. `ScanStats { candidates, opened, reused,
unreadable, gone }`: every candidate lands in exactly one of the last four.

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

**Bench (`tests/bench_real.rs`, `#[ignore]`, env `RIFT_BENCH_LIBRARY=<the mods folder>`):**
read-only, in release. A cold scan, a warm scan from the cache it built, and the startup
preview (`from_cache`, asserted equal to the warm scan); package, option, entry, problem
and `tree::nest` counts; three searches with `limit: 0`, timed. A second bench times each
package's phases (listing, open + list, the modinfo reads, classification). Both print
formats, sizes, counts and milliseconds, never a name. The main session runs them against
the mods folder (`RIFT_BENCH_LIBRARY='/mnt/c/<install dir>/mods'`).

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
- **catalog, one id per package** (v0.3): package ids stay unique. Two names that
  differ only in case (possible only in a case-sensitive folder, such as one a Linux
  bench made) share an id; the first by name keeps it, and the other is left out,
  logged and not cached. A reader panic marks that package `Unreadable`.
- **catalog, nothing passed over** (v0.3): v0.2's Downloads root and its passed-over
  memory are gone with the roots. Every candidate in the mods folder is opened, and one
  that holds no mod shows as a package with `nothingToInstall`.
- **conflicts:** one game file, one conflict. A hash conflict that names a `natives/`
  path and holds every layer of that path's conflict, with the same winner, replaces
  the path conflict.
- **cache:** `CatalogCache::SCHEMA` = 5 (2: nested variants, the new sentences and
  folder stamps; 3: the singular `OutdatedFormats` sentence; 4: the vouched very old
  names; 5: one mods folder — no root id on a package, folder ids by name, the
  archives-in-a-folder problem). An older cache is not trusted: the first scan after it
  reopens every package once.

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
Discovery order: env `RIFTSTONE_GAME_ROOT` (the bench and a manual override; found as
`Env`) → on Windows the registry `HKCU\Software\Valve\Steam\SteamPath`, then
`HKLM\SOFTWARE\WOW6432Node\Valve\Steam\InstallPath` → env `RIFTSTONE_STEAM_ROOT` (the
WSL bench: `/mnt/c/Program Files (x86)/Steam`) → on Windows Steam's default folders
(`C:\Program Files (x86)\Steam`, `C:\Program Files\Steam`). For each Steam root: every
library in `steamapps/libraryfolders.vdf` (those whose `apps` hold `2054970` first) → its
`steamapps/appmanifest_2054970.acf` `installdir` → `<lib>/steamapps/common/<installdir>`
(found as `Steam`). Valid root = contains `DD2.exe` (case-insensitive). The spine tries a
folder the player picked (`Manual`, kept in `settings.json`) after the env override and
before Steam.

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
pub fn read_pak_load_report(game_root: &Path) -> Option<PakLoadReport>; // § The DD2 load rules, "Did it load?"
```

```rust
// journal.rs — the write-ahead journal (RS-4): journal-<16 hex>.json in the app data dir (the hex
// an FNV-1a of the lower-cased game root), replaced atomically on every save (temp file, fsync,
// rename). A damaged journal is an error, never silently reset.
pub const JOURNAL_SCHEMA: u32 = 2;            // v0.3; schema 1 (v0.1, v0.2) is lifted as it loads
pub const STAND_IN_PACKAGE: &str = "adopted"; // a lifted stand-in id reads `adopted#<root half>`
pub struct Journal { pub schema: u32, pub game_root: PathBuf, pub installs: Vec<InstalledOption>,
                     pub pak_order: Vec<OptionId>, pub pending: Option<Intent>, pub next_seq: u64,
                     pub cleanup: Vec<Leftover>, pub parked: Vec<ParkedBackup>, pub created_dirs: Vec<String> }
pub struct InstalledOption { pub seq: u64, pub option_id: OptionId, pub name: String,
    pub package_file: String, pub package_path: PathBuf, pub package_size: u64,
    pub option_root: String, pub version: Option<String>, pub installed_at: String,
    pub game_build: Option<String>, pub origin: Origin, pub files: Vec<DeployedFile> }
impl InstalledOption { pub fn is_adopted(&self) -> bool; }
pub enum Origin { Riftstone, Adopted { label: String } } // serde { "kind": "adopted", "label": … }
pub struct DeployedFile { pub game_path: String, pub kind: DeployKind, pub size: u64,
    pub entry: Option<String>, pub below: Below, pub fingerprint: Option<String> }
pub enum DeployKind { Pak, Loose, Reframework, Root }
/// What sat at this path before this install wrote it.
pub enum Below { Nothing, Backup { store: String, owner: BelowOwner }, UnknownVanilla }
/// Whose bytes a backup holds: no install, or an install in the journal (an adopted one too).
pub enum BelowOwner { Unowned, Riftstone { option_id: OptionId } }
pub struct ParkedBackup { pub store: String, pub game_path: String, pub owner: BelowOwner,
                          pub parked_at: String, pub reason: String }
pub struct Intent { pub action: IntentAction, pub seq: u64, pub option_id: OptionId,
                    pub touched: Vec<String>, pub started_at: String, pub option_name: String,
                    pub steps: Vec<IntentStep>, pub phase: u8, pub dirs: Vec<String> }
pub enum IntentAction { Install, Uninstall, Reorder, RestoreParked }
pub enum IntentStep { Place { .. }, Remove { .. }, Restore { .. }, Rename { .. }, Park { .. } }
impl Journal {
    pub fn load(dir: &Path, game_root: &Path) -> Result<Journal, RiftError>; // lifts schema 1; never writes
    pub fn save(&self, dir: &Path) -> Result<(), RiftError>;
    pub fn holds(&self, option_id: &OptionId) -> bool; // an install, the pak order, a backup or parked owner
    pub fn rekey(&mut self, map: &BTreeMap<OptionId, OptionId>) -> RiftResult<usize>;
    pub fn rekey_with_files(&mut self, map: &BTreeMap<OptionId, OptionId>,
                            package_files: &BTreeMap<OptionId, String>) -> RiftResult<usize>;
}
pub fn journal_path(dir: &Path, game_root: &Path) -> PathBuf;
pub fn is_stand_in(option_id: &OptionId) -> bool; // its package half is not 16 lowercase hex digits
```

```rust
// engine.rs — plan, install, uninstall, reorder, put a parked file back or discard it, re-key, recover
pub struct DeployContext<'a> { pub game: &'a GameInstall, pub state_dir: &'a Path,
    pub ref_status: &'a RefStatus, pub game_running: bool }
pub fn plan_install(ctx: &DeployContext, journal: &Journal, package: &Package, option: &ModOption) -> InstallPlan;
pub fn install(ctx: &DeployContext, journal: &mut Journal, package: &Package, option: &ModOption,
               source: &mut dyn PackageSource, progress: &mut dyn FnMut(ExtractProgress)) -> Result<InstallReport, RiftError>;
pub fn plan_uninstall(ctx: &DeployContext, journal: &Journal, option_id: &OptionId) -> UninstallPlan;
pub fn uninstall(ctx: &DeployContext, journal: &mut Journal, option_id: &OptionId) -> Result<UninstallReport, RiftError>;
pub fn reorder_paks(ctx: &DeployContext, journal: &mut Journal, order: &[OptionId]) -> Result<(), RiftError>;
pub fn restore_parked(ctx: &DeployContext, journal: &mut Journal, store: &str) -> Result<ParkedRestoreReport, RiftError>;
pub fn discard_parked(ctx: &DeployContext, journal: &mut Journal, store: &str) -> Result<(), RiftError>;
pub fn rekey(ctx: &DeployContext, journal: &mut Journal, map: &BTreeMap<OptionId, OptionId>) -> RiftResult<usize>;
pub fn rekey_with_files(ctx: &DeployContext, journal: &mut Journal, map: &BTreeMap<OptionId, OptionId>,
                        package_files: &BTreeMap<OptionId, String>) -> RiftResult<usize>; // one atomic save
pub fn recover(ctx: &DeployContext, journal: &mut Journal) -> Result<Option<RecoveryReport>, RiftError>;
pub fn is_game_running() -> bool; // DD2.exe in the process list (Windows); false elsewhere
// relink.rs — the relink pass (v0.3), pure
pub fn propose(installs: &[InstalledOption], options: &[&ModOption]) -> BTreeMap<OptionId, OptionId>;
// launch.rs — Play (v0.3)
pub const REFUSED_NO_GAME: &str; // the sentence when no game was found at all (the spine says it)
pub fn plan_launch(game: &GameInstall, ref_status: &RefStatus, journal: &Journal,
                   game_state: &GameState, running: bool, busy: bool) -> LaunchPlan; // pure
pub fn launch_route(game: &GameInstall) -> LaunchRoute; // the same route rule, for GameSummary.launch
pub fn start_exe(exe: &Path, cwd: &Path) -> RiftResult<()>;
pub struct LaunchPlan { pub route: LaunchRoute, pub warnings: Vec<LaunchWarning>, pub refused: Option<String> }
pub enum LaunchRoute { Steam { url: String }, Exe { exe: PathBuf, cwd: PathBuf } } // kind(): "steam" | "exe"
pub enum LaunchWarning { ReframeworkMissing { paks: usize }, PakFolderOff { paks: usize },
                         LooseLoaderOff { files: usize }, BuildChanged { recorded: String, now: String } }
// state.rs — what is in the game right now
pub fn read_game_state(ctx: &DeployContext, journal: &Journal) -> GameState;
pub enum PakOwner { Riftstone { option_id: OptionId, name: String, adopted: bool }, Unowned }
// preflight.rs
pub fn preflight(state: &GameState) -> Vec<Check>;
pub enum CheckAction { EnableSwitch { switch_key: RefSwitch }, PickGameFolder, OpenUrl { url: String },
                       Rescan, ReviewParked }
```
`InstallPlan`, `UninstallPlan`, `InstallReport`, `UninstallReport`, `RecoveryReport`,
`ParkedRestoreReport` (plan.rs), `GameState` (state.rs) and `Check` (preflight.rs) serialize
camelCase with `kind`-tagged enums, the shapes of `src/types/rift.ts`. A planned write
carries `entry → game_path`, kind, size and an action: `Create`, `OverwriteRiftstone {
option_name, option_id }` (a file an install in the journal owns, an adopted one too),
`OverwriteUnowned` (a file no install claims), `Reinstall`. Plan warnings: `pakFolderOff`,
`looseLoaderOff`, `reframeworkMissing`, `gameRunning`, `requiresMissing {name}`,
`conflicts {optionNames, paths}`, `outdatedFormats {count}`; `refused` is a sentence when
the plan cannot run. A planned removal: `delete`, `restoreBackup {owner, ownerKind}`,
`keepChanged`, `keepVanilla`, `handDown {toOptionName, toOptionId}`, `alreadyGone`.
`OwnerKind { Unowned, Adopted, Riftstone }` (Adopted: the owning install is adopted).
`GameState`: `game`, `running`, `build_changed` (the build id now vs the one recorded at
the newest install), `reframework`, `installs`, `pak_order`, `paks` (every `.pak` in
`pak_mods/`, any case and depth, in load order, each with its `PakOwner`), `strays` (files
in `pak_mods/`, `natives/`, `reframework/autorun/`, `reframework/plugins/` that no install
in the journal claims), `missing` (files the journal says are installed but are gone),
`parked`, `pak_load`, `pak_count`, `pak_count_warning`, `interrupted`, `outdated_files`.
`Check`: `{ id, level: ok|info|warn|error, title, detail, action? }`.

### The rules (each one is a test)

1. **Target guard** (`guard.rs`, used by every write): game paths are normalized
   (`rift_core::normalize_entry_path`); the only writable places are `natives/**`,
   `reframework/**`, `pak_mods/*.pak`, and files directly in the game root. **Refused
   outright:** `DD2.exe`, any `*.exe` in the root, `re_chunk_000.pak` and every
   `re_chunk_000.pak.*`, everything under `dlc/`, `steam_api64.dll`, `steam_appid.txt`,
   `shader.cache2`, `config.ini`, `config_default.ini`. The joined path must stay inside
   the game root (check the canonical parent). *(As built: also `_storage_/`, any `.pak`
   directly in the root, names Windows would bend into another file (trailing dots or
   spaces, device names, 8.3 aliases), and names carrying the engine's own temporary
   marker `.riftstone-`. Every folder that already exists on the way to the target must
   be a real folder, not a link, junction or file, and resolve to exactly the folders it
   names.)*
2. **Backups before overwrites.** A target that exists and is not this install's own:
   move (rename; copy+delete across volumes) the existing file into
   `<state_dir>/backups/<seq>/<game_path>` BEFORE the new bytes land; record `Below::Backup`
   with its owner (an install in the journal, the Riftstone's own or an adopted one, or
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
6. **Adopted installs.** v0.1 and v0.2 could take over installs another tool had made
   in the game folder. v0.3 removed the taking-over; the installs taken over before stay
   in the journal as they were recorded: `Origin::Adopted { label }`, no package entries
   (`entry: None`), `below = Nothing` for files under `natives/`, `reframework/`,
   `pak_mods/` and for `dinput8.dll`, `UnknownVanilla` for any other root file. They
   follow every rule here like the Riftstone's own: they uninstall (a root file below
   `UnknownVanilla` stays), hand down and join `pak_order`. An adopted pak keeps its
   numbered `xNNNN-` name until the first reorder renames it `zrift-NNNN-<stem>.pak`.
   Nothing creates a new adopted install.
7. **Never delete what you do not own.** Uninstall touches only files recorded in the
   journal. Strays are reported, never removed.
8. **Game running:** `plan_install` for a pak while DD2 runs → warning (it loads at next
   launch); `uninstall`/`reorder_paks` of paks while it runs → refused (the files are
   open). Loose files may install while it runs (REFramework's loose loader).
9. **Requirements:** `requires=` names (resolved by the caller to option ids or not) that
   are not installed → `requiresMissing` warning, never a refusal.
10. **The lift (v0.3).** `Journal::load` parses the file into `serde_json::Value` and
    reads `schema` first: a newer schema is refused. Schema 1 is lifted in memory by
    rules that name no legacy kind. An origin of any kind other than `riftstone` becomes
    `adopted`, labelled with its `section`, else the install's name. An owner of any kind
    other than `unowned` or `riftstone` becomes `unowned`, in `installs[].files[].below.owner`,
    `parked[].owner` and `pending.steps[].owner`. A stand-in option id (its package half
    is not 16 lowercase hex digits) becomes `adopted#<root half>` in `installs[].optionId`,
    `pakOrder`, every `owner.optionId` and `pending.optionId`; one that would collide
    gets `#2`, `#3` …; an empty reorder id stays empty. The JSON then deserializes
    strictly as schema 2. Loading never writes: the file keeps schema 1 until the next
    save. Tests lift INVENTED legacy kinds, never the old literal.
11. **Re-key (v0.3).** `Journal::rekey` rewrites every mention of a held id (the install,
    the pak order, the owner of a backup or of a parked file) and returns how many
    installs took a new id. `rekey_with_files` also sets each re-keyed install's
    `package_file` from a map keyed by the NEW id. Entries onto themselves and for ids
    the journal does not hold change nothing. Refused, and then nothing changes: while
    `pending` is set, when a target id is already held (`holds`), and when two ids would
    become one. `engine::rekey` / `engine::rekey_with_files` add the engine's common
    refusals and save once, atomically: a refusal or a failed save changes nothing on disk
    or in memory, and a map that changes nothing saves nothing. Nothing in the game
    folder moves.
12. **The relink matcher (v0.3).** `relink::propose` takes the journal's installs and
    every option of every package in the mods folder. A candidate is an install whose
    package no given option belongs to (a package without options counts as absent). An
    option fits it when, compared case-blind: the option root is the install's recorded
    root; every recorded loose, script and root file is in the option's payload at the
    same game path; a recorded pak asks only that the option carries paks; and, when the
    root is empty or only paks speak for the install, the install's name is the option's
    name or its package's (read from `option.trail[0]`, the catalog's package crumb). It
    proposes a match only for exactly one fitting option, never onto an id an install or a
    backup owner holds, and never when two installs fit one target.
13. **Play (v0.3).** Route: `Steam { url: "steam://rungameid/2054970" }` when
    `game.source == GameSource::Steam`, else `Exe { exe: DD2.exe as spelled on disk, cwd:
    the game root }`. Refusals, one sentence each, checked in this order: the game folder
    holds no `DD2.exe`; the game runs; `busy` (a change holds the game folder); the journal
    holds a pending intent. Warnings, kept even when Play is refused, count the journal's
    files (adopted installs too): `ReframeworkMissing { paks }` (paks installed, no
    `dinput8.dll`), `PakFolderOff { paks }` (the switch reads off), `LooseLoaderOff {
    files }` (loose files installed, the loader not on), `BuildChanged { recorded, now }`.
    `start_exe` refuses a missing exe or working folder. On Windows it calls
    `CreateProcessW` with `bInheritHandles = FALSE` and `DETACHED_PROCESS |
    CREATE_NEW_PROCESS_GROUP` and closes both handles at once; elsewhere it uses
    `std::process::Command` and a thread that reaps the child.

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
  the numbered `xNNNN-` names other tools give paks (NTFS compares upcased names: `X` <
  `Z`), so a new install wins over what they left in `pak_mods/`. Always lowercase
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
`libraryfolders.vdf` shape and an `.acf`. RefStatus from fixture log/config files. (v0.3)
`adopted_rules.rs` over a hand-seeded journal of adopted installs; `legacy_rules.rs`: the
lift over hand-written schema-1 JSON with invented legacy kinds, rekey and relink;
`launch_rules.rs`: each route, warning and refusal, and `start_exe`.
**Never point a test at the real game folder.**

**Bench (`tests/bench_real.rs`, `#[ignore]`, env `RIFT_BENCH_GAME=<the game folder>`):**
READ-ONLY — `discover_game_from` (the Steam root three folders above the game),
`read_status`, `read_pak_load_report`, `read_game_state`, `preflight` and `plan_launch`
against a journal in a tempdir. It prints counts and states, never a mod name. No
install, no uninstall, no switch, no launch, ever, in a bench test.

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
  misleads. v1 is key-only: `validate_key` (`GET /v1/users/validate.json`, free of the
  hourly limit), `files_v1(mod_id)` (the chain and, since v0.3, each file's archive name)
  and the key's extras of § v0.3 (download links, tracking, endorsing). The anonymous
  update check never uses it.
- **Download names** (decoded live): 2026 shape
  `{name} {modId} {version} {YYYY-MM-DDTHH-MMZ} {sqid}.ext` — the timestamp is the FILE'S
  UPLOAD time, the last token is that file's `sqid` (not random). Classic shape
  `{name}-{modId}-{version, dots as hyphens}-{unix seconds}.ext`, and very old names
  without a timestamp (invented example: `Lantern Glow-123-1-2.zip`). Browser duplicates
  append ` (1)` before the extension.

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
    pub fn validate_key(&self) -> Result<NexusAccount, NexusError>;                    // v1, needs a key, no quota
    // v0.2.1: find_mods (§ v0.2.1). v0.3: files_v1, download_links, tracked_mods, track, untrack,
    // endorsements, endorse, abstain, mod_file_details, browse, confirm_download, downloader (§ v0.3)
}
pub struct NexusMod { pub mod_id: u64, pub name: String, pub version: Option<String>, pub updated_ms: Option<u64>, pub status: String,
                      pub category: Option<String>, pub tags: Vec<String>, pub adult: bool } // labels: v0.2
pub struct NexusFile { pub file_id: u64, pub group_id: Option<u64>, pub sqid: Option<String>, pub name: String, pub version: Option<String>, pub category: FileCategory, pub uploaded_ms: u64, pub primary: bool, pub size_bytes: Option<u64> }
pub enum FileCategory { Main, Update, Optional, OldVersion, Miscellaneous, Removed, Archived, Unknown }
pub struct Md5Hit { pub mod_id: u64, pub file_id: u64, pub version: Option<String>, pub name: String, pub size_bytes: Option<u64> }
pub struct NexusAccount { pub name: String, pub user_id: Option<u64>, pub is_premium: bool, pub is_supporter: bool }
pub enum NexusError { Offline(String), RateLimited { retry_after_s: Option<u64> }, KeyRejected, NoApiKey, BadResponse(String), Http(u16),
                      // v0.3: the key's extras and the downloader
                      PremiumRequired, LinkExpired, LinkRefused, FileGone, Cancelled, SizeMismatch { expected: u64, got: u64 },
                      NotTheFile, Disk(String), EndorseRefused(EndorseRefusal) }
// v0.3: NexusError::reason() -> Option<rift_core::NexusReason>; From<NexusError> for RiftError sets it

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
**Bench (`tests/bench_real.rs`, `#[ignore]`, each gated on its own env var; counts and states
only, never a mod name, file name or version):** `RIFT_BENCH_NEXUS_MOD=<mod id>` (with an
optional `RIFT_BENCH_NEXUS_SQID`) sends exactly one live `mods(&[id])` and one
`mod_files(&[id])`. `RIFT_BENCH_NEXUS_FIND=1` (v0.2.1) sends only two fixed phrases
(`lantern glow`, `armor`), never a name from the library: at most four anonymous
requests. `RIFT_BENCH_NEXUS_BROWSE=1` (v0.3) reads four browse pages (endorsements,
downloads and updated without text; relevance with the fixed word `armor`) and, with
`RIFT_BENCH_NEXUS_MOD`, that mod's file notes: at most five anonymous requests. A mod id
comes from whoever runs the bench, never from the repo.

## § src-tauri (integration)

**Owns:** `src-tauri/**` (except `icons/`, `tauri.conf.json` window/bundle blocks may be
edited only for capability/plugin needs).

**Contract:** `docs/WINDOW-CONTRACT.md` (every command, event, the startup sequence) and
`src/types/rift.ts` (every shape, authoritative). Mirror the TS types in
`src-tauri/src/dto.rs` with `#[serde(rename_all = "camelCase")]` and tagged enums
(`#[serde(tag = "kind", rename_all = "camelCase")]` + per-variant `rename_all`) so the JSON
matches the TS unions EXACTLY. Add a `dto` round-trip test per union that serializes a
sample and asserts the JSON keys the TS side reads.

**Modules (as built through v0.3):**
- `state.rs` — `AppState` and the startup sequence. It holds the `Shelf` (the catalog,
  swapped whole), the `World` (the game and its journal slot: no game, ready, or damaged
  and never reset), the `deploy_lock` (one writer in the game folder; a `Turn` holds it),
  the game-folder `Snapshot` (shared per change through `fresh.rs`), the Nexus store and
  client, the account word, the standings cache, the downloads and the game word. No lock
  is held across an archive read, a game-folder walk, a network call or a file write.
  Lock order: `deploy_lock` → `world` → the rest, and `shelf` → `nexus`.
- `SpineConfig { dirs, discovery, library_dir, app_version, nexus_endpoints,
  watch_library, launcher: Arc<dyn GameLauncher>, game_watch: Arc<dyn GameWatch>,
  game_cadence, protocols: Arc<dyn ProtocolRegistry>, exe }`. `SpineConfig::system(dirs,
  library_dir)` is the app's (the desktop's launcher, process list and registry, the
  running exe). `Discovery::System` tries `RIFTSTONE_GAME_ROOT`, then the folder the
  player picked, then Steam; `Discovery::Fixed { game_root }` (tests) searches nothing.
  There are no roots.
- `persist.rs` — JSON in the app data dir: `settings.json`, `catalog-cache.json`,
  `nexus.json`, `nxm-handler.json` (v0.3); rift-deploy's `journal-<hex>.json` and
  `backups/` live in the same dir; `thumbs/` in the cache dir. Every write is a temp file +
  rename. A file that cannot be parsed is set aside as `<name>.damaged` and starts fresh;
  one that exists but cannot be read is held: that session never writes over it.
  `roots.json` (v0.2) is never read, written or deleted.
- `library.rs` (`Shelf`; `SourcePool`, a few packages kept open between reads and closed
  by a timer), `fresh.rs` (one game-folder read per change, shared by everyone who asks;
  a re-read takes one at most 250 ms old), `commands/*.rs`, `dto.rs`, `events.rs`.
- `thumbs.rs` — decode with `image`, resize to 360 px / 1600 px wide max, JPEG quality 82,
  base64 `data:image/jpeg;base64,` URL, disk cache keyed `<package id>-<fnv(entry)>-<width>.jpg`
  (a picture that cannot be decoded leaves a `.fail` marker instead).
- `watch.rs` — `notify`, recursive on the one mods folder, 1.5 s debounce → rescan. Reads,
  metadata touches, files still being written (`.crdownload`, `.part`, `.partial`, `.tmp`,
  `.download`, the stone's own `.riftstone-<id>.part`) and the stone's own data folders
  never count.
- `conflicts.rs` — `rift_catalog::Layer`s from the journal's installs (the Riftstone's own
  and adopted ones) in install order; pak hashes read from `pak_mods/` files on disk with
  `rift_pak` (head + TOC only; cached by file name + size + mtime); for a not-installed
  option in `get_option`, its pak TOCs read from the archive with `read_entry(limit =
  toc_len)`. A pak whose TOC read fails (`PakError::TooManyEntries` too) contributes
  nothing; one with `toc.unknown_flags() != 0` keeps its hashes, and its conflicts are
  unconfirmed. Files no install claims take no part.
- `nexus.rs` — the link store per package id, in order of authority: the player's own
  link or unlink (`manual`); an MD5 match (`md5`) or a download the stone made (`download`,
  v0.3); a pin by an update check; the catalog's file name or `homepage=` link. Update
  results, labels per mod (v0.2), the last check. `NexusStore::move_records` lets a record
  follow its package to a new id (v0.3).
- `keys.rs` — `keyring = "4"`, service `nl.zmuuzn.riftstone`, account `nexus-api-key`
  (`Entry::new`, `set_password`, `get_password`, `delete_credential`;
  `keyring::Error::NoEntry` = absent).
- v0.3: `migrate.rs` (the pure half of the migration), `play.rs` (`GameLauncher`,
  `GameWatch`, `GameWord`, `GameCadence`), `account.rs`, `downloads.rs`, `nxm.rs`,
  `protocol.rs` (`ProtocolRegistry`). § v0.3 § src-tauri has their rules.

**Plugins**, in this order: `tauri-plugin-single-instance` FIRST (v0.3; 2.5.2 in
`Cargo.lock`, no `deep-link` feature), then `opener`, `dialog`, `log`. `tauri.conf.json`
declares no URL scheme; its `main` window is built in `setup` (`create: false`), so a
bench data home takes the WebView2 profile along.

**Threading:** every command is a thin `#[tauri::command] async fn` wrapper around a plain
`*_impl(&AppState, …)` function, run on Tauri's blocking pool (`commands::blocking`), even
the quick ones; a panic in the work still answers the window with a `RiftError`. Named
threads: `riftstone-wake` (the watchers, the first scan, the launch's update check),
`riftstone-scan`, `riftstone-watch`, `riftstone-game` (the game watcher),
`riftstone-account` (the key's validation, then the launch's nxm links),
`riftstone-download` (the one download worker), `riftstone-pool`.
Never hold the catalog write lock while opening archives: scan into a fresh `Catalog`,
then swap it in.

**Mappings that matter:**
- `SearchResult` = a `SearchHit` + the option's card fields + `installed` (the journal's
  origin, `riftstone` or `adopted`) + `update` (package link state: `available` /
  `current` / `unknown`; `null` unlinked) + `conflicted` (the option is a layer in any
  current conflict) + the package's Nexus labels (v0.2).
- `Overview.checks` = `rift_deploy::preflight(&state)` under the window's ids
  (`dto::WINDOW_CHECK_IDS`, `faultyFiles` left out) + the spine's own (`game`, `journal`,
  `mods`, `relinked`, `settingsUnreadable`, `nexusUnreadable`, `unreadable`,
  `nothingToInstall`), most severe first.
- `get_thumbnail` for `card` on an option without a screenshot → `null` (the window draws
  its own placeholder).
- Every command error → `RiftError` (serialize as rift-core does). Never `unwrap()` on a
  lock or a user path in a command: the lock helpers recover from poisoning.

**Env overrides** (read once at startup): `RIFTSTONE_GAME_ROOT` and `RIFTSTONE_STEAM_ROOT`
(rift-deploy's discovery), `RIFTSTONE_MODS_DIR` (the mods folder; else `mods` beside
`riftstone.exe`, never canonicalized), `RIFTSTONE_DATA_HOME` (replaces the app data and
cache dirs, and takes the log and the WebView2 profile with it: tests and the bench never
write into the real app data).

**Tests:** dto shape tests (an exact-key test per shape); a command-layer harness
(`tests/support`, `Bench`) that builds `AppState` over a tempdir (a fake game root, one
`mods` folder with two invented zips and a Nexus-named download) WITHOUT a Tauri runtime,
through the `*_impl` functions, with a recording launcher, a switched game watch and a
memory registry; `spine_ipc.rs` drives every command through Tauri's mock runtime by its
invoke name and camelCase argument keys. Suites: account, contention, downloads, flow,
ipc, legacy, nexus, nxm, play, startup, window. Gates: `cargo clippy -p riftstone
--all-targets -- -D warnings`, `cargo test -p riftstone`, and `cargo build -p riftstone`.

## § The Window

*(the Artisan's direction, then the Illusionist)*

**Owns:** `src/**`, `tests/**`, `index.html`, `uno.config.ts`.

**Contract:** `docs/WINDOW-CONTRACT.md` for every command, event and behaviour rule,
`src/types/rift.ts` for every shape, and design system #00015 for every look (tokens,
components, surfaces, states and words; §16 for v0.3). Where the design system and the contract
disagree on behaviour, the contract stands and the design system is amended.

**As built through v0.3:** four rooms in `src/pages/` (THE RIFT, YOUR WORLD, NEXUS with its
page inscription THE FAR WORLD, PREFLIGHT) beside the stone (`RiftstonePillar.vue`, with Play's
slab in `PlayStone.vue`); one composable per concern in `src/composables/`, each with a reset
the specs call; `src/services/` is the only door to Tauri (`rift.ts`), with the voice
(`voice.ts`: every failure says what happened, that the stone understood what you tried, and
what to do next), the ledger's wording and the seam's arithmetic beside it;
`src/services/mock/` runs the window in a browser on invented data (`npm run dev`, `?mock=`
scenes). The specs in `tests/` drive the window over the mock through `tests/harness.ts`.
Gates: `npm run typecheck`, `lint`, `test`, `format:check`, `build`. § v0.3 § The window records
what the v0.3 builders chose.

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
  per-package labels (`set_labels(HashMap<PackageId, PackageLabels { category, tags, adult }>)`),
  searchable as a field `nexusLabel` (weight 4, between bundle and author) and used by the
  two filters; `MatchedField::NexusLabel`. Labels compare case-insensitively; display keeps
  Nexus's spelling.
- **spine:** the Nexus store keeps each linked mod's labels from the latest check (persisted in
  `nexus.json`), feeds them to the index after every check and scan, and serves
  `get_labels() -> LabelFacets { categories: LabelCount[], tags: LabelCount[], unlabeledPackages:
  number }` (`LabelCount { name, options }` — option counts, so a facet predicts the results).
  `SearchResult` gains `nexusCategory: string | null`, `nexusTags: string[]`, `adult: boolean`;
  `OptionDetail.labels` carries the same three, read from the same index.
- **window:** a NEXUS facet group in the stone below STATE — CATEGORY chips (any-of) and TAG
  chips (all-of) with counts; an unlinked-packages line with the one-click identify; the card
  carries the category as a quiet chip; the detail lists category, tags and the mod-page link.
  Settings: the veil gains a mode — `off` / `adult` (mods Nexus flags adult, and packages it has
  not labelled yet) / `all`.

### v0.2.1 — Find on Nexus (the labels' reach, 2026-10-16)

MD5 identify linked 2 of the investor's 8 unlinked packages: 4 are unpacked folders (nothing to
hash) and 2 are repacks. Labels need a link, so the stone searches Nexus by NAME:
`rift_nexus::search_terms` cleans a name into search words, `NexusClient::find_mods` asks the
anonymous `mods(filter: nameStemmed …)` (MATCHES, then one WILDCARD retry), and the spine's
`find_on_nexus(packageId) -> NexusFindings` tries the display name, then the first option's
bundle or name. Up to five candidates; the player links one, and the update check brings its
labels. The full rules are WINDOW-CONTRACT § Find on Nexus. A link by mod id stays unpinned.

### v0.2 backlog beside the labels (from the deploy safety review, 2026-10-16)

- **Parked backups in the window:** `InstalledView.parked: ParkedFile[]` ({store, gamePath,
  owner, ownerKind, size | null, parkedAt, reason, occupied}); `UninstallResultView.parked` (a
  `RecoveryNote.parked` count was dropped: the recovery sentence names the file and the review
  check covers the rest); commands `restore_parked(store) -> ParkedRestoreReport` ({gamePath,
  restored, swappedOut | null, restartNeeded}) and `discard_parked(store)`, both followed by
  `rift://game-changed` (a restore while the game runs still happens and answers
  `restartNeeded: true`); `CheckAction` gains `{ kind: "reviewParked" }`; `RecoveryNote.action`
  gains `"restoreParked"`.
- **rift-archive defence in depth:** the extractor opens destinations with
  `OpenOptions::create_new(true)` (the engine already pre-checks and re-guards part names; this
  closes the live race between that check and `File::create`). Built: every progress report names
  its item (`ExtractProgress.item`); when an extraction stops, the engine marks the parts it never
  saw reported `IntentStep::Place.unextracted` (saved to the journal), and the rollback, including
  a reinstall's restore of the old copy, leaves those names alone. The extractor removes its own
  half-written file itself. After a crash (a panic, not an error) which parts were written is not
  known, so recovery keeps the old rule: a part at a name the install checked free is its own.

---

## § v0.3 — one folder, the tree, Play, the key's extras (investor ask, 2026-10-17)

*"i want to be able to get rid of [the old mod manager], should not be any reference to it, i moved
the mods to <install dir>\mods where the riftstone itself is also installed. That should be the
one place to place the installable mods, no need for difficult scanning and multiple places. In
the rift i want to see the standard tree view, so in my case something like 9 packages first and
then i can go deeper, … but once i search i should be able to see everything. I added an api key
for nexus but i did not see any difference in what i can do with it, so let's see if we an do some
more with that, and i was wondering if we can do a download from the riftstone itself? And i want
a start to play button so i can start the game from the riftstone and i like the name riftstone,
so i want to work with that."* (the investor's install path written as `<install dir>`.)

The behaviour is WINDOW-CONTRACT § v0.3 (authoritative for the wire and every rule). This section
is the crate-level contract: who builds what, the public API each crate offers the spine, and the
rules each crate proves with tests.

### Rulings taken for the build (main session, 2026-10-17)

1. **"No reference" covers the gadget's tree**: code, identifiers, comments, tests, fixtures,
   scripts, docs and UI text. No literal of the old manager's name or its initials survives
   anywhere in `crates/`, `src-tauri/`, `src/`, `tests/`, `scripts/`, `docs/`, `README.md`,
   `CLAUDE.md`, `index.html`. Persisted legacy data is read by a generic lift that names nothing
   (§ rift-deploy). The git history is not rewritten.
2. **What survives under neutral words**: the modinfo.ini packaging format (`addonfor`,
   `NameAsBundle`, `DummyMod`, `MenuPriority`, `requires`, screenshots) — it is how mods are
   packaged; adopted installs (origin `adopted`); the `zrift-` prefix and the stripping of a
   numbered `xNNNN-` prefix another tool gave a pak.
3. **What goes**: the drive-wide discovery, the ledger reader, adoption, un-adopted owners and
   their conflict layers, the renamed-crash-reporter check, Find's manager-name noise rule, the
   Downloads root, custom roots, root filters, the DD2-shaped filter and the passed-over memory.
4. **Folder package ids follow the name** (`PackageId::for_folder(name)`); the spine re-keys v0.2
   ids exactly (`PackageId::legacy_folder`) and relinks the rest by option (§ src-tauri).
5. **Downloads**: premium in the stone; free through the site's Mod Manager Download link and an
   opt-in nxm handler that gives the protocol back on opt-out. Never installs by itself.
6. **Play** through Steam when the game came from Steam, else `DD2.exe`. Warnings never block:
   the spine never refuses Play for one, and the window asks once, in Play's inline confirm
   (WINDOW-CONTRACT § v0.3 Play), before it starts the game.
7. **Version 0.3.0** (workspace `Cargo.toml`, `package.json`, `tauri.conf.json`). Name, product
   name, identifier (`nl.zmuuzn.riftstone`) and keyring service unchanged.

### Ownership in the build (parallel builders never edit outside their row)

| Builder | Owns | Gates it runs (own `CARGO_TARGET_DIR=target/v03-<name>`) |
|---|---|---|
| main session (done before the build) | `crates/rift-core` API, `src/types/rift.ts`, `src/services/rift.ts` (interface + invoke names), `docs/WINDOW-CONTRACT.md`, this section | — |
| catalog | `crates/rift-catalog`, `crates/rift-archive`, `crates/rift-pak` (+ word-only edits in `crates/rift-core`) | fmt/clippy/test `-p rift-catalog -p rift-archive -p rift-pak -p rift-core` |
| deploy | `crates/rift-deploy` | fmt/clippy/test `-p rift-deploy` |
| nexus | `crates/rift-nexus` | fmt/clippy/test `-p rift-nexus` |
| spine | `src-tauri/**` (after the three crates land) | fmt/clippy/test/build `-p riftstone`, then the workspace |
| window | `src/**` (except the two contract files' shapes), `tests/**`, `index.html`, `uno.config.ts` | npm typecheck, lint, test, format:check, build |

A builder that needs a contract change says so in its report; it never edits another row.

### § rift-core (landed with the contract)

- `RootKind`, `LibraryRoot` and `Package.root_id` are gone.
- `PackageId::for_folder(folder_name: &str)` = fnv1a64(`folder|<lower name>`);
  `PackageId::legacy_folder(absolute_path: &Path)` = v0.2's formula, for the re-key only.
- `LinkSource::Download`.
- `RiftError::Nexus { detail, reason: Option<NexusReason> }` (reason skipped when `None`),
  `NexusReason` (camelCase: offline, rateLimited, noKey, keyRejected, premiumRequired,
  linkExpired, linkNotYours, linkRefused, fileGone, sizeMismatch, notTheFile,
  endorseNotDownloaded, endorseTooSoon, endorseOwnMod), `RiftError::nexus(detail)`,
  `RiftError::nexus_because(reason, detail)`.

### § rift-catalog (builder: catalog)

- **One folder.** `Catalog::scan(library: &Path, cache: &CatalogCache, progress) -> (Catalog,
  CatalogCache)` and `Catalog::from_cache(library: &Path, cache) -> Catalog` (keeps cached packages
  whose path sits directly in `library`). `scan::list_library(&Path)` lists one level as today
  (folders and zip/7z/rar; never `.`-names, `desktop.ini`, `.crdownload`/`.part`/`.tmp`).
  `scan_package(path)`/`scan_stamped`/`unreadable_package` lose the root parameter. Folder packages
  use `PackageId::for_folder(<file name>)`; archives keep `for_archive`.
- **Gone:** `dd2_shaped_only`, `classify::is_dd2_shaped` (unless a non-root caller remains),
  `PassedOverCandidate`, `CatalogCache.passed_over`, `Outcome::PassedOver`, the multi-root
  `ScanStats` fields, root ordering (library order = case-blind file name), the cross-root
  duplicate settlement, `SearchFilters.root_ids` and the index's root interning,
  `Catalog::options_named` (its only caller was adoption).
- **`CatalogCache::SCHEMA` 4 → 5** (Package lost `root_id`): an older cache is not trusted (the
  first v0.3 scan reopens every package once).
- **Search:** `SearchQuery.limit == 0` means no cap (`DEFAULT_LIMIT` goes); a positive limit still
  caps. Ranking unchanged. A v0.2 query JSON carrying `rootIds` still parses (unknown fields are
  ignored).
- **`tree::nest(package: &Package) -> Vec<Nest>`** (pure; moved from the spine's `package_tree`):
  `enum Nest { Option { id: OptionId, children: Vec<Nest> }, Bundle { label: String, children:
  Vec<Nest> } }` — an option whose parent is in the same package nests under it; an option whose
  parent is elsewhere (or missing) is a top node of its own package; at each level, options sharing
  a `bundle` (case-blind, at least two) gather under one `Bundle` at the first member's place;
  order = the package's option order; a loop never repeats a node. The spine's
  `the_tree_nests_addons_and_gathers_bundles` moves here as a catalog test.
- **A precise problem for archives inside a folder package** (gift): a folder package whose
  entries include `.zip`/`.7z`/`.rar` files no option carries gets `ProblemKind::FilesSkipped`
  with the message "holds N archives of its own; put each archive directly in the mods folder"
  (counts only, no names).
- **Words:** every comment, doc, test name and fixture that names the old manager or "library
  roots" (catalog.rs, scan.rs, tree.rs, modinfo.rs's test named after the other manager,
  classify.rs, search.rs fixtures, test_support.rs, lib.rs; rift-archive lib.rs and
  bench_real.rs; rift-pak lib.rs's REtool sentence goes). The scan's unreadable messages say
  "the mods folder".
- **Benches** (`#[ignore]`, env-gated, counts only): `RIFT_BENCH_LIBRARY` is THE mods folder;
  `RIFT_BENCH_DOWNLOADS` goes.

**As built (catalog builder, 2026-10-17).**
- `Catalog::scan(library: &Path, cache: &CatalogCache, progress: &mut dyn FnMut(ScanProgress))
  -> (Catalog, CatalogCache)`; with no cache, pass `&CatalogCache::default()`. A cache of
  another schema answers nothing. `Catalog::from_cache(library, cache)` keeps the cached
  packages whose `path.parent() == Some(library)`, in scan order. Unchanged: `packages`,
  `package`, `option`, `entries`, `search`, `set_labels`, `labels`, `label_facets`, `stats`.
- `ScanStats { candidates, opened, reused, unreadable, gone }`: every candidate lands in exactly
  one of the last four. `scan::list_library(&Path) -> Vec<PathBuf>` sorts by (lower-case name,
  path). `scan::scan_package(path) -> Result<ScannedPackage, RiftError>` lost its `Option`:
  nothing is passed over any more. `scan_stamped` and `unreadable_package` are crate-private.
- `tree::nest` and `Nest` (`Debug, Clone, PartialEq, Eq`; re-exported as `rift_catalog::Nest`);
  `the_tree_nests_addons_and_gathers_bundles` lives in tree.rs. A bundle compares case-blind
  and trimmed, a blank one is none, and a bundle of one stays a plain option.
- `SearchFilters { installed, include_menu_only, categories, tags }`. A query JSON carrying
  `rootIds` still parses and narrows nothing.
- Folder ids: `PackageId::for_folder(<file name>)`. v0.2's id was
  `PackageId::legacy_folder(std::path::absolute(<folder>))`, the absolute path, never
  canonicalized; the spine's re-key relies on that.
- The archives-in-a-folder problem appends after the classifier's own problems: "it holds 1
  archive of its own; put that archive directly in the mods folder." / "it holds N archives of
  its own; put each archive directly in the mods folder." A folder that also has
  `NothingToInstall` gets the same sentence there, so its trouble card does not lead with the
  usual advice to download the mod again.
- Wording: `MissingParent` ends "Add the mod that provides "X" to the mods folder to restore
  the menu."; the password-protected `Unreadable` message ends "put the unpacked folder in the
  mods folder instead."
- Deleted: the `LibraryRoot` use, `Catalog.roots`, `Catalog::options_named`,
  `PassedOverCandidate`, `CatalogCache.passed_over`, `Outcome::PassedOver`, the passed-over and
  duplicates `ScanStats` fields, the cross-root duplicate settlement, `classify::is_dd2_shaped`,
  `SearchFilters.root_ids`, the index's root interning, `DEFAULT_LIMIT`,
  `test_support::root`, and the tests whose only subject was a removed feature (the Downloads
  root, two roots, the DD2-shape test). modinfo.rs's test is `reads_every_key_the_format_knows`.
- The bench reads only `RIFT_BENCH_LIBRARY`; it also prints `tree::nest` counts and the
  startup-preview time, and asserts that the preview equals the warm scan.
- **Open (needs a ruling):** an archive file at an option root (beside a `modinfo.ini` or a
  `natives/` anchor) falls under classify rule 3's "any other file at the root" and is `Root`
  payload: it would install into the game root. The archives problem does not count it,
  because an option installs it. Skipping archives at option roots would change
  classification.

### § rift-deploy (builder: deploy)

- **Gone:** the other manager's module (its discovery, its env override, the ledger parser and
  its types), the context's ledger field, the set of paths the ledger listed, the old-manager
  arms of `foreign_below`, the helper that named an install by its ledger section, the
  requires-satisfied-by-ledger rule, the `restore_parked` refusal over ledger files,
  `owner_label`'s old-manager arm, adoption (the adopt function, `adopted_below`,
  `adopted_kind`), the stack helper that found the layer above a ledger file,
  `Journal::adopted_sections`, `AdoptReport`, the `WriteAction` variant that named the other
  manager (those files are `OverwriteUnowned`), `PakOwner`'s old-manager variant, the
  un-adopted list and its state struct, the crash-reporter check, `CheckAction`'s adopt
  action, the un-adopted check; the deps `dirs` and `encoding_rs` if nothing else uses them.
- **Journal schema 2.** `Origin { Riftstone, Adopted { label: String } }`, `BelowOwner { Unowned,
  Riftstone { option_id } }`, `OwnerKind { Unowned, Adopted, Riftstone }` (Adopted: the owner is
  an adopted install), `JOURNAL_SCHEMA = 2`. `Journal::load` parses into `serde_json::Value`, reads
  `schema` first (newer → refused as today), lifts schema 1 in memory (the rules in
  WINDOW-CONTRACT § v0.3 Legacy data: any non-`riftstone` origin → `adopted` with its `section`,
  else the install name, as `label`; any owner not `unowned`/`riftstone` → `unowned`, in
  `installs[].files[].below.owner`, `parked[].owner` and `pending.steps[].owner`; a stand-in option
  id — package half not 16 lowercase hex digits — → `adopted#<root half>` in `installs[].optionId`,
  `pakOrder`, every `owner.optionId` and `pending.optionId`; a stand-in that would collide gets
  `#2`, `#3`), then deserializes strictly and keeps `schema = 2` for the next save (no save at
  load). Tests write schema-1 JSON with INVENTED legacy kinds (never the old literal).
- **Re-key and relink.** `Journal::rekey(&mut self, map: &BTreeMap<OptionId, OptionId>) ->
  RiftResult<usize>`: rewrites every reference (installs, pak order, backup and parked owners);
  refuses while `pending` is set and when a target id is already in the journal (nothing changes on
  refusal); also fills `package_file` from an optional new-file map. `engine::rekey(ctx, journal,
  map) -> RiftResult<usize>` saves it once (atomic like every save). `relink::propose(installs:
  &[InstalledOption], options: &[&ModOption]) -> BTreeMap<OptionId, OptionId>`: the pure matcher of
  WINDOW-CONTRACT § v0.3 Legacy data (same root case-blind; every recorded loose/script/root game
  path in the option's payload; paks only if the option has paks; name check when the root is
  empty or the install is pak-only; exactly one candidate; never onto an id the journal holds).
- **Play.** `launch::plan_launch(game: &GameInstall, ref_status: &RefStatus, journal: &Journal,
  game_state: &GameState, running: bool, busy: bool) -> LaunchPlan { route: LaunchRoute,
  warnings: Vec<LaunchWarning>, refused: Option<String> }`; `LaunchRoute::Steam { url:
  "steam://rungameid/2054970" }` when `game.source == GameSource::Steam`, else `LaunchRoute::Exe {
  exe: <root>/DD2.exe as found on disk, cwd: <root> }`; warnings `ReframeworkMissing { paks }`
  (pak installs and no dinput8.dll), `PakFolderOff { paks }`, `LooseLoaderOff { files }` (loose
  files installed), `BuildChanged { recorded, now }`; refusals: running, busy (a change in
  progress), `journal.pending` (cut short). `launch::start_exe(exe, cwd) -> RiftResult<()>`
  (spawn, detached, no window of its own). The Steam route is opened by the spine.
- **Strays** are files no journal layer claims (wording names the game's mod places, never a
  manager). The x-prefix stripping in `paks.rs` stays, its doc and test renamed.
- **Tests:** the other manager's rules suite goes, and `adopted_rules.rs` replaces it over a
  HAND-SEEDED journal of adopted installs (load, uninstall keeps an unknown-vanilla root file and
  deletes nothing-below files, hand-down to a Riftstone layer above, a path listed twice in two
  spellings uninstalls once, reorder renames adopted `x000N-` paks to `zrift-NNNN-<stem>`); a lift
  test over schema-1 JSON with invented legacy kinds and a stand-in id; rekey and relink tests
  (exact, refused on pending, refused on collision, ambiguous → no match); `plan_launch` tests
  (each route, each warning, each refusal). Keep every surviving invariant's twin
  (WINDOW-CONTRACT: unowned backup, hand-down, never deleting what is not journaled). Same-size
  byte fixtures keep their lengths.

**As built (deploy builder, 2026-10-17).** The base § rift-deploy above lists the API as
built; these are the builder's notes beside it.
- `foreign_below` is now `unowned_below`. Strays read "no install in the journal claims them".
  `paks.rs` keeps the `xNNNN-` stripping with a reworded doc, its test renamed
  `ntfs_order_puts_ours_after_numbered_prefixes`. `WriteAction::OverwriteRiftstone` carries
  `{ option_name, option_id }`; `PakOwner::Riftstone` carries `{ option_id, name, adopted }`.
- The "optional new-file map" is `Journal::rekey_with_files` / `engine::rekey_with_files`, keyed
  by the NEW option id; both rekeys are re-exported at the crate root. `Journal::holds`,
  `journal::is_stand_in`, `journal::STAND_IN_PACKAGE`, `launch::launch_route` and
  `launch::REFUSED_NO_GAME` are additions the spec did not name.
- `dirs` and `encoding_rs` left rift-deploy. `encoding_rs` stays a workspace dependency for
  rift-catalog; `dirs` left the workspace when its last user went (822c913).
- Tests: `adopted_rules.rs` (7), `legacy_rules.rs` (16: the lift over hand-written schema-1
  JSON with the invented kinds `adoptedFromElsewhere` and `takenOverByHand` and the owner
  `elsewhere`, plus rekey and relink), `launch_rules.rs` (8). `common::ctx_with(running)` and
  `common::adopted(...)` seed them. The same-length byte fixtures are `RIVAL GLOW` (10 bytes)
  and `RIVAL BYTES` (11). The bench reads only `RIFT_BENCH_GAME`.
- The spine's migration order, one save: build the exact map (each install whose package is a
  v0.2 folder id → `OptionId::new(&PackageId::for_folder(name), id.root())`); apply it to a
  clone with `Journal::rekey`; run `relink::propose` over the clone and every option of every
  package; merge both maps; call `engine::rekey_with_files` once on the real journal.
- **Windows:** the `CreateProcessW` path in `launch.rs` was first type-checked against
  `windows-sys` 0.61.2 in a scratch crate; it has since compiled natively, and the native run
  found the `DD2.exe` spelling bug that 0b34ac4 fixed (§ v0.3 § The first native Windows run).
  No test spawns the game (`start_exe`'s test only checks its refusals), so the
  `CreateProcessW` call has compiled on Windows but never run there.
- **Open:** press Play once by each route on Windows (Steam, then `DD2.exe` from a folder picked
  by hand). `relink::propose` reads a package's name from `ModOption.trail[0]`: if the catalog
  changes how trails start, the name half of the match breaks (the rest does not depend on it).

### § rift-nexus (builder: nexus)

- **Account.** `NexusAccount { name, user_id: Option<u64>, is_premium, is_supporter }` from
  validate.json (`is_supporter` / legacy `is_supporter?`); never the echoed key or the e-mail.
- **v1 calls (metered unless noted)**, all through the existing `v1_*` path (headers, quota
  refusal, 401 → `KeyRejected`), a POST/DELETE helper with `application/x-www-form-urlencoded`
  bodies (hand-encoded or reqwest's `form` feature):
  `download_links(mod_id, file_id, grant: Option<&NxmGrant>) -> Vec<DownloadMirror { name,
  short_name, uri }>` (403 → `PremiumRequired`, 410 → `LinkExpired`, 400 → `LinkRefused`, 404 →
  `FileGone`; mirrors whose `uri` is not https are dropped unless loopback);
  `files_v1` gains each file's archive `file_name`; `tracked_mods() -> Vec<u64>` (domain
  `dragonsdogma2` only); `track(mod_id)`, `untrack(mod_id)` (POST/DELETE
  `user/tracked_mods.json?domain_name=dragonsdogma2`, form `mod_id`; 200 "already" and 201 both
  succeed); `endorsements() -> Vec<(u64, EndorseStatus)>`; `endorse(mod_id, version)`,
  `abstain(mod_id, version)` (form `Version`; Nexus's refusal codes → `EndorseNotDownloaded`,
  `EndorseTooSoon`, `EndorseOwnMod`).
- **v2 (anonymous):** `mod_file_details(mod_id) -> Vec<NexusFileDetail>` (fileId, name, version,
  category, sizeInBytes, date, description, changelogText, groupId, primary) for one mod;
  `browse(query: &BrowseQuery { text, sort, offset }) -> BrowsePage { total, mods:
  Vec<NexusCandidate> }` (`mods(filter: { gameDomainName: EQUALS dragonsdogma2 [, nameStemmed:
  MATCHES text] }, sort: [{ <key>: { direction: DESC } }], offset, count: 20)`; text only as a
  variable; never a top-level `op: OR`; `total` from `totalCount`, page size from the nodes).
- **The downloader.** A second blocking client built for archives: never holds or sends the API
  key (the download path takes only a mirror URI), sends the identity headers, https only except
  loopback, `connect_timeout` 15 s, an idle timeout of 60 s per read and no total timeout, at most
  5 redirects (no secret rides along). `download_to(uri, dest: &Path, expected: Option<u64>,
  progress: &mut dyn FnMut(u64, Option<u64>), cancel: &AtomicBool) -> Result<Downloaded { bytes,
  md5 }, NexusError>`: streams 1 MiB chunks into `dest` (created new, never over an existing
  file), MD5 on the way, checks `cancel` between chunks, compares the byte count with the
  server's length and `expected` (`SizeMismatch`), deletes `dest` on every failure and on cancel
  (`Cancelled`). The URI never appears in an error, a log line or Debug output.
- **nxm links.** `parse_nxm(text) -> Result<NxmMessage, NxmRefusal>`: `NxmMessage::File(NxmLink {
  mod_id, file_id, grant: Option<NxmGrant { key, expires_s }>, user_id: Option<u64> })` for
  `nxm://dragonsdogma2/mods/<id>/files/<id>[?key&expires&user_id&…]`, `NxmMessage::Premium` for
  `nxm://premium`; refusals: another game, a collection, unreadable. `NxmGrant`'s Debug redacts the
  key; `expired(now_s)`.
- **Errors.** `NexusError` gains `PremiumRequired`, `LinkExpired`, `LinkRefused`, `FileGone`,
  `Cancelled`, `SizeMismatch { expected, got }`, `NotTheFile`, `Disk(String)`,
  `EndorseRefused(EndorseRefusal)`; `From<NexusError> for RiftError` sets the matching
  `NexusReason` (offline, rateLimited, noKey, keyRejected too). No sentence says "401", "403" or
  "reject" unless the key really was rejected.
- **Find's terms** lose the manager-name rule and its tests; everything else in `terms.rs` stays.
- **Tests** (the loopback stand-in; a second stand-in plays the CDN): download_link with and
  without a grant, each refusal code, quota spent → refused locally; the CDN request never carries
  `apikey` (assert on the CDN stand-in) while `a_redirect_is_never_followed_so_the_key_never_reaches_another_host`
  stays green; a binary body (`Reply::bytes`), a short body, a size mismatch, a cancel from the
  progress callback (no file left), an existing destination (refused, untouched); track/untrack
  bodies; endorse refusals; browse request shape and paging; mod_file_details changelog; parse_nxm
  cases with invented ids only.

**As built (nexus builder, 2026-10-17).** All blocking. New files: `src/download.rs`,
`src/nxm.rs`, `tests/key_extras.rs`, `tests/downloader.rs`; no new dependency (form bodies
are encoded by hand, so reqwest's `form` feature is not needed).
- `validate_key` is unmetered and reads `is_premium?` / `is_supporter?` as fallbacks. Every
  other v1 call goes through one shared path: the identity headers, the local refusal while
  the quota is spent, 401 → `KeyRejected`. `files_v1` → `V1FileList.file_names:
  BTreeMap<u64, String>` (`#[serde(default)]`) and `file_name(file_id)`.
- `download_links(mod_id, file_id, grant: Option<&NxmGrant>)`: 400 `LinkRefused`, 401
  `KeyRejected`, 403 `PremiumRequired`, 404 `FileGone`, 410 `LinkExpired`. Mirrors that are
  not https are dropped (loopback http is kept); none left → `BadResponse`. The grant travels
  as `?key=&expires=`.
- `tracked_mods()`: dragonsdogma2 mods only, in Nexus order, each once. `track`: 200 and 201
  succeed, 404 → `FileGone`. `untrack`: 2xx or 404 succeeds (untracked either way).
  `endorsements()`: undecided mods are left out; for a repeated mod the latest date wins.
  `endorse` / `abstain(mod_id, version)`: form `Version` (trimmed); `NOT_DOWNLOADED_MOD`,
  `TOO_SOON_AFTER_DOWNLOAD` and `IS_OWN_MOD` are matched anywhere in the body, whatever the
  status, and become `EndorseRefused(EndorseRefusal::{NotDownloaded, TooSoon, OwnMod})`; 404 →
  `FileGone`.
- `mod_file_details(mod_id) -> Vec<NexusFileDetail>`: anonymous, oldest upload first. Any
  GraphQL error refuses the whole answer; an id outside 1..=i32::MAX answers no files and
  sends nothing. `NexusFileDetail { #[serde(flatten)] file: NexusFile, description:
  Option<String>, changelog: Vec<String> }`: the description whitespace-collapsed and cut at
  400 characters, the changelog lines trimmed and blank ones dropped.
- `browse(&BrowseQuery { text, sort, offset }) -> BrowsePage { total, offset, mods:
  Vec<ModCandidate> }`. The text travels only as `$text` (whitespace collapsed, at most 200
  characters); without text no variable is declared. `BrowseSort` (serde camelCase):
  `endorsements` (default), `downloads`, `updated` (the key `updatedAt`), `relevance`
  (without text it sorts by endorsements). The offset is clamped, a page is 20, and no
  top-level `op: OR` is ever sent. Without `totalCount`, `total` = offset + mods.
- `confirm_download(md5, mod_id, file_id)`: a hit for this file → `DownloadCheck::Confirmed`;
  no Dragon's Dogma 2 hit → `Unknown` (Nexus has not hashed it yet; the size alone vouches);
  hits for other files only → `Err(NotTheFile)`. A malformed md5 → `BadResponse`, nothing sent.
- `Downloader` (`Send + Sync + Clone`, never holds a key): `new(app_version)`,
  `with_idle_timeout`, `download_to(uri, dest, expected, progress, cancel)`. Reads of up to
  1 MiB into a `create_new` file, MD5 on the way, `cancel` checked between reads. A
  `Content-Length` that disagrees with `expected` is refused before the body; a body that runs
  past the stated size is stopped early. CDN 401/403/410 → `LinkExpired` (never
  `KeyRejected`), 404 → `FileGone`, 429 → `RateLimited`. The URL is stripped from reqwest's
  errors and scrubbed from messages. A drop guard deletes the file on every error, on cancel
  and on a panic in the progress callback; an existing `dest` → `Disk`, left untouched.
- nxm: `NxmMessage { File(NxmLink), Premium }`; `NxmLink::made_for(account_user_id)` is true
  unless both ids are known and differ; `NxmGrant` prints `<redacted>`, never serializes, and
  `expired(now_s)` is `now_s >= expires_s`; `NxmRefusal { OtherGame, Collection, Unreadable }`,
  each a sentence. A key without its expiry (or the reverse), or a `user_id` that is not a
  number, makes the link unreadable.
- `NexusError::reason() -> Option<NexusReason>` covers offline, rateLimited, noKey,
  keyRejected and every new case; `BadResponse`, `Http` and `Cancelled` have none.
  `From<NexusError> for RiftError` uses `RiftError::nexus_because` when there is a reason,
  else `RiftError::nexus`; `Disk` becomes `RiftError::Io`. `Http(401|403)` no longer spells the
  number. There is no `NexusError` for `linkNotYours`: the spine builds it with
  `RiftError::nexus_because(NexusReason::LinkNotYours, ..)`.
- Types: `DownloadMirror { name, short_name, uri }` (Debug redacts `uri`; no Serialize),
  `EndorseStatus { Endorsed, Abstained }`. `mod_manager_page_url(mod_id, file_id)` is the
  file's page with `&nmm=1`. Constants: `BROWSE_PAGE_SIZE` 20, `FILE_DESCRIPTION_CHARS` 400,
  `DOWNLOAD_CHUNK_BYTES` 1 MiB, `DOWNLOAD_CONNECT_TIMEOUT` 15 s, `DOWNLOAD_IDLE_TIMEOUT` 60 s,
  `MAX_DOWNLOAD_REDIRECTS` 5.
- Find's manager-name rule (its constant, enum and function) and its test are deleted;
  `terms.rs` keeps `game_name` and a test for the `for`-before-the-game's-name rule.
- **Open:** the live API never confirmed the browse sort keys `downloads` and `updatedAt`
  (the research measured `endorsements` and relevance). To check them:
  `RIFT_BENCH_NEXUS_BROWSE=1 cargo test -p rift-nexus --test bench_real -- --ignored --nocapture`.

### § src-tauri (builder: spine, after the three crates)

- **Mods folder.** `SpineConfig.library_dir: PathBuf` (replaces `downloads_dir` and every root);
  `mods_dir_from(exe: &Path, override: Option<OsString>) -> PathBuf` pure and tested; `run()` uses
  `RIFTSTONE_MODS_DIR` else `<current_exe dir>/mods` (no `canonicalize`). `AppState::open` makes
  it (`create_dir_all`; failure → the `mods` Error check and an empty shelf, never a panic) and
  removes `.riftstone-*.part` files directly inside it. `roots.json` is never read, written or
  deleted. The watcher arms on the one folder. `open_mods_folder` opens it with
  `tauri_plugin_opener::open_path` (made first if missing).
- **Gone:** everything that discovered, read or matched the old manager (its finder, its ledger
  reader, its matcher and its root; `downloads_root`, `custom_root`, `same_folder`, the ledger
  params of `game_layers`, the stand-in conflict layers), the adopt command, `add_root`,
  `remove_root`, `StoredRoots`, the `roots` checks, `Overview`'s old-manager field and `roots`.
- **Migration after every scan** (under the deploy lock, never while a change is pending): (1)
  the exact folder re-key: for each folder package now in the mods folder,
  `legacy_folder(<mods dir>/<name>)` → `for_folder(<name>)` for every journal option id and every
  `nexus.json` key that carries the legacy id; (2) the relink pass (`relink::propose` over the
  installs whose package is missing); one `engine::rekey` save, one `nexus.json` save (moving a
  link record follows its package), then `rift://game-changed` and the `relinked` Info check
  (count) for the session.
- **Library tree.** `get_library` → `LibraryView`: every shelf package sorted by display name
  (case-blind) then file name; `tree::nest` decorated into `TreeNode` (installed from the journal,
  conflicted from the snapshot, kinds, hasScreenshot, menuOnly; bundles: any/sum); package counts,
  update badge, labels, adult, conflicted. `get_package` keeps its tree (now from `tree::nest`).
- **Search:** the window's `limit: 0` reaches the catalog as no cap.
- **Play.** A `GameLauncher` seam (as built: `SpineConfig.launcher`, beside `game_watch` and
  `game_cadence`, not beside `KeyStore`): `System` (Steam route via
  `tauri_plugin_opener::open_url`, exe route via `rift_deploy::launch::start_exe`) and a
  recording double for tests. `plan_launch`/`launch_game` commands (busy = the deploy lock is held
  or an install is in progress). A `GameWatch` seam (`fn running(&self) -> bool`; System =
  `rift_deploy::is_game_running`) sampled every 2 s by the `riftstone-game` thread; a flip updates
  `GameSummary.running`/`startedMs`, invalidates the snapshot and emits `rift://game-changed`
  (the snapshot reads the watcher's state, so `spine_contention`'s exact build count holds). After
  a launch the watcher samples every 1 s for up to 120 s.
- **Account.** The wake thread validates a stored key after the first scan (unmetered) →
  `NexusSummary.key`/`account` and the quota; `set_nexus_key` keeps the account it validated;
  `refresh_nexus_account`. The update check no longer waits on the v1 quota.
- **Standings.** `get_nexus_standings(refresh)` (session cache), `set_tracked`, `set_endorsement`
  (version: the linked package's local version, else the mod's current version from v2).
- **Mod files and browse.** `get_mod_files(modId)` (marks `inLibrary` from pinned links and
  `newest` per group with rift-nexus's update rules), `browse_nexus(query)` (fills
  `linkedPackages`).
- **Downloads** (`downloads.rs`): an in-memory list + one worker thread `riftstone-download`
  (`Weak<AppState>`), a cancel flag per job, phases and events per WINDOW-CONTRACT § v0.3
  Downloads (progress throttled to 200 ms), the file name rule, the collision rule, the
  `nexus.json` pre-link (`LinkSource::Download`, pinned mod + file + group + version + upload time),
  then `request_scan`. Never holds `deploy_lock` or `nexus_check`. The free path parks the job as
  `waiting` with `nexusPageUrl = https://www.nexusmods.com/dragonsdogma2/mods/<mod>?tab=files&file_id=<file>&nmm=1`.
- **nxm.** `tauri-plugin-single-instance` (2.x for tauri 2.12; as built 2.5.2, pinned in
  `Cargo.lock`) registered FIRST; its callback and the cold-start argv hand any `nxm://` argument
  to `handle_nxm(state, text)` (parse → refusals per WINDOW-CONTRACT § The nxm handler → resume
  the waiting job for the same mod and file, else start one) and bring the main window forward;
  `rift://nxm-received`. A `ProtocolRegistry` seam (as built: `SpineConfig.protocols`, with
  `SpineConfig.exe` naming the stone's own `riftstone.exe`): `System` (Windows, `winreg`; it
  reads HKCU, HKLM and HKCR and writes only HKCU; the previous command is remembered in
  `nxm-handler.json`), `Unsupported` (elsewhere), `Memory` (tests). Never registered at install:
  `tauri.conf.json` declares no deep-link schemes and no deep-link plugin is added.
- **Wire:** dto mirrors of every v0.3 shape with exact-key tests; `WINDOW_CHECK_IDS` and
  `OWN_CHECKS` move together (`mods` replaces the old row); events.rs gains the three events;
  `with_commands` + `spine_ipc.rs` cover every command by invoke name and argument keys.
- **Tests:** the Bench becomes one `mods` folder (the two invented zips and the Nexus-named
  download) with no other manager anywhere; a seeded schema-1 journal with an adopted install loads,
  lists as adopted, uninstalls; the folder re-key and the relink against a bench that moved its
  mods folder; Play refused/started through the recording launcher; the watcher's flip emits once;
  a download through a loopback API stand-in and a loopback CDN stand-in lands under its Nexus
  name, links itself, and a cut-off body leaves nothing; the nxm handler claim/give-back on the
  memory registry; `handle_nxm` refusals (expired, another account, another game, no key).

**As built (two builders, 2026-10-17: spine-core for the folder, the tree and Play; spine-nexus
for the key's extras).** WINDOW-CONTRACT § v0.3 records the behaviour each one chose.
- **Commands:** `with_commands` registers 44, every invoke name in `src/services/rift.ts`
  (`spine_ipc.rs` reads that file and checks). New: `get_library`, `open_mods_folder`,
  `plan_launch`, `launch_game`, `refresh_nexus_account`, `get_mod_files`, `browse_nexus`,
  `get_nexus_standings`, `set_tracked`, `set_endorsement`, `get_downloads`, `start_download`,
  `cancel_download`, `clear_downloads`, `get_nxm_handler`, `set_nxm_handler`. Removed: the
  adopt command, `add_root`, `remove_root`.
- **Events:** `DOWNLOADS_CHANGED`, `DOWNLOAD_PROGRESS`, `NXM_RECEIVED` and `events::ALL`, which
  a test reads against the `EVENTS` block of `src/types/rift.ts`. `rift://game-changed` also
  fires on a game-watcher flip (once per flip, never per sample) and after the migration
  re-keys the journal. `rift://updates-changed` also fires when the migration moves link
  records, when the account's standing changes, after a track or an endorsement, and when a
  download lands.
- **DTOs** (each with an exact-key test): gone are `RootKind`, `LibraryRoot`, the old
  manager's summary and row views, `JournalOrigin`, `AdoptResultView`, `Overview`'s
  old-manager field and `roots`, `SearchFilters.rootIds` (a v0.2 query still parses; the
  filter is ignored), `SearchResult.rootId`, `PackageSummary.rootId`, `OptionDetail`'s
  old-manager flag, `InstalledView`'s un-adopted list, `CheckAction`'s adopt action and
  `WriteAction`'s old-manager variant. Added: `ModsFolderView` and `Overview.mods`,
  `LaunchRoute`, `GameSummary.startedMs` / `.launch`, `NexusKeyStanding`,
  `NexusAccountView`, `NexusSummary.key` / `.account`, `InstallOrigin` (`riftstone` |
  `adopted`, in `InstalledInfo.origin` and `InstalledRow.origin`), `OwnerKind::Adopted`,
  `LinkSource::Download`, `CheckAction::OpenModsFolder`, `TreeNode.conflicted`,
  `LibraryPackage`, `LibraryView`, `LaunchWarning`, `LaunchPlanView`, `LaunchReport`,
  `NexusFileCategory`, `NexusFileEntry`, `NexusModFiles`, `NexusBrowseSort` / `Query` /
  `Page`, `EndorseStatus`, `EndorsementView`, `NexusStandings`, `DownloadPhase`,
  `DownloadView`, `DownloadProgressEvent`, `NxmHandlerState` / `View` and `NxmReceivedEvent`
  (Serialize-only, because rift-core's `RiftError` has no Deserialize).
  `WINDOW_CHECK_IDS`: `("mods", "mods")` replaces the old row.
- **State:** `SpineConfig` (see § src-tauri above), `SpineConfig::system(dirs, library_dir)`,
  `Discovery::{System, Fixed { game_root }}`, `mods_dir_from`, `MODS_DIR_ENV`,
  `is_partial_download(&str)` (the `.riftstone-<id>.part` rule every download name follows).
  `AppState` gains `library_dir`, `mods_unmade`, `arm_watcher`, `watching_library`, `migrate`,
  `relinked_count`, `watch_game`, `sample_game`, `observe_game`, `game_run`, `game_launched`,
  `deploy_busy`, `installs_under_way`, `begin_install`, `turn`, `try_turn`,
  `queue_launch_links`, `take_launch_links`. A `Turn` holds the deploy lock and, when it drops,
  runs any migration a scan deferred.
- **Play** (`play.rs`): the trait `GameLauncher` (`launch(&LaunchRoute)`, `show_folder(&Path)`;
  the launcher also shows the mods folder, so the IPC test can call `open_mods_folder` without
  opening Explorer); `SystemLauncher`; `RecordingLauncher` (`starting(watch)` makes a launch
  flip the watch; `seen()`); `GameWatch`, `SystemGameWatch`, `SwitchedGameWatch` (`set`,
  `asked`); `GameCadence` (2 s; 1 s for 120 s after Play); `GameWord`; `spawn_watcher`.
- **Migration** (`migrate.rs`, pure): `folder_moves`, `plan_rekey`, `journal_ids`, `RekeyPlan {
  map, files, package_moves }`. `NexusStore::move_records(&BTreeMap<PackageId, PackageId>) ->
  bool`; `nexus_error` is `err.into()`.
- **Account** (`account.rs`): `AccountWord { standing, known, epoch }`, `KnownAccount {
  account, checked_ms }`, `StandingsCache { epoch, tracked, endorsements, checked_ms }`;
  `validate`, `validate_at_wake`, `key_changed`, `account_now`, `key_rejected`,
  `account_user_id`. Every key change bumps the epoch, and an answer that started under an
  older key is never applied. `wake()` scans, then starts the `riftstone-account` thread
  (validate the key, then the launch's nxm links) while the anonymous update check runs
  beside it, so the check never waits on the key or its quota.
- **Downloads** (`downloads.rs`): `Downloads` (`views()`, `view(id)`), `PROGRESS_EVERY` (200 ms),
  `part_name(id)`, `start`, `take_link`, `cancel`, `clear`, `ensure_worker`, `Landing {
  Written, AlreadyHad }`, `landing_name`, `clean_file_name`, `link_file_name`, `land`. Phases:
  linking (the file's facts from v2, the archive name from `files.json`, `download_link.json`
  with the grant), downloading (rift-nexus's `Downloader`), verifying (the size, then
  `confirm_download`), landed. The name rule: Windows cleaning (no separators, reserved
  device names take a leading `_`, no leading or trailing dots or spaces, at most 180
  characters); a name without zip/7z/rar takes the extension its magic bytes show. The
  collision rule: a case-blind name match with the same size and MD5 is `alreadyHad`, else
  ` (n)` before the extension; a hard-link no-clobber rename means a file is never written
  over. Pre-link: `NexusStore::set_link_for(&PackageId, NexusLink)` with `LinkSource::Download`.
- **nxm** (`protocol.rs`, `nxm.rs`): the trait `ProtocolRegistry` (`supported`, `user_command`,
  `machine_command`, `effective_command`, `write_user`, `remove_user`), `system_registry()`,
  `UnsupportedRegistry`, `MemoryRegistry` (`holding`, `user`, `set_user`, `writes`), `command_for`,
  `command_program`, `program_name`, `points_at`, `NOT_ON_THIS_SYSTEM`; `handler_view`,
  `set_handler` (writes `nxm-handler.json` `{ schema: 1, previous: <the command HKCU held, or
  null> }` BEFORE it touches the registry), `nxm_arguments`, `handle_nxm` (never waits on the
  network: the single-instance callback runs on the window's thread), `handle_launch_links`.
  `persist::DataDirs::nxm_handler_file()`.
- **lib.rs:** single-instance first, with `second_launch` (each nxm argument to `handle_nxm`,
  then unminimize, show and focus `main`). A cold start queues the argv's nxm links before
  `wake()`.
- **Windows-only, never compiled on the Linux bench** (`winreg` refuses other targets; compiled
  natively on 2026-10-17, § v0.3 § The first native Windows run):
  `protocol.rs`'s `mod win` (`winreg` 0.56: reads the default value of
  `HKCU\Software\Classes\nxm\shell\open\command`, the same path under HKLM, and
  `HKCR\nxm\shell\open\command`; writes `HKCU\Software\Classes\nxm` (`URL:NXM Protocol`, `URL
  Protocol` = "", `shell\open\command`); removes it with `delete_subkey_all`), the
  `cfg(windows)` line in `system_registry()`, and `[target.'cfg(windows)'.dependencies] winreg
  = "0.56"`. These compile everywhere but act only on Windows: the single-instance
  `WM_COPYDATA` callback and the `hard_link` landing on NTFS.
- **Open:**
  - The Windows-only paths have compiled and the suites have run natively (§ v0.3 § The first
    native Windows run), but nothing has been pressed on a Windows desktop yet. Owed, once
    each: press Play by each route; use Open mods folder; turn the nxm handler on and off, with
    and without another program registered; with a free account, press Mod Manager Download
    while the stone runs (a second launch through single-instance) and while it is closed (a
    cold start); with a premium account, download one file and cancel another mid-download.
  - `tauri-plugin-single-instance` is pinned to 2.5.2 (published 2026-10-01) by a hand-edited
    lock entry (version and checksum), because `cargo update --precise` also moved unrelated
    `windows-sys` entries. `cargo metadata --locked` passes; a later `cargo update` will move
    it.
  - A known, benign race: when `deploy_busy()`'s momentary `try_lock` (from `plan_launch`)
    meets a scan's migration attempt, the migration waits for the next turn or scan.
  - A Nexus file that is not a zip, 7z or rar (a raw `.pak` or `.exe`) lands under its own
    name, and the scan never lists it.

### § The window (builder: window — the Illusionist, after the Artisan's amendment to #00015)

Everything in WINDOW-CONTRACT § v0.3 that the player sees: the library tree as the Rift's idle
view (with the trouble fold above it), Play on the stone, the mods-folder row and empty state, the
account card, downloads, browsing Nexus, a mod's files and patch notes, track and endorse, the nxm
handler switch, an uninstall door on every load-order row, every old-manager and multi-root
surface removed, the mock and specs to match, and a source sweep spec that proves the old
manager's name appears nowhere in `src/` or `tests/` without spelling it.

**As built (four window builders, 2026-10-17: the floor, the tree and Play, the far world, the
specs; committed in c7ed0c2).** 227 specs in 26 files (new: tree 14, play 7, farWorld 22,
modsFolder 6, yourWorld 3, noOldManager 3), green over 10 shuffled runs; typecheck, lint, format
and build clean.
- **New:** `LibraryTree.vue`, `LibraryBranch.vue`, `PlayStone.vue`, `AccountCard.vue`,
  `NxmSwitch.vue`, `DownloadShelf.vue`, `ModFiles.vue`, `NexusBrowse.vue`, `FarModCard.vue`,
  `StandingControls.vue`; the composables `useLibrary`, `useLaunch`, `useDownloads`,
  `useNexusAccount` and `useNxmHandler`, each reset by `resetRiftstone`; the mock's `far.ts` and
  `library.ts`; the scenes `emptymods`, `premium`, `rejected`, `running` and `nxmother`.
- **Choices the builders made where §16 was silent or read otherwise** (as committed):
  1. Keys: Space zooms an option's screenshot, else acts as Enter; `*` opens every sibling
     branch; ↓ from the well lands on the row the tree last held, else the first. These follow
     WINDOW-CONTRACT § v0.3 The library tree, which is authoritative for behaviour; R16.6 reads
     Space = Enter, `*` unbound and ↓ to the first row, and is owed the amendment.
  2. R16.10's 12ch label floor is approximated: the kind badges fold to `+N kinds` when a row is
     at most 420 px wide (a container query in `LibraryBranch.vue`), an estimate, not the 12ch
     rule itself.
  3. 4 px (`--rs-space-1`) between collapsed package slabs; R16.4 names only the 8 px after an
     expanded one.
  4. Play while stirring is drawn pressable (R16.13), but `useLaunch().play()` ignores a press
     while a launch is in flight: until the watcher sees the game, or the 120 s wait ends with a
     warn toast. A second press can never start a second game; R16.13 wanted a press there so
     that a start that never comes leaves no dead stone.
  5. A free account's update row reads `Get it on Nexus` (it starts a waiting download and opens
     the file's page); R16.23 names only `Download v{version}`. The files view says `Download`
     for every account, as R16.21 rules.
  6. `Replace key…` opens THE FAR WORLD's own key form, as v0.2 did; R16.17 points it at
     Preflight's key row.
  7. The TRACKED section's head reads `TRACKED ON NEXUS`; R16.16 names it `TRACKED`.
  8. The mock's other nxm program is `OtherManager.exe` (`src/services/mock/far.ts`); R16.24
     names `Wagonmaster.exe`.
  9. `Fix it` maps each warning to its PREFLIGHT row (`reframeworkMissing` → `reframework`,
     `pakFolderOff` → `pakdir`, `looseLoaderOff` → `loose`, `buildChanged` → `build`); the rows
     carry `data-check`.
  10. Tracked cards carry no Download (a card holds no file id): a download goes through Files.
  11. A refusal to track or endorse is kept per mod and said under the pair, never as a toast
      (R16.22).
- **Drift from §16 that the specs pin as built** (a ruling, or a fix): R16.25 asks for `Scan`
  Primary, `Open mods folder` Secondary and the path ellipsized from the left with the whole
  path in `title`; built: `Open mods folder` is Primary unless the row is OK, `Scan` is
  Secondary, and there is no `title`. R16.26 asks for `its package isn't in your mods folder`
  and a `title` on `adopted`; built: a `no package` mark, and `adopted` with no title. The Mods
  folder row's detail follows the check's title with no space between them, so a screen reader
  may run the two together.
- **Measured on the mock** (headless Chromium over `npm run dev`, 2026-10-17, by the v0.3 review
  and its fixers; #00015 §16.8 and §15 still read predicted or unmeasured):
  - Q16.1: at 1024×680 the Play slab sits wholly inside the window in every state, above the
    foot at 634 px: Ready 573–634, Running 565–634, refused 531–634, a two-warning confirm
    478–634. The facet block keeps 211 px of its 602 px content at rest (169 refused, 116 in
    the confirm) and scrolls. At 1320×860: the slab at 753–814, the facets 385 of 532 px.
  - Q16.2: `Play anyway` (95 px) and `Fix it` (53 px) share one line in the slab's 216 px at
    1024×680.
  - Q16.3: 250 answers are on screen 358 ms (1320×860) and 382 ms (1024×680) after the
    keystroke, the 120 ms debounce included; the scroll's frame p95 is 17.6 and 17.2 ms. No
    virtualisation is needed at this size.
  - §15 question 2: the seam's lit length at reach 1 is 746 px at 1320×860 and 572 px at
    1024×680 (predicted 742 and 562).
- **Open:**
  - Rulings on R16.6 (to match the contract), R16.16, R16.17, R16.23, R16.24, R16.25 and R16.26,
    each from the lists above, then #00015 or the window amended to match; and the Q16 values
    above written into #00015 as measured.
  - Play shows a stale refusal for one plan read after the game stops: `boot.onGameChanged`
    reads the overview before `useLaunch().onGameChanged` reads the plan again (`play.spec`
    waits for Ready).
  - The README's three frames were v0.2 shots (the roots facet, no Play, and the retired name in
    their pixels). They are out of the README until they are shot again from the v0.3 mock; the
    files in `docs/images/` still need replacing or removing.

### The first native Windows run (2026-10-17, after 822c913)

A Windows clone of the branch ran the Rust tests natively (`cargo test --workspace`):

- **First run:** under `RUSTFLAGS="-D warnings"` the build stopped on three unused imports in
  `src-tauri/tests/spine_contention.rs`: the named-pipe test is unix-only and its imports were
  not. cd90a90 gives them the test's `cfg` (earlier Windows builds never denied warnings).
- **Second run:** every crate compiled for Windows without a warning, with every
  `#[cfg(windows)]` path: rift-deploy's Steam registry lookup, the process list and Play's
  `CreateProcessW` start, and the spine's `winreg` handler (`protocol.rs` `mod win`). 806 tests
  passed and 1 failed in 44 test targets; the 8 env-gated benches were ignored. The failure was
  rift-deploy `launch_rules` `play_starts_dd2_exe_as_spelled_on_disk_in_the_game_folder_otherwise`:
  `child_ci` tries the asked-for name first, and a case-blind file system answers it for any
  spelling, so the exe route carried the asked-for spelling instead of the one on disk. 0b34ac4
  makes `game_exe` list the game folder and take the entry whose name matches case-blind.
- **Still owed on Windows:** a test run after 0b34ac4 (the Linux bench passes it); `cargo clippy`
  (not installed on that machine); `npm run tauri build` and its installer; and the presses no
  test makes (§ rift-deploy and § src-tauri, Open).
