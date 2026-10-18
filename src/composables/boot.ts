// Wakes the stone: loads the first frame, subscribes to the spine's events
// and routes each one to the composables that care. The only module that
// knows every composable — none of them import it, so no cycles.

import { nextTick } from "vue";
import {
  EVENTS,
  type DownloadProgressEvent,
  type InstallProgressEvent,
  type NxmReceivedEvent,
  type ScanProgressEvent,
  type UpdatesProgressEvent,
} from "@/types/rift";
import { asRiftFailure, rift } from "@/services/rift";
import { voice, voicedLine, type Voiced } from "@/services/voice";
import { filePageUrl, resetDownloads, useDownloads } from "./useDownloads";
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
import { resetToasts, useToasts, type ToastAction } from "./useToasts";
import { resetWorld, useWorld } from "./useWorld";

let unsubscribers: (() => void)[] = [];

/** A scan finished. The far world's rows are read again too: a landed
 *  download's package is linked (rift://updates-changed) BEFORE the scan
 *  shelves it, so only after this event can get_updates list it under
 *  UPDATES. get_updates is the cached state, no network (WINDOW-CONTRACT). */
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
    useFarWorld().load(),
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
  const downloads = useDownloads();
  useNavigation().go("nexus");
  await downloads.load();
  if (event.accepted) {
    // §9 Nexus link received: THE FAR WORLD ANSWERS, the mod by its name
    const row = downloads.downloads.value.find((d) => d.id === event.downloadId);
    const name = row?.modName ?? row?.fileName ?? null;
    push(
      "rift",
      name
        ? `Nexus sent the file for ${name}. Its download is on the shelf.`
        : "Nexus sent the file. Its download is on the shelf.",
      [{ label: "Show downloads", run: showDownloads }],
      { inscription: "THE FAR WORLD ANSWERS" },
    );
  } else if (event.refusal) {
    const voiced = voice(asRiftFailure(event.refusal), "nxm");
    push("ember", nxmRefusalLine(voiced), nxmRefusalActions(voiced, event));
  }
}

/** §9 Nexus link received's door: the far world's DOWNLOADS shelf. */
async function showDownloads(): Promise<void> {
  useNavigation().go("nexus");
  await nextTick();
  document.getElementById("far-downloads")?.scrollIntoView({ block: "start" });
}

/** §9 Nexus link refused: the refusal in its own sentence, never wrapped in
 *  "The stone held back:" (whose colon would meet the spine's own). */
function nxmRefusalLine(voiced: Voiced): string {
  const line = voicedLine(voiced);
  return voiced.kind === "refused" ? line.replace(/^The stone held back: /, "") : line;
}

/** §9 Nexus link refused's doors: Open on Nexus, and Open Preflight when the
 *  key is the way out (none stored, refused, or another account's: the voice
 *  offers Add key… or Replace key…, both of kind replaceKey). A link that
 *  names no file of this game (another game, a collection, an unreadable
 *  link: its ids are null) has no page here to open. */
function nxmRefusalActions(voiced: Voiced, event: Pick<NxmReceivedEvent, "modId" | "fileId">): ToastAction[] {
  const actions: ToastAction[] = [];
  const { modId, fileId } = event;
  if (modId !== null) actions.push({ label: "Open on Nexus", run: () => openRefusedOnNexus(modId, fileId) });
  if (voiced.recovery.some((r) => r.kind === "replaceKey"))
    actions.push({ label: "Open Preflight", run: () => useNavigation().go("preflight") });
  return actions;
}

/** The refused link's file on its Nexus page, where the site's Mod Manager
 *  Download button makes a fresh link (the refusals' own advice); the mod's
 *  files when the link named no file. */
async function openRefusedOnNexus(modId: number, fileId: number | null): Promise<void> {
  const url =
    fileId === null
      ? `https://www.nexusmods.com/dragonsdogma2/mods/${modId}?tab=files`
      : filePageUrl({ modId, fileId, nexusPageUrl: null });
  const backend = await rift();
  await backend.openUrl(url);
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
  // `await subscribe()` must stay BEFORE the first refreshOverview(): the
  // spine takes the window's first get_overview as the sign that the window
  // listens, and only then lets the nxm:// links waiting at its door through
  // (WINDOW-CONTRACT startup step 9). Read first and subscribe after, and a
  // cold start's rift://nxm-received could fire into a window not yet
  // listening: the link would be taken or refused with nobody told.
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
