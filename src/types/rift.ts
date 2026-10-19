// The window contract — every shape that crosses the Tauri bridge.
//
// The Rust side (src-tauri/src/dto.rs) mirrors these types field for field
// with `#[serde(rename_all = "camelCase")]`. Change one, change both, in the
// same commit. Command names and semantics: docs/WINDOW-CONTRACT.md.

// ---------------------------------------------------------------------------
// Shared atoms (mirrors of rift-core)
// ---------------------------------------------------------------------------

export type PackageFormat = "zip" | "sevenZip" | "rar" | "folder";
export type DeployKind = "pak" | "loose" | "reframework" | "root";

export interface PayloadKinds {
  pak: number;
  loose: number;
  reframework: number;
  root: number;
}

/** v0.3: the one library. Every zip, 7z, rar or folder directly inside it is
 *  one package. It sits beside the installed riftstone.exe (`<install dir>\mods`);
 *  a bench may point it elsewhere with RIFTSTONE_MODS_DIR. The spine creates it
 *  at launch, and again when Open mods folder (open_mods_folder) finds it
 *  missing. The stone adds downloads to it and never changes, renames or
 *  deletes what is already there (RS-2). */
export interface ModsFolderView {
  path: string;
  /** False whenever the folder is not there at the moment of the read (it
   *  could not be made at launch, or it was moved or deleted since). The
   *  `mods` check says which: an Error with no action, or a Warn whose action
   *  (openModsFolder) makes it again. */
  exists: boolean;
}

export type ProblemKind =
  | "unreadable"
  | "nothingToInstall"
  | "filesSkipped"
  | "missingParent"
  | "missingRequirement"
  | "modinfoGarbled"
  | "outdatedFormats";

export interface Problem {
  kind: ProblemKind;
  message: string;
}

export interface ModInfo {
  name: string | null;
  description: string | null;
  author: string | null;
  version: string | null;
  category: string | null;
  screenshot: string | null;
  homepage: string | null;
  addonFor: string | null;
  nameAsBundle: string | null;
  requires: string[];
  dummy: boolean;
  menuPriority: number;
  extra: Record<string, string>;
}

export type DeployTarget =
  | { kind: "pak"; fileName: string }
  | { kind: "loose"; gamePath: string }
  | { kind: "reframework"; gamePath: string }
  | { kind: "root"; gamePath: string };

export interface PayloadFile {
  entry: string;
  target: DeployTarget;
  size: number;
}

export type SkipReason = "modinfo" | "picture" | "document" | "unknownPlace";

export interface SkippedFile {
  entry: string;
  reason: SkipReason;
}

export interface ModOption {
  id: string;
  packageId: string;
  root: string;
  name: string;
  info: ModInfo | null;
  screenshot: string | null;
  payload: PayloadFile[];
  skipped: SkippedFile[];
  kinds: PayloadKinds;
  parent: string | null;
  addonFor: string | null;
  bundle: string | null;
  menuOnly: boolean;
  trail: string[];
}

/** v0.3: "download" — the stone fetched this exact file from Nexus itself. v0.5: "joined" — the
 *  player's word that the package is a version of a file another package in the mods folder is
 *  pinned to (WINDOW-CONTRACT § v0.5, The player's word). */
export type LinkSource = "filename" | "homepage" | "md5" | "manual" | "download" | "joined";

export interface NexusLink {
  modId: number;
  fileId: number | null;
  groupId: number | null;
  sqid: string | null;
  version: string | null;
  uploadedMs: number | null;
  source: LinkSource;
}

export interface ArchiveEntry {
  path: string;
  size: number;
}

/** Every RiftError rejects an invoke with this shape. */
export interface RiftError {
  kind:
    | "unreadable"
    | "unsupportedFormat"
    | "encrypted"
    | "corruptArchive"
    | "unsafeEntryPath"
    | "entryNotFound"
    | "gameNotFound"
    | "refused"
    | "journal"
    | "nexus"
    | "save"
    | "io";
  /** For kind "nexus" (v0.3): `{ detail: <sentence>, reason?: NexusReason }`.
   *  For kind "save" (v0.4): `{ detail: <sentence>, reason: SaveReason }`.
   *  The window voices by `reason` first and reads the sentence only as a fallback. */
  detail?: Record<string, string>;
}

/** v0.3: why Nexus said no, as a code the window can voice without reading
 *  the sentence. Absent on a plain "nexus" failure (an odd answer). */
export type NexusReason =
  | "offline"
  | "rateLimited"
  | "noKey"
  | "keyRejected"
  /** download_link.json without a site link: only premium accounts may. */
  | "premiumRequired"
  /** The site's Mod Manager Download link has expired (HTTP 410). */
  | "linkExpired"
  /** The link was made for another account than the stored key's. */
  | "linkNotYours"
  /** Nexus refused the link's key/expiry for this file (HTTP 400). */
  | "linkRefused"
  /** The file or mod is gone from Nexus (HTTP 404). */
  | "fileGone"
  /** The bytes that arrived are not the size Nexus stated. */
  | "sizeMismatch"
  /** The bytes that arrived hash to another file than the one asked for. */
  | "notTheFile"
  /** Endorsing: this account has not downloaded the mod from Nexus. */
  | "endorseNotDownloaded"
  /** Endorsing: Nexus wants 15 minutes between a download and an endorsement. */
  | "endorseTooSoon"
  /** Endorsing: authors cannot endorse their own mods. */
  | "endorseOwnMod";

// ---------------------------------------------------------------------------
// get_overview — the home frame
// ---------------------------------------------------------------------------

/** v0.3: how Play starts the game. "steam" opens steam://rungameid/2054970
 *  (the game was found through Steam); "exe" starts DD2.exe in the game folder
 *  (a folder picked by hand or set by RIFTSTONE_GAME_ROOT). */
export type LaunchRoute = "steam" | "exe";

export interface GameSummary {
  root: string;
  buildId: string | null;
  source: "steam" | "env" | "manual";
  /** DD2.exe is in the process list. v0.3: kept current by the spine's game
   *  watcher, which emits rift://game-changed when the game starts or stops. */
  running: boolean;
  /** v0.3: when the stone first saw this run of the game (ms since the epoch);
   *  null while the game is not running. */
  startedMs: number | null;
  /** v0.3: the route Play takes. */
  launch: LaunchRoute;
  /** The build recorded at the newest install differs from the build now. */
  buildChanged: { recorded: string; now: string } | null;
}

export interface ReframeworkSummary {
  installed: boolean;
  version: string | null;
  buildDate: string | null;
  looseFileLoader: boolean | null;
  pakDirectory: boolean | null;
  faultyFiles: string[];
}

export interface LibraryStats {
  packages: number;
  options: number;
  entries: number;
  installedOptions: number;
  updatesAvailable: number;
  conflicts: number;
  lastScanMs: number | null;
}

export type ScanState = { state: "idle" } | { state: "scanning"; done: number; total: number; current: string };

export type CheckLevel = "ok" | "info" | "warn" | "error";

export type CheckAction =
  | { kind: "enableSwitch"; switch: "looseFileLoader" | "pakDirectory" }
  | { kind: "pickGameFolder" }
  | { kind: "openUrl"; url: string }
  | { kind: "rescan" }
  /** v0.2: open the parked-backup review (InstalledView.parked). */
  | { kind: "reviewParked" }
  /** v0.3: open the mods folder in Explorer (open_mods_folder). */
  | { kind: "openModsFolder" };

export interface Check {
  /** Stable id (WINDOW-CONTRACT § Check ids). PreflightPage draws `game`,
   * `build`, `reframework`, `pakdir`, `loose` and `mods` as rows of its own
   * (OWN_CHECKS) and lists every other check below them; a spine test reads
   * OWN_CHECKS from PreflightPage.vue, so the two cannot drift apart. */
  id: string;
  level: CheckLevel;
  title: string;
  detail: string;
  action: CheckAction | null;
}

/** v0.3: who the stored key belongs to, from /v1/users/validate.json (which
 *  spends no quota). The name is private: never logged, never in the repo;
 *  mocks and screenshots use an invented one. Never the key, never the e-mail. */
export interface NexusAccountView {
  name: string;
  /** Premium accounts download in the stone; free accounts go through the
   *  site's Mod Manager Download button (an nxm:// link) — Nexus's rule. */
  premium: boolean;
  supporter: boolean;
  /** When the key was last validated, ms since the epoch. */
  checkedMs: number;
}

/** v0.3: the stored key's standing.
 *  - "none": no key; everything anonymous still works.
 *  - "connected": validated this session; `account` is set.
 *  - "unchecked": a key is stored but not validated this session yet (the
 *    launch validation is pending, or Nexus was out of reach).
 *  - "rejected": Nexus refused the stored key at its last validation. */
export type NexusKeyStanding = "none" | "connected" | "unchecked" | "rejected";

export interface NexusSummary {
  lastCheckMs: number | null;
  checking: boolean;
  linked: number;
  unlinked: number;
  updates: number;
  hasApiKey: boolean;
  /** v0.3 */
  key: NexusKeyStanding;
  /** v0.3: set exactly when `key` is "connected". */
  account: NexusAccountView | null;
}

export interface RecoveryNote {
  optionName: string;
  action: "install" | "uninstall" | "reorder" | "restoreParked";
  filesRolledBack: number;
}

export interface Overview {
  game: GameSummary | null;
  reframework: ReframeworkSummary | null;
  /** v0.3: the one library folder (replaces v0.2's `roots`). */
  mods: ModsFolderView;
  library: LibraryStats;
  scan: ScanState;
  checks: Check[];
  nexus: NexusSummary;
  recovered: RecoveryNote | null;
}

// ---------------------------------------------------------------------------
// search — the headline ask
// ---------------------------------------------------------------------------

export type MatchedField =
  | "name"
  | "trail"
  | "bundle"
  | "category"
  | "author"
  | "description"
  | "packageName"
  | "version"
  | "file"
  /** v0.2: the package's Nexus category or one of its tags. */
  | "nexusLabel";

export interface SearchFilters {
  installed: boolean | null;
  includeMenuOnly: boolean;
  /** v0.2: Nexus categories, ANY-of (a mod has one category). Empty = no filter. */
  categories: string[];
  /** v0.2: Nexus tags, ALL-of (each tag narrows). Empty = no filter. */
  tags: string[];
}

export interface SearchQuery {
  text: string;
  filters: SearchFilters;
  /** 0 = no cap: every match (v0.3; v0.2 capped 0 at 200). */
  limit: number;
}

/** v0.3: "adopted" — an install the journal took over from files already in
 *  the game (it has no archive entries of its own; it uninstalls like any other). */
export type InstallOrigin = "riftstone" | "adopted";

export interface InstallBadge {
  origin: InstallOrigin;
  installedAt: string | null;
}

export type UpdateBadge = "available" | "current" | "unknown";

export interface SearchResult {
  optionId: string;
  packageId: string;
  score: number;
  name: string;
  trail: string[];
  bundle: string | null;
  packageName: string;
  packageFormat: PackageFormat;
  author: string | null;
  version: string | null;
  category: string | null;
  /** First ~180 chars of the description, whitespace-collapsed. */
  descriptionSnippet: string | null;
  hasScreenshot: boolean;
  kinds: PayloadKinds;
  menuOnly: boolean;
  matched: MatchedField[];
  fileMatches: string[];
  installed: InstallBadge | null;
  /** Package-level update state; null when the package is not linked to Nexus. */
  update: UpdateBadge | null;
  conflicted: boolean;
  /** v0.2: the linked mod's Nexus category (null: unlinked or never checked). */
  nexusCategory: string | null;
  /** v0.2: the linked mod's Nexus tags, Nexus's own spelling. */
  nexusTags: string[];
  /** v0.2: Nexus flags the linked mod as adult content. */
  adult: boolean;
}

// ---------------------------------------------------------------------------
// v0.2 — Nexus labels as filters (get_labels)
// ---------------------------------------------------------------------------

/** What Nexus says about a linked mod, from the latest update check. */
export interface NexusLabels {
  category: string | null;
  tags: string[];
  adult: boolean;
}

export interface LabelCount {
  /** Nexus's spelling. Filters compare case-insensitively. */
  name: string;
  /** Installable options in the whole library carrying it. A chip predicts
   *  its results with this only while nothing else narrows the search; the
   *  window counts the live answers otherwise (useSearch predictions). */
  options: number;
}

export interface LabelFacets {
  /** Count descending, then name. */
  categories: LabelCount[];
  tags: LabelCount[];
  /** Packages with at least one installable option and neither a category nor
   *  a tag (rift-catalog search.rs label_facets): unlinked, linked but never
   *  checked, and also linked mods whose answer carried no category or tag
   *  (an adult-only flag, or no labels at all). */
  unlabeledPackages: number;
}

// ---------------------------------------------------------------------------
// v0.2 — Find on Nexus by name (find_on_nexus)
// ---------------------------------------------------------------------------

/** A Nexus mod that may be where a package came from. A find never links:
 *  the player picks one, and the window links it with `link_package`. */
export interface NexusCandidate {
  modId: number;
  name: string;
  /** The author Nexus shows, else the uploader's name. */
  author: string | null;
  version: string | null;
  category: string | null;
  adult: boolean;
  downloads: number | null;
  endorsements: number | null;
  /** Nexus's summary, whitespace-collapsed, at most 200 characters. */
  summary: string | null;
  /** The mod's last update on Nexus, ms since the epoch. */
  updatedMs: number | null;
  /** https://www.nexusmods.com/dragonsdogma2/mods/<modId> */
  pageUrl: string;
  /** Packages in the library already linked to this mod. */
  linkedPackages: number;
}

export interface NexusFindings {
  packageId: string;
  /** The words sent to Nexus (the set answered last): from the package's
   *  display name, or, when that finds nothing or holds nothing searchable,
   *  from its first option's bundle or name. "" when none of those held
   *  anything to search for: then nothing was sent and `candidates` is empty. */
  terms: string;
  /** Nexus's relevance order, at most 5. */
  candidates: NexusCandidate[];
}

// ---------------------------------------------------------------------------
// get_option / get_package
// ---------------------------------------------------------------------------

export interface PackageSummary {
  id: string;
  path: string;
  fileName: string;
  displayName: string;
  format: PackageFormat;
  size: number;
  modifiedMs: number;
  entryCount: number;
  optionCount: number;
  nexus: NexusLink | null;
  problems: Problem[];
}

export interface OptionRef {
  optionId: string;
  name: string;
  menuOnly: boolean;
  hasScreenshot: boolean;
  installed: boolean;
}

export interface InstalledFile {
  gamePath: string;
  kind: DeployKind;
  size: number;
}

export interface InstalledInfo {
  seq: number;
  installedAt: string;
  version: string | null;
  gameBuild: string | null;
  origin: InstallOrigin;
  files: InstalledFile[];
}

export interface LayerRef {
  optionId: string;
  name: string;
}

export type ConflictKey =
  | { kind: "path"; path: string }
  | { kind: "hash"; hash: { lower: number; upper: number }; path: string | null };

export interface ConflictView {
  key: ConflictKey;
  /** Loser first, winner last. */
  layers: LayerRef[];
  winner: LayerRef;
  /** v0.2: a pak in it carries table flags rift-pak does not know, so the
   * stone cannot be sure the hash is what the engine reads. */
  unconfirmed: boolean;
}

export interface NexusFileView {
  fileId: number;
  name: string;
  version: string | null;
  uploadedMs: number;
}

export type UpdateStateView =
  | { kind: "current" }
  | { kind: "updateAvailable"; newer: NexusFileView }
  | { kind: "localArchived"; newer: NexusFileView | null }
  | { kind: "unpinned" }
  | { kind: "modUnavailable"; status: string }
  | { kind: "unlinked" }
  | { kind: "notChecked" }
  | { kind: "error"; message: string };

export interface UpdateRow {
  packageId: string;
  packageName: string;
  link: NexusLink | null;
  modName: string | null;
  localVersion: string | null;
  localUploadedMs: number | null;
  state: UpdateStateView;
  pageUrl: string | null;
  checkedMs: number | null;
  /** v0.5: on a family's lead, the older versions of its file still in the mods folder, newest
   *  first; empty otherwise. An older version has no row of its own. */
  older: OlderVersionView[];
}

export interface OptionDetail {
  option: ModOption;
  package: PackageSummary;
  parentName: string | null;
  children: OptionRef[];
  installed: InstalledInfo | null;
  conflicts: ConflictView[];
  update: UpdateRow | null;
  /** v0.2: null when Nexus has not labelled the package: unlinked, never
   *  checked, or an answer that carried no labels (the spine drops a blank
   *  answer). The window reads a blank NexusLabels as unknown too. */
  labels: NexusLabels | null;
}

export interface TreeNode {
  /** null for a bundle node. */
  optionId: string | null;
  label: string;
  kind: "option" | "bundle";
  menuOnly: boolean;
  hasScreenshot: boolean;
  /** A bundle: any child is installed. */
  installed: boolean;
  /** v0.3: the option takes part in a conflict (a bundle: any child does). */
  conflicted: boolean;
  kinds: PayloadKinds;
  children: TreeNode[];
}

export interface PackageDetail {
  package: PackageSummary;
  tree: TreeNode[];
  entries: ArchiveEntry[];
}

// ---------------------------------------------------------------------------
// v0.3 — the library tree (get_library): the Rift's idle view
// ---------------------------------------------------------------------------

/** One package of the mods folder with its options nested the way their menus
 *  nest them. The tree is today's PackageDetail.tree shape (addonfor children
 *  under a parent in the same package, NameAsBundle siblings under a bundle
 *  node, an option whose parent sits in another package as a top node of its
 *  own package), built by rift-catalog `tree::nest` — without the entry list. */
export interface LibraryPackage {
  package: PackageSummary;
  /** Options that install something (not menu-only). */
  installable: number;
  /** Options of this package in your world now (journal installs). */
  installed: number;
  /** Package-level update state; null when the package is not linked to Nexus. */
  update: UpdateBadge | null;
  /** Null when Nexus has not labelled the package (see OptionDetail.labels). */
  labels: NexusLabels | null;
  /** Nexus flags the linked mod adult. */
  adult: boolean;
  /** At least one option of it takes part in a conflict. */
  conflicted: boolean;
  /** v0.5: its place among the versions of one Nexus file in the mods folder; null when no other
   *  version of the same file is there (WINDOW-CONTRACT § v0.5, Versions of one file). */
  family: FamilyView | null;
  /** v0.5 (ruling 10): the stone asks whether this package is an older version of another
   *  package's file; null when there is nothing to ask (WINDOW-CONTRACT § v0.5, The player's
   *  word). */
  versionQuestion: VersionQuestion | null;
  /** Empty for a package that yielded no options (its `problems` say why). */
  tree: TreeNode[];
}

export interface LibraryView {
  mods: ModsFolderView;
  /** Every package in the mods folder — troubled ones too, carrying their
   *  problems — ordered by display name (case-blind), then file name. */
  packages: LibraryPackage[];
}

// ---------------------------------------------------------------------------
// install / uninstall
// ---------------------------------------------------------------------------

export type WriteAction =
  | { kind: "create" }
  /** Over a file an install in the journal owns (adopted installs too). */
  | { kind: "overwriteRiftstone"; optionName: string }
  | { kind: "overwriteUnowned" }
  | { kind: "reinstall" };

export interface PlannedWrite {
  entry: string;
  gamePath: string;
  kind: DeployKind;
  size: number;
  action: WriteAction;
}

export type PlanWarning =
  | { kind: "pakFolderOff" }
  | { kind: "looseLoaderOff" }
  | { kind: "reframeworkMissing" }
  | { kind: "gameRunning" }
  | { kind: "requiresMissing"; name: string }
  | { kind: "conflicts"; optionNames: string[]; paths: number }
  | { kind: "outdatedFormats"; count: number };

export interface InstallPlanView {
  optionId: string;
  name: string;
  writes: PlannedWrite[];
  totalBytes: number;
  warnings: PlanWarning[];
  /** v0.5: the installs of other versions of the same file whose place this install takes, in
   *  the same journaled step (their removals run first, and a failure rolls both back). */
  replaces: ReplacesView[];
  /** A full sentence when the install cannot run. */
  refused: string | null;
}

export interface InstallResultView {
  optionId: string;
  filesWritten: number;
  bytesWritten: number;
  backups: number;
  durationMs: number;
  /** The game is running: paks and new loose files load at the next launch. */
  restartNeeded: boolean;
  /** v0.5: older versions moved to the older-versions folder after the install committed. */
  putAway: PutAwayView[];
  /** v0.5: a sentence when an older version could not be moved; the install stands. */
  putAwayFailed: string | null;
}

export type RemovalAction =
  | { kind: "delete" }
  /** v0.2: the file is already gone from the game folder. */
  | { kind: "alreadyGone" }
  | { kind: "restoreBackup"; owner: string }
  | { kind: "keepChanged" }
  | { kind: "keepVanilla" }
  | { kind: "handDown"; toOptionName: string };

export interface PlannedRemoval {
  gamePath: string;
  kind: DeployKind;
  action: RemovalAction;
}

export interface UninstallPlanView {
  optionId: string;
  name: string;
  removals: PlannedRemoval[];
  refused: string | null;
}

export interface UninstallResultView {
  optionId: string;
  filesRemoved: number;
  filesRestored: number;
  /** Left in place: changed by something else, or possibly vanilla. */
  filesKept: number;
  /** v0.2: handed down to the install below (it now owns them). */
  filesHandedDown: number;
  /** v0.2: files kept aside in the backup store (InstalledView.parked). */
  parked: number;
  restartNeeded: boolean;
}

// ---------------------------------------------------------------------------
// get_installed — what is in the game now
// ---------------------------------------------------------------------------

export interface InstalledRow {
  /** Opaque: the window never parses it. An adopted install that matched no
   *  library option carries a stand-in id; get_option answers entryNotFound
   *  for it and plan_uninstall works on it. */
  optionId: string;
  name: string;
  packageName: string;
  /** The mods folder holds no package for this install: its package left the
   * folder, or (an adopted stand-in) it never had one. Uninstall still works:
   * the journal knows every file. */
  packageMissing: boolean;
  version: string | null;
  installedAt: string;
  gameBuild: string | null;
  origin: InstallOrigin;
  kinds: PayloadKinds;
  fileCount: number;
  /** The paks in pak_mods/ this install owns, in load order. */
  paks: PakView[];
}

/** What REFramework's own log says about a managed pak. */
export type PakLoadState =
  /** Injected at the last game launch (slot from the log). */
  | { kind: "loaded"; slot: number; nativeName: string }
  /** Installed after the last launch began — loads next time. */
  | { kind: "pending" }
  /** Installed before the last launch, yet REFramework never injected it. */
  | { kind: "notLoaded" }
  /** No REFramework log to read. */
  | { kind: "unknown" };

export interface PakView {
  fileName: string;
  size: number;
  load: PakLoadState;
}

export interface StrayFile {
  gamePath: string;
  size: number;
}

export interface MissingFile {
  optionId: string;
  optionName: string;
  gamePath: string;
}

export interface PakLoadReport {
  /** Timestamp of the log's first line — when the game last started. */
  logStarted: string | null;
  injected: { fileName: string; nativeName: string; slot: number }[];
  /** REFramework's "NONE were injected" line, verbatim, when present. */
  error: string | null;
}

export interface InstalledView {
  installs: InstalledRow[];
  /** Option ids of pak-bearing installs, lowest priority first (last wins). */
  pakOrder: string[];
  conflicts: ConflictView[];
  strays: StrayFile[];
  missing: MissingFile[];
  pakLoad: PakLoadReport | null;
  /** pak_mods/ count above which REFramework's slot table is at risk (100). */
  pakCountWarning: number;
  /** v0.2: files kept aside in the backup store, each with put back / discard. */
  parked: ParkedFileView[];
}

/** v0.2: a file the stone kept aside rather than delete or force back. */
export interface ParkedFileView {
  /** Its handle in the backup store: what restore_parked / discard_parked take. */
  store: string;
  /** Where it belongs in the game folder. */
  gamePath: string;
  /** Whose bytes it holds: "the file that was there before", or an install's name. */
  owner: string;
  /** v0.3: "adopted" — the owner is an adopted install. */
  ownerKind: "unowned" | "adopted" | "riftstone";
  /** Its size in the store; null when the kept file is gone from it. */
  size: number | null;
  /** RFC 3339, UTC. */
  parkedAt: string;
  /** Why it was kept aside, one sentence. */
  reason: string;
  /** Something sits at gamePath now. Putting it back swaps that out, unless
   *  an install owns the occupant: then the engine refuses and names the
   *  owner (rift-deploy engine.rs restore_parked). */
  occupied: boolean;
}

/** v0.2: what restore_parked did. */
export interface ParkedRestoreReport {
  gamePath: string;
  restored: boolean;
  /** The store handle of the file that sat at gamePath, now kept aside in its turn. */
  swappedOut: string | null;
  restartNeeded: boolean;
}

// ---------------------------------------------------------------------------
// Nexus
// ---------------------------------------------------------------------------

/** Nexus request budget from the X-RL-* headers of the last v1 response
 * (anonymous v2 calls carry none — then the spine returns null). */
export interface NexusQuota {
  hourlyRemaining: number;
  hourlyLimit: number;
  dailyRemaining: number;
  dailyLimit: number;
  /** When the hourly budget refills. */
  hourlyResetMs: number | null;
}

export interface UpdatesView {
  lastCheckMs: number | null;
  rows: UpdateRow[];
}

export interface Md5Result {
  packageId: string;
  matched: boolean;
  link: NexusLink | null;
}

// ---------------------------------------------------------------------------
// v0.3 — a mod's files on Nexus (get_mod_files), browsing Nexus (browse_nexus)
// ---------------------------------------------------------------------------

/** Nexus's file categories. Retired: oldVersion, removed, archived. */
export type NexusFileCategory =
  | "main"
  | "update"
  | "optional"
  | "oldVersion"
  | "miscellaneous"
  | "removed"
  | "archived"
  | "unknown";

export interface NexusFileEntry {
  fileId: number;
  /** The file's name on its Nexus page (not the archive's file name). */
  name: string;
  version: string | null;
  category: NexusFileCategory;
  /** Bytes; null when Nexus does not say. */
  size: number | null;
  uploadedMs: number;
  /** The file's description on Nexus, whitespace-collapsed, at most 400 characters. */
  description: string | null;
  /** The file's own patch notes on Nexus (ModFile.changelogText), one line
   *  each, as Nexus wrote them. Anonymous: no key needed. */
  changelog: string[];
  /** A package in the mods folder is pinned to this exact file. */
  inLibrary: boolean;
  /** The newest file of its group that is not retired: what an update fetches. */
  newest: boolean;
}

export interface NexusModFiles {
  modId: number;
  /** The mod's name on Nexus; null when Nexus did not say. */
  modName: string | null;
  /** https://www.nexusmods.com/dragonsdogma2/mods/<modId> */
  pageUrl: string;
  adult: boolean;
  /** Main, update, optional, miscellaneous, then the retired ones; newest
   *  upload first within a category. */
  files: NexusFileEntry[];
}

/** How browse_nexus orders Dragon's Dogma 2 mods. "relevance" needs text. */
export type NexusBrowseSort = "endorsements" | "downloads" | "updated" | "relevance";

export interface NexusBrowseQuery {
  /** Words the player typed; "" browses the whole game by `sort`. Sent to
   *  Nexus only when the player asks (privacy: nothing from the library). */
  text: string;
  sort: NexusBrowseSort;
  /** Paging: 0, 20, 40 … (20 per page; Nexus caps a page at 80). */
  offset: number;
}

export interface NexusBrowsePage {
  /** Every match Nexus counts, across pages. */
  total: number;
  offset: number;
  /** The same card shape as Find on Nexus. */
  mods: NexusCandidate[];
}

// ---------------------------------------------------------------------------
// v0.3 — the key's extras: tracked mods and endorsements (get_nexus_standings)
// ---------------------------------------------------------------------------

export type EndorseStatus = "endorsed" | "abstained";

export interface NexusStandings {
  /** Dragon's Dogma 2 mods this account tracks on Nexus, as cards
   *  (`linkedPackages` says which are in the mods folder already). */
  tracked: NexusCandidate[];
  /** This account's decided endorsements; a mod absent here is undecided. */
  endorsements: { modId: number; status: EndorseStatus }[];
  /** When the spine last asked Nexus (ms). */
  checkedMs: number;
}

// ---------------------------------------------------------------------------
// v0.3 — downloads into the mods folder
// ---------------------------------------------------------------------------

/** - waiting: a free account's download, parked until the site's Mod Manager
 *    Download link (nxm://) for the same mod and file arrives.
 *  - queued → linking (asking Nexus for a download link) → downloading →
 *    verifying (size, then the MD5 round trip) → landed (in the mods folder).
 *  - failed / cancelled: nothing was left in the mods folder. */
export type DownloadPhase =
  | "waiting"
  | "queued"
  | "linking"
  | "downloading"
  | "verifying"
  | "landed"
  | "failed"
  | "cancelled";

export interface DownloadView {
  id: string;
  modId: number;
  fileId: number;
  /** The mod's name on Nexus, when known. */
  modName: string | null;
  /** The file's name on its Nexus page, when known. */
  fileName: string | null;
  version: string | null;
  /** The package this download updates, when it started from an update row. */
  forPackageId: string | null;
  phase: DownloadPhase;
  bytesDone: number;
  /** Null until Nexus or the server states it. */
  bytesTotal: number | null;
  /** The archive's file name in the mods folder once landed. */
  landedAs: string | null;
  /** The new package's id once landed. Null also for a landed file that is no
   *  archive (a raw .pak, an .exe): it sits in the mods folder under
   *  `landedAs`, but the scan never lists it, so no package carries it. */
  packageId: string | null;
  /** The same file was in the mods folder already: nothing new was written. */
  alreadyHad: boolean;
  /** For phase "failed": the failure the window voices (kind "nexus" carries a reason). */
  failure: RiftError | null;
  /** For phase "waiting": the file's page on Nexus, where Mod Manager Download
   *  sends the link back to the stone. The window opens it. */
  nexusPageUrl: string | null;
  startedMs: number;
  finishedMs: number | null;
}

// ---------------------------------------------------------------------------
// v0.3 — the nxm:// handler (Nexus's Mod Manager Download button)
// ---------------------------------------------------------------------------

/** - ours: Windows sends nxm:// links to this riftstone.exe.
 *  - none: no program answers them.
 *  - other: another program answers them (`other` names its exe).
 *  - unsupported: not on Windows (the bench). */
export type NxmHandlerState = "ours" | "none" | "other" | "unsupported";

export interface NxmHandlerView {
  state: NxmHandlerState;
  /** The other program's file name only, e.g. "Vortex.exe"; never a path. */
  other: string | null;
  /** Turning the stone off gives the link back to this program's file name
   *  (the handler the stone replaced), or to nobody. */
  givesBackTo: string | null;
}

// ---------------------------------------------------------------------------
// v0.3 — Play (plan_launch, launch_game)
// ---------------------------------------------------------------------------

/** Reasons a launch may not go as the player expects; never blockers. */
export type LaunchWarning =
  /** Pak installs exist but REFramework's dinput8.dll is missing: they won't load. */
  | { kind: "reframeworkMissing"; paks: number }
  /** REFramework's pak folder switch is off: the installed paks won't load. */
  | { kind: "pakFolderOff"; paks: number }
  /** The loose file loader is off: installed loose files won't load. */
  | { kind: "looseLoaderOff"; files: number }
  /** Steam updated the game since the newest install. */
  | { kind: "buildChanged"; recorded: string; now: string };

export interface LaunchPlanView {
  launch: LaunchRoute;
  warnings: LaunchWarning[];
  /** A full sentence when Play cannot start now: the game already runs, no
   *  game is found, a change is being made, or one was cut short. */
  refused: string | null;
}

export interface LaunchReport {
  launch: LaunchRoute;
  /** When the stone asked Steam or Windows to start the game (ms). */
  launchedMs: number;
}

// ---------------------------------------------------------------------------
// settings
// ---------------------------------------------------------------------------

/** v0.2: which screenshots are veiled (blurred until hovered). */
export type VeilMode = "off" | "adult" | "all";

export interface Settings {
  /** v0.2: replaces v0.1's `veilThumbnails: boolean` (true → "all", false → "off";
   * the spine reads the old key from settings.json). Default "off". `adult`
   * veils the options of mods Nexus flags as adult AND of packages Nexus has
   * not labelled yet (unlinked, never checked, or an answer without labels):
   * a veil exists to protect, so the unknown is veiled (WINDOW-CONTRACT § v0.2
   * "Veil"; the window's one rule is services/ledger.ts veils + adultnessOf). */
  veil: VeilMode;
  /** Check Nexus on launch when the last check is older than 12 hours. Default true. */
  autoCheckUpdates: boolean;
  /** v0.5: THE MUSTER's chosen character (`MusterSlot.character`), or null for the character of
   *  the newest save. Private like the names it is made from: never logged. Default null. */
  musterCharacter: string | null;
}

// ---------------------------------------------------------------------------
// events (payloads)
// ---------------------------------------------------------------------------

export interface ScanProgressEvent {
  done: number;
  total: number;
  current: string;
}

export interface InstallProgressEvent {
  optionId: string;
  filesDone: number;
  filesTotal: number;
  bytesDone: number;
  bytesTotal: number;
}

export interface UpdatesProgressEvent {
  phase: "linking" | "hashing" | "checking";
  done: number;
  total: number;
}

/** v0.3: throttled to one per 200 ms per download, plus the last. */
export interface DownloadProgressEvent {
  id: string;
  phase: DownloadPhase;
  bytesDone: number;
  bytesTotal: number | null;
}

/** v0.3: an nxm:// link reached the stone (the site's Mod Manager Download
 *  button, through Windows). `accepted`: a download was started or a waiting
 *  one resumed (`downloadId`); otherwise `refusal` says why (an expired link,
 *  another account's link, another game, no key). `nxm://premium` answers
 *  `accepted: false` with `refusal: null`: nothing was refused, while the
 *  account is validated again in the background. The window brings itself
 *  forward on THE FAR WORLD. Ids are null when the link could not be read. */
export interface NxmReceivedEvent {
  modId: number | null;
  fileId: number | null;
  accepted: boolean;
  downloadId: string | null;
  refusal: RiftError | null;
}

export const EVENTS = {
  scanProgress: "rift://scan-progress",
  libraryChanged: "rift://library-changed",
  installProgress: "rift://install-progress",
  /** Also when the game starts or stops (v0.3, the spine's game watcher). */
  gameChanged: "rift://game-changed",
  updatesProgress: "rift://updates-progress",
  updatesChanged: "rift://updates-changed",
  /** v0.3: a download row appeared, changed phase or was cleared. */
  downloadsChanged: "rift://downloads-changed",
  /** v0.3 */
  downloadProgress: "rift://download-progress",
  /** v0.3 */
  nxmReceived: "rift://nxm-received",
  /** v0.4: a slot file appeared, changed or went, or the game's names were rebuilt. */
  musterChanged: "rift://muster-changed",
} as const;

export type ThumbnailSize = "card" | "full";

// ---------------------------------------------------------------------------
// v0.4 — the Muster (WINDOW-CONTRACT § v0.4): the party from the player's own
// saves, named from the player's own game. Read-only.
// ---------------------------------------------------------------------------

/** v0.4: why a save slot would not open. */
export type SaveReason =
  /** The slot opens with no Steam account the stone found: another account's save. */
  | "otherAccount"
  /** The slot is busy: twice, 500 ms apart, it was cut short or disagreed with its signature
   *  while it changed (the game writing it), or it would not be read at all (another program
   *  holding it). Never remembered: the next read tries again. */
  | "beingWritten"
  /** The container or its stream is broken. */
  | "damaged"
  /** The file opens but holds no party. */
  | "noParty"
  /** The slot left the folder since the list was read. */
  | "gone";

/** A thing the game names: its id, and the game's own English name for it, or null when the
 *  game's text has no entry for the id or the names could not be read. */
export interface Named {
  id: number;
  name: string | null;
}

export interface MusterSlotsView {
  /** Why no slot can be listed, or null when the stone found a save folder. */
  problem: MusterFolderProblem | null;
  /** Newest first, across every Steam account's save folder. */
  slots: MusterSlot[];
  /** How many Steam accounts on this machine have Dragon's Dogma 2 saves. */
  accountCount: number;
  /** Whether the slots' vocations are named from the game (`game`) or the names could not be
   *  read (`missing`), so a row can tell "the game has no name" from "no names at all". */
  names: "game" | "missing";
}

export type MusterFolderProblem =
  /** No Steam folder was found (the registry, or the game's own library). */
  | "noSteam"
  /** Steam was found, but no account has Dragon's Dogma 2 saves. */
  | "noSaves";

export interface MusterSlot {
  /** The handle: the file's name without `.bin` (`data009Slot`). */
  slotId: string;
  /** The game's slot number. */
  slot: number;
  /** When the game last wrote the slot (the file's modified time, ISO 8601). */
  savedAt: string;
  /** 1-based when the machine has more than one account with saves; null otherwise. */
  account: number | null;
  /** The Arisen's summary, null when the slot would not open (then `problem` says why). */
  arisen: MusterSlotArisen | null;
  problem: SaveReason | null;
  /** v0.5: the character the slot belongs to, 16 hex digits: the Arisen with their main pawn,
   *  within one Steam account (WINDOW-CONTRACT § v0.5, THE MUSTER by character). Made from
   *  private names, so it is private too: never logged. Null when the slot would not open. */
  character: string | null;
  /** v0.5: the main pawn's name, the player's own: shown, never logged. */
  mainPawnName: string | null;
}

export interface MusterSlotArisen {
  /** The player's own: shown, never logged. */
  name: string | null;
  level: number;
  vocation: Named;
}

export interface MusterView {
  slotId: string;
  slot: number;
  savedAt: string;
  names: "game" | "missing";
  /** With `missing`: where the names live and why they could not be read. */
  namesNote: string | null;
  arisen: MusterMember | null;
  mainPawn: MusterMember | null;
  /** The hired pawns, in the order the party keeps them (at most two). */
  hired: MusterMember[];
  gold: number | null;
  riftCrystals: number | null;
}

export type MusterRole = "arisen" | "mainPawn" | "hiredPawn";

export interface MusterMember {
  /** Stable for one pawn across re-reads of a slot: 16 hex digits hashed from the save's own
   *  record id. Neither a name nor a Rift id; the window uses it only to tell members apart. */
  memberKey: string;
  role: MusterRole;
  /** The player's own name for the Arisen or a pawn: shown, never logged. */
  name: string | null;
  level: number;
  experience: number;
  vocation: Named;
  /** Every vocation this member has experience in, the most experienced first. */
  vocations: MusterVocation[];
  gains: MusterGains;
  /** Always eight: main weapon, sub weapon, head, body, legs, cloak, ring, ring. */
  gear: MusterGear[];
  /** For the current vocation, in the game's order, empty places left out. */
  augments: Named[];
  skills: Named[];
  /** A pawn's inclination and specialization; null for the Arisen. */
  pawn: MusterPawnTraits | null;
  /** A hired pawn's creator: their level and vocation (their name stays out). */
  creator: MusterCreator | null;
}

export interface MusterVocation {
  vocation: Named;
  experience: number;
}

/** What the levels gave (the game's level-up increases, not the gear's). */
export interface MusterGains {
  health: number;
  stamina: number;
  strength: number;
  defence: number;
  magick: number;
  magickDefence: number;
  weight: number;
  knockdown: number;
}

export type MusterGearSlot = "mainWeapon" | "subWeapon" | "head" | "body" | "legs" | "cloak" | "ring1" | "ring2";

export interface MusterGear {
  slot: MusterGearSlot;
  item: Named | null;
}

export interface MusterPawnTraits {
  inclination: Named | null;
  specialization: Named | null;
}

export interface MusterCreator {
  level: number;
  vocation: Named;
}

// ---------------------------------------------------------------------------
// v0.5 — versions of one file (WINDOW-CONTRACT § v0.5): one entry per Nexus
// file, an install that replaces the other version's, and the older-versions
// folder beside the mods folder. THE MUSTER by character is in the v0.4 types
// above (MusterSlot.character, MusterSlot.mainPawnName, Settings.musterCharacter).
// ---------------------------------------------------------------------------

/** v0.5: a package's place among the versions of one Nexus file in the mods folder. */
export interface FamilyView {
  /** The newest version's package id; this package's own id when it leads. */
  lead: string;
  /** This package's Nexus file version, when Nexus named one. */
  version: string | null;
  /** On the lead only: the older versions still in the mods folder, newest first. Empty on
   *  an older version. */
  older: OlderVersionView[];
}

/** v0.5 (ruling 10): is this package a version of another package's file? The file names say
 *  it is likely; only the player's answer joins them. */
export interface VersionQuestion {
  /** The newest package of that file in the mods folder (its family's lead when it has one). */
  ofPackageId: string;
  ofFileName: string;
  /** That package's Nexus file version, when Nexus named one. */
  ofVersion: string | null;
  /** File names both hold, folders and case ignored, modinfo.ini and screenshots left out. */
  sharedFiles: number;
  /** How many file names this package holds, counted the same way. */
  files: number;
}

/** v0.5: an older version of a file, still in the mods folder. */
export interface OlderVersionView {
  packageId: string;
  fileName: string;
  version: string | null;
  /** Options of it in your world now (journal installs). */
  installed: number;
  /** Its options that are the same option as one of the lead's: an install of the lead's
   *  takes its place (WINDOW-CONTRACT § v0.5, Install replaces). An option of it that is not
   *  listed here is one only this version holds. The window reads the pairing from here and
   *  never works it out itself. */
  matches: OlderOptionMatch[];
}

/** v0.5: an option of an older version and the lead's option that is the same option. */
export interface OlderOptionMatch {
  /** The older version's option. */
  optionId: string;
  /** The lead's option an install of which takes its place. */
  leadOptionId: string;
  /** The older version's option is in your world now. */
  installed: boolean;
}

/** v0.5: an install of another version of the same file that an install plan replaces. */
export interface ReplacesView {
  /** The installed option the plan uninstalls first. */
  optionId: string;
  name: string;
  /** The version it belongs to, when Nexus named one. */
  version: string | null;
  /** How many of its files the replace removes or hands back. */
  removals: number;
}

/** v0.5: an older version moved out of the mods folder. */
export interface PutAwayView {
  /** Its name in the mods folder. */
  fileName: string;
  /** Its name in the older-versions folder: the same name, or ` (2)` … when that one was taken. */
  landedAs: string;
}

/** v0.5: what "keep only the newest" would do to one file's versions. */
export interface KeepNewestPlanView {
  /** The family's lead. */
  packageId: string;
  /** The file name of the version kept: the newest one with an install, else the lead. Null
   *  only when the package is in no family. */
  keeps: string | null;
  /** The installs of older versions it uninstalls. */
  uninstalls: ReplacesView[];
  /** The older versions' file names it then moves to the older-versions folder. */
  putsAway: string[];
  /** A full sentence when it cannot run: the package is the only version of its file, no
   *  version is older than the one kept, the game folder or its journal cannot be read, or an
   *  uninstall it needs is refused (the game holds a pak open). */
  refused: string | null;
}

export interface KeepNewestResultView {
  uninstalled: number;
  putAway: PutAwayView[];
  /** A sentence when an older version could not be moved; the uninstalls stand. */
  putAwayFailed: string | null;
}

/** v0.5: the older-versions folder beside the mods folder. */
export interface OlderShelfView {
  /** The folder exists (it is made at the first put-away). */
  exists: boolean;
  /** Archives and folders in it, by name. */
  items: OlderShelfItem[];
}

export interface OlderShelfItem {
  fileName: string;
  size: number;
  /** The Nexus mod it is linked to, from nexus.json, when it was linked before it was put away. */
  modName: string | null;
  version: string | null;
  /** A newer version of the same file is in the mods folder now. */
  newerInMods: boolean;
}

export interface BringBackResult {
  /** Its name in the mods folder again: the same name, or ` (2)` … when that one was taken. */
  landedAs: string;
}
