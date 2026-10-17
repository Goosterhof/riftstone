# The Riftstone — Window Contract

The bridge between the Rust spine (`src-tauri/`) and the Vue window (`src/`).
Types: `src/types/rift.ts` (TypeScript, authoritative) mirrored by
`src-tauri/src/dto.rs` (serde, `rename_all = "camelCase"`). The window never
imports `@tauri-apps/*` outside `src/services/` — components use composables,
composables use services, services use Tauri.

Every command rejects with a `RiftError` (`{ kind, detail }`) on failure. Since
v0.3 a `nexus` error's detail is `{ detail: <sentence>, reason?: NexusReason }`,
and the window voices it by `reason` first. The window gives every failure voice
(`src/services/voice.ts`): what happened, that we understand what you tried, what
to do next.

## Commands

| Command (invoke name) | Args (JS, camelCase) | Returns | Notes |
|---|---|---|---|
| `get_overview` | — | `Overview` | The home frame polls nothing; it re-reads on events. Not free: it re-reads the game folder so the window sees changes the spine did not make. That read is shared by the calls a burst of events triggers, and is at most 250 ms old. |
| `rescan` | — | `void` | Starts a background scan; progress via `rift://scan-progress`, done via `rift://library-changed`. A second call while scanning is a no-op. |
| `search` | `query: SearchQuery` | `SearchResult[]` | Synchronous, < 50 ms at the investor's scale. Every keystroke may call it (debounce 120 ms in the window, `DEBOUNCE_MS` in `src/composables/useSearch.ts`). **v0.3:** `limit: 0` means no cap (every match); v0.2 capped it at 200. |
| `get_option` | `optionId` | `OptionDetail` | |
| `get_package` | `packageId` | `PackageDetail` | Includes the full entry list (thousands of rows is fine). **v0.3:** its tree is the one `get_library` serves (rift-catalog `tree::nest`). |
| `get_library` | — | `LibraryView` | **v0.3.** Every package in the mods folder with its option tree and no entry list: the Rift's idle view (§ v0.3 The library tree). By display name (case-blind), then file name; a package that yielded no option carries its problems and an empty tree. Re-read after `rift://library-changed`, `rift://game-changed` and `rift://updates-changed`. |
| `get_thumbnail` | `optionId, size: "card" \| "full"` | `string \| null` | A `data:image/jpeg;base64,…` URL. `card` ≤ 360 px wide, `full` ≤ 1600 px. Cached on disk by package id + entry; first call per image may take ~100 ms (7z/rar longer). `null` when the option has no picture. |
| `plan_install` | `optionId` | `InstallPlanView` | Pure: nothing is written. |
| `install` | `optionId` | `InstallResultView` | Progress via `rift://install-progress`; `rift://game-changed` after. |
| `plan_uninstall` | `optionId` | `UninstallPlanView` | Pure. |
| `uninstall` | `optionId` | `UninstallResultView` | `rift://game-changed` after. |
| `get_installed` | — | `InstalledView` | |
| `reorder_paks` | `order: string[]` | `void` | Option ids, lowest priority first. Refused while DD2 runs. |
| `set_reframework_switch` | `switch: "looseFileLoader" \| "pakDirectory", on: boolean` | `void` | Refused while DD2 runs. |
| `set_game_root` | `path: string` | `GameSummary` | Manual pick; validates `DD2.exe`. |
| `open_mods_folder` | — | `void` | **v0.3.** Opens the mods folder in Explorer, Rust-side (the opener scope refuses folders from the webview). A missing folder is made first, then watched and scanned. |
| `plan_launch` | — | `LaunchPlanView` | **v0.3.** Pure. The route Play takes, its warnings (never blockers), or the sentence that refuses it (§ v0.3 Play). No game found: route `exe` and the no-game sentence. A journal that cannot be read plans as if nothing were installed. |
| `launch_game` | — | `LaunchReport` | **v0.3.** Starts the game Rust-side: `steam://rungameid/2054970` through the shell, or `DD2.exe` with the game folder as working directory. Refused exactly when `plan_launch` refuses. It holds the game folder's turn while it asks Steam or Windows, so no change starts in the same moment; then the game watcher samples every 1 s for up to 120 s. `rift://game-changed` follows when the watcher sees DD2.exe start. |
| `get_updates` | — | `UpdatesView` | Cached state; no network. |
| `check_updates` | — | `UpdatesView` | Network (anonymous Nexus v2). Progress via `rift://updates-progress`; `rift://updates-changed` after. |
| `identify_packages` | `packageIds: string[]` | `Md5Result[]` | Hashes the archives (progress `hashing`), one batched MD5 lookup. |
| `find_on_nexus` | `packageId` | `NexusFindings` | **v0.2.** Searches Nexus's Dragon's Dogma 2 mods by the package's name (see § Find on Nexus). Sends only on this call; never links. Errors: `entryNotFound`, `nexus` (offline, quota, a refused answer). |
| `link_package` | `packageId, urlOrId: string` | `UpdateRow` | Accepts a nexusmods.com/dragonsdogma2 URL or a bare mod id. |
| `unlink_package` | `packageId` | `void` | |
| `set_nexus_key` | `key: string \| null` | `NexusSummary` | Optional. Stored in Windows Credential Manager; validated with `/v1/users/validate.json` first: a key Nexus refuses is not stored (`nexus` error, reason `keyRejected`). `null` removes the stored key. **v0.3:** the summary carries the account (`key`, `account`); `rift://updates-changed` follows when the standing changed. |
| `refresh_nexus_account` | — | `NexusSummary` | **v0.3.** Validates the stored key again (validate.json spends no quota). Rejected key → `key: "rejected"`, not an error; a validation that cannot finish (offline, rate-limited, an odd answer) → that `nexus` error (reason `offline` when Nexus is out of reach), and the key reads `unchecked`. Without a key nothing is asked. `rift://updates-changed` follows when the standing changed. |
| `get_mod_files` | `modId: number` | `NexusModFiles` | **v0.3.** Anonymous v2, two calls (`modFiles` with each file's description and patch notes; the mod's name and adult flag): every file of one mod with its category and size, which one the library is pinned to (`inLibrary`) and the newest of each group (`newest`, by rift-nexus's update rules). Sorted main, update, optional, miscellaneous, then the retired ones; newest upload first within a category. A mod Nexus knows nothing of → `nexus` error, reason `fileGone`. |
| `browse_nexus` | `query: NexusBrowseQuery` | `NexusBrowsePage` | **v0.3.** Anonymous v2 `mods` over `dragonsdogma2`, 20 a page. Sends only what the player typed (whitespace collapsed, at most 200 characters, as a GraphQL variable). `relevance` without text sorts by endorsements. Each card's `linkedPackages` counts the library's packages linked to its mod now. |
| `get_nexus_standings` | `refresh: boolean` | `NexusStandings` | **v0.3, key.** v1 `user/tracked_mods.json` + `user/endorsements.json`, tracked mods named through one v2 lookup. Cached for the session (per key: a new key forgets it); `refresh` asks again. No key → `nexus` error with reason `noKey`. |
| `set_tracked` | `modId: number, tracked: boolean` | `NexusStandings` | **v0.3, key.** v1 POST/DELETE `user/tracked_mods.json?domain_name=dragonsdogma2`. Untracking a mod that is not tracked succeeds. Answers the standings after the change: the session cache patched in place, or asked whole when nothing was heard yet this session. `rift://updates-changed` follows. |
| `set_endorsement` | `modId: number, status: "endorsed" \| "abstained"` | `NexusStandings` | **v0.3, key.** v1 `endorse.json` / `abstain.json` with the version the library holds (else the mod's current version). Nexus's refusals come back with reasons `endorseNotDownloaded`, `endorseTooSoon`, `endorseOwnMod`. Answers like `set_tracked`; `rift://updates-changed` follows a change Nexus accepted. |
| `get_downloads` | — | `DownloadView[]` | **v0.3.** This session's downloads, newest first. Not persisted: a download cut short by a quit leaves nothing (its partial file is removed at the next launch). |
| `start_download` | `modId: number, fileId: number, forPackageId: string \| null` | `DownloadView` | **v0.3, key.** No key → `nexus`/`noKey`. A stored key not validated this session is validated first (a refused key → `keyRejected`; Nexus out of reach → `offline`). The file must be on the mod's list (anonymous v2), else `fileGone`. Premium: queued at once. Free: phase `waiting` with `nexusPageUrl` (the file's page, `?tab=files&file_id=<id>&nmm=1`), which the window opens; the site's Mod Manager Download link resumes it. One download runs at a time. Emits `rift://downloads-changed`. A download of the same mod and file already on its way (any phase but landed, failed or cancelled) comes back as it is: no second download, no event. |
| `cancel_download` | `id: string` | `void` | **v0.3.** Waiting, queued, linking, downloading or verifying → `cancelled` at once; the worker removes its partial file at its next look. Once the landing commits (the archive is being put in place), the cancel is refused with a sentence. Refused (`refused`) for a landed, failed or cancelled row, and for an id the list no longer holds. |
| `clear_downloads` | — | `void` | **v0.3.** Forgets landed, failed and cancelled rows. Emits `rift://downloads-changed` only when it removed a row. |
| `get_nxm_handler` | — | `NxmHandlerView` | **v0.3.** Reads what Windows runs for nxm:// links (`HKCR\nxm\shell\open\command`): `ours` when it starts this `riftstone.exe`, `other` (with the program's file name) when it starts another program, `none`, or `unsupported` off Windows. While `ours`, `givesBackTo` names what turning it off gives the links back to: the remembered previous command, else the machine's own handler (HKLM); never a program named `riftstone.exe`. |
| `set_nxm_handler` | `on: boolean` | `NxmHandlerView` | **v0.3.** On, when the user's key does not point at this exe yet: remembers the user's own command (HKCU, unless it starts a `riftstone.exe`, this one or another) in `nxm-handler.json` FIRST, then writes `HKCU\Software\Classes\nxm` for this exe. Off: when the user's key still points at this exe, the remembered command goes back, or the key is removed when there was none (the machine's handler, if any, answers again); a key another program took back is left alone. Off forgets the memory either way. Off Windows: `refused`. |
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
- **`mods`** (v0.3): Error, with no action, when the mods folder is missing: it could not be made
  at launch (the detail names the path and the reason), or it was deleted after launch (the
  detail says the stone makes it again at its next start). Warn, action `rescan`, when it exists
  but cannot be listed. Info "The mods folder is empty", action `openModsFolder`, when it holds no
  package (files that are not packages do not count). Absent when it holds packages. The page's
  own Mods folder row draws it.

**Stand-in option ids.** An adopted install that matched no library option carries a stand-in id
(v0.3: `adopted#<root half>`, the second half of v0.2's stand-in id, re-keyed when the journal is
lifted, § v0.3 Legacy data). It shows as `InstalledRow.optionId` with `packageMissing: true`.
`get_option` answers `entryNotFound` for it, and `plan_uninstall` works on it. The window treats
every option id as opaque and never parses it.

## Events

| Event | Payload | When |
|---|---|---|
| `rift://scan-progress` | `ScanProgressEvent` | After each package of a scan. |
| `rift://library-changed` | `{}` | A scan finished (manual, startup, or the library watcher). |
| `rift://install-progress` | `InstallProgressEvent` | During extraction. |
| `rift://game-changed` | `{}` | After install / uninstall / reorder / switch / game root change / a relink, and after `restore_parked` / `discard_parked`. **v0.3:** also when the game watcher sees DD2.exe start or stop. Never after a refusal. |
| `rift://updates-progress` | `UpdatesProgressEvent` | During `check_updates` / `identify_packages`. |
| `rift://updates-changed` | `{}` | After an update check or a link change. **v0.3:** also after a download lands (its package is linked by the download), after the account's standing changes (a validation, `set_nexus_key`), after `set_tracked` and `set_endorsement`, and after the migration moves a link record to its package's new id. |
| `rift://downloads-changed` | `{}` | **v0.3.** A download row appeared, changed phase or was cleared (`clear_downloads` sends it only when it removed a row). |
| `rift://download-progress` | `DownloadProgressEvent` | **v0.3.** While a download moves bytes: at most one per 200 ms per download, plus the last. |
| `rift://nxm-received` | `NxmReceivedEvent` | **v0.3.** An nxm:// link reached the stone (cold start or a second launch). The spine also brings the window forward. `nxm://premium` sends `accepted: false` with `refusal: null`: nothing to say, while the account is validated again in the background. |

## Startup sequence (Rust side)

Steps 1–3 run in Tauri's `setup` (`AppState::open`), before the window opens, and are fast.
Steps 4–9 run in the background (`AppState::wake`), so the window shows at once. The spine's
comments cite these numbers.

0. (v0.3) `tauri-plugin-single-instance` is the first plugin: a second launch (Windows starting
   `riftstone.exe "nxm://…"`) hands its arguments to the running stone and exits. A link that
   arrives before the spine is awake is dropped with a log line; the player presses the button
   again.
1. Load settings, the catalog cache, the Nexus cache and the stored key. Discover the game
   (`RIFTSTONE_GAME_ROOT`, else the folder the player picked, else Steam) and load its journal
   (lifting a schema-1 journal in memory, § v0.3 Legacy data). A stored file that exists but
   cannot be read starts that part from its defaults and is never written over that session.
   `roots.json` (v0.2) is never read, written or deleted.
2. `recover()` a stale journal intent → `Overview.recovered`.
3. Resolve the mods folder (`RIFTSTONE_MODS_DIR`, else `<folder of riftstone.exe>\mods`) and make it
   if it is missing; remove the stone's own leftover partial downloads in it
   (`.riftstone-*.part`, nothing else). A folder that cannot be made leaves an empty library and
   the `mods` Error check, never a panic.
4. Show the window immediately with the cached catalog (the cached packages that sit directly in
   the mods folder); start a background rescan.
5. Arm a debounced (1.5 s) watcher on the mods folder: a change rescans; the window hears
   `rift://library-changed`. After every scan: the folder-id re-key and the relink pass
   (§ v0.3 Legacy data).
6. (v0.3) Start the game watcher: every 2 s it samples the process list; a start or a stop
   emits `rift://game-changed`.
7. (v0.3) Once the first scan is in, on a thread of its own: with a stored key, validate it
   (`/v1/users/validate.json`, no quota) → `NexusSummary.key`, `account` and the quota.
8. If `autoCheckUpdates`, the last check is older than 12 h and at least one package is linked:
   `check_updates` in the background after the first scan. It is anonymous and runs beside
   step 7: it never waits on the key, its validation or its quota.
9. (v0.3) The nxm:// links in the stone's own command line (a cold start) are handled on step 7's
   thread right after the validation, so each is checked against the account.

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
  starting with `.`, `desktop.ini` and in-flight downloads (`.crdownload`, `.part`, `.tmp`) are
  never packages. Any other file directly inside it (a loose `.pak`, a text file) is not a package
  and is not scanned.
- `mods.exists` (in `Overview` and `LibraryView`) is false whenever the folder is not there at the
  moment of the read: it could not be made at launch, or it was deleted since.
- RS-2, v0.3 wording: **the stone never changes, renames or deletes a file in the mods folder.** It
  adds exactly one kind of file there: a download the player asked for, written under a hidden
  partial name (`.riftstone-<id>.part`) and renamed to its final name only when whole and checked,
  never over an existing file. That partial file is the only file the stone ever removes there: a
  download that fails or is cancelled removes its own, and the next launch removes leftovers.
- Uninstalling the stone leaves the mods folder alone: Tauri's NSIS uninstaller deletes only the
  files it installed and removes `$INSTDIR` only when empty (`RMDir`, not `RMDir /r`; read from
  tauri-cli 2.12's template on 2026-10-17). An upgrade install likewise. The uninstaller's
  "Delete the application data" box (unticked unless the player ticks it; ignored on an upgrade)
  removes `%APPDATA%\nl.zmuuzn.riftstone` and `%LOCALAPPDATA%\nl.zmuuzn.riftstone` whole
  (`RmDir /r`): the journal and its backups, `nexus.json`, the settings, the caches and the logs.
  The mods folder stays either way, but the files the stone installed stay in the game folder
  with no journal left to uninstall them, and the backups under them are gone.

### Legacy data (the investor's v0.2 data keeps working)

- **Journal schema 2.** rift-deploy reads a schema-1 journal through a lift on the raw JSON, then
  saves schema 2 at its next write (never at load): an install origin of any kind other than
  `riftstone` becomes `{ "kind": "adopted", "label": <its section, else the install's name> }`; a
  backup or parked owner of any kind other than `unowned` or `riftstone` becomes `unowned`; an option
  id whose package half is not 16 lowercase hexadecimal digits (a stand-in) becomes
  `adopted#<root half>` (`#2`, `#3` … when that is taken), everywhere it is referenced. No legacy
  literal is spelled in code or tests (the tests lift an invented kind). A v0.2 build that meets a
  schema-2 journal calls it damaged and refuses changes; it never resets it.
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
  `packageMissing: true` and uninstall from the journal as before. A `relinked` Info check reports
  how many installs this session took a new id, by either pass.
- **When it runs.** The re-key and the relink run after every scan, under the deploy lock, and
  never while a change is pending. When a change holds the lock, they run as soon as it lets go.
  A link record that moves sends `rift://updates-changed`.

### The library tree (get_library)

- The Rift's idle view: no text in the well and no facet narrowing (state, menu-only, labels) → the
  tree. Any text or narrowing → today's flat answers, uncapped (`limit: 0`). Clearing returns to the
  tree with its expansion as it was (per session, never stored).
- Top level: every package of the mods folder, collapsed, by display name. A package row carries its
  counts (`installable`, `installed`), update badge, labels and problems. Under it, `tree` nests the
  options exactly as `get_package` does (built by rift-catalog `tree::nest`, decorated by the spine);
  menu nodes are branches and always show in the tree.
- The window builds the tree as a WAI-ARIA tree: one tab stop (a roving `tabindex`), ↑ ↓ ← → Home
  End. Enter on an option opens it in Your World and opens its branch; on a package or a bundle it
  toggles; on a troubled package with nothing to open it opens the trouble fold and focuses that
  package's card. Space zooms an option's screenshot when it has one, else acts as Enter. `*` opens
  every sibling branch of the focused row. No type-ahead: any other printable key goes to the search
  well and starts a search, except the room keys `1`–`4` and `/` (so a word that starts with `l` or
  `i` is a search, never the lookbook or Install…; those stay on Ctrl+L and the detail pane). ↑ on the first
  row returns to the well; ↓ from the well lands on the row the tree last held, else the first.
  Opening an answer from the flat list reveals that option's ancestors, so clearing the well shows
  it in the tree. The trouble fold stays above the tree, outside its role.
- These keys are this contract's rule, and the build follows it. Design #00015 R16.6 reads
  otherwise on three points (Space = Enter, `*` unbound, ↓ from the well to the first row); this
  file is authoritative for behaviour (BUILD-SPEC § v0.3), so the rule above stands and R16.6 is
  owed an amendment to match.

### Play

- Route: `steam://rungameid/2054970` when the game was found through Steam (Steam starts if needed and
  applies the player's launch options; REFramework's `dinput8.dll` loads from the game folder either
  way); otherwise (a folder picked by hand, or `RIFTSTONE_GAME_ROOT`) `DD2.exe` as it is spelled on
  disk, started with the game folder as its working directory, detached.
- Refused (one sentence each, checked in this order): no game found (the route reads `exe`); the
  game folder holds no `DD2.exe` any more; the game already runs; a change is being made (an
  install, uninstall, reorder, switch or restore holds the game folder); a change was cut short
  (the journal holds a pending intent).
- Warnings, never blockers, kept even while Play is refused: paks installed and REFramework
  missing; the pak folder switch off while paks are installed; the loose file loader not on while
  loose files are installed; Steam updated the game since the newest install. The counts come from
  the journal (adopted installs too). A journal that cannot be read does not block Play: it plans
  as if nothing were installed.
- The spine never refuses Play for a warning; the window asks once. A press reads `plan_launch`
  first: a refusal is said; warnings turn Play into an inline confirm on the stone (design #00015
  R16.14: `Play anyway` starts the game, `Fix it` opens PREFLIGHT at the first warned row, `Esc`
  backs out); with no warnings the game starts at once. Starting reads the plan once more and
  calls `launch_game` only while it still refuses nothing.
- Running state: the spine's game watcher samples the process list every 2 s, and every 1 s for up
  to 120 s after Play (Windows; a seam in tests). Only a start or a stop changes what it believes,
  and each emits `rift://game-changed` once. `GameSummary.running` and `startedMs` come from it.
  While the game runs, the engine's existing rules stand: installs land and load at the next
  launch; pak uninstalls, reorders and switches are refused.

### The key's extras

- **Account.** A stored key is validated at launch (validate.json spends no quota), on
  `refresh_nexus_account` and on `nxm://premium`. The window shows the account's name and whether it
  is premium or a supporter; never the key, never the e-mail. The name reaches the window, never a
  log line. A key Nexus refused stays stored (`key: "rejected"`) until the player replaces or
  removes it. Every key change starts over: an answer that began under the old key is never
  applied, and the old key's standings are forgotten.
- **What the key unlocks** (all v1, all counted against the account's quota except validate):
  downloads (premium: in the stone; free: through the site's Mod Manager Download button and the
  nxm handler), tracking and untracking mods, endorsing and abstaining. Without a key all of the
  anonymous v0.2 features keep working, and browsing Nexus and reading a mod's files and patch notes
  need no key either (v2, anonymous).

### Downloads

- `start_download(modId, fileId, forPackageId)`: no key → `nexus`/`noKey`. A stored key not
  validated this session is validated first (refused → `keyRejected`; out of reach → `offline`).
  The file must be on the mod's list (anonymous v2), else `fileGone`. Premium → `queued`; free →
  `waiting` with `nexusPageUrl`. A download of the same mod and file already on its way (any phase
  but landed, failed or cancelled) comes back as it is: no second download. One download runs at
  a time, in order, on a worker of its own: a download never waits for an install or an update
  check, and neither waits for a download.
- The run: `linking` (the file's facts from anonymous v2 when the start did not read them; the
  archive's own name from v1 `files.json`; v1 `download_link.json`, with the nxm link's `key` and
  `expires` for a free account; the first mirror is Nexus's preferred one) → `downloading` (a
  separate HTTP client that never holds the API key, https only except loopback, an idle timeout
  instead of a total one, streaming to `<mods>\.riftstone-<id>.part` with the MD5 computed on the
  way) → `verifying` (the byte count against Nexus's size and the server's length; then the MD5
  through the anonymous `fileHashes` lookup must name the same mod and file; a hash Nexus does not
  know yet is accepted on the size alone) → `landed`. A cancel is honoured at every step until
  the landing commits.
- Landing: the archive keeps Nexus's own file name (v1 `files.json` `file_name`, else the mirror
  URL's last path segment, else `Nexus mod <mod> file <file>`), cleaned for Windows: only the last
  path part; control characters and `<>:"|?*` become `_`; no leading or trailing dots or spaces;
  a reserved device name (`CON`, `NUL`, `COM1` …) takes a leading `_`; at most 180 characters, the
  extension kept. A name without a zip, 7z or rar extension takes the one its first bytes show, so
  the scan sees the package. A file that is no archive at all (a raw `.pak`, an `.exe`) lands
  under its own name, and the scan never lists it. If a file of that name exists (compared
  case-blind): the same size and MD5 → `alreadyHad`, nothing written; else ` (2)`, ` (3)` … before
  the extension. The final name is taken by a hard link where the file system has them (elsewhere
  the stone looks first, then moves), so a file is never written over. Then `nexus.json`
  links the package (`PackageId::for_archive(name, size)`) to the exact mod and file with source
  `download` (`rift://updates-changed`), and a scan runs (`rift://library-changed`) unless the
  landing was `alreadyHad`. The stone never installs a download by itself. The old archive of an
  updated package stays where it is: removing it is the player's act in Explorer.
- Failure or cancel: the partial file is deleted, nothing else in the mods folder is touched. A
  failed row carries the failure the window voices (`failure`, a `nexus` error with its reason
  where Nexus said no). The CDN URL and the nxm `key` are never logged, never put in an error and
  never sent to the window.

### The nxm handler

- Off by default. Turning it on is the player's act on THE FAR WORLD (the investor's machine had no
  handler on 2026-10-17). On: the spine remembers the command the user's own key held (in
  `nxm-handler.json` in the app data folder, `{ schema: 1, previous: <command or null> }`, written
  before the registry is touched) and writes `HKCU\Software\Classes\nxm` = `URL:NXM Protocol`, `URL
  Protocol` = "", `shell\open\command` = `"<riftstone.exe>" "%1"`. A previous command that starts a
  `riftstone.exe`, this one or one elsewhere (an install the player moved away from), is not
  remembered: giving the links back to it would hand them to a dead path. Off: when the user's key
  still points at this exe, the remembered command goes back, or the key is deleted when there was
  none; a key another program took back is left alone. The installer never registers it.
- An arriving link is untrusted input. A second launch hands it over through single-instance; a
  cold start's links wait for the key's validation (startup step 9). Handling a link never waits
  on the network. The checks, in this order:
  1. It must parse as `nxm://dragonsdogma2/mods/<mod>/files/<file>?key=…&expires=…` (unknown
     parameters ignored; a `key` without `expires`, or the reverse, or a `user_id` that is not a
     number, makes it unreadable). Another game, a collection, or a link that cannot be read →
     `refused` with a sentence, and the event's ids are null.
  2. `nxm://premium` → with a stored key, the account is validated again in the background. The
     event says `accepted: false` and `refusal: null`: there is nothing to refuse.
  3. No key stored → `nexus`/`noKey`. A stored key Nexus refused → `nexus`/`keyRejected`.
  4. A file link without its `key` and `expires` → `refused` as unreadable: the site's button
     always sends them.
  5. Expired → `nexus`/`linkExpired`. Made for another account (its `user_id` against the
     validated account's, when both are known) → `nexus`/`linkNotYours`.

  Accepted: the link resumes the waiting download of the same mod and file; a download of that
  file already on its way takes it, with no second download; else it starts a new one, queued.
  The event carries that download's id.
