// v0.5 — THE FAR WORLD over versions of one file (#00015 §18.5, R18.8): one
// UPDATES row per family, the lead's, and under it one `↳` line that says how
// the older versions stand, chosen by two facts the window holds (an older
// version installed? the lead installed?), with the door that case needs. No
// row anywhere shows an older version's own update state. Every name is the
// mock's invented one.

import { afterEach, describe, expect, it, vi } from "vitest";
import type { LibraryPackage } from "@/types/rift";
import { useLibrary } from "@/composables/useLibrary";
import { useNavigation } from "@/composables/useNavigation";
import { useVersions } from "@/composables/useVersions";
import { FAMILY_NAME, MAIN_NAME, familyFileName } from "@/services/mock/versions";
import { farSubRow } from "@/services/versions";
import { button, hasButton, sleep, spine, textOf, until, wakeOn, type Scene } from "./harness";

afterEach(() => {
  vi.restoreAllMocks();
  sleep();
});

function must<T>(found: T | null | undefined, what: string): T {
  if (found === null || found === undefined) throw new Error(`The far world drew no ${what}`);
  return found;
}

const member = (version: "1.3" | "1.2" | "1.1"): LibraryPackage =>
  must(
    useLibrary().packages.value.find((p) => p.package.fileName === familyFileName(version)),
    `package v${version}`,
  );

const farRow = (packageId: string): HTMLElement | null => document.getElementById(`far-linked-${packageId}`);
/** The `↳` line under a row: the first sub-row after it that carries one. */
function olderLine(packageId: string): HTMLElement | null {
  let next = farRow(packageId)?.nextElementSibling ?? null;
  while (next?.classList.contains("far__subrow")) {
    const found = next.querySelector<HTMLElement>(".far__older");
    if (found) return found;
    next = next.nextElementSibling;
  }
  return null;
}
const words = (line: HTMLElement): string =>
  textOf(must(line.querySelector(":scope > span:not([aria-hidden])"), "the line's words")).trim();

/** An older version as the spine names it, for the pure rule. */
const olderAt = (version: string, installed: number) => ({
  packageId: version,
  fileName: `${version}.zip`,
  version,
  installed,
  matches: [],
});

async function openFar(scene: Scene): Promise<string> {
  await wakeOn(scene);
  await until(() => useLibrary().packages.value.some((p) => p.family?.older.length), "the family in the library");
  const lead = member("1.3").package.id;
  useNavigation().go("nexus");
  await until(() => olderLine(lead) !== null, "the family's UPDATES row and its line");
  return lead;
}

describe("THE FAR WORLD, one UPDATES row per family (R18.8)", () => {
  it("should show one row for the family, the warn line under it, and no row for v1.2", async () => {
    const lead = await openFar("family");
    const named = [...document.querySelectorAll<HTMLElement>(".far__linkedrow")].filter((r) =>
      textOf(r).includes(FAMILY_NAME),
    );
    expect(named).toHaveLength(1);
    expect(farRow(member("1.2").package.id)).toBeNull();
    const line = must(olderLine(lead), "the line");
    expect(line.classList.contains("is-warn")).toBe(true);
    const glyph = must(line.querySelector(".far__older-glyph"), "glyph");
    expect(textOf(glyph).trim()).toBe("↳");
    expect(glyph.getAttribute("aria-hidden")).toBe("true");
    expect(words(line)).toBe("v1.2 is in your mods folder too, still installed under v1.3.");
    const door = must(line.querySelector<HTMLButtonElement>("button"), "the door");
    expect(textOf(door).trim()).toBe("Show versions in the Rift");
    expect(door.className).toContain("rs-btn--secondary");
    expect(door.className).toContain("rs-btn--sm");
    // the line spans the table: no column widens
    expect(line.closest("td")?.getAttribute("colspan")).toBe("5");
    // no row anywhere shows v1.2's own update state
    expect(document.querySelector(".far__body")?.textContent).not.toContain(familyFileName("1.2"));
  });

  it("should say v1.3 takes v1.2's place when only v1.2 is installed, with `Show in the Rift` on its first option", async () => {
    const lead = await openFar("familyfresh");
    const line = must(olderLine(lead), "the line");
    expect(line.classList.contains("is-text")).toBe(true);
    expect(words(line)).toBe("v1.2 is installed. Install v1.3 from the Rift and it takes v1.2's place.");
    const door = must(line.querySelector<HTMLButtonElement>("button"), "the door");
    expect(textOf(door).trim()).toBe("Show in the Rift");
    expect(door.className).toContain("rs-btn--primary");
    expect(door.className).toContain("rs-btn--sm");
    door.click();
    const library = useLibrary();
    const first = must(member("1.3").tree.find((n) => n.label === MAIN_NAME)?.optionId, "the lead's option");
    await until(() => useNavigation().page.value === "rift", "THE RIFT");
    await until(() => library.focusedKey.value === first, "the lead's first option focused");
    expect(library.isExpanded(lead)).toBe(true);
    expect(useVersions().leadId.value).toBeNull();
  });

  it("should say faintly when no older version has an install", async () => {
    await wakeOn("family");
    await until(() => useLibrary().packages.value.some((p) => p.family?.older.length), "the family");
    await spine().uninstall(must(member("1.2").tree[0]?.optionId, "v1.2's option"));
    await until(() => member("1.3").family?.older[0]?.installed === 0, "v1.2 uninstalled");
    const lead = member("1.3").package.id;
    useNavigation().go("nexus");
    await until(() => olderLine(lead) !== null, "the line");
    const line = must(olderLine(lead), "the line");
    expect(line.classList.contains("is-faint")).toBe(true);
    expect(words(line)).toBe("v1.2 is in your mods folder too, with nothing installed.");
    expect(textOf(must(line.querySelector("button"), "door")).trim()).toBe("Show versions in the Rift");
  });

  it("should name two older versions together", async () => {
    const lead = await openFar("familythree");
    expect(words(must(olderLine(lead), "the line"))).toBe(
      "v1.2 and v1.1 are in your mods folder too, still installed under v1.3.",
    );
  });

  it("should open THE VERSIONS card from `Show versions in the Rift`, the lead open and its first version row focused", async () => {
    const lead = await openFar("family");
    button("Show versions in the Rift").click();
    const library = useLibrary();
    await until(() => useNavigation().page.value === "rift", "THE RIFT");
    await until(() => library.focusedKey.value === member("1.2").package.id, "the first version row focused");
    expect(library.isExpanded(lead)).toBe(true);
    await until(() => document.querySelector(".vc") !== null, "THE VERSIONS card");
    expect(useVersions().leadId.value).toBe(lead);
    expect(hasButton("Keep only the newest…")).toBe(true);
  });

  it("should count three or more older versions instead of naming them (the pure rule)", () => {
    expect(farSubRow([], "v1.4", true)).toBeNull();
    expect(farSubRow([olderAt("1.3", 0), olderAt("1.2", 0), olderAt("1.1", 0)], "v1.4", true)?.text).toBe(
      "3 older versions are in your mods folder too, with nothing installed.",
    );
    expect(farSubRow([olderAt("1.3", 1), olderAt("1.2", 0)], "v1.4", false)).toStrictEqual({
      text: "v1.3 and v1.2 are installed. Install v1.4 from the Rift and it takes their place.",
      tone: "text",
      door: "rift",
      doorLabel: "Show in the Rift",
    });
  });
});
