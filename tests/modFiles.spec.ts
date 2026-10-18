// v0.3 A MOD'S FILES (design system #00015 R16.21): the retired files fold
// under one "Older files" disclosure that controls them and opens below
// itself; Esc returns to the far world on its first press, whatever is open
// in Your World (which this page does not show); and without a key, Download
// stays reachable by the keyboard, saying why it is held back.
import { afterEach, describe, expect, it, vi } from "vitest";
import type { DownloadView } from "@/types/rift";
import { MOCK_PACKAGES } from "@/services/mock/library";
import { useDownloads } from "@/composables/useDownloads";
import { useNavigation } from "@/composables/useNavigation";
import { useWorld } from "@/composables/useWorld";
import { button, hasButton, sleep, spine, textOf, until, wakeOn, type Scene } from "./harness";

afterEach(() => {
  sleep();
  vi.restoreAllMocks();
});

function must<T>(found: T | null | undefined, what: string): T {
  if (found === null || found === undefined) throw new Error(`The files view drew no ${what}`);
  return found;
}

async function openFarWorld(scene: Scene = ""): Promise<void> {
  await wakeOn(scene);
  useNavigation().go("nexus");
  await until(() => hasButton("Find all by name") || hasButton("Search"), "the far world");
}

const lanternFiles = (): HTMLButtonElement | null =>
  document.querySelector<HTMLButtonElement>('[aria-label="Files of Lantern Glow Recolor on Nexus"]');
const filesView = (): HTMLElement | null => document.querySelector<HTMLElement>("[data-mod-files]");
/** THE FAR WORLD's key field, in the ACCOUNT section's form: where every key door leads. */
const keyField = (): HTMLInputElement | null =>
  document.querySelector<HTMLInputElement>("#far-account input[type=password]");
const fileRow = (fileId: number): HTMLElement =>
  must(document.querySelector<HTMLElement>(`[data-mod-files] [data-file="${fileId}"]`), `file ${fileId}`);

function buttonIn(el: Element, label: string): HTMLButtonElement {
  return must(
    [...el.querySelectorAll<HTMLButtonElement>("button")].find(
      (b) => textOf(b.querySelector(".rs-btn__label") ?? b).trim() === label,
    ),
    `"${label}" button`,
  );
}

async function openLanternFiles(): Promise<void> {
  await until(() => lanternFiles() !== null, "the update row's Files");
  lanternFiles()?.click();
  await until(() => document.querySelectorAll("[data-mod-files] [data-file]").length > 0, "the mod's files");
}

describe("a mod's files", () => {
  it("should open the older files below their disclosure, inside the region it controls", async () => {
    await openFarWorld("premium");
    await openLanternFiles();
    const older = must(document.querySelector<HTMLButtonElement>(".files__older"), "Older files");
    const region = must(document.getElementById(older.getAttribute("aria-controls") ?? ""), "the older files' region");
    expect(older.getAttribute("aria-expanded")).toBe("false");
    expect(region.querySelector("h3")).toBeNull();
    older.click();
    await until(() => older.getAttribute("aria-expanded") === "true", "the older files open");
    const head = must(
      [...region.querySelectorAll("h3")].find((h) => textOf(h).trim() === "OLD VERSIONS"),
      "OLD VERSIONS inside the region",
    );
    // the disclosure stays above what it opened
    expect(Boolean(older.compareDocumentPosition(head) & Node.DOCUMENT_POSITION_FOLLOWING)).toBe(true);
    expect(region.contains(fileRow(2210))).toBe(true);
  });

  it("should return to the far world on the first Esc, with an option open in Your World that this page does not show", async () => {
    await openFarWorld("premium");
    // an option the player opened in the Rift before coming here
    const option = must(MOCK_PACKAGES.flatMap((p) => p.options).at(0), "an option").id;
    await useWorld().open(option);
    expect(useWorld().selectedId.value).toBe(option);
    await openLanternFiles();
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", cancelable: true }));
    await until(() => filesView() === null, "the far world after one Esc");
    // Your World keeps what the player opened: it was never on this page to close
    expect(useWorld().selectedId.value).toBe(option);
  });

  it("should keep a keyless Download reachable by the keyboard, saying why, and send nothing", async () => {
    await openFarWorld();
    await openLanternFiles();
    const download = buttonIn(fileRow(2391), "Download");
    expect(download.disabled).toBe(false);
    expect(download.getAttribute("aria-disabled")).toBe("true");
    const why = must(document.getElementById(download.getAttribute("aria-describedby") ?? ""), "the reason");
    expect(textOf(why).trim()).toBe("Add your Nexus key to download from the stone.");
    download.focus();
    expect(document.activeElement).toBe(download);
    const started = vi.spyOn(spine(), "startDownload");
    download.click();
    await new Promise((resolve) => setTimeout(resolve, 80));
    expect(started).not.toHaveBeenCalled();
    // back to the far world: the way out is the page's own key field
    expect(hasButton("‹ The far world")).toBe(true);
    button("‹ The far world").click();
    await until(() => filesView() === null, "the far world again");
  });

  it("should say what the key adds above a keyless mod's files, and open the far world's key form on Add key…", async () => {
    await openFarWorld();
    await openLanternFiles();
    const nokey = must(document.querySelector<HTMLElement>("[data-mod-files] .files__nokey"), "the keyless line");
    expect(textOf(must(nokey.querySelector("p"), "its sentence")).trim()).toBe(
      "Add your Nexus key to download from the stone.",
    );
    buttonIn(nokey, "Add key…").click();
    // the files give way to the far world's scroll: its ACCOUNT form open, its field focused
    await until(() => filesView() === null, "the far world's scroll again");
    await until(() => keyField() !== null && document.activeElement === keyField(), "the key field, focused");
    expect(useNavigation().page.value).toBe("nexus");
  });

  it("should hold a file's Download while that very file is on its way, saying so, and leave the mod's other files free", async () => {
    await openFarWorld("premium");
    await openLanternFiles();
    const lanternId = must(
      MOCK_PACKAGES.find((p) => p.summary.displayName === "Lantern Glow Recolor"),
      "Lantern Glow Recolor",
    ).summary.id;
    const onItsWay: DownloadView = {
      id: "download-7",
      modId: 412,
      fileId: 2391,
      modName: "Lantern Glow Recolor",
      fileName: "Lantern Glow Recolor",
      version: "2.2",
      forPackageId: lanternId,
      phase: "downloading",
      bytesDone: 1_048_576,
      bytesTotal: 12_845_056,
      landedAs: null,
      packageId: null,
      alreadyHad: false,
      failure: null,
      nexusPageUrl: null,
      startedMs: Date.now() - 2_000,
      finishedMs: null,
    };
    vi.spyOn(spine(), "getDownloads").mockResolvedValue([onItsWay]);
    await useDownloads().load();
    const held = buttonIn(fileRow(2391), "Download");
    await until(() => held.getAttribute("aria-disabled") === "true", "the file's Download held");
    expect(held.disabled).toBe(false);
    const why = must(document.getElementById(held.getAttribute("aria-describedby") ?? ""), "the reason");
    expect(textOf(why).trim()).toBe("Downloading: see DOWNLOADS");
    const started = vi.spyOn(spine(), "startDownload");
    held.click();
    await new Promise((resolve) => setTimeout(resolve, 80));
    expect(started).not.toHaveBeenCalled();
    // another file of the same mod is another download: it stays free
    const other = buttonIn(fileRow(2215), "Download");
    expect(other.getAttribute("aria-disabled")).toBeNull();
    other.click();
    await until(() => started.mock.calls.length === 1, "the other file asked for");
    expect(started.mock.calls).toStrictEqual([[412, 2215, lanternId]]);
  });
});
