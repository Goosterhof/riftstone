// The only door to the Rust spine. Components never import @tauri-apps/*;
// they use composables, which use this service.
//
// Two backends share one interface: the real one (Tauri invoke + events) and
// the mock (src/services/mock/), chosen when the page runs outside Tauri —
// `npm run dev` in a plain browser, the headless render harness, and the
// vitest suite. The mock is a separate chunk, loaded only when needed.

import type {
  BringBackResult,
  DownloadView,
  EndorseStatus,
  GameSummary,
  InstallPlanView,
  InstallResultView,
  InstalledView,
  KeepNewestPlanView,
  KeepNewestResultView,
  LaunchPlanView,
  LaunchReport,
  LibraryView,
  Md5Result,
  LabelFacets,
  MusterSlotsView,
  MusterView,
  NexusBrowsePage,
  NexusBrowseQuery,
  NexusFindings,
  NexusModFiles,
  NexusQuota,
  NexusStandings,
  NxmHandlerView,
  OlderShelfView,
  ParkedRestoreReport,
  PutAwayView,
  NexusSummary,
  OptionDetail,
  Overview,
  PackageDetail,
  PackageSummary,
  RiftError,
  SearchQuery,
  SearchResult,
  Settings,
  ThumbnailSize,
  UninstallPlanView,
  UninstallResultView,
  UpdateRow,
  UpdatesView,
  EVENTS,
} from "@/types/rift";

export type RiftEvent = (typeof EVENTS)[keyof typeof EVENTS];

const ERROR_KINDS: ReadonlySet<string> = new Set([
  "unreadable",
  "unsupportedFormat",
  "encrypted",
  "corruptArchive",
  "unsafeEntryPath",
  "entryNotFound",
  "gameNotFound",
  "refused",
  "journal",
  "nexus",
  "save",
  "io",
]);

/** Every rejected backend call surfaces as a RiftFailure — the real spine's
 * serialized RiftError and the mock's alike — so the window handles one shape. */
export class RiftFailure extends Error {
  readonly kind: RiftError["kind"];
  readonly detail: Record<string, string>;

  constructor(error: RiftError) {
    const detail = error.detail ?? {};
    super(Object.values(detail).join(" — ") || error.kind);
    this.name = "RiftFailure";
    this.kind = error.kind;
    this.detail = detail;
  }
}

function readDetail(raw: object): Record<string, string> {
  const detailRaw: unknown = "detail" in raw ? raw.detail : undefined;
  if (typeof detailRaw === "string") return { detail: detailRaw };
  if (typeof detailRaw !== "object" || detailRaw === null) return {};
  const detail: Record<string, string> = {};
  for (const [k, v] of Object.entries(detailRaw)) {
    detail[k] = typeof v === "string" ? v : JSON.stringify(v);
  }
  return detail;
}

/** Normalize anything a backend call rejected with. */
export function asRiftFailure(raw: unknown): RiftFailure {
  if (raw instanceof RiftFailure) return raw;
  if (
    typeof raw === "object" &&
    raw !== null &&
    "kind" in raw &&
    typeof raw.kind === "string" &&
    ERROR_KINDS.has(raw.kind)
  ) {
    return new RiftFailure({ kind: raw.kind as RiftError["kind"], detail: readDetail(raw) });
  }
  const message = raw instanceof Error ? raw.message : String(raw);
  return new RiftFailure({ kind: "io", detail: { detail: message } });
}

export interface RiftBackend {
  getOverview(): Promise<Overview>;
  rescan(): Promise<void>;
  search(query: SearchQuery): Promise<SearchResult[]>;
  getOption(optionId: string): Promise<OptionDetail>;
  getPackage(packageId: string): Promise<PackageDetail>;
  getThumbnail(optionId: string, size: ThumbnailSize): Promise<string | null>;
  planInstall(optionId: string): Promise<InstallPlanView>;
  install(optionId: string): Promise<InstallResultView>;
  planUninstall(optionId: string): Promise<UninstallPlanView>;
  uninstall(optionId: string): Promise<UninstallResultView>;
  getInstalled(): Promise<InstalledView>;
  reorderPaks(order: string[]): Promise<void>;
  setReframeworkSwitch(which: "looseFileLoader" | "pakDirectory", on: boolean): Promise<void>;
  setGameRoot(path: string): Promise<GameSummary>;
  /** v0.3: every package in the mods folder with its option tree (the Rift's idle view). */
  getLibrary(): Promise<LibraryView>;
  /** v0.3: open the mods folder in Explorer (made first if it is missing). */
  openModsFolder(): Promise<void>;
  /** v0.3: what Play would do now: its route, warnings, or a refusal. Pure. */
  planLaunch(): Promise<LaunchPlanView>;
  /** v0.3: start Dragon's Dogma 2 (through Steam, or DD2.exe). */
  launchGame(): Promise<LaunchReport>;
  getUpdates(): Promise<UpdatesView>;
  checkUpdates(): Promise<UpdatesView>;
  identifyPackages(packageIds: string[]): Promise<Md5Result[]>;
  /** v0.2: search Nexus by the package's name. Sends the name; never links. */
  findOnNexus(packageId: string): Promise<NexusFindings>;
  linkPackage(packageId: string, urlOrId: string): Promise<UpdateRow>;
  unlinkPackage(packageId: string): Promise<void>;
  setNexusKey(key: string | null): Promise<NexusSummary>;
  /** v0.3: validate the stored key again (no quota spent); the summary carries the account. */
  refreshNexusAccount(): Promise<NexusSummary>;
  /** v0.3: one mod's files on Nexus, with each file's patch notes. Anonymous. */
  getModFiles(modId: number): Promise<NexusModFiles>;
  /** v0.3: Dragon's Dogma 2 mods on Nexus, by words and an order. Anonymous; sends only `query.text`. */
  browseNexus(query: NexusBrowseQuery): Promise<NexusBrowsePage>;
  /** v0.3 (key): tracked mods and endorsements; cached by the spine unless `refresh`. */
  getNexusStandings(refresh: boolean): Promise<NexusStandings>;
  /** v0.3 (key): track or untrack a mod on Nexus. */
  setTracked(modId: number, tracked: boolean): Promise<NexusStandings>;
  /** v0.3 (key): endorse a mod, or abstain. Nexus may refuse (see NexusReason). */
  setEndorsement(modId: number, status: EndorseStatus): Promise<NexusStandings>;
  /** v0.3: this session's downloads, newest first. */
  getDownloads(): Promise<DownloadView[]>;
  /** v0.3 (key): fetch one file of a mod into the mods folder. A free account's
   *  download waits for the site's link (phase "waiting", `nexusPageUrl` to open). */
  startDownload(modId: number, fileId: number, forPackageId: string | null): Promise<DownloadView>;
  /** v0.3: stop a waiting, queued, linking, downloading or verifying download;
   *  nothing is left behind. Refused once the landing commits (the archive is
   *  being put in place), and for a row that is over or no longer listed. */
  cancelDownload(id: string): Promise<void>;
  /** v0.3: forget the landed, failed and cancelled rows. */
  clearDownloads(): Promise<void>;
  /** v0.3: who answers nxm:// links on this PC. */
  getNxmHandler(): Promise<NxmHandlerView>;
  /** v0.3: make the stone answer nxm:// links (on), or give them back (off). */
  setNxmHandler(on: boolean): Promise<NxmHandlerView>;
  getSettings(): Promise<Settings>;
  setSettings(settings: Settings): Promise<Settings>;
  /** Packages with problems, including those that yielded zero options. */
  troubledPackages(): Promise<PackageSummary[]>;
  /** The Nexus v1 request budget, when a key is set and a v1 call has run. */
  nexusQuota(): Promise<NexusQuota | null>;
  /** v0.2: the Nexus categories and tags the library carries, with option counts. */
  getLabels(): Promise<LabelFacets>;
  /** v0.2: put a parked file back in the game folder. */
  restoreParked(store: string): Promise<ParkedRestoreReport>;
  /** v0.2: delete a parked file from the backup store for good. */
  discardParked(store: string): Promise<void>;
  /** v0.4: every save slot that holds a party, newest first. */
  getMusterSlots(): Promise<MusterSlotsView>;
  /** v0.4: the whole party of one slot; a slot that will not open rejects with a `save` error. */
  getMuster(slotId: string): Promise<MusterView>;
  /** v0.4: open the slot's save folder in Explorer, Rust-side (its path stays out of the window). */
  openSavesFolder(slotId: string): Promise<void>;
  /** v0.5: what keeping only the newest version of one file would uninstall and put away. */
  planKeepNewest(packageId: string): Promise<KeepNewestPlanView>;
  /** v0.5: uninstall the older versions' installs of one file, then put them away. */
  keepNewest(packageId: string): Promise<KeepNewestResultView>;
  /** v0.5: move an older version with no installs to the older-versions folder. */
  putAway(packageId: string): Promise<PutAwayView>;
  /** v0.5: what the older-versions folder holds. */
  getOlderVersions(): Promise<OlderShelfView>;
  /** v0.5: move a put-away version back into the mods folder. */
  bringBack(fileName: string): Promise<BringBackResult>;
  /** v0.5: show the older-versions folder in Explorer, Rust-side (refused before it exists). */
  openOlderVersionsFolder(): Promise<void>;
  /** Subscribe to a backend event; resolves to an unsubscribe function. */
  on<T>(event: RiftEvent, handler: (payload: T) => void): Promise<() => void>;
  openUrl(url: string): Promise<void>;
  revealPath(path: string): Promise<void>;
  /** Native folder picker; null when cancelled. */
  pickFolder(title: string): Promise<string | null>;
}

function insideTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function tauriBackend(): Promise<RiftBackend> {
  const core = await import("@tauri-apps/api/core");
  const { listen } = await import("@tauri-apps/api/event");
  const opener = await import("@tauri-apps/plugin-opener");
  const dialog = await import("@tauri-apps/plugin-dialog");
  const invoke = <T>(cmd: string, args?: Record<string, unknown>): Promise<T> =>
    core.invoke<T>(cmd, args).catch((raw: unknown) => {
      throw asRiftFailure(raw);
    });

  return {
    getOverview: () => invoke("get_overview"),
    rescan: () => invoke("rescan"),
    search: (query) => invoke("search", { query }),
    getOption: (optionId) => invoke("get_option", { optionId }),
    getPackage: (packageId) => invoke("get_package", { packageId }),
    getThumbnail: (optionId, size) => invoke("get_thumbnail", { optionId, size }),
    planInstall: (optionId) => invoke("plan_install", { optionId }),
    install: (optionId) => invoke("install", { optionId }),
    planUninstall: (optionId) => invoke("plan_uninstall", { optionId }),
    uninstall: (optionId) => invoke("uninstall", { optionId }),
    getInstalled: () => invoke("get_installed"),
    reorderPaks: (order) => invoke("reorder_paks", { order }),
    setReframeworkSwitch: (which, on) => invoke("set_reframework_switch", { switch: which, on }),
    setGameRoot: (path) => invoke("set_game_root", { path }),
    getLibrary: () => invoke("get_library"),
    openModsFolder: () => invoke("open_mods_folder"),
    planLaunch: () => invoke("plan_launch"),
    launchGame: () => invoke("launch_game"),
    getUpdates: () => invoke("get_updates"),
    checkUpdates: () => invoke("check_updates"),
    identifyPackages: (packageIds) => invoke("identify_packages", { packageIds }),
    findOnNexus: (packageId) => invoke("find_on_nexus", { packageId }),
    linkPackage: (packageId, urlOrId) => invoke("link_package", { packageId, urlOrId }),
    unlinkPackage: (packageId) => invoke("unlink_package", { packageId }),
    setNexusKey: (key) => invoke("set_nexus_key", { key }),
    refreshNexusAccount: () => invoke("refresh_nexus_account"),
    getModFiles: (modId) => invoke("get_mod_files", { modId }),
    browseNexus: (query) => invoke("browse_nexus", { query }),
    getNexusStandings: (refresh) => invoke("get_nexus_standings", { refresh }),
    setTracked: (modId, tracked) => invoke("set_tracked", { modId, tracked }),
    setEndorsement: (modId, status) => invoke("set_endorsement", { modId, status }),
    getDownloads: () => invoke("get_downloads"),
    startDownload: (modId, fileId, forPackageId) => invoke("start_download", { modId, fileId, forPackageId }),
    cancelDownload: (id) => invoke("cancel_download", { id }),
    clearDownloads: () => invoke("clear_downloads"),
    getNxmHandler: () => invoke("get_nxm_handler"),
    setNxmHandler: (on) => invoke("set_nxm_handler", { on }),
    getSettings: () => invoke("get_settings"),
    setSettings: (settings) => invoke("set_settings", { settings }),
    troubledPackages: () => invoke("get_troubled_packages"),
    nexusQuota: () => invoke("get_nexus_quota"),
    getLabels: () => invoke("get_labels"),
    restoreParked: (store) => invoke("restore_parked", { store }),
    discardParked: (store) => invoke("discard_parked", { store }),
    getMusterSlots: () => invoke("get_muster_slots"),
    getMuster: (slotId) => invoke("get_muster", { slotId }),
    openSavesFolder: (slotId) => invoke("open_saves_folder", { slotId }),
    planKeepNewest: (packageId) => invoke("plan_keep_newest", { packageId }),
    keepNewest: (packageId) => invoke("keep_newest", { packageId }),
    putAway: (packageId) => invoke("put_away", { packageId }),
    getOlderVersions: () => invoke("get_older_versions"),
    bringBack: (fileName) => invoke("bring_back", { fileName }),
    openOlderVersionsFolder: () => invoke("open_older_versions_folder"),
    on: async (event, handler) => {
      const unlisten = await listen(event, (e) => {
        handler(e.payload as never);
      });
      return unlisten;
    },
    openUrl: (url) => opener.openUrl(url),
    revealPath: (path) => opener.revealItemInDir(path),
    pickFolder: async (title) => {
      const picked = await dialog.open({ directory: true, multiple: false, title });
      return typeof picked === "string" ? picked : null;
    },
  };
}

let backend: Promise<RiftBackend> | null = null;

/** The backend for this page — the real spine inside Tauri, the mock outside. */
export function rift(): Promise<RiftBackend> {
  backend ??= insideTauri() ? tauriBackend() : import("./mock/index").then((m) => m.createMockBackend());
  return backend;
}

/** Tests swap in their own backend. */
export function setRiftBackend(next: RiftBackend | null): void {
  backend = next ? Promise.resolve(next) : null;
}
