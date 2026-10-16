// The window contract — every shape that crosses the Tauri bridge.
//
// The Rust side (src-tauri/src/dto.rs) mirrors these types field for field
// with `#[serde(rename_all = "camelCase")]`. Change one, change both, in the
// same commit. Command names and semantics: docs/WINDOW-CONTRACT.md.

// ---------------------------------------------------------------------------
// Shared atoms (mirrors of rift-core)
// ---------------------------------------------------------------------------

export type PackageFormat = "zip" | "sevenZip" | "rar" | "folder";
export type RootKind = "fluffy" | "downloads" | "custom";
export type DeployKind = "pak" | "loose" | "reframework" | "root";

export interface PayloadKinds {
  pak: number;
  loose: number;
  reframework: number;
  root: number;
}

export interface LibraryRoot {
  id: string;
  path: string;
  label: string;
  kind: RootKind;
  dd2ShapedOnly: boolean;
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

export type LinkSource = "filename" | "homepage" | "md5" | "manual";

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
    | "io";
  detail?: Record<string, string>;
}

// ---------------------------------------------------------------------------
// get_overview — the home frame
// ---------------------------------------------------------------------------

export interface GameSummary {
  root: string;
  buildId: string | null;
  source: "steam" | "env" | "manual";
  running: boolean;
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

export interface FluffySummary {
  root: string;
  modsDir: string;
  installedCount: number;
  unadoptedCount: number;
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
  | { kind: "adoptFluffy" }
  | { kind: "enableSwitch"; switch: "looseFileLoader" | "pakDirectory" }
  | { kind: "pickGameFolder" }
  | { kind: "openUrl"; url: string }
  | { kind: "rescan" }
  /** v0.2: open the parked-backup review (InstalledView.parked). */
  | { kind: "reviewParked" };

export interface Check {
  /** Stable id (WINDOW-CONTRACT § Check ids). PreflightPage draws `game`,
   * `build`, `reframework`, `pakdir`, `loose` and `fluffy` as rows of its own
   * (OWN_CHECKS) and lists every other check below them; a spine test reads
   * OWN_CHECKS from PreflightPage.vue, so the two cannot drift apart. */
  id: string;
  level: CheckLevel;
  title: string;
  detail: string;
  action: CheckAction | null;
}

export interface NexusSummary {
  lastCheckMs: number | null;
  checking: boolean;
  linked: number;
  unlinked: number;
  updates: number;
  hasApiKey: boolean;
}

export interface RecoveryNote {
  optionName: string;
  action: "install" | "uninstall" | "reorder" | "restoreParked";
  filesRolledBack: number;
}

export interface Overview {
  game: GameSummary | null;
  reframework: ReframeworkSummary | null;
  fluffy: FluffySummary | null;
  roots: LibraryRoot[];
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
  rootIds: string[];
  includeMenuOnly: boolean;
  /** v0.2: Nexus categories, ANY-of (a mod has one category). Empty = no filter. */
  categories: string[];
  /** v0.2: Nexus tags, ALL-of (each tag narrows). Empty = no filter. */
  tags: string[];
}

export interface SearchQuery {
  text: string;
  filters: SearchFilters;
  /** 0 = backend default (200). */
  limit: number;
}

export type InstallOrigin = "riftstone" | "fluffy" | "fluffyUnadopted";

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
  rootId: string;
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
// get_option / get_package
// ---------------------------------------------------------------------------

export interface PackageSummary {
  id: string;
  rootId: string;
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
  origin: "riftstone" | "fluffy";
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
}

export interface OptionDetail {
  option: ModOption;
  package: PackageSummary;
  parentName: string | null;
  children: OptionRef[];
  installed: InstalledInfo | null;
  /** Installed by Fluffy and not adopted yet (matched by name). */
  fluffyInstalled: boolean;
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
  installed: boolean;
  kinds: PayloadKinds;
  children: TreeNode[];
}

export interface PackageDetail {
  package: PackageSummary;
  tree: TreeNode[];
  entries: ArchiveEntry[];
}

// ---------------------------------------------------------------------------
// install / uninstall
// ---------------------------------------------------------------------------

export type WriteAction =
  | { kind: "create" }
  | { kind: "overwriteRiftstone"; optionName: string }
  | { kind: "overwriteFluffy"; section: string }
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

/** Option ids starting with this name a Fluffy install no library option
 * matched: `fluffy#<section>` — an adopted install's InstalledRow.optionId, or an
 * un-adopted install's LayerRef.optionId in a conflict. get_option answers
 * entryNotFound for them; plan_uninstall works on an adopted one. */
export const FLUFFY_STAND_IN_PREFIX = "fluffy#";

export interface InstalledRow {
  optionId: string;
  name: string;
  packageName: string;
  /** The library holds no package for this install: its package left the
   * library, or (a `fluffy#` stand-in id) it never had one. */
  packageMissing: boolean;
  version: string | null;
  installedAt: string;
  gameBuild: string | null;
  origin: "riftstone" | "fluffy";
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

export interface FluffyModView {
  section: string;
  name: string;
  fileCount: number;
  /** The library option it matches, when one does. */
  matchedOptionId: string | null;
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
  fluffyUnadopted: FluffyModView[];
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
  /** Whose bytes it holds: "the file that was there before", a Fluffy mod's name, or an install's. */
  owner: string;
  ownerKind: "unowned" | "fluffy" | "riftstone";
  /** Its size in the store; null when the kept file is gone from it. */
  size: number | null;
  /** RFC 3339, UTC. */
  parkedAt: string;
  /** Why it was kept aside, one sentence. */
  reason: string;
  /** Something sits at gamePath now. Putting it back swaps that out, unless
   *  an install or an un-adopted Fluffy mod owns the occupant: then the engine
   *  refuses and names the owner (rift-deploy engine.rs restore_parked). */
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

export interface AdoptResultView {
  adopted: number;
  alreadyAdopted: number;
  missingFiles: number;
  /** v0.2: files left to Fluffy (outside the places the stone manages). */
  refusedFiles: number;
  /** v0.2: files whose bytes belong to another layer. */
  sharedFiles: number;
  /** Fluffy sections no library option matched (adopted under their Fluffy name). */
  unmatched: string[];
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

export const EVENTS = {
  scanProgress: "rift://scan-progress",
  libraryChanged: "rift://library-changed",
  installProgress: "rift://install-progress",
  gameChanged: "rift://game-changed",
  updatesProgress: "rift://updates-progress",
  updatesChanged: "rift://updates-changed",
} as const;

export type ThumbnailSize = "card" | "full";
