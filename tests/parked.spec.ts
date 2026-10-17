// v0.2 — PARKED on Your World: files the stone kept aside rather than delete
// or force back. Put back says what it swapped out; Discard, the one act here
// that loses bytes, asks a second time (Keep it backs out); a refusal speaks
// in the parked register and reads the list again; every slip's ticket is the
// seq of its `backups/<seq>/<gamePath>` handle; and Preflight's Review lands
// on PARKED, focused, exactly once.

import { flushPromises } from "@vue/test-utils";
import { afterEach, describe, expect, it, vi } from "vitest";
import { PARKED_OCCUPIED_LINE, parkedTicket } from "@/services/ledger";
import { useInstalled } from "@/composables/useInstalled";
import { useNavigation } from "@/composables/useNavigation";
import { useToasts } from "@/composables/useToasts";
import { button, hasButton, sleep, spine, text, textAt, textOf, until, wakeOn } from "./harness";

afterEach(() => {
  vi.restoreAllMocks();
  sleep();
});

/** The two files the mock keeps aside (./mock/index.ts), under the spine's own handle shape. */
const SKY = "natives/stm/environment/sky/sky_noon.tex.251211553";
const BANTER = "reframework/autorun/pawn_banter.lua";
const SKY_STORE = `backups/17/${SKY}`;
const BANTER_STORE = `backups/21/${BANTER}`;

const slips = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>(".home__parked .park")];
function slip(store: string): HTMLElement {
  const found = slips().find((s) => s.dataset.store === store);
  if (!found)
    throw new Error(
      `PARKED holds no slip for ${store}: ${slips()
        .map((s) => s.dataset.store)
        .join(" | ")}`,
    );
  return found;
}
function within(store: string, selector: string): HTMLButtonElement {
  const found = slip(store).querySelector<HTMLButtonElement>(selector);
  if (!found) throw new Error(`The slip for ${store} has no ${selector}`);
  return found;
}
const part = (el: Element, selector: string): string => textOf(el.querySelector(selector) ?? el).trim();
const tickets = (): string[] => slips().map((s) => part(s, ".park__tag"));
const note = (): string | null => textAt(".park__note");
const lastToast = (): { tone: string; line: string } | null => {
  const last = useToasts().toasts.value.at(-1);
  return last ? { tone: last.tone, line: last.line } : null;
};

async function wakeOnParked(): Promise<void> {
  await wakeOn();
  useNavigation().go("world");
  await until(() => slips().length === 2, "the two parked slips");
}

describe("PARKED", () => {
  it("should put a parked file back, say what it swapped out, and list the swapped-out file in its turn", async () => {
    await wakeOnParked();
    expect(part(slip(SKY_STORE), ".park__occ")).toBe(PARKED_OCCUPIED_LINE);
    expect(within(SKY_STORE, ".park__putback").title).toBe(
      `Something sits at ${SKY} now: putting this back keeps that one aside in its turn, unless an install owns it — then the stone holds back and says which.`,
    );
    const restore = vi.spyOn(spine(), "restoreParked");

    within(SKY_STORE, ".park__putback").click();
    await until(() => note()?.includes("is back in your game") === true, "the put-back note");
    expect(restore).toHaveBeenCalledWith(SKY_STORE);
    const line = `${SKY} is back in your game. The file that sat there is kept aside in its turn: it is in the PARKED list now.`;
    expect(note()).toBe(line);
    expect(lastToast()).toStrictEqual({ tone: "world", line });

    await until(() => tickets().includes("#41"), "the swapped-out file under PARKED");
    expect(tickets()).toStrictEqual(["#21", "#41"]);
    const swapped = slip(`backups/41/${SKY}`);
    expect(part(swapped, ".park__path")).toBe(SKY);
    expect(part(swapped, ".park__meta")).toMatch(/^the file that sat there · 3\.0 KB · kept aside /);
    // the next press belongs to the slip now first in line
    await until(() => document.activeElement === within(BANTER_STORE, ".park__putback"), "focus on the next Put back");
  });

  it("should ask before discarding a kept copy, and back out on Keep it", async () => {
    await wakeOnParked();
    const discard = vi.spyOn(spine(), "discardParked");
    const asking = () => textOf(slip(SKY_STORE)).includes("Discard for good? The kept copy can't come back.");

    within(SKY_STORE, ".park__discard").click();
    await until(asking, "the second question");
    expect(discard).not.toHaveBeenCalled();
    // the safe answer takes the focus
    await until(() => document.activeElement === within(SKY_STORE, ".park__keep"), "focus on Keep it");

    within(SKY_STORE, ".park__keep").click();
    await until(() => !asking(), "the question withdrawn");
    expect(discard).not.toHaveBeenCalled();
    await until(() => document.activeElement === within(SKY_STORE, ".park__discard"), "focus back on Discard");
    expect(tickets()).toStrictEqual(["#17", "#21"]);

    within(SKY_STORE, ".park__discard").click();
    await until(asking, "the second question, again");
    button("Discard for good").click();
    await until(() => slips().length === 1, "the slip gone");
    expect(discard).toHaveBeenCalledTimes(1);
    expect(discard).toHaveBeenCalledWith(SKY_STORE);
    expect(note()).toBe(`The kept copy of ${SKY} is gone for good. Your game folder was not touched.`);
    expect(tickets()).toStrictEqual(["#21"]);
  });

  it("should voice a refused put back in the parked register and read the list again", async () => {
    await wakeOnParked();
    const reads = vi.spyOn(spine(), "getInstalled");

    within(BANTER_STORE, ".park__putback").click();
    await until(() => note()?.startsWith("The stone held back") === true, "the refusal");
    const line = `The stone held back: "Pawn Chatter Filter" is installed over ${BANTER}, so the kept file cannot go back there now. Uninstall it first, then put the kept file back. Nothing was moved, and the PARKED list has been read again.`;
    expect(note()).toBe(line);
    expect(lastToast()).toStrictEqual({ tone: "ember", line });
    expect(reads).toHaveBeenCalledTimes(1);
    expect(tickets()).toStrictEqual(["#17", "#21"]);
    expect(text()).not.toContain("THE WORLD IS AWAKE");
  });

  it("should read each ticket from its store handle's seq, never from the file name", () => {
    expect(parkedTicket(SKY_STORE, 0)).toBe("#17");
    expect(parkedTicket(BANTER_STORE, 4)).toBe("#21");
    // a handle in another shape falls back to the row's place in the list
    expect(parkedTicket(SKY, 2)).toBe("#3");
    expect(parkedTicket("sky_noon.tex.251211553", 0)).toBe("#1");
    expect(parkedTicket(`backups/1234567/${SKY}`, 5)).toBe("#6");
  });

  it("should draw the tickets on the slips' stubs", async () => {
    await wakeOnParked();
    expect(tickets()).toStrictEqual(["#17", "#21"]);
  });

  it("should land Preflight's Review on PARKED, focused, exactly once", async () => {
    await wakeOn();
    const nav = useNavigation();
    nav.go("preflight");
    await until(() => hasButton("Review"), "the parked-backups check");
    expect(text()).toContain("2 files are kept aside in the backup store");
    const scrolls = vi.spyOn(Element.prototype, "scrollIntoView");
    const parkedScrolls = () =>
      scrolls.mock.contexts.filter((el) => el instanceof Element && el.classList.contains("home__parked")).length;

    button("Review").click();
    await until(() => document.activeElement?.getAttribute("aria-label") === "Parked files", "PARKED in focus");
    expect(nav.page.value).toBe("world");
    expect(parkedScrolls()).toBe(1);
    expect(useInstalled().parkedSummons.value).toBe(0);

    // an ordinary later visit to Your World is not pulled down to PARKED again
    nav.go("rift");
    await until(() => document.querySelector(".home__parked") === null, "Your World left");
    nav.go("world");
    await until(() => slips().length === 2, "PARKED on the second visit");
    await new Promise((resolve) => setTimeout(resolve, 60));
    await flushPromises();
    expect(document.activeElement?.getAttribute("aria-label")).not.toBe("Parked files");
    expect(parkedScrolls()).toBe(1);
  });
});
