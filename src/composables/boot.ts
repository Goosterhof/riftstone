// Wakes the stone: loads the first frame, subscribes to the spine's events
// and routes each one to the composables that care. The only module that
// knows every composable — none of them import it, so no cycles.

import { EVENTS, type InstallProgressEvent, type ScanProgressEvent, type UpdatesProgressEvent } from "@/types/rift";
import { rift } from "@/services/rift";
import { resetFarWorld, useFarWorld } from "./useFarWorld";
import { resetInstalled, useInstalled } from "./useInstalled";
import { resetLabels, useLabels } from "./useLabels";
import { resetNavigation, useNavigation } from "./useNavigation";
import { resetPackage, usePackage } from "./usePackage";
import { resetSearch, useSearch } from "./useSearch";
import { resetSettings, useSettings } from "./useSettings";
import { resetStone, useStone } from "./useStone";
import { resetThumbnails } from "./useThumbnails";
import { resetToasts } from "./useToasts";
import { resetWorld, useWorld } from "./useWorld";

let unsubscribers: (() => void)[] = [];

async function onLibraryChanged(): Promise<void> {
  const stone = useStone();
  stone.onLibraryChanged();
  await Promise.all([
    stone.refreshOverview(),
    stone.refreshTroubled(),
    stone.refreshRootCounts(),
    useSearch().run(),
    usePackage().reload(),
    useLabels().load(),
  ]);
}

async function onGameChanged(): Promise<void> {
  const tasks: Promise<void>[] = [
    useStone().refreshOverview(),
    useSearch().run(),
    useWorld().refresh(),
    usePackage().reload(),
  ];
  if (useInstalled().view.value || useNavigation().page.value === "world") tasks.push(useInstalled().load());
  await Promise.all(tasks);
}

/** A check found (or cleared) updates, or a link changed: the far world,
 *  every card's badge and labels, and the open detail — its labels decide
 *  the "adult" veil, so a stale detail would show what its card now blurs. */
async function onUpdatesChanged(): Promise<void> {
  await Promise.all([
    useFarWorld().load(),
    useSearch().run(),
    useStone().refreshOverview(),
    useLabels().load(),
    useWorld().refresh(),
  ]);
}

async function subscribe(): Promise<void> {
  const backend = await rift();
  const stone = useStone();
  unsubscribers = await Promise.all([
    backend.on<ScanProgressEvent>(EVENTS.scanProgress, (e) => stone.onScanProgress(e)),
    backend.on(EVENTS.libraryChanged, () => void onLibraryChanged()),
    backend.on<InstallProgressEvent>(EVENTS.installProgress, (e) => useWorld().onInstallProgress(e)),
    backend.on(EVENTS.gameChanged, () => void onGameChanged()),
    backend.on<UpdatesProgressEvent>(EVENTS.updatesProgress, (e) => useFarWorld().onProgress(e)),
    backend.on(EVENTS.updatesChanged, () => void onUpdatesChanged()),
  ]);
}

/** First frame: overview, settings, search, the troubled list, root counts. */
export async function wakeRiftstone(): Promise<void> {
  await subscribe();
  const stone = useStone();
  await stone.refreshOverview();
  await Promise.all([
    useSettings().loadSettings(),
    useSearch().run(),
    stone.refreshTroubled(),
    stone.refreshRootCounts(),
    useFarWorld().load(),
    useLabels().load(),
  ]);
}

export function sleepRiftstone(): void {
  for (const off of unsubscribers) off();
  unsubscribers = [];
}

/** Tests: every composable back to its first frame. */
export function resetRiftstone(): void {
  sleepRiftstone();
  resetStone();
  resetSearch();
  resetWorld();
  resetInstalled();
  resetFarWorld();
  resetLabels();
  resetSettings();
  resetNavigation();
  resetToasts();
  resetThumbnails();
  resetPackage();
}
