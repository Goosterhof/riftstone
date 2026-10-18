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
| `get_overview` | — | `Overview` | The home frame polls nothing; it re-reads on events. Not free: it re-reads the game folder so the window sees changes the spine did not make. That read is shared by the calls a burst of events triggers, and is at most 250 ms old. **v0.3:** the first call is also the window's signal that it listens: the nxm links held at the door until then go through (startup step 9). So `src/composables/boot.ts` subscribes to every event BEFORE its first `get_overview` (`tests/wake.spec.ts` pins the order). |
| `rescan` | — | `void` | Starts a background scan; progress via `rift://scan-progress`, done via `rift://library-changed`. A second call while scanning is a no-op. **v0.3:** when the mods folder is there and nothing watches it (made or put back by hand while unwatched), it also arms the watcher (§ v0.3 The one folder). The scan's span includes the relink's content check (§ v0.3 Legacy data). |
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
| `open_mods_folder` | — | `void` | **v0.3.** Opens the mods folder in Explorer, Rust-side (the opener scope refuses folders from the webview). A missing folder is made first (empty). When it was missing, or nothing watches it (a scan that found the folder gone dropped its watcher, so a folder put back by hand counts), the watcher is armed afresh and a scan runs. |
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
| `get_nxm_handler` | — | `NxmHandlerView` | **v0.3.** Reads what Windows runs for nxm:// links (`HKCR\nxm\shell\open\command`): `ours` when it starts this `riftstone.exe`, `other` (with the program's file name) when it starts another program, `none`, or `unsupported` off Windows. While `ours`, `givesBackTo` names what turning it off gives the links back to: the command the user's key held when the stone claimed it (a schema-1 memory: the remembered previous command), else the machine's own handler (HKLM); never a program named `riftstone.exe`. `null` while the memory cannot be read: off is refused then. |
| `set_nxm_handler` | `on: boolean` | `NxmHandlerView` | **v0.3.** On, when the user's key does not point at this exe yet: reads the user's key as the claim will change it and writes that to `nxm-handler.json` (schema 2, § The nxm handler) FIRST, then writes `HKCU\Software\Classes\nxm` for this exe. Off, when the user's key still points at this exe: the key goes back exactly as the claim found it (removed whole only when the claim made it; a command that starts a `riftstone.exe` is not put back), and the machine's handler, if any, answers again where the user's key no longer does. A key another program took back is left alone. Off forgets the memory, except while the memory cannot be read or parsed: then the file stays exactly where it is, and while the user's key still starts this exe off is refused (`refused`, a sentence naming the file) and changes nothing. Off Windows: `refused`. |
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
  Info: how many installs this session took a new id; its detail reads "Each was known by an id
  the mods folder no longer answers to (an id from before v0.3, a folder or archive renamed), and
  was matched to the one option there that installs the same files: each at its game path, the
  same size, and the same fingerprint (its size and its first and last MiB) wherever the
  Riftstone recorded one. So it shows, updates and uninstalls as before. Nothing in the game
  folder changed."; § v0.3 Legacy data), `settingsUnreadable`, `nexusUnreadable`. The last two appear when a stored file exists but cannot be read; that
  session then never writes over it. (v0.3 retires `roots` and `rootsUnreadable`.)
- **`mods`** (v0.3), one of five states:
  - Error "The mods folder is missing", with no action: the folder was not there at launch and
    could not be made (the detail names the path and the reason, and says to make it by hand or
    install the stone where it may write, then start it again).
  - Warn "The mods folder is gone", action `openModsFolder`: it was made at launch and is not
    there now (moved or deleted while the stone ran). The detail says Open mods folder makes it
    again, empty, and watches it; then the player puts the mods back in it.
  - Warn "The mods folder could not be read", action `rescan`: it exists but cannot be listed.
  - Info "The mods folder is empty", action `openModsFolder`: it holds no package (files that are
    not packages do not count).
  - Absent when it holds packages.

  The page's own Mods folder row draws it (§ v0.3 The one folder says how the window answers
  each state).

**Stand-in option ids.** An adopted install that matched no library option carries a stand-in id
(v0.3: `adopted#<root half>`, the second half of v0.2's stand-in id, re-keyed when the journal is
lifted, § v0.3 Legacy data). It shows as `InstalledRow.optionId` with `packageMissing: true`.
`get_option` answers `entryNotFound` for it, and `plan_uninstall` works on it. The window treats
every option id as opaque and never parses it.

## Events

| Event | Payload | When |
|---|---|---|
| `rift://scan-progress` | `ScanProgressEvent` | After each package of a scan. **v0.3:** also while the scan confirms a relink by reading bytes (§ v0.3 Legacy data), for each confirmation it reads: `current` is "checking {package file name} against the installed files ({i} of {n})", n being the confirmations this pass reads; `done` and `total` stay as the scan left them. The window shows it as it shows any current ("now: …"). |
| `rift://library-changed` | `{}` | A scan finished (manual, startup, the library watcher, Open mods folder making the folder again or arming its watcher afresh, or an archive a download landed). The scan's span includes the relink's content check (startup step 5), so this comes after its reads. |
| `rift://install-progress` | `InstallProgressEvent` | During extraction. |
| `rift://game-changed` | `{}` | After install / uninstall / reorder / switch / game root change / a relink, and after `restore_parked` / `discard_parked`. **v0.3:** also when the game watcher sees DD2.exe start or stop. Never after a refusal. |
| `rift://updates-progress` | `UpdatesProgressEvent` | During `check_updates` / `identify_packages`. |
| `rift://updates-changed` | `{}` | After an update check or a link change. **v0.3:** also after an archive lands (the download links its package; a file that is no archive links nothing and sends nothing), after the account's standing changes (a validation, `set_nexus_key`), after `set_tracked` and `set_endorsement`, and after the migration moves a link record to its package's new id. |
| `rift://downloads-changed` | `{}` | **v0.3.** A download row appeared, changed phase or was cleared (`clear_downloads` sends it only when it removed a row). |
| `rift://download-progress` | `DownloadProgressEvent` | **v0.3.** While a download moves bytes: at most one per 200 ms per download, plus the last. |
| `rift://nxm-received` | `NxmReceivedEvent` | **v0.3.** An nxm:// link reached the stone (cold start or a second launch) and was handled; a link that waited at the door (startup step 9) is told when the door opens. A second launch also brings the window forward. `nxm://premium` sends `accepted: false` with `refusal: null`: nothing to say, while the account is validated again in the background. |

## Startup sequence (Rust side)

Steps 1–3 run in Tauri's `setup` (`AppState::open`), before the window opens, and are fast.
Steps 4–9 run in the background (`AppState::wake`), so the window shows at once. The spine's
comments cite these numbers.

0. (v0.3) Before any plugin, `riftstone.exe --give-back-nxm` (the uninstaller's, § The nxm
   handler) gives the nxm:// links back and exits: no window, no hand-over, no scan.
   `tauri-plugin-single-instance` is the first plugin: a second launch (Windows starting
   `riftstone.exe "nxm://…"`) hands its arguments to the running stone and exits. A link that
   arrives before the spine exists at all (before steps 1–3 have run) is dropped with a log line;
   the player presses the button again.
1. Load settings, the catalog cache, the Nexus cache and the stored key. Discover the game
   (`RIFTSTONE_GAME_ROOT`, else the folder the player picked, else Steam) and load its journal
   (lifting a schema-1 journal in memory, § v0.3 Legacy data). A stored file that exists but
   cannot be read starts that part from its defaults and is never written over that session.
   `roots.json` (v0.2) is never read, written or deleted. Empty `<cache>\relink-check` (parts a
   relink check left when a quit cut it short, § v0.3 Legacy data).
2. `recover()` a stale journal intent → `Overview.recovered`.
3. Resolve the mods folder (`RIFTSTONE_MODS_DIR`, else `<folder of riftstone.exe>\mods`) and make it
   if it is missing; remove the stone's own leftover partial downloads in it
   (`.riftstone-*.part`, nothing else). A folder that cannot be made leaves an empty library and
   the `mods` Error check, never a panic.
4. Show the window immediately with the cached catalog (the cached packages that sit directly in
   the mods folder); start a background rescan.
5. Arm a debounced (1.5 s) watcher on the mods folder: a change rescans; the window hears
   `rift://library-changed`. A scan that finds the folder gone drops the watcher; `rescan` and
   `open_mods_folder` arm it again once the folder is there (§ v0.3 The one folder). After every
   scan: the relink's content check (read with no lock held), then the folder-id re-key and the
   relink pass (§ v0.3 Legacy data). The scan's span includes the content check: while it reads,
   `rift://scan-progress` names each read, and `rift://library-changed` comes after it.
6. (v0.3) Start the game watcher: every 2 s it samples the process list; a start or a stop
   emits `rift://game-changed`.
7. (v0.3) At the wake, before the scan, on a thread of its own (`riftstone-account`): with a
   stored key, validate it (`/v1/users/validate.json`, no quota) → `NexusSummary.key`, `account`
   and the quota. It needs no library, so a long first scan never delays it (an nxm link's grant
   expires): `AppState::wake` starts it first, before the watchers and the scan.
8. If `autoCheckUpdates`, the last check is older than 12 h and at least one package is linked:
   `check_updates` in the background after the first scan. It is anonymous and runs beside
   step 7: it never waits on the key, its validation or its quota.
9. (v0.3) The link door. Every nxm:// link that arrives before BOTH the launch's validation has
   run (step 7, whatever it found) AND the window has read its first frame (its first
   `get_overview`) waits at a door: the links in the stone's own command line (a cold start) and
   a second launch's early links alike. When the second of the two happens, the held links are
   handled in the order they came, each checked against the validated account, and the window
   hears each `rift://nxm-received`. After that, a link is handled the moment it arrives.

## v0.3 — one folder, the tree, Play, and the key's extras

The investor's ask (2026-10-17): no other mod manager is needed any more; one `mods`
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
  moment of the read: it could not be made at launch, or it was moved or deleted since. The
  `mods` check tells the two apart (§ Check ids): no action when it could not be made, the action
  `openModsFolder` when it went.
- The window answers each state of the folder. In the Rift column, while no scan reads: a folder
  that could not be made carves THE RIFT HAS NO FLOOR (ember) with `Scan again`; a folder that
  went carves THE RIFT LOST ITS FLOOR (warn) with `Open mods folder`, which makes it again,
  empty, and the stone watches it (a scan never makes a folder); an empty folder carves THE RIFT
  IS QUIET with `Open mods folder`, `Scan` and `Find on Nexus`. PREFLIGHT's Mods folder row draws
  `Open mods folder` whenever the folder is there or the check's action asks for it, and then it
  is the row's Primary with `Scan` its Secondary (an empty or gone folder: opening is what
  helps); otherwise `Scan` is Primary (`Scan again` while the folder is not there). The row's
  count is ember only at the Error level.
- The watcher (startup step 5) watches the folder that was there when it was armed. A scan that
  finds the folder gone drops it. `rescan` arms it again when the folder is there and nothing
  watches it (a folder made by hand after a launch that could not make it, or put back after it
  went); `open_mods_folder` does the same, making the folder first when it is missing, and then
  scans. So after Open mods folder or Scan the stone sees new mods by itself again.
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
- Before it removes a file, the uninstaller gives the nxm:// links back: `NSIS_HOOK_PREUNINSTALL`
  (`src-tauri/windows/hooks.nsh`, wired in through `tauri.conf.json` `bundle > windows > nsis >
  installerHooks`) runs `riftstone.exe --give-back-nxm` and goes on whatever it answers. The stone
  then does what the switch's off does (§ The nxm handler); where it cannot put the user's key
  back (its memory cannot be read, or Windows refused the restore) while that key still starts
  this exe, it removes the key whole, because the exe the key starts is about to go. The hook
  is skipped when the uninstaller runs with `/UPDATE` (an updater's update puts the same
  `riftstone.exe` back in the same place; the stone has no updater). An upgrade (a newer version
  over an older one) runs the old version's uninstaller first when the player keeps the
  installer's default choice, so it gives the links back too: the player turns the switch on
  again after the upgrade. Reinstalling the same version keeps them by default (its default
  choice uninstalls nothing). (Tauri's NSIS template, read in tauri-bundler 2.10.3 on
  2026-10-18.) v0.2 never claimed the links, so an upgrade from it has nothing to give back.

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
- **The relink pass.** After each scan, an install whose package is not in the mods folder at all
  is a candidate (a package that is there but unreadable this scan, or that yielded no option,
  keeps its installs: a relink is permanent). It is matched to a library option when exactly one
  option fits, compared case-blind:
  - the same option root;
  - every recorded loose, script or root file present in the option's payload at the same game
    path and of the same size (where two entries target one path, the later one, as an install
    writes it);
  - the install's paks are the option's paks by size: as many, of the same sizes;
  - when the root is empty or only paks speak for the install, the same name as the option or its
    package.

  Sizes are not bytes, so the sizes only propose, and the content confirms: wherever the stone
  recorded a fingerprint for a file the install wrote (the engine's own: SHA-256 over the size and
  the first and last MiB), the candidate's entries must carry the same fingerprint (a loose, script
  or root file at its game path; a pak among the option's paks of its size). Any mismatch drops
  the proposal. An install whose files carry no fingerprint keeps the size rule; a package that
  cannot be read confirms nothing. So a newer version that kept every file's size (a recolor whose
  textures keep their size) takes over the older install only when every recorded file also kept
  its first and last MiB: a change that lies wholly between them, in a file over 2 MiB, is not
  seen (CLAUDE.md, Known Limitations). The match rewrites the
  install's option id everywhere in one save (refused while a change is pending; never onto an id
  the journal already holds, nor onto one two installs both fit), then `rift://game-changed`.
  Unmatched installs stay listed with `packageMissing: true` and uninstall from the journal as
  before. A `relinked` Info check reports how many installs this session took a new id, by either
  pass. The installs v0.2 took over carry fingerprints of the files they hold, so they relink
  only onto a package whose entries carry those same fingerprints.
- **When it runs.** After every scan, and after `set_game_root` switches the game. First, with no
  lock held, the content check: each proposal the fingerprints must confirm, and that has no verdict yet,
  opens its candidate package as an install would, unpacks the entries it names into
  `<cache>\relink-check` (emptied after each scan and at every launch; one reader at a time) and
  fingerprints them. At most 4 GiB per scan, counted by the sizes the packages list: a check that
  does not fit what is left waits for the next scan (each launch or mods-folder change), and one
  over 4 GiB on its own is never read, so its install keeps its id. Each verdict lasts for the
  session, until the package's size or time, or the install, changes. Then the re-key and the
  relink run under the deploy lock, never read, and never while a change is pending; a proposal
  still waiting for its verdict is left out. When a change holds the lock, they run as soon as it
  lets go. A link record that moves sends `rift://updates-changed`. After a scan, the content
  check runs inside the scan's span: the window keeps READING THE RIFT, each read sends
  `rift://scan-progress` with `current` "checking {package file name} against the installed
  files ({i} of {n})", n being the confirmations this pass reads (shown as any current is,
  "now: …"), and `rift://library-changed` follows the reads. A Scan pressed meanwhile is part of
  that scan (`rescan` is a no-op while one runs).

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
  `i` is a search, never the lookbook or Install…; those stay on Ctrl+L and the detail pane). ↑ on
  the first row returns to the well; ↓ from the well lands on the row the tree last held, else the
  first.
  Opening an answer from the flat list reveals that option's ancestors, so clearing the well shows
  it in the tree. The trouble fold stays above the tree, outside its role.
- These keys are this contract's rule, and the build follows it (`tests/tree.spec.ts` pins them).
  Design #00015 R16.6 was amended to the same keys on 2026-10-18; this file stays authoritative
  for behaviour (BUILD-SPEC § v0.3).

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
  R16.14: `Play anyway` starts the game, `Fix it` opens PREFLIGHT at the first warned row with the
  keyboard on that row's own button, else on the row; `Esc` backs out); with no warnings the game
  starts at once. Starting reads the plan once more and calls `launch_game` only while it still
  refuses nothing.
- While the world stirs (from a press until the watcher sees the game, or until 120 s pass, which
  ends with a warn toast), Play stays pressable (R16.13): a press reads the plan and calls
  `launch_game` again, and the wait starts over. Only a press whose `plan_launch` and
  `launch_game` are still on their way swallows the next one. The spine refuses `launch_game`
  as soon as its watcher believes the game runs. A plan read while the game ran is void once the
  overview says the game stopped, until it is read again, so Play never says "already running"
  for a game that is not.
- Running state: the spine's game watcher samples the process list every 2 s, and every 1 s for up
  to 120 s after Play (Windows; a seam in tests). Only a start or a stop changes what it believes,
  and each emits `rift://game-changed` once. `GameSummary.running` and `startedMs` come from it.
  While the game runs, the engine's existing rules stand: installs land and load at the next
  launch; pak uninstalls, reorders and switches are refused.

### The key's extras

- **Account.** A stored key is validated at launch (at the wake, before the scan, startup step 7;
  validate.json spends no quota), on `refresh_nexus_account` and on `nxm://premium`. The window shows the account's name and whether it
  is premium or a supporter; never the key, never the e-mail. The name reaches the window, never a
  log line. A key Nexus refused stays stored (`key: "rejected"`) until the player replaces or
  removes it. Every key change starts over: an answer that began under the old key is never
  applied, and the old key's standings are forgotten.
- **What the key unlocks** (all v1, all counted against the account's quota except validate):
  downloads (premium: in the stone; free: through the site's Mod Manager Download button and the
  nxm handler), tracking and untracking mods, endorsing and abstaining. Without a key all of the
  anonymous v0.2 features keep working, and browsing Nexus and reading a mod's files and patch notes
  need no key either (v2, anonymous).
- **No key.** THE FAR WORLD carves THE FAR WORLD IS SILENT, its body: "Add your Nexus Mods
  personal API key and the stone will show your account, and download, track and endorse mods
  for you. It stays on this PC. Update checks and browsing Nexus work without it."
- **The key's doors.** Every door the window opens for a missing or refused key leads to THE FAR
  WORLD's key form, and none sends the player to PREFLIGHT for the key: a mod's FILES without a
  key says "Add your Nexus key to download from the stone." with a small button `Add key…`; a
  `Download` held for want of a key gives the same sentence as its reason; a refused nxm link's
  toast offers the voice's own recovery label (`Add key…` or `Replace key…`), not `Open
  Preflight` (§ The nxm handler). Each door goes to THE FAR WORLD when the player is elsewhere,
  opens the ACCOUNT section's key form and focuses its password field (NexusPage's
  `replaceKey`). PREFLIGHT's own NEXUS KEY row (`Add` / `Replace`) is the same door, and so is
  a Find row failure's `Add key…` / `Replace key…` (a link Nexus refused for the key lands on
  the row): one composable door, `useFarWorld().openKeyForm()`.

### Downloads

- `start_download(modId, fileId, forPackageId)`: no key → `nexus`/`noKey`. A stored key not
  validated this session is validated first (refused → `keyRejected`; out of reach → `offline`).
  The file must be on the mod's list (anonymous v2), else `fileGone`. Premium → `queued`; free →
  `waiting` with `nexusPageUrl`. A download of the same mod and file already on its way (any phase
  but landed, failed or cancelled) comes back as it is: no second download. One download runs at
  a time, in order, on a worker of its own: a download never waits for an install or an update
  check, and neither waits for a download.
- The run: `linking` (the file's facts from anonymous v2 when the start did not read them; the
  archive's own name and its size in bytes from one v1 `files.json` read; v1
  `download_link.json`, with the nxm link's `key` and `expires` for a free account; the first
  mirror is Nexus's preferred one) → `downloading` (a separate HTTP client that never holds the
  API key, https only except loopback, an idle timeout instead of a total one, streaming to
  `<mods>\.riftstone-<id>.part` with the MD5 computed on the way) → `verifying` → `landed`. A
  cancel is honoured at every step until the landing commits.
- Verifying. The byte count must match Nexus's size (v2's, else v1 `files.json`'s
  `size_in_bytes`, never its kilobyte fields) and the server's `Content-Length`; a body that runs
  past either is stopped as soon as it does, and a body nobody sized is stopped past 32 GiB. Then
  the MD5 through the anonymous `fileHashes` lookup. A hit for this mod and file passes; hits for
  other files only fail the download (`notTheFile`). Where a stated size vouched for the bytes, a
  hash Nexus does not know yet, or a lookup that cannot be asked (Nexus out of reach, its 429
  cooldown, an answer the stone cannot use), lands the archive on that size. With no size stated
  anywhere, only a hash Nexus knows for this very file passes: an unknown hash, or a lookup that
  cannot answer, fails the download.
- Landing: the archive keeps Nexus's own file name (v1 `files.json` `file_name`, else the mirror
  URL's last path segment, else `Nexus mod <mod> file <file>`), cleaned for Windows: only the last
  path part; control characters, `<>:"|?*`, Unicode's invisible format characters (category Cf,
  such as the right-to-left override that makes `piz.exe` read `exe.zip`) and the line and
  paragraph separators become `_`; no leading or trailing dots or spaces; a reserved device name
  (`CON`, `PRN`, `AUX`, `NUL`, `CONIN$`, `CONOUT$`, `COM0`–`COM9`, `LPT0`–`LPT9` and their
  superscript-digit forms, with an extension or without) takes a leading `_`; at most 180 UTF-16
  units (what Windows counts), the extension kept and the leading `_` counted. A name without a
  zip, 7z or rar extension takes the one its first bytes show, so the scan sees the package. If a
  file of that name exists (compared case-blind): the same size and MD5 → `alreadyHad`, nothing
  written; else ` (2)`, ` (3)` … before the extension. The final name is never written over: it
  is taken by a hard link where the file system has them (a link fails on an existing name; the
  partial name then goes), elsewhere (FAT32, exFAT, some network shares) by a move that itself
  refuses an existing name: `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING` on Windows,
  `renameat2` with `RENAME_NOREPLACE` on the Linux bench, and where a file system cannot refuse
  in the move, a copy into a file made new (`create_new`). A name taken in between moves on to
  the next ` (n)`.
- After the landing: an archive is linked in `nexus.json` (`PackageId::for_archive(name, size)`)
  to the exact mod and file with source `download` (`rift://updates-changed`), and a scan runs
  (`rift://library-changed`) unless the landing was `alreadyHad`. A file that is no archive at all
  (a raw `.pak`, an `.exe`) lands under its own name, but the scan never lists it: its row carries
  no `packageId`, `nexus.json` gains no link, and no scan follows. The stone never installs a
  download by itself. The old archive of an updated package stays where it is: removing it is the
  player's act in Explorer.
- The window. A landed archive is told once, with `Show in the Rift`. A landed file that is no
  archive is told as being in the mods folder but no archive, so the Rift does not list it, with
  `Show in folder` (`revealPath` of `<mods>\<landedAs>`) in place of `Show in the Rift`: in the
  toast, on the update row and on the shelf alike. While a download of a package, or of the same
  mod and file, is unfinished (`waiting` through `verifying`), the update row's `Download
  v{version}` (`Get it on Nexus` for a free account) and that file's `Download` in FILES are held:
  `aria-disabled` with the reason `Downloading: see DOWNLOADS`, still focusable, and a press asks
  nothing (the spine would answer with the same row). A `Download` held for want of a key gives
  "Add your Nexus key to download from the stone." as its reason (§ The key's extras, The key's
  doors). A free account's start opens the file's
  page and says, by the nxm handler's state, that the page's Mod Manager Download button brings
  the file to the stone, or, while nothing hands that button to the stone, that the stone must
  answer it first, with `Answer Nexus's button` (the switch's act).
- Failure or cancel: the partial file is deleted, nothing else in the mods folder is touched. A
  failed row carries the failure the window voices (`failure`, a `nexus` error with its reason
  where Nexus said no). The CDN URL and the nxm `key` are never logged, never put in an error and
  never sent to the window.

### The nxm handler

- Off by default. Turning it on is the player's act on THE FAR WORLD (the investor's machine had no
  handler on 2026-10-17). On: before the registry is touched, the spine reads the user's key
  `HKCU\Software\Classes\nxm` as the claim will change it and writes that to `nxm-handler.json` in
  the app data folder: `{ schema: 2, previous, before: { exists, default, urlProtocol,
  commandDepth, command } }` (whether the key was there; its default and `URL Protocol` values,
  `null` where unset; how many of `shell`, `shell\open`, `shell\open\command` were there; the
  command key's default value as it was, a blank one kept; `previous` is that command when the
  links may go back to it). Then it writes the key: default `URL:NXM Protocol`, `URL Protocol` =
  "", `shell\open\command` = `"<riftstone.exe>" "%1"`. A memory left by an earlier claim is
  replaced: that claim is no longer in the registry. The installer never registers anything.
- Off, when the user's key still points at this exe, puts the key back exactly as the claim found
  it: removed whole only when the claim made it (the machine's handler, if any, answers again);
  otherwise its three values go back as they were (or are deleted where they were unset), the
  `shell\open\command` keys the claim made go while nothing else is in them, and everything the
  claim never wrote (another program's `DefaultIcon`, its other verbs, a `DelegateExecute`) stays.
  A command that starts a `riftstone.exe`, this one or one elsewhere (an install the player moved
  away from), is not put back: giving the links to it would hand them to a dead path. A schema-1
  memory (`{ schema: 1, previous }`) gives back as before: its previous command written back, else
  the key removed. A key another program took back is left alone. The memory is forgotten
  afterwards. While `nxm-handler.json` cannot be read or parsed, it is never set aside or removed
  (it is read in place, `persist::inspect_json`): with the user's key still starting this exe,
  off is refused with a sentence naming the file and changes nothing, and `givesBackTo` is null
  meanwhile; with the key another program's, off changes nothing and keeps the file.
- The uninstaller gives the links back too (§ v0.3 The one folder): `riftstone.exe
  --give-back-nxm` runs the off over the installed stone's app data folder (`RIFTSTONE_DATA_HOME`
  when set) with no window, and where it cannot finish while the user's key still starts this
  exe, it removes the key whole. It exits 0 when nothing is left pointing at the exe (or there is
  no registry), else 1; the uninstaller goes on either way.
- The switch, on THE FAR WORLD. While another program answers (`other`), its line reads "{other}
  answers it now. Turn this on and the stone remembers it, and gives it back when you turn this
  off." ({other} is the program's name, else "Another program".) Its state (ON / OFF) is a plain
  mono word: no border, padding or radius; ON in the Rift's cyan, OFF muted. The mock's
  `?mock=nxmother` scene has another program answer: `Wagonmaster.exe` (#00015 R16.24).
- An arriving link is untrusted input. A second launch hands it over through single-instance; a
  cold start's links come in the stone's own command line. Every early link waits for the key's
  validation and the window's first frame (startup step 9). Handling a link never waits on the
  network. The checks, in this order:
  1. It must parse as `nxm://dragonsdogma2/mods/<mod>/files/<file>?key=…&expires=…`, its ids in
     1..=2147483647 and its `key` printable ASCII without spaces, at most 512 characters (unknown
     parameters ignored). A `key` without `expires` or the reverse, a `user_id` that is not a
     positive number (a blank one included), or a `key`, `expires` or `user_id` given twice with
     two different values makes it unreadable; a blank `key` with a blank `expires` is no grant.
     Another game, a collection, or a link that cannot be read → `refused` with a sentence, and
     the event's ids are null.
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

  The window shows THE FAR WORLD for every event: an accepted link is told with the inscription
  THE FAR WORLD ANSWERS and `Show downloads`; a refusal is said in its own sentence with `Open on
  Nexus` when the event names a mod (the file's own page, `?tab=files&file_id=<id>&nmm=1`, where
  the site's button makes a fresh link; the mod's files when no file is named) and, when the key
  is the way out, the voice's own recovery label (`Add key…` or `Replace key…`), which opens THE
  FAR WORLD's key form (§ The key's extras, The key's doors).

## v0.4 — the Muster

The party of a save slot, read from the player's own saves and named from the player's own game
(BUILD-SPEC § v0.4 has the formats and the id rules). Read-only: no command here writes a save,
and none ever sends one. Types live in `src/types/rift.ts` (§ v0.4).

### Commands

| Command (invoke name) | Args (JS, camelCase) | Returns | Notes |
|---|---|---|---|
| `get_muster_slots` | — | `MusterSlotsView` | Every save slot that holds a party, newest first, in every Steam account's DD2 save folder the stone found. Each slot is read once to fill its summary, then cached by file size and modified time. A slot that cannot be read is still listed, with its `problem` and no summary. The game's system data (`data00-1.bin`) and the `SS1_*` files are no slots. A slot's id is its file name without `.bin` (`data009Slot`), and `@<account>` follows when the machine has more than one account with saves (`data009Slot@2`), so an id is unique. A save copied in from another account on this machine opens with that account's key. |
| `get_muster` | `slotId: string` | `MusterView` | The whole party of one slot. A slot that cannot be read rejects with a `save` error (below). Reading the game's names (the first call after a game update) takes about 2 s; later calls read the cache. |
| `open_saves_folder` | `slotId: string` | `void` | Opens the slot's save folder in Explorer, Rust-side. The window never sees the folder's path, because it holds the Steam account number. |

### Events

| Event | Payload | When |
|---|---|---|
| `rift://muster-changed` | `{}` | A slot file appeared, changed or went (the game saved), debounced 1 s; also when the game's names were rebuilt after a game update. The window re-reads the slot list and the open slot. |

### The `save` error

`RiftError` gains the kind `save`, with the detail `{ detail: <sentence>, reason: SaveReason }`.
Reasons:
- `otherAccount`: the slot opens with no account the stone found, so it is another Steam
  account's save, copied in.
- `beingWritten`: the slot is busy. Its signature disagrees, or it is cut short (an empty file
  included), twice 500 ms apart while the file changes, which is the game writing it. Or the
  file would not be read at all, twice 500 ms apart: another program holds it for a moment
  (the game, Steam Cloud, an antivirus). It is never remembered, so the next read tries again.
- `damaged`: the container or the stream is broken, or the same failure as above with the file
  unchanged. A file larger than any save is damaged without being read.
- `noParty`: the file opens but holds no party.
- `gone`: the slot's file is not there: it left the folder since the list was read.

The sentence names the slot by its file name and never by its path, because the path holds the
Steam account number.

### What a member carries

`MusterMember`:
- `memberKey`: 16 hex digits hashed from the save's own record id, stable for one pawn across
  re-reads (the window's THE RE-CARVING tells members apart by it). It is neither a name nor a
  Rift id.
- `role` (`arisen`, `mainPawn`, `hiredPawn`) and `name`, which is the player's own and private:
  shown, never logged.
- `level` and `experience`.
- `vocation` and `vocations`: each vocation with experience, highest first.
- `gains`: what the levels gave to health, stamina, strength, defence, magick, magick defence,
  weight and knockdown.
- `gear`: the eight slots in the game's order, main weapon, sub weapon, head, body, legs, cloak,
  then two rings, with the item or null.
- `augments` and `skills` for the current vocation, in the order the game keeps them, empty
  places left out.
- `pawn`: inclination and specialization, null for the Arisen.
- `creator`: a hired pawn's creator's level and vocation. Their name stays out of the view.

Every game-named thing is a `Named` (`{ id, name }`), with `name: null` when the game's text has
no entry for the id or the names could not be read. `MusterView.names` says which: `game` (read
from the found game), or `missing` with `namesNote`, a sentence on where the names live.
`MusterSlotsView.names` says the same for the slot list's vocations. `gains` are the integers
the save keeps; the game's unit for `weight` is not stated, so the window shows it as given.
