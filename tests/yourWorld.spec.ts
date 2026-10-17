// v0.3 YOUR WORLD's doors (WINDOW-CONTRACT § v0.3 "Legacy data", design
// system #00015 R16.26): every load-order row ends with an uninstall door, so
// an install with no package in the mods folder still leaves the game. An
// adopted pak install that never had a package goes by the opaque id the
// spine gave it, and one whose package left the folder goes the same way: the
// journal knows every file.

import { afterEach, describe, expect, it, vi } from "vitest";
import { useInstalled } from "@/composables/useInstalled";
import { useNavigation } from "@/composables/useNavigation";
import { useToasts } from "@/composables/useToasts";
import { button, idle, sleep, spine, textAt, textOf, until, wakeOn } from "./harness";

afterEach(() => {
  vi.restoreAllMocks();
  sleep();
});

function must<T>(found: T | null | undefined, what: string): T {
  if (found === null || found === undefined) throw new Error(`YOUR WORLD drew no ${what}`);
  return found;
}

const loadOrder = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>(".lorder__list .lrow")];
const nameOf = (row: Element): string => textOf(must(row.querySelector(".lrow__name"), "name")).trim();
const orderRow = (name: string): HTMLElement | undefined => loadOrder().find((r) => nameOf(r) === name);
const marksOf = (row: Element): string[] => [...row.querySelectorAll(".lrow__mark")].map((m) => textOf(m).trim());
const door = (row: Element, name: string): HTMLButtonElement =>
  must(row.querySelector<HTMLButtonElement>(`[aria-label="Uninstall ${name}"]`), `the door of ${name}`);
const returnRows = (): string[][] =>
  [...document.querySelectorAll(".home__return .home__row")].map((r) =>
    [...r.children].map((c) => textOf(c).replace(/\s+/g, " ").trim()),
  );
const toastLines = (): string[] => useToasts().toasts.value.map((t) => t.line);

/** The opaque option id the spine gave an install: read, never spelled. */
const idOf = (name: string): string =>
  must(
    useInstalled().view.value?.installs.find((r) => r.name === name),
    `the install of ${name}`,
  ).optionId;

async function openYourWorld(scene: "" | "emptymods" = ""): Promise<void> {
  await wakeOn(scene);
  useNavigation().go("world");
  await until(() => loadOrder().length > 0, "the load order");
}

describe("the load order's doors", () => {
  it("should uninstall an adopted pak install with no package through its door, by the id the spine gave it", async () => {
    await openYourWorld();
    expect(loadOrder().map(nameOf)).toStrictEqual([
      "Pawn Banter Quieter",
      "Brine Water Clarity",
      "Sacred Arbor Fog Removal",
    ]);
    const stand = must(orderRow("Pawn Banter Quieter"), "the adopted stand-in");
    expect(marksOf(stand)).toStrictEqual(["adopted", "no package"]);
    expect(must(stand.querySelector(".lrow__mark.is-warn"), "no-package mark").getAttribute("title")).toBe(
      "Adopted: no package in your mods folder matches it. Uninstall still works: the journal knows every file.",
    );
    // a package-less install stays one press from leaving: every row has its door
    expect(loadOrder().map((r) => r.querySelector(".lrow__door") !== null)).toStrictEqual([true, true, true]);

    const id = idOf("Pawn Banter Quieter");
    const planned = vi.spyOn(spine(), "planUninstall");
    const removed = vi.spyOn(spine(), "uninstall");
    door(stand, "Pawn Banter Quieter").click();
    await until(() => textAt(".home__return .home__h") !== null, "THE RETURN");
    expect(planned.mock.calls).toStrictEqual([[id]]);
    expect(textAt(".home__return .home__h")).toBe("THE RETURN — 1 file leaves your game");
    expect(returnRows()).toStrictEqual([["pak_mods/zrift-0000-PawnBanterQuieter.pak", "− removed"]]);

    await until(() => idle(button("Uninstall Pawn Banter Quieter")), "the plan's Uninstall");
    button("Uninstall Pawn Banter Quieter").click();
    await until(() => orderRow("Pawn Banter Quieter") === undefined, "the stand-in gone", 4000);
    expect(removed.mock.calls).toStrictEqual([[id]]);
    expect(toastLines()).toContain("Pawn Banter Quieter has returned to the Rift. 1 file removed.");
    expect(loadOrder().map((r) => r.getAttribute("aria-label"))).toStrictEqual([
      "Brine Water Clarity, position 1 of 2. Alt plus arrow keys move it.",
      "Sacred Arbor Fog Removal, position 2 of 2. Alt plus arrow keys move it.",
    ]);
  });

  it("should mark an adopted install whose package is in the folder as adopted only", async () => {
    await openYourWorld();
    const ox = must(
      [...document.querySelectorAll<HTMLElement>(".home__row--install")].find((r) =>
        textOf(r).includes("Ox Cart Express"),
      ),
      "Ox Cart Express",
    );
    expect(textOf(ox)).toContain("adopted");
    expect(textOf(ox)).not.toContain("no package");
    expect(ox.querySelector('[aria-label="Uninstall Ox Cart Express"]')).not.toBeNull();
  });
});

describe("an emptied mods folder", () => {
  it("should keep a door on every install, and uninstall one whose package left the folder", async () => {
    await openYourWorld("emptymods");
    expect(loadOrder().map((r) => [nameOf(r), marksOf(r)])).toStrictEqual([
      ["Pawn Banter Quieter", ["adopted", "no package"]],
      ["Brine Water Clarity", ["no package"]],
      ["Sacred Arbor Fog Removal", ["no package"]],
    ]);
    const brine = must(orderRow("Brine Water Clarity"), "Brine Water Clarity");
    expect(must(brine.querySelector(".lrow__mark.is-warn"), "no-package mark").getAttribute("title")).toBe(
      "The package is no longer in your mods folder. Uninstall still works: the journal knows every file.",
    );

    const id = idOf("Brine Water Clarity");
    const removed = vi.spyOn(spine(), "uninstall");
    door(brine, "Brine Water Clarity").click();
    await until(() => textAt(".home__return .home__h") !== null, "THE RETURN");
    expect(returnRows()).toStrictEqual([["pak_mods/zrift-0001-BrineWaterClarity.pak", "− removed"]]);
    await until(() => idle(button("Uninstall Brine Water Clarity")), "the plan's Uninstall");
    button("Uninstall Brine Water Clarity").click();
    await until(() => orderRow("Brine Water Clarity") === undefined, "Brine Water Clarity gone", 4000);
    expect(removed.mock.calls).toStrictEqual([[id]]);
    expect(toastLines()).toContain("Brine Water Clarity has returned to the Rift. 1 file removed.");
    expect(loadOrder().map(nameOf)).toStrictEqual(["Pawn Banter Quieter", "Sacred Arbor Fog Removal"]);
  });
});
