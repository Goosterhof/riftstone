# The Riftstone

*Touch the stone and the Rift answers. Mods from other worlds wait inside; the stone shows you every one of them and pulls through the one you choose.*

The Riftstone is a Windows desktop mod manager for **Dragon's Dogma 2**. Its library is one folder: the `mods` folder beside the installed `riftstone.exe`. Put a mod there (zip, 7z, rar or a plain folder, nested as deep as the modder packed it) and the stone reads all of it: option names, descriptions, the menu trail a variant hides behind, and every file path inside the archives.

- Open the stone and the Rift shows your packages as a tree. Open one and go deeper, menu by menu, down to the variant you want.
- Type `cloak_301` and get the variants that carry that mesh, even out of a collection with hundreds of options. A search shows every match, each with its menu trail.
- Install, uninstall and reorder the way the game loads mods after the September 2026 engine update. Pak mods go in REFramework's `pak_mods/` folder in name order (the last name wins). Loose `natives/` files beat paks. Scripts go in `reframework/`.
- Press Play and the stone starts the game (through Steam when Steam found it), and knows when it is running.
- It asks Nexus Mods whether a newer file of each download exists. The check is anonymous and needs no account. A 2026 download name carries the file's own id, so the stone checks that exact file.
- Filter by what Nexus calls a mod. Each linked mod's category and tags come home with that same check. Press categories in the stone (any of them) or tags (all of them), and every chip tells you how many answers it gives. The screenshots of mods Nexus flags adult, and of anything it has not labelled yet, can stay blurred until you hover. A download Nexus cannot recognise by its bytes (an unpacked folder, a repack) can be found by name, and you pick the right mod from up to five.
- Give it your personal Nexus key and it shows your account, tracks and endorses mods, and downloads files straight into the mods folder: a newer version from the update list, any file of a mod, or a mod you found by browsing Nexus from the stone. A premium account downloads in the stone. A free account presses Nexus's own Mod Manager Download button, which reaches the stone once you turn on its link handler. A download lands in the mods folder; installing it stays your choice.

## The promises

- **The mods folder is yours.** The stone never changes, renames or deletes anything you put in it. The one thing it adds is a download you asked for. While it arrives it sits under a hidden partial name, which the stone removes if the download fails or you cancel it, and it takes its real name only once it is whole and checked.
- **Vanilla is sacred.** `DD2.exe`, the game's own `re_chunk_000.pak*`, `dlc/` and their kin are refused outright. Any other file a mod would overwrite is backed up first and put back on uninstall.
- **A write-ahead journal.** Every install records what it means to do before the first byte moves. If one is cut short by a crash or a power cut, it is rolled back at the next launch.
- **Nexus is optional.** Searching, installing, update checks, labels and browsing Nexus work without an account. Only the key's extras need one: your account, downloads, tracking and endorsing. A personal API key lives in Windows Credential Manager and never in a file; the stone never shows it, logs it or sends it anywhere but Nexus.
- **Uninstalling the stone leaves your mods.** The uninstaller removes only the files it installed. Its *Delete the application data* box, unticked unless you tick it, also removes the stone's journal and backups, so uninstall your mods from the stone before you tick it.

## Development

```sh
npm install
npm run dev            # the window in a browser on the mock backend (?mock=empty|nogame|scanning|premium|running|…)
npm run tauri dev      # the real app (WSL bench: set RIFTSTONE_GAME_ROOT / RIFTSTONE_MODS_DIR / RIFTSTONE_DATA_HOME)
npm run tauri build    # on Windows: target/release/riftstone.exe and an unsigned NSIS installer
```

The lab journal is in [`CLAUDE.md`](CLAUDE.md): architecture contract, crate map, load rules and gates. Beside it are the contract every crate was built to ([`docs/BUILD-SPEC.md`](docs/BUILD-SPEC.md)) and the window contract ([`docs/WINDOW-CONTRACT.md`](docs/WINDOW-CONTRACT.md)).

The git log and the journals are the laboratory's working diary, kept as they were written. In them, "the investor" is the player the stone was built for. Everyone else named there (the main session, its lenses and fixers, the Chaos Monkey, the Artisan, the Illusionist) is an AI agent that built or reviewed it.

Part of the [Zmuuzn laboratory](https://github.com/Goosterhof/zmuuzn). Not affiliated with Capcom, Nexus Mods or the authors of REFramework. Dragon's Dogma 2 is Capcom's; the stone only reads and places your own downloads in your own game folder.
