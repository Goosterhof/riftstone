// v0.3 THE LIBRARY TREE (WINDOW-CONTRACT § v0.3 "The library tree", design
// system #00015 §16.1): with no words in the well and no facet narrowing the
// Rift is the mods folder as a classic tree, every package collapsed; words or
// a label chip bring back the flat answers, uncapped; clearing returns the
// tree exactly as it was left. A WAI-ARIA tree: one tab stop, ↑ ↓ ← → Home
// End, Enter opens an option in YOUR WORLD. The trouble fold stays above it,
// outside its role, and an install turns a row's marker gold.

import { afterEach, describe, expect, it, vi } from "vitest";
import type { SearchResult } from "@/types/rift";
import { useLibrary } from "@/composables/useLibrary";
import { useNavigation } from "@/composables/useNavigation";
import { useSearch } from "@/composables/useSearch";
import { useStone } from "@/composables/useStone";
import { useWorld } from "@/composables/useWorld";
import {
  button,
  hasButton,
  idle,
  press,
  sleep,
  spine,
  text,
  textAt,
  textOf,
  typeQuery,
  until,
  wakeOn,
} from "./harness";

afterEach(() => {
  vi.restoreAllMocks();
  sleep();
});

/** The mods folder's eleven packages, as the tree shelves them: by display name, case-blind. */
const SHELF = [
  "Battahli Sun Bleach",
  "Brine Water Clarity",
  "Corrupted Download",
  "Lantern Glow Recolor",
  "Old Road Armor Pack",
  "Ox Cart Express",
  "Pawn Chatter Filter",
  "Readme Only",
  "REFramework",
  "Sacred Arbor Fog Removal",
  "Vernworth Wardrobe Collection",
];

function must<T>(found: T | null | undefined, what: string): T {
  if (found === null || found === undefined) throw new Error(`The tree carved no ${what}`);
  return found;
}

const tree = (): HTMLElement | null => document.getElementById("rift-tree");
const rows = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('#rift-tree [role="treeitem"]')];
const topRows = (): HTMLElement[] => rows().filter((r) => r.getAttribute("aria-level") === "1");
const answers = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>("#rift-answers [data-index]")];
/** The Rift's header line, its whitespace read as a reader hears it. */
const echo = (): string => (textAt(".rift__echo") ?? "").replace(/\s+/g, " ");

/** A node's text as a reader hears it: aria-hidden parts (the ⚔ glyph) are skipped. */
function spoken(node: Node): string {
  if (node.nodeType === Node.TEXT_NODE) return textOf(node);
  if (!(node instanceof Element) || node.getAttribute("aria-hidden") === "true") return "";
  return [...node.childNodes].map(spoken).join("");
}

/** The text of the elements an id-reference attribute points at, as a reader hears it. */
function referenced(item: Element, attribute: "aria-labelledby" | "aria-describedby"): string {
  return (item.getAttribute(attribute) ?? "")
    .split(/\s+/)
    .filter(Boolean)
    .map((id) => spoken(must(document.getElementById(id), `#${id} (${attribute})`)))
    .join(" ")
    .replace(/\s+/g, " ")
    .trim();
}

/** A treeitem's accessible name: its own label, never the rows nested inside it (R16.2). */
const nameOf = (item: Element): string => referenced(item, "aria-labelledby");
/** A treeitem's description: the state its row shows (counts, verdict, chips, the hidden "conflict"). */
const descriptionOf = (item: Element): string => referenced(item, "aria-describedby");

/** Every visible row, in reading order, as `level:label`. */
const visible = (): string[] => rows().map((r) => `${r.getAttribute("aria-level")}:${nameOf(r)}`);

/** The visible row with this label at this level (a package and its one option share a name). */
const row = (label: string, level = 1): HTMLElement | undefined =>
  rows().find((r) => nameOf(r) === label && r.getAttribute("aria-level") === String(level));

/** A row's own line, not the group of children nested inside its treeitem. */
const lineOf = (item: HTMLElement): HTMLElement =>
  must(
    [...item.children].find((c): c is HTMLElement => c instanceof HTMLElement && c.classList.contains("ltr__row")),
    "row line",
  );

const chipOf = (item: HTMLElement): string | null => {
  const chip = lineOf(item).querySelector(".rs-chip");
  return chip ? textOf(chip).trim() : null;
};

/** How many kind-badge groups the visible row with this label at this level carries on its own line. */
const badges = (label: string, level: number): number =>
  lineOf(must(row(label, level), label)).querySelectorAll(".ltr__badges, .ltr__kinds").length;

const focusedRow = (): string | null => {
  const active = document.activeElement;
  if (!(active instanceof HTMLElement) || active.getAttribute("role") !== "treeitem") return null;
  return `${active.getAttribute("aria-level")}:${nameOf(active)}`;
};

/** Put the keyboard on a row, the way a click or a Tab does. */
async function focusRow(item: HTMLElement): Promise<void> {
  item.focus();
  await until(() => document.activeElement === item, `the focus on ${nameOf(item)}`);
}

/** Press a key on the focused row and wait for the focus to land on `expected` (`level:label`). */
async function step(key: string, expected: string): Promise<void> {
  press(key);
  await until(() => focusedRow() === expected, `${key} landing on ${expected}`);
}

/** The NEXUS LABELS chip for the Visuals category. */
const visuals = (): HTMLButtonElement =>
  must(
    [...document.querySelectorAll<HTMLButtonElement>("#nexus-labels .lchip")].find(
      (b) => textOf(b.querySelector(".lchip__name") ?? b).trim() === "Visuals",
    ),
    "Visuals chip",
  );

async function wakeOnTree(): Promise<void> {
  await wakeOn();
  await until(() => topRows().length === SHELF.length, "the library tree");
}

/** Open Vernworth, then its Hoods menu and the hood menu option, all by keyboard. */
async function openHoods(): Promise<void> {
  await focusRow(must(row("Vernworth Wardrobe Collection"), "Vernworth"));
  press("ArrowRight");
  await until(() => row("Hoods", 2) !== undefined, "Vernworth open");
  await step("ArrowDown", "2:Cloaks");
  await step("ArrowDown", "2:Hoods");
  press("ArrowRight");
  await until(() => row("Pilgrim Hood (Menu Option)", 3) !== undefined, "Hoods open");
  // → on an open branch steps to its first child
  await step("ArrowRight", "3:Pilgrim Hood (Menu Option)");
  press("ArrowRight");
  await until(() => row("Pilgrim Hood - Southern Dyes", 4) !== undefined, "the hood menu option open");
}

describe("the library tree: the Rift's idle view", () => {
  it("should open on one collapsed row per package, by display name, with one tab stop", async () => {
    await wakeOnTree();
    expect(must(tree(), "tree").getAttribute("role")).toBe("tree");
    expect(document.getElementById("rift-answers")).toBeNull();
    expect(topRows().map(nameOf)).toStrictEqual(SHELF);
    expect(rows()).toHaveLength(SHELF.length);
    // a package with options is a closed branch; the two that yielded none have nothing to open
    expect(topRows().map((r) => [nameOf(r), r.getAttribute("aria-expanded")])).toStrictEqual(
      SHELF.map((name) => [name, ["Corrupted Download", "Readme Only"].includes(name) ? null : "false"]),
    );
    expect(topRows().map((r) => [r.getAttribute("aria-setsize"), r.getAttribute("aria-posinset")])).toStrictEqual(
      SHELF.map((_, i) => [String(SHELF.length), String(i + 1)]),
    );
    expect(rows().filter((r) => r.getAttribute("tabindex") === "0")).toHaveLength(1);
    expect(echo()).toBe("Library · 11 packages · 32 options");
    expect(hasButton("Expand all") && hasButton("Collapse all")).toBe(true);
  });

  it("should expand a package by a click and a menu by →, down to a bundle node and its options", async () => {
    await wakeOnTree();
    const wardrobe = must(row("Vernworth Wardrobe Collection"), "Vernworth");
    lineOf(wardrobe).click();
    await until(() => wardrobe.getAttribute("aria-expanded") === "true", "Vernworth open by a click");
    expect(visible().slice(SHELF.length)).toStrictEqual(["2:Cloaks", "2:Hoods", "2:Mantles"]);
    expect(
      must(wardrobe.querySelector('[role="group"]'), "children group").contains(must(row("Hoods", 2), "Hoods")),
    ).toBe(true);

    await focusRow(must(row("Hoods", 2), "Hoods"));
    press("ArrowRight");
    await until(() => row("Pilgrim Hood (Menu Option)", 3) !== undefined, "Hoods open by →");
    await step("ArrowRight", "3:Pilgrim Hood (Menu Option)");
    press("ArrowRight");
    await until(() => row("Pilgrim Hood - Southern Dyes", 4) !== undefined, "the bundle node");
    await step("ArrowDown", "4:Pilgrim Hood - Variation 1");
    await step("ArrowDown", "4:Pilgrim Hood - Variation 2");
    await step("ArrowDown", "4:Pilgrim Hood - Southern Dyes");
    press("ArrowRight");
    await until(() => row("Pilgrim Hood - Variation 4", 5) !== undefined, "the bundle open");

    expect(visible().slice(SHELF.length)).toStrictEqual([
      "2:Cloaks",
      "2:Hoods",
      "3:Pilgrim Hood (Menu Option)",
      "4:Pilgrim Hood - Variation 1",
      "4:Pilgrim Hood - Variation 2",
      "4:Pilgrim Hood - Southern Dyes",
      "5:Pilgrim Hood - Variation 3",
      "5:Pilgrim Hood - Variation 4",
      "2:Mantles",
    ]);
    // a bundle is a branch that installs nothing: no option to select, no chip
    const bundle = must(row("Pilgrim Hood - Southern Dyes", 4), "bundle");
    expect(bundle.dataset.key?.startsWith("bundle|")).toBe(true);
    expect(bundle.getAttribute("aria-expanded")).toBe("true");
    expect(bundle.getAttribute("aria-selected")).toBeNull();
    expect(chipOf(bundle)).toBeNull();
    expect(chipOf(must(row("Pilgrim Hood - Variation 3", 5), "a dye"))).toBe("NOT INSTALLED");
    expect(
      ["Pilgrim Hood - Variation 1", "Pilgrim Hood - Variation 2", "Pilgrim Hood - Southern Dyes"].map((label) => {
        const item = must(row(label, 4), label);
        return [item.getAttribute("aria-setsize"), item.getAttribute("aria-posinset")];
      }),
    ).toStrictEqual([
      ["3", "1"],
      ["3", "2"],
      ["3", "3"],
    ]);
  });

  it("should close an open branch with ←, and step up to the parent from a child", async () => {
    await wakeOnTree();
    await openHoods();
    const hoods = must(row("Hoods", 2), "Hoods");
    await step("ArrowDown", "4:Pilgrim Hood - Variation 1");
    // a leaf: ← moves to its parent and closes nothing
    await step("ArrowLeft", "3:Pilgrim Hood (Menu Option)");
    expect(row("Pilgrim Hood - Variation 1", 4)).toBeDefined();
    // an open branch: ← closes it and keeps the focus
    press("ArrowLeft");
    await until(() => row("Pilgrim Hood - Variation 1", 4) === undefined, "the hood menu option closed");
    expect(focusedRow()).toBe("3:Pilgrim Hood (Menu Option)");
    // a closed branch: ← moves to its parent
    await step("ArrowLeft", "2:Hoods");
    press("ArrowLeft");
    await until(() => hoods.getAttribute("aria-expanded") === "false", "Hoods closed");
    await step("ArrowLeft", "1:Vernworth Wardrobe Collection");
  });

  it("should jump to the first and last visible rows on Home and End, and leave for the well above the first", async () => {
    await wakeOnTree();
    await openHoods();
    await step("End", "2:Mantles");
    await step("Home", "1:Battahli Sun Bleach");
    expect(rows().filter((r) => r.getAttribute("tabindex") === "0")).toStrictEqual([
      must(row("Battahli Sun Bleach"), "first row"),
    ]);
    press("ArrowUp");
    await until(() => document.activeElement?.id === "rift-search", "the well");
  });

  it("should name each row by its label alone and describe it by the state it shows (WCAG 1.3.1, 4.1.2)", async () => {
    await wakeOnTree();
    // no aria-label: it would replace the row's state words with its label
    expect(rows().filter((r) => r.hasAttribute("aria-label"))).toStrictEqual([]);
    // a package: its format, its counts or verdict, its UPDATE chip and the hidden "conflict"
    expect(descriptionOf(must(row("Brine Water Clarity"), "Brine"))).toBe("ZIP 1 option · 1 in your world conflict");
    expect(descriptionOf(must(row("Lantern Glow Recolor"), "Lantern"))).toBe("ZIP 1 option UPDATE");
    expect(descriptionOf(must(row("Corrupted Download"), "Corrupted Download"))).toBe("7Z won't open");
    // the name never pulls in the rows nested under an open package
    const lantern = must(row("Lantern Glow Recolor"), "Lantern");
    lineOf(lantern).click();
    await until(() => row("Lantern Glow Recolor", 2) !== undefined, "Lantern open");
    expect(nameOf(lantern)).toBe("Lantern Glow Recolor");
    // a leaf: its kind badges and its state chip
    const leaf = must(row("Lantern Glow Recolor", 2), "Lantern's option");
    expect(nameOf(leaf)).toBe("Lantern Glow Recolor");
    expect(descriptionOf(leaf)).toMatch(/NOT INSTALLED$/);
    // a conflicted leaf says so in words
    lineOf(must(row("Brine Water Clarity"), "Brine")).click();
    await until(() => row("Brine Water Clarity", 2) !== undefined, "Brine open");
    expect(descriptionOf(must(row("Brine Water Clarity", 2), "Brine's option"))).toMatch(/CONFLICT$/);
  });

  it("should zoom an option's screenshot on Space, as the answers do, and act as Enter on a row without one", async () => {
    // WINDOW-CONTRACT § v0.3 "The library tree": Space zooms as it does in the answers (§8.1)
    await wakeOnTree();
    const sun = must(row("Battahli Sun Bleach"), "Battahli");
    await focusRow(sun);
    press(" ");
    await until(() => sun.getAttribute("aria-expanded") === "true", "Space on a package opens it, as Enter does");
    await step("ArrowDown", "2:Battahli Sun Bleach");
    press(" ");
    await until(() => useNavigation().lightbox.value !== null, "the screenshot zoomed");
    expect(useNavigation().lightbox.value?.name).toBe("Battahli Sun Bleach");
    expect(useSearch().text.value).toBe("");
    press("Escape");
    // the lightbox hands the keyboard back to the row that zoomed it
    await until(() => focusedRow() === "2:Battahli Sun Bleach", "the focus back on the leaf");
  });

  it("should open every sibling branch of the focused row on *, as the APG's tree does", async () => {
    // WINDOW-CONTRACT § v0.3 "The library tree"; never a search, never a jump
    await wakeOnTree();
    await focusRow(must(row("Battahli Sun Bleach"), "Battahli"));
    press("*");
    await until(
      () => must(row("Vernworth Wardrobe Collection"), "Vernworth").getAttribute("aria-expanded") === "true",
      "every sibling open",
    );
    // every package with something to open is open; the two with nothing stay as they were
    expect(
      topRows()
        .filter((r) => r.getAttribute("aria-expanded") === "true")
        .map(nameOf),
    ).toStrictEqual(SHELF.filter((name) => !["Corrupted Download", "Readme Only"].includes(name)));
    expect(focusedRow()).toBe("1:Battahli Sun Bleach");
    expect(useSearch().text.value).toBe("");
  });

  it("should open an option in YOUR WORLD on Enter, and mark it selected", async () => {
    await wakeOnTree();
    await focusRow(must(row("Lantern Glow Recolor"), "Lantern"));
    press("ArrowRight");
    await until(() => row("Lantern Glow Recolor", 2) !== undefined, "Lantern open");
    await step("ArrowDown", "2:Lantern Glow Recolor");
    press("Enter");
    const option = must(row("Lantern Glow Recolor", 2), "Lantern's option");
    await until(() => textAt(".odetail__name") === "Lantern Glow Recolor", "the option in YOUR WORLD");
    expect(useWorld().selectedId.value).toBe(option.dataset.key);
    expect(option.getAttribute("aria-selected")).toBe("true");
    expect(lineOf(option).classList.contains("is-sel")).toBe(true);
    // the tree stays: opening an option is not a search
    expect(tree()).not.toBeNull();
    expect(hasButton("Install…")).toBe(true);
  });
});

describe("the tree and the answers", () => {
  it("should answer words with the flat answers, uncapped, and give the tree back as it was left when cleared", async () => {
    await wakeOnTree();
    await openHoods();
    const before = visible();
    const opened = [...useLibrary().expanded.value].sort();
    const sent = vi.spyOn(spine(), "search");

    await typeQuery("variation");
    await until(() => tree() === null && answers().length > 0, "the flat answers");
    expect(answers()).toHaveLength(18);
    expect(useSearch().results.value).toHaveLength(18);
    expect(echo()).toBe("18 answered “variation”");
    // every query the window sent asked for every match
    expect(new Set(sent.mock.calls.map(([q]) => q.limit))).toStrictEqual(new Set([0]));

    // another room and back: the memory is the session's, not the page's
    useNavigation().go("world");
    await until(() => tree() === null && document.getElementById("rift-answers") === null, "YOUR WORLD");
    useNavigation().go("rift");
    await typeQuery("");
    await until(() => tree() !== null && document.getElementById("rift-answers") === null, "the tree back");
    expect(visible()).toStrictEqual(before);
    expect([...useLibrary().expanded.value].sort()).toStrictEqual(opened);
  });

  it("should draw every answer the spine sends: the window caps nothing either", async () => {
    await wakeOnTree();
    const seed = must(
      (
        await spine().search({
          text: "lantern",
          filters: { installed: null, includeMenuOnly: false, categories: [], tags: [] },
          limit: 0,
        })
      ).at(0),
      "seed answer",
    );
    const many: SearchResult[] = Array.from({ length: 250 }, (_, i) => ({
      ...seed,
      optionId: `${seed.optionId}~${i}`,
      name: `Vernworth Cloak ${i + 1}`,
      hasScreenshot: false,
    }));
    vi.spyOn(spine(), "search").mockResolvedValue(many);
    await typeQuery("cloak");
    await until(() => answers().length === 250, "all 250 answers", 6000);
    expect(echo()).toBe("250 answered “cloak”");
    expect(textAt(".pillar__count")).toBe("250 ANSWERED");
  });

  it("should give a label chip the flat answers with an empty well, and the tree again when it lets go", async () => {
    await wakeOnTree();
    await until(() => document.querySelector("#nexus-labels .lchip") !== null, "the label chips");
    visuals().click();
    await until(() => tree() === null && answers().length === 2, "the Visuals answers");
    expect(useSearch().text.value).toBe("");
    expect(
      answers()
        .map((a) => textOf(must(a.querySelector(".card__title"), "title")).trim())
        .sort(),
    ).toStrictEqual(["Battahli Sun Bleach", "Lantern Glow Recolor"]);
    expect(echo()).toBe("2 options · Visuals");

    visuals().click();
    await until(() => tree() !== null && document.getElementById("rift-answers") === null, "the tree back");
    expect(topRows()).toHaveLength(SHELF.length);
  });

  it("should step from the well into the tree on ↓, onto the row it last left", async () => {
    await wakeOnTree();
    const well = must(document.getElementById("rift-search"), "well");
    well.focus();
    press("ArrowDown");
    await until(() => focusedRow() === "1:Battahli Sun Bleach", "the first row");

    await focusRow(must(row("Ox Cart Express"), "Ox Cart"));
    well.focus();
    press("ArrowDown");
    await until(() => focusedRow() === "1:Ox Cart Express", "the row the tree last held");
    expect(rows().filter((r) => r.getAttribute("tabindex") === "0")).toStrictEqual([
      must(row("Ox Cart Express"), "Ox Cart"),
    ]);
  });

  it("should send a letter typed in the tree to the well, never jump by it", async () => {
    await wakeOnTree();
    await focusRow(must(row("Brine Water Clarity"), "Brine"));
    press("v");
    expect(useSearch().text.value).toBe("v");
    expect(document.activeElement?.id).toBe("rift-search");
  });

  it("should send l and i to the well like any letter: 'lantern' searches, never opens the lookbook, and 'iron' plans no install", async () => {
    // R16.6 and Bold Choice 10: every word typed in the tree becomes a search
    await wakeOnTree();
    const planned = vi.spyOn(spine(), "planInstall");
    await focusRow(must(row("Lantern Glow Recolor"), "Lantern"));
    const l = press("l");
    expect(l.defaultPrevented).toBe(true);
    expect(useSearch().text.value).toBe("l");
    expect(document.activeElement?.id).toBe("rift-search");
    expect(useNavigation().riftMode.value).toBe("results");
    useSearch().clearText();
    await until(() => tree() !== null && useSearch().settledText.value === "", "the tree again");

    // a leaf: "i" (either case) starts "iron", and nothing is planned for the row under the keyboard
    lineOf(must(row("Lantern Glow Recolor"), "Lantern")).click();
    await until(() => row("Lantern Glow Recolor", 2) !== undefined, "Lantern open");
    await focusRow(must(row("Lantern Glow Recolor", 2), "Lantern's option"));
    press("I");
    expect(useSearch().text.value).toBe("I");
    expect(document.activeElement?.id).toBe("rift-search");
    await new Promise((resolve) => setTimeout(resolve, 80));
    expect(planned).not.toHaveBeenCalled();
    expect(useWorld().selectedId.value).toBeNull();
    expect(useNavigation().riftMode.value).toBe("results");
  });

  it("should still pass the room keys and / through the tree", async () => {
    await wakeOnTree();
    await focusRow(must(row("Brine Water Clarity"), "Brine"));
    press("3");
    await until(() => useNavigation().page.value === "nexus", "THE FAR WORLD on 3");
    expect(useSearch().text.value).toBe("");
  });

  it("should never open an option nobody sees on Enter in an empty well", async () => {
    await wakeOnTree();
    await until(() => useSearch().results.value.length > 0, "the idle answers behind the tree");
    must(document.getElementById("rift-search"), "well").focus();
    press("Enter");
    await new Promise((resolve) => setTimeout(resolve, 80));
    expect(useWorld().selectedId.value).toBeNull();
    expect(tree()).not.toBeNull();
  });

  it("should keep the seam at rest while the tree opens and closes: browsing is not an answer", async () => {
    await wakeOnTree();
    const answered = useSearch().answerSeq.value;
    button("Expand all").click();
    await until(() => row("Pilgrim Hood - Variation 4", 5) !== undefined, "every branch open");
    button("Collapse all").click();
    await until(() => rows().length === SHELF.length, "every branch closed");
    expect(useSearch().answerSeq.value).toBe(answered);
    expect(useSearch().seam.value).toBe("rest");
    expect(document.querySelector(".pillar")?.getAttribute("data-seam")).toBe("rest");
  });
});

describe("the tree beside the trouble and the crossing", () => {
  it("should carve the trouble fold above the tree, outside its role, and open it from a troubled package", async () => {
    await wakeOnTree();
    const fold = must(document.querySelector<HTMLElement>(".trouble-fold"), "trouble fold");
    expect(fold.closest('[role="tree"]')).toBeNull();
    expect(Boolean(fold.compareDocumentPosition(must(tree(), "tree")) & Node.DOCUMENT_POSITION_FOLLOWING)).toBe(true);
    // the troubled packages are still rows of the tree, their lead problem in place of their counts
    const broken = must(row("Corrupted Download"), "Corrupted Download");
    expect(textOf(must(lineOf(broken).querySelector(".ltr__count"), "count")).trim()).toBe("won't open");
    expect(textOf(must(lineOf(must(row("Readme Only"), "Readme")).querySelector(".ltr__count"), "count")).trim()).toBe(
      "opens with a warning",
    );

    await focusRow(broken);
    press("Enter");
    await until(() => (document.activeElement?.closest("#rift-trouble") ?? null) !== null, "the focus in the fold");
    expect(useStone().troubleOpen.value).toBe(true);
    const card = must(document.activeElement?.closest<HTMLElement>("[data-trouble-index]"), "card");
    expect(textOf(must(card.querySelector(".trouble__name"), "card name")).trim()).toBe("Corrupted Download");
  });

  it("should give a package row the trouble fold's verdict only for a trouble the fold lists, and its counts otherwise", async () => {
    // R16.2: the same verdict as the fold, never a second one. A problem the fold never lists (a missing
    // menu parent, a missing requirement, a garbled modinfo.ini) is no verdict of the row's.
    await wakeOnTree();
    const real = await spine().getLibrary();
    const sunId = must(
      real.packages.find((p) => p.package.displayName === "Battahli Sun Bleach"),
      "Battahli in the library",
    ).package.id;
    const parentless = {
      ...real,
      packages: real.packages.map((p) =>
        p.package.id === sunId
          ? {
              ...p,
              package: {
                ...p.package,
                problems: [
                  {
                    kind: "missingParent" as const,
                    message:
                      'Its menu sits under "Sun Bleach Presets", which is not in the mods folder. Add the mod that provides "Sun Bleach Presets" to the mods folder to restore the menu.',
                  },
                ],
              },
            }
          : p,
      ),
    };
    vi.spyOn(spine(), "getLibrary").mockResolvedValue(parentless);
    await useLibrary().load();
    const count = (): string =>
      textOf(must(lineOf(must(row("Battahli Sun Bleach"), "Battahli")).querySelector(".ltr__count"), "count"))
        .replace(/\s+/g, " ")
        .trim();
    await until(() => count() === "1 option", "the counts, not a verdict nothing explains");
    expect(descriptionOf(must(row("Battahli Sun Bleach"), "Battahli"))).not.toContain("warning");
    // the fold's own troubles keep their verdicts
    expect(textOf(must(lineOf(must(row("Readme Only"), "Readme")).querySelector(".ltr__count"), "count")).trim()).toBe(
      "opens with a warning",
    );
  });

  it("should turn an option row's marker gold when it is installed", async () => {
    await wakeOnTree();
    const pkg = must(row("Lantern Glow Recolor"), "Lantern");
    lineOf(pkg).click();
    await until(() => row("Lantern Glow Recolor", 2) !== undefined, "Lantern open");
    const option = (): HTMLElement => must(row("Lantern Glow Recolor", 2), "Lantern's option");
    expect(chipOf(option())).toBe("NOT INSTALLED");
    expect(lineOf(option()).classList.contains("rail--rift")).toBe(true);
    expect(textOf(must(lineOf(pkg).querySelector(".ltr__count"), "count")).trim()).toBe("1 option");

    lineOf(option()).click();
    await until(() => hasButton("Install…"), "the option in YOUR WORLD");
    button("Install…").click();
    await until(() => hasButton("Install ▶") && idle(button("Install ▶")), "the install plan");
    button("Install ▶").click();
    await until(() => chipOf(option()) === "INSTALLED", "the row's marker flipped", 5000);
    expect(lineOf(option()).classList.contains("rail--world")).toBe(true);
    expect(lineOf(option()).classList.contains("rail--rift")).toBe(false);
    expect(
      textOf(must(lineOf(pkg).querySelector(".ltr__count"), "count"))
        .replace(/\s+/g, " ")
        .trim(),
    ).toBe("1 option · 1 in your world");
    expect(lineOf(pkg).classList.contains("rail--world")).toBe(true);
    expect(text()).toContain("Lantern Glow Recolor is in your world.");
  });
});

describe("the library tree's rows as R16.2–R16.4 and §16.7 draw them", () => {
  it("should write a package's format as a word, 7Z for a 7z, never the wire's SEVENZIP", async () => {
    await wakeOnTree();
    const formats = topRows().map((r) => textOf(must(lineOf(r).querySelector(".ltr__fmt"), "format")).trim());
    expect(formats).toContain("7Z");
    expect(formats.every((f) => ["ZIP", "7Z", "RAR", "FOLDER"].includes(f))).toBe(true);
  });

  it("should give menu and bundle nodes no kind badges, since they install nothing (R16.3)", async () => {
    await wakeOnTree();
    await openHoods();
    await step("ArrowDown", "4:Pilgrim Hood - Variation 1");
    await step("ArrowDown", "4:Pilgrim Hood - Variation 2");
    await step("ArrowDown", "4:Pilgrim Hood - Southern Dyes");
    press("ArrowRight");
    await until(() => row("Pilgrim Hood - Variation 4", 5) !== undefined, "the bundle open");
    expect(badges("Pilgrim Hood - Southern Dyes", 4)).toBe(0);
    expect(badges("Hoods", 2)).toBe(0);
    expect(badges("Pilgrim Hood - Variation 3", 5)).toBeGreaterThan(0);
  });
});
