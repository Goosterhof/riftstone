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
     nfo htm html rtf doc docx`) → skipped `Document`; *(as built)* a program (`exe msi
     bat cmd ps1 vbs com scr`) or an archive (`zip 7z rar`: the game never opens one,
     465b7d6) → skipped `UnknownPlace`; anything else (`.dll .asi .ini .json` …) →
     `Root { game_path: rel }`;
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
folder package holding zip/7z/rar files that no option lists (as payload or as skipped) gets
one `FilesSkipped` problem that counts them, never names them: "it holds N archives of its
own; put each archive directly in the mods folder." When the folder installs nothing at all,
its `NothingToInstall` problem says the same. (An archive an option lists is the classifier's
to explain, once: § As built (catalog builder), the ruling after the verify pass.)

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
// relink.rs — the relink pass (v0.3): the matcher is pure; the content check reads only what it is handed
pub fn propose_on_shelf(installs: &[InstalledOption], shelf: &[Package]) -> BTreeMap<OptionId, OptionId>;
pub fn propose(installs: &[InstalledOption], options: &[&ModOption]) -> BTreeMap<OptionId, OptionId>; // tests only now
pub fn content_check(install: &InstalledOption, option: &ModOption) -> Option<ContentCheck>; // None: no fingerprint recorded
pub struct ContentCheck { /* install, option, entries (entry → listed size), fingerprinted files and pak groups */ }
impl ContentCheck { pub fn install(&self) -> &OptionId; pub fn option(&self) -> &OptionId;
    pub fn entries(&self) -> impl Iterator<Item = (&str, u64)>; pub fn bytes(&self) -> u64;
    pub fn confirms(&self, prints: &HashMap<String, String>) -> bool; }
pub fn read_fingerprints(check: &ContentCheck, source: &mut dyn PackageSource, scratch: &Path)
    -> RiftResult<HashMap<String, String>>; // entry → the engine's fingerprint; parts removed again
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
12. **The relink matcher (v0.3).** `relink::propose_on_shelf` takes the journal's installs
    and every package of the mods folder (those that yielded no option included). A
    candidate is an install whose package is not on that shelf at all: a package that is
    there but unreadable this scan, or that yielded no option, keeps its installs, because a
    relink is permanent. An option fits a candidate when, compared case-blind: the option
    root is the install's recorded root; every recorded loose, script and root file is in
    the option's payload at the same game path and of the same size (where two entries
    target one path, the later one, as an install writes it); the install's paks are the
    option's paks by size, as many of each (a pak's name in `pak_mods/` is the stone's own,
    so its size and their count tie it to the option); and, when the root is empty or only
    paks speak for the install, the install's name is the option's name or its package's
    (read from `option.trail[0]`, the catalog's package crumb). It proposes a match only for
    exactly one fitting option, never onto an id an install or a backup owner holds, and
    never onto an id two installs both fit. (`relink::propose`, the two-argument form, sees
    only options, so it cannot tell an unreadable package from a gone one; only the tests
    call it.) **The fingerprints confirm (31d26bf).** Sizes are not bytes: a version that kept every
    file's size (a recolor) fits the sizes too. `relink::content_check(install, option)`
    names what confirms a proposal: for every loose, script or root file the install
    recorded a fingerprint for, the entry an install of the option writes at that game path;
    for its fingerprinted paks, the option's pak entries of the same size. `None` when the
    install recorded no fingerprint at all: its sizes are all there is.
    `relink::read_fingerprints(check, source, scratch)` extracts those entries in one pass
    through the package's own `PackageSource` into the caller's scratch folder (every part
    created new, every part removed again before it returns) and fingerprints each with the
    engine's private `fsops::fingerprint`, the one an install records (SHA-256 over the size
    and the first and last MiB). `ContentCheck::confirms(prints)`: every fingerprinted file
    is its entry's, and the fingerprinted paks of each size are found among the option's pak
    entries of that size, each entry used once; a missing entry confirms nothing. A file
    with no fingerprint is held to its size alone. A change that lies wholly between the
    first and last MiB of a file over 2 MiB is not seen, so a same-size version whose
    changes all lie there still relinks (CLAUDE.md, Known Limitations).
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
`launch_rules.rs`: each route, warning and refusal, and `start_exe`. `legacy_rules.rs` also
holds the relink's sizes and fingerprints: a newer archive of the same size told apart by its
fingerprints, paks held to their fingerprints in any order, a file without a fingerprint held to
its size, the later entry for a path read, and a read that fails leaving no part behind.
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
    // endorsements, endorse, abstain, mod_file_details, browse, confirm_download, verify_download,
    // downloader (§ v0.3)
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
  client, the account word, the standings cache, the downloads, the nxm link door (startup
  step 9), the relink's verdicts and the game word. No lock is held across an archive read,
  a game-folder walk, a network call or a file write. Lock order: `deploy_lock` → `world` →
  the rest, and `shelf` → `nexus`.
- `SpineConfig { dirs, discovery, library_dir, app_version, nexus_endpoints,
  watch_library, launcher: Arc<dyn GameLauncher>, game_watch: Arc<dyn GameWatch>,
  game_cadence, protocols: Arc<dyn ProtocolRegistry>, exe }`. `SpineConfig::system(dirs,
  library_dir)` is the app's (the desktop's launcher, process list and registry, the
  running exe). `Discovery::System` tries `RIFTSTONE_GAME_ROOT`, then the folder the
  player picked, then Steam; `Discovery::Fixed { game_root }` (tests) searches nothing.
  There are no roots.
- `persist.rs` — JSON in the app data dir: `settings.json`, `catalog-cache.json`,
  `nexus.json`, `nxm-handler.json` (v0.3); rift-deploy's `journal-<hex>.json` and
  `backups/` live in the same dir; `thumbs/` and `relink-check/` (the relink's content
  check, emptied after every scan and at every launch) in the cache dir. Every write is a
  temp file + rename. A file that cannot be parsed is set aside as `<name>.damaged` and
  starts fresh; one that exists but cannot be read is held: that session never writes over
  it. `nxm-handler.json` is the exception: `persist::inspect_json` reads it where it lies, a
  damaged one is never set aside, and the switch's off is refused while it cannot be read.
  `DataDirs::installed()` finds the installed stone's folders without Tauri (`%APPDATA%` and
  `%LOCALAPPDATA%` with the identifier) for the uninstaller's `--give-back-nxm`. `roots.json`
  (v0.2) is never read, written or deleted.
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
`Cargo.lock`, no `deep-link` feature), then `opener`, `dialog`, `log`. Before any of them,
`run()` answers the uninstaller's `--give-back-nxm` and exits (`nxm::give_back_on_uninstall`).
`tauri.conf.json` declares no URL scheme; its `main` window is built in `setup` (`create:
false`), so a bench data home takes the WebView2 profile along; its `bundle > windows > nsis >
installerHooks` names `windows/hooks.nsh` (v0.3, the uninstaller's give-back).

**Threading:** every command is a thin `#[tauri::command] async fn` wrapper around a plain
`*_impl(&AppState, …)` function, run on Tauri's blocking pool (`commands::blocking`), even
the quick ones; a panic in the work still answers the window with a `RiftError`. Named
threads: `riftstone-account` (started first at the wake, before the scan: the key's
validation, then the link door's first half; also `nxm://premium`'s validation),
`riftstone-wake` (the watchers, the first scan, the launch's update check),
`riftstone-scan`, `riftstone-watch`, `riftstone-game` (the game watcher),
`riftstone-download` (the one download worker), `riftstone-pool`. The relink's content check
runs on the scanning thread (after a game switch, on that command's), with no lock held.
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
write into the real app data; `--give-back-nxm` reads its memory there too when it is set).

**Tests:** dto shape tests (an exact-key test per shape); the window's own files read at
compile time (`include_str!`), so a rename on either side fails: `src/types/rift.ts`'s
`EVENTS` (`events.rs`), PreflightPage's `OWN_CHECKS` (`dto.rs`), every invoke name in
`src/services/rift.ts` (`spine_ipc.rs`), and (v0.3, `dto.rs`)
`error_kinds_and_nexus_reasons_are_the_literals_the_window_reads`, which strips rift.ts's
comments, reads its `RiftError` kind union and its `NexusReason` union, and compares each,
sorted, with `serde_json` of every rift-core variant; exhaustive matches make a variant added
in rift-core fail to build until the test lists it. A
command-layer harness (`tests/support`, `Bench`) builds `AppState` over a tempdir (a fake
game root, one `mods` folder with two invented zips and a Nexus-named download) WITHOUT a
Tauri runtime, through the `*_impl` functions, with a recording launcher, a switched game
watch and a memory registry; `spine_ipc.rs` drives every command through Tauri's mock
runtime by its invoke name and camelCase argument keys. Suites: account, contention,
downloads, flow, ipc, legacy, nexus, nxm, play, startup, window. Gates: `cargo clippy -p
riftstone --all-targets -- -D warnings`, `cargo test -p riftstone`, and `cargo build -p
riftstone`.

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
`src/composables/boot.ts` subscribes to every event before its first `get_overview`, which
the spine takes as the window's signal that it listens (`tests/wake.spec.ts`).
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

The ask opened with the stone standing alone, with no reference to other mod managers left in
it. Then, in the investor's words: *"… i moved the mods to <install dir>\mods where the
riftstone itself is also installed. That should be the one place to place the installable mods,
no need for difficult scanning and multiple places. In the rift i want to see the standard tree
view, so in my case something like 9 packages first and then i can go deeper, … but once i
search i should be able to see everything. I added an api key for nexus but i did not see any
difference in what i can do with it, so let's see if we an do some more with that, and i was
wondering if we can do a download from the riftstone itself? And i want a start to play button
so i can start the game from the riftstone and i like the name riftstone, so i want to work
with that."* (the investor's install path written as `<install dir>`.)

The behaviour is WINDOW-CONTRACT § v0.3 (authoritative for the wire and every rule). This section
is the crate-level contract: who builds what, the public API each crate offers the spine, and the
rules each crate proves with tests.

### Rulings taken for the build (main session, 2026-10-17)

1. **"No reference" covers the gadget's tree**: code, identifiers, comments, tests, fixtures,
   scripts, docs and UI text. They describe what the stone does; where a removal must be
   mentioned, they say "other mod managers", never one product. No forbidden word survives
   anywhere in `crates/`, `src-tauri/`, `src/`, `tests/`, `scripts/`, `docs/`, `README.md`,
   `CLAUDE.md`, `index.html`. Two gates keep it so without spelling a word:
   the stands-alone spec (`tests/standsAlone.spec.ts`) builds the words from their letters'
   codes and reads the
   window's sources and specs, the page, the docs, `README.md`, `CLAUDE.md`, the sweep's
   Python and every Rust file under `src-tauri/` and `crates/` on every `npm test`; the privacy
   sweep's `--forbid` list (in its git-ignored args file) checks the working tree and the tip a
   push publishes. Both match a word of five letters or more anywhere and a shorter one only as
   a whole word, and the sweep never reads a `data:` URI's base64 payload (the audition's
   embedded fonts), where any letters or digits are chance; the spec's own fixtures build even
   their identifiers from the codes. Persisted legacy data is read by a generic lift that names nothing
   (§ rift-deploy). The git history is not rewritten. Pictures are every gate's blind spot: the
   README's frames are shot from the mock and looked at by hand (§ The window, Open).
2. **What survives under neutral words**: the modinfo.ini packaging format (`addonfor`,
   `NameAsBundle`, `DummyMod`, `MenuPriority`, `requires`, screenshots) — it is how mods are
   packaged; adopted installs (origin `adopted`); the `zrift-` prefix and the stripping of the
   numbered `xNNNN-` prefix other tools give paks.
3. **What goes**: everything that found, read or matched other mod managers' data (the search
   for them across the drive, the reader of the install lists they keep, adoption, un-adopted
   owners and their conflict layers), the renamed-crash-reporter check, Find's rule that dropped
   mod managers' names as noise, the Downloads root, custom roots, root filters, the DD2-shaped
   filter and the passed-over memory.
4. **Folder package ids follow the name** (`PackageId::for_folder(name)`); the spine re-keys v0.2
   ids exactly (`PackageId::legacy_folder`) and relinks the rest by option (§ src-tauri).
5. **Downloads**: premium in the stone; free through the site's Mod Manager Download link and an
   opt-in nxm handler that gives the protocol back on opt-out (and, since 80b1ca8, when the
   stone is uninstalled). Never installs by itself.
6. **Play** through Steam when the game came from Steam, else `DD2.exe`. Warnings never block:
   the spine never refuses Play for one, and the window asks once, in Play's inline confirm
   (WINDOW-CONTRACT § v0.3 Play), before it starts the game.
7. **Version 0.3.0** (workspace `Cargo.toml`, `package.json`, `tauri.conf.json`). Name, product
   name, identifier (`nl.zmuuzn.riftstone`) and keyring service unchanged. **0.3.1** on
   2026-10-18: the final review's round (§ The window, After the final review; the spine's
   relink progress and watcher re-arm), so an installer built before it can be told apart.

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
- **Words:** every comment, doc, test name and fixture that names other mod managers or
  "library roots" is reworded (catalog.rs, scan.rs, tree.rs, a modinfo.rs test name,
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
- **Ruled after the verify pass (465b7d6): an archive inside a mod is never payload.** A zip, 7z
  or rar at an option root (beside a `modinfo.ini` or a `natives/` anchor) was `Root` payload
  and would have been copied into the game root, where the game never opens it. It is now a
  skipped `UnknownPlace` file, like a program at the root; one under `natives/` or
  `reframework/` stays payload (there it is a game file). Every archive an option carries gets
  one `FilesSkipped` why of its own: "1 archive sits inside the mod, and the game never opens
  archives. The options list it under skipped files; to install what it holds, put that archive
  directly in the mods folder." (and the plural). The folder scan's held-archives count leaves
  out every archive an option lists (payload or skipped), so one archive is never explained
  twice.

### § rift-deploy (builder: deploy)

- **Gone:** the module that read other mod managers' data (the search for their installs, its
  env override, the parser of the install lists they keep and its types), the context's field
  for such a list, the set of paths it named, the arms of `foreign_below` and of
  `owner_label` that named other mod managers, the helper that named an install by its
  section in such a list, the rule that let such a list satisfy `requires`, the
  `restore_parked` refusal over the files it listed, adoption (the adopt function,
  `adopted_below`, `adopted_kind`), the stack helper that found the layer above such a file,
  `Journal::adopted_sections`, `AdoptReport`, the `WriteAction` and `PakOwner` variants for
  files other mod managers own (those files are `OverwriteUnowned` and `Unowned`), the
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
  *(As fixed after the verify pass: 301b339 added `relink::propose_on_shelf(installs:
  &[InstalledOption], shelf: &[Package])`, which decides presence from every package on the
  shelf, so an unreadable package keeps its installs, and held each recorded file to its size
  and the paks to the option's paks by size and count; 31d26bf added the content check:
  `relink::content_check(install, option) -> Option<ContentCheck>` (`install`, `option`,
  `entries`, `bytes`, `confirms`) and `relink::read_fingerprints(check, source, scratch)`.
  Rule 12 of the base § rift-deploy has the rule as built.)*
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
  mod manager). The x-prefix stripping in `paks.rs` stays, its doc and test renamed.
- **Tests:** the suite of rules over other mod managers' data goes, and `adopted_rules.rs`
  replaces it over a HAND-SEEDED journal of adopted installs (load, uninstall keeps an
  unknown-vanilla root file and deletes nothing-below files, hand-down to a Riftstone layer
  above, a path listed twice in two spellings uninstalls once, reorder renames adopted `x000N-`
  paks to `zrift-NNNN-<stem>`); a lift
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
- Tests: `adopted_rules.rs` (7), `legacy_rules.rs` (16 then; 26 after the verify pass: the
  lift over hand-written schema-1 JSON with the invented kinds `adoptedFromElsewhere` and
  `takenOverByHand` and the owner `elsewhere`, rekey, and the relink by paths, sizes and
  fingerprints), `launch_rules.rs` (8). `common::ctx_with(running)` and `common::adopted(...)` seed
  them. The same-length byte fixtures are `RIVAL GLOW` (10 bytes) and `RIVAL BYTES` (11). The
  bench reads only `RIFT_BENCH_GAME`.
- After the verify pass (301b339): recovery's leftover cleanup saves the journal only when it
  changed it, so a leftover that cannot go yet no longer rewrites the journal at every launch
  (one an older build wrote stays schema 1 until a real change).
- The spine's migration order, one save: build the exact map (each install whose package is a
  v0.2 folder id → `OptionId::new(&PackageId::for_folder(name), id.root())`); apply it to a
  clone with `Journal::rekey`; run `relink::propose_on_shelf` over the clone and every package
  of the shelf (`relink::propose` over every option before 80b1ca8); keep each proposal whose
  install recorded fingerprints only on a verdict that its fingerprints confirmed it (31d26bf, read
  before the migration, § src-tauri); merge both maps; call `engine::rekey_with_files` once on
  the real journal.
- **Windows:** the `CreateProcessW` path in `launch.rs` was first type-checked against
  `windows-sys` 0.61.2 in a scratch crate; it has since compiled natively, and the native run
  found the `DD2.exe` spelling bug that 0b34ac4 fixed (§ v0.3 § The first native Windows run).
  No test spawns the game (`start_exe`'s test only checks its refusals), so the
  `CreateProcessW` call has compiled on Windows but never run there.
- **Open:** press Play once by each route on Windows (Steam, then `DD2.exe` from a folder picked
  by hand). The relink reads a package's name from `ModOption.trail[0]`: if the catalog
  changes how trails start, the name half of the match breaks (the rest does not depend on it).
  `relink::propose` (the two-argument form) has no caller outside the tests and may go.

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
- **Find's terms** lose the rule that dropped mod managers' names, and its tests; everything
  else in `terms.rs` stays.
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
  BTreeMap<u64, String>` (`#[serde(default)]`) and `file_name(file_id)`; after the verify pass
  also `size_of(file_id)`, the file's `size_in_bytes` (never the kilobyte fields): the size a
  download is checked against when v2 states none.
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
  no Dragon's Dogma 2 hit → `Unknown` (Nexus has not hashed it yet); hits for other files
  only → `Err(NotTheFile)`. A malformed md5 → `BadResponse`, nothing sent. After the verify
  pass (301b339) the whole verifying step is `verify_download(&Downloaded, mod_id, file_id)`:
  `confirm_download` on its MD5, and `Unknown` stands only when a stated size vouched for the
  byte count (`Downloaded.size_vouched`); with no size stated it is `BadResponse` ("nothing
  vouched for the download").
- `Downloader` (`Send + Sync + Clone`, never holds a key): `new(app_version)`,
  `with_idle_timeout`, `download_to(uri, dest, expected, progress, cancel) -> Downloaded {
  bytes, md5, size_vouched }`. Reads of up to 1 MiB into a `create_new` file, MD5 on the way,
  `cancel` checked between reads. A `Content-Length` that disagrees with `expected` is refused
  before the body; a body that runs past the stated size is stopped early. A body that neither
  Nexus nor the server sized is stopped past `MAX_UNSTATED_DOWNLOAD_BYTES` (32 GiB;
  `with_unstated_cap` lowers it for tests) with `BadResponse`, and `size_vouched` is `false`
  for one that lands under it. CDN 401/403/410 → `LinkExpired` (never `KeyRejected`), 404 →
  `FileGone`, 429 → `RateLimited`. The URL is stripped from reqwest's errors and scrubbed from
  messages. A drop guard deletes the file on every error, on cancel and on a panic in the
  progress callback; an existing `dest` → `Disk`, left untouched.
- nxm: `NxmMessage { File(NxmLink), Premium }`; `NxmLink::made_for(account_user_id)` is true
  unless both ids are known and differ; `NxmGrant` prints `<redacted>`, never serializes, and
  `expired(now_s)` is `now_s >= expires_s`; `NxmRefusal { OtherGame, Collection, Unreadable }`,
  each a sentence. Ids must be in 1..=i32::MAX and the key printable ASCII without spaces, at
  most 512 characters. A key without its expiry (or the reverse), a `user_id` that is not a
  positive number (a blank one included), or a `key`, `expires` or `user_id` given twice with
  two different values makes the link unreadable (the last two since 301b339); a blank `key`
  with a blank `expires` is no grant.
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
  `MAX_DOWNLOAD_REDIRECTS` 5, `MAX_UNSTATED_DOWNLOAD_BYTES` 32 GiB (301b339).
- Find's rule that dropped mod managers' names (its constant, enum and function) and its test
  are deleted; `terms.rs` keeps `game_name` and a test for the `for`-before-the-game's-name
  rule.
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
  `tauri_plugin_opener::open_path` (made first if missing). *(As built after the verify pass,
  80b1ca8: a folder made at launch and gone since is the `mods` Warn "The mods folder is gone"
  with `CheckAction::OpenModsFolder`, and `open_mods_folder` makes it again, then re-arms the
  watcher and scans whenever it made the folder or nothing watched it. Since the final review
  (2026-10-18), a scan that finds the folder gone drops its watcher, and `rescan` arms one when
  the folder is there and nothing watches it, so a folder made or put back by hand is watched
  again after Open mods folder or Scan.)*
- **Gone:** everything that found, read or matched other mod managers' data (the finder, the
  reader of their install lists, the matcher and its root; `downloads_root`, `custom_root`,
  `same_folder`, the list params of `game_layers`, the stand-in conflict layers), the adopt
  command, `add_root`, `remove_root`, `StoredRoots`, the `roots` checks, `Overview`'s field for
  other mod managers' installs and `roots`.
- **Migration after every scan** (under the deploy lock, never while a change is pending): (1)
  the exact folder re-key: for each folder package now in the mods folder,
  `legacy_folder(<mods dir>/<name>)` → `for_folder(<name>)` for every journal option id and every
  `nexus.json` key that carries the legacy id; (2) the relink pass (`relink::propose` over the
  installs whose package is missing); one `engine::rekey` save, one `nexus.json` save (moving a
  link record follows its package), then `rift://game-changed` and the `relinked` Info check
  (count) for the session. *(As built after the verify pass: the pass is
  `relink::propose_on_shelf` over every package of the shelf (80b1ca8), and its proposals stand
  on their fingerprints (31d26bf): `AppState::confirm_relinks` runs after every scan (in `scan_once`)
  and after `switch_game` lets go of its turn, BEFORE the migration and holding no lock. It
  plans with `migrate::plan_rekey(journal, moves, packages, verdict)`, which returns
  `RekeyPlan { map, files, package_moves, unconfirmed, asked }`: every proposal whose install
  recorded fingerprints and whose `migrate::Confirmation` (the check, the package's path, its
  size and time) has no verdict yet is left out of `map` and listed in `unconfirmed`.
  `migrate::within_budget(unconfirmed, RELINK_READ_BUDGET)` picks what one scan reads (4 GiB
  by the sizes the packages list; a check alone over the budget is never read and gets the
  verdict `false`, so its install keeps its id). Each is read through `rift_archive::open`
  into `DataDirs::relink_scratch()` (`<cache>/relink-check`, emptied after each scan and at
  every launch), one reader at a time, and the verdicts last for the session (those `asked`
  no more are dropped). `migrate_locked` then applies only confirmed proposals and never
  reads. `AppState::relink_checks_read()` counts the reads, for the tests. Since the final
  review (2026-10-18), while a scan confirms a relink by reading bytes, the scan-progress
  event's `current` is "checking {package file name} against the installed files ({i} of
  {n})", n being the confirmations this pass reads; the reads lie inside the scan's span, so
  `rift://library-changed` follows them.)*
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
  `refresh_nexus_account`. The update check no longer waits on the v1 quota. *(As built after
  the verify pass, 80b1ca8: the validation starts first at the wake, before the scan, on its
  own `riftstone-account` thread, so a long first scan never delays it.)*
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
  *(As built after the verify pass, 80b1ca8: the size comes from v2, else v1 `files.json`
  (one read for the name and the size); the MD5 half is rift-nexus's `verify_download`, and a
  lookup that cannot be asked lands on a stated size; the name rule cleans Unicode format
  characters and reserves `CONIN$`/`CONOUT$`, counting 180 UTF-16 units; the landing moves
  with `MoveFileExW` (no `MOVEFILE_REPLACE_EXISTING`) where hard links fail; a file that is no
  archive gets no package id, no pre-link and no scan. WINDOW-CONTRACT § v0.3 Downloads has
  each rule.)*
- **nxm.** `tauri-plugin-single-instance` (2.x for tauri 2.12; as built 2.5.2, pinned in
  `Cargo.lock`) registered FIRST; its callback and the cold-start argv hand any `nxm://` argument
  to `handle_nxm(state, text)` (parse → refusals per WINDOW-CONTRACT § The nxm handler → resume
  the waiting job for the same mod and file, else start one) and bring the main window forward;
  `rift://nxm-received`. A `ProtocolRegistry` seam (as built: `SpineConfig.protocols`, with
  `SpineConfig.exe` naming the stone's own `riftstone.exe`): `System` (Windows, `winreg`; it
  reads HKCU, HKLM and HKCR and writes only HKCU; the previous command is remembered in
  `nxm-handler.json`), `Unsupported` (elsewhere), `Memory` (tests). Never registered at install:
  `tauri.conf.json` declares no deep-link schemes and no deep-link plugin is added. *(As built
  after the verify pass, 80b1ca8: early links wait at a door until the launch's validation has
  run and the window has read its first frame; the memory is schema 2, the user's key as far as
  the claim writes it, and off puts that back; the uninstaller runs `--give-back-nxm` through
  an NSIS hook. The spine's as-built notes below have the names.)*
- **Wire:** dto mirrors of every v0.3 shape with exact-key tests; `WINDOW_CHECK_IDS` and
  `OWN_CHECKS` move together (`mods` replaces the old row); events.rs gains the three events;
  `with_commands` + `spine_ipc.rs` cover every command by invoke name and argument keys.
- **Tests:** the Bench becomes one `mods` folder (the two invented zips and the Nexus-named
  download) with no other mod managers' data anywhere; a seeded schema-1 journal with an
  adopted install loads, lists as adopted, uninstalls; the folder re-key and the relink against
  a bench that moved its mods folder; Play refused/started through the recording launcher; the
  watcher's flip emits once; a download through a loopback API stand-in and a loopback CDN
  stand-in lands under its Nexus name, links itself, and a cut-off body leaves nothing; the nxm
  handler claim/give-back on the memory registry; `handle_nxm` refusals (expired, another
  account, another game, no key).

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
  records, when the account's standing changes, after a track or an endorsement, and when an
  archive lands (a file that is no archive links nothing, since 80b1ca8).
- **DTOs** (each with an exact-key test): gone are `RootKind`, `LibraryRoot`, the summary and
  row views of other mod managers' installs, `JournalOrigin`, `AdoptResultView`, `Overview`'s
  field for them and `roots`, `SearchFilters.rootIds` (a v0.2 query still parses; the filter
  is ignored), `SearchResult.rootId`, `PackageSummary.rootId`, `OptionDetail`'s flag for
  them, `InstalledView`'s un-adopted list, `CheckAction`'s adopt action and `WriteAction`'s
  variant for their files. Added: `ModsFolderView` and `Overview.mods`,
  `LaunchRoute`, `GameSummary.startedMs` / `.launch`, `NexusKeyStanding`,
  `NexusAccountView`, `NexusSummary.key` / `.account`, `InstallOrigin` (`riftstone` |
  `adopted`, in `InstalledInfo.origin` and `InstalledRow.origin`), `OwnerKind::Adopted`,
  `LinkSource::Download`, `CheckAction::OpenModsFolder`, `TreeNode.conflicted`,
  `LibraryPackage`, `LibraryView`, `LaunchWarning`, `LaunchPlanView`, `LaunchReport`,
  `NexusFileCategory`, `NexusFileEntry`, `NexusModFiles`, `NexusBrowseSort` / `Query` /
  `Page`, `EndorseStatus`, `EndorsementView`, `NexusStandings`, `DownloadPhase`,
  `DownloadView`, `DownloadProgressEvent`, `NxmHandlerState` / `View` and `NxmReceivedEvent`
  (Serialize-only, because rift-core's `RiftError` has no Deserialize).
  `WINDOW_CHECK_IDS`: `("mods", "mods")` replaces the old row. After the verify pass
  (31d26bf), `error_kinds_and_nexus_reasons_are_the_literals_the_window_reads` pins rift.ts's
  `RiftError` kinds and `NexusReason` literals against `serde_json` of every rift-core variant
  (§ src-tauri, Tests; a deliberate rename on the Rust side made it fail before it was
  reverted).
- **State:** `SpineConfig` (see § src-tauri above), `SpineConfig::system(dirs, library_dir)`,
  `Discovery::{System, Fixed { game_root }}`, `mods_dir_from`, `MODS_DIR_ENV`,
  `is_partial_download(&str)` (the `.riftstone-<id>.part` rule every download name follows).
  `AppState` gains `library_dir`, `mods_unmade`, `arm_watcher`, `watching_library`, `migrate`,
  `relinked_count`, `watch_game`, `sample_game`, `observe_game`, `game_run`, `game_launched`,
  `deploy_busy`, `installs_under_way`, `begin_install`, `turn`, `try_turn`,
  `queue_launch_links`, `take_launch_links`. A `Turn` holds the deploy lock and, when it drops,
  runs any migration a scan deferred. After the verify pass: `library_unwatched` (80b1ca8),
  the link door (`hold_link`, `launch_validated`, `window_listens`, over a `LinkDoor { held,
  validated, listening }`; 80b1ca8), `confirm_relinks` and `relink_checks_read` (31d26bf).
  `deploy_busy` probes the lock through `try_turn`, so a migration it put off runs when the
  probe's turn ends (80b1ca8).
- **Play** (`play.rs`): the trait `GameLauncher` (`launch(&LaunchRoute)`, `show_folder(&Path)`;
  the launcher also shows the mods folder, so the IPC test can call `open_mods_folder` without
  opening Explorer); `SystemLauncher`; `RecordingLauncher` (`starting(watch)` makes a launch
  flip the watch; `seen()`); `GameWatch`, `SystemGameWatch`, `SwitchedGameWatch` (`set`,
  `asked`); `GameCadence` (2 s; 1 s for 120 s after Play); `GameWord`; `spawn_watcher`.
- **Migration** (`migrate.rs`, pure): `folder_moves`, `plan_rekey`, `journal_ids`, `RekeyPlan {
  map, files, package_moves }`. `NexusStore::move_records(&BTreeMap<PackageId, PackageId>) ->
  bool`; `nexus_error` is `err.into()`. After the verify pass (31d26bf): `plan_rekey(journal,
  moves, packages, verdict)` runs `relink::propose_on_shelf` and asks `verdict` about each
  proposal the fingerprints must confirm; `RekeyPlan` gains `unconfirmed` and `asked`;
  `Confirmation { check, package, stamp }`, `within_budget`, `RELINK_READ_BUDGET` (4 GiB).
- **Account** (`account.rs`): `AccountWord { standing, known, epoch }`, `KnownAccount {
  account, checked_ms }`, `StandingsCache { epoch, tracked, endorsements, checked_ms }`;
  `validate`, `validate_at_wake`, `key_changed`, `account_now`, `key_rejected`,
  `account_user_id`. Every key change bumps the epoch, and an answer that started under an
  older key is never applied. `wake()` scans, then starts the `riftstone-account` thread
  (validate the key, then the launch's nxm links) while the anonymous update check runs
  beside it, so the check never waits on the key or its quota. *(Since 80b1ca8 `wake()`
  starts the `riftstone-account` thread first, before the watchers and the scan; when it is
  done it opens the link door's first half, `nxm::launch_validated`.)*
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
  After the verify pass (80b1ca8):
  - `v1_listing` reads `files.json` once for the archive's name and its size
    (`V1FileList::size_of`); the size checked is v2's, else v1's.
  - `verify` is `NexusClient::verify_download`, and where `Downloaded.size_vouched`, an
    `Offline`, `RateLimited`, `Http` or `BadResponse` from the lookup lands the archive on its
    size (logged); `NotTheFile` always fails.
  - The name rule turns Unicode format characters (`hidden_in_a_name`: category Cf and the
    line and paragraph separators) into `_`, reserves `CONIN$` and `CONOUT$` and the
    superscript `COM`/`LPT` names, and counts 180 UTF-16 units (`LONGEST_NAME`) with the leading
    `_` included (`fit`, `clamp`, `is_reserved`).
  - `rename_new` takes the name by `fs::hard_link` (an `AlreadyExists` moves on to the next
    ` (n)`); where the link fails for another reason, `move_new`: `MoveFileExW` without
    `MOVEFILE_REPLACE_EXISTING` through the folder's canonical (`\\?\`) spelling on Windows,
    `renameat2(RENAME_NOREPLACE)` on Linux (falling back to `copy_new` on `EINVAL`, `ENOSYS` or
    `EOPNOTSUPP`), `copy_new` elsewhere: a copy into a `create_new` file, flushed, then the
    partial removed (a copy that fails removes only the file it made).
  - A landing whose name has no zip, 7z or rar extension gets no package id, no pre-link and no
    scan (`DownloadView.packageId` is `null`).
  - Dependencies: `windows-sys` 0.61 (`Win32_Storage_FileSystem`, Windows only, the version
    rift-deploy already uses) and `libc` 0.2 (Linux only). Cargo.lock gained no new crate
    version: only the riftstone entry's dependency list names them.
- **nxm** (`protocol.rs`, `nxm.rs`): the trait `ProtocolRegistry` (`supported`, `user_command`,
  `machine_command`, `effective_command`, `write_user`, `remove_user`), `system_registry()`,
  `UnsupportedRegistry`, `MemoryRegistry` (`holding`, `user`, `set_user`, `writes`), `command_for`,
  `command_program`, `program_name`, `points_at`, `NOT_ON_THIS_SYSTEM`; `handler_view`,
  `set_handler` (writes `nxm-handler.json` `{ schema: 1, previous: <the command HKCU held, or
  null> }` BEFORE it touches the registry), `nxm_arguments`, `handle_nxm` (never waits on the
  network: the single-instance callback runs on the window's thread), `handle_launch_links`.
  `persist::DataDirs::nxm_handler_file()`. After the verify pass (80b1ca8):
  - `ProtocolRegistry` gains `user_key() -> UserKey` and `restore_user(&UserKey)`. `UserKey {
    exists, default, url_protocol, command_depth, command }` is the user's key as far as the
    claim writes it (`UserKey::claimed`, `handler()`), with `PROTOCOL_NAME` and `COMMAND_PATH`
    (`shell`, `shell\open`, `shell\open\command`). `restore_user` removes the key whole when it
    was not there; otherwise it puts the three values back (deleting the unset ones) and
    removes, innermost first, the command keys the claim made while they are empty.
    `MemoryRegistry` keeps the values the claim never writes (`add_extra`, `extras`,
    `set_user_key`, `user_key_now`) and merges like Windows: a user command key answers even
    with a blank value.
  - `set_handler`'s claim writes `{ schema: 2, previous, before: <UserKey> }` first (a memory
    left by an earlier claim is spent); off is `give_back(registry, exe, memory_file) ->
    Result<bool>`: `restore_user` of the remembered key, with a command that starts a
    `riftstone.exe` (`worth_giving_back`) left out; a schema-1 memory writes its previous
    command back, else removes the key. The memory is read by `persist::inspect_json` and is
    `Nothing`, `Held` or `Unknown(why)`: an unknown memory refuses the off while the key is
    ours, is never set aside or removed, and makes `givesBackTo` null.
  - The uninstaller: `GIVE_BACK_ARG` (`--give-back-nxm`), `give_back_on_uninstall() -> i32` (the
    system registry, `current_exe`, `DataDirs::from_env()` else `DataDirs::installed()`; exit 0
    when nothing points at the exe or there is no registry, else 1) and
    `give_back_as_uninstalled`, which removes the key whole when `give_back` cannot finish
    while the key still starts the exe. `src-tauri/windows/hooks.nsh` defines
    `NSIS_HOOK_PREUNINSTALL`: `${If} $UpdateMode <> 1`, `ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe"
    --give-back-nxm'`, `ClearErrors`.
  - The link door: `second_launch` holds or handles each nxm argument (`AppState::hold_link`),
    `launch_validated` (the account thread, after the validation) and `window_listens` (the
    first `get_overview`) each hand over the held links once both have happened. A cold
    start's links are queued with `queue_launch_links` before `wake()`.
    `handle_launch_links` is gone.
- **lib.rs:** single-instance first, with `second_launch` (each nxm argument to `handle_nxm`,
  then unminimize, show and focus `main`). A cold start queues the argv's nxm links before
  `wake()`. Since 80b1ca8 `run()` first answers `--give-back-nxm` (scanning `args_os`, so an
  argument that is not Unicode never stops the stone from starting) and exits, before any
  plugin; and `second_launch` goes through the link door (`nxm::second_launch`).
- **Windows-only, never compiled on the Linux bench** (`winreg` refuses other targets):
  `protocol.rs`'s `mod win` (`winreg` 0.56: reads the default value of
  `HKCU\Software\Classes\nxm\shell\open\command`, the same path under HKLM, and
  `HKCR\nxm\shell\open\command`; writes `HKCU\Software\Classes\nxm` (`URL:NXM Protocol`, `URL
  Protocol` = "", `shell\open\command`); removes it with `delete_subkey_all`), the
  `cfg(windows)` line in `system_registry()`, and `[target.'cfg(windows)'.dependencies] winreg
  = "0.56"`. These compiled natively on 2026-10-17 (§ v0.3 § The first native Windows run).
  **Added after that run** (80b1ca8): in `mod win`, `user_key`, `restore_user`, `raw_value`,
  `put_value` and `remove_if_empty`; `downloads.rs`'s Windows `move_new` (`MoveFileExW` through
  `fs::canonicalize`) with its `windows-sys` 0.61 dependency, and the `cfg_attr(windows,
  allow(dead_code))` on `copy_new`. These compiled natively, without a warning, on 2026-10-18
  (§ v0.3 § The second native Windows run). `src-tauri/windows/hooks.nsh` is compiled into the
  installer only by `npm run tauri build`, which first built it that day (at 85ba93c). These
  compile everywhere but act only on Windows: the single-instance
  `WM_COPYDATA` callback and the `hard_link` landing on NTFS.
- **Open:**
  - The Windows-only paths compiled and their suites ran natively, the code the verify pass
    added included, and `npm run tauri build` built the installer with its hook (§ v0.3 § The
    first native Windows run, § The second native Windows run). `cargo clippy` has never run on
    Windows (it is not installed on the Windows bench), and nothing has been pressed on a
    Windows desktop yet. Owed, in this order: `cargo clippy` natively; then, once each: press Play by each
    route; use Open mods folder, also after deleting the folder while the stone runs; turn the
    nxm handler on and off, with and without another program registered before the claim, and
    over a key another program left with a `DefaultIcon`; with a free account, press Mod
    Manager Download while the stone runs (a second launch through single-instance) and while
    it is closed (a cold start); with a premium account, download one file and cancel another
    mid-download; download once into a mods folder on FAT32 or exFAT (the `MoveFileExW`
    landing); uninstall with the switch on, with and without another program's handler before
    the claim; and an upgrade over an installed v0.3.
  - `tauri-plugin-single-instance` is pinned to 2.5.2 (published 2026-10-01) by a hand-edited
    lock entry (version and checksum), because `cargo update --precise` also moved unrelated
    `windows-sys` entries. `cargo metadata --locked` passes; a later `cargo update` will move
    it.
  - Fixed after the verify pass (80b1ca8): `deploy_busy()`'s probe (from `plan_launch`) went
    through a bare `try_lock`, so a scan's migration that met it waited for the next turn or
    scan; the probe is a `Turn` now, and the migration runs when it ends.
  - A Nexus file that is not a zip, 7z or rar (a raw `.pak` or `.exe`) lands under its own
    name, and the scan never lists it (no package id, no link, no scan since 80b1ca8).
  - The relink's 4 GiB budget counts the entries' listed sizes. A solid 7z or RAR may decode
    more than that to reach them, so the CPU time a scan spends on solid archives is not
    strictly bounded by it.
  - An archive overwritten in place by another version with the same name and the same size
    keeps its package id (`for_archive(name, size)`), so no relink is proposed: the install
    shows on the new archive's option while the game folder holds the old bytes.
  - A relink confirms each file by the engine's fingerprint (its size and its first and last
    MiB), not by every byte. A newer version of the same size whose changes all lie between the
    first and last MiB of a file over 2 MiB still takes over the older install, and the Rift
    then shows the newer option installed while the game folder holds the older bytes
    (CLAUDE.md, Known Limitations).
  - Two failures carry no reason code, so the window voices both as an answer it could not
    read: no download link offered (`downloads.rs`) and an HTTP 401/403 on a key that
    validates (rift-nexus `http_sentence`). A reason each would let the window say what
    happened.
  - The new scratch-folder extraction (`relink-check`) and its `remove_dir_all` under
    `%LOCALAPPDATA%` have not run on Windows; an antivirus may slow the part writes.

### § The window (builder: window — the Illusionist, after the Artisan's amendment to #00015)

Everything in WINDOW-CONTRACT § v0.3 that the player sees: the library tree as the Rift's idle
view (with the trouble fold above it), Play on the stone, the mods-folder row and empty state, the
account card, downloads, browsing Nexus, a mod's files and patch notes, track and endorse, the nxm
handler switch, an uninstall door on every load-order row, every surface for other mod managers
and for several roots removed, the mock and specs to match, and a source sweep spec
(the stands-alone spec, `tests/standsAlone.spec.ts`) that proves the forbidden words appear nowhere without spelling
them.

**As built (four window builders, 2026-10-17: the floor, the tree and Play, the far world, the
specs; committed in c7ed0c2).** 227 specs in 26 files (new: tree 14, play 7, farWorld 22,
modsFolder 6, yourWorld 3, standsAlone 3), green over 10 shuffled runs; typecheck, lint, format
and build clean.
- **New:** `LibraryTree.vue`, `LibraryBranch.vue`, `PlayStone.vue`, `AccountCard.vue`,
  `NxmSwitch.vue`, `DownloadShelf.vue`, `ModFiles.vue`, `NexusBrowse.vue`, `FarModCard.vue`,
  `StandingControls.vue`; the composables `useLibrary`, `useLaunch`, `useDownloads`,
  `useNexusAccount` and `useNxmHandler`, each reset by `resetRiftstone`; the mock's `far.ts` and
  `library.ts`; the scenes `emptymods`, `premium`, `rejected`, `running` and `nxmother`.
- **Choices the builders made where §16 was silent or read otherwise** (as committed in c7ed0c2;
  where the verify pass changed one, it says so):
  1. Keys: Space zooms an option's screenshot, else acts as Enter; `*` opens every sibling
     branch; ↓ from the well lands on the row the tree last held, else the first. These follow
     WINDOW-CONTRACT § v0.3 The library tree, which is authoritative for behaviour. *(R16.6 was
     amended to these keys on 2026-10-18.)*
  2. R16.10's 12ch label floor is approximated: the kind badges fold to `+N kinds` when a row is
     at most 420 px wide (a container query in `LibraryBranch.vue`), an estimate, not the 12ch
     rule itself.
  3. 4 px (`--rs-space-1`) between collapsed package slabs; R16.4 names only the 8 px after an
     expanded one.
  4. Play while stirring is drawn pressable (R16.13). *(As committed, `useLaunch().play()`
     ignored a press while a launch was in flight. Since 85ba93c a press while THE WORLD STIRS
     plans again and calls `launch_game` again, and the wait starts over, so a start that never
     comes leaves no dead stone; only a press whose `plan_launch` and `launch_game` are still on
     their way swallows the next. The spine refuses `launch_game` once its watcher believes the
     game runs.)*
  5. A free account's update row reads `Get it on Nexus` (it starts a waiting download and opens
     the file's page); R16.23 names only `Download v{version}`. The files view says `Download`
     for every account, as R16.21 rules. *(Settled 2026-10-18: R16.23 now names `Get it on
     Nexus` for a free account.)*
  6. `Replace key…` opens THE FAR WORLD's own key form, as v0.2 did; R16.17 points it at
     Preflight's key row. *(Since the final review, 2026-10-18, every key door opens that form
     and none sends the player to PREFLIGHT for the key: § After the final review, below.
     Settled 2026-10-18: R16.17 now points `Replace key…` at that form.)*
  7. The TRACKED section's head reads `TRACKED ON NEXUS`; R16.16 names it `TRACKED`. *(Settled
     2026-10-18: R16.16 and R16.20 now name `TRACKED ON NEXUS`.)*
  8. The mock's other nxm program had a placeholder name; R16.24 names `Wagonmaster.exe`.
     *(Since the final review, 2026-10-18, the mock's program is `Wagonmaster.exe`
     (`src/services/mock/far.ts`), as R16.24 names it.)*
  9. `Fix it` maps each warning to its PREFLIGHT row (`reframeworkMissing` → `reframework`,
     `pakFolderOff` → `pakdir`, `looseLoaderOff` → `loose`, `buildChanged` → `build`); the rows
     carry `data-check`. *(Since 85ba93c the keyboard lands on that row's own button, else on
     the row, so the focus never drops to the page.)*
  10. Tracked cards carry no Download (a card holds no file id): a download goes through Files.
  11. A refusal to track or endorse is kept per mod and said under the pair, never as a toast
      (R16.22).
- **Drift from §16 recorded at c7ed0c2 or found by the verify pass**, all settled by 85ba93c and
  the 2026-10-18 amendments of #00015: R16.25 (now amended to the build: `Open mods folder` is
  the row's Primary and `Scan` its Secondary whenever the `mods` check's action is
  `openModsFolder`, else `Scan` Primary; and built to it: the path ellipsized from the left with
  the whole path in `title`, the check's detail on a line of its own); R16.26 (built: `its
  package isn't in your mods folder`, and `adopted` with its `title`); R16.4's slabs in the
  package view (`PackageTree.vue` now follows R16.3–R16.5).
- **After the verify pass (committed in 85ba93c, 2026-10-18).** 309 specs in 32 files (227 in
  26 before; new files `account`, `downloads`, `modFiles`, `packageTree`, `treeDraw`, `wake`),
  green over shuffled runs; `npm run typecheck`, `lint`, `test`, `format:check` and `build`
  clean (run again for this record on 2026-10-18: 309 of 309). New: `ModFileRow.vue`; the
  scenes `nofloor` and `gonemods`. What changed, each with a spec that failed first:
  - The tree: every letter typed goes to the well (only `1`–`4` and `/` pass through); each
    row's accessible name is its label and its description the format, counts, problems,
    chips and badges; a package row's verdict comes from the trouble kinds only; formats read
    `ZIP`, `7Z`, `RAR` and `FOLDER`. The package view's `PackageTree.vue` follows R16.3–R16.5:
    option rows on the void at 32 px, a 2 px state rail on each leaf (none on menus or
    bundles), 16 px per level with a 1 px `--rs-edge` guide, `--rs-misthi` on hover, the focus
    ring inside the row, a `CONFLICT` chip on a conflicted leaf and no chips or kind badges on
    bundles and menus; a bundle no longer wears the selected frame when no option is open.
  - Play: item 4 above; a plan read while the game ran is void once the overview says it
    stopped (`planStale`), so Play never shows a stale "already running"; `Fix it`'s focus
    (item 9).
  - The mods folder: `useStone()` gains `modsWantOpen` and `modsMissing` (`gone` or `unmade`);
    the Rift carves THE RIFT LOST ITS FLOOR (warn) with `Open mods folder` for a folder that
    went, and keeps THE RIFT HAS NO FLOOR for one that could never be made; PREFLIGHT's row as
    R16.25 now reads. The mock's `nofloor` sends the spine's Error word for word, and
    `gonemods` the Gone Warn, whose `Open mods folder` makes the folder again, empty, then
    scans.
  - THE FAR WORLD: the verifying phase reaches the shelf (`Checking the file`); an update row is
    read again after a landing's scan; the low-quota line says update checks don't use the
    quota, and nothing gates `Check now`; the update row's `Download v{version}` / `Get it on
    Nexus` and a FILES row's `Download` are held (`refused`, reason `Downloading: see
    DOWNLOADS`) while that download is unfinished, the mod's other files free; a free
    account's start toast reads the nxm handler first and, while nothing answers Nexus's
    button, says so and offers `Answer Nexus's button`; a landed file that is no archive says
    the Rift does not list it and offers `Show in folder`; landed toasts are cyan with `Show in
    the Rift` (gold stays for THE CROSSING and THE WORLD IS AWAKE); the nxm switch stands alone
    in ACCOUNT when no card shows, so the stone can always give the link back; Endorse and
    Abstain live in a mod's FILES only, and the update rows' buttons wrap below 1199 px; the
    subtitle names what the page holds (`Your account, downloads, updates, tracked mods and
    Find on Nexus · updates checked …`); an accepted nxm link is told under THE FAR WORLD
    ANSWERS with `Show downloads`, and a refusal's `Open on Nexus` opens the file's own page
    (`file_id`, `nmm=1`), where Nexus makes a fresh link.
  - The voice: §9's sentences carried into `voice.ts` and `DownloadShelf.vue` (a link for
    another account, Premium required's first sentence, no key in the download and nxm
    contexts, the write failure's partial-file sentence, the waiting row's second sentence,
    `The download of X stopped.`).
  - The wake: `boot.ts` keeps `await subscribe()` before the first `refreshOverview()`, the
    spine's sign that the window listens (WINDOW-CONTRACT startup step 9); `tests/wake.spec.ts`
    fails when the order is reversed.
  - The stands-alone spec (`tests/standsAlone.spec.ts`) reads the docs, `README.md`, `CLAUDE.md`, the sweep's Python
    and every Rust file too (§ Rulings, 1), and never matches `scripts/privacy-sweep.args`.
- **After the final review (2026-10-18, 0.3.1).** 316 specs in 32 files (new: the tree tools'
  absence, the key doors from FILES, a held Download, the refused link's toast and a Find row,
  the switch's line, the gone-package line); typecheck, lint, format and build clean. The Rust
  workspace passes 878 tests on the Linux bench (8 env-gated benches ignored, 44 test targets).
  - THE FAR WORLD IS SILENT's body (no key): "Add your Nexus Mods personal API key and the stone
    will show your account, and download, track and endorse mods for you. It stays on this PC.
    Update checks and browsing Nexus work without it." The update check is anonymous, so the
    body no longer promises it as the key's work.
  - The key's doors (WINDOW-CONTRACT § The key's extras): a mod's FILES without a key says "Add
    your Nexus key to download from the stone." with a small button `Add key…`; a held
    Download's reason without a key is the same sentence; a refused nxm link's toast offers the
    voice's own recovery label (`Add key…` or `Replace key…`) instead of `Open Preflight`. Every
    one of these doors opens THE FAR WORLD's key form: it goes to THE FAR WORLD if elsewhere,
    opens the ACCOUNT section's key form and focuses its password field (NexusPage's
    `replaceKey`). No door sends the player to PREFLIGHT for the key. PREFLIGHT's own NEXUS KEY
    row (`Add` / `Replace`) is the same door, and so is a Find row failure's `Add key…` /
    `Replace key…` (a link Nexus refused for the key lands on the row): each opens the form with
    its field focused.
  - The nxm switch's line while another program answers: "{other} answers it now. Turn this on
    and the stone remembers it, and gives it back when you turn this off." ({other} is the
    program's name, else "Another program".) Its state (ON / OFF) is a plain mono word: no
    border, padding or radius; ON in the Rift's cyan, OFF muted. Both as R16.24 reads.
  - The mock's `?mock=nxmother` program is `Wagonmaster.exe` (item 8).
  - The scan's progress names a relink's read (WINDOW-CONTRACT § v0.3 Legacy data, When it
    runs), and the window shows that `current` as it shows any ("now: …").
  - The source sweep spec is the stands-alone spec, `tests/standsAlone.spec.ts`; its test reads
    "should carry none of the forbidden words". Its fixtures build every forbidden letter run
    from the words' character codes, including the identifier that proves the short word is
    matched only as a word of its own.
  - The tree header's `Expand all` and `Collapse all` are drawn only when the library has a
    package and the folder is there, as the answers' sort and group tools already were.
  - The small leftovers are gone: `ledger.ts`'s `DOWNLOAD_PHASE_WORD` and `LoadOrderList.vue`'s
    unused `.is-warn` rule are removed, and YOUR WORLD's gone-package line (`home__gone`) is
    drawn as R16.26 says, Alegreya 14 in `--rs-faint`.
- **§16 and §9 drift:** none left. On 2026-10-18 #00015 was amended to the build: items 5–8
  above (R16.23, R16.17, R16.16 and R16.20, R16.24), R16.18's Failed and Waiting rows, R16.21's
  keyless line, R16.25's two folder states, and §9 row by row (the expired link, another game or
  a collection, another account, Premium required, the nothing-changed sentences, the free
  account's start toasts, no key in and outside the download and nxm contexts, key rejected,
  offline and not checked yet with their ACCOUNT doors, endorsing refused). §15 question 2 is
  written as measured.
- **Measured on the mock** (headless Chromium over `npm run dev`, 2026-10-17, by the v0.3 review
  and its fixers; #00015 §16.8 records Q16.1–Q16.3 as measured since 2026-10-18, its Q16.3
  leaving smoothness in WebView2 on a 3,000-option library unmeasured; §15 question 2 reads
  measured since 2026-10-18):
  - Q16.1: at 1024×680 the Play slab sits wholly inside the window in every state, above the
    foot at 634 px: Ready 573–634, Running 565–634, refused 531–634, a two-warning confirm
    478–634. The facet block keeps 211 px of its 602 px content at rest (169 refused, 116 in
    the confirm) and scrolls. At 1320×860: the slab at 753–814, the facets 385 of 532 px. The
    build applies §8.0's fallback: `Show menu-only` sits above `STATE`.
  - Q16.2: `Play anyway` (95 px) and `Fix it` (53 px) share one line in the slab's 216 px at
    1024×680.
  - Q16.3: 250 answers are on screen 358 ms (1320×860) and 382 ms (1024×680) after the
    keystroke, the 120 ms debounce included; the scroll's frame p95 is 17.6 and 17.2 ms. No
    virtualisation is needed at this size.
  - §15 question 2: the seam's lit length at reach 1 is 746 px at 1320×860 and 572 px at
    1024×680 (predicted 742 and 562).
- **Open:**
  - Q16.3's smoothness in WebView2 on a real 3,000-option library (the mock's 250 answers
    scroll at a frame p95 of about 17 ms in headless Chromium).
  - Fixed in 85ba93c: the stale refusal Play showed for one plan read after the game stopped
    (`planStale`).
  - Play's second press while THE WORLD STIRS asks Steam or Windows again until the watcher
    (sampling every 1 s after a launch) sees the game: on the exe route Windows is asked to
    start `DD2.exe` once more. What Steam and the game do with a start that is already under way
    is unmeasured: pressing each route twice in quick succession is owed with the Windows
    presses.
  - The README's frames: the three v0.2 shots (the roots facet, no Play, and words ruling 1
    forbids in their pixels) were shot again on 2026-10-18 from the v0.3 mock, beside two new
    ones: `docs/images/riftstone-tree.png`, `riftstone-answers.png`, `riftstone-crossing.png`,
    `riftstone-far-world.png` and `riftstone-nothing-answers.png`. Each was looked at for this
    record: invented names only, no forbidden word, and the README embeds all five again. All five were shot
    again after the final review's round (the nxm switch's state word changed in the far world
    frame) and looked at once more: invented names only, no forbidden word, no text chunks. No
    text gate can see a picture, so the next window change that shows in them owes new frames
    and another look (CLAUDE.md, Privacy). The older commits keep the v0.2 frames (ruling 1:
    the history is not rewritten).
  - A mock divergence: the mock refuses at once an nxm link for a file the mod's list lacks
    (`fileGone` in the event), while the spine accepts the link and the download fails later, at
    linking.

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
- **Still owed on Windows after this run:** a native run of the tree as it stands (31d26bf for
  the Rust, 85ba93c for the window), the first compile of the Windows-only code the verify pass
  added (`protocol.rs`'s key memory in `mod win`, `downloads.rs`'s `MoveFileExW` `move_new`, the
  `cfg_attr(windows, allow(dead_code))` on `copy_new`) and the first build of
  `src-tauri/windows/hooks.nsh`. The second run, below, did all three. The Linux bench passes
  875 workspace tests at 31d26bf (8 env-gated benches ignored, 44 test targets) and the window's
  309 specs at 85ba93c.

### The second native Windows run (2026-10-18, at 31d26bf)

- **Tests:** `cargo test --workspace` ran natively on Windows (MSVC, Rust 1.98.0,
  `RUSTFLAGS=-D warnings`) at 31d26bf: 861 passed, 0 failed. So every `#[cfg(windows)]` line up
  to 80b1ca8 compiles there without a warning, including `mod win`'s `user_key`,
  `restore_user`, `raw_value`, `put_value` and `remove_if_empty`, `downloads.rs`'s `move_new`
  and `copy_new`'s `cfg_attr`.
- **Lint:** `cargo clippy` is not installed on the Windows bench and has never run there.
- **Build:** `npm run tauri build` at 85ba93c built `riftstone.exe` and the NSIS installer with
  the uninstall hook (5.88 MiB), the hook's first build.
- **Probe:** a read-only bench probe of that build, on a copy of real app data, passed. Nothing
  it counted or named is recorded here (CLAUDE.md, Privacy).
- **Still owed:** `cargo clippy` natively, and every press on a Windows desktop: Play by each
  route; Open mods folder, also after deleting it while the stone runs; the nxm switch on and
  off, also over another program's key; a download with each account type, one cancelled, one
  into a FAT32 or exFAT folder; uninstall with the switch on; an upgrade over an installed v0.3
  (§ src-tauri, Open, lists each press in full).

### The third native Windows run (2026-10-18, 0.3.1 at d68df79)

- **Tests:** `cargo test --workspace --no-fail-fast` natively (MSVC, Rust 1.98.0,
  `RUSTFLAGS=-D warnings`) at d68df79: 864 passed, 0 failed, 8 ignored in 44 test targets. The
  final review's round (the relink progress, the watcher's re-arm) compiles there without a
  warning. 75de070 changed only a test fixture's number, the docs and the sweep after it.
- **Build:** `npm run tauri build` at d68df79 built `riftstone.exe` and
  `The Riftstone_0.3.1_x64-setup.exe` (5.88 MiB).
- **Probe:** the same read-only bench probe, on a copy of real app data, passed again; the stone
  was idle about 5 s after its launch, the relink's reading included. Nothing it counted or
  named is recorded here.
- **Still owed:** as after the second run. `cargo clippy` is still not installed on the
  Windows bench.

## § v0.4 — the Muster: the party from the player's own saves (investor ask, 2026-10-18)

The ask, in the investor's words: *"Can we also check savegames? So I can see equipement of the
team? Vocations, stats and stuff like that?"*, and after the feasibility read: *"go for both,
let's do the muster and let's publish"*. The Muster is a new page: the party of the save slot the
player picks (the Arisen, the main pawn and the hired pawns) with each one's level, vocation,
vocation experience, the stats their levels gave, everything they wear by slot, the augments and
skills set for the vocation they follow, a pawn's inclination and specialization, and the
Arisen's gold and Rift Crystals. It reads; it never writes.

The behaviour is WINDOW-CONTRACT § v0.4 (authoritative for the wire). This section is the
crate-level contract and the ground truth it rests on.

### The ground truth (measured 2026-10-18 on a copy of the investor's save, read-only)

Every fact below was read off the investor's own save and game install by a read-only spike that
printed structure, counts and game text, never a character's name or an id. The party it read
was confirmed in-game by the investor (a specialization, and armour the first pass had missed).

- **Where.** Steam keeps DD2's saves in `<Steam>\userdata\<account id>\2054970\remote\win64_save\`:
  `data0NNSlot.bin` per save slot (the 2026 slots are 428,780 bytes each), `data000.bin`,
  `data00-1.bin` (system settings, no party) and `SS1_*` files. The account id is the folder's
  name, a 32-bit number; it is also the key.
- **The container (DSSS).** `DSSS`, u32 version 2, u32 flags (`0x10` Mandarin, `0x08` deflate:
  the 2026 slots carry `0x18`), u32 0; then the encrypted slices; then a 0x8C-byte footer: a
  128-byte salt, the u64 decrypted length and a u32 MurmurHash3 x86_32 (seed `0xFFFFFFFF`) of
  every byte before it.
- **Mandarin** (the cipher's community name; the math follows MandarinJuice, MIT, © 2026 Michał
  Gębicki, <https://github.com/mi5hmash/MandarinJuice>). DD2's seed is `0x5EC646997D69AE1B` and
  `uid = !(account_id as u64)`.
  - Slice sizes come from SplitMix64 over `(len >> 14) + 1` laps: each lap that still has bytes to
    cover takes `((state & 7) + 1) << 14`. Then `state += uid`.
  - Per slice: 16 SplitMix steps are skipped. The 528-byte slice header is XOR-ed with the low
    byte of each next step. It holds four ElGamal pairs `(c1, c2)` of 64 bytes each over the
    256-bit prime `p`, then a u64 checksum and a u64 length.
  - Each pair gives one u64 of the slice key: `(c2 / c1^uid mod p) mod 2^64`. The four u64s are
    an AES-128 key and an OFB IV for the slice's bytes.
  - Every `c1` is the constant `g^20 mod p`, so a wrong account fails on the first header at
    once, before any AES. The math is all public: decrypting needs no secret beyond the account.
- **Deflate (`0x08`).** The decrypted block is u64 `zsize + 16`, u32 1, u32 `zsize`, u64 size,
  then `zsize` bytes of raw deflate.
- **The class stream needs no type dump.** It is `(u32 field hash, Class)*`. A Class is a u32
  field count, a u32 type hash, then its fields. A Field is a u32 MurmurHash3 (seed `0xFFFFFFFF`)
  of the field's UTF-8 name, an i32 kind and a value. The kinds:
  - −1 array; 1 enum; 2 bool; 3–14 the integers, floats and chars; 15 string; 16 struct;
    17 class.
  - A scalar aligns to 4, carries a u32 size, aligns to that size, then the value.
  - A string aligns to 4, carries a u32 length, then UTF-16.
  - An array aligns to 4, carries the i32 member kind, u32 member size, u32 count and i32 array
    kind (0 values, 1 classes; a class array may carry a `0xFFEEFFEE` marker and one type hash
    per item), then its items, then aligns to 4.
  - Alignment counts from the file's start, after the 16-byte header, because a struct aligns to
    its own size.
  - The investor's 3.77 MB payload parses end to end this way: 227 class types, about 260,000
    values. The field names the Muster needs were matched against RE_RSZ's `rszdd2.json` (MIT,
    alphazolam, <https://github.com/alphazolam/RE_RSZ>). The stone carries only those names and
    hashes them itself.
- **The party.** In `app.ContextDatabaseSaveDataRecord.Contexts`, every record that holds an
  `app.JobContext` is one member (four in the investor's save). The member's other contexts:
  - The role. `app.PlayerPersonalDataContext` marks the Arisen (`Name`, `NickName`),
    `app.MainPawnDataContext` the main pawn, and `app.SubPawnDataContext` a hired pawn.
  - Pawns. `app.PawnDataContext` holds `_Name`, `_Nickname`, `_Personality` (a PersonalityID
    hash) and `_SubSkill`. `app.PawnOwnerContext` holds the creator's `_Name`, `_Level` and
    `_Job`.
  - Level and stats. `app.HumanStatusContext` holds `_Level`, `_Exp` and `_IncreaseParams`
    (`_HitPoint`, `_Stamina`, `_Attack`, `_Defence`, `_MagicAttack`, `_MagicDefence`, `_Weight`,
    `_Blow`: what the levels gave).
  - Vocation, augments and skills. `app.JobContext` holds `CurrentJob` (1–10) and
    `CulmativeExp[10]`. `app.AbilityContext.EquipedAbilities[11]` holds seven augment ids per
    vocation, and `app.HumanSkillContext.EquipedSkills[11]` six skill ids per vocation, both
    indexed by vocation number.
  - Identity. `app.CharacterIDContext.CharacterID` holds the member's character id.
- **The gear.** `app.ItemManager.GameSaveData.ItemStorageSaves` holds `_CharaId`
  (= `CharacterID & 0xFFFFFFFF`), `_ItemId`, `_EquipSlot` (0 main weapon, 1 sub weapon, 2 head,
  3 body, 4 legs, 5 cloak, 6 and 7 the rings) and `_IsEquipped`.
  - **The list mixes two classes.** It holds `ItemStorageBackup` and its subclass
    `ExpandItemStorageBackup` (15 of 4,350 in the investor's save, adding `_ExpandValues`), and
    the subclass carried the investor's Arisen's body and leg armour. The Muster reads every
    item in the list that carries the item fields, whatever its class.
  - `MoneySave` (`app.ItemManager.MoneyBackup`) holds `_Gold` and `_Rim` (Rift Crystals).
- **The names live in the game's own pak.**
  - **The pak.** `re_chunk_000.pak` is 69 GB: KPKA v4.2, flags `0x68`, 318,966 entries. Its
    table is encrypted. After the table come a chunk table of 16 bytes per chunk, the 128-byte
    key and a chunk table of 8 bytes per chunk. The key is the RSA public operation
    (`e = 65537` and the 1024-bit modulus every RE Engine pak reader publishes), then
    `t[i] ^= i + k[i % 32] × k[i % 29]`. Reading and decrypting the table takes 2.3 s over the
    WSL bench's mount.
  - **The entries** come in three forms:
    - chunked (attributes `0x1000000`, `0x1000400`, `0x1000800`, `0x1000C00`): 512 KiB chunks,
      zstd unless a chunk is exactly 512 KiB;
    - deflate (1) or zstd (2);
    - encrypted resources (attribute bits 16–23 in 1–4): 128-byte blocks, each a 64-byte `k`
      and a 64-byte `d`, giving 8 plaintext bytes of `d / k^E mod M`.
  - **References for the format.** ree-pak-rs (MIT, eigeen, <https://github.com/eigeen/ree-pak-rs>)
    and the facts REE.PAK.Tool documents (it has no license: read, never copied).
  - **The text.** `.msg.24` is GMSG v`0x18`: entries of GUID, u32, u32 hash, a name pointer, an
    attributes pointer and one text pointer per language (14; English = 1). The strings are
    UTF-16 in a data block XOR-chained with a 16-byte key (`prev ^ cur ^ key[i & 15]`). The files
    the Muster reads, under `natives/stm/message/ui/` unless noted:
    - `itemname`, `itemname_end` and `appdata/extracontents/contents_0011/itemname_contents_0011`
      (`item_name_<id>`);
    - `job` (`job_name_01`–`10`);
    - `personality` (`Personality_00`–`03`);
    - `subskill` and `subskill_tu_end` (`Subskill_NN_00`);
    - `job01act`–`job10act` (`jobNN_skill_MM`, `jobNN_ability_MM`).
- **The id rules**, each checked on the investor's party:
  - **Vocation:** `CurrentJob` N reads `job_name_NN`.
  - **Inclination:** the PersonalityID hash Kind / Cool / Simple / Frank reads `Personality_00` /
    `01` / `02` / `03`.
  - **Specialization:** N reads `Subskill_{N−1:02}_00`, 0 is none. The investor confirmed one
    in-game.
  - **Augment:** id `4 + (job − 1) × 5 + i` for vocations 1–9 and `49 + i` for vocation 10 reads
    `jobNN_ability_{i:02}`; vocation 10's text counts from 01, so `i + 1` there. Every augment the
    four characters had equipped decoded to a vocation that character has experience in.
  - **Skill:** the custom-skill ids run in one block per vocation (1–12, 13–23, 24–37, 38–49,
    50–61, 62–69, 70–79, 80–91, 92–99, 100 for vocations 1–10), and `MM` is the id minus its
    block's first. One id breaks the pattern: 101 is a Thief skill a title update appended, and
    it is the 13th Thief skill in the text (`job04_skill_12`). A skill's text can be a
    reference (`<REF job03_skill_00>`: a Sorcerer skill the Mage shares), resolved through the
    referenced entry. The decoded loadouts read as real loadouts: the main pawn's five Thief
    skills, the Arisen's three Mystic Spearhand skills and the hired Mage's six.
  - Ids beyond these tables, added by title updates, stay unnamed: the window says so and shows
    the id.

### Rulings taken for the build (main session, 2026-10-18)

1. **RS-8 — the saves are the player's.** The stone reads a save into memory and nothing else:
   it never writes, renames, re-signs or copies a save file, never sends one, and keeps no
   decrypted copy on disk. The stone carries no way to encrypt a save. The test fixtures are
   encrypted by code compiled only for tests (`rift-save`'s `fixtures` feature, a dev-dependency
   only).
   **Amendment, investor 2026-10-27:** the explicit request to back up saves and
   add a Riftstone option permits copies of the original encrypted files to a
   separate chosen destination. It permits no original-save writes, restoration,
   re-signing, uploading or decrypted-on-disk data. The manual backup contract
   below supersedes only the original no-copy clause.
2. **Names come from the player's own game, never from the repo** (the Smuggler's Ledger's legal
   floor). The pak is opened read-only (RS-3 still forbids writing it). The name tables are
   cached in the app's cache folder, keyed by the pak's size and modified time, so the 2 s read
   runs once per game update. Without the game the Muster still shows the party, with ids, and
   says where the names live. The mock, which ships as a lazy chunk, carries invented names only:
   invented gear, augments and skills, and only the six vocations whose names are plain English
   words.
3. **What is private in a save:** the Arisen's name and nickname, the pawns' names, nicknames
   and Rift ids (`_SearchId`), the hired pawns' creators' names and user ids, the slot titles,
   and the account id. The window shows names, because it is the player's own screen. Logs,
   errors, events, bench output and the repo never carry them. The privacy sweep learns to read
   saves (`--saves`), and fixtures use invented people only.
4. **The Muster never waits on the game being closed.** DD2 writes a slot when it saves. The
   stone reads whole files and retries a read that lands mid-write (a short file or a bad
   signature) once, 500 ms later.
5. **Version 0.4.0** (workspace `Cargo.toml`, `package.json`, `tauri.conf.json`), and **0.4.1** since
   the General's review round (`e24a73a`), so the 0.4.0 installer built before the round can be told
   apart from one built after it, as 0.3.1 was for v0.3.

### § rift-save (new crate: the DSSS container, Mandarin, the class stream)

- `rift_save::open(bytes: &[u8], account_id: u32) -> Result<Vec<u8>, SaveError>`: the decrypted
  (and inflated) payload. `SaveError` kinds: `NotDsss`, `Version(u32)`, `Truncated`,
  `SliceLayout`, `WrongAccount`, `Inflate`, `Signature` (the footer's MurmurHash3 disagrees: a
  read mid-write). No error message carries a byte of the payload.
- `rift_save::stream::parse(payload: &[u8]) -> Result<Vec<Root>, StreamError>`: the whole tree,
  `Value::{Bool, Int(i64), UInt(u64), Float(f64), Str(String), Struct(Vec<u8>), Class(Class),
  Array(Vec<Value>)}` with `Class { type_hash, fields: Vec<(u32, Value)> }`.
  `field(name)` hashes the name and looks it up. A malformed stream names the byte offset, never
  the bytes.
- `rift_save::field_hash(name: &str) -> u32` (rift-pak's MurmurHash3, seed `0xFFFFFFFF`).
- `fixtures` feature only: `seal(payload, account_id, deflate: bool) -> Vec<u8>` (Mandarin
  encrypt, after MandarinJuice's `EncryptData`) and a tiny stream writer, so every test builds its
  saves at run time from invented data.
- Tests:
  - round trips at several sizes, including an exact multiple of 16 KiB (the laps rule's edge);
  - a wrong account is `WrongAccount` before any AES;
  - a flipped footer byte is `Signature`, and a cut file is `Truncated`;
  - the stream's alignment cases: a 24-byte struct at an offset that is 8 but not 16 mod 24, a
    class array with and without the marker, an empty string;
  - the Mandarin math checked against MandarinJuice's own published test vector shapes (header
    key, slice sizes) with invented data.

### § rift-pak (grows: the game's own pak, and its text)

- `rift_pak::game::GamePak::open(path) -> Result<GamePak, GamePakError>`: reads the header and
  the table (decrypting `0x08` tables, with the `0x68` and `0x28` chunk-table layouts), read-only.
  `read(&self, game_path: &str) -> Result<Option<Vec<u8>>, GamePakError>` finds the entry by
  `path_hash` and returns its bytes (chunked, deflate, zstd; encrypted resources decrypted).
  Never maps or writes the file.
- `rift_pak::msg::Msg::parse(bytes) -> Result<Msg, MsgError>`: entries by name, with each
  language's text. `english(name) -> Option<&str>`.
- Tests: an invented `0x68` pak built at run time (a key chosen at random, its table XOR-ed with
  `key^e mod N`, one chunked entry, one zstd entry, one encrypted resource built as
  `block × k^E mod M`) and an invented GMSG, all read back. The existing table-of-contents
  reader's behaviour for mod paks does not change.

### § rift-muster (new crate: the party and its names)

- `Codex::from_pak(&GamePak) -> Codex`: the name tables above, plus `serde` so the spine can
  cache it.
- `Muster::read(payload_tree) -> Muster`: members (role, level, experience, vocation id,
  vocation experience, gains, gear by slot with item ids, augment and skill ids for the current
  vocation, pawn inclination and specialization, a hired pawn's creator's level and vocation),
  the Arisen's gold and crystals. The names (the Arisen's and pawns' own, the creators') travel in
  the model as `Private<String>`, whose `Debug` and `Display` print `‹private›`. Only the
  window's view gets them.
- `name(&Codex)` turns ids into names, leaving `None` where the codex has no entry.
- `slots(dir) -> Vec<SlotFile>`: the `data*.bin` files that hold a party (a quick read of each),
  newest first.
- Tests: an invented party sealed into a save, read back into members. Also: an
  `ExpandItemStorageBackup` item is equipped; a pawn with no specialization; a codex without the
  game; ids beyond the tables; a save without a party (`data00-1.bin`-shaped).

### § src-tauri (the spine)

- **Discovery.** The Steam folder is the one the game was found through, else the registry's
  `SteamPath`. Each `userdata\<account>\2054970\remote\win64_save` that holds a slot is one
  save folder. The bench may set `RIFTSTONE_SAVE_DIR`, with the account id taken from the folder
  three levels up, or from `RIFTSTONE_SAVE_ACCOUNT`.
- **Commands** `get_muster_slots`, `get_muster` (WINDOW-CONTRACT § v0.4). A watcher on each save
  folder emits `rift://muster-changed`, debounced 1 s, when a slot file changes.
- **The codex** is read on first use from the found game's pak, cached as above, and rebuilt
  when the pak's size or time changes.
- **Logs** name slots by file name and say counts. Never a name.

### § The window (the Illusionist, after the Artisan's §17 in #00015)

THE MUSTER: a fifth page in the nav, beside THE RIFT, YOUR WORLD, NEXUS and PREFLIGHT, with
hotkey `5`. Its states:
- no Steam saves found;
- saves found, none with a party;
- the slot reads;
- a slot that will not open, voiced by kind (another account's save, a save being written,
  damaged);
- the names missing (no game found), with the party still shown, with ids.

The mock gets `?mock=muster…` scenes with an invented party.

### As built (2026-10-18)

- **rift-save** (16 tests): `open` checks the footer's signature before any arithmetic, then
  decrypts with ONE modular power per file: every slice's `c1` is the same constant, so
  `c1^uid mod p` is computed once. The stream's alignment is the engine's bitmask form
  `(pos + n − 1) & !(n − 1)`, even for sizes that are not powers of two (a 12-byte vector).
  Rounding to a true multiple instead parses the first 336 bytes of a real save and then loses its
  place: the bench caught it. On the investor's slots (copies): 3.77 MB payloads, opened in
  about 7 ms and parsed in 20 ms; 40,821 classes of 227 types, the spike's exact counts.
- **rift-pak** (41 unit tests + its bench): the game pak's table is read and decrypted in 225 ms
  in Rust (2.3 s in the Python spike); the seventeen texts in 53 ms. An encrypted resource is
  refused (`EncryptedResource`): one entry in the whole pak is one, and none the Muster reads.
- **rift-muster** (14 tests + its bench): on the investor's slot, the four members read whole
  and every equipped thing named: 17/17, 21/21, 19/19 and 21/21. The codex built from the
  install holds 1,404 items, 10 vocations, 4 inclinations, 9 specializations, 47 augments and 101
  skills. The inclinations' four ids are no name hashes; they are carried as facts.
- **The spine** (11 integration tests, 2 DTO tests). Save discovery is `rift_deploy::steam_roots()`,
  now public, under `userdata`. The codex cache is `muster-codex.json` in the cache folder. A
  slot whose signature or length disagrees is read once more after 500 ms: a file that changed
  meanwhile is `beingWritten`, one that did not is `damaged`. A slot being written is never
  cached. The save folders are watched with a labelled watcher (`watch::arm_labelled`), whose log
  lines name "save folder N", never the path. The launcher's own error names the folder it
  could not show, so the spine replaces it with a sentence that names the slot's file.
- **The window** (the Illusionist, after the Artisan's §17): THE MUSTER as specified. R17.2 is
  adopted: the room and `5` stay live while no game is found. Measured headless on the mock: at
  1024 × 680 the Play slab ends at 622 px, inside the window (Q17.1); every tab fits at both
  widths (Q17.3); two long gear names ellipsize at 1024 with the full name in `title` (Q17.2).
  355 specs in 34 files.
- **The privacy sweep** learned `--saves` (`scripts/dd2_saves.py`, AES in pure Python, checked
  against FIPS-197 in the self-test). A save's names count even when they are short words.
  Steam's `userdata` is a stock folder, so a save folder's private chain ends at the account
  number. The first real run read 312 hits that were all noise, and three rules came out of it.
  First, the game's own id records (two record types whose `Name` is an id with a digit in it,
  2,767 strings) are not names. Second, a short name matches in its own case only. Third, a name
  the repo already held at `BEFORE_SAVES` (830fdf0, the last commit before any save was read) is
  set aside and counted, since its hits cannot be told from the code's own words. One test
  account number in `rift-save` read as a mod id across a digit separator, so it was changed.
  Then the sweep read 0 hits over the tree, history and messages, in 2 m 46 s.
- **The first native Windows run of v0.4** (2026-10-18, at `f653d8b`, MSVC, Rust 1.98.0). The
  workspace's tests ran with `RUSTFLAGS=-D warnings`: 913 passed, 0 failed, 11 benches ignored.
  v0.4 added 49 tests on each platform (Linux 878 → 927, Windows 864 → 913), so every new test
  ran on both. On Windows' own file notifications that includes
  `the_window_hears_when_the_game_saves_and_reads_the_new_slot` and
  `reading_the_muster_writes_nothing_to_the_save_folder`. `npm run tauri build` built
  `riftstone.exe` and `The Riftstone_0.4.0_x64-setup.exe` (6.06 MiB). A read-only probe ran the
  built exe on a fresh bench data home, with `RIFTSTONE_SAVE_DIR` on a COPY of the investor's
  save folder, removed afterwards. It found 5 slots, all readable, listed in 272 ms. The newest
  slot held 4 members, and 80 of 80 equipped things were named from the game's own pak. Key 5
  drew THE MUSTER with 4 member tabs, with 0 page exceptions and 0 console errors. It printed
  counts only.
- **The General's review of PR #1** (2026-10-18, at `2928e89`): four issues and five nits, each
  confirmed at the head before it was fixed.
  - **RS-8 was false about the repo.** `scripts/dd2_saves.py` carried a working sealer that the
    sweep imports on every run. The sealer and the stream writer left the Python. The
    self-test opens `scripts/fixtures/sweep-invented-save.bin` instead. `rift-save`'s test-only
    sealer made it, and its `sweep_fixture` test checks it opens to exactly that invented party.
    The check is the content, not the bytes, because deflate's output depends on the backend a
    build links. The Sentinel now measures that the shipped graph switches on no `fixtures`
    feature.
  - **Any read error was `gone`, and `gone` was cached.** A slot another program held (or one this
    user may not read) stayed gone until the game wrote it again. Now only a missing file is
    `gone`. Any other failure gets the 500 ms second look and is then `beingWritten`, never
    cached. A file larger than `rift_save::MAX_FILE` is `damaged` from its size alone. A file of
    0–3 bytes is `Truncated`, not foreign, so it gets the second look too: the game empties a
    slot before it writes it.
  - **THE RE-CARVING lost its ground after one failed re-read.** `sameSlot` came from the view the
    failure had blanked. It now comes from the slot whose party was drawn last, which a failed
    re-read keeps and a pick of another slot clears (R17.25 holds).
  - **A failed codex build was not remembered**, so every call opened the 69 GB pak again, and two
    cold callers built it twice. A failure is now held against the pak's stamp just as a success
    is, and one build runs at a time behind its own gate.
  - **The sweep's reader crashed on a damaged slot** (`zlib.error`, `struct.error`). With no
    signature check and no length cap, a torn slot's footer could also send it looping. It now
    checks in `rift-save`'s order and turns every failure into a `SaveError`. A slot that does
    not open is read once more a second later, then counted.
  - **Why the sweep reads the live save folder** rather than a copy is now in the journal (Privacy).
  - The held, emptied, failure-memo, single-build and re-carving cases each failed at `2928e89`
    before their fixes, and the sweep's new damaged-body check crashed the old reader. The
    oversized-file test and the ceiling test guard the new cap. New: 5 spine tests, 2 rift-save
    tests, 1 window spec and 5 sweep self-test checks.
  - **The General re-verified at `3791af0` and approved.** It agreed with keying THE RE-CARVING on
    the slot drawn last rather than the slot picked. It raised one new nit: the RS-8 step passed
    when `cargo tree` itself failed, because the step runs as `bash -e` without `pipefail` and an
    `if` ignores `-e`. The graph is now held in a variable, which fails the step when
    `cargo tree` fails, and a graph without `riftstone` fails it too. All four cases were checked
    locally: the real graph passes, and a failing `cargo tree`, a fixtures feature and an empty
    graph each fail.
- **The second native Windows run of v0.4** (0.4.1, at `e24a73a`, MSVC, Rust 1.98.0). The first
  attempt ran out of Windows commit memory while it compiled every test binary at once (`os error
  1455`, "the paging file is too small"), so no test ran. With `-j 4`, the workspace's tests under
  `RUSTFLAGS=-D warnings` read 920 passed, 0 failed, 11 benches ignored: 0.4.0's 913 plus the review
  round's 7. `npm run tauri build` built `The Riftstone_0.4.1_x64-setup.exe` (6.06 MiB). The same
  read-only probe, on a fresh copy of the investor's save folder deleted after the run, found 5
  slots, all readable, and named 80 of 80 equipped things. Key 5 drew THE MUSTER with 4 member tabs,
  with 0 page exceptions and 0 console errors.

### Ownership in the build

| Builder | Owns |
|---|---|
| main session | `crates/rift-save`, `crates/rift-muster`, `crates/rift-pak` (game, msg), `src-tauri/**`, `src/types/rift.ts` and `src/services/rift.ts` (the contract), `scripts/` (the sweep's `--saves`), the docs |
| the Artisan | #00015 §17 (the Muster's design), in the lab repo |
| the Illusionist | `src/**` beyond the contract files, `tests/**`, the mock's Muster scenes |

## § v0.5 — versions of one file, and THE MUSTER by character (investor ask, 2026-10-18)

The ask, in the investor's words, after running 0.4.1: *"i downloaded an update from one of the
packages and that got installed, now it show as 2 entries in the library and in nexus. can we do
something about that? And the muster works! But can we do the muster based on character instead of
save slot? and that when you select a character that's the one that will show up, and then a select
menu somewhere for when you want to switch saveslot?"* Asked how, they ruled: **put the old version
away** (installing the newer version replaces the older install in one step, and the older archive
moves to an older-versions folder beside the mods folder, from which it can come back), and
**remember the character** (THE MUSTER opens on the character picked last, at their newest save,
and follows them when the game saves).

The wire is WINDOW-CONTRACT § v0.5, the design #00015 §18.

### The ground truth (measured 2026-10-18)

- **Each version of a file is its own package.** `PackageId::for_archive` hashes the file name and
  size, so a new version is a new package. A 2026 Nexus name parses to the same display name for
  every version. v0.3's contract left the old archive where it was ("removing it is the player's
  act in Explorer").
- **Today an install of the newer version stacks over the older.** The older install stays
  underneath (its bytes in `backups/<seq>/`, owner the newer install), and any of its files the
  newer version lacks stay in the game. That is the investor's state now.
- **The group id is there when it matters.** An update check stores the pinned link
  (`record_check`), file and group id included, and a download's link carries the group from the
  start. A package that showed "update available" was pinned.
- **The saves hold no playthrough id.**
  - In all five readable slots of the investor's saves, the Arisen's and the main pawn's record
    ids (`UniqueIDContext`) were the same, across two characters.
  - The Arisen's name grouped the slots into two characters, two slots and three, and the levels
    agreed: close within each character, far apart between them. The main pawn's name agreed within
    each group and differed between them.
  - Of the 1,549 leaves of the Arisen's record and the slot's detail, the 29 that agreed within each
    character and differed between them were all progress (`JobContext`, `HumanStatusContext`);
    none was an id.
  - The measurement printed slot numbers, levels and booleans only, from a copy of the folder
    deleted after it. This record keeps neither: the investor's game is the investor's.

### Rulings taken for the build (main session, 2026-10-18)

1. **RS-2, amended.** The stone still never changes, renames or deletes a file in the mods
   folder, with one exception. It may move an older version of a file, whole, to
   `<mods folder> (older versions)` beside it, and move it back at the player's word. The move
   happens only after the install that superseded it committed, or at the player's own press.
   It is never a delete or an edit, and it never lands on an existing file.
2. **A family is proven by Nexus, never guessed.** The same mod and the same file group, or a
   check that named the other's exact file as newer. Names and sizes never make a family.
   *Amended by ruling 10:* the player's word is proof too; names may make a question, never a
   family.
3. **One entry per file.** The window folds; the spine keeps listing every package, with
   `family` on each.
4. **Installing a version replaces the other versions' installs of the same option** in one
   journaled step. The same option is the same option root, else the same name, else the one
   installable option each version has (WINDOW-CONTRACT § v0.5, The same option), so a version
   whose top folder carries its number still replaces its older self. Rolling back to an older
   version is the same replace; only versions older than the installed one are ever put away.
5. **The put-away follows the commit.** If a move fails, the install still stands and the window
   says so.
6. **Keep only the newest** cleans up a family whose older versions are still installed, the
   investor's own state included.
7. **A character is the Arisen with their main pawn, by name, within one account.** The key is
   private like the names.
8. **The chosen character is remembered in `settings.json`.** v0.4 kept the pick for the session
   only; the investor's "that's the one that will show up" overrides that.
9. **Version 0.5.0.**
10. **The player's word (investor, 2026-10-19).** The native probe of 0.5.0 on the investor's own
    library found 0 families. The older version of the very file that started v0.5 is an
    unpacked folder linked by mod id through Find: no MD5 can pin a folder, and a mod-id link
    names no file group, so ruling 2 could never tie it to the pinned download beside it. Asked,
    the investor ruled that the player's word pins it. A package whose link names a mod and no
    file group may be joined to a file group another package of the same mod is pinned to
    (`source: "joined"`), and the family rule takes it from there unchanged. The stone asks only
    when the file names say it is likely, and never joins on its own. A no is remembered per file
    group, and a join can be taken back (WINDOW-CONTRACT § v0.5, The player's word).

### § rift-nexus (grows: families)

`family.rs`, pure: `families(members: &[FamilyMember]) -> Vec<Family>`.
- A member is a package id with its effective link (`link_for`) and its last check's newer file id.
- A family is the connected set under the two ties of ruling 2, ordered newest first: every
  proof kept (a named member before its namer, a circle of proofs condensed into one block),
  and among the blocks no proof holds back, the highest priority first (the newest recency among
  a member and every member whose proofs reach it), then own recency, then file name (the
  General's review, rounds 1–3, below). `tests/family_order.rs` holds it to that over 30,000
  invented families. A set of one is no family.
- Tested on invented links, including the transitive tie and a mixed pinned and unpinned pair
  that must not fold.

### § rift-deploy (grows: replace)

`plan_install_replacing` and `install_replacing(ctx, journal, package, option, replacing, source,
progress)`: the reinstall branch of `install` generalised from "the same option" to "these other
options"; `plan_install` and `install` call them with none.
- Each replaced install's removal steps (`prepare_uninstall` → `removal_steps`) go into the one
  intent before the placements, bottom layer first (by journal position), each planned against
  the journal the removals before it leave. Each replaced install moves through its own
  temporaries (`{seq}-{n}`), so a path two stacked versions held moves twice without a clash.
- A failure rolls everything back (`fail_and_roll_back`, which already handles a path touched
  twice).
- The new install takes the first replaced install's pak number and its place in the pak load
  order. The plan reads a path whose top layer is a replaced install as `reinstall`.
- A replaced install with paks is refused while the game runs, as its own uninstall would be.
- Tested against the stack rules (`tests/replace_rules.rs`, 7 tests):
  - a replaced lower layer hands its backups to the layer above;
  - a replaced top layer restores;
  - a failure and a crash mid-step roll both back;
  - two stacked versions replaced at once, bottom first and top first.

### § src-tauri (the spine)

- **Folding the views** (`family.rs`): `get_library`, `get_updates` and `search` compute families
  from the shelf and `nexus.json` on every call. The DTOs gain `family`, `older` and each older
  version's `matches` (`lead_match`: the lead's option whose install would take its place).
  `search` drops an older version's answer when the lead's same option answers too.
- **A download a check led to:** `NexusStore::row` and `badge` read a package with no verdict of
  its own as `current` when any record in `nexus.json` (an older version put away included) last
  named its exact file as newer (`vouched`), so the newer version stays current after the put-away
  instead of falling back to *not checked*.
- **Conflicts:** the snapshot drops a conflict whose layers are all versions of one file (the
  investor's stacked state is an older version installed, not a conflict). It reads the families,
  so it takes the Nexus store's lock: nothing asks for a snapshot while holding it. `relabel`
  invalidates the snapshot, so a link or a check that ties a family reads conflicts afresh.
- **The replace:** `plan_install` and `install` find the replaced installs by the same option
  (`family::replaced`, `matching_option`) among the family. After the commit, the put-away runs
  under the deploy lock, before the scan the install requests.
- **The move:** the downloads' never-overwrite helpers (`occupant`, `rename_new`, `move_new`,
  `copy_new`) move to a shared `landing` module, used by both downloads and the older-versions
  folder, with `move_aside` (the next free ` (n)` name, compared case-blind). A folder package
  moves as a whole folder: a rename, never a copy. The pool of open archives is closed first.
  When a move takes another name, the version's `nexus.json` record follows it (`move_records`).
- **New commands** (`commands/versions.rs`): `plan_keep_newest`, `keep_newest`, `put_away`,
  `get_older_versions`, `bring_back` and `open_older_versions_folder` (#00015 Q18.6).
- **The Muster:** `slots_view` fills `character` and `mainPawnName`, and `Settings` gains
  `musterCharacter`, kept only as a 16-hex character key (`persist::character_key`).
- **Tests:** a spine test per ruling, on invented packages and invented links, and none on the
  investor's library (`tests/spine_versions.rs`, 11 tests; `spine_muster.rs`'s character test;
  `spine_ipc.rs`'s versions commands).

### § The window (the Illusionist, after the Artisan's §18 in #00015)

- **The Rift's tree and THE FAR WORLD** fold each family into one entry. THE CROSSING shows what
  the install replaces. The put-away is told once, with `Bring it back`.
- **The older-versions shelf** gets a door: where it lives is §18's call.
- **THE MUSTER** lists characters, keeps a slot menu, and follows or pins.
- **The mock** gains a family scene (two versions of one file, the older one installed) and a
  two-character save folder.

### Ownership in the build (v0.5)

| Builder | Owns |
|---|---|
| main session | `crates/**`, `src-tauri/**`, `src/types/rift.ts` and `src/services/rift.ts` (the contract), the docs |
| the Artisan | #00015 §18 (versions in the Rift and THE FAR WORLD, the older-versions shelf, THE MUSTER by character), in the lab repo |
| the Illusionist | `src/**` beyond the contract files, `tests/**`, the mock's v0.5 scenes |

The window was built by two Illusionists side by side, each on files the other never touched. To
make that hold, the mock's Muster backend moved whole into `src/services/mock/muster.ts`
(`createMusterMock`) before either started. A machine restart killed both mid-build and wiped
their reports; the working tree survived. A third Illusionist wrote the versions side's specs
against the code they had left, and fixed what those specs found.

### As built (2026-10-18)

- **rift-nexus** (`family.rs`, 5 tests): the family rule as ruling 2 states it, a union-find
  within each mod, newest first by upload time, file id, then file name.
- **rift-deploy** (`install_replacing`, `tests/replace_rules.rs`, 7 tests; 178 in the crate): the
  replaced installs' removals go first in the one intent, bottom layer first, each through its
  own temporaries (`{seq}-{n}`). The new install takes the first replaced install's pak number. A
  top-first order, which the first draft used, put `glow 1.0` back instead of the game's own file
  on the stacked case; the test that caught it stays.
- **The spine** (`spine_versions.rs` 12 tests, the character test in `spine_muster.rs`, the
  versions commands through `spine_ipc.rs`, the shape tests in `dto.rs`). The put-away runs inside
  the install's own deploy closure, after the engine committed, so it holds the deploy lock as the
  ruling asks; the scan follows by `request_scan`. Three things the window's design asked of the
  contract came in here: `OlderVersionView.matches` (the same option, worked out once), the
  conflicts among one family's versions dropped from the snapshot (Q18.3), and
  `open_older_versions_folder` (Q18.6). Two came from reading the result: an io detail for these
  moves carries the OS's words only, since the window names the file itself, and `vouched` keeps a
  download current after the version whose check led to it is put away.
- **The window** (#00015 §18 as merged with §18.17): 53 new specs over four files for the
  versions (`versions.spec.ts` 21, `versionsCrossing.spec.ts` 13, `undercroft.spec.ts` 13,
  `versionsFar.spec.ts` 6), and `musterCharacters.spec.ts` (24, the spacing test among them) beside
  the grown `muster.spec.ts`. The versions specs found three gaps: Esc from the tree left THE VERSIONS
  card open, a rollback hid the put-away sentence §18.17 keeps, and the far world's line went stale
  after an uninstall, because it read `get_updates` instead of the library. A render pass found
  three more that no spec could see: Vue condenses a leading blank, and a blank between tags that
  holds a newline, so `Lv 41Warrior` and `data009Slot· following` read joined, and a flex row drops
  a leading space, so the slot button read `Slot 9· today`. Each is fixed, and a spec now pins the
  words.
- **The widths** (§18.14, measured headless on the mock, 1320 × 860 and 1024 × 680):
  - Q18.1: the lead row is 544 px at 1320, as predicted, and its label keeps 195 px against 88
    for 12ch, so the older mark never folds there. At 1024 it is 728 and 379.
  - Q18.2: THE VERSIONS card is 391 px wide (392 predicted), and all three version lines hold on
    one line.
  - Q18.8: the slot menu's bottom is at 209 px, inside the 680 px window.
  - Q18.9: the head's line 2 stays one 28 px line at 1024 in the longest state, followed into
    another slot.
  - Q18.10: neither character row's names clip.
- **The README's frames**: all seven older frames were shot again. They dated from v0.3 and showed
  four rooms, not five, a staleness v0.4's review missed. Two new frames show the family entry with
  THE VERSIONS card and the put-away landing. Each one was looked at before the push.
- **Gates** (Linux bench, `feat/v0.5`): cargo fmt clean, clippy with warnings denied clean, 967
  tests passed, 0 failed, 11 env-gated benches ignored; npm typecheck 0, lint 0, 433/433 tests in
  39 files, format:check clean, build clean. The privacy sweep, with the real library, journal,
  `nexus.json` and save folder as sources, read 0 hits over 332 files, 772 file versions in the
  history and 51 commit messages.
- **The sweep at the tip** (2026-10-19): with the four commits pushed, the sweep's sources had
  grown by one Nexus id, a mod the investor linked after v0.4.1. Its number was already the
  repo's own, in files v0.5 never touched and in history public since v0.3: 11 hits, none from
  v0.5. A collision, not a leak, and not a fixture that could be renumbered. Ruled by the
  investor: the sweep sets aside, and counts, every Nexus id the public tree carried at
  `SWEPT_CLEAN` (5a01aa5, v0.4.1, where it last passed clean), as it sets aside the save names
  `BEFORE_SAVES` held; a Steam account's numbers never are. Four self-test checks pin the rule,
  each seen to fail against its mutant. The rerun read 0 hits, 1 id set aside.
- **The player's word** (ruling 10, 2026-10-19). The spine (`9efd0e3`): `LinkSource::Joined`; a
  `nexus.json` record keeps the link it had before a join (`joinedOver`) and the groups the player
  declined (`declinedGroups`), both skipped when empty; `LibraryPackage.versionQuestion`; the
  three commands. `spine_word.rs` (5 tests) rebuilds the investor's shape with invented names, and
  five mutants (a no ignored, a take-back losing the old link, a pinned package asked, the mod
  check dropped, `modinfo.ini` counted) each turn a test red. The window (#00015 §18.18, Bold
  Choice 23): the tree's mark and question row, THE VERSIONS card's question form, `by your word`
  and *Take it back*, the option detail's line and the one manual door, THE FAR WORLD's sub-row,
  `INSTALLED older`, and YOUR WORLD reloaded on `rift://updates-changed`. `playersWord.spec.ts`
  (19), five mock scenes (`question`, `questioninstalled`, `questionjoined`, `questionmanual`,
  `questionrefused`). A headless render pass at 1320 and 1024 looked at the question, the yes, the
  manual pick and THE FAR WORLD row: 0 page errors. Gates: Rust 973 passed, 0 failed, 11 ignored;
  window 452/452 in 40 files; typecheck, lint, format and build clean.
- **The native Windows runs** (MSVC, Rust 1.98.0, `RUSTFLAGS=-D warnings`, `-j 4`). At c372344,
  after `cargo clean -p riftstone` cleared the stale artifacts behind the first attempt's E0460:
  953 passed, 0 failed, 11 ignored, and `npm run tauri build` built the 0.5.0 installer
  (6.15 MiB). Its read-only probe, on copies of the app data and the save folder over the real
  mods folder, read THE MUSTER by character with 0 page errors and wrote nothing, but found **no
  family**: the investor's older version was a folder linked by mod id (ruling 10). At 2f410ea,
  with the player's word: 959 passed, 0 failed, 11 ignored; the installer again (6.18 MiB); and
  the probe asked about exactly one package, that folder, with all of its file names in the
  pinned download of its own mod, again with 0 page errors, nothing written and the mods folder's
  entries unchanged. The probe never answers a question: the yes is the investor's.
- **Left for the native run:** the Windows-only code v0.5 adds (`landing::move_new`'s
  `MoveFileExW`, moved from `downloads.rs` unchanged) compiles nowhere but Windows, and THE
  UNDERCROFT's move has never crossed NTFS. Still open from §18.14: Q18.11 (whether a slot reads as
  being written mid-save) needs the game itself.
- **The General's review of PR #2** (2026-10-20, at `3df7eb4`): five issues and six nits. Each
  issue now has a spec that fails at `3df7eb4` and passes with its fix, each seen failing.
  - **Install… and Uninstall… stood side by side.** `Keep only the newest…` sat between the
    Install `v-if` and the Uninstall `v-else`, so the `v-else` paired with it. The door now
    follows the pair, as R18.4 has it (an extra door, never Uninstall's place). The specs now
    read the doors in order.
  - **A named-newer pair lost to the upload times.** A link pasted as a file URL has a file id and
    no upload time, so the older version led its family. `rift-nexus::family` now orders by that
    proof first: a member goes next only when no member still waiting was named newer than it.
    It is a pass over the recency order and not a comparator, because `sort_by` must be given a
    total order and the proof is a relation between two members.
  - **A late answer drew on whatever card was open.** A keep-newest plan or result, a yes's
    landing and a no's close all landed on the card open when the answer came, not the card that
    asked. `useVersions` now numbers each card the pane shows, and every press that awaits the
    spine takes that number first. A late answer for a card the player left draws nothing there.
    A finished keep-newest still says so in a toast, and a failed one in ember.
  - **Try again lost a move that worked.** The window put the older versions away one by one, and
    a failure on the second dropped the first from the landing. A new spine command,
    `retry_put_away(packageId)`, makes the install's own put-away again, under one turn, and
    answers `{ putAway, putAwayFailed }` as the install does. The window no longer works out the
    set. The mock's `putawayhalf` scene moves v1.2 on the retry while v1.1 still stays.
  - Nits: the older mark counts the installed ones (`+2 older, 1 installed`; the README frame was
    shot again and looked at). The privacy sweep keeps a colliding id in the search and sets aside
    only its hits on lines the repo held at or before `SWEPT_CLEAN`, so a new fixture still
    counts. `put_away` reads the family under the turn. A check keeps a joined link's group
    (v1 lists files with none). The version question needs two shared names, or one name that
    both packages hold alone: measured read-only on the investor's library, the folder ruling 10
    was built for shares 5 of 5 and still asks. THE MUSTER follows the slot that following
    opens, not the newest slot by time, so picking it while the newest is written stays following.
  - New: 4 Rust tests (2 rift-nexus, 1 spine store, 1 spine_versions, and 2 extended), 8 window
    specs, 3 sweep self-test checks. Gates on the Linux bench: cargo fmt clean, clippy with
    warnings denied clean, 977 passed, 0 failed, 11 ignored; npm typecheck 0, lint 0 warnings,
    460/460 in 40 files, format:check clean, build clean.
- **The General's second round on PR #2** (2026-10-20, at `43652f1`): 10 of round 1's 11
  threads resolved, and the specs for issues 1, 3 and 4 went red when their fixes were reverted.
  Three issues and one nit were left; two issues are on the Heretic's `43652f1` (the
  `useWorld` landing that returns before its detail reloads, and the lightbox's keys once focus
  leaves it) and are the Heretic's, fixed in `75ebf56`: a return to the same option before its
  write finishes now refreshes that option's detail without reviving the earlier plan, and the
  viewer takes its keys whatever holds the focus, a click on the picture keeping it inside.
  Four new checks in `roundTwoRegressions.spec.ts` failed at `43652f1`; a fifth, Esc with THE
  VERSIONS card under the viewer, was already right and stays as a control. Mine:
  - **Round 1's family order was half fixed.** With X (uploaded 50), A (uploaded 100, which named
    B newer) and B (no upload time), recency gave A, X, B; A waited on B, so X went first and
    the oldest led. The order now lifts a named member's recency to its namer's, up a chain of
    proofs, before it sorts, so the pair sits together and the pass only settles ties. The
    General's case and a three-step chain each failed at `43652f1`. Asked about cycles (two
    checks at different times can name each other newer), two- and three-member cycles were
    specced over every ordering of the input. The lift ends: it runs at most once per member and
    only raises a member to a recency some member already has. The three-member cycle caught a
    flaw in the pass: with every cycle member waiting on another, an older outsider went first.
    The pass now runs one group of equal lifted recency at a time, so a cycle falls back to its
    sorted order inside its own group, the same however the members are given.
  - **Nit: the lone shared name still asked for two packages that each hold only the same
    `config.json`.** The lone-name clause now counts only a name that is no settings or text
    file, so a one-pak or one-script mod's other version still asks. Its assertion failed
    against the old clause. The spine, the mock and WINDOW-CONTRACT each name the list, and
    `settingsExtensions.spec.ts` reads all three and fails when they differ (seen failing on a
    spine list missing one extension).
- **The Heretic's usability pass** (`43652f1`, 2026-10-20, from its whole-app Assay at the lab
  root, `documents/assessments/2026-10-20-riftstone.md`). The investor ruled it stays on v0.5
  rather than splitting off. Window only: no command, event or type, and the library actions call
  `open_mods_folder` and `rescan`. Each room in the pillar carries a plain line under its name.
  The search well gains `Clear search`, and the Rift's answers gain `Open mods folder` and `Scan`,
  a line reading the active filters with `Clear filters` (the query kept), and `NO MODS MATCH
  THESE FILTERS` in place of an empty list. The screenshot viewer keeps the keyboard, and an
  option's detail, plan and landing draw only on the selection that asked
  (WINDOW-CONTRACT § v0.5, The Rift's library tools). Specs: `selectionLifetime.spec.ts` and
  `libraryUsability.spec.ts`. The Assay's finding RS-A2 stays open in Known Limitations: a
  collision rename moves the package before `nexus.json` saves its new id, so a failed save
  loses a joined folder's family after a restart.
- **The README frames, round 2** (2026-10-20): all nine shot again from the mock at 1320 × 860
  on the combined head (`c69dba5` with the docs), 0 page errors each, and each looked at. Every
  frame shows the pillar's new room lines; the Rift's frames show `Open mods folder` and `Scan`,
  and the two search frames the `×` in the well. One thing the frames show that no spec states:
  at 860 px the pillar's taller rooms push the Nexus tags below the fold of the filter column, so
  only the categories show without a scroll.
- **The General's third round on PR #2** (2026-10-20, at `7622333`): one issue and two nits.
  The issue (the Uninstall… door live while an answer is in flight) and one nit (two lines of
  the viewer no spec held) are the Heretic's. Mine was the family order's third defect in three
  rounds: A named D newer while B and D named each other, so every member of that group waited,
  and A went out first. The reviewer's property run (30,000 random families) found 0 violations
  without a cycle and 480 with one, all this shape. On the General's advice the pass gave way
  to a procedure with no special cases: the proofs' closure, its circles condensed into blocks,
  and a topological order taking the ready block with the highest priority. A named member's
  priority is never below its namer's, so the order is sorted by priority as well as keeping
  every proof. The hand-picked cases were not enough three times, so `tests/family_order.rs`
  now gates the rule: 30,000 families of two to five versions, drawn from a fixed seed, with
  random upload times and file ids, chains, cycles and proofs naming files outside the family.
  An oracle works out the proofs by search, and each family must keep every member once, keep
  every proof that no proof leads back from, never let priority rise down the order, and give
  the same order for four shuffles of its input. At `7622333` it failed at case 249. With the
  new order it passes, with 952 families carrying a cycle. A mutant that ignores priority fails
  at case 45. The docblock's claim that Nexus never gives a cycle is gone; the rule handles one.
  The Heretic's two, in `579eeab`: the option detail now owns its writes per option. An
  install, uninstall or Undo in flight keeps that option's door busy when the player returns to
  it, and refuses a second plan or confirmation until the write and its detail refresh finish;
  success and failure both release it (WINDOW-CONTRACT § v0.5, answers in flight). At `7622333`
  a revisited door was not busy, and one pending write drew three plans. `roundTwoRegressions.spec.ts`
  now holds 12 cases, four of them red at `7622333`, and each of the viewer's two lines the
  General named fails its own spec when mutated.
- **The General's fourth round on PR #2** (2026-10-20, at `0f9226e`): approved, with three nits,
  fixed before the merge at the investor's word. The family procedure held, and every docblock
  and contract sentence was true of the code; 5 of the reviewers' 8 mutants failed the property
  gate and 3 failed unit tests only. Those three (a block ordered oldest first, the name
  tie-break reversed, the name alone as tie-break) passed the four properties, so the gate gains
  three: each cycle sits together, a cycle's members go by rank (own recency, then file name),
  and between two blocks of one priority a higher-ranked block comes later only when it had to
  wait on a member outside it not yet placed. The draw now repeats a file id now and then and
  adds a package of another mod in a quarter of the families, which must stay out of the family.
  Each of the three mutants now fails the gate (cases 20, 2 and 7). The window contract no longer
  says THE VERSIONS card keeps the busy-door rule: it guards its landings by card number, and its
  own writes rely on the spine's deploy lock, under which keep-newest works out its set again.
  The Heretic's nit, the in-flight check no spec held, is the Heretic's own commit.

## § v0.7 — THE WORKSHOP: Tamsin, a Rift pawn summoned through the stone to build mods with (investor ask, 2026-10-28)

The ask, in the investor's words: *"for the riftstone i was wandering if we could get a claude
instance in there as well, that way i can develop mods inside there, and we could give it a real
persona, that it is a pawn itself"*. The rulings, the same day: *"only me"* (the public build
carries no pawn); *"generic rift pawn, cause i have multiple save"* (one pawn, the same in every
save, who reads no save); *"yes"* to a workshop folder beside the library and an RS-2 exception
that brings a finished build into it; *"Yes Tamsin"* (the pawn's name); and *"yes"* to Play
waiting for the Arisen's press.

THE WORKSHOP is a sixth room, compiled only into the investor's build. It holds a Claude Code
session in a terminal, launched by the stone inside a fence the stone builds and proves, and
working in `<mods folder> (workshop)`. The stone serves the pawn a few tools of its own (search
the library, plan and install, uninstall, ask to play, read the REFramework log). The investor
adds a finished build to the library with one press, by its digest. The design is #00015 §20
(lab `documents/design-systems/riftstone-pawn-workshop-2026-10-28/design/`). The behaviour on
the wire is WINDOW-CONTRACT § v0.7. This section is the crate-level contract and the ground truth
it rests on.

### The ground truth (measured 2026-10-28 on the investor's homestation, Claude Code 2.1.312)

Every fact below was read off the disk after the step, never from the pawn's own report, on
dummy folders under Windows' temp directory (`workshop`, `outside`, and `saves` holding a
canary). The record and the raw outputs are the lab's
`documents/design-systems/riftstone-pawn-workshop-2026-10-28/spike/`.

- **Native Windows cannot fence a pawn.** Claude Code runs commands unsandboxed on native Windows
  (documented). With deny rules alone, `echo >` and `cat` were refused, but
  `python3 -c "open('../outside/py.txt','w')"` **wrote the file**. Policy stops the shell file
  commands it recognises, and any script walks past it. **The pawn runs in WSL, never as
  `claude.exe`.**
- **Claude Code's own sandbox fences the shell, not the file tools.** In WSL with `bubblewrap`
  and `socat`, shell writes and reads on `/mnt/c` were refused by the OS, the Python write
  included. The Write and Read tools were held only by `--restricted`, which is policy.
  Without `socat` the sandbox does not start, and with `sandbox.failIfUnavailable` Claude Code
  refuses to launch rather than run unfenced (measured).
- **A plain `bwrap` does not contain a Windows process.** With `/` mounted read-only, and
  again with `--unshare-net`, `cmd.exe /c "echo x > …\outside\netns.txt"` **wrote the file**:
  WSL interop runs the `.exe` as a Windows process outside every Linux namespace.
- **A seccomp filter closes interop and leaves the network open.** The first version (11
  instructions) refuses `socket(AF_VSOCK, …)` (and any non-x86_64 or x32 syscall) with `EPERM`.
  Under it, `cmd.exe` fails with `UtilBindVsockAnyPort: socket failed 1`, and a TCP connection to
  `api.anthropic.com:443` still opens. **The contract's filter is 15 instructions:** the same,
  plus `ioctl(fd, TIOCSTI, …)` refused with `EPERM` (measured: refused under the filter, while
  `TIOCGWINSZ` works, TCP opens and `cmd.exe` is still refused). The homestation's kernel already
  refuses `TIOCSTI` on its own (`dev.tty.legacy_tiocsti = 0`, `EIO`). The filter does not rely
  on that.
- **The whole-process fence holds every channel.** `claude` itself was launched inside a `bwrap`
  that mounts an allowlist (`/usr` and its links, `/etc`, WSL's `resolv.conf`, Claude Code's
  own binary, a tmpfs home, the workshop read-write), with `--remount-ro /` and the filter above.
  Claude Code's own sandbox ran nested inside it. **The permission rules were deliberately left
  open for `/mnt/c`**, so every refusal is the OS's:
  - the Write tool outside the workshop: `EROFS`;
  - the Read tool on the dummy save: the file does not exist in the namespace;
  - a Python write outside: the folder does not exist;
  - a real `cmd.exe` copied into the workshop and run as `./planted.exe`: `cannot execute:
    required file not found` (interop's interpreter, `/init`, is not mounted). The filter
    above is a second, independent close;
  - writing in the workshop (the control) succeeded every time.
  - Without `--remount-ro /`, a write outside "succeeded" into `bwrap`'s throwaway root and was
    gone after the session: contained, but a pawn that believes a file was saved is told an
    untruth. The root is therefore read-only.
- **Networking is NAT** on the homestation (`.wslconfig` sets no mirrored mode). A Windows
  listener is reachable from WSL only through the host's address and the Windows firewall. That
  is why the tools travel through a mailbox folder (below), not a socket.

### RS-2 — second exception: bring it through, by digest

RS-2 gains its second exception. **At the player's press, the stone adds one built artifact
from the workshop to the mods folder.** An artifact is a `.zip` in the workshop's `dist\` with
a receipt beside it, `<name>.receipt.json`. The press refers to the SHA-256 the window showed
when the card opened (`plan_bring_through`). On the press the stone copies the file to
`<mods>\.riftstone-<id>.part`, hashes the copy, and renames it to `<name>.zip` only when the hash
equals that digest, and never over an existing file: the download landing's own rules
(`landing.rs`). Identical bytes already under that name answer `alreadyHad` and add nothing. A
different file under that name refuses with `nameTaken`. Changed bytes refuse with
`bytesChanged` and both digests. The workshop copy is never moved or changed. Nothing else
of the workshop ever reaches the mods folder, and the pawn has no tool that does this.

### RS-9 — The pawn's fence (new)

**The stone launches the pawn only through the fence, and only the fence's own probe may say
what it holds.** Rules, each a test or a Sentinel step:

1. **The host is WSL.** The stone runs `wsl.exe -d <distro> -e /bin/sh -c 'exec bwrap … 9<"$1"'`
   with the filter file as `$1`. It never runs `claude.exe`. The distro is the default one,
   unless a setting names another.
2. **Reads by allowlist.** The namespace mounts exactly these, and nothing else of `/mnt`:
   - read-only: `/usr` (`/bin`, `/lib`, `/lib64` and `/sbin` as its links), `/etc`,
     `/mnt/wsl/resolv.conf`, Claude Code's binary tree (`~/.local/share/claude`, `~/.local/bin`),
     the relay (`/stone/relay`), and each read-only input the investor names in Settings (for
     example a pinned vanilla extract). Each input is mounted at `/inputs/<n>`;
   - read-write: the workshop at its own path, the run's request folder
     (`/stone/mailbox/requests`), and the pawn's own Claude config dir (`/stone/claude`);
   - read-only: the run's answer folder (`/stone/mailbox/answers`). The mailbox's own parent is
     not mounted, so the pawn can neither replace either folder nor plant a link in the answers;
   - fresh: `/dev`, `/proc`, a tmpfs `/tmp` and a tmpfs home. Then `--remount-ro /`.
   The saves, Steam's `userdata`, the game folder, the mods folder, the investor's own `~/.claude`
   and the rest of `/mnt/c` are not mounted.
   **Every Windows source is validated before it is mounted, at each summon** (the Heretic's
   Assay, WC2). The stone opens the source by handle and takes its **final resolved path**
   (`GetFinalPathNameByHandleW`, junctions and symlinks followed). It refuses the source when that
   path **overlaps a protected root in either direction**: equal, inside it, or containing it.
   The protected roots are Steam's install folder (and so `userdata` and the saves), the game
   folder, the mods folder and its `(older versions)` and `(workshop)` siblings (the workshop is
   mounted on its own, never as an input), the stone's app data (Roaming and Local), the
   investor's `%USERPROFILE%\.claude`, `%USERPROFILE%` itself, and every save-backup folder the
   investor's build has registered. **The backup inventory** (`save-backup-destinations.json`, the
   Heretic's WD3) is a record, not a cache: a folder is registered durably BEFORE a backup is
   written into it, and a backup whose folder cannot be registered is not made. No file is an
   empty list. A file that cannot be read or parsed refuses every summon and input change, and is
   left exactly as it is (never set aside, never overwritten), so registered coverage is never
   lost silently. One writer at a time. **Coverage begins with the investor's v0.7 build:** a
   backup made by v0.6.2, or by any build without the pawn, registered nothing. Such a folder is
   protected only when it lies under another protected root. A backup made into it once from
   this build registers it. Steam's install is every root the stone's own discovery knows (the
   registry, the environment) and the default one. A link *inside* a mounted input resolves in the namespace's own view:
   an NTFS junction appears to WSL as a link to `/mnt/c/…`, which is not mounted, so it dangles.
   The probe measures that (step 9). A source whose target changed since the last summon is
   validated again; validation is never cached. **The check governs every mounted Windows
   object, not only inputs**: the argv builder validates each mount source it emits. Only the
   declared trusted mounts (the workshop, the live run's two mailbox folders and the pawn's
   config dir) are excepted from the input bans, and each of those is checked to be exactly the
   stone-made folder it names (the Heretic's follow-up, 2026-10-28).
3. **Interop is closed twice.** `/init` is not mounted, and the stone's seccomp filter (the 15
   instructions above, built by `pawn::fence::fence_filter()` as bytes, never by a library at
   run time) refuses `AF_VSOCK`, `TIOCSTI`, non-x86_64 and x32 syscalls. `--die-with-parent`,
   `--unshare-pid` and `--unshare-ipc` are always set; the network is shared. `--new-session`
   is **not** set, because it would detach the TUI from its terminal. The injection it guards
   against (`TIOCSTI`) is refused by the filter instead, as bubblewrap's own documentation
   offers. The PTY is the stone's, made for this pawn alone, with no shell of the investor's
   behind it (the Assay's answer 4; measured at the build's bench: unmeasured list).
4. **Claude Code's own sandbox runs inside the fence.** The settings the stone writes set
   `sandbox.enabled`, `sandbox.failIfUnavailable`, `sandbox.allowUnsandboxedCommands: false` and
   `sandbox.autoAllowBashIfSandboxed`. A pawn whose inner sandbox cannot start does not start.
5. **Nothing of the investor's Claude reaches the pawn.** `CLAUDE_CONFIG_DIR=/stone/claude`
   (WSL-side `~/.local/share/riftstone/pawn/claude`, made on first summon; the investor signs in
   there once, inside the terminal). The pawn is launched with `--restricted` (only managed
   settings and `--settings` load; file tools stay in the working dirs; no bypass mode), with
   `--tools` naming exactly `Bash Read Write Edit Glob Grep`, and with
   `--strict-mcp-config --mcp-config <the relay only>`. No hooks or plugins load.
   **The environment is an allowlist** (the Assay, WC1: a config dir does not stop what a
   process inherits; an invented canary variable survived the fence until `--clearenv`). `bwrap`
   runs with `--clearenv` and then sets exactly `PATH=/usr/local/bin:/usr/bin:/bin`,
   `HOME=<the tmpfs home>`, `USER`, `TERM=xterm-256color`, `COLORTERM=truecolor`,
   `LANG=C.UTF-8`, `TMPDIR=/tmp`, `CLAUDE_CONFIG_DIR=/stone/claude` and
   `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`. Nothing else crosses: no `ANTHROPIC_*`, no
   `*_TOKEN`, `*_KEY` or proxy variables, no `SSH_AUTH_SOCK`, no `WSLENV`-carried Windows
   variable. The stone starts `wsl.exe` with `WSLENV` emptied. The fence's process holds no
   descriptor but the PTY's three and the filter's, which `bwrap` consumes. The persona is
   `--append-system-prompt-file` (#00015 §20.10, the voice block filled from the dial).
6. **Approval prompts stay on.** The permission mode is `default`. The stone never passes
   `bypassPermissions`, `dontAsk` or `--dangerously-skip-permissions` to a pawn.
7. **The ground is named.** `pawn::fence::ground()` hashes the launch argv (paths normalised),
   the settings file, the filter bytes and the relay's bytes, and records them with the distro,
   `claude --version` and the WSL kernel version. A probe record holds only for the ground it
   was proved on (#00015 R20.14).

### The probe: deterministic, no model

The design's §20.13 had the probe summon a throwaway pawn. **The contract replaces that with a
probe that needs no model.** Every channel the pawn has (its shell, its file tools, its scripts)
is a system call from a process in the same namespace. So the stone runs small processes in the
**identical** fence (the same `bwrap` argv and filter, with `/bin/sh -c '<step>'` in place of
`claude`) and reads the result from the Windows side of the disk. It is deterministic, costs no
tokens, and does not depend on a model following instructions. Steps, in order:

| # | Step (inside the fence) | Must | Read from (outside the fence) |
|---|---|---|---|
| C | write `probe/control.txt` in the workshop | land | the file exists, with the bytes written |
| 1 | write `<mods>\probe-<nonce>.txt` (its `/mnt/c` path) | not land | the file does not exist |
| 2 | write `<game>\probe-<nonce>.txt` beside the game's `NEVER_WRITE` files | not land | does not exist |
| 3 | list the saves' Steam folder, `<steam>\userdata` (its `/mnt/c` path) | fail | the step's exit status, and its output names no entry. The stone writes nothing there |
| 4 | read a canary the stone wrote in its own app data (`%LOCALAPPDATA%\nl.zmuuzn.riftstone\pawn\probe-canary.txt`, a fresh nonce each run) | fail | the step's output does not carry the nonce |
| 5 | write `<real home>/.claude/riftstone-probe-<nonce>` (the investor's own config dir, by its real path) | not land | the investor's `~/.claude` has no such file. Inside, `$HOME` is a tmpfs, so a write to `~` lands only there and vanishes |
| 6 | run a copy of `cmd.exe` the stone placed in `probe/`, writing `<mods>\probe-pe-<nonce>.txt` | not land | does not exist |
| 7 | run `cmd.exe` by its `/mnt/c/Windows` path, writing the same | not land | does not exist |
| 8 | `claude --version` | succeed, and equal the ground's | the output |
| 9 | read the app-data canary (step 4) through an NTFS junction the stone made in a probe-only input folder (`<app data>\pawn\probe-input\link` → the canary's folder), mounted at `/inputs/probe` | fail | the output does not carry the nonce |
| 10 | print the environment (`env`). The stone launches the probe with `RIFTSTONE_PROBE_SECRET=<nonce>` set on its own side, and in `WSLENV` | not carry it | the output does not carry the nonce |
| 11 | create a link `/stone/mailbox/answers/x.json` → the workshop, and write into `answers/` | fail | `answers\` is unchanged |
| 12 | `ioctl(TIOCSTI)` on the PTY | fail | the step's own report of the refusal, and nothing typed into the stone's PTY |

Two broker checks run with the probe, on the Windows side, without the fence:
- the stone plants a WSL symlink (an `LX_SYMLINK` reparse point) and an NTFS junction in a probe
  request folder, both pointing outside it. Its own request reader must refuse both, unread;
- a request named outside the UUID pattern, and one of 64 KiB + 1, must be refused unparsed.

**A refusal counts only when the stone saw it** (the Heretic's Assay of the probe, WP1). Each
step and each broker check gets one verdict:
- `held`: the step began inside the fence (its first line is the run's marker, printed by the
  script itself), finished within 30 s, aimed at something that exists outside (the mods and game
  folders, Steam's `userdata` with entries in it, the canary and its link, the planted `cmd.exe`,
  the investor's `~/.claude`, the run's `answers\`), and came back with the refusal itself. A
  write must exit non-zero and leave nothing on disk. A program that cannot run answers 126 or
  127. Step 12 calls the ioctl through libc and exits with its errno (41 for `EPERM`, 42 for
  `EIO`). Step 10 first checks that the secret reaches WSL **without** the fence, so its absence
  inside means something.
- `breached`: something crossed: a file landed, the nonce or the secret came back, `userdata`
  was listed, a Windows program ran (exit 0), or the ioctl succeeded. This is judged first,
  whatever else is missing.
- `inconclusive`: anything else. That covers a step that could not start, timed out, never
  printed the marker, aimed at nothing, or failed for a reason other than the refusal.

The record is `pawn-fence.json` in the stone's app data:
`{ version: 2, ground, provedAt, control, steps: [{ step, expected, observed, ok, verdict, layer }],
broker: [B1, B2] }`. `layer` is `os` for every refusal the fence makes. No step runs Claude
Code, so no permission rule takes part. A step that lands where it must not is removed by the
stone at once (the stone's own probe file, never anything else), and the record is `BREACHED`.
**A record holds only when it is whole** (WP2):
- this version;
- the control;
- exactly rows 1–12 in order, each with its expectation;
- B1 and B2;
- every one of them `held`.

An inconclusive row leaves the chip `UNPROBED`, naming the row. A record from an earlier version
reads back, and a recorded breach stays one, but it never holds.

**The chip is judged against the ground now** (WP3). `get_fence` and `get_workshop` survey the
host again (one `wsl.exe` call) and build the plan a summon would get, with the same function
`prepare` uses. A changed input, distro, `claude` or kernel therefore reads `UNPROBED` at once.
When the host cannot be checked, a holding record reads `UNPROBED` and says why.

**The chip has no `POLICY` in v0.7.** The stone never launches a pawn without the fence. If
`bwrap`, `socat` or the WSL distro is missing, summoning refuses (`noFence`, with the missing
piece named) rather than fall back to Claude Code's settings. So the chip reads `CONTAINED`,
`UNPROBED` or `BREACHED`. This narrows #00015 R20.12, whose `POLICY` row stays reserved for a
host that cannot fence. The chip
(WINDOW-CONTRACT § v0.7) is `CONTAINED` only when the control succeeded and every step failed as
it must, on the current ground. A step that succeeded when it must fail makes the record
`BREACHED`, and the stone refuses to summon until a probe holds. A missing record, or one on
another ground, is `UNPROBED`. A pawn may still be summoned then, **inside the same fence**: the
fence is present but unproven on this ground, and Claude Code's approval prompts stay on as they
always do. The chip claims exactly that and no more (the Assay's answer 2).

### Unmeasured until the build's first bench (each one is the [SPIKE]'s remainder)

The measurements above ran Claude Code non-interactively (`claude -p`) from a WSL shell, with the
investor's own `~/.claude` bound in. None of the following was measured. Each is the first thing
the build checks on the homestation, and the contract changes if one fails:
- the interactive TUI in a `portable-pty` started by the Windows stone through `wsl.exe`,
  inside this fence: resize, Ctrl+C, exit codes, and `--die-with-parent` when the window closes;
- `--unshare-pid` and `--unshare-ipc` under Claude Code's nested sandbox;
- a fresh `CLAUDE_CONFIG_DIR`: the first sign-in inside the terminal, and its credentials
  persisting in `/stone/claude`;
- `--restricted` with `--append-system-prompt-file`, with the relay's MCP tools, and whether it
  loads the workshop's `CLAUDE.md`;
- the relay's mailbox round trip across drvfs (latency, and Windows' change notification for a
  file WSL renamed into place).
- the fence's process tree dying when the window closes, and when `wsl.exe` is killed from the
  Windows side (`--die-with-parent` across the `wsl.exe` boundary);
- that the PTY is the pawn's alone: no shell of the investor's sits behind it to return to;
- the environment that actually reaches `bwrap` through `wsl.exe` before `--clearenv`;
- **the interval between final-path validation and the bind** (the Heretic's review of PR #9,
  answer b): the stone resolves every mounted Windows folder by its final path and then hands
  the path to `bwrap`, which binds it by path. Validation narrows that interval but does not
  prove it safe. A deliberate native swap (a junction planted in a validated folder's place
  after validation and before the bind) must be shown to fail, or to be caught, before the chip
  may claim more than the probe's own steps.

Measured since (Goosterhof/riftstone#9 and the probe PR, the lab's spike runs K–M): the TUI in a
PTY inside the fence; the Windows stone's ConPTY through `wsl.exe`, which waits on the cursor
query `ESC[6n` that xterm.js answers; the tree's death across `wsl.exe` on departure and on
process exit; the environment at the fence (only the nine names, no `WSLENV` secret); and the
probe itself, natively on the homestation, `contained`. After the Assay's WP1–WP4 it ran again natively: every row and both broker checks were `held`, with step 10's control crossing without the fence, and the chip read `contained`.

### The tools: a mailbox, not a socket

The relay is the stone's own stdio MCP server, a single file the stone writes to WSL-side
`~/.local/share/riftstone/pawn/relay/relay.py` and mounts read-only. It speaks MCP to Claude Code
and nothing else. For each tool call it writes `requests/<uuid>.json` (an exclusive create of
`<uuid>.tmp`, then a rename) and waits for `answers/<uuid>.json`, polling every 100 ms (inotify
does not see Windows-side writes to drvfs).

**Each summoning is a run** (the Assay, WC4). At summon the stone mints a `runId` (a UUID) and
makes `%LOCALAPPDATA%\nl.zmuuzn.riftstone\pawn\runs\<runId>\{requests,answers}`. Only that
run's two folders are mounted, at `/stone/mailbox/requests` (read-write) and
`/stone/mailbox/answers` (read-only). The stone reads requests only from the live run's folder.
At departure it **retires** the run: it stops reading, waits for any mutation already started to
commit or roll back through the journal, writes that mutation's answer into the retired run's
`answers\` (where no relay reads it), records it in the run's ledger, and keeps the folder for
the ledger's lifetime. A request left by run A is never read in run B: B has its own folders.
The plan token includes the `runId`, and a held Play belongs to its run and is dropped at
retirement.

**The broker's file handling is a boundary of its own** (the Assay, WC3: JSON validation is half
of it). The pawn can create, replace and link objects in `requests\`, so:
- the stone opens each run's `requests\` and `answers\` once, by handle, refusing either if it
  carries a reparse attribute, and keeps both handles for the run;
- every request is opened **relative to the pinned `requests\` handle**
  (`NtCreateFile` with `RootDirectory`), by a name that must match
  `^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\.json$`, with
  `FILE_FLAG_OPEN_REPARSE_POINT`. It is refused unread when it is not a regular file or carries
  any reparse attribute (WSL symlinks are `LX_SYMLINK` reparse points). Reads stop at
  64 KiB + 1 byte, and one byte too many refuses it. The stone deletes the request by the same
  handle;
- every answer is written relative to the pinned `answers\` handle: an exclusive create of
  `<uuid>.tmp`, a flush, then a rename to `<uuid>.json` by handle (`FileRenameInfo` with
  `RootDirectory`). The pawn cannot write `answers\` at all, because it is mounted read-only;
- no path the pawn wrote is ever followed, and no path from a request's contents names a file
  the stone opens. Tool arguments are ids, never paths.

**Every request is untrusted input.** The pawn can write `requests\` directly, so a forged request
must be no stronger than the tool call it imitates. The stone also checks:
- UTF-8 JSON with exactly `{ "id", "tool", "args" }`, where `id` equals the file's UUID, one known
  `tool`, exact argument types and no extra keys;
- at most 4 requests in flight;
- **idempotency.** The run's ledger keeps every `id` it acted on with its outcome. A request
  whose `id` is already in the ledger gets the recorded answer and is never acted on twice. The
  relay retries a timed-out call with the same `id`. **An `id` is reserved before its work
  starts**: a duplicate arriving while the first is still in flight joins it and receives the
  same answer when that finishes, rather than starting a second mutation (the Heretic's
  follow-up, 2026-10-28).
It answers in the same shape the relay returns to Claude Code: `{ id, outcome, text, data }`, where
`outcome` is `done`, `refused`, `held` or `unknown`. **`unknown`** (the Heretic's WB2) answers a
mutation the stone made but could not record: the ledger keeps its intent, written before the
mutation started, and the answer tells Tamsin to read `list_installed`. A ledger that cannot be
written stops every further mutation in that run (`refused`, the stone's own sentence); reading
tools still answer. An uninstall's ledger entry records whose install it removed,
`removed: { actor, seq }`, as the journal had it at that moment (WB4). `install` hands the plan its
token named to the install, which checks it again under the lock that commits (WB1). A malformed request is answered
`refused` with a sentence and is never acted on. Requests are never logged with their arguments.

**The held Play's decision** (the Heretic's review, 2026-10-28). The `play` call that asked has
already been answered `held`, and that answer is the ledger's for its id forever: a retry of the
same id gets `held` again, never the decision. The Arisen's press is recorded as its own
**decision**, `{ runId, heldRequestId, decision: played | declined, at, delivered: false }`, in the
run's ledger. It reaches Tamsin in two ways:
- **read, never consumed:** every `list_installed` answer carries
  `play: { waiting: heldRequestId | null, lastDecision: { heldRequestId, decision, at } | null }`;
- **delivered once:** the next `play` call with a **new** id, while a decision is undelivered,
  answers that decision (`done`, `played at …` or `declined: the Arisen said not now`), marks it
  delivered, and holds nothing new. Asking again is a further new `play` call.
A new `play` id while a request is still waiting holds nothing new either: it answers `held` with
the waiting request's id in `data.heldRequestId`. At most one Play is held per run, and a
departure drops it unanswered (`lastDecision` stays as it was).

Three rules close the edges (the Heretic's review of `d1dee4e`, P1 and P2):
- **Only a launch the stone started is `played`** (P1, narrowed by the Heretic's WD6). The
  Arisen's *Play* runs `plan_launch` and `launch_game` as the slab does. If either refuses,
  fails, or the Arisen cancels at one of its warnings, **no decision is recorded and the request
  stays waiting**. The window shows the stone's refusal under the held card, and the Arisen can
  press *Play* again or *Not now*. `played` is recorded once `launch_game` has started the
  launch: `DD2.exe` started, or Steam accepted the request. It never claims the game came up;
  the game watcher says that. The words Tamsin receives say the same: *the stone started the
  launch; read the REFramework log once the game is up*.
- **The press names what it answers** (P2). `answer_pawn_play` carries the `runId` and
  `heldRequestId` the window was shown. The stone acts only when they are the live run and its
  waiting request, checked under the decision's lock when the press arrives, and again after any
  warning the launch waited on, right before the game starts. Anything else is refused `pawn`
  `staleHeld`, and nothing is recorded or launched: a press from a retired run, or one for an
  earlier request of the same run.
- **Play calls are taken one at a time per run.** Delivering a decision is committed in the same
  ledger write as the new id's answer, so a retry or a crash can neither consume a decision
  without its answer being recorded nor make a second hold.

**A timeout has a defined status.** The relay waits up to 120 s. Then it tells the model
`unknown: the stone may still be doing this. Call list_installed, or call the same tool again,
which repeats nothing`. A mutation's true outcome is always in the ledger and the hands row.

| Tool | Arguments | What the stone does | Answer |
|---|---|---|---|
| `search_library` | `query: string` (≤ 200 chars), `limit ≤ 50` | the window's `search` | option ids, names, menu trails, install state |
| `get_option` | `optionId` | the window's `get_option` | the option's detail, files included |
| `list_installed` | — | `get_installed` | installs with their ids, origins and actors |
| `plan_install` | `optionId` | `plan_install`, plus a **plan token**: a hash of the plan, the library's generation and the journal's sequence | the plan and the token |
| `install` | `optionId, planToken` | refuses `stalePlan` unless the token still matches; otherwise `install` through the RS-4 journal with **`actor: pawn`**, inheriting RS-3's refusals word for word | the outcome |
| `uninstall` | `installId` | `plan_uninstall` + `uninstall`, `actor: pawn` | the outcome |
| `play` | — | **held**: the window shows the request; the tool answers at once `held: waiting for the Arisen`. `Play` in the window runs `plan_launch` + `launch_game` exactly as the slab does; `Not now` records `declined`. A held request is dropped when the pawn departs. The Arisen's choice reaches Tamsin later, never by rewriting this answer (*The held Play's decision*, below) | `held`, or the run's undelivered decision |
| `read_reframework_log` | `tailLines ≤ 2000` | reads `<game>\reframework\re2_framework_log.txt` (or the build's log name, as THE FRAY finds it) read-only | the lines, and their count |

There is no tool that writes a file, reads a save, touches THE MUSTER, the Nexus key or the
downloads, or brings a build through. Every answered call is one `rift://pawn-hands` event
(WINDOW-CONTRACT § v0.7).

### The journal's actor

`InstalledOption` gains `actor: Actor`, `#[serde(default)]` = `player`, where `Actor` is
`player` | `pawn { runId }`.
The schema stays 2: v0.6.2 ignores unknown fields when reading (`journal.rs` carries no
`deny_unknown_fields`), so a downgrade reads the journal and only drops the mark at its next
save. Tests: a v0.6.2 journal reads as `player`; a v0.7 journal with `actor: pawn` round-trips;
and v0.6.2's struct reads v0.7's bytes.

**The run ledger.** An install's `actor` says who placed it, but not what a run removed. So
`pawn-runs.json` in the stone's app data keeps one entry per run:
`{ runId, startedAt, endedAt, ops: [{ id, tool, installId, optionId, outcome, at }] }`. It is
written before each mutation's answer and kept for the last 20 runs.

**Put back is narrowed** (the Assay, WC5; #00015 §20.13's "exactly as the investor left it" was
too strong). `plan_put_back(runId)` offers two lists:
- **remove:** installs whose `actor` is that run's, still installed and unchanged since. Each is
  uninstalled newest first through the journal, after the investor reviews it in THE RETURN;
- **review:** what that run **uninstalled** that was not its own install. It is shown, never
  acted on in bulk, each with the journal's own Undo where that still applies, else `Install
  again from the Rift`.
The window says what the press does: *Removes the 3 installs Tamsin made in this summoning.
She also uninstalled 1 of yours; it is listed below.* It never promises the world as it was.

### RS-10 — The pawn never reaches the public build (new)

- **Rust.** `src-tauri` gains the feature `pawn`, off by default. Every pawn module, command and
  dependency (`portable-pty`) sits behind it. The investor's installer is built with
  `npm run tauri build -- --features pawn`.
- **Window.** The room is imported only under `import.meta.env.VITE_RIFT_PAWN === '1'`, which the
  investor's build script sets. Vite drops the dead branch, so the public `dist/` carries no
  workshop code.
- **The Sentinel measures both**, as it does RS-8:
  - `cargo tree -p riftstone -e normal,build -f '{p} {f}'` lists no `pawn` feature on the default
    build;
  - after `npm run build` without the flag, `grep -r "THE WORKSHOP" dist/` finds nothing (the
    room's inscription, a string only the room carries);
  - the same build with the flag finds it (a positive control, so the grep cannot pass by being
    wrong);
  - a test built under the default features lists the commands `generate_handler!` registers and
    finds none of WINDOW-CONTRACT § v0.7's. The same test under `pawn` finds all of them (the
    Assay's answer 5: an absent inscription is one piece of evidence, not the proof).

### Modules (all behind `pawn`)

- `src-tauri/src/pawn/fence.rs`: pure functions only, with no I/O and every one unit-tested. They
  build `bwrap_argv(&FencePlan)` (the plan names every mount), `fence_filter()` (the exact
  bytes, tested instruction by instruction), `pawn_settings(&Voice)`, `ground()`, and the
  `wsl.exe` command line.
- `src-tauri/src/pawn/summon.rs`: the PTY (`portable-pty`, as the Mezzanine's `pty/`), its
  reader pump to `rift://pawn-output`, input, resize, depart, exit codes, and one pawn at a time.
  The terminal stays mounted across rooms (§20.12), so the PTY lives in state, not in a page.
- `src-tauri/src/pawn/mailbox.rs`: the watcher, request validation and the tool table above.
- `src-tauri/src/pawn/probe.rs`: the probe table, the record and its reading.
- `src-tauri/src/pawn/workshop.rs`: makes `<mods folder> (workshop)` on first summon; lists
  `dist\` artifacts with receipts; streams the digest of each; and watches the folder.
- `src-tauri/src/pawn/bring.rs`: the bring-through, built on `landing.rs`.
- `src-tauri/resources/pawn/`: `relay.py`, the persona template (#00015 §20.10) and the
  workshop's starter `CLAUDE.md` (DD2 mod facts and the plain-where-it-lasts rule), all written
  out by the stone at summon.

### The receipt

`<name>.receipt.json`, written by the pawn, read by the stone and never trusted:
`{ "artifact": "<name>.zip", "sha256": "<64 hex>", "builtAt": "<RFC 3339>", "inputs": [{ "path",
"sha256" }], "checks": [{ "name", "passed", "total" }] }`, ≤ 64 KiB. The stone shows it under
`THE RECEIPT SAYS`, attributed, with no check marks (§20, Bold Choice 26). The receipt's
`sha256` is compared with the stone's own hash of the file. When they differ the row says
`the receipt names other bytes`, and the card cannot open.

### Criteria (#00068 Phase 8)

- [GATE] `fence.rs`: the argv mounts exactly the allowlist (a test lists every `--bind`,
  `--ro-bind` and `--symlink` against the plan); `--remount-ro /`, `--die-with-parent` and the
  filter fd are present; no `/init`, `/mnt/c` root, `userdata` or `~/.claude` mount can be
  produced from any `FencePlan` (a property test over drawn plans). The filter's bytes equal the
  15 instructions. The argv carries `--clearenv` and exactly the environment allowlist. The input
  validator refuses an equal path, a child, an ancestor and an alias of each protected root, and
  accepts a sibling (a table test over invented trees, with junctions on the bench).
- [GATE] The mailbox refuses every malformed, oversized, unknown-tool, extra-key, mis-named,
  linked, reparse-pointed, non-regular, wrong-run and stale-plan request (a table test). A
  replayed `id` returns the ledger's answer without acting, and a duplicate sent while the first
  is still running joins it (one mutation, two identical answers). `install` from a forged request is
  exactly as strong as the tool's (the same RS-3 refusals).
- [GATE] Runs: a request in a retired run's folder is never read; a mutation in flight at
  departure commits or rolls back, and is recorded; a held Play dies with its run.
- [GATE] Put back: a run that only uninstalled a player's install offers an empty *remove* and
  lists that uninstall under *review* (the Assay's counterexample, as a test).
- [GATE] Bring-through: lands only the frozen digest; `alreadyHad`, `nameTaken` and
  `bytesChanged` each change nothing in the mods folder (a tempdir test per case).
- [GATE] The journal's actor: the three tests above.
- [GATE] RS-10's three Sentinel steps, green, with the positive control.
- [BENCH] The probe on the homestation: `CONTAINED`, the record kept in the lab. Then once again
  after `claude` updates, showing `UNPROBED` until it is rerun.
- [BENCH] The investor summons Tamsin, signs in once, and she builds Stature 0.2 against
  pinned inputs. The investor brings that digest through, she installs it, the investor presses
  Play, and she reads the log.

### Out of scope

THE MUSTER and the saves (the generic pawn reads none: RS-8 is not amended). Any public or
distributed form of the pawn. A pawn on native Windows. Restoring a build into the workshop from
the library. Network limits beyond Claude Code's own sandbox (the fence shares the network so
the pawn can reach Anthropic; the shell's sandbox limits its own domains).
