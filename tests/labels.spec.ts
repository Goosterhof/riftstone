// v0.2 — the NEXUS LABELS chips (WINDOW-CONTRACT § v0.2): category chips are
// ANY-of (a mod has one category, so they widen), tag chips are ALL-of (each
// tag narrows). The window sends both to the spine, reads the filter aloud,
// prunes a chosen label the library stopped carrying, and makes every chip's
// count the number of answers pressing it would show right now. When a label
// filter hides every answer, the Rift says which filter, and how to undo it.

import { afterEach, describe, expect, it, vi } from "vitest";
import type { SearchQuery } from "@/types/rift";
import { MOCK_PACKAGES } from "@/services/mock/library";
import { useFarWorld } from "@/composables/useFarWorld";
import { useSearch } from "@/composables/useSearch";
import { button, sleep, spine, text, textAt, textOf, typeQuery, until, wakeOn } from "./harness";

afterEach(() => {
  vi.restoreAllMocks();
  sleep();
});

const chipButtons = (): HTMLButtonElement[] => [
  ...document.querySelectorAll<HTMLButtonElement>("#nexus-labels .lchip"),
];
const nameOf = (b: Element): string => textOf(b.querySelector(".lchip__name") ?? b).trim();

function chip(name: string): HTMLButtonElement {
  const found = chipButtons().find((b) => nameOf(b) === name);
  if (!found) throw new Error(`No NEXUS chip "${name}": the stone carves ${chipButtons().map(nameOf).join(" | ")}`);
  return found;
}

const counts = (): Record<string, number> =>
  Object.fromEntries(chipButtons().map((b) => [nameOf(b), Number(textOf(b.querySelector(".lchip__n") ?? b))]));
const pressed = (): string[] =>
  chipButtons()
    .filter((b) => b.getAttribute("aria-pressed") === "true")
    .map(nameOf);
const dead = (): string[] =>
  chipButtons()
    .filter((b) => b.classList.contains("is-dead"))
    .map(nameOf);
const answers = (): string[] => useSearch().results.value.map((h) => h.name);
const sortedAnswers = (): string[] => [...answers()].sort();
const reading = (): string | null => textAt(".flab__readtext");
const clearChip = (): HTMLButtonElement => {
  const found = [...document.querySelectorAll<HTMLButtonElement>(".flab__reading .flab__clear")].at(0);
  if (!found) throw new Error("The reading line has no Clear");
  return found;
};
const troubleNames = (): string[] => [...document.querySelectorAll(".trouble__name")].map((n) => textOf(n).trim());
const inscriptionBody = (): string => textOf(document.querySelector(".rs-ins__body") ?? document.body).trim();
const inscriptionButtons = (): string[] =>
  [...document.querySelectorAll(".rs-ins__actions .rs-btn__label")].map((l) => textOf(l).trim());

/** The answer query the window sent last. v0.3: both queries ask uncapped
 *  (`limit: 0`), so the answer query is told apart by what the chips' count
 *  query leaves out: it is the one that carries the labels. */
function lastAnswerQuery(calls: [SearchQuery][]): SearchQuery {
  const query = calls
    .map(([q]) => q)
    .filter((q) => q.limit === 0 && (q.filters.categories.length > 0 || q.filters.tags.length > 0))
    .at(-1);
  if (!query) throw new Error("The window sent no labelled answer query");
  return query;
}

const idOf = (name: string): string => {
  const pkg = MOCK_PACKAGES.find((p) => p.summary.displayName === name);
  if (!pkg) throw new Error(`The mock library has no ${name}`);
  return pkg.summary.id;
};

/** The library with nothing chosen: installable options per label (./mock/library.ts). */
const LIBRARY_TOTALS = {
  Armour: 18,
  Visuals: 2,
  Utilities: 1,
  Retexture: 20,
  Fashion: 18,
  Audio: 1,
  Lighting: 1,
  Sexualised: 1,
  "Utilities for Players": 1,
};
/** Every installable option in the library, menu entries hidden. */
const LIBRARY_OPTIONS = 26;

async function wakeWithChips(): Promise<void> {
  await wakeOn();
  await until(() => chipButtons().length === 9 && answers().length === LIBRARY_OPTIONS, "the NEXUS LABELS chips");
}

describe("the NEXUS LABELS chips", () => {
  it("should send category chips ANY-of: two categories answer with the union of their mods", async () => {
    await wakeWithChips();
    const sent = vi.spyOn(spine(), "search");
    chip("Visuals").click();
    await until(() => answers().length === 2, "the Visuals answers");
    expect(sortedAnswers()).toStrictEqual(["Battahli Sun Bleach", "Lantern Glow Recolor"]);

    chip("Utilities").click();
    await until(() => answers().length === 3, "Visuals or Utilities");
    expect(sortedAnswers()).toStrictEqual(["Battahli Sun Bleach", "Lantern Glow Recolor", "Pawn Chatter Filter"]);
    const { categories, tags } = lastAnswerQuery(sent.mock.calls).filters;
    expect({ categories, tags }).toStrictEqual({ categories: ["Visuals", "Utilities"], tags: [] });
    expect(pressed()).toStrictEqual(["Visuals", "Utilities"]);
    expect(reading()).toBe("Visuals or Utilities");
  });

  it("should send tag chips ALL-of: each tag narrows, so two tags answer only with the mods carrying both", async () => {
    await wakeWithChips();
    const sent = vi.spyOn(spine(), "search");
    chip("Retexture").click();
    await until(() => answers().length === 20, "the Retexture answers");

    // Fashion alone finds 18 and Retexture alone 20; ALL-of keeps the 18 that carry both
    chip("Fashion").click();
    await until(() => answers().length === 18, "Retexture and Fashion");
    expect(new Set(useSearch().results.value.map((h) => h.packageName))).toStrictEqual(
      new Set(["Vernworth Wardrobe Collection"]),
    );
    const { categories, tags } = lastAnswerQuery(sent.mock.calls).filters;
    expect({ categories, tags }).toStrictEqual({ categories: [], tags: ["Retexture", "Fashion"] });
    expect(reading()).toBe("Any mod, carrying Retexture and Fashion");

    // no wardrobe mod carries Lighting: a third tag narrows to nothing
    chip("Lighting").click();
    await until(() => answers().length === 0, "a tag nothing else carries");
    expect(lastAnswerQuery(sent.mock.calls).filters.tags).toStrictEqual(["Retexture", "Fashion", "Lighting"]);
  });

  it("should read the label filter aloud as it changes, on the stone and in the Rift, and Clear should empty both lists", async () => {
    await wakeWithChips();
    chip("Visuals").click();
    await until(() => reading() === "Visuals", "one category");
    chip("Lighting").click();
    await until(() => reading() === "Visuals, carrying Lighting", "a category and a tag");
    chip("Retexture").click();
    await until(() => reading() === "Visuals, carrying Lighting and Retexture", "two tags");
    chip("Armour").click();
    await until(() => reading() === "Visuals or Armour, carrying Lighting and Retexture", "two categories");
    await until(() => answers().length === 1, "the one mod in Visuals or Armour carrying both tags");
    expect(answers()).toStrictEqual(["Lantern Glow Recolor"]);
    expect(textOf(document.querySelector(".pillar__labeltext") ?? document.body).trim()).toBe(
      "Visuals or Armour, carrying Lighting and Retexture",
    );
    expect(
      textOf(document.querySelector(".rift__echo") ?? document.body)
        .replace(/\s+/g, " ")
        .trim(),
    ).toBe("1 option · Visuals or Armour, carrying Lighting and Retexture");

    clearChip().click();
    await until(() => answers().length === LIBRARY_OPTIONS, "the whole library");
    expect({ categories: useSearch().categories.value, tags: useSearch().tags.value }).toStrictEqual({
      categories: [],
      tags: [],
    });
    expect(reading()).toBeNull();
    expect(pressed()).toStrictEqual([]);
    expect(
      textOf(document.querySelector(".rift__echo") ?? document.body)
        .replace(/\s+/g, " ")
        .trim(),
    ).toBe("Library · 11 packages · 32 options");
  });

  it("should prune a chosen label the library stopped carrying after an unlink, and keep the ones it still carries", async () => {
    await wakeWithChips();
    chip("Visuals").click();
    chip("Utilities").click();
    await until(() => answers().length === 3, "Visuals or Utilities");
    chip("Audio").click();
    await until(() => answers().length === 1, "… carrying Audio");
    expect(answers()).toStrictEqual(["Pawn Chatter Filter"]);

    // the chatter filter was the only mod in Utilities and the only one carrying Audio
    await useFarWorld().unlink(idOf("Pawn Chatter Filter"));
    await until(() => !chipButtons().some((b) => nameOf(b) === "Utilities"), "the Utilities chip gone");
    await until(() => answers().length === 2, "Visuals alone");
    expect({ categories: useSearch().categories.value, tags: useSearch().tags.value }).toStrictEqual({
      categories: ["Visuals"],
      tags: [],
    });
    expect(sortedAnswers()).toStrictEqual(["Battahli Sun Bleach", "Lantern Glow Recolor"]);
    expect(reading()).toBe("Visuals");
    expect(text()).not.toContain("THE LABELS HIDE EVERY ANSWER");
  });

  it("should make every chip's count the answers its press would show, when the words or another chip narrow", async () => {
    await wakeWithChips();
    expect(counts()).toStrictEqual(LIBRARY_TOTALS);

    await typeQuery("cape");
    await until(() => counts().Armour === 6, "the words' predictions");
    expect(counts()).toStrictEqual({
      Armour: 6,
      Visuals: 0,
      Utilities: 0,
      Retexture: 6,
      Fashion: 6,
      Audio: 0,
      Lighting: 0,
      Sexualised: 0,
      "Utilities for Players": 0,
    });
    expect(dead()).toStrictEqual(["Visuals", "Utilities", "Audio", "Lighting", "Sexualised", "Utilities for Players"]);
    expect(chip("Armour").getAttribute("aria-label")).toBe("Armour, 6 answers now, 18 options in the library");
    chip("Armour").click();
    await until(() => useSearch().labelFiltered.value && answers().length === 6, "the six capes, as predicted");

    useSearch().clearLabels();
    useSearch().clearText();
    await until(() => answers().length === LIBRARY_OPTIONS, "the whole library");
    chip("Retexture").click();
    await until(() => counts().Utilities === 0, "another chip's predictions");
    expect(counts()).toStrictEqual({
      Armour: 18,
      Visuals: 2,
      Utilities: 0,
      Retexture: 20,
      Fashion: 18,
      Audio: 0,
      Lighting: 1,
      Sexualised: 1,
      "Utilities for Players": 0,
    });
    chip("Sexualised").click();
    await until(() => answers().length === 1, "the one mod carrying both, as predicted");
    expect(answers()).toStrictEqual(["Battahli Sun Bleach"]);
  });
});

describe("when the labels hide every answer", () => {
  it("should say THE LABELS HIDE EVERY ANSWER, name the filter, and offer Clear labels first", async () => {
    await wakeWithChips();
    chip("Fashion").click();
    chip("Lighting").click();
    await until(() => text().includes("THE LABELS HIDE EVERY ANSWER"), "the labels' empty state");
    expect(inscriptionBody()).toBe(
      "Any mod, carrying Fashion and Lighting: none of the 26 options in view carry that. Labels come from Nexus: an unlinked package carries none, so a label filter hides it too.",
    );
    expect(inscriptionButtons()).toStrictEqual(["Clear labels"]);

    button("Clear labels").click();
    await until(() => answers().length === LIBRARY_OPTIONS, "the library back");
    expect(text()).not.toContain("THE LABELS HIDE EVERY ANSWER");
  });

  it("should tell the words' own count when the labels hid what the words found, never that nothing answers", async () => {
    await wakeWithChips();
    await typeQuery("cape");
    await until(() => answers().length === 6, "the six capes");
    chip("Lighting").click();
    await until(() => text().includes("THE LABELS HIDE EVERY ANSWER"), "the labels' empty state");
    expect(inscriptionBody()).toBe(
      "Any mod, carrying Lighting: the words found 6 answers; the labels hid every one. Labels come from Nexus: an unlinked package carries none, so a label filter hides it too.",
    );
    expect(inscriptionButtons()).toStrictEqual(["Clear labels", "Clear search"]);
    expect(text()).not.toContain("NOTHING ANSWERS");
    expect(
      textOf(document.querySelector(".rift__echo") ?? document.body)
        .replace(/\s+/g, " ")
        .trim(),
    ).toBe("0 answered “cape” · Any mod, carrying Lighting");

    button("Clear labels").click();
    await until(() => answers().length === 6, "the capes back");
  });

  it("should hide the trouble cards while a label filter is on, and bring them back when it clears", async () => {
    await wakeWithChips();
    // §8.1.1: the trouble is folded at launch; open it so its cards are in view
    await until(() => document.querySelector(".trouble-fold") !== null, "the trouble fold");
    button("Show ▸").click();
    await until(() => troubleNames().length === 3, "the trouble cards");
    expect(troubleNames()).toStrictEqual(["Corrupted Download", "Readme Only", "Old Road Armor Pack"]);

    chip("Visuals").click();
    await until(() => answers().length === 2, "the Visuals answers");
    expect(troubleNames()).toStrictEqual([]);
    expect(document.querySelector(".trouble-fold")).toBeNull();

    clearChip().click();
    await until(() => answers().length === LIBRARY_OPTIONS && troubleNames().length === 3, "the cards back");
    expect(button("Close ▾").getAttribute("aria-expanded")).toBe("true");
  });
});
