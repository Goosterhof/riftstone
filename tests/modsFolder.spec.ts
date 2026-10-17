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
/** The row's measured value without the check's detail line under it. */
function lineOf(row: Element): string {
  const value = must(row.querySelector(".pre__val"), "value").cloneNode(true);
  if (!(value instanceof HTMLElement)) throw new Error("The value is not an element");
  value.querySelector(".pre__detail")?.remove();
  return textOf(value).replace(/\s+/g, " ").trim();
}
const labelsIn = (el: Element): string[] =>
  [...el.querySelectorAll(".rs-btn__label")].map((l) => textOf(l).trim()).filter(Boolean);
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
    expect(partOf(row, "val")).toBe(`${MOCK_MODS_DIR} · 11 packages`);
    expect(labelsIn(row)).toStrictEqual(["Open mods folder", "Scan"]);
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
    expect(lineOf(row)).toBe(`${MOCK_MODS_DIR} · The mods folder is missing`);
    expect(partOf(row, "detail")).toBe(`${MOCK_MODS_DIR} could not be made: access is denied.`);
    expect(labelsIn(row)).toStrictEqual(["Scan"]);
    // the check is the row's own: it is not listed again below
    expect(text().split("The mods folder is missing").length - 1).toBe(1);
  });
});

describe("an emptied mods folder", () => {
  it("should speak THE RIFT IS QUIET in place of the tree, and open the folder from it", async () => {
    await wakeOn("emptymods");
    await until(() => text().includes("THE RIFT IS QUIET"), "the quiet Rift");
    expect(textAt(".rs-ins__body")).toBe(
      "No mods in your mods folder yet. Put a mod's zip, 7z, rar or folder in it, and the stone reads it.",
    );
    expect(document.getElementById("rift-tree")).toBeNull();
    expect(document.getElementById("rift-answers")).toBeNull();
    expect(textAt(".pillar__count")).toBe("0 PACKAGES · 0 OPTIONS");
    const actions = must(document.querySelector(".rs-ins__actions"), "inscription actions");
    expect(labelsIn(actions)).toStrictEqual(["Open mods folder"]);
    const door = must(actions.querySelector<HTMLButtonElement>("button"), "Open mods folder");
    expect(door.classList.contains("rs-btn--primary")).toBe(true);

    const opened = vi.spyOn(spine(), "openModsFolder");
    door.click();
    await until(() => opened.mock.calls.length === 1, "the folder opened");
    expect(opened.mock.calls).toStrictEqual([[]]);
  });
});
