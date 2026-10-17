// v0.2 — the veil (WINDOW-CONTRACT § v0.2 "Veil", ruled 2026-10-16). "off"
// veils nothing and "all" veils everything; "adult" veils what Nexus flags
// adult AND what Nexus has not labelled yet: a veil exists to protect, so the
// unknown is veiled, and it clears as packages are linked and checked. One
// helper decides (ledger veils + adultnessOf), so the card, the detail and
// the lookbook can never disagree about a package.

import { flushPromises } from "@vue/test-utils";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { NexusLabels, OptionDetail, SearchResult, VeilMode } from "@/types/rift";
import { adultnessOf, veils, type Adultness } from "@/services/ledger";
import { MOCK_PACKAGES } from "@/services/mock/library";
import { useNavigation } from "@/composables/useNavigation";
import { usePackage } from "@/composables/usePackage";
import { useSearch } from "@/composables/useSearch";
import { useSettings } from "@/composables/useSettings";
import { useWorld } from "@/composables/useWorld";
import { sleep, spine, textOf, until, wakeOn } from "./harness";

afterEach(() => {
  vi.restoreAllMocks();
  sleep();
});

const labels = (category: string | null, tags: string[], adult: boolean): NexusLabels => ({ category, tags, adult });
const hit = (
  nexusCategory: string | null,
  nexusTags: string[],
  adult: boolean,
): Pick<SearchResult, "nexusCategory" | "nexusTags" | "adult"> => ({ nexusCategory, nexusTags, adult });

describe("veils() — the one rule", () => {
  it("should answer the full matrix: off veils nothing, all veils everything, adult veils the flagged and the unknown", () => {
    const matrix: [VeilMode, Adultness, boolean][] = [
      ["off", "adult", false],
      ["off", "notAdult", false],
      ["off", "unknown", false],
      ["adult", "adult", true],
      ["adult", "notAdult", false],
      ["adult", "unknown", true],
      ["all", "adult", true],
      ["all", "notAdult", true],
      ["all", "unknown", true],
    ];
    expect(matrix.map(([mode, adultness]) => [mode, adultness, veils(mode, adultness)])).toStrictEqual(matrix);
  });

  it("should read a detail's labels: null and the blank answer are unknown, any label without the flag is not adult", () => {
    expect(adultnessOf(null)).toBe("unknown");
    expect(adultnessOf(labels(null, [], false))).toBe("unknown");
    expect(adultnessOf(labels("  ", [], false))).toBe("unknown");
    expect(adultnessOf(labels("Visuals", [], false))).toBe("notAdult");
    expect(adultnessOf(labels(null, ["Fashion"], false))).toBe("notAdult");
    expect(adultnessOf(labels(null, [], true))).toBe("adult");
    expect(adultnessOf(labels("Visuals", ["Retexture", "Sexualised"], true))).toBe("adult");
  });

  it("should read a card's SearchResult the same way as the detail reads its labels", () => {
    const cases: [Pick<SearchResult, "nexusCategory" | "nexusTags" | "adult">, NexusLabels | null][] = [
      [hit(null, [], false), null],
      [hit(null, [], false), labels(null, [], false)],
      [hit("Visuals", ["Lighting"], false), labels("Visuals", ["Lighting"], false)],
      [hit(null, ["Fashion"], false), labels(null, ["Fashion"], false)],
      [hit(null, [], true), labels(null, [], true)],
      [hit("Visuals", ["Sexualised"], true), labels("Visuals", ["Sexualised"], true)],
    ];
    for (const [card, detail] of cases) expect(adultnessOf(card)).toBe(adultnessOf(detail));
    expect(cases.map(([card]) => adultnessOf(card))).toStrictEqual([
      "unknown",
      "unknown",
      "notAdult",
      "notAdult",
      "adult",
      "adult",
    ]);
  });
});

// --- the three surfaces, on the mock library --------------------------------

/** Three packages, one per answer Nexus can give (./mock/library.ts). */
const SUBJECTS = ["Battahli Sun Bleach", "Lantern Glow Recolor", "Brine Water Clarity"] as const;
type Subject = (typeof SUBJECTS)[number];

/** What each mode must do to each subject, written out (not computed with veils()). */
const EXPECTED: Record<VeilMode, Record<Subject, boolean>> = {
  off: { "Battahli Sun Bleach": false, "Lantern Glow Recolor": false, "Brine Water Clarity": false },
  // Battahli: Nexus flags it adult. Lantern: labelled, not flagged. Brine: unlinked, so unknown.
  adult: { "Battahli Sun Bleach": true, "Lantern Glow Recolor": false, "Brine Water Clarity": true },
  all: { "Battahli Sun Bleach": true, "Lantern Glow Recolor": true, "Brine Water Clarity": true },
};

function packageOf(name: Subject): { packageId: string; optionId: string } {
  const pkg = MOCK_PACKAGES.find((p) => p.summary.displayName === name);
  const option = pkg?.options.at(0);
  if (!pkg || !option) throw new Error(`The mock library has no ${name}`);
  return { packageId: pkg.summary.id, optionId: option.id };
}

/** Every get_option answer in flight, so a check reads the settled veil, not a guess. */
function trackOptionReads(stall: (optionId: string) => boolean = () => false): () => Promise<void> {
  const backend = spine();
  const read = backend.getOption.bind(backend);
  const inFlight: Promise<OptionDetail>[] = [];
  vi.spyOn(backend, "getOption").mockImplementation((optionId: string) => {
    // a stalled answer never lands: the window is left waiting on its labels
    const answer = stall(optionId) ? new Promise<OptionDetail>(() => undefined) : read(optionId);
    if (!stall(optionId)) inFlight.push(answer);
    return answer;
  });
  return async () => {
    let seen = -1;
    while (seen !== inFlight.length) {
      seen = inFlight.length;
      await Promise.allSettled(inFlight);
      await flushPromises();
    }
  };
}

function cardVeiled(optionId: string): boolean {
  const card = [...document.querySelectorAll<HTMLElement>("#rift-answers .card")].find(
    (c) => c.dataset.option === optionId,
  );
  if (!card) throw new Error(`No card for ${optionId} in the Rift`);
  return card.classList.contains("is-veiled");
}

const lookbookTiles = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>(".lookbook .tile")];

function lookbookVeiled(): boolean {
  const grid = document.querySelector(".lookbook");
  const tiles = lookbookTiles();
  if (!grid || !tiles.length) throw new Error("The lookbook shows no tiles");
  const tileVeils = new Set(tiles.map((t) => t.classList.contains("is-veiled")));
  // the grid and every tile carry one verdict: the package's
  expect([...tileVeils]).toStrictEqual([grid.classList.contains("is-veiled-all")]);
  return grid.classList.contains("is-veiled-all");
}

async function surfaces(name: Subject, settle: () => Promise<void>): Promise<Record<string, boolean>> {
  const { packageId, optionId } = packageOf(name);
  const nav = useNavigation();
  const world = useWorld();

  world.close();
  nav.backToResults();
  // v0.3: the idle Rift is the tree; a facet brings the flat answers (every option)
  useSearch().setMenuOnly(true);
  await until(() => document.querySelector(`#rift-answers .card`) !== null, "the Rift's answers");
  const card = cardVeiled(optionId);

  void world.open(optionId);
  await until(() => world.detail.value?.option.id === optionId && document.querySelector(".odetail") !== null, name);
  await settle();
  const detail = document.querySelector(".odetail")?.classList.contains("is-veiled") ?? false;

  world.close();
  nav.openLookbook(packageId);
  await until(
    () => usePackage().detail.value?.package.id === packageId && lookbookTiles().length > 0,
    `${name}'s lookbook`,
  );
  await settle();
  const lookbook = lookbookVeiled();
  nav.backToResults();
  return { card, detail, lookbook };
}

describe("the veil on the card, the detail and the lookbook", () => {
  for (const mode of ["off", "adult", "all"] as const) {
    it(`should veil the same packages on every surface in "${mode}" mode`, async () => {
      await wakeOn();
      const settle = trackOptionReads();
      await useSettings().setVeil(mode);
      for (const name of SUBJECTS) {
        const want = EXPECTED[mode][name];
        expect({ name, ...(await surfaces(name, settle)) }).toStrictEqual({
          name,
          card: want,
          detail: want,
          lookbook: want,
        });
      }
    }, 15_000);
  }

  it("should veil the answers grouped by package by the same rule", async () => {
    await wakeOn();
    await useSettings().setVeil("adult");
    useSearch().grouped.value = true;
    useSearch().setMenuOnly(true);
    await until(() => document.querySelector("#rift-answers .rift__groupbox .card") !== null, "the grouped answers");
    expect(SUBJECTS.map((name) => [name, cardVeiled(packageOf(name).optionId)])).toStrictEqual(
      SUBJECTS.map((name) => [name, EXPECTED.adult[name]]),
    );
  });

  it("should keep a lookbook veiled in adult mode while its labels have not arrived, never borrowing the last package's verdict", async () => {
    await wakeOn();
    const nav = useNavigation();
    const lantern = packageOf("Lantern Glow Recolor");
    const battahli = packageOf("Battahli Sun Bleach");
    // Battahli's labels never land in this test: its lookbook only ever knows "not yet"
    const settle = trackOptionReads((optionId) => optionId === battahli.optionId);
    await useSettings().setVeil("adult");

    nav.openLookbook(lantern.packageId);
    await until(
      () => usePackage().detail.value?.package.id === lantern.packageId && lookbookTiles().length > 0,
      "Lantern's lookbook",
    );
    await settle();
    expect(lookbookVeiled()).toBe(false);

    nav.openLookbook(battahli.packageId);
    await until(
      () => usePackage().detail.value?.package.id === battahli.packageId && lookbookTiles().length > 0,
      "Battahli's lookbook",
    );
    expect(lookbookVeiled()).toBe(true);
    await settle();
    expect(lookbookVeiled()).toBe(true);
  }, 15_000);
});

describe("the veil control on Preflight", () => {
  it("should say that adult mode blurs what Nexus flags adult and what it has not labelled yet, and set that mode", async () => {
    await wakeOn();
    const saved = vi.spyOn(spine(), "setSettings");
    useNavigation().go("preflight");
    await until(() => document.querySelector(".veil") !== null, "the veil control");
    expect([...document.querySelectorAll(".veil__word")].map((w) => textOf(w).trim())).toStrictEqual([
      "Off",
      "Adult & unlabeled",
      "All",
    ]);

    const adult = document.querySelector<HTMLInputElement>('input[name="rift-veil"][value="adult"]');
    if (!adult) throw new Error("The veil control has no adult mode");
    adult.checked = true;
    adult.dispatchEvent(new Event("change"));
    await until(() => useSettings().settings.value.veil === "adult", "the adult mode chosen");
    expect(saved).toHaveBeenCalledWith({ veil: "adult", autoCheckUpdates: true });
    expect(textOf(document.querySelector(".pre__val--veil") ?? document.body).trim()).toBe(
      "Blurs the screenshots of mods Nexus flags as adult, and of packages Nexus has not labelled yet, until you hover or focus their card. A veil protects, so the unknown stays veiled: link a package and check for updates, and its veil lifts once Nexus labels it without the adult flag. The lightbox always opens unveiled.",
    );

    // and the Rift obeys it: the unlinked water mod is blurred as unknown
    useNavigation().go("rift");
    useSearch().setMenuOnly(true);
    const brine = packageOf("Brine Water Clarity");
    await until(() => document.querySelector("#rift-answers .card") !== null, "the Rift's answers");
    expect(cardVeiled(brine.optionId)).toBe(true);
    expect(cardVeiled(packageOf("Lantern Glow Recolor").optionId)).toBe(false);
  });
});
