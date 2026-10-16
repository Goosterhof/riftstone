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
| `search` | `query: SearchQuery` | `SearchResult[]` | Synchronous, < 50 ms at the investor's scale. Every keystroke may call it (debounce 80 ms in the window). |
| `get_option` | `optionId` | `OptionDetail` | |
| `get_package` | `packageId` | `PackageDetail` | Includes the full entry list (thousands of rows is fine). |
| `get_thumbnail` | `optionId, size: "card" \| "full"` | `string \| null` | A `data:image/jpeg;base64,…` URL. `card` ≤ 360 px wide, `full` ≤ 1600 px. Cached on disk by package id + entry; first call per image may take ~100 ms (7z/rar longer). `null` when the option has no picture. |
| `plan_install` | `optionId` | `InstallPlanView` | Pure: nothing is written. |
| `install` | `optionId` | `InstallResultView` | Progress via `rift://install-progress`; `rift://game-changed` after. |
| `plan_uninstall` | `optionId` | `UninstallPlanView` | Pure. |
| `uninstall` | `optionId` | `UninstallResultView` | `rift://game-changed` after. |
| `get_installed` | — | `InstalledView` | |
| `reorder_paks` | `order: string[]` | `void` | Option ids, lowest priority first. Refused while DD2 runs. |
| `adopt_fluffy` | — | `AdoptResultView` | |
| `set_reframework_switch` | `switch: "looseFileLoader" \| "pakDirectory", on: boolean` | `void` | Refused while DD2 runs. |
| `add_root` | `path: string` | `LibraryRoot` | The window picks the folder with `@tauri-apps/plugin-dialog` `open({ directory: true })`. Triggers a rescan. |
| `remove_root` | `rootId: string` | `void` | Built-in roots (`fluffy`, `downloads`) can be removed too; they come back only via `add_root`. |
| `set_game_root` | `path: string` | `GameSummary` | Manual pick; validates `DD2.exe`. |
| `get_updates` | — | `UpdatesView` | Cached state; no network. |
| `check_updates` | — | `UpdatesView` | Network (anonymous Nexus v2). Progress via `rift://updates-progress`; `rift://updates-changed` after. |
| `identify_packages` | `packageIds: string[]` | `Md5Result[]` | Hashes the archives (progress `hashing`), one batched MD5 lookup. |
| `find_on_nexus` | `packageId` | `NexusFindings` | **v0.2.** Searches Nexus's Dragon's Dogma 2 mods by the package's name (see § Find on Nexus). Sends only on this call; never links. Errors: `entryNotFound`, `nexus` (offline, quota, a refused answer). |
| `link_package` | `packageId, urlOrId: string` | `UpdateRow` | Accepts a nexusmods.com/dragonsdogma2 URL or a bare mod id. |
| `unlink_package` | `packageId` | `void` | |
| `set_nexus_key` | `key: string \| null` | `NexusSummary` | Optional (v1 extras only). Stored in Windows Credential Manager; validated with `/v1/users/validate.json`. |
| `get_troubled_packages` | — | `PackageSummary[]` | The §9 trouble cards: every package carrying one of the four trouble kinds, including those with zero options — `unreadable` (can't be opened; ember rail), `nothingToInstall`, `outdatedFormats` (pre-TU 3.2 files), `filesSkipped`. Ordered by the first of those kinds a package carries, in that order, then library order. Packages with `outdatedFormats` or `filesSkipped` still install; a card speaks for its first trouble kind (`services/voice.ts` `troubleLine`), never for `problems[0]` blindly. |
| `get_nexus_quota` | — | `NexusQuota \| null` | The v1 budget from the last `X-RL-*` headers; `null` when no key is set or no v1 call has run (anonymous v2 carries no quota headers). |
| `get_labels` | — | `LabelFacets` | **v0.2.** The Nexus categories and tags of every linked package, from the latest update check (stored in `nexus.json`), with installable-option counts. The window refetches it after `rift://updates-changed` and `rift://library-changed`. |
| `restore_parked` | `store: string` | `ParkedRestoreReport` | **v0.2.** Puts a parked file back (rift-deploy `restore_parked`). Anything at the path is parked in its turn (`swappedOut`). Refused while an install or an un-adopted Fluffy mod owns the path, or for a handle the list no longer holds (the refusal names the game path, never the store handle, and says to look again at PARKED; the window reloads the list after any refusal). While the game runs it still puts the file back and answers `restartNeeded: true`; a file the game holds open fails the move, which is rolled back and comes back as an error, never as a half-restore. So `restored` is always `true` on success. Emits `rift://game-changed`. |
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
- **Kept like verdicts:** a check keeps the labels of every mod it did not ask about (a package whose
  root is out, say). A mod's labels are forgotten only when no package on the shelf and no
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
  `1080p`) stay. "Fluffy" goes only as the manager's name ("Fluffy Mod Manager", or "Fluffy" with no
  kept word after it), and so does "Dragon's Dogma 2", any apostrophe, with a "for" just before
  either. At most 8 words and 100 characters after all that. "Searchable" means at least one word
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
  (`pakDirectory`), `loose` (`looseFileLoader`), `fluffy` (`fluffyUnadopted`). The spine maps
  rift-deploy's names to these (`dto::WINDOW_CHECK_IDS`). rift-deploy's `faultyFiles` is never sent
  as a check, because the page draws that row from `reframework.faultyFiles`.
- **From rift-deploy, listed below:** `interrupted`, `paksLoaded`, `pakCount`, `pakNested`,
  `pakExtension`, `strayPakOrder`, `missingFiles`, `parkedBackups`, `strays`, `outdatedFormats`,
  `crashReporter`, `gameRunning`.
- **From the spine, listed below:** `journal`, `roots`, `unreadable`, `nothingToInstall`,
  `rootsUnreadable`, `settingsUnreadable`, `nexusUnreadable`. The last three appear when a stored file
  exists but cannot be read; that session then never writes over it.

**Stand-in option ids.** `fluffy#<section>` names a Fluffy install that no library option matched:
an adopted install's `InstalledRow.optionId` (with `packageMissing: true`), or an un-adopted one's
`LayerRef.optionId` in a conflict. `get_option` answers `entryNotFound` for these ids, and
`plan_uninstall` works on an adopted one. Exported as `FLUFFY_STAND_IN_PREFIX` in `rift.ts`.

## Events

| Event | Payload | When |
|---|---|---|
| `rift://scan-progress` | `ScanProgressEvent` | After each package of a scan. |
| `rift://library-changed` | `{}` | A scan finished (manual, startup, or the library watcher). |
| `rift://install-progress` | `InstallProgressEvent` | During extraction. |
| `rift://game-changed` | `{}` | After install / uninstall / reorder / adopt / switch / game root change, and after `restore_parked` / `discard_parked`. Never after a refusal. |
| `rift://updates-progress` | `UpdatesProgressEvent` | During `check_updates` / `identify_packages`. |
| `rift://updates-changed` | `{}` | After an update check or a link change. |

## Startup sequence (Rust side)

1. Load settings, the journal, the catalog cache, the Nexus cache from the app data dir.
2. `recover()` a stale journal intent → `Overview.recovered`.
3. Discover the game, REFramework status, Fluffy (and its ledger), the default roots
   (Fluffy's Mods folder when found, the user's Downloads as a DD2-shaped-only root).
4. Show the window immediately with the cached catalog; start a background rescan.
5. Arm a debounced (1.5 s) watcher on every root: a changed root rescans; the
   window hears `rift://library-changed`.
6. If `autoCheckUpdates` and the last check is older than 12 h: `check_updates` in the
   background after the first scan.
