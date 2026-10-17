// DOWNLOADS (v0.3): this session's downloads into the mods folder, newest
// first (get_downloads), read again on rift://downloads-changed; the bytes in
// flight from rift://download-progress (at most one per 200 ms per download,
// plus the last), laid over the rows until the next read.
//
// A premium account's download runs in the stone: queued → linking →
// downloading → verifying → landed. A free account's waits (phase "waiting")
// for the site's Mod Manager Download link: the window opens the file's page
// (`nexusPageUrl`), and the link that button sends (nxm://) resumes it. One
// download runs at a time. The stone never installs a download by itself:
// a landed archive is a package in the Rift like any other.
//
// The rows say what changed: a download that landed or failed since the last
// read is told once, as a toast, in the stone's voice.

import { computed, nextTick, ref } from "vue";
import type { DownloadPhase, DownloadProgressEvent, DownloadView } from "@/types/rift";
import { asRiftFailure, rift } from "@/services/rift";
import { landedLine } from "@/services/ledger";
import { voice, voicedLine, type Voiced } from "@/services/voice";
import { useLibrary } from "./useLibrary";
import { useNavigation } from "./useNavigation";
import { useSearch } from "./useSearch";
import { useToasts } from "./useToasts";

/** A download still on its way (the stone can cancel these). */
const IN_FLIGHT: ReadonlySet<DownloadPhase> = new Set(["waiting", "queued", "linking", "downloading", "verifying"]);
/** A download that is over (clear_downloads forgets these). */
const OVER: ReadonlySet<DownloadPhase> = new Set(["landed", "failed", "cancelled"]);

const rows = ref<DownloadView[]>([]);
const live = ref<Record<string, DownloadProgressEvent>>({});
const failure = ref<Voiced | null>(null);
/** What is being pressed: `start:<mod>:<file>`, `cancel:<id>`, or `clear`. */
const busy = ref<string | null>(null);
/** Each row's phase at the last read: a change since is told once. */
let seen = new Map<string, DownloadPhase>();
let requestSeq = 0;

/** The rows with the bytes in flight laid over them. */
const downloads = computed<DownloadView[]>(() =>
  rows.value.map((row) => {
    const progress = live.value[row.id] as DownloadProgressEvent | undefined;
    if (!progress || OVER.has(row.phase)) return row;
    return {
      ...row,
      phase: progress.phase,
      bytesDone: progress.bytesDone,
      bytesTotal: progress.bytesTotal ?? row.bytesTotal,
    };
  }),
);

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
    if (d.phase === "landed") push("world", landedLine(d));
    else if (d.phase === "failed" && d.failure) push("ember", voicedLine(voice(asRiftFailure(d.failure), "download")));
  }
}

function remember(next: DownloadView[]): void {
  seen = new Map(next.map((d) => [d.id, d.phase]));
  const kept: Record<string, DownloadProgressEvent> = {};
  for (const d of next) {
    const progress = live.value[d.id] as DownloadProgressEvent | undefined;
    if (progress && IN_FLIGHT.has(d.phase)) kept[d.id] = progress;
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
  useToasts().push("ember", voicedLine(failure.value));
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
      useToasts().push(
        "rift",
        "The file's Nexus page is open: press Mod Manager Download there, and the stone takes the file from that link.",
      );
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

/** R16.18 Show in the Rift: THE RIFT on its tree (the well emptied if it held
 *  words, so the tree is what shows), the landed package opened in the tree's
 *  memory, and the focus on its row. A landing is not an install: nothing
 *  crosses into your world here. */
async function showInRift(packageId: string): Promise<void> {
  const nav = useNavigation();
  const library = useLibrary();
  const search = useSearch();
  if (search.text.value) search.clearText();
  nav.go("rift");
  nav.backToResults();
  if (!library.packages.value.some((p) => p.package.id === packageId)) await library.load();
  library.expand(packageId);
  library.focusedKey.value = packageId;
  for (let i = 0; i < 3; i += 1) {
    await nextTick();
    const row = document.querySelector<HTMLElement>(`[data-key="${CSS.escape(packageId)}"]`);
    if (row) {
      row.focus({ preventScroll: true });
      try {
        row.scrollIntoView({ block: "nearest" });
      } catch {
        // a bench without layout has nothing to scroll
      }
      return;
    }
  }
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
    forPackage,
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
