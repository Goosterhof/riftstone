// The stone's own state: what get_overview says about the game, REFramework,
// the mods folder and the library; the scan in flight; the packages that
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

/** v0.3: the one library folder (`<install dir>\\mods`), and the spine's
 *  `mods` check on it: absent while the folder holds packages; Info, action
 *  openModsFolder, when it is empty; Warn, action rescan, when it cannot be
 *  listed; Warn, action openModsFolder, when it went while the stone ran
 *  (moved or deleted); Error, no action, when it was missing at launch and
 *  could not be made (WINDOW-CONTRACT § Check ids). */
const modsFolder = computed(() => overview.value?.mods ?? null);
const modsCheck = computed(() => overview.value?.checks.find((c) => c.id === "mods") ?? null);
/** The `mods` check's own way on is Open mods folder: the folder is empty, or
 *  it went and the press makes it again, empty, and watches it. */
const modsWantOpen = computed(() => modsCheck.value?.action?.kind === "openModsFolder");
/** The folder is not there now, and why: `gone`, it went while the stone ran
 *  (Open mods folder makes it again); `unmade`, it could not be made at launch
 *  (nothing to open: the check names the cause). Null while it is there. */
const modsMissing = computed<"gone" | "unmade" | null>(() => {
  if (modsFolder.value?.exists !== false) return null;
  return modsWantOpen.value ? "gone" : "unmade";
});

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
    modsFolder,
    modsCheck,
    modsWantOpen,
    modsMissing,
    troubleOpen: readonly(troubleOpen),
    toggleTrouble: () => {
      troubleOpen.value = !troubleOpen.value;
    },
    buildChange,
    refreshOverview,
    refreshTroubled,
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
  dismissedBuild.value = null;
}
