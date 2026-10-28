# The workshop

This folder is a Dragon's Dogma 2 mod workshop. The Riftstone made it beside the
mods folder. It is the only place files are created or changed here.

## What reaches the game, and how

- A finished build is a `.zip` in `dist/`, with a receipt beside it named
  `<name>.receipt.json`. Nothing in the workshop reaches the game by itself: the
  player brings one build into the library with one press, by its SHA-256, and
  installs it from there.
- The receipt is JSON, at most 64 KiB, with exactly these keys:
  `{ "artifact": "<name>.zip", "sha256": "<64 lowercase hex>", "builtAt": "<RFC 3339>",
  "inputs": [{ "path", "sha256" }], "checks": [{ "name", "passed", "total" }] }`.
  It states only what was actually done and measured. A check that did not run is
  not listed. `sha256` must be the hash of the zip's bytes as they are now; a rebuild
  changes it.
- Read-only inputs (for example a pinned extract of the game's files) are mounted
  at `/inputs/<n>`. They cannot be written.
- The stone's tools are the only way to touch the library or the game: search it,
  read an option, plan and install, uninstall, ask to play (the player answers),
  and read REFramework's log. Their refusals are final.

## How Dragon's Dogma 2 loads mods (2026 engine)

- `pak_mods/*.pak` are loaded by REFramework in file-name order (alphabetical);
  the last name wins. The `.pak` extension check is case-sensitive. More than 100
  paks is unsafe.
- Loose files under `natives/` that exist on disk are served ahead of every pak.
- REFramework Lua scripts live in `reframework/autorun/`; their data files in
  `reframework/data/`.
- Both lists are read at game start: an install while the game runs loads at the
  next launch.
- Title Update 3.2 (2026-09) changed asset suffixes, e.g. `.mesh.240423143` to
  `.mesh.260421070` and `.tex.760230703` to `.tex.251211553`. Loose files with the
  old suffixes do not load.
- A package the Riftstone can install has a `modinfo.ini` (`name=`, `version=`,
  `author=`, `description=`) at each option's root, beside that option's
  `natives/`, `pak_mods/` or `reframework/` folders.

## Plain where it lasts

Code, comments, file and folder names, commit messages, receipts and every tool
argument are plain and technical. Any voice belongs in conversation only.
