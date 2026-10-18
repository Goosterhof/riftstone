// v0.3 DOWNLOADS (WINDOW-CONTRACT § v0.3 "Downloads", design system #00015
// R16.18 and §9): the shelf says every phase the spine's row is in, a landed
// download's package reaches UPDATES once the scan shelved it, Show in the Rift
// finds the package whatever narrowed the answers, and a failed row offers the
// way out its reason calls for, never a Try again that fails again.
import { afterEach, describe, expect, it, vi } from "vitest";
import type { DownloadView, NexusReason, RiftError } from "@/types/rift";
import { MOCK_PACKAGES } from "@/services/mock/library";
import { useDownloads } from "@/composables/useDownloads";
import { useLibrary } from "@/composables/useLibrary";
import { useNavigation } from "@/composables/useNavigation";
import { useSearch } from "@/composables/useSearch";
import { hasButton, sleep, spine, textOf, until, wakeOn, type Scene } from "./harness";

afterEach(() => {
  sleep();
  vi.restoreAllMocks();
});

function must<T>(found: T | null | undefined, what: string): T {
  if (found === null || found === undefined) throw new Error(`The shelf drew no ${what}`);
  return found;
}

const lanternId = must(
  MOCK_PACKAGES.find((p) => p.summary.displayName === "Lantern Glow Recolor"),
  "Lantern Glow Recolor in the mock library",
).summary.id;

/** The file's own page on Nexus, where Mod Manager Download makes a fresh link. */
const LANTERN_PAGE = "https://www.nexusmods.com/dragonsdogma2/mods/412?tab=files&file_id=2391&nmm=1";

async function openFarWorld(scene: Scene = ""): Promise<void> {
  await wakeOn(scene);
  useNavigation().go("nexus");
  await until(() => hasButton("Find all by name") || hasButton("Search"), "the far world");
}

/** One of this session's downloads, as the spine lists it. */
function downloadRow(over: Partial<DownloadView>): DownloadView {
  return {
    id: "download-41",
    modId: 412,
    fileId: 2391,
    modName: "Lantern Glow Recolor",
    fileName: "Lantern Glow Recolor",
    version: "2.2",
    forPackageId: lanternId,
    phase: "downloading",
    bytesDone: 4_194_304,
    bytesTotal: 12_845_056,
    landedAs: null,
    packageId: null,
    alreadyHad: false,
    failure: null,
    nexusPageUrl: null,
    startedMs: Date.now() - 4_000,
    finishedMs: null,
    ...over,
  };
}

const shelf = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>("#far-downloads [data-download]")];
const phaseWord = (row: Element): string => textOf(must(row.querySelector(".shelf__phase"), "phase word")).trim();
const labelsIn = (el: Element): string[] =>
  [...el.querySelectorAll(".rs-btn__label")].map((l) => textOf(l).trim()).filter(Boolean);
const updateDoor = (): HTMLButtonElement | null =>
  document.querySelector<HTMLButtonElement>(`[data-update-download="${lanternId}"]`);

function buttonIn(el: Element, label: string): HTMLButtonElement {
  return must(
    [...el.querySelectorAll<HTMLButtonElement>("button")].find(
      (b) => textOf(b.querySelector(".rs-btn__label") ?? b).trim() === label,
    ),
    `"${label}" button`,
  );
}

/** Stand in for the spine's list with one row, read through the window's own load. */
async function listOne(row: DownloadView): Promise<void> {
  vi.spyOn(spine(), "getDownloads").mockResolvedValue([row]);
  await useDownloads().load();
  await until(() => shelf().at(0)?.dataset.phase === row.phase, `the ${row.phase} row`);
}

describe("the downloads shelf says the phase the spine's row is in", () => {
  it("should say Checking the file while the stone verifies, its bar full: the last bytes never cover the phase", async () => {
    await openFarWorld("premium");
    const listed = vi.spyOn(spine(), "getDownloads").mockResolvedValue([downloadRow({ bytesDone: 1_048_576 })]);
    await useDownloads().load();
    // the last download-progress event of the run: its phase is always "downloading"
    useDownloads().onProgress({
      id: "download-41",
      phase: "downloading",
      bytesDone: 12_845_056,
      bytesTotal: 12_845_056,
    });
    listed.mockResolvedValue([downloadRow({ phase: "verifying", bytesDone: 12_845_056 })]);
    await useDownloads().onChanged();
    expect(useDownloads().downloads.value.map((d) => d.phase)).toStrictEqual(["verifying"]);
    await until(() => shelf().at(0)?.dataset.phase === "verifying", "the verifying row");
    const row = must(shelf().at(0), "row");
    expect(phaseWord(row)).toBe("Checking the file");
    const bar = must(row.querySelector('[role="progressbar"]'), "bar");
    expect([bar.getAttribute("aria-valuenow"), bar.getAttribute("aria-valuemax")]).toStrictEqual([
      "12845056",
      "12845056",
    ]);
  });

  it("should still lay the bytes in flight over a downloading row between two reads", async () => {
    await openFarWorld("premium");
    vi.spyOn(spine(), "getDownloads").mockResolvedValue([downloadRow({ bytesDone: 1_048_576 })]);
    await useDownloads().load();
    useDownloads().onProgress({
      id: "download-41",
      phase: "downloading",
      bytesDone: 6_291_456,
      bytesTotal: 12_845_056,
    });
    expect(useDownloads().downloads.value.map((d) => [d.phase, d.bytesDone])).toStrictEqual([
      ["downloading", 6_291_456],
    ]);
  });

  it("should draw every phase of a real run, Checking the file among them, and never gold", async () => {
    await openFarWorld("premium");
    await until(() => updateDoor() !== null, "the update row's Download");
    updateDoor()?.click();
    const words = new Set<string>();
    await until(
      () => {
        const row = shelf().at(0);
        if (row) words.add(phaseWord(row));
        return row?.dataset.phase === "landed";
      },
      "the download landed",
      10_000,
    );
    expect([...words]).toStrictEqual([
      "Queued",
      "Asking Nexus",
      "Downloading",
      "Checking the file",
      "In your mods folder",
    ]);
  }, 20_000);
});

describe("a landed download in the far world and the Rift", () => {
  it("should list the landed package under UPDATES once the scan has shelved it, linked to the exact file", async () => {
    await openFarWorld("premium");
    await until(() => updateDoor() !== null, "the update row's Download");
    updateDoor()?.click();
    await until(() => shelf().at(0)?.dataset.phase === "landed", "the download landed", 10_000);
    const packageId = must(useDownloads().downloads.value.at(0)?.packageId, "the landed package's id");
    // the spine links the archive before its scan shelves it: the row comes with the library-changed
    await until(() => document.getElementById(`far-linked-${packageId}`) !== null, "the new package under UPDATES");
    expect(textOf(must(document.getElementById(`far-linked-${packageId}`), "row"))).toContain("mod 412 · download");
  }, 20_000);

  it("should take Show in the Rift to the landed package on the tree, whatever narrowed the answers, and focus it", async () => {
    await openFarWorld("premium");
    // the player narrowed the Rift to what is not installed, and left it so
    useSearch().setStateFacet("notInstalled");
    useSearch().setMenuOnly(true);
    await until(() => !useSearch().idle.value, "the narrowed Rift");
    await until(() => updateDoor() !== null, "the update row's Download");
    updateDoor()?.click();
    await until(() => shelf().at(0)?.dataset.phase === "landed", "the download landed", 10_000);
    const packageId = must(useDownloads().downloads.value.at(0)?.packageId, "the landed package's id");
    const row = must(shelf().at(0), "landed row");
    buttonIn(row, "Show in the Rift").focus();
    buttonIn(row, "Show in the Rift").click();

    await until(() => useNavigation().page.value === "rift", "THE RIFT");
    const item = (): HTMLElement | null =>
      document.querySelector<HTMLElement>(`#rift-tree [data-key="${CSS.escape(packageId)}"]`);
    await until(() => item() !== null && document.activeElement === item(), "the landed package focused on the tree");
    expect(useSearch().idle.value).toBe(true);
    expect([useSearch().stateFacet.value, useSearch().includeMenuOnly.value]).toStrictEqual(["all", false]);
    expect(useLibrary().isExpanded(packageId)).toBe(true);
  }, 20_000);
});

/** A row that failed for `reason`, the spine's sentence beside it. */
function failed(reason: NexusReason, detail: string): DownloadView {
  const failure: RiftError = { kind: "nexus", detail: { detail, reason } };
  return downloadRow({ phase: "failed", failure, finishedMs: Date.now() });
}

describe("a failed download's way out (§9, R16.18)", () => {
  it.each<[NexusReason, string[]]>([
    ["premiumRequired", ["Open on Nexus"]],
    ["linkExpired", ["Open on Nexus"]],
    ["fileGone", ["Open on Nexus"]],
    ["noKey", ["Add key…", "Open on Nexus"]],
    ["keyRejected", ["Replace key…", "Open on Nexus"]],
    ["linkNotYours", ["Open on Nexus", "Replace key…"]],
    ["offline", ["Try again", "Open on Nexus"]],
    ["notTheFile", ["Try again", "Copy details", "Open on Nexus"]],
  ])("should offer what %s calls for, and Open on Nexus once", async (reason, labels) => {
    await openFarWorld("premium");
    await listOne(failed(reason, "Nexus answered HTTP 403."));
    expect(labelsIn(must(shelf().at(0), "failed row"))).toStrictEqual(labels);
  });

  it("should open the file's own Nexus page from Open on Nexus, where a fresh link is made", async () => {
    await openFarWorld("key");
    await listOne(failed("linkExpired", "The download link from Nexus Mods has expired."));
    const opened = vi.spyOn(window, "open");
    const started = vi.spyOn(spine(), "startDownload");
    buttonIn(must(shelf().at(0), "row"), "Open on Nexus").click();
    await until(() => opened.mock.calls.length === 1, "the file's page");
    expect(opened.mock.calls[0]?.[0]).toBe(LANTERN_PAGE);
    expect(started).not.toHaveBeenCalled();
  });

  it("should open the key form from Add key…, the key being what the download lacked", async () => {
    await openFarWorld();
    await listOne(failed("noKey", "This needs a Nexus Mods personal API key and none is set."));
    buttonIn(must(shelf().at(0), "row"), "Add key…").click();
    await until(() => document.querySelector("#far-account input[type=password]") !== null, "the key field");
    const field = must(document.querySelector("#far-account input[type=password]"), "key field");
    await until(() => document.activeElement === field, "the key field, focused");
    expect(document.activeElement).toBe(field);
  });

  it("should start the same mod, file and package again from Try again when Nexus was out of reach", async () => {
    await openFarWorld("premium");
    await listOne(failed("offline", "Nexus Mods could not be reached (connection refused)."));
    const started = vi.spyOn(spine(), "startDownload");
    buttonIn(must(shelf().at(0), "row"), "Try again").click();
    await until(() => started.mock.calls.length === 1, "the same file asked for again");
    expect(started.mock.calls).toStrictEqual([[412, 2391, lanternId]]);
  });

  it("should say a cancelled download left nothing behind (§9)", async () => {
    await openFarWorld("premium");
    await listOne(downloadRow({ phase: "cancelled", finishedMs: Date.now() }));
    const row = must(shelf().at(0), "cancelled row");
    expect(phaseWord(row)).toBe("Cancelled");
    expect(textOf(must(row.querySelector(".shelf__gone"), "the cancelled line")).trim()).toBe(
      "Cancelled. The partial file is gone.",
    );
    expect(labelsIn(row)).toStrictEqual([]);
  });
});
