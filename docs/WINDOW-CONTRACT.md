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

## v0.5 — versions of one file, and THE MUSTER by character

The investor's ask (2026-10-18), after installing an update the stone had downloaded: the library
and THE FAR WORLD showed the mod twice. Their rulings: installing a newer version replaces the
older install in one step, and the older archive is put away in a folder beside the mods folder,
from which it can come back; THE MUSTER opens by character, remembers the one picked, and offers
the character's other saves in a menu. Types live in `src/types/rift.ts` (§ v0.5, and the v0.5
fields on the v0.4 Muster types and on `Settings`). The design is #00015 §18.

### Commands

| Command (invoke name) | Args (JS, camelCase) | Returns | Notes |
|---|---|---|---|
| `plan_keep_newest` | `packageId: string` | `KeepNewestPlanView` | What keeping only the newest version of the package's file would uninstall and put away. Any member names the family. Nothing is written. |
| `keep_newest` | `packageId: string` | `KeepNewestResultView` | Uninstalls the older versions' installs, each by its own uninstall, then puts those versions away; one turn of the deploy lock. `rift://game-changed` when anything was uninstalled, a scan when anything moved. |
| `put_away` | `packageId: string` | `PutAwayView` | Moves one older version with no installs to the older-versions folder. A scan follows. |
| `retry_put_away` | `packageId: string` | `PutAwayResultView` | The put-away an install of the package makes, made again (the landing's `Try again`): every member older than it left without installs, the family read under one turn of the deploy lock. Moves what it can; `putAwayFailed` names what would not move. A scan when anything moved. |
| `get_older_versions` | — | `OlderShelfView` | What the older-versions folder holds. |
| `bring_back` | `fileName: string` | `BringBackResult` | Moves an item of the older-versions folder back into the mods folder. A scan follows. |
| `open_older_versions_folder` | — | `void` | Shows the older-versions folder in Explorer, Rust-side; refused before the first put-away makes it. |
| `join_versions` | `packageId: string, ofPackageId: string` | `void` | The player's word (ruling 10): the package is a version of `ofPackageId`'s file. Its link takes that file group, `source: "joined"`. `rift://updates-changed`. |
| `part_versions` | `packageId: string` | `void` | Takes a `joined` link back to the link it had before, and remembers the answer as a no. `rift://updates-changed`. |
| `decline_version_question` | `packageId: string, ofPackageId: string` | `void` | The player's no to `versionQuestion`: the question is not asked again for that file group. `rift://updates-changed`. |

`plan_install`, `install`, `get_library`, `get_updates`, `search`, `get_installed`,
`get_muster_slots` and `set_settings` keep their names and arguments; what v0.5 adds to their
answers is below.

### Versions of one file

**The family rule.** Two packages in the mods folder are versions of one Nexus file when both are
linked to the same mod and either both links name the same file group (Nexus gives every version
of one file the same group id; an update check or a download pins it), or one package's last
check named the other's exact file as its newer file. Packages tied that way, directly or through
each other, form a family. The newest leads. A check that named another member's file as newer is
the one proof of age the rule holds: the named member comes before the member that named it,
whatever its own link says (a link pasted as a file URL has a file id and no upload time).
Members whose proofs run in a circle (two checks at different times can name each other) are
placed together. Among the members no proof holds back, the one with the highest priority goes
next: the newest recency among itself and every member whose proofs reach it. So the newest
version leads, and a version that sits between a proven pair by upload time alone never leads over
them. Recency is the latest upload time, then the higher file id; a member whose link states
neither, and that no proof lifts, comes last. Ties fall to a member's own recency, then its file
name, so the order is the same however the mods folder lists the packages. A package with no link, or with an
unpinned link no check named, is in no family. Names and sizes alone never make a family.

**The same option.** An option of one version is the same option as an option of another when
both have the same option root (the part of the option id after `#`), else the same name in any
case, else when each version holds exactly one installable option. A version that renamed its top
folder (`Lantern Glow v1.1/`) still finds its older self. The spine works this out and says it
(`OlderVersionView.matches`, `InstallPlanView.replaces`); the window never works it out itself.

**One entry.** `LibraryPackage.family` is null for a package alone. `get_library` still lists
every package, older versions too, so a scan's answer stays whole; the window draws each family as
one entry: the lead, with its older versions inside it (`family.older`, on the lead only). An
older version's own tree opens from there. Each `family.older` entry lists `matches`: which of its
options is the same option as which of the lead's, and whether it is installed. An option of an
older version that `matches` does not list is one only that version holds.

`search` leaves an older version's answer out when the lead's same option answers the same search
too. An answer only an older version gives (a file only it holds, an option only it has) stays;
the window marks it as the older version's from the library (`family.lead` is another package).
`get_updates` lists one row per family (the lead's, with `older`) and no row for an older version,
and its counts count families. A package no check has judged yet (a version just downloaded) reads
`current` when a check named its exact file as another package's newer one, the older version's
check that led to the download, and its `checkedMs` is that check's. That holds after the older
version is put away, since its record stays in `nexus.json`, and the search card's badge agrees.

A conflict among versions of one file only (an older version still installed under a newer one,
as v0.4 stacked them) is not listed in `InstalledView.conflicts` and sets no `conflicted` flag:
that is an older version still installed, and `family.older[].installed` says so. A conflict that
also involves another mod is listed whole. A link or a check that ties or unties a family reads
the game folder's conflicts afresh.

**Install replaces.** `plan_install` of a family member's option also plans the uninstall of every
install of the same option in another member: `InstallPlanView.replaces`, each with its own
uninstall plan's file count and that version's name. `install` runs both in one journaled step:
the replaced installs' removals first, bottom layer first, then the writes. A write onto a path a
replaced install held reads `reinstall`, never an overwrite. A failure anywhere rolls the whole
step back, and the new install takes the first replaced install's place in the pak load order. An
install that replaces nothing works as before; so does a package in no family, which stacks as in
v0.4.

**Put away.** After an install of a family member commits, every member older than it that has no
installs left moves to the older-versions folder (`InstallResultView.putAway`), an older member
never installed too. The folder sits beside the mods folder, named for it:
`<mods folder> (older versions)`, so `…\mods (older versions)\` beside the stone's own `mods`. It
is outside the library: no scan reads it and no watcher wakes for it. The move:
- is the one change the stone makes to an entry of the mods folder (RS-2 as amended in v0.5);
- moves the whole archive or folder: never a copy left behind, never a delete, never an edit. It
  keeps the name, or takes ` (2)` … when the name is taken there (compared the way Windows
  compares names), and never lands on an existing file (the rules a download lands by, § v0.3);
- runs only after the install's step committed and before the scan the install requests, still
  under the deploy lock, so the relink never sees an older version leave while an install of it
  is still journaled.

A move that fails leaves the install standing and the file where it was; `putAwayFailed` says so
in one sentence that names the files and the reason, and the entry keeps showing the older
version. The landing's `Try again` calls `retry_put_away(packageId)` with the installed package:
the spine works out the set again and answers as the install did, what moved and what would not,
so a move that worked is never lost to a later one that failed. The window never works out the
set itself. A mods folder at the root of a drive has no folder beside it: every put-away there fails
that way. A member newer than the install (an older version installed on purpose) is never put
away, and an uninstall never puts anything away.

**Keep only the newest.** `plan_keep_newest(packageId)` and `keep_newest(packageId)` take any
member of a family; the plan's `packageId` is the family's lead. The version kept is the newest
member with an install, else the lead (`keeps`, its file name). Every install of a member older
than it is uninstalled, bottom layer first, each by its own uninstall under one turn of the deploy
lock, and then every older member left without installs is put away (`putsAway` names them). The
plan is `refused` when the package is the only version of its file, when no member is older than
the version kept, when the game folder or its journal cannot be read, or when an uninstall it
needs is refused (the game holds a pak open). `keep_newest` refuses the same way with a `refused`
error. An uninstall that fails stops it there: what it uninstalled stays uninstalled
(`rift://game-changed` follows), nothing is put away, and the error says why.

**Put one away.** `put_away(packageId)` moves an older member with no installs to the folder, at
the player's press, under the deploy lock; the family is read under it too, so a join taken back
since the press cannot let the newest version move. It is refused (a `refused` error) for a family's lead,
a package in no family, or a member with installs, and needs the journal (no game found, a
damaged journal: that error).

**The older-versions folder.** `get_older_versions()` lists what it holds, as a scan would list a
mods folder (archives and folders, by name, case-blind). `exists` is false until the first
put-away makes it. Each item carries its Nexus mod name and version from `nexus.json`, where its
record stays under its package id while it is away, and whether a newer version of the same file
is in the mods folder now (the family rule over the mods folder and that record). A move that
has to take another name carries the record to the id that name gives, both ways, and saves it
there BEFORE the move (v0.6, RS-A2), so the next launch finds it whatever happens after. It
writes over a leftover record under that name, and passes over a name whose id another archive
or folder in either folder already carries (ids go by name), so `landedAs` can skip a number.
`bring_back(fileName)` takes a name the folder lists (any case; anything else is `entryNotFound`,
never a path), moves it back into the mods folder by the same rules, never onto an existing name,
and returns `landedAs`. A scan follows (`rift://library-changed`), and the version is in its
family again. `open_older_versions_folder()` shows the folder in Explorer, Rust-side, like
`open_mods_folder`; before the folder exists it is refused.

**Errors.** A move or a read of the folder that cannot be made is an `io` error whose detail is
the OS's own words only (`Access is denied. (os error 5)`): the window voices it in the context
`olderVersions`, naming the file and what was tried itself. One detail is the stone's own (v0.6,
RS-A2): `its Nexus record could not be saved under the new name first`, when the record written
ahead of a renaming move could not reach `nexus.json`; nothing moved, and the same press can be
made again. `putAwayFailed` is a whole sentence
that names the files and the reason; the window shows it as it is and adds where the version
stays. No sentence names more of a path than a file's own name.

### The player's word (ruling 10, 2026-10-19)

The family rule needs Nexus's proof, and some older versions can never carry it: an unpacked
folder has no archive to look up by MD5, and a link by mod id (Find, a pasted mod page) names no
file group, so no check can pin it. The native probe of 0.5.0 met exactly that on the investor's
library: the older version of the very file that started v0.5 sat in the mods folder as a folder
linked by mod id, beside the pinned download, as two entries. So the player's word is proof too.
The stone asks; it never joins on its own.

**The question.** `LibraryPackage.versionQuestion` is set on a package when all of these hold:
- its link names a mod and no file group, and it is in no family;
- another package in the mods folder links to the same mod with a file group (the question's
  file group), and the player has not said no to that group for this package;
- the file names they hold, compared without their folders and in any case, and leaving out the
  catalog's own `modinfo.ini` and `screenshot.*`, have at least two in common, or both packages
  hold just the one same name and it is no settings or text file (`.json`, `.ini`, `.cfg`,
  `.toml`, `.yaml`, `.yml`, `.xml`, `.txt`, `.md`, `.log`); and one package's names are all among
  the other's. A single name in common beside others never asks: an optional file of the same
  mod that holds only a `config.json` its main file holds too is no version of it, and two
  packages that each hold only that `config.json` are two settings. A one-file mod's other
  version (one pak, one script) still asks.

When several file groups qualify, it asks about the one with the most names in common, then the
newest. `ofPackageId` is that group's newest package in the mods folder (its family's lead when it
has one), `sharedFiles` how many names they have in common and `files` how many this package
holds. A question is a hint, not a proof: the answer is the proof.

**Yes.** `join_versions(packageId, ofPackageId)` gives the package's link `ofPackageId`'s mod and
file group and `source: "joined"`; its file id, version and upload time stay as they were (none,
for a folder). The family rule then reads it unchanged: one entry, THE VERSIONS card, install
replaces, put away, keep only the newest. A joined version with no upload time and no file id
comes last in its family, so it never leads. A later update check keeps the joined group: a pin
adds the file's id, version and upload time, but a listed file with no group (v1 lists none) or
another group never parts the join; only *Take it back* does. It needs no question: any package whose link names a
mod and no file group may be joined to a package of the same mod that names one. Anything else is
`refused`: either package not in the mods folder, different mods, a package that already names a
file group, or `ofPackageId` naming none. An update check keeps the joined group unless it pins
the package to its own exact file, which then wins (Nexus's word over the player's).

**No, and taking it back.** `decline_version_question(packageId, ofPackageId)` remembers, in the
package's `nexus.json` record, that it is not a version of that file group, and the question goes.
`part_versions(packageId)` takes a `joined` link back to the link the package had before it was
joined (its record keeps that one), and counts as a no for that group. It is `refused` for a link
that is not `joined`, and takes a package in the mods folder only: a put-away version answers
`entryNotFound` (bring it back first). A put-away version that was joined keeps its joined link
in the older-versions folder, like any other record.

### THE MUSTER by character

**A character** is the Arisen with their main pawn, within one Steam account. The saves hold no
playthrough id. On the investor's five slots (2026-10-18), the Arisen's and the main pawn's record
ids were the same in every slot, across two different characters, and no other field of the
Arisen's record told the two apart except progress. So the stone tells a character by the
Arisen's name with the main pawn's name. Two playthroughs begun with the same two names read as
one character, and the slot menu tells their saves apart by level and time.

- `MusterSlot.character`: 16 lowercase hex digits, the FNV-1a 64 of the save folder's account id
  and the two names. The folder's account, not the one whose key opened the slot: a save copied
  in from another account is the character it becomes when the game saves it here again. It is
  made from private names, so it is private too: logged nowhere, in no DOM attribute, and used by
  the window only to group slots and to remember a choice. Null when the slot would not open.
- `MusterSlot.mainPawnName`: the main pawn's name, shown on the character's entry; null when the
  slot would not open or holds no main pawn.
- `Settings.musterCharacter`: the character picked last. The window writes it through
  `set_settings` when the player picks a character, and never on its own. THE MUSTER opens on
  that character at their newest slot, or on the character of the newest slot when it is null or
  no longer listed. The spine keeps it only as a character key: anything else, in the window's
  settings or in `settings.json`, reads as null. A window from before v0.5 sends no
  `musterCharacter`, which is null too.
- The window groups the slots by `character`, newest save first. A slot that would not open is
  listed apart, under its problem. The slot menu holds the character's slots, newest first.
  - **Following:** on the newest slot that opens (the newest, unless the game is writing it),
    the sheet follows the character. Picking that slot from the menu keeps following. When the game writes a
    newer save of that character, in any slot, the sheet moves to it and is re-carved against the
    party it showed (members matched by `memberKey`).
  - **Pinned:** a slot picked from the menu stays, and the menu marks the newer save.

### The Rift's library tools and filters, and answers in flight (window only)

Added on this branch by the Heretic (`43652f1`) and kept on v0.5 by the investor. These rows are
the window's alone: they add no command, event or type, and the two library actions call commands
§ v0.3 already defines.

- **The rooms say what they hold.** Each room in the pillar carries a plain line under its name:
  THE RIFT *Mod library*, YOUR WORLD *Installed mods & load order*, NEXUS *Browse, download &
  update*, PREFLIGHT *Game checks & settings*, THE MUSTER *Characters & saves*. The names and keys
  `1`–`5` are unchanged.
- **Clear search.** While the search well holds text, a `×` (`Clear search`) stands where the `/`
  key hint stood. It empties the query and puts the focus back in the well.
- **Library actions.** The Rift's answers, over a library that is there (not empty, the mods
  folder not missing), carry `Open mods folder` (`open_mods_folder`) and `Scan` (`rescan`), each
  busy while its command runs.
- **Active filters.** While a state other than *All*, *Show menu-only* or a Nexus label filters
  the answers, a line under the heading reads them, joined by ` · ` (*Update available ·
  Menu-only included · Armour, carrying Fashion*), with `Clear filters`. It resets the state to
  *All*, turns *Show menu-only* off and clears every category and tag; the typed query stays.
  With no query, the heading reads `{n} options · filtered library` (or the label reading
  instead of *filtered library* when labels filter).
- **Filters that hide everything.** When a state filter or *Show menu-only* leaves no answer, the
  Rift reads `NO MODS MATCH THESE FILTERS` instead of an empty list, with `Clear filters`
  (Primary). Under *Update available* it says only mods with a newer file identified on Nexus
  appear there and adds `Check on Nexus`. Nexus labels that leave no answer keep their own
  inscription and its recovery, `THE LABELS HIDE EVERY ANSWER`, ahead of this one.
- **The screenshot viewer keeps the keyboard.** While it is open, whatever holds the focus
  (a click on the picture keeps it inside), Esc and Space close it, Left and Right step through
  the pictures in the lookbook, Tab stays inside it, and the room keys never reach the app
  behind it.
- **An answer belongs to the selection that asked.** An option's detail, its plan and its landing
  are drawn only on the option they were asked for. An answer that comes back after the player
  opened another option never draws on that one. A landing that finished elsewhere is still told
  in its toast, and the option shows its new state when it is opened again. When the player
  returns to the same option before its install or uninstall finishes, the finish refreshes that
  option's current detail; it never brings the earlier plan or landing back. While this pane's install or
  uninstall (including Undo) is in flight for an option, returning to that option keeps its
  `Install…` or `Uninstall…` door busy. Another plan or confirmation for that option cannot
  start until the write and its detail refresh finish; success and failure both release it.
  Other options remain browsable. This holds for the option detail's own writes; YOUR WORLD's
  load-order list (`useInstalled`) returns its own way and is not part of it. THE VERSIONS card
  guards its landings by its card number (§ Versions of one file): an answer for a card the
  player left never draws on the card open now. Its own writes (*Keep only the newest*, *Put it
  away*, *Bring it back*) do not wait on a write the option detail has in flight. The spine takes
  every write one at a time under its deploy lock and works each one out again when its turn
  comes, so a keep-newest after an uninstall of the same version finds that install gone.

## v0.6 — THE FRAY, the party screen (in progress, branch `feat/fray`)

A second window, `fray` (`fray.html`, #00015 §19), for the player's second monitor: the party's
damage while a fight runs, the fight's record when it settles, and the last five fights. The
numbers come from the eye, a read-only REFramework script inside the game that writes every hit,
death, foe sample and heartbeat as one JSON line under `<game>\reframework\data\riftstone\probe\`.
`crates/rift-fray` reads those lines, credits each hit, cuts the session into fights and builds
the frame; `src-tauri/src/fray.rs` (the desk) follows the newest session. Read-only: nothing
here writes, and the autorun folder is never listed (the desk checks the eye's one file by its
exact path). Types live in `src/types/fray.ts`, mirrored by `crates/rift-fray/src/frame.rs`.

### Commands

| Command (invoke name) | Args (JS, camelCase) | Returns | Notes |
|---|---|---|---|
| `fray_frame` | — | `FrayFrame` | The window pulls it every 250 ms (no events: a push travels by webview eval, which WebView2 starves). Each pull reads the lines the eye wrote since the last one. `feed.state` is `live` while heartbeats arrive (one within 3 s, FQ.3's fallback), `stale` when the game runs and they stopped, `none` with no session, with the game closed, or after the eye's stop line (`eyeRecording: false`, `eyeReason: "stopped"`). The last fight and the shelf stay in the frame after the game closes. `party` carries the eye's last roster outside any fight (vitals, no damage; empty before the first roster), `fight.endedAtMs` the settle as epoch ms, and `feed.gameStartedMs` when the spine first saw this run of the game. Vocations and custom skills (`kind: "skill"`) are named from THE MUSTER's Codex, in the fight, on the shelf and in `party`; a skill id the Codex lacks keeps `name: null`. Members, foes, actions and shells travel as ids (`name: null`). |
| `open_fray` | — | `void` | With two monitors or more, THE FRAY takes one whole: a borderless window sized to exactly that monitor's bounds, not true fullscreen (the War Tent's Second Screen). The monitor is the one it took last, by the OS's name, else by place, else the last one; moved to another (Win+Shift+Arrow), it fills that one and remembers it (`settings.json` `frayDisplayName`, `frayDisplay`). With one monitor it opens as a plain window, so it never covers the game. Either way it is built visible and never takes focus, so the game keeps the keyboard: tao force-activates a window built fullscreen, and a window built hidden takes focus when shown. It is born on its monitor at a logical point chosen by tao's own placement rule (the first monitor holding the point in that monitor's scale), searched on a 17 × 17 grid of the target; on a desk where no grid point lands there it is born at Windows' default spot, which can be the game's screen, and fitted at once. When it is open, the same press settles its mode for the monitors there are now: a plain window fills a monitor plugged in since, and a borderless one turns plain with one monitor left. While borderless it never fills the only screen, even when Windows moves it there. |

### Credit, fights and the two totals

- **Credit** is the reviewed analyzer's `attackerLabel` (party-screen probe, `analyze.mjs` §4): an
  attacker resolved as `gimmick` is the world's; no attacker, or `none`, is unattributed (damage
  over time arrives this way until the debilitation's causer is read); otherwise the slot the eye
  wrote, or the slot of (address, CharacterID) under the line's roster generation, names a
  member, and any other character (an ally, a foe striking a foe) goes to `others`, kept apart
  and outside `hAll`. A hit credits its `dmg1` capped at the HP its victim had before it
  (`min(dmg1, hp0)`), the game's own ledger rule: on the first real session the party's raw
  `dmg1` summed to 96,058, the capped sum to 40,008, and TotalDamageFromPlayerParty to exactly
  40,008 (the HP read after each hit summed to 38,820, so it is not the rule). `dmg1` stands
  alone only when `hp0` was not read.
- **Keys**: the custom skill in use (`kind: "skill"`, its CustomSkillState id), else the shell's
  param id (`"shell <id>"`), else the action hash in hex (`kind: "action"`), else `unknown`.
- **A fight** opens at the party's first damage to a foe and gathers every hit on the foes the
  party struck. It settles after 8 game seconds without party damage while the game does not
  signal battle, after 30 whatever it signals, or at the eye's stop line. A settled fight lasts
  from the party's first hit to its last.
- **Cards are people**, known by CharacterID (else address): a member who changes slot or
  respawns keeps their card, one who leaves keeps what they dealt (`left: true`, back to false if
  they return), and a newcomer takes their own slot's place when free, else the first free one;
  with all four taken, their hits are unattributed.
- **`hParty`** is the four members' damage, **`hAll`** that plus the unattributed and the world;
  **`ledger`** sums, over the fight's foes, the rise of the game's own `TotalDamageFromPlayerParty`.
  The honesty line is hParty ÷ ledger.
- **`types`** is `additive` when 95% of the fight's readable hits split into their seven parts
  within max(1 HP, 1%) (the probe's G7 rule); then each member's `parts` sum to its damage
  exactly (the remainder on the largest part). Otherwise `mix`, with `parts: null`. A hit's parts
  are scaled by its credit over its roll before they are summed, so a capped hit's parts shrink
  with it (`parts` and `mix` alike).
- **`dps`** is the member's damage over the fight's `durationS`, `null` for a fight of no length.
  The window divides the party's head by the same rule and prints `—` where there is none.
- **The eye's lines** are read segment by segment in order; a segment the OS will not read stops
  that pull there, and the lines already read from earlier segments still reach the fight.
- **`attribution`** is `unqualified` until a witness check runs live; the probe's offline
  analysis (8.B) is still owed.

`fray-read <the eye's folder>` (the crate's binary) prints one session's fights, ids and numbers
only, for a recording read away from the window.

### What the window may not claim (PR #3's review)

- **Pulls that fail.** The last frame stands, but a `live` feed reads `stale` once no pull has
  answered for 3 s (the spine's own heartbeat deadline), its heartbeat aged by the wait.
- **The eye's stop line** (`eyeRecording: false`, `eyeReason: "stopped"`) is a finished recording:
  THE EYE HAS CLOSED, with no doors. Any other `eyeRecording: false` is THE EYE IS SHUT.
- **Browsing** a fight credited unchecked gives the doors' words to the legend: the doors read
  `◀` and `▶` (named Older fight and Newer fight), and the credit's note shortens to
  `≈ unchecked`, so every verdict fits beside the switch at 720 px. A step along the shelf keeps
  the keyboard on the door pressed.


## Manual save backups — investor request, 27 October 2026

`backup_saves(destination: string) -> SaveBackupReport` runs on the blocking pool.
THE MUSTER's **Back up all saves…** first asks for an existing destination folder;
cancelling the picker makes no call and writes nothing. The action covers every
found Steam account's whole `win64_save` folder, including system/creator files,
regardless of which character is currently selected or whether a slot parses.

The game-folder turn prevents Play launching through the stone during a backup.
The process watcher is checked before, during and after copying. A running game
refuses the action. The destination is canonicalized and must lie outside every
source's Steam Cloud subtree, game folder, mod library, its older-versions shelf
and Riftstone app data.
Sources containing links/special files are refused rather than followed.

A first inventory records each relative file's size, timestamp and SHA-256.
Original encrypted bytes are copied into an exclusively created staging folder,
read back and hashed; a second full source inventory must match. Failure removes
only this attempt's staging folder and never reports success. Publication uses
an atomic no-replace move to a fresh dated `DD2 saves …` directory, so an earlier
backup is never replaced. An interrupted process can leave a hidden staging
folder; it is incomplete and is never presented as a completed snapshot.

The backup contains account-N/win64_save files, manifest.json (local account
mapping, relative names, SHA-256 and file sizes) and RESTORE.txt. No parser, decryption
or save sealer runs. No original save, Steam Cloud setting or source metadata is
written. Restoration and scheduled backups are outside this change.

`SaveBackupReport` is `{path, folderName, createdAt, accounts, files, bytes}`.
Only the player-chosen destination path is returned; source paths and account
ids never cross the bridge or appear in errors/logs. THE MUSTER reports verified
file/account counts and offers **Show backup** using the existing reveal action.
A refused/failed attempt keeps an actionable inline error; repeated presses are
held until the picker and backup finish. This is RS-8's explicit backup exception.

## v0.7 — THE WORKSHOP (investor build only)

A sixth room, `THE WORKSHOP` (key `6`), where the investor works with **Tamsin**: a Claude Code
session the stone launches in WSL inside its fence (BUILD-SPEC § v0.7, RS-9), working in
`<mods folder> (workshop)`. Everything in this section exists only in a build with the `pawn`
feature and `VITE_RIFT_PAWN=1` (RS-10). In the public build none of these commands is
registered, none of these events is emitted, and the room, its nav row and its key do not exist.
Types live in `src/types/workshop.ts`. Design: #00015 §20.

### Commands

| Command (invoke name) | Args (JS, camelCase) | Returns | Notes |
|---|---|---|---|
| `get_workshop` | — | `WorkshopView` | The folder (made or not yet), the built artifacts with the stone's digests, the pawn's state, the fence and the voice. The fence is judged against the ground now, which costs one `wsl.exe` survey; the window calls it on entering the room and on `rift://workshop-changed`. |
| `summon_pawn` | `cols: number, rows: number` | `void` | Makes the workshop folder, the mailbox and Tamsin's config dir on first use, writes the relay, settings and persona for the current voice, and starts the PTY. Refuses `pawn` `alreadyHere`, `breached` or `noFence` (the missing piece named: the WSL distro, `bwrap`, `socat`, or `claude` in that distro). Output arrives as `rift://pawn-output`. |
| `pawn_write` | `runId: string, data: string` | `void` | Keystrokes and pastes, UTF-8, to the PTY of the summoning `runId`. Refuses `notHere` when no pawn is here or `runId` is not the live summoning, so a key typed for one run never reaches the next. |
| `pawn_resize` | `cols: number, rows: number` | `void` | Called on fit; ignored when no pawn is here. |
| `depart_pawn` | — | `void` | Ends the session: SIGTERM to the fence, then a kill after 3 s. A held Play is dropped. The pawn's installs stay (see `plan_put_back`). |
| `answer_pawn_play` | `runId: string, heldRequestId: string, accept: boolean` | `{ decision: "played" \| "declined", persisted: boolean }` | The held Play request the window was shown, by its run and id. `true` runs `plan_launch` and `launch_game` exactly as the Play slab does, with the same warnings and refusals. `played` is recorded only once `launch_game` has started the launch: `DD2.exe` started, or Steam accepted the request. Whether the game then came up is the game watcher's to say, never this decision's (the Heretic's WD6). `persisted: false` says the decision stands but did not reach the run's ledger on disk; the window says so (WD5). A refusal, a failure or a cancel at a warning records nothing, and the request stays waiting with the refusal shown. `false` records `declined`. The decision reaches Tamsin through her next `list_installed` or a new `play` call (BUILD-SPEC § v0.7, *The held Play's decision*), and the held call's own answer stays `held`. Refuses `nothingHeld`, and `staleHeld` when the run and id are not the live run's waiting request, both when the press arrives and right before the game starts. |
| `get_fence` | — | `FenceView` | The last probe record judged against the ground now: the host surveyed again and the plan a summon would get (BUILD-SPEC § v0.7, the probe). |
| `run_fence_probe` | — | `FenceView` | The deterministic probe (BUILD-SPEC § v0.7, no model, about 10 s). Refuses `pawn` `pawnHere` while a pawn is summoned. |
| `plan_bring_through` | `artifact: string` | `BringThroughPlan` | `artifact` is a file name in the workshop's `dist\`. The plan carries the digest **frozen now**. That digest is what the press refers to. |
| `bring_through` | `artifact: string, sha256: string` | `BringThroughResult` | RS-2's second exception: copy to a partial, hash the copy, land only on equality, never over a file. Rejects `bring` (below). |
| `plan_put_back` | `runId: string` | `PutBackPlan` | The Gift, narrowed (BUILD-SPEC § v0.7, the run ledger). `remove`: that run's own installs still installed and unchanged, newest first, as THE RETURN's plans. `review`: what that run uninstalled that was not its own, each with its Undo when the journal still allows it. It never claims to restore the world as it was. |
| `put_back` | `runId: string, installIds: string[]` | `UninstallResultView[]` | Uninstalls exactly the `remove` items named, newest first, each through the journal. An id that is not that run's own still-installed install is refused, not skipped. `review` items are never acted on here. |
| `open_workshop_folder` | — | `void` | Explorer, Rust-side. |
| `set_pawn_voice` | `voice: "full" \| "sparing" \| "plain"` | `void` | Saved in `pawn.json`; takes effect at the next summon (the window says so while a pawn is here). |
| `set_pawn_inputs` | `inputs: string[]` | `void` | Saved in `pawn.json` after each folder passes the protected-root check (below); refuses `pawn` `inputRefused`, naming the root, and saves nothing. |

The pawn's own settings live in the stone's app data as `pawn.json` (the voice, the inputs, the
distro), never in `get_settings` / `set_settings`, so the public `Settings` carries nothing of the
pawn (RS-10; ruled with the Heretic's review of PR #9). `set_pawn_inputs(inputs: string[])` sets the
Windows folders mounted read-only at `/inputs/<n>` (for example a pinned vanilla extract), and
`get_workshop` returns them. On set, and again at every summon, the stone
resolves each folder's final path and refuses any that **overlaps a protected root in either
direction** (BUILD-SPEC § v0.7, RS-9.2): Steam, the game, the mods folder and its siblings, the
stone's app data, the investor's profile and `.claude`, and every registered save-backup folder
(BUILD-SPEC § v0.7, RS-9.2: registered before each backup, read strictly). The refusal
is `pawn` `inputRefused`, naming the root it overlaps, never a path under `userdata`.

### Events

| Event | Payload | When |
|---|---|---|
| `rift://pawn-output` | `{ runId: string, seq: number, data: string }` (base64 bytes) | Each read of the PTY. `seq` starts at 0 for each summoning and rises by one; the window drops nothing of the run on show and writes it in order. The run is announced (`summoning`) before its PTY can say a word; a run's bytes that still arrive before its state wait for it, and an earlier run's late bytes are dropped. |
| `rift://pawn-state` | `PawnStateView` | Summoning, here (first bytes), departed (clean, exit 0), torn (non-zero exit or PTY closed), and after every `rift://pawn-hands` row, since a call can hold a Play or drop one. The window reads `held` from this state alone; only this event moves the run on show. |
| `rift://pawn-hands` | `PawnHand` | Every tool call the stone answered, refused calls included. |
| `rift://workshop-changed` | `{}` | The workshop's `dist\` changed (debounced 500 ms). The window refetches `get_workshop`; a digest in flight shows as `hashing`. |

### Errors

`RiftError` gains two kinds.
- `pawn`, with `{ detail, reason }`, `reason` ∈ `noFence` (detail names the missing piece and the
  command that installs it, e.g. `sudo apt install socat`), `breached`, `alreadyHere`, `notHere`,
  `pawnHere`, `nothingHeld`, `staleHeld` (an `answer_pawn_play` for a run or request that is not
  the one waiting now), `inputRefused`, `notThisRun` (a `put_back` id that is not the run's
  own still-installed install).
- `bring`, with `{ detail, reason }`, `reason` ∈ `nameTaken` (a different file has the name),
  `bytesChanged` (detail carries both digests' first 16 hex), `noReceipt`, `receiptMismatch` (the
  receipt names other bytes), `notInDist`.

No detail carries a path under `userdata`, the saves or the investor's home. Workshop paths are
the investor's own and may be shown.

### Types (`src/types/workshop.ts`)

- `WorkshopView`: `{ folder: { made: boolean, path: string }, artifacts: ArtifactView[],
  pawn: PawnStateView, fence: FenceView, voice: Voice, inputs: string[] }`.
- `ArtifactView`: `{ name, bytes, sha256: string | null` (null while hashing), `receipt:
  ReceiptView | null, receiptProblem: "missing" | "unreadable" | "mismatch" | null, inLibrary:
  boolean }`. `inLibrary` is true when a file with these exact bytes is in the mods folder.
- `ReceiptView`: the receipt's own fields (BUILD-SPEC § v0.7), passed through as the pawn wrote
  them, for display under `THE RECEIPT SAYS`.
- `PawnStateView`: `{ runId: string | null, state: "absent" | "summoning" | "here" | "departed" |
  "torn", since: string | null, exitCode: number | null, host: { distro, claudeVersion } | null, held: { heldRequestId, since } | null }`.
  `held` names the waiting Play request, so the window's press can carry it with `runId`.
- `PawnHand`: `{ runId, seq, at, tool, outcome: "done" | "refused" | "held" | "declined" |
  "repeated" (a replayed id answered from the ledger) | "unknown" (made but not recorded: the
  ledger could not be written), summary,
  installId: string | null, refusal: string | null }`. `summary` is the row's text
  (`installed Stature 0.2.0 · 7 files`). `refusal` is the stone's own sentence, verbatim.
- `FenceView`: `{ chip: "contained" | "unprobed" | "breached", control: boolean | null, ground: {
  distro, claudeVersion, kernel, digest } | null, provedAt: string | null, steps: FenceStep[],
  changed: string | null }`. `control` is whether the record's control landed (null without a
  record); the window shows it first, before any refusal.
  `changed` is the sentence for a record that does not make the chip `contained`: one proved on
  another ground (`proved on claude 2.1.312; this is 2.1.320`), one that could not show a row
  (`the last probe could not show step 3`), one from an earlier probe, or a host the stone could
  not check now.
  `FenceStep` is `{ step, expected: "land" | "notLand" | "fail" | "succeed", observed, ok,
  verdict: "held" | "breached" | "inconclusive" }`. `steps` lists rows 1–12, then the broker
  checks `B1` and `B2`. `ok` is `verdict === "held"`.
- `BringThroughPlan`: `{ artifact, bytes, sha256, landsAs: string, receipt: ReceiptView,
  conflict: "none" | "alreadyHad" | "nameTaken" }`. A build without a readable, matching receipt
  has no plan (`bring` `noReceipt` / `receiptMismatch`), so the plan always carries one.
- `BringThroughResult`: `{ outcome: "landed" | "alreadyHad" }`. The landed zip is named as a
  package by the scan the press requests; the window reads it from `rift://library-changed`.
- `PutBackPlan`: `{ runId, remove: UninstallPlanView[], review: { installId, name, at, undo:
  "available" | "reinstallFromRift", known: "yours" | "anotherSummoning" | "uncertain" }[],
  sentence }`. The journal keys an install by its option, so a `remove` item's `optionId` is the
  install id `put_back` takes. `known` is what the ledger can say: done and the player's, done and another
  summoning's, or `uncertain` (an outcome or owner that was not recorded; the Heretic's WB5).
  `sentence` is the press's own promise and counts each kind in its own words, e.g. *Removes the 3
  installs Tamsin made in this summoning. She also uninstalled 1 of yours. The stone cannot tell
  whether 1 more was uninstalled, or whose it was. They are listed below.*
- `Voice`: `"full" | "sparing" | "plain"` (default `sparing`).

### The room as built (#00015 §20, rendered at 1320 and 1024)

- The window side lives in `src/workshop/`, imported only by `src/composables/useRooms.ts` under
  `VITE_RIFT_PAWN === "1"`. The public window holds the registry and nothing else.
- The nav row's state (R20.1) takes the hint's line, `Tamsin is here` (rift) or `Tamsin asks to
  play` (warn), instead of a `· HERE` / `· ASKS` suffix after the name. The suffix wrapped the
  row onto two lines at the 248 px pillar.
- The terminal at 14 px measured 73 columns at 1320 and 74 at 1024 in headless Chromium, under
  Q20.3's prediction of about 80. The bench stays 320 until WebView2 is read on the installed
  build, which Q20.3 names as the settling read.
- The fence fold shows the control first, then rows 1–12 and B1/B2 by their verdict: `✓` held,
  `✕` breached (ember), `?` could not show (faint).
- The terminal takes input from `summoning` on, because xterm's `disableStdin` drops every
  reply, including its answer to ConPTY's first cursor query, and ConPTY holds all later output
  behind that answer. What the terminal says before the run is `here` (the only state in which
  the stone accepts `pawn_write`) is held, sent in order once the run is here, and dropped if
  the run never arrives. Each send names its run and is checked again before it goes, so a drain
  under way stops when another run is on show.
- A Play press answers the request that was waiting at the click: if another took its place
  while §16.2's plan was read, nothing is answered and the room says so. The decision reads
  `The stone started the launch.`; whether the game runs is the watcher's line, apart.
- A bring-through reports `landed` as soon as the stone does. The rescan it requests runs on,
  and its result arrives as `rift://library-changed`.
- No per-row Undo and no log viewer in the hands: the contract carries neither. Put back is
  the undo. `Resume the last conversation` (Q20.8) is not offered, since it is unmeasured.
- **The player's hands on the terminal (0.7.1, `src/workshop/hollowHands.ts`).** The installed
  0.7.0 could not sign Tamsin in: Ctrl+C reached her as `^C`, Ctrl+V as `^V`, and her sign-in
  link was only text. Now Ctrl+C with a selection copies it and sends nothing (with none it
  still interrupts her), and Ctrl+Shift+C always copies. Ctrl+V, Ctrl+Shift+V and Shift+Insert
  go to the webview's own paste, which xterm reads and sends as one paste. Ctrl+click opens an
  `https` link, a plain one or an OSC 8 one, in the player's browser through the opener, with
  `Opened <host> in your browser.`; a link the stone cannot open goes on the clipboard instead.
  Claude Code 2.1.312 prints its sign-in URL as one OSC 8 link (465 characters, measured in a
  PTY on 2026-10-29) over text it hard-wraps at the width, so a selection of it copies broken
  lines and the link handler is the way to it.

### What the window may not do

It never writes the workshop, never starts the PTY itself and never sees the fence's argv. It
never offers bring-through without a plan opened in this session, and it never shows a chip
the stone did not send. The terminal stays mounted while the investor visits other rooms, and
`rift://pawn-output` keeps writing into it.

It never opens anything from her screen but an `https` link the player Ctrl+clicked, and never
a plain click. It does not honour OSC 52: her `c to copy` writes nothing, because honouring it
would let her set the player's clipboard without a press of theirs.
