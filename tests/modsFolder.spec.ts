// v0.3 THE ONE FOLDER (WINDOW-CONTRACT § v0.3 "The one folder" and § Check
// ids `mods`; design system #00015 R16.25): the mods folder beside the
// installed stone is the only library. PREFLIGHT draws it as one of its own
// rows (the path, the package count, Open mods folder and Scan, the spine's
// `mods` check inside it when there is something to say); the folder opens
// in Explorer through the spine; an emptied folder makes the Rift quiet.

import { afterEach, describe, expect, it, vi } from "vitest";
import type { Overview } from "@/types/rift";
import { MOCK_MODS_DIR } from "@/services/mock/library";
import { RiftFailure } from "@/services/rift";
import { useNavigation } from "@/composables/useNavigation";
import { useStone } from "@/composables/useStone";
import { useToasts } from "@/composables/useToasts";
import { button, sleep, spine, text, textAt, textOf, until, wakeOn } from "./harness";

afterEach(() => {
  vi.restoreAllMocks();
  sleep();
});

function must<T>(found: T | null | undefined, what: string): T {
  if (found === null || found === undefined) throw new Error(`PREFLIGHT drew no ${what}`);
  return found;
}

const modsRow = (): HTMLElement | null => document.querySelector<HTMLElement>(".pre__row--mods");
const partOf = (row: Element, part: string): string =>
  textOf(must(row.querySelector(`.pre__${part}`), part))
    .replace(/\s+/g, " ")
    .trim();
const labelsIn = (el: Element): string[] =>
  [...el.querySelectorAll(".rs-btn__label")].map((l) => textOf(l).trim()).filter(Boolean);
/** Each button of the row's last column: its label, and whether it is the row's Primary. */
const actsIn = (row: Element): [string, boolean][] =>
  [...row.querySelectorAll<HTMLElement>(".pre__acts button")].map((b) => [
    textOf(b.querySelector(".rs-btn__label") ?? b).trim(),
    b.classList.contains("rs-btn--primary"),
  ]);
/** The spine's overview.rs `mods` sentences for a folder that is not there, word for word. */
const UNMADE_DETAIL = `The Riftstone keeps your mods in ${MOCK_MODS_DIR}, and that folder could not be made (access is denied). Make it by hand, or install the Riftstone where it may write, then start it again.`;
const GONE_DETAIL = `${MOCK_MODS_DIR} was moved or deleted while the Riftstone ran, and it reads mods from nowhere else. Open mods folder makes it again, empty, and watches it; then put your mods back in it.`;
const toastLines = (): string[] => useToasts().toasts.value.map((t) => t.line);

async function openPreflight(): Promise<HTMLElement> {
  useNavigation().go("preflight");
  await until(() => modsRow() !== null, "the Mods folder row");
  return must(modsRow(), "Mods folder row");
}

describe("the Mods folder row on PREFLIGHT", () => {
  it("should draw the one folder as the page's own row: its path, its packages, Open mods folder and Scan", async () => {
    await wakeOn();
    const row = await openPreflight();
    expect([partOf(row, "glyph"), partOf(row, "word"), partOf(row, "what")]).toStrictEqual(["✓", "OK", "Mods folder"]);
    // R16.25: the path in a left-ellipsized box with the whole path in title, the count faint beside it
    expect(textOf(must(row.querySelector<HTMLElement>(".pre__path"), "path")).trim()).toBe(MOCK_MODS_DIR);
    expect(row.querySelector<HTMLElement>(".pre__path")?.title).toBe(MOCK_MODS_DIR);
    expect(textOf(must(row.querySelector(".pre__count"), "count")).trim()).toBe("11 packages");
    // R16.25: with nothing to say, Scan is the row's Primary and Open mods folder its Secondary
    expect(actsIn(row)).toStrictEqual([
      ["Open mods folder", false],
      ["Scan", true],
    ]);
    expect(row.dataset.check).toBe("mods");
    // a folder that holds packages has nothing to say: the spine sends no `mods` check
    expect(useStone().modsCheck.value).toBeNull();
    // one of the page's six own rows, never a root list: the spine's checks follow it
    const own = [...document.querySelectorAll(".pre__list > .pre__row")].slice(0, 6).map((r) => partOf(r, "what"));
    expect(own).toStrictEqual([
      "Game install",
      "Steam build",
      "REFramework",
      "Alternate PAK Folder",
      "Loose File Loader",
      "Mods folder",
    ]);
    expect(text()).not.toMatch(/LIBRARY ROOTS|Search all roots/);
  });

  it("should open the folder through the spine, and read the stone again after", async () => {
    await wakeOn();
    const row = await openPreflight();
    const opened = vi.spyOn(spine(), "openModsFolder");
    const read = vi.spyOn(spine(), "getOverview");
    must(
      [...row.querySelectorAll<HTMLButtonElement>("button")].find((b) => textOf(b).trim() === "Open mods folder"),
      "Open mods folder",
    ).click();
    await until(() => opened.mock.calls.length === 1 && read.mock.calls.length > 0, "the folder opened");
    expect(opened.mock.calls).toStrictEqual([[]]);
    expect(toastLines()).toStrictEqual([]);
  });

  it("should scan the folder again on Scan", async () => {
    await wakeOn();
    await openPreflight();
    const scanned = vi.spyOn(spine(), "rescan");
    button("Scan").click();
    await until(() => scanned.mock.calls.length === 1, "the scan");
    expect(scanned.mock.calls).toStrictEqual([[]]);
  });

  it("should voice a folder Explorer could not open, and change nothing", async () => {
    await wakeOn();
    await openPreflight();
    vi.spyOn(spine(), "openModsFolder").mockRejectedValueOnce(
      new RiftFailure({ kind: "io", detail: { reason: "Explorer did not start" } }),
    );
    button("Open mods folder").click();
    const line =
      "The stone couldn't open your mods folder: Explorer did not start. Nothing in it changed. Its path is on PREFLIGHT, so Explorer can open it too.";
    await until(() => toastLines().includes(line), "the voiced failure");
    expect(useToasts().toasts.value.map((t) => t.tone)).toStrictEqual(["ember"]);
  });

  it("should mark a folder that is missing and could not be made, say why, and offer nothing to open", async () => {
    await wakeOn();
    const backend = spine();
    const read = backend.getOverview.bind(backend);
    vi.spyOn(backend, "getOverview").mockImplementation(async (): Promise<Overview> => {
      const real = await read();
      return {
        ...real,
        mods: { path: MOCK_MODS_DIR, exists: false },
        checks: [
          ...real.checks,
          {
            id: "mods",
            level: "error",
            title: "The mods folder is missing",
            detail: `${MOCK_MODS_DIR} could not be made: access is denied.`,
            action: null,
          },
        ],
      };
    });
    await useStone().refreshOverview();
    const row = await openPreflight();
    expect([partOf(row, "glyph"), partOf(row, "word")]).toStrictEqual(["✗", "MISSING"]);
    // the check's title in place of the count, its detail on a line of its own
    expect(textOf(must(row.querySelector(".pre__count"), "count")).trim()).toBe("The mods folder is missing");
    expect(partOf(row, "detail")).toBe(`${MOCK_MODS_DIR} could not be made: access is denied.`);
    expect(labelsIn(row)).toStrictEqual(["Scan again"]);
    // the check is the row's own: it is not listed again below
    expect(text().split("The mods folder is missing").length - 1).toBe(1);
  });

  it("should draw the spine's Error for a folder that could not be made at launch, and offer only Scan again", async () => {
    await wakeOn("nofloor");
    const row = await openPreflight();
    expect([partOf(row, "glyph"), partOf(row, "word")]).toStrictEqual(["✗", "MISSING"]);
    expect(textOf(must(row.querySelector(".pre__count"), "count")).trim()).toBe("The mods folder is missing");
    expect(partOf(row, "detail")).toBe(UNMADE_DETAIL);
    // the check carries no action: nothing to open, only a scan that looks again
    expect(useStone().modsCheck.value?.action).toBeNull();
    expect(actsIn(row)).toStrictEqual([["Scan again", true]]);
    // never the empty folder's note over a folder that is not there
    expect(text()).not.toContain("The mods folder is empty");
  });

  it("should draw the spine's Warn for a folder that went while the stone ran, and make it again with Open mods folder", async () => {
    await wakeOn("gonemods");
    const row = await openPreflight();
    expect([partOf(row, "glyph"), partOf(row, "word")]).toStrictEqual(["⚠", "CHECK"]);
    expect(textOf(must(row.querySelector(".pre__count"), "count")).trim()).toBe("The mods folder is gone");
    expect(partOf(row, "detail")).toBe(GONE_DETAIL);
    // the detail points at Open mods folder: it is drawn, and it leads (a scan never makes the folder)
    expect(actsIn(row)).toStrictEqual([
      ["Open mods folder", true],
      ["Scan again", false],
    ]);
    expect(text().split("The mods folder is gone").length - 1).toBe(1);

    const opened = vi.spyOn(spine(), "openModsFolder");
    must(
      [...row.querySelectorAll<HTMLButtonElement>("button")].find((b) => textOf(b).trim() === "Open mods folder"),
      "Open mods folder",
    ).click();
    await until(() => opened.mock.calls.length === 1, "the folder made again");
    // made again, empty: the row reads the empty folder's note, and the folder is there
    await until(() => useStone().modsCheck.value?.title === "The mods folder is empty", "the empty folder's note");
    const after = must(modsRow(), "Mods folder row");
    expect([partOf(after, "glyph"), partOf(after, "word")]).toStrictEqual(["·", "NOTE"]);
    expect(useStone().modsFolder.value?.exists).toBe(true);
    expect(actsIn(after)).toStrictEqual([
      ["Open mods folder", true],
      ["Scan", false],
    ]);
    expect(toastLines()).toStrictEqual([]);
  });
});

describe("an emptied mods folder", () => {
  it("should speak THE RIFT IS QUIET in place of the tree, and open the folder from it", async () => {
    await wakeOn("emptymods");
    await until(() => text().includes("THE RIFT IS QUIET"), "the quiet Rift");
    expect(textAt(".rs-ins__body")).toBe(
      `Your mods folder is empty. Put a mod's zip, 7z or rar, or its unpacked folder, in ${MOCK_MODS_DIR}, or download one from the far world. The stone reads it when you scan.`,
    );
    expect(document.getElementById("rift-tree")).toBeNull();
    expect(document.getElementById("rift-answers")).toBeNull();
    expect(textAt(".pillar__count")).toBe("0 PACKAGES · 0 OPTIONS");
    const actions = must(document.querySelector(".rs-ins__actions"), "inscription actions");
    expect(labelsIn(actions)).toStrictEqual(["Open mods folder", "Scan", "Find on Nexus"]);
    const door = must(actions.querySelector<HTMLButtonElement>("button"), "Open mods folder");
    expect(door.classList.contains("rs-btn--primary")).toBe(true);

    const opened = vi.spyOn(spine(), "openModsFolder");
    door.click();
    await until(() => opened.mock.calls.length === 1, "the folder opened");
    expect(opened.mock.calls).toStrictEqual([[]]);
  });
});
