// Wakes the stone: loads the first frame, subscribes to the spine's events
// and routes each one to the composables that care. The only module that
// knows every composable — none of them import it, so no cycles.

import {
  EVENTS,
  type DownloadProgressEvent,
  type InstallProgressEvent,
  type NxmReceivedEvent,
  type ScanProgressEvent,
  type UpdatesProgressEvent,
} from "@/types/rift";
import { asRiftFailure, rift } from "@/services/rift";
import { voice, voicedLine } from "@/services/voice";
import { resetDownloads, useDownloads } from "./useDownloads";
import { resetFarWorld, useFarWorld } from "./useFarWorld";
import { resetInstalled, useInstalled } from "./useInstalled";
import { resetLabels, useLabels } from "./useLabels";
import { resetLaunch, useLaunch } from "./useLaunch";
import { resetLibrary, useLibrary } from "./useLibrary";
import { resetNavigation, useNavigation } from "./useNavigation";
import { resetNexusAccount } from "./useNexusAccount";
import { resetNxmHandler } from "./useNxmHandler";
import { resetPackage, usePackage } from "./usePackage";
import { resetPreflight } from "./usePreflight";
import { resetSearch, useSearch } from "./useSearch";
import { resetSettings, useSettings } from "./useSettings";
import { resetStone, useStone } from "./useStone";
import { resetThumbnails } from "./useThumbnails";
import { resetToasts, useToasts } from "./useToasts";
import { resetWorld, useWorld } from "./useWorld";

let unsubscribers: (() => void)[] = [];

async function onLibraryChanged(): Promise<void> {
  const stone = useStone();
  stone.onLibraryChanged();
  await Promise.all([
    stone.refreshOverview(),
    stone.refreshTroubled(),
    useSearch().run(),
    useLibrary().load(),
    usePackage().reload(),
    useLabels().load(),
  ]);
}

/** v0.3: also when the game watcher sees Dragon's Dogma 2 start or stop. */
async function onGameChanged(): Promise<void> {
  const tasks: Promise<void>[] = [
    useStone().refreshOverview(),
    useSearch().run(),
    useLibrary().load(),
    useWorld().refresh(),
    usePackage().reload(),
  ];
  if (useInstalled().view.value || useNavigation().page.value === "world") tasks.push(useInstalled().load());
  await Promise.all(tasks);
  // after the overview: the running state Play reads is in it
  await useLaunch().onGameChanged();
}

/** A check found (or cleared) updates, a link changed, a download landed or
 *  the account's standing changed: the far world, every card's badge and
 *  labels, the tree, and the open detail — its labels decide the "adult"
 *  veil, so a stale detail would show what its card now blurs. */
async function onUpdatesChanged(): Promise<void> {
  await Promise.all([
    useFarWorld().load(),
    useSearch().run(),
    useStone().refreshOverview(),
    useLabels().load(),
    useLibrary().load(),
    useWorld().refresh(),
  ]);
}

/** v0.3: an nxm:// link reached the stone (the site's Mod Manager Download
 *  button). The spine brought the window forward; the window says what became
 *  of the link and shows THE FAR WORLD, where downloads live. A link that was
 *  neither taken nor refused (nxm://premium only re-validates the account)
 *  has nothing to say. */
async function onNxmReceived(event: NxmReceivedEvent): Promise<void> {
  const { push } = useToasts();
  if (event.accepted) {
    push("rift", "A Mod Manager Download link reached the stone: the download is on its way to your mods folder.");
  } else if (event.refusal) {
    push("ember", voicedLine(voice(asRiftFailure(event.refusal), "nxm")));
  }
  useNavigation().go("nexus");
  await useDownloads().load();
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
    backend.on(EVENTS.downloadsChanged, () => void useDownloads().onChanged()),
    backend.on<DownloadProgressEvent>(EVENTS.downloadProgress, (e) => useDownloads().onProgress(e)),
    backend.on<NxmReceivedEvent>(EVENTS.nxmReceived, (e) => void onNxmReceived(e)),
  ]);
}

/** First frame: overview, settings, search, the library tree, the troubled
 *  list, the far world, the labels and this session's downloads. */
export async function wakeRiftstone(): Promise<void> {
  await subscribe();
  const stone = useStone();
  await stone.refreshOverview();
  await Promise.all([
    useSettings().loadSettings(),
    useSearch().run(),
    useLibrary().load(),
    stone.refreshTroubled(),
    useFarWorld().load(),
    useLabels().load(),
    useDownloads().load(),
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
  resetLibrary();
  resetWorld();
  resetInstalled();
  resetFarWorld();
  resetLabels();
  resetSettings();
  resetNavigation();
  resetToasts();
  resetThumbnails();
  resetPackage();
  resetLaunch();
  resetDownloads();
  resetNexusAccount();
  resetNxmHandler();
  resetPreflight();
}
