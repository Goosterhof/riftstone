// The only door to the Rust spine. Components never import @tauri-apps/*;
// they use composables, which use this service.
//
// Two backends share one interface: the real one (Tauri invoke + events) and
// the mock (src/services/mock/), chosen when the page runs outside Tauri —
// `npm run dev` in a plain browser, the headless render harness, and the
// vitest suite. The mock is a separate chunk, loaded only when needed.

import type {
  AdoptResultView,
  GameSummary,
  InstallPlanView,
  InstallResultView,
  InstalledView,
  LibraryRoot,
  Md5Result,
  LabelFacets,
  NexusFindings,
  NexusQuota,
  ParkedRestoreReport,
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
  adoptFluffy(): Promise<AdoptResultView>;
  setReframeworkSwitch(which: "looseFileLoader" | "pakDirectory", on: boolean): Promise<void>;
  addRoot(path: string): Promise<LibraryRoot>;
  removeRoot(rootId: string): Promise<void>;
  setGameRoot(path: string): Promise<GameSummary>;
  getUpdates(): Promise<UpdatesView>;
  checkUpdates(): Promise<UpdatesView>;
  identifyPackages(packageIds: string[]): Promise<Md5Result[]>;
  /** v0.2: search Nexus by the package's name. Sends the name; never links. */
  findOnNexus(packageId: string): Promise<NexusFindings>;
  linkPackage(packageId: string, urlOrId: string): Promise<UpdateRow>;
  unlinkPackage(packageId: string): Promise<void>;
  setNexusKey(key: string | null): Promise<NexusSummary>;
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
    adoptFluffy: () => invoke("adopt_fluffy"),
    setReframeworkSwitch: (which, on) => invoke("set_reframework_switch", { switch: which, on }),
    addRoot: (path) => invoke("add_root", { path }),
    removeRoot: (rootId) => invoke("remove_root", { rootId }),
    setGameRoot: (path) => invoke("set_game_root", { path }),
    getUpdates: () => invoke("get_updates"),
    checkUpdates: () => invoke("check_updates"),
    identifyPackages: (packageIds) => invoke("identify_packages", { packageIds }),
    findOnNexus: (packageId) => invoke("find_on_nexus", { packageId }),
    linkPackage: (packageId, urlOrId) => invoke("link_package", { packageId, urlOrId }),
    unlinkPackage: (packageId) => invoke("unlink_package", { packageId }),
    setNexusKey: (key) => invoke("set_nexus_key", { key }),
    getSettings: () => invoke("get_settings"),
    setSettings: (settings) => invoke("set_settings", { settings }),
    troubledPackages: () => invoke("get_troubled_packages"),
    nexusQuota: () => invoke("get_nexus_quota"),
    getLabels: () => invoke("get_labels"),
    restoreParked: (store) => invoke("restore_parked", { store }),
    discardParked: (store) => invoke("discard_parked", { store }),
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
