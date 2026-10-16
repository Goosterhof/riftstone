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
  | { kind: "rescan" };

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
  action: "install" | "uninstall" | "reorder";
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
  | "file";

export interface SearchFilters {
  installed: boolean | null;
  rootIds: string[];
  includeMenuOnly: boolean;
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
  filesKept: number;
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
}

export interface AdoptResultView {
  adopted: number;
  alreadyAdopted: number;
  missingFiles: number;
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

export interface Settings {
  /** Blur screenshots until hovered. Default false. */
  veilThumbnails: boolean;
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
