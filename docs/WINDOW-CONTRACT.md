# The Riftstone — Window Contract

The bridge between the Rust spine (`src-tauri/`) and the Vue window (`src/`).
Types: `src/types/rift.ts` (TypeScript, authoritative) mirrored by
`src-tauri/src/dto.rs` (serde, `rename_all = "camelCase"`). The window never
imports `@tauri-apps/*` outside `src/services/` — components use composables,
composables use services, services use Tauri.

Every command rejects with a `RiftError` (`{ kind, detail }`) on failure. The
window gives it voice (`src/services/voice.ts`): what happened, that we
understand what you tried, what to do next.

## Commands

| Command (invoke name) | Args (JS, camelCase) | Returns | Notes |
|---|---|---|---|
| `get_overview` | — | `Overview` | The home frame polls nothing; it re-reads on events. Not free: it re-reads the game folder so the window sees changes the spine did not make. That read is shared by the calls a burst of events triggers, and is at most 250 ms old. |
| `rescan` | — | `void` | Starts a background scan; progress via `rift://scan-progress`, done via `rift://library-changed`. A second call while scanning is a no-op. |
| `search` | `query: SearchQuery` | `SearchResult[]` | Synchronous, < 50 ms at the investor's scale. Every keystroke may call it (debounce 80 ms in the window). **v0.3:** `limit: 0` means no cap (every match); v0.2 capped it at 200. |
| `get_option` | `optionId` | `OptionDetail` | |
| `get_package` | `packageId` | `PackageDetail` | Includes the full entry list (thousands of rows is fine). |
| `get_library` | — | `LibraryView` | **v0.3.** Every package in the mods folder with its option tree and no entry list: the Rift's idle view (§ v0.3 The library tree). Re-read after `rift://library-changed`, `rift://game-changed` and `rift://updates-changed`. |
| `get_thumbnail` | `optionId, size: "card" \| "full"` | `string \| null` | A `data:image/jpeg;base64,…` URL. `card` ≤ 360 px wide, `full` ≤ 1600 px. Cached on disk by package id + entry; first call per image may take ~100 ms (7z/rar longer). `null` when the option has no picture. |
| `plan_install` | `optionId` | `InstallPlanView` | Pure: nothing is written. |
| `install` | `optionId` | `InstallResultView` | Progress via `rift://install-progress`; `rift://game-changed` after. |
| `plan_uninstall` | `optionId` | `UninstallPlanView` | Pure. |
| `uninstall` | `optionId` | `UninstallResultView` | `rift://game-changed` after. |
| `get_installed` | — | `InstalledView` | |
| `reorder_paks` | `order: string[]` | `void` | Option ids, lowest priority first. Refused while DD2 runs. |
| `set_reframework_switch` | `switch: "looseFileLoader" \| "pakDirectory", on: boolean` | `void` | Refused while DD2 runs. |
| `set_game_root` | `path: string` | `GameSummary` | Manual pick; validates `DD2.exe`. |
| `open_mods_folder` | — | `void` | **v0.3.** Opens the mods folder in Explorer, Rust-side (the opener scope refuses folders from the webview); makes the folder first when it is missing. |
| `plan_launch` | — | `LaunchPlanView` | **v0.3.** Pure. The route Play takes, its warnings (never blockers), or the sentence that refuses it (§ v0.3 Play). |
| `launch_game` | — | `LaunchReport` | **v0.3.** Starts the game Rust-side: `steam://rungameid/2054970` through the shell, or `DD2.exe` with the game folder as working directory. Refused exactly when `plan_launch` refuses. `rift://game-changed` follows when the game watcher sees DD2.exe start. |
| `get_updates` | — | `UpdatesView` | Cached state; no network. |
| `check_updates` | — | `UpdatesView` | Network (anonymous Nexus v2). Progress via `rift://updates-progress`; `rift://updates-changed` after. |
| `identify_packages` | `packageIds: string[]` | `Md5Result[]` | Hashes the archives (progress `hashing`), one batched MD5 lookup. |
| `find_on_nexus` | `packageId` | `NexusFindings` | **v0.2.** Searches Nexus's Dragon's Dogma 2 mods by the package's name (see § Find on Nexus). Sends only on this call; never links. Errors: `entryNotFound`, `nexus` (offline, quota, a refused answer). |
| `link_package` | `packageId, urlOrId: string` | `UpdateRow` | Accepts a nexusmods.com/dragonsdogma2 URL or a bare mod id. |
| `unlink_package` | `packageId` | `void` | |
| `set_nexus_key` | `key: string \| null` | `NexusSummary` | Optional. Stored in Windows Credential Manager; validated with `/v1/users/validate.json`. **v0.3:** the summary carries the account (`key`, `account`). |
| `refresh_nexus_account` | — | `NexusSummary` | **v0.3.** Validates the stored key again (validate.json spends no quota). Rejected key → `key: "rejected"`, not an error; offline → `nexus` error with reason `offline` and `key: "unchecked"`. |
| `get_mod_files` | `modId: number` | `NexusModFiles` | **v0.3.** Anonymous v2 `modFiles`: every file of one mod with its category, size, description and patch notes, and which one the library holds. |
| `browse_nexus` | `query: NexusBrowseQuery` | `NexusBrowsePage` | **v0.3.** Anonymous v2 `mods` over `dragonsdogma2`, 20 a page. Sends only what the player typed. |
| `get_nexus_standings` | `refresh: boolean` | `NexusStandings` | **v0.3, key.** v1 `user/tracked_mods.json` + `user/endorsements.json`, tracked mods named through one v2 lookup. Cached for the session; `refresh` asks again. No key → `nexus` error with reason `noKey`. |
| `set_tracked` | `modId: number, tracked: boolean` | `NexusStandings` | **v0.3, key.** v1 POST/DELETE `user/tracked_mods.json?domain_name=dragonsdogma2`. |
| `set_endorsement` | `modId: number, status: "endorsed" \| "abstained"` | `NexusStandings` | **v0.3, key.** v1 `endorse.json` / `abstain.json` with the version the library holds (else the mod's current version). Nexus's refusals come back with reasons `endorseNotDownloaded`, `endorseTooSoon`, `endorseOwnMod`. |
| `get_downloads` | — | `DownloadView[]` | **v0.3.** This session's downloads, newest first. Not persisted: a download cut short by a quit leaves nothing (its partial file is removed at the next launch). |
| `start_download` | `modId: number, fileId: number, forPackageId: string \| null` | `DownloadView` | **v0.3, key.** Premium: queued at once. Free: phase `waiting` with `nexusPageUrl` (the file's page, `?tab=files&file_id=<id>&nmm=1`), which the window opens; the site's Mod Manager Download link resumes it. One download runs at a time. Emits `rift://downloads-changed`. |
| `cancel_download` | `id: string` | `void` | **v0.3.** Waiting, queued, linking or downloading → `cancelled`, its partial file removed. Refused (`refused`) for a landed, failed or cancelled row. |
| `clear_downloads` | — | `void` | **v0.3.** Forgets landed, failed and cancelled rows. |
| `get_nxm_handler` | — | `NxmHandlerView` | **v0.3.** Reads `HKCR\nxm\shell\open\command`. |
| `set_nxm_handler` | `on: boolean` | `NxmHandlerView` | **v0.3.** On: remembers the current command, then writes `HKCU\Software\Classes\nxm` for this exe. Off: restores the remembered command, or removes the key only when it still points at this exe. Off Windows: `refused`. |
| `get_troubled_packages` | — | `PackageSummary[]` | The §9 trouble cards: every package carrying one of the four trouble kinds, including those with zero options — `unreadable` (can't be opened; ember rail), `nothingToInstall`, `outdatedFormats` (pre-TU 3.2 files), `filesSkipped`. Ordered by the first of those kinds a package carries, in that order, then library order. Packages with `outdatedFormats` or `filesSkipped` still install; a card speaks for its first trouble kind (`services/voice.ts` `troubleLine`), never for `problems[0]` blindly. |
| `get_nexus_quota` | — | `NexusQuota \| null` | The v1 budget from the last `X-RL-*` headers; `null` when no key is set or no v1 call has run (anonymous v2 carries no quota headers). **v0.3:** the launch validation runs a v1 call, so a stored key shows its quota from the start. The v1 quota governs only key calls (downloads, tracking, endorsing); the anonymous update check never waits on it. |
| `get_labels` | — | `LabelFacets` | **v0.2.** The Nexus categories and tags of every linked package, from the latest update check (stored in `nexus.json`), with installable-option counts. The window refetches it after `rift://updates-changed` and `rift://library-changed`. |
| `restore_parked` | `store: string` | `ParkedRestoreReport` | **v0.2.** Puts a parked file back (rift-deploy `restore_parked`). Anything at the path is parked in its turn (`swappedOut`). Refused while an install owns the path, or for a handle the list no longer holds (the refusal names the game path, never the store handle, and says to look again at PARKED; the window reloads the list after any refusal). While the game runs it still puts the file back and answers `restartNeeded: true`; a file the game holds open fails the move, which is rolled back and comes back as an error, never as a half-restore. So `restored` is always `true` on success. Emits `rift://game-changed`. |
| `discard_parked` | `store: string` | `void` | **v0.2.** Deletes a parked file from the backup store for good (rift-deploy `discard_parked`). Refused (`refused`) for a handle the list no longer holds. Emits `rift://game-changed`. |
| `get_settings` | — | `Settings` | |
| `set_settings` | `settings: Settings` | `Settings` | |

Opening a URL or revealing a file is done window-side with
`@tauri-apps/plugin-opener` (`openUrl`, `revealItemInDir`) through
`src/services/rift.ts`.

## v0.2 — Nexus labels as filters

The investor's ask: *"is it also possible to use the tags from nexus for our overview? so that i can
easily filter on those tags?"* Every Nexus mod carries a `category` (always one) and `tags`
(often none), plus an `adultContent` flag. They arrive in the same batched `legacyModsByDomain`
call the update check already makes, at zero extra requests. Labels belong to the Nexus MOD, so
every option of a linked package inherits them.

- **Search:** `SearchFilters.categories` is ANY-of and `SearchFilters.tags` is ALL-of. Both compare
  case-insensitively, and both are empty for no filter. Free text also matches labels, as the
  `nexusLabel` field (weight 4, between bundle and author). Every `SearchResult` carries
  `nexusCategory`, `nexusTags` and `adult`. A package Nexus has not labelled (unlinked, linked but
  never checked, or checked with an answer that carried no labels) has none of them, so a label
  filter hides it. `LabelFacets.unlabeledPackages` counts the packages with at least one
  installable option and no category or tag; an adult flag alone still counts as unlabeled. The
  facets leave out labels that only menu entries carry, since their chips would find nothing.
- **Detail:** `OptionDetail.labels`, read from the same index as search, so the two never disagree.
- **Kept like verdicts:** a check keeps the labels of every mod it did not ask about (a package that
  was out of the mods folder at the time, say). A mod's labels are forgotten only when no package on the shelf and no
  record's link or verdict names it any more (an unlink, or a re-link to another mod).
- **Veil:** `Settings.veil` is `off`, `adult` or `all`. `adult` veils the options of mods Nexus
  flags adult **and of packages Nexus has not labelled yet** (no category, no tags and not adult:
  unlinked, never checked, or an answer without labels; `ledger.ts` `adultnessOf`). A veil exists to protect, so the unknown is veiled; it
  clears as packages are linked and checked. Cards, the detail and the lookbook apply the same
  rule. (Ruled 2026-10-16, after the contract verifier found the three surfaces disagreeing.)

## Find on Nexus (v0.2)

Tags reach linked packages only, and MD5 cannot identify an unpacked folder or a repack. Find
searches Nexus by the package's NAME, anonymously, and offers candidates; the player picks one.

- **Terms** (`rift_nexus::search_terms`; its doc lists the ten rules in order). A trailing
  `.zip`/`.7z`/`.rar` goes, and a Nexus download name (2026 or classic shape) keeps only its name
  part. `(…)`/`[…]` go, nested ones too, though a name that is all brackets reads its brackets. A
  2026 upload stamp goes wherever it sits, and so does the sqid that ends a name right after one.
  Any punctuation separates words; typographic apostrophes become `'`. camelCase splits, but
  acronyms, `REFramework` and `ReShade` stay whole. Versions (`v1.2b`, `1.2.3a`, a lone `v`, `ver`,
  `version`) and standalone numbers go, while multipliers (`1.5x`) and words with digits (`4K`,
  `1080p`) stay. "Dragon's Dogma 2" goes (any apostrophe, with a "for" just before it), and so
  does the abbreviation `dd2`. At most 8 words and 100 characters after all that. "Searchable" means at least one word
  of 2+ letters. A dropped word only widens the search (Nexus wants every term); a stray one finds
  nothing.
- **Which names:** the first two different searches among the display name, the first option's
  bundle and that option's name. A name with nothing searchable is skipped. A second search runs
  only when the first found nothing. `terms` is the set answered last; `""` when none held anything
  searchable, and then nothing was sent.
- **Query:** GraphQL `mods(filter: { gameDomainName: dragonsdogma2, nameStemmed: <terms> MATCHES },
  sort: relevance DESC, count: 5)`, the terms as a variable, never in the query text. No usable
  candidate → one retry with `WILDCARD`. At most 2 POSTs per search and 4 per call, through the
  update check's GraphQL path (headers, the shared 429 back-off, a GraphQL error refuses the
  answer). Skipped: a node for another game, without a name, or with a mod id outside
  1..=i32::MAX; a repeated mod id counts once.
- **Linking** stays the player's act: `link_package(packageId, String(modId))`, then
  `check_updates`. The check brings the mod's labels and name, but a link by mod id carries no
  file id, sqid, upload stamp or version, so the row stays `unpinned`: the stone cannot tell
  whether the player's copy is the newest. Pasting the file's own Nexus URL (`file_id=`) pins it.
  (A version seeded from modinfo was considered and left out: it is a guess the pinning design
  avoids.)
- **Privacy:** names leave the machine only when the player presses Find (one row, or Find all
  by name): the display name, and when that finds nothing, the first option's bundle or name.

## Check ids

`Overview.checks[].id` is stable. PreflightPage draws six facts as rows of its own (`OWN_CHECKS` in
`src/pages/PreflightPage.vue`) and lists every other check below them. A spine test (`dto.rs`) reads
`OWN_CHECKS` at compile time, so a rename on either side fails the build.

- **The page's own rows:** `game`, `build` (rift-deploy's `buildChanged`), `reframework`, `pakdir`
  (`pakDirectory`), `loose` (`looseFileLoader`), `mods` (v0.3, the spine's mods-folder check). The
  spine maps rift-deploy's names to these (`dto::WINDOW_CHECK_IDS`). rift-deploy's `faultyFiles` is
  never sent as a check, because the page draws that row from `reframework.faultyFiles`.
- **From rift-deploy, listed below:** `interrupted`, `paksLoaded`, `pakCount`, `pakNested`,
  `pakExtension`, `strayPakOrder`, `missingFiles`, `parkedBackups`, `strays`, `outdatedFormats`,
  `gameRunning`. (v0.3 retires two v0.2 checks: the renamed-crash-reporter note and the
  un-adopted-installs note.)
- **From the spine, listed below:** `journal`, `unreadable`, `nothingToInstall`, `relinked` (v0.3,
  Info: installs the stone found again in the mods folder after a move, § v0.3 Legacy data),
  `settingsUnreadable`, `nexusUnreadable`. The last two appear when a stored file exists but cannot
  be read; that session then never writes over it. (v0.3 retires `roots` and `rootsUnreadable`.)
- **`mods`** (v0.3): Error when the mods folder is missing and could not be made (action
  `openModsFolder` is absent then; the detail names the path and the reason); Warn when it exists but
  cannot be listed; Info "The mods folder is empty" with action `openModsFolder` when it holds no
  package; absent when it holds packages. The page's own Mods folder row draws it.

**Stand-in option ids.** An adopted install that matched no library option carries a stand-in id
(v0.3: `adopted#<label>`, re-keyed from v0.2's spelling when the journal is lifted, § v0.3 Legacy
data). It shows as `InstalledRow.optionId` with `packageMissing: true`. `get_option` answers
`entryNotFound` for it, and `plan_uninstall` works on it. The window treats every option id as
opaque and never parses it.

## Events

| Event | Payload | When |
|---|---|---|
| `rift://scan-progress` | `ScanProgressEvent` | After each package of a scan. |
| `rift://library-changed` | `{}` | A scan finished (manual, startup, or the library watcher). |
| `rift://install-progress` | `InstallProgressEvent` | During extraction. |
| `rift://game-changed` | `{}` | After install / uninstall / reorder / switch / game root change / a relink, and after `restore_parked` / `discard_parked`. **v0.3:** also when the game watcher sees DD2.exe start or stop. Never after a refusal. |
| `rift://updates-progress` | `UpdatesProgressEvent` | During `check_updates` / `identify_packages`. |
| `rift://updates-changed` | `{}` | After an update check or a link change. **v0.3:** also after a download lands (its package is linked by the download) and after the account's standing changes. |
| `rift://downloads-changed` | `{}` | **v0.3.** A download row appeared, changed phase or was cleared. |
| `rift://download-progress` | `DownloadProgressEvent` | **v0.3.** While a download moves bytes: at most one per 200 ms per download, plus the last. |
| `rift://nxm-received` | `NxmReceivedEvent` | **v0.3.** An nxm:// link reached the stone (cold start or a second launch). The spine also brings the window forward. |

## Startup sequence (Rust side)

0. (v0.3) `tauri-plugin-single-instance` is the first plugin: a second launch (Windows starting
   `riftstone.exe "nxm://…"`) hands its arguments to the running stone and exits.
1. Load settings, the journal (lifting a schema-1 journal in memory, § v0.3 Legacy data), the catalog
   cache and the Nexus cache from the app data dir. `roots.json` (v0.2) is never read, written or
   deleted.
2. `recover()` a stale journal intent → `Overview.recovered`.
3. Resolve the mods folder (`RIFTSTONE_MODS_DIR`, else `<folder of riftstone.exe>\mods`) and make it
   if it is missing; remove the stone's own leftover partial downloads in it
   (`.riftstone-*.part`, nothing else). Discover the game and REFramework.
4. Show the window immediately with the cached catalog; start a background rescan.
5. Arm a debounced (1.5 s) watcher on the mods folder: a change rescans; the window hears
   `rift://library-changed`. After every scan: the folder-id re-key and the relink pass
   (§ v0.3 Legacy data).
6. (v0.3) Start the game watcher: every 2 s it samples the process list; a start or a stop
   emits `rift://game-changed`.
7. (v0.3) With a stored key: validate it (`/v1/users/validate.json`, no quota) → `NexusSummary.key`,
   `account` and the quota.
8. If `autoCheckUpdates` and the last check is older than 12 h: `check_updates` in the
   background after the first scan.
9. (v0.3) An nxm:// link in the stone's own command line is handled once the spine is awake.

## v0.3 — one folder, the tree, Play, and the key's extras

The investor's ask (2026-10-17): no other mod manager is needed or named any more; one `mods`
folder beside the installed stone is the only place mods come from; the Rift opens on a classic
tree and a search shows everything; the Nexus key does visible work, including downloading from
the stone itself; a Play button starts the game; the name stays.

### The one folder

- `<install dir>\mods` (the folder holding `riftstone.exe`), or `RIFTSTONE_MODS_DIR` when set (the
  WSL bench, `npm run tauri dev`). Resolved once at launch; never chosen in the window. No roots, no
  Downloads root, no root filter, no `roots.json`.
- Every zip, 7z or rar and every folder directly inside it is one package. Nothing is hidden for
  "not looking like a mod": a stray archive shows as a package with `nothingToInstall`. Names
  starting with `.` and in-flight downloads (`.crdownload`, `.part`, `.tmp`) are never packages.
- RS-2, v0.3 wording: **the stone never changes, renames or deletes a file in the mods folder.** It
  adds exactly one kind of file there: a download the player asked for, written under a hidden
  partial name (`.riftstone-<id>.part`) and renamed to its final name only when whole and checked,
  never over an existing file.
- Uninstalling the stone leaves the mods folder alone: Tauri's NSIS uninstaller deletes only the
  files it installed and removes `$INSTDIR` only when empty (`RMDir`, not `RMDir /r`; read from
  tauri-cli 2.12's template on 2026-10-17). An upgrade install likewise.

### Legacy data (the investor's v0.2 data keeps working)

- **Journal schema 2.** rift-deploy reads a schema-1 journal through a lift on the raw JSON, then
  saves schema 2 at its next write (never at load): an install origin of any kind other than
  `riftstone` becomes `{ "kind": "adopted", "label": <its section, else the install's name> }`; a
  backup or parked owner of any kind other than `unowned` or `riftstone` becomes `unowned`; an option
  id whose package half is not 16 hexadecimal digits (a stand-in) becomes `adopted#<root half>`,
  everywhere it is referenced. No legacy literal is spelled in code or tests (the tests lift an
  invented kind). A v0.2 build that meets a schema-2 journal calls it damaged and refuses changes;
  it never resets it.
- **Folder package ids follow the folder's name.** v0.2 hashed a folder package's absolute path, so
  the investor's move gave every unpacked folder a new id. v0.3: `PackageId::for_folder(<folder
  name>)`. After each scan, the spine re-keys, in one journal save and one `nexus.json` save, every
  package id it can compute exactly: the v0.2 id of each folder now in the mods folder
  (`PackageId::legacy_folder(<absolute path>)`) → its v0.3 id.
- **The relink pass.** After each scan, an install whose package is not in the mods folder is matched
  to a library option when exactly one option fits: the same option root (case-blind), every
  recorded loose, script or root file present in the option's payload at the same game path, pak
  files only if the option carries paks, and — when the root is empty or the install has only pak
  files — the same name as the option or its package (case-blind). The match rewrites the install's
  option id everywhere in one save (refused while a change is pending; never onto an id the journal
  already holds), then `rift://game-changed`. Unmatched installs stay listed with
  `packageMissing: true` and uninstall from the journal as before. A `relinked` Info check reports the
  count once per session.

### The library tree (get_library)

- The Rift's idle view: no text in the well and no facet narrowing (state, menu-only, labels) → the
  tree. Any text or narrowing → today's flat answers, uncapped (`limit: 0`). Clearing returns to the
  tree with its expansion as it was (per session, never stored).
- Top level: every package of the mods folder, collapsed, by display name. A package row carries its
  counts (`installable`, `installed`), update badge, labels and problems. Under it, `tree` nests the
  options exactly as `get_package` does (built by rift-catalog `tree::nest`, decorated by the spine);
  menu nodes are branches and always show in the tree.
- The window builds the tree as a WAI-ARIA tree (one tab stop, ↑ ↓ ← → Home End, Enter opens an
  option in Your World or toggles a branch). The trouble fold stays above it, outside its role.

### Play

- Route: `steam://rungameid/2054970` when the game was found through Steam (Steam starts if needed and
  applies the player's launch options; REFramework's `dinput8.dll` loads from the game folder either
  way); otherwise `DD2.exe` started with the game folder as its working directory.
- Refused (one sentence each): no game found; the game already runs; a change is being made
  (an install, uninstall, reorder, switch or restore holds the game folder); a change was cut short
  (the journal holds a pending intent).
- Warnings, never blockers: paks installed and REFramework missing; the pak folder switch off; the
  loose file loader off while loose files are installed; Steam updated the game since the newest
  install.
- Running state: the spine's game watcher samples the process list every 2 s (Windows; a seam in
  tests) and emits `rift://game-changed` on a start or a stop. `GameSummary.running` and `startedMs`
  come from it. While the game runs, the engine's existing rules stand: installs land and load at the
  next launch; pak uninstalls, reorders and switches are refused.

### The key's extras

- **Account.** A stored key is validated at launch (validate.json spends no quota) and on
  `refresh_nexus_account`. The window shows the account's name and whether it is premium; never the
  key, never the e-mail.
- **What the key unlocks** (all v1, all counted against the account's quota except validate):
  downloads (premium: in the stone; free: through the site's Mod Manager Download button and the
  nxm handler), tracking and untracking mods, endorsing and abstaining. Without a key all of the
  anonymous v0.2 features keep working, and browsing Nexus and reading a mod's files and patch notes
  need no key either (v2, anonymous).

### Downloads

- `start_download(modId, fileId, forPackageId)`: no key → `nexus`/`noKey`. Premium → `queued`; free
  → `waiting` with `nexusPageUrl`. One download runs at a time, in order.
- The run: `linking` (v1 `download_link.json`, with the nxm link's `key` and `expires` for a free
  account; the first mirror is Nexus's preferred one) → `downloading` (a separate HTTP client that
  never holds the API key, https only except loopback, an idle timeout instead of a total one,
  streaming to `<mods>\.riftstone-<id>.part` with the MD5 computed on the way) → `verifying` (the byte
  count against Nexus's size and the server's length; then the MD5 through the anonymous
  `fileHashes` lookup must name the same mod and file; a hash Nexus does not know yet is accepted on
  the size alone) → `landed`.
- Landing: the archive keeps Nexus's own file name (v1 `files.json` `file_name`, else the mirror
  URL's last path segment), cleaned for Windows. If a file of that name exists: the same size and MD5
  → `alreadyHad`, nothing written; else ` (2)`, ` (3)` … before the extension. Then `nexus.json`
  links the new package (`PackageId::for_archive(name, size)`) to the exact mod and file with source
  `download`, and a scan runs (`rift://library-changed`). The old archive of an updated package
  stays where it is: removing it is the player's act in Explorer.
- Failure or cancel: the partial file is deleted, nothing else in the mods folder is touched. The
  CDN URL and the nxm `key` are never logged, never put in an error and never sent to the window.

### The nxm handler

- Off by default. Turning it on is the player's act on THE FAR WORLD (the investor's machine had no
  handler on 2026-10-17). On: the spine remembers the command it replaces (in `nxm-handler.json` in
  the app data folder) and writes `HKCU\Software\Classes\nxm` = `URL:NXM Protocol`, `URL Protocol`
  = "", `shell\open\command` = `"<riftstone.exe>" "%1"`. Off: it restores the remembered command, or
  deletes the key only when it still points at this exe. The installer never registers it.
- An arriving link is untrusted input. Accepted only when it parses as
  `nxm://dragonsdogma2/mods/<mod>/files/<file>?key=…&expires=…` (unknown parameters ignored), has not
  expired, carries a `user_id` equal to the validated account's (when both are known) and a key is
  stored. It resumes the waiting download for the same mod and file, else starts a new one. `nxm://
  premium` re-validates the account. Collections and other games are refused with a sentence.
