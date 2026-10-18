// v0.5 — VERSIONS OF ONE FILE in the Rift (#00015 §18.3, R18.1–R18.4, R18.7,
// with the main session's rulings in §18.17): a family is one entry, its lead
// carrying its version and the older mark, its rail warn while an older version
// is still installed; each older version is a row inside it after the lead's
// own options; a lead leaf in your world only from an older version reads
// `INSTALLED v1.2`; `Enter` on a version row opens THE VERSIONS card in Your
// World; an answer from an older version says so. Every name is the mock's
// invented one (src/services/mock/versions.ts).

import { afterEach, describe, expect, it, vi } from "vitest";
import type { LibraryPackage } from "@/types/rift";
import { useLibrary } from "@/composables/useLibrary";
import { useVersions } from "@/composables/useVersions";
import { useWorld } from "@/composables/useWorld";
import { CANDLE_NAME, FAMILY_NAME, MAIN_NAME, familyFileName } from "@/services/mock/versions";
import { olderMark, versionLabel } from "@/services/versions";
import {
  button,
  hasButton,
  idle,
  press,
  sleep,
  spine,
  text,
  textOf,
  typeQuery,
  until,
  wakeOn,
  type Scene,
} from "./harness";

afterEach(() => {
  vi.restoreAllMocks();
  sleep();
});

function must<T>(found: T | null | undefined, what: string): T {
  if (found === null || found === undefined) throw new Error(`The stone drew no ${what}`);
  return found;
}

/** A version of the family, as the library lists it. */
const member = (version: "1.3" | "1.2" | "1.1"): LibraryPackage =>
  must(
    useLibrary().packages.value.find((p) => p.package.fileName === familyFileName(version)),
    `package v${version}`,
  );

const mainOf = (pkg: LibraryPackage): string =>
  must(
    pkg.tree.find((n) => n.label === MAIN_NAME),
    `${MAIN_NAME} in ${pkg.package.fileName}`,
  ).optionId ?? "";

const items = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('#rift-tree [role="treeitem"]')];
const item = (key: string): HTMLElement =>
  must(document.querySelector<HTMLElement>(`#rift-tree [data-key="${CSS.escape(key)}"]`), `tree row ${key}`);
/** A treeitem's own line (never its nested group's). */
const line = (el: HTMLElement): HTMLElement => must(el.querySelector<HTMLElement>(":scope > .ltr__row"), "row line");
const lineText = (el: HTMLElement): string => textOf(line(el)).replace(/\s+/g, " ").trim();
const label = (el: HTMLElement): string => textOf(must(line(el).querySelector(".ltr__label"), "label")).trim();
const card = (): HTMLElement | null => document.querySelector<HTMLElement>(".world .vc, .vc");
const cardRows = (): HTMLElement[] => [...(card()?.querySelectorAll<HTMLElement>(".vc__row") ?? [])];

/** An older version as the spine names it, for the pure rules. */
const olderAt = (installed: number) => ({ packageId: "p", fileName: "f.zip", version: "1.2", installed, matches: [] });

const cardOf = (name: string): HTMLElement | null =>
  [...document.querySelectorAll<HTMLElement>("#rift-answers .card")].find((c) =>
    textOf(c.querySelector(".card__title") ?? c).includes(name),
  ) ?? null;

async function wakeFamily(scene: Scene): Promise<void> {
  await wakeOn(scene);
  await until(() => useLibrary().packages.value.some((p) => p.family?.older.length), "the family in the library");
  await until(() => items().length > 0, "the tree");
}

async function expandLead(): Promise<HTMLElement> {
  const lead = member("1.3");
  useLibrary().expand(lead.package.id);
  await until(() => item(lead.package.id).getAttribute("aria-expanded") === "true", "the lead open");
  return item(lead.package.id);
}

describe("THE RIFT, one entry per family (R18.1)", () => {
  it("should draw the stacked family as one row with its version, the warn rail and `+1 older, installed`", async () => {
    await wakeFamily("family");
    const lead = member("1.3");
    const row = item(lead.package.id);
    // one row of that name at level 1, and the library's twelve packages fold to eleven entries
    const top = items().filter((r) => r.getAttribute("aria-level") === "1");
    expect(top.filter((r) => label(r) === FAMILY_NAME)).toHaveLength(1);
    expect(useLibrary().packages.value).toHaveLength(12);
    expect(top).toHaveLength(11);
    expect(top.every((r) => r.getAttribute("aria-setsize") === "11")).toBe(true);
    expect(document.querySelector(`#rift-tree [data-key="${CSS.escape(member("1.2").package.id)}"]`)).toBeNull();

    expect(textOf(must(line(row).querySelector(".ltr__ver"), "version")).trim()).toBe("v1.3");
    expect(line(row).classList.contains("rail--warn")).toBe(true);
    const mark = must(line(row).querySelector<HTMLElement>(".ltr__older"), "older mark");
    expect(mark.classList.contains("is-warn")).toBe(true);
    expect(textOf(must(mark.querySelector(".ltr__older-full"), "the mark's words")).trim()).toBe("+1 older, installed");
    expect(textOf(must(mark.querySelector(".rs-sr-only"), "the mark's hidden words")).trim()).toBe(
      "1 older version of this file, still installed",
    );
    expect(mark.getAttribute("title")).toBe("1 older version of this file, still installed");
    // the counts count this package's options only (two numbers for two things)
    expect(lineText(row)).toContain("1 option · 1 in your world");
    // the mark's words are part of what describes the row
    const described = (row.getAttribute("aria-describedby") ?? "").split(/\s+/);
    expect(described).toContain(mark.id);
  });

  it("should keep R16.2's row for a package in no family: no version, no older mark", async () => {
    await wakeFamily("family");
    const lead = member("1.3").package.id;
    const others = items().filter((r) => r.getAttribute("aria-level") === "1" && r.dataset.key !== lead);
    expect(others.length).toBe(10);
    for (const r of others) {
      expect(line(r).querySelector(".ltr__ver")).toBeNull();
      expect(line(r).querySelector(".ltr__older")).toBeNull();
      expect(line(r).classList.contains("rail--warn")).toBe(false);
    }
  });

  it("should read `+1 older` faint, and the rail gold, once no older version has an install", async () => {
    await wakeFamily("familythree");
    const leadId = member("1.3").package.id;
    expect(textOf(must(line(item(leadId)).querySelector(".ltr__older-full"), "mark")).trim()).toBe(
      "+2 older, installed",
    );
    await spine().uninstall(mainOf(member("1.2")));
    await until(() => !line(item(leadId)).classList.contains("rail--warn"), "the warn rail going out");
    const mark = must(line(item(leadId)).querySelector<HTMLElement>(".ltr__older"), "older mark");
    expect(textOf(must(mark.querySelector(".ltr__older-full"), "mark")).trim()).toBe("+2 older");
    expect(mark.classList.contains("is-warn")).toBe(false);
    expect(textOf(must(mark.querySelector(".rs-sr-only"), "hidden")).trim()).toBe(
      "2 older versions of this file in your mods folder",
    );
    expect(line(item(leadId)).classList.contains("rail--world")).toBe(true);
  });

  it("should word the mark from `family.older` alone (the pure rule the row reads)", () => {
    expect(olderMark([])).toBeNull();
    expect(olderMark([olderAt(0)])).toStrictEqual({
      text: "+1 older",
      short: "+1",
      warn: false,
      hidden: "1 older version of this file in your mods folder",
    });
    expect(olderMark([olderAt(0), olderAt(2)])?.text).toBe("+2 older, installed");
    expect(olderMark([olderAt(1)])?.hidden).toBe("1 older version of this file, still installed");
    // a leading v is never doubled
    expect(versionLabel("v1.3")).toBe("v1.3");
    expect(versionLabel("1.3")).toBe("v1.3");
  });
});

describe("THE RIFT, inside the entry (R18.2)", () => {
  it("should list the lead's own option first, then each older version with its file and its state words", async () => {
    await wakeFamily("family");
    const row = await expandLead();
    const lead = member("1.3");
    const older = member("1.2");
    const inside = items().filter((r) => r.getAttribute("aria-level") === "2");
    expect(inside.map((r) => r.dataset.key)).toStrictEqual([mainOf(lead), older.package.id]);
    expect(inside.map((r) => r.getAttribute("aria-setsize"))).toStrictEqual(["2", "2"]);
    expect(inside.map((r) => r.getAttribute("aria-posinset"))).toStrictEqual(["1", "2"]);
    // the version rows sit after the lead's options in document order too
    expect(items().indexOf(must(inside[1], "the version row"))).toBeGreaterThan(items().indexOf(row));

    const version = item(older.package.id);
    expect(label(version)).toBe("v1.2 · older version");
    expect(version.getAttribute("aria-expanded")).toBe("false");
    const file = must(line(version).querySelector<HTMLElement>(".ltr__file"), "the version's file");
    expect(textOf(file).trim()).toBe(familyFileName("1.2"));
    expect(file.getAttribute("title")).toBe(familyFileName("1.2"));
    expect(file.classList.contains("rs-tail")).toBe(true);
    const state = must(line(version).querySelector<HTMLElement>(".ltr__count"), "the version's state words");
    expect(textOf(state).trim()).toBe("1 in your world");
    expect(state.classList.contains("is-world")).toBe(true);
    // no ground of its own; its rail is the leaf's plain gold, never warn
    expect(line(version).classList.contains("is-pkg")).toBe(false);
    expect(line(version).classList.contains("rail--world")).toBe(true);
    // no button inside a treeitem (Bold Choice 11)
    expect(version.querySelector("button")).toBeNull();
  });

  it("should open a version row onto that version's own tree, one level deeper, its leaves with their own chips", async () => {
    await wakeFamily("family");
    await expandLead();
    const older = member("1.2");
    const version = item(older.package.id);
    version.focus();
    press("ArrowRight");
    await until(() => version.getAttribute("aria-expanded") === "true", "the version row open");
    const own = items().filter((r) => r.getAttribute("aria-level") === "3");
    expect(own.map((r) => label(r))).toStrictEqual([MAIN_NAME, CANDLE_NAME]);
    expect(lineText(item(mainOf(older)))).toMatch(/INSTALLED$/);
    expect(lineText(item(mainOf(older)))).not.toContain("NOT INSTALLED");
    expect(lineText(must(own[1], "Candle Warm"))).toMatch(/NOT INSTALLED$/);
  });

  it("should order two older versions newest first, a version with nothing installed in faint words and no rail", async () => {
    await wakeFamily("familythree");
    await expandLead();
    const versions = items().filter((r) => r.getAttribute("aria-level") === "2" && line(r).querySelector(".ltr__file"));
    expect(versions.map(label)).toStrictEqual(["v1.2 · older version", "v1.1 · older version"]);
    expect(versions.map((r) => r.getAttribute("aria-posinset"))).toStrictEqual(["2", "3"]);
    expect(versions.map((r) => r.getAttribute("aria-setsize"))).toStrictEqual(["3", "3"]);
    const v11 = must(versions[1], "v1.1");
    const state = must(line(v11).querySelector<HTMLElement>(".ltr__count"), "state words");
    expect(textOf(state).trim()).toBe("nothing installed");
    expect(state.classList.contains("is-world")).toBe(false);
    expect([...line(v11).classList].some((c) => c.startsWith("rail--"))).toBe(false);
  });
});

describe("THE RIFT, a lead leaf in your world from an older version (R18.3)", () => {
  it("should read `INSTALLED v1.2`, gold, when only v1.2 of that option is installed", async () => {
    await wakeFamily("familyfresh");
    await expandLead();
    const leaf = item(mainOf(member("1.3")));
    const chip = must(line(leaf).querySelector(".ltr__chip-older"), "the INSTALLED v1.2 chip");
    expect(textOf(chip).trim()).toBe("INSTALLED v1.2");
    expect(lineText(leaf)).not.toContain("NOT INSTALLED");
    expect(line(leaf).classList.contains("rail--world")).toBe(true);
  });

  it("should keep a plain INSTALLED on a lead leaf installed itself over v1.2, the warn on the lead's rail", async () => {
    await wakeFamily("family");
    const row = await expandLead();
    const leaf = item(mainOf(member("1.3")));
    expect(line(leaf).querySelector(".ltr__chip-older")).toBeNull();
    expect(lineText(leaf)).toMatch(/INSTALLED$/);
    expect(line(row).classList.contains("rail--warn")).toBe(true);
  });
});

describe("THE VERSIONS card (R18.4)", () => {
  it("should open on Enter on a version row, and expand the row, while Enter on the lead still toggles it", async () => {
    await wakeFamily("family");
    const leadRow = await expandLead();
    leadRow.focus();
    press("Enter");
    await until(() => leadRow.getAttribute("aria-expanded") === "false", "Enter folds the lead");
    expect(card()).toBeNull();
    press("Enter");
    await until(() => leadRow.getAttribute("aria-expanded") === "true", "Enter opens the lead again");
    expect(card()).toBeNull();

    const version = item(member("1.2").package.id);
    version.focus();
    press("Enter");
    await until(() => card() !== null, "THE VERSIONS card");
    expect(version.getAttribute("aria-expanded")).toBe("true");
    expect(textOf(must(card()?.querySelector(".vc__title"), "head")).trim()).toBe("VERSIONS OF THIS FILE");
    expect(textOf(must(card()?.querySelector(".vc__name"), "name")).trim()).toBe(FAMILY_NAME);
    // the card stands in Your World, never on a tree row
    expect(card()?.closest("#rift-tree")).toBeNull();
  });

  it("should list each member newest first, `newest` on the lead, and `Keep only the newest…` with no `Put it away` on the stacked family", async () => {
    await wakeFamily("family");
    useVersions().open(member("1.2").package.id);
    await until(() => cardRows().length === 2, "two version rows");
    const [v13, v12] = cardRows() as [HTMLElement, HTMLElement];
    expect(textOf(must(v13.querySelector(".vc__ver"), "v")).trim()).toBe("v1.3");
    expect(textOf(must(v13.querySelector(".vc__badge"), "badge")).trim()).toBe("newest");
    expect(must(v13.querySelector(".vc__file"), "file").getAttribute("title")).toBe(familyFileName("1.3"));
    expect(textOf(must(v13.querySelector(".vc__line2"), "line 2")).trim()).toBe("1 option in your world");
    expect(v13.classList.contains("has-world")).toBe(true);

    expect(textOf(must(v12.querySelector(".vc__ver"), "v")).trim()).toBe("v1.2");
    expect(v12.querySelector(".vc__badge")).toBeNull();
    expect(
      textOf(must(v12.querySelector(".vc__line2"), "line 2"))
        .replace(/\s+/g, " ")
        .trim(),
    ).toBe("1 option in your world, under v1.3");
    expect(textOf(must(v12.querySelector(".is-warn"), "the warn words")).trim()).toBe(", under v1.3");
    expect(hasButton("Keep only the newest…")).toBe(true);
    expect(button("Keep only the newest…").className).toMatch(/primary/);
    expect(hasButton("Put it away")).toBe(false);
  });

  it("should offer `Put it away` on an older version with no installs only", async () => {
    await wakeFamily("familythree");
    useVersions().open(member("1.3").package.id);
    await until(() => cardRows().length === 3, "three version rows");
    const doors = cardRows().map((r) => [...r.querySelectorAll("button")].map((b) => textOf(b).trim()));
    expect(doors).toStrictEqual([[], [], ["Put it away"]]);
    expect(textOf(must(cardRows()[2]?.querySelector(".vc__line2 span"), "line 2")).trim()).toBe("nothing installed");
    expect(hasButton("Keep only the newest…")).toBe(true);
  });

  it("should say the newest is not installed, and hide `Keep only the newest…` when nothing older than v1.2 remains", async () => {
    await wakeFamily("familyfresh");
    useVersions().open(member("1.3").package.id);
    await until(() => cardRows().length === 2, "two version rows");
    expect(text()).toContain(
      "v1.3 is in your mods folder but not installed. Open its options in the Rift and install the one you use: it takes v1.2's place in one step.",
    );
    expect(hasButton("Keep only the newest…")).toBe(false);
    expect(textOf(must(cardRows()[1]?.querySelector(".vc__line2"), "line 2")).trim()).toBe("1 option in your world");
  });

  it("should return the pane to what it held on Esc, or on the pane's own Esc", async () => {
    await wakeFamily("family");
    const leadOption = mainOf(member("1.3"));
    await useWorld().open(leadOption);
    await until(() => document.querySelector(".odetail") !== null, "the option's detail");
    const version = (await expandLead(), item(member("1.2").package.id));
    version.focus();
    press("Enter");
    await until(() => card() !== null, "THE VERSIONS card");
    expect(document.querySelector(".odetail")).toBeNull();
    press("Escape");
    await until(() => card() === null, "Esc closes the card");
    expect(useWorld().selectedId.value).toBe(leadOption);
    await until(() => document.querySelector(".odetail") !== null, "the option's detail back");

    useVersions().open(member("1.3").package.id);
    await until(() => card() !== null, "the card again");
    must(document.querySelector<HTMLButtonElement>(".world__esc"), "the pane's Esc").click();
    await until(() => card() === null, "the pane's Esc closes the card");
    expect(useWorld().selectedId.value).toBe(leadOption);
  });

  it("should close on Esc with nothing selected under it", async () => {
    await wakeFamily("family");
    await expandLead();
    const version = item(member("1.2").package.id);
    version.focus();
    press("Enter");
    await until(() => card() !== null, "THE VERSIONS card");
    press("Escape");
    await until(() => card() === null, "Esc closes the card");
    expect(useVersions().leadId.value).toBeNull();
  });
});

describe("Your World, an option among its file's versions (R18.4 doors)", () => {
  it("should offer `Install v1.3…` on a lead option in your world only from v1.2", async () => {
    await wakeFamily("familyfresh");
    await useWorld().open(mainOf(member("1.3")));
    await until(() => document.querySelector(".odetail__family") !== null, "the family line");
    expect(textOf(must(document.querySelector(".odetail__family"), "line")).trim()).toBe(
      "v1.2 of this option is in your world. Installing v1.3 takes its place in one step.",
    );
    expect(button("Install v1.3…").className).toMatch(/primary/);
    button("Install v1.3…").click();
    await until(() => hasButton("Install ▶") && idle(button("Install ▶")), "THE CROSSING");
  });

  it("should offer `Keep only the newest…` on a lead option installed over v1.2, opening THE RETURN — OLDER VERSIONS", async () => {
    await wakeFamily("family");
    await useWorld().open(mainOf(member("1.3")));
    await until(() => document.querySelector(".odetail__family") !== null, "the family line");
    expect(textOf(must(document.querySelector(".odetail__family"), "line")).trim()).toBe(
      "v1.2 of this option is still installed under it, so any of its files v1.3 lacks are still in your game.",
    );
    button("Keep only the newest…").click();
    await until(() => text().includes("THE RETURN — OLDER VERSIONS"), "the keep-newest plan");
  });

  it("should say an older version's option is from v1.2, and keep `Install…`", async () => {
    await wakeFamily("familythree");
    await useWorld().open(mainOf(member("1.1")));
    await until(() => document.querySelector(".odetail__family") !== null, "the family line");
    expect(textOf(must(document.querySelector(".odetail__family"), "line")).trim()).toBe(
      "This option is from v1.1, an older version. v1.3 is in your mods folder.",
    );
    expect(hasButton("Install…")).toBe(true);
  });
});

describe("An older version's answer in the flat answers (R18.7, §18.17)", () => {
  it("should mark an answer only v1.2 holds `only in v1.2, an older version`", async () => {
    await wakeFamily("family");
    await typeQuery("candle warm");
    await until(() => cardOf(CANDLE_NAME) !== null, "the Candle Warm answer");
    const cards = [...document.querySelectorAll<HTMLElement>("#rift-answers .card")];
    expect(cards.filter((c) => textOf(c).includes(CANDLE_NAME))).toHaveLength(1);
    const meta = textOf(must(cardOf(CANDLE_NAME)?.querySelector(".card__metatext"), "meta line"));
    expect(meta).toMatch(/ · only in v1\.2, an older version$/);
  });

  it("should leave out v1.2's answer when the lead's same option answers too, and leave the lead's unmarked", async () => {
    await wakeFamily("family");
    await typeQuery("glow_01");
    await until(() => cardOf(MAIN_NAME) !== null, "the Main answer");
    const mains = [...document.querySelectorAll<HTMLElement>("#rift-answers .card")].filter((c) =>
      textOf(c.querySelector(".card__title") ?? c).includes(MAIN_NAME),
    );
    expect(mains).toHaveLength(1);
    expect(mains[0]?.querySelector(".card__older")).toBeNull();
  });

  it("should mark a kept answer of a matched option `in v1.2, an older version`", async () => {
    await wakeFamily("family");
    const leadMain = mainOf(member("1.3"));
    const older = member("1.2");
    const olderMain = mainOf(older);
    const backend = spine();
    const real = backend.search.bind(backend);
    // the spine keeps an older answer the lead's option did not give (a file only it holds):
    // stand in that answer for v1.2's Main, which `matches` names
    vi.spyOn(backend, "search").mockImplementation(async (query) =>
      (await real(query)).map((hit) =>
        hit.optionId === leadMain ? { ...hit, optionId: olderMain, packageId: older.package.id } : hit,
      ),
    );
    await typeQuery("glow_01");
    await until(() => Boolean(cardOf(MAIN_NAME)?.querySelector(".card__older")), "the Main answer, marked as v1.2's");
    const meta = textOf(must(cardOf(MAIN_NAME)?.querySelector(".card__metatext"), "meta line"));
    expect(meta).toMatch(/ · in v1\.2, an older version$/);
    expect(meta).not.toContain("only in");
  });
});
