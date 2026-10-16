// v0.2 — the NEXUS line in the option detail: the mod's category and tags as
// the pillar's quiet chips (pressing one toggles that facet), "Nexus #id" as
// the door to the mod's page (opened through the opener, never the webview),
// an honest line when the stone has no labels for it, and a detail that stays
// true while it is open: a link or a check re-reads it, so its labels (and
// the veil they decide) are never stale.

import { afterEach, describe, expect, it, vi } from "vitest";
import type { OptionDetail } from "@/types/rift";
import { MOCK_PACKAGES } from "@/services/mock/library";
import { useFarWorld } from "@/composables/useFarWorld";
import { useSearch } from "@/composables/useSearch";
import { useSettings } from "@/composables/useSettings";
import { useWorld } from "@/composables/useWorld";
import { sleep, spine, textAt, textOf, until, wakeOn } from "./harness";

afterEach(() => {
  vi.restoreAllMocks();
  sleep();
});

function packageOf(name: string): { packageId: string; optionId: string } {
  const pkg = MOCK_PACKAGES.find((p) => p.summary.displayName === name);
  const option = pkg?.options.at(0);
  if (!pkg || !option) throw new Error(`The mock library has no ${name}`);
  return { packageId: pkg.summary.id, optionId: option.id };
}

const detailChips = (): HTMLButtonElement[] => [
  ...document.querySelectorAll<HTMLButtonElement>(".odetail__labels .odetail__lab"),
];
const detailChipNames = (): string[] => detailChips().map((b) => textOf(b).trim());
function detailChip(name: string): HTMLButtonElement {
  const found = detailChips().find((b) => textOf(b).trim() === name);
  if (!found) throw new Error(`The detail carries no "${name}" chip: ${detailChipNames().join(" | ")}`);
  return found;
}
function pillarChip(name: string): HTMLButtonElement {
  const found = [...document.querySelectorAll<HTMLButtonElement>("#nexus-labels .lchip")].find(
    (b) => textOf(b.querySelector(".lchip__name") ?? b).trim() === name,
  );
  if (!found) throw new Error(`The stone carves no "${name}" chip`);
  return found;
}
const unlabeledLine = (): string | null => textAt(".odetail__nolab");
const detailVeiled = (): boolean => document.querySelector(".odetail")?.classList.contains("is-veiled") ?? false;
const answers = (): string[] => useSearch().results.value.map((h) => h.name);

async function openDetail(name: string): Promise<void> {
  const { optionId } = packageOf(name);
  void useWorld().open(optionId);
  await until(
    () => useWorld().detail.value?.option.id === optionId && document.querySelector(".odetail__labels") !== null,
    `${name}'s detail`,
  );
}

describe("the detail's NEXUS line", () => {
  it("should show the mod's category and tags as chips that toggle the Rift's filters", async () => {
    await wakeOn();
    await openDetail("Lantern Glow Recolor");
    expect(detailChipNames()).toStrictEqual(["Visuals", "Lighting", "Retexture"]);
    expect(detailChip("Visuals").title).toBe("Nexus category: Visuals. Press to filter the Rift by it.");
    expect(detailChip("Lighting").title).toBe("Nexus tag: Lighting. Press to filter the Rift by it.");
    expect(unlabeledLine()).toBeNull();

    detailChip("Visuals").click();
    await until(() => answers().length === 2, "the Rift filtered by Visuals");
    expect(useSearch().categories.value).toStrictEqual(["Visuals"]);
    expect(detailChip("Visuals").getAttribute("aria-pressed")).toBe("true");
    expect(pillarChip("Visuals").getAttribute("aria-pressed")).toBe("true");

    detailChip("Lighting").click();
    await until(() => answers().length === 1, "… carrying Lighting");
    expect(useSearch().tags.value).toStrictEqual(["Lighting"]);
    expect(answers()).toStrictEqual(["Lantern Glow Recolor"]);

    detailChip("Visuals").click();
    await until(() => useSearch().categories.value.length === 0, "Visuals pressed off");
    expect(detailChip("Visuals").getAttribute("aria-pressed")).toBe("false");
    expect(pillarChip("Visuals").getAttribute("aria-pressed")).toBe("false");
    expect(useSearch().tags.value).toStrictEqual(["Lighting"]);
  });

  it("should open the mod's Nexus page through the opener, never inside the window", async () => {
    await wakeOn();
    const opener = vi.spyOn(spine(), "openUrl").mockResolvedValue(undefined);
    const popup = vi.spyOn(window, "open").mockReturnValue(null);
    await openDetail("Lantern Glow Recolor");
    const door = document.querySelector<HTMLButtonElement>(".odetail__nexus");
    if (!door) throw new Error("The by-line has no Nexus door");
    expect(textOf(door).trim()).toBe("Nexus #412 ↗");
    expect(door.title).toBe("Open Nexus mod 412 in your browser");

    door.click();
    await until(() => opener.mock.calls.length === 1, "the opener");
    expect(opener).toHaveBeenCalledWith("https://www.nexusmods.com/dragonsdogma2/mods/412");
    expect(popup).not.toHaveBeenCalled();
  });

  it("should say honestly why a package carries no labels: unlinked, or linked and not checked yet", async () => {
    await wakeOn();
    await openDetail("Brine Water Clarity");
    expect(detailChipNames()).toStrictEqual([]);
    expect(unlabeledLine()).toBe(
      "No Nexus labels: link this package on the NEXUS page and its category and tags follow.",
    );
    sleep();

    await wakeOn("fresh");
    await openDetail("Lantern Glow Recolor");
    expect(detailChipNames()).toStrictEqual([]);
    expect(unlabeledLine()).toBe("No Nexus labels yet: the stone learns them at the next update check.");
  });

  it("should say so when Nexus flags a mod adult and gives it no category or tags, and veil it as adult", async () => {
    await wakeOn();
    const { optionId } = packageOf("Battahli Sun Bleach");
    const backend = spine();
    const read = backend.getOption.bind(backend);
    vi.spyOn(backend, "getOption").mockImplementation(async (id: string): Promise<OptionDetail> => {
      const detail = await read(id);
      return id === optionId ? { ...detail, labels: { category: null, tags: [], adult: true } } : detail;
    });
    await useSettings().setVeil("adult");
    await openDetail("Battahli Sun Bleach");
    expect(detailChipNames()).toStrictEqual([]);
    expect(unlabeledLine()).toBe("Nexus flags this mod as adult content, and gives it no category or tags.");
    expect(detailVeiled()).toBe(true);
  });

  it("should re-read an open detail when updates change: a new link brings its labels, and the adult veil lifts", async () => {
    await wakeOn();
    await useSettings().setVeil("adult");
    await openDetail("Brine Water Clarity");
    expect(unlabeledLine()).toBe(
      "No Nexus labels: link this package on the NEXUS page and its category and tags follow.",
    );
    // unlinked, so unknown: the adult veil keeps it blurred
    expect(detailVeiled()).toBe(true);

    await useFarWorld().link(packageOf("Brine Water Clarity").packageId, "3302");
    await until(() => detailChipNames().length > 0, "Brine's labels in the open detail");
    expect(detailChipNames()).toStrictEqual(["Visuals", "Environment"]);
    expect(unlabeledLine()).toBeNull();
    expect(detailVeiled()).toBe(false);
    expect(textOf(document.querySelector(".odetail__nexus") ?? document.body).trim()).toBe("Nexus #3302 ↗");
  });
});
