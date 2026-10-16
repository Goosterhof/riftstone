// The stone's own state: what get_overview says about the game, REFramework,
// Fluffy, the roots and the library; the scan in flight; the packages that
// would not open. Other composables read it; it reads only the services.

import { computed, readonly, ref } from "vue";
import type { Overview, PackageSummary, ScanProgressEvent } from "@/types/rift";
import { asRiftFailure, rift } from "@/services/rift";
import { troubledProbe } from "@/services/pending";
import { voice, type Voiced } from "@/services/voice";

/** waking → (nogame | found | ready). "found" = first launch, nothing scanned yet. */
export type WakePhase = "waking" | "nogame" | "found" | "ready";

const overview = ref<Overview | null>(null);
const overviewFailure = ref<Voiced | null>(null);
const scanProgress = ref<ScanProgressEvent | null>(null);
const scanRequested = ref(false);
const troubled = ref<PackageSummary[]>([]);
const hiddenTroubled = ref<string[]>([]);
/** §8.1.1 the trouble fold: closed at every launch, held for the session,
 *  never written to settings (a remembered "open" rebuilds the wall). */
const troubleOpen = ref(false);
/** Packages per root, counted from an unfiltered search (contract request: carry it on LibraryRoot). */
const rootCounts = ref<Record<string, number>>({});
const dismissedBuild = ref<string | null>(null);

const phase = computed<WakePhase>(() => {
  const o = overview.value;
  if (!o) return "waking";
  if (!o.game) return "nogame";
  if (o.library.lastScanMs === null && !scanRequested.value && o.scan.state === "idle") return "found";
  return "ready";
});

const scanning = computed(() => scanProgress.value !== null || overview.value?.scan.state === "scanning");

const scanReading = computed<ScanProgressEvent | null>(() => {
  if (scanProgress.value) return scanProgress.value;
  const scan = overview.value?.scan;
  return scan?.state === "scanning" ? { done: scan.done, total: scan.total, current: scan.current } : null;
});

const visibleTroubled = computed(() => troubled.value.filter((p) => !hiddenTroubled.value.includes(p.id)));

/** The last card left: the next trouble of the session arrives folded. */
function closeIfEmpty(): void {
  if (!visibleTroubled.value.length) troubleOpen.value = false;
}

/** The build-changed banner, unless the user said "I've tested it". */
const buildChange = computed(() => {
  const changed = overview.value?.game?.buildChanged ?? null;
  if (!changed || dismissedBuild.value === changed.now) return null;
  return changed;
});

async function refreshOverview(): Promise<void> {
  try {
    const backend = await rift();
    overview.value = await backend.getOverview();
    overviewFailure.value = null;
  } catch (raw) {
    overviewFailure.value = voice(asRiftFailure(raw), "game");
  }
}

async function refreshTroubled(): Promise<void> {
  const backend = await rift();
  const probe = troubledProbe(backend);
  troubled.value = probe ? await probe() : [];
  closeIfEmpty();
}

async function refreshRootCounts(): Promise<void> {
  const backend = await rift();
  const all = await backend.search({
    text: "",
    filters: { installed: null, rootIds: [], includeMenuOnly: true, categories: [], tags: [] },
    limit: 1_000_000,
  });
  const seen = new Map<string, Set<string>>();
  for (const hit of all) {
    const set = seen.get(hit.rootId) ?? new Set<string>();
    set.add(hit.packageId);
    seen.set(hit.rootId, set);
  }
  rootCounts.value = Object.fromEntries([...seen].map(([root, set]) => [root, set.size]));
}

async function startScan(): Promise<void> {
  scanRequested.value = true;
  const backend = await rift();
  scanProgress.value = { done: 0, total: overview.value?.library.packages ?? 0, current: "" };
  await backend.rescan();
}

function onScanProgress(event: ScanProgressEvent): void {
  scanProgress.value = event;
}

function onLibraryChanged(): void {
  scanProgress.value = null;
}

export function useStone() {
  return {
    overview,
    overviewFailure,
    phase,
    scanning,
    scanReading,
    troubled: visibleTroubled,
    troubleOpen: readonly(troubleOpen),
    toggleTrouble: () => {
      troubleOpen.value = !troubleOpen.value;
    },
    rootCounts,
    buildChange,
    refreshOverview,
    refreshTroubled,
    refreshRootCounts,
    startScan,
    onScanProgress,
    onLibraryChanged,
    hideTroubled: (id: string) => {
      hiddenTroubled.value = [...hiddenTroubled.value, id];
      closeIfEmpty();
    },
    dismissBuildChange: () => {
      dismissedBuild.value = overview.value?.game?.buildChanged?.now ?? null;
    },
  };
}

export function resetStone(): void {
  overview.value = null;
  overviewFailure.value = null;
  scanProgress.value = null;
  scanRequested.value = false;
  troubled.value = [];
  hiddenTroubled.value = [];
  troubleOpen.value = false;
  rootCounts.value = {};
  dismissedBuild.value = null;
}
