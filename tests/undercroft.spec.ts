// v0.5 — THE UNDERCROFT (#00015 §18.7, R18.11–R18.14, with §18.17's Q18.6):
// the trouble fold turned upside down, at the foot of the Rift's tree and
// outside role="tree": a carved count of what was put away in mods (older
// versions), `Show ▸` to open it, a row per archive with `Bring it back`, the
// folder's empty and never-made states, and `Show in Explorer` once the folder
// exists. Every name is the mock's invented one.

import { afterEach, describe, expect, it, vi } from "vitest";
import { useLibrary } from "@/composables/useLibrary";
import { useNavigation } from "@/composables/useNavigation";
import { useToasts } from "@/composables/useToasts";
import { RiftFailure } from "@/services/rift";
import { formatBytes } from "@/services/format";
import { FAMILY_NAME, familyFileName } from "@/services/mock/versions";
import { broughtBackToast } from "@/services/versions";
import { button, hasButton, sleep, spine, text, textAt, textOf, typeQuery, until, wakeOn, type Scene } from "./harness";

afterEach(() => {
  vi.restoreAllMocks();
  sleep();
});

function must<T>(found: T | null | undefined, what: string): T {
  if (found === null || found === undefined) throw new Error(`The stone drew no ${what}`);
  return found;
}

const crypt = (): HTMLElement | null => document.querySelector<HTMLElement>(".crypt");
const count = (): string | null => textAt("#rift-undercroft-count");
const toggle = (): HTMLButtonElement =>
  must(crypt()?.querySelector<HTMLButtonElement>("button[aria-controls='rift-undercroft']"), "the fold's door");
const body = (): HTMLElement => must(document.getElementById("rift-undercroft"), "the fold's body");
const rows = (): HTMLElement[] => [...body().querySelectorAll<HTMLElement>(".crypt__row")];
const said = (el: Element): string => textOf(el).replace(/\s+/g, " ").trim();

async function wakeShelf(scene: Scene): Promise<void> {
  await wakeOn(scene);
  await until(() => crypt() !== null && count() !== null, "the undercroft line");
  await until(() => document.querySelector("#rift-tree [role='treeitem']") !== null, "the tree");
}

async function openFold(): Promise<void> {
  toggle().click();
  await until(() => !body().hidden && body().querySelector(".crypt__head") !== null, "the open fold");
  await until(() => body().querySelector(".crypt__say") !== null, "the fold's sentence");
}

describe("THE UNDERCROFT, its line at the tree's foot (R18.11)", () => {
  it("should close the Rift column with `1 PUT AWAY` and `Show ▸`, outside the tree", async () => {
    await wakeShelf("undercroft");
    await until(() => count() === "1 PUT AWAY", "the carved count");
    const line = must(crypt(), "the undercroft");
    expect(line.closest("[role='tree']")).toBeNull();
    // the mirror of the trouble fold: after the tree, the last thing in the column
    const tree = must(document.getElementById("rift-tree"), "the tree");
    expect((tree.compareDocumentPosition(line) & Node.DOCUMENT_POSITION_FOLLOWING) !== 0).toBe(true);
    expect(line.nextElementSibling).toBeNull();
    expect(said(toggle())).toBe("Show ▸");
    expect(toggle().getAttribute("aria-expanded")).toBe("false");
    expect(toggle().className).toContain("rs-btn--secondary");
    expect(toggle().className).toContain("rs-btn--sm");
    expect(body().hidden).toBe(true);
    // the count is an inscription, never a button
    expect(must(document.getElementById("rift-undercroft-count"), "count").tagName).not.toBe("BUTTON");
  });

  it("should carve `NOTHING PUT AWAY` in the quiet tone and still offer `Show ▸`", async () => {
    await wakeShelf("undercroftempty");
    await until(() => count() === "NOTHING PUT AWAY", "the empty count");
    expect(crypt()?.classList.contains("is-empty")).toBe(true);
    expect(said(toggle())).toBe("Show ▸");
  });

  it("should be gone under a query, and back when the well is cleared", async () => {
    await wakeShelf("undercroft");
    await typeQuery("lantern");
    await until(() => crypt() === null, "the undercroft gone under a query");
    expect(document.getElementById("rift-undercroft")).toBeNull();
    await typeQuery("");
    await until(() => crypt() !== null, "the undercroft back with the tree");
    expect(count()).toBe("1 PUT AWAY");
  });

  it("should read the folder on entering the Rift and again each time the fold opens", async () => {
    await wakeOn("undercroft");
    const read = vi.spyOn(spine(), "getOlderVersions");
    useNavigation().go("world");
    await until(() => crypt() === null, "away from the Rift");
    useNavigation().go("rift");
    await until(() => read.mock.calls.length === 1, "a read on entering the Rift");
    await until(() => count() === "1 PUT AWAY", "the count");
    await openFold();
    expect(read).toHaveBeenCalledTimes(2);
  });
});

describe("THE UNDERCROFT, open (R18.12)", () => {
  it("should name itself, say what it holds, and set each archive down with `Bring it back`", async () => {
    await wakeShelf("undercroft");
    await openFold();
    expect(said(toggle())).toBe("Close ▾");
    expect(toggle().getAttribute("aria-expanded")).toBe("true");
    expect(said(must(body().querySelector(".crypt__head"), "head"))).toBe("THE UNDERCROFT");
    expect(said(must(body().querySelector(".crypt__say"), "sentence"))).toBe(
      "Older versions the stone put away, in mods (older versions) beside your mods folder. Nothing here is read or installed. Bring one back and it joins its file in the Rift again.",
    );
    expect(rows()).toHaveLength(1);
    const row = must(rows()[0], "the row");
    expect(said(must(row.querySelector(".crypt__name"), "name"))).toBe(FAMILY_NAME);
    expect(said(must(row.querySelector(".crypt__ver"), "version"))).toBe("v1.2");
    expect(said(must(row.querySelector(".crypt__size"), "size"))).toBe(formatBytes(1_468_006));
    const file = must(row.querySelector<HTMLElement>("p.crypt__file"), "line 2");
    expect(said(file)).toBe(familyFileName("1.2"));
    expect(file.getAttribute("title")).toBe(familyFileName("1.2"));
    expect(file.classList.contains("rs-tail")).toBe(true);
    expect(said(must(row.querySelector(".crypt__newer"), "line 3"))).toBe("A newer version is in your mods folder.");
    const back = button("Bring it back");
    expect(back.className).toContain("rs-btn--primary");
    expect(back.className).toContain("rs-btn--sm");
  });

  it("should read line 3 one step louder when it is the only copy, and line 1 by file name when never linked", async () => {
    await wakeOn("undercroft");
    vi.spyOn(spine(), "getOlderVersions").mockResolvedValue({
      exists: true,
      items: [{ fileName: "Hand_Moved_Archive.zip", size: 2048, modName: null, version: null, newerInMods: false }],
    });
    await until(() => crypt() !== null, "the undercroft");
    await openFold();
    await until(() => rows().length === 1 && textOf(body()).includes("Hand_Moved"), "the row");
    const row = must(rows()[0], "the row");
    expect(row.querySelector(".crypt__name")).toBeNull();
    expect(said(must(row.querySelector(".crypt__line1 .crypt__file"), "line 1's file"))).toBe("Hand_Moved_Archive.zip");
    expect(row.querySelector("p.crypt__file")).toBeNull();
    const newer = must(row.querySelector(".crypt__newer"), "line 3");
    expect(said(newer)).toBe("No other version of it is in your mods folder.");
    expect(newer.classList.contains("is-only")).toBe(true);
  });
});

describe("THE UNDERCROFT, bring it back (R18.13)", () => {
  it("should let the row leave, the count drop, and say so once in a cyan toast with `Show in the Rift`", async () => {
    await wakeShelf("undercroft");
    await openFold();
    button("Bring it back").click();
    await until(() => rows().length === 0, "the row gone");
    await until(() => count() === "NOTHING PUT AWAY", "the count dropped");
    await until(() => useToasts().toasts.value.some((t) => t.line.includes("is back")), "the toast");
    const toast = useToasts().toasts.value.find((t) => t.line.includes("is back"));
    expect(toast?.line).toBe(`v1.2 of ${FAMILY_NAME} is back in your mods folder. It sits under v1.3 in the Rift.`);
    expect(toast?.tone).toBe("rift");
    expect(toast?.actions.map((a) => a.label)).toStrictEqual(["Show in the Rift"]);
    expect(text().split("is back in your mods folder").length - 1).toBe(1);

    const library = useLibrary();
    const older = must(
      library.packages.value.find((p) => p.package.fileName === familyFileName("1.2")),
      "v1.2 in the library",
    );
    const lead = must(older.family?.lead, "v1.2's lead");
    await toast?.actions[0]?.run();
    await until(() => library.focusedKey.value === older.package.id, "the version row focused");
    expect(library.isExpanded(lead)).toBe(true);
    const row = must(
      document.querySelector<HTMLElement>(`#rift-tree [data-key="${CSS.escape(older.package.id)}"]`),
      "the version row",
    );
    expect(document.activeElement).toBe(row);
  });

  it("should say a failed move under its row in ember, with `Try again` and `Copy details`", async () => {
    await wakeShelf("undercroft");
    await openFold();
    vi.spyOn(spine(), "bringBack").mockRejectedValueOnce(
      new RiftFailure({ kind: "io", detail: { reason: "access is denied" } }),
    );
    button("Bring it back").click();
    await until(() => body().querySelector(".crypt__fail") !== null, "the failure under the row");
    const fail = must(rows()[0]?.querySelector<HTMLElement>(".crypt__fail"), "the row's failure");
    expect(fail.getAttribute("role")).toBe("alert");
    expect(said(must(fail.querySelector("p"), "the sentence"))).toBe(
      `The stone couldn't move ${familyFileName("1.2")} back into your mods folder: access is denied. It is still in mods (older versions), exactly as it was.`,
    );
    const again = button("Try again");
    expect(again.className).toContain("rs-btn--primary");
    expect(again.className).toContain("rs-btn--sm");
    expect(hasButton("Copy details")).toBe(true);
    expect(count()).toBe("1 PUT AWAY");
    again.click();
    await until(() => rows().length === 0, "Try again brings it back");
  });

  it("should word the toast for no newer version and for a taken name", () => {
    expect(broughtBackToast("v1.2", FAMILY_NAME, null, null)).toBe(
      `v1.2 of ${FAMILY_NAME} is back in your mods folder.`,
    );
    expect(broughtBackToast("v1.2", FAMILY_NAME, "v1.3", "Lantern (2).zip")).toBe(
      `v1.2 of ${FAMILY_NAME} is back in your mods folder, as Lantern (2).zip: a file of that name was already there.`,
    );
  });
});

describe("THE UNDERCROFT, empty states (R18.14) and its folder door (Q18.6)", () => {
  it("should explain the folder not made yet, with no door at all", async () => {
    await wakeShelf("undercroftnone");
    await openFold();
    expect(said(must(body().querySelector(".crypt__say"), "sentence"))).toBe(
      "The stone hasn't put anything away yet. When an install replaces an older version of a file, the older archive waits here, in a folder named mods (older versions) beside your mods folder, until you bring it back. The stone makes the folder the first time it needs it.",
    );
    expect(body().querySelectorAll("button")).toHaveLength(0);
    expect(hasButton("Show in Explorer")).toBe(false);
  });

  it("should say the folder is empty, with `Show in Explorer` beside `Close ▾`", async () => {
    await wakeShelf("undercroftempty");
    expect(hasButton("Show in Explorer")).toBe(false);
    await openFold();
    expect(said(must(body().querySelector(".crypt__say"), "sentence"))).toBe(
      "Nothing is put away. The folder mods (older versions) beside your mods folder is empty. An archive you move into it by hand shows here too, the next time the stone looks.",
    );
    expect(body().querySelectorAll("button")).toHaveLength(0);
    const explorer = button("Show in Explorer");
    expect(explorer.className).toContain("rs-btn--secondary");
    expect(explorer.className).toContain("rs-btn--sm");
    const open = vi.spyOn(spine(), "openOlderVersionsFolder");
    explorer.click();
    await until(() => open.mock.calls.length === 1, "the folder shown");
  });

  it("should offer `Show in Explorer` on the one-item shelf, and take it away with the fold", async () => {
    await wakeShelf("undercroft");
    await openFold();
    expect(hasButton("Show in Explorer")).toBe(true);
    toggle().click();
    await until(() => body().hidden, "the fold closed");
    expect(hasButton("Show in Explorer")).toBe(false);
  });

  it("should say a failed read in ember, nothing changed, with `Try again`", async () => {
    await wakeShelf("undercroft");
    const read = vi
      .spyOn(spine(), "getOlderVersions")
      .mockRejectedValueOnce(new RiftFailure({ kind: "io", detail: { reason: "access is denied" } }));
    await openFold();
    await until(() => body().querySelector(".crypt__say.is-ember") !== null, "the failed read");
    expect(said(must(body().querySelector(".crypt__say"), "sentence"))).toBe(
      "The stone couldn't look inside mods (older versions): access is denied. Nothing in it was changed.",
    );
    expect(rows()).toHaveLength(0);
    button("Try again").click();
    await until(() => rows().length === 1, "Try again reads the folder");
    expect(read.mock.calls.length).toBeGreaterThanOrEqual(2);
  });
});
