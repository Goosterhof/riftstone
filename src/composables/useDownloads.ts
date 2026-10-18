// DOWNLOADS (v0.3): this session's downloads into the mods folder, newest
// first (get_downloads), read again on rift://downloads-changed; the bytes in
// flight from rift://download-progress (at most one per 200 ms per download,
// plus the last), laid over a DOWNLOADING row until the next read. Only the
// bytes: the phase is always the row's own. The spine sends progress only
// while bytes move (its phase is always "downloading"), so an event laid over
// the phase would hide "verifying" — "Checking the file" (R16.18) — for good.
//
// A premium account's download runs in the stone: queued → linking →
// downloading → verifying → landed. A free account's waits (phase "waiting")
// for the site's Mod Manager Download link: the window opens the file's page
// (`nexusPageUrl`), and the link that button sends (nxm://) resumes it. One
// download runs at a time. The stone never installs a download by itself:
// a landed archive is a package in the Rift like any other.
//
// The rows say what changed: a download that landed or failed since the last
// read is told once, as a toast, in the stone's voice. A landed file that is
// no archive (a raw .pak, an .exe) has no package: the Rift never lists it, so
// its way on is the file in Explorer, never the Rift.

import { computed, nextTick, ref, watch } from "vue";
import type { DownloadPhase, DownloadProgressEvent, DownloadView } from "@/types/rift";
import { asRiftFailure, rift } from "@/services/rift";
import { landedLine } from "@/services/ledger";
import { voice, voicedLine, type Voiced } from "@/services/voice";
import { useLibrary } from "./useLibrary";
import { useNavigation } from "./useNavigation";
import { useNxmHandler } from "./useNxmHandler";
import { useSearch } from "./useSearch";
import { useStone } from "./useStone";
import { useToasts, type ToastAction } from "./useToasts";

/** A download still on its way (the stone can cancel these). */
const IN_FLIGHT: ReadonlySet<DownloadPhase> = new Set(["waiting", "queued", "linking", "downloading", "verifying"]);
/** A download that is over (clear_downloads forgets these). */
const OVER: ReadonlySet<DownloadPhase> = new Set(["landed", "failed", "cancelled"]);
/** How long Show in the Rift waits for the scan that shelves a landed archive. */
const SHELF_WAIT_MS = 5_000;

/** Why a download door is held while that file's download is on its way. */
export const HELD_WHILE_DOWNLOADING = "Downloading: see DOWNLOADS";

/** A free account's start: the file's page is open, and the site's button
 *  reaches the stone (the stone answers nxm:// links, or this PC cannot say). */
const PAGE_OPEN_LINE =
  "The file's Nexus page is open: press Mod Manager Download there, and the stone takes the file from that link.";
/** ...and while nothing hands that button's link to the stone (Chaos D1). */
const PAGE_OPEN_SILENT_LINE =
  "The file's Nexus page is open, but nothing hands Nexus's button to the stone now. Let the stone answer it first, then press Mod Manager Download there.";

const rows = ref<DownloadView[]>([]);
const live = ref<Record<string, DownloadProgressEvent>>({});
const failure = ref<Voiced | null>(null);
/** What is being pressed: `start:<mod>:<file>`, `cancel:<id>`, or `clear`. */
const busy = ref<string | null>(null);
/** Each row's phase at the last read: a change since is told once. */
let seen = new Map<string, DownloadPhase>();
let requestSeq = 0;

/** The rows, a downloading one with the bytes in flight laid over it: its
 *  bytes only (never fewer than the row's own), never its phase. */
const downloads = computed<DownloadView[]>(() =>
  rows.value.map((row) => {
    const progress = live.value[row.id] as DownloadProgressEvent | undefined;
    if (!progress || row.phase !== "downloading") return row;
    return {
      ...row,
      bytesDone: Math.max(row.bytesDone, progress.bytesDone),
      bytesTotal: progress.bytesTotal ?? row.bytesTotal,
    };
  }),
);

/** A download's file on its Nexus page, where Mod Manager Download makes a
 *  fresh link (the waiting row's own page, else the same address). */
export function filePageUrl(d: Pick<DownloadView, "modId" | "fileId" | "nexusPageUrl">): string {
  return (
    d.nexusPageUrl ?? `https://www.nexusmods.com/dragonsdogma2/mods/${d.modId}?tab=files&file_id=${d.fileId}&nmm=1`
  );
}

const inFlight = computed(() => downloads.value.filter((d) => IN_FLIGHT.has(d.phase)));
const waiting = computed(() => downloads.value.filter((d) => d.phase === "waiting"));
const finished = computed(() => downloads.value.filter((d) => OVER.has(d.phase)));
/** The one download moving bytes now, if any. */
const current = computed(() => downloads.value.find((d) => d.phase === "downloading") ?? null);

/** Each share done, 0–1; null while the total is unknown. */
export function shareDone(d: Pick<DownloadView, "bytesDone" | "bytesTotal">): number | null {
  if (d.bytesTotal === null || d.bytesTotal <= 0) return null;
  return Math.min(1, Math.max(0, d.bytesDone / d.bytesTotal));
}

/** A row that landed or failed since the last read speaks once. */
function tell(next: DownloadView[]): void {
  const { push } = useToasts();
  for (const d of next) {
    const before = seen.get(d.id);
    if (before === undefined || before === d.phase) continue;
    // R16.18 / Bold Choice 8: a landing is cyan, never gold; §9 says it once,
    // with the way to the package, or to the file when it is no archive. An
    // archive the folder already had is the shelf row's to say.
    if (d.phase === "landed" && !d.alreadyHad) push("rift", landedLine(d), landedDoors(d));
    else if (d.phase === "failed" && d.failure && !farWorldShown())
      push("ember", voicedLine(voice(asRiftFailure(d.failure), "download")));
  }
}

/** A landing's one door: its package in the Rift, or, for a file that is no
 *  archive (no package), the file in its folder. None while neither is known. */
function landedDoors(d: DownloadView): ToastAction[] {
  const packageId = d.packageId;
  if (packageId) return [{ label: "Show in the Rift", run: () => showInRift(packageId) }];
  return landedPath(d) ? [{ label: "Show in folder", run: () => showInFolder(d) }] : [];
}

/** THE FAR WORLD is on screen: its shelf row already says a failure, so no
 *  toast repeats it (§9 Download failed lives on the row). */
const farWorldShown = (): boolean => useNavigation().page.value === "nexus";

/** Where a landed file sits: the mods folder's path and the name it landed
 *  under, joined with the folder's own separator. Null before the overview
 *  names the folder, or for a row with no landed name. */
export function landedPath(d: Pick<DownloadView, "landedAs">): string | null {
  const folder = useStone().modsFolder.value?.path;
  if (!folder || !d.landedAs) return null;
  const separator = folder.includes("\\") ? "\\" : "/";
  return `${folder.replace(/[\\/]+$/, "")}${separator}${d.landedAs}`;
}

/** Show in folder: Explorer opens on the landed file (the way to a file the
 *  Rift does not list). A refusal is voiced, never swallowed. */
async function showInFolder(d: Pick<DownloadView, "landedAs">): Promise<void> {
  const path = landedPath(d);
  if (!path) return;
  try {
    const backend = await rift();
    await backend.revealPath(path);
  } catch (raw) {
    useToasts().push("ember", voicedLine(voice(asRiftFailure(raw), "modsFolder")));
  }
}

/** A download still on its way for this package's update row, or of this very
 *  file of this mod: the spine answers a second start of a file already on
 *  its way with that same row, so a press would do nothing. */
function unfinishedFor(packageId: string | null, modId: number, fileId: number): DownloadView | null {
  return (
    downloads.value.find(
      (d) =>
        IN_FLIGHT.has(d.phase) &&
        ((packageId !== null && d.forPackageId === packageId) || (d.modId === modId && d.fileId === fileId)),
    ) ?? null
  );
}

/** A free account's start opened the file's page: say what its Mod Manager
 *  Download button will do. It reaches the stone only while the stone answers
 *  nxm:// links; until then the toast says so and carries the switch's act.
 *  An unread handler is read first, so the line is never a guess. */
async function tellPageOpen(): Promise<void> {
  const nxm = useNxmHandler();
  if (!nxm.handler.value) await nxm.load();
  if (!nxm.silent.value) {
    useToasts().push("rift", PAGE_OPEN_LINE);
    return;
  }
  useToasts().push("rift", PAGE_OPEN_SILENT_LINE, [
    {
      label: "Answer Nexus's button",
      run: async () => {
        await nxm.set(true);
      },
    },
  ]);
}

/** The bytes in flight outlive a read only while the row still downloads: the
 *  spine's row carries the same count (it moves both at once), so nothing is
 *  lost when a later phase lets them go. */
function remember(next: DownloadView[]): void {
  seen = new Map(next.map((d) => [d.id, d.phase]));
  const kept: Record<string, DownloadProgressEvent> = {};
  for (const d of next) {
    const progress = live.value[d.id] as DownloadProgressEvent | undefined;
    if (progress && d.phase === "downloading") kept[d.id] = progress;
  }
  live.value = kept;
}

async function load(): Promise<void> {
  const mine = ++requestSeq;
  try {
    const backend = await rift();
    const next = await backend.getDownloads();
    if (mine !== requestSeq) return;
    tell(next);
    rows.value = next;
    remember(next);
  } catch {
    // The list is the session's memory of its own downloads: when it cannot
    // be read, the rows on screen stay until the next downloads-changed.
  }
}

/** A row the window just made, before the next read lists it. */
function upsert(row: DownloadView): void {
  seen.set(row.id, row.phase);
  rows.value = [row, ...rows.value.filter((r) => r.id !== row.id)];
}

function failedAct(raw: unknown): void {
  failure.value = voice(asRiftFailure(raw), "download");
  // a press that failed before any row exists has only the toast to say it;
  // the same sentence already on the shelf is never stacked again
  const line = voicedLine(failure.value);
  const toasts = useToasts();
  if (!toasts.toasts.value.some((t) => t.line === line)) toasts.push("ember", line);
}

/** Fetch one file of a mod into the mods folder (needs the key). A free
 *  account's download waits for the site's link: its file page is opened,
 *  where Mod Manager Download sends the link back to the stone. */
async function start(modId: number, fileId: number, forPackageId: string | null = null): Promise<DownloadView | null> {
  busy.value = `start:${modId}:${fileId}`;
  failure.value = null;
  try {
    const backend = await rift();
    const row = await backend.startDownload(modId, fileId, forPackageId);
    upsert(row);
    if (row.phase === "waiting" && row.nexusPageUrl) {
      await backend.openUrl(row.nexusPageUrl);
      await tellPageOpen();
    }
    return row;
  } catch (raw) {
    failedAct(raw);
    return null;
  } finally {
    busy.value = null;
  }
}

/** Open a waiting download's file page again (its tab was closed, say). */
async function openPage(id: string): Promise<void> {
  const url = rows.value.find((r) => r.id === id)?.nexusPageUrl;
  if (!url) return;
  const backend = await rift();
  await backend.openUrl(url);
}

async function cancel(id: string): Promise<boolean> {
  busy.value = `cancel:${id}`;
  failure.value = null;
  try {
    const backend = await rift();
    await backend.cancelDownload(id);
    await load();
    return true;
  } catch (raw) {
    failedAct(raw);
    await load();
    return false;
  } finally {
    busy.value = null;
  }
}

/** Forget the landed, failed and cancelled rows. */
async function clear(): Promise<void> {
  busy.value = "clear";
  failure.value = null;
  try {
    const backend = await rift();
    await backend.clearDownloads();
    await load();
  } catch (raw) {
    failedAct(raw);
  } finally {
    busy.value = null;
  }
}

/** The landed archive is on the shelf: get_library lists its package (the
 *  scan the landing asked for may still be on its way). False after a wait. */
async function shelved(packageId: string): Promise<boolean> {
  const library = useLibrary();
  const has = (): boolean => library.packages.value.some((p) => p.package.id === packageId);
  if (has()) return true;
  await library.load();
  if (has()) return true;
  return new Promise<boolean>((resolve) => {
    // both run later than both declarations: the watch is not immediate
    const settle = (found: boolean): void => {
      clearTimeout(timer);
      stop();
      resolve(found);
    };
    const timer = setTimeout(() => settle(false), SHELF_WAIT_MS);
    const stop = watch(has, (now) => {
      if (now) settle(true);
    });
  });
}

/** The keyboard onto a tree row once it is drawn; else onto the well, never <body>. */
async function focusTreeRow(key: string): Promise<void> {
  for (let i = 0; i < 10; i += 1) {
    await nextTick();
    const row = document.querySelector<HTMLElement>(`#rift-tree [data-key="${CSS.escape(key)}"]`);
    if (row) {
      row.focus({ preventScroll: true });
      try {
        row.scrollIntoView({ block: "nearest" });
      } catch {
        // a bench without layout has nothing to scroll
      }
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 16));
  }
  document.getElementById("rift-search")?.focus();
}

/** R16.18 Show in the Rift: THE RIFT on its tree, the landed package opened in
 *  the tree's memory and the focus on its row. The tree is the idle Rift, so
 *  whatever narrowed the answers goes: the words, the state facet, menu-only
 *  and the label chips (the tree's expansion stays). A landing is not an
 *  install: nothing crosses into your world here. */
async function showInRift(packageId: string): Promise<void> {
  const nav = useNavigation();
  const library = useLibrary();
  const search = useSearch();
  nav.go("rift");
  nav.backToResults();
  if (!search.idle.value || search.text.value) await search.clearAll();
  await shelved(packageId);
  library.expand(packageId);
  library.focusedKey.value = packageId;
  await focusTreeRow(packageId);
}

/** The newest download started from an update row for this package. */
function forPackage(packageId: string): DownloadView | null {
  return downloads.value.find((d) => d.forPackageId === packageId) ?? null;
}

function onProgress(event: DownloadProgressEvent): void {
  live.value = { ...live.value, [event.id]: event };
}

export function useDownloads() {
  return {
    rows,
    downloads,
    inFlight,
    waiting,
    finished,
    current,
    failure,
    busy,
    load,
    start,
    openPage,
    cancel,
    clear,
    onProgress,
    showInRift,
    showInFolder,
    forPackage,
    unfinishedFor,
    /** rift://downloads-changed: a row appeared, changed phase or was cleared. */
    onChanged: () => load(),
    canCancel: (d: Pick<DownloadView, "phase">) => IN_FLIGHT.has(d.phase),
  };
}

export function resetDownloads(): void {
  requestSeq = 0;
  rows.value = [];
  live.value = {};
  failure.value = null;
  busy.value = null;
  seen = new Map();
}
