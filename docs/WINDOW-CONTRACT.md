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
| `link_package` | `packageId, urlOrId: string` | `UpdateRow` | Accepts a nexusmods.com/dragonsdogma2 URL or a bare mod id. |
| `unlink_package` | `packageId` | `void` | |
| `set_nexus_key` | `key: string \| null` | `NexusSummary` | Optional (v1 extras only). Stored in Windows Credential Manager; validated with `/v1/users/validate.json`. |
| `get_troubled_packages` | — | `PackageSummary[]` | The §9 trouble cards: every package carrying one of the four trouble kinds, including those with zero options — `unreadable` (can't be opened; ember rail), `nothingToInstall`, `outdatedFormats` (pre-TU 3.2 files), `filesSkipped`. Ordered by the first of those kinds a package carries, in that order, then library order. Packages with `outdatedFormats` or `filesSkipped` still install; a card speaks for its first trouble kind (`services/voice.ts` `troubleLine`), never for `problems[0]` blindly. |
| `get_nexus_quota` | — | `NexusQuota \| null` | The v1 budget from the last `X-RL-*` headers; `null` when no key is set or no v1 call has run (anonymous v2 carries no quota headers). |
| `get_settings` | — | `Settings` | |
| `set_settings` | `settings: Settings` | `Settings` | |

Opening a URL or revealing a file is done window-side with
`@tauri-apps/plugin-opener` (`openUrl`, `revealItemInDir`) through
`src/services/rift.ts`.

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
| `rift://game-changed` | `{}` | After install / uninstall / reorder / adopt / switch / game root change. |
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
