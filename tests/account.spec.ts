// v0.3 THE FAR WORLD's ACCOUNT section (WINDOW-CONTRACT § v0.3 "The key's
// extras", design system #00015 R16.17, R16.24): the key chip says what Nexus
// said about a paste, the update check never waits on the key's v1 quota, a
// key that changes hands forgets the old account's standings, and the nxm
// switch stays within reach while the stone answers nxm:// links.
import { afterEach, describe, expect, it, vi } from "vitest";
import type { NexusStandings } from "@/types/rift";
import { MOCK_ACCOUNT_NAME } from "@/services/mock/far";
import { RiftFailure } from "@/services/rift";
import { useFarWorld } from "@/composables/useFarWorld";
import { useNavigation } from "@/composables/useNavigation";
import { button, hasButton, sleep, spine, text, textOf, until, wakeOn, type Scene } from "./harness";

afterEach(() => {
  sleep();
  vi.restoreAllMocks();
});

/** An invented key: the window passes it to the spine and never shows it. */
const KEY = "rift-bench-key-0000-invented";
/** A second invented account, whose key replaces the first's. */
const OTHER_ACCOUNT = "Warden of the Old Road";

function must<T>(found: T | null | undefined, what: string): T {
  if (found === null || found === undefined) throw new Error(`The far world drew no ${what}`);
  return found;
}

async function openFarWorld(scene: Scene = ""): Promise<void> {
  await wakeOn(scene);
  useNavigation().go("nexus");
  await until(() => hasButton("Find all by name") || hasButton("Search"), "the far world");
}

const keyChip = (): string => textOf(must(document.querySelector("#far-account .far__chip"), "key chip")).trim();
const trackedCards = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>("#far-tracked [data-mod]")];
/** Lantern Glow Recolor's Endorse / Abstain pair, in its FILES view (R16.22). */
async function lanternStanding(): Promise<HTMLElement> {
  await until(
    () => document.querySelector('[aria-label="Files of Lantern Glow Recolor on Nexus"]') !== null,
    "Lantern's update row",
  );
  must(
    document.querySelector<HTMLButtonElement>('[aria-label="Files of Lantern Glow Recolor on Nexus"]'),
    "Files",
  ).click();
  await until(() => document.querySelector('[data-mod-files] [data-standing="412"]') !== null, "Lantern's standing");
  return must(document.querySelector<HTMLElement>('[data-mod-files] [data-standing="412"]'), "Lantern's standing");
}
/** Back from a mod's FILES to the far world. */
async function leaveFiles(): Promise<void> {
  window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
  await until(() => document.querySelector("[data-mod-files]") === null, "the far world again");
}
const cardName = (card: Element): string => textOf(must(card.querySelector(".fcard__name"), "card name")).trim();
const nxmSwitch = (): HTMLButtonElement | null => document.querySelector<HTMLButtonElement>("[role=switch]");
const nxmLine = (): string => textOf(document.getElementById("nxm-why") ?? document.body).trim();

function buttonIn(el: Element, label: string): HTMLButtonElement {
  return must(
    [...el.querySelectorAll<HTMLButtonElement>("button")].find(
      (b) => textOf(b.querySelector(".rs-btn__label") ?? b).trim() === label,
    ),
    `"${label}" button`,
  );
}

/** Paste a key into the ACCOUNT section's field and press Save & test. */
async function pasteKey(key: string): Promise<void> {
  const opener = hasButton("Paste key…") ? "Paste key…" : "Replace key…";
  button(opener).click();
  await until(() => document.querySelector("#far-account input[type=password]") !== null, "the key field");
  const field = must(document.querySelector<HTMLInputElement>("#far-account input[type=password]"), "key field");
  field.value = key;
  field.dispatchEvent(new Event("input"));
  await until(() => !button("Save & test").disabled, "Save & test ready");
  button("Save & test").click();
}

describe("the key chip says what Nexus said about a paste", () => {
  it("should read a paste Nexus could not be asked about as OFFLINE, never REJECTED, with nothing stored", async () => {
    await openFarWorld("offline");
    await until(() => keyChip() === "NO KEY", "the keyless account");
    await pasteKey(KEY);
    await until(() => text().includes("THE FAR WORLD IS OUT OF REACH"), "the paste's failure");
    expect(keyChip()).toBe("OFFLINE");
    expect(text()).not.toContain("REJECTED");
    // the spine stored nothing: there is no key to remove or to check again
    expect(hasButton("Remove key")).toBe(false);
    expect(hasButton("Check again")).toBe(false);
  });

  it("should read a paste Nexus rationed as no key at all, never REJECTED", async () => {
    await openFarWorld();
    await until(() => keyChip() === "NO KEY", "the keyless account");
    vi.spyOn(spine(), "setNexusKey").mockRejectedValue(
      new RiftFailure({
        kind: "nexus",
        detail: {
          detail: "Nexus Mods asked the Riftstone to slow down. Try again in 30 seconds.",
          reason: "rateLimited",
        },
      }),
    );
    await pasteKey(KEY);
    await until(() => text().includes("Nexus has rationed the stone for now"), "the paste's failure");
    expect(keyChip()).toBe("NO KEY");
  });

  it("should keep REJECTED for a key Nexus refused", async () => {
    await openFarWorld();
    await until(() => keyChip() === "NO KEY", "the keyless account");
    // the mock plays a key Nexus refuses when it carries the word
    await pasteKey("rift-bench-reject-0000");
    await until(() => keyChip() === "REJECTED", "the refused key");
    expect(text()).toContain("THE FAR WORLD REFUSES");
  });
});

describe("the update check and the key's v1 quota", () => {
  it("should let Check now run with the hourly v1 quota spent, and say the quota is the key's calls only", async () => {
    await openFarWorld("quotalow");
    vi.spyOn(spine(), "nexusQuota").mockResolvedValue({
      hourlyRemaining: 0,
      hourlyLimit: 500,
      dailyRemaining: 1_204,
      dailyLimit: 20_000,
      hourlyResetMs: Date.now() + 25 * 60_000,
    });
    await useFarWorld().refreshQuota();
    await until(() => text().includes("No requests left this hour"), "the spent quota's line");
    expect(text()).toContain(
      "No requests left this hour for downloads, tracking and endorsing; update checks don't use them.",
    );
    expect(text()).not.toContain("re-checks only the mods you open");
    expect(hasButton("Check now anyway")).toBe(false);
    const check = button("Check now");
    expect([check.disabled, check.getAttribute("aria-disabled")]).toStrictEqual([false, null]);
    const checked = vi.spyOn(spine(), "checkUpdates");
    check.click();
    await until(() => checked.mock.calls.length === 1, "the anonymous check sent");
  });

  it("should count the key's requests left in the same words while some remain", async () => {
    await openFarWorld("quotalow");
    await until(() => text().includes("12 requests left this hour"), "the low quota's line");
    expect(text()).toContain(
      "12 requests left this hour for downloads, tracking and endorsing; update checks don't use them.",
    );
  });
});

describe("a key that changes hands", () => {
  it("should forget the old account's tracked mods and refusals, and read the new account's", async () => {
    await openFarWorld("key");
    await until(() => trackedCards().length === 3, "the first account's tracked mods");
    // a refusal under the first account's own controls, in Lantern's FILES (R16.22:
    // Endorse / Abstain live there, never on the update row)
    vi.spyOn(spine(), "setEndorsement").mockRejectedValueOnce(
      new RiftFailure({
        kind: "nexus",
        detail: { detail: "Nexus refused: you cannot endorse your own mod.", reason: "endorseOwnMod" },
      }),
    );
    const pair = await lanternStanding();
    buttonIn(pair, "Endorse").click();
    await until(() => document.querySelector(".stand__refusal") !== null, "the first account's refusal");
    // the refusal outlives the view: leaving and coming back still says it
    await leaveFiles();
    await lanternStanding();
    expect(document.querySelector(".stand__refusal")).not.toBeNull();
    await leaveFiles();

    // the new key validates as another account, which tracks one mod of its own
    const overview = spine().getOverview.bind(spine());
    vi.spyOn(spine(), "getOverview").mockImplementation(async () => {
      const o = await overview();
      const account = o.nexus.account ? { ...o.nexus.account, name: OTHER_ACCOUNT } : null;
      return { ...o, nexus: { ...o.nexus, account } };
    });
    const first = await spine().getNexusStandings(false);
    const theirs: NexusStandings = {
      tracked: first.tracked.slice(2),
      endorsements: [],
      checkedMs: Date.now(),
    };
    const asked = vi.spyOn(spine(), "getNexusStandings").mockResolvedValue(theirs);
    await pasteKey(KEY);
    await until(() => text().includes(OTHER_ACCOUNT), "the new account named");
    expect(text()).not.toContain(MOCK_ACCOUNT_NAME);
    await until(() => trackedCards().length === 1, "the new account's tracked mods");
    expect(trackedCards().map(cardName)).toStrictEqual(theirs.tracked.map((c) => c.name));
    expect(asked).toHaveBeenCalled();
    // the new account never refused anything: Lantern's FILES say nothing under the pair
    await lanternStanding();
    expect(document.querySelector(".stand__refusal")).toBeNull();
  });
});

describe("the nxm switch while the stone answers nxm:// links", () => {
  it("should stay within reach when the key goes, so the protocol can be given back", async () => {
    await openFarWorld("key");
    await until(() => nxmLine() === "No program answers it now.", "the switch at none");
    must(nxmSwitch(), "switch").click();
    await until(() => nxmSwitch()?.getAttribute("aria-checked") === "true", "the switch on");

    button("Remove key").click();
    await until(() => keyChip() === "NO KEY", "the key removed");
    // the stone still answers the button: the switch that gives it back stays on the page
    await until(() => nxmSwitch() !== null, "the switch, keyless");
    expect(nxmSwitch()?.getAttribute("aria-checked")).toBe("true");
    expect(nxmLine()).toBe("The stone answers it now. Turning this off gives it back to no program.");
    const set = vi.spyOn(spine(), "setNxmHandler");
    must(nxmSwitch(), "switch").click();
    await until(() => nxmSwitch()?.getAttribute("aria-checked") === "false", "the switch off");
    expect(set.mock.calls).toStrictEqual([[false]]);
    // one switch, never two
    expect(document.querySelectorAll("[role=switch]")).toHaveLength(1);
  });
});
