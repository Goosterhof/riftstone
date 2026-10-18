// v0.5 — THE MUSTER by character (#00015 §18.8–§18.10): the characters held
// by key, the slots apart, the remembered pick written once per pick and
// never on the window's own, the slot menu's select-only combobox, following
// and pinned, THE RE-CARVING across slots, `5`, the character who is gone,
// and the slot the game is writing. Every name is the mock's invented party.

import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { useSettings } from "@/composables/useSettings";
import { MOCK_CHARACTERS } from "@/services/mock/muster";
import { GONE_LINE, PINNED_NEWER_SENTENCE } from "@/services/muster";
import { press, sleep, spine, text, textAt, textOf, until, wakeOn, type Scene } from "./harness";

afterEach(() => {
  vi.restoreAllMocks();
  sleep();
});

/** §18.15's scenes (the harness's union predates them). */
type V05Scene = "musterchars" | "musterpinned" | "musterfollow" | "musterwriting" | "mustergone";

async function openMuster(scene: V05Scene = "musterchars", ready = "Odalric"): Promise<void> {
  await wakeOn(scene as unknown as Scene);
  press("5", {}, window);
  await until(() => !!document.querySelector('[role="tablist"]') && sheet().includes(ready), `the party of ${ready}`);
  await flushPromises();
}

const sheet = (): string => textOf(document.querySelector(".muster__sheetcol") ?? document.createElement("i"));
const live = (): string | null => textAt('[aria-live="polite"].rs-sr-only');
/** The head's line 2 as it reads on screen: the screen-reader-only words left out. */
function line(): string {
  const at = document.querySelector(".muster__slotline");
  if (!at) return "";
  const copy = at.cloneNode(true) as HTMLElement;
  for (const hidden of copy.querySelectorAll(".rs-sr-only")) hidden.remove();
  return textOf(copy).replace(/\s+/g, " ").trim();
}

function characterRows(): HTMLElement[] {
  return [...document.querySelectorAll<HTMLElement>('[aria-label="Characters"] [role="option"]')];
}

function characterRow(name: string): HTMLElement {
  const row = characterRows().find((r) => textOf(r).includes(name));
  if (!row) throw new Error(`No character row for ${name}`);
  return row;
}

function slotButton(): HTMLElement {
  const el = document.querySelector<HTMLElement>('[role="combobox"]');
  if (!el) throw new Error("No slot menu button");
  return el;
}

const entry = (slotId: string): HTMLElement | null => document.getElementById(`muster-slotopt-${slotId}`);
const buttonSlot = (): string => textOf(slotButton().querySelector(".smenu__slot") ?? slotButton()).trim();

/** An element's words as they read, trimmed. */
const words = (el: Element | null): string => textOf(el ?? document.createElement("i")).trim();

/** Open the slot menu by click and pick an entry. */
async function pickFromMenu(slotId: string): Promise<void> {
  slotButton().click();
  await until(() => entry(slotId) !== null, `the menu entry for ${slotId}`);
  entry(slotId)?.click();
}

/** The `musterCharacter` of every settings write a spy saw, in order. */
function writesOf(set: { mock: { calls: unknown[][] } }): (string | null)[] {
  return set.mock.calls.map((call) => (call[0] as { musterCharacter: string | null }).musterCharacter);
}

describe("THE MUSTER by character — the left column (R18.15–R18.17)", () => {
  it("should list two characters, newest first, with no slot number among them", async () => {
    await openMuster();
    const rows = characterRows().map((r) => textOf(r).replace(/\s+/g, " ").trim());
    expect(rows).toHaveLength(2);
    expect(rows[0]).toMatch(/^Odalric & Wren\s*Lv 41\s*Warrior\s*3 saves · today 21:04$/);
    expect(rows[1]).toMatch(/^Aldith & Corvin\s*Lv 19\s*Fighter\s*2 saves · 14 Oct 2026, 22:10$/);
    for (const row of rows) expect(row).not.toMatch(/Slot \d/);
    expect(textAt("#muster-slots-h")).toBe("CHARACTERS");
    expect(characterRow("Odalric").getAttribute("aria-label")).toBe(
      "Odalric and Wren, level 41 Warrior, 3 saves, last saved today 21:04",
    );
    expect(characterRow("Odalric").getAttribute("aria-selected")).toBe("true");
  });

  it("should keep the words apart where Vue would condense a blank away", async () => {
    await openMuster();
    expect(words(characterRow("Odalric").querySelector(".who__sum"))).toBe("Lv 41 Warrior");
    expect(words(document.querySelector(".muster__rest"))).toBe("data009Slot · following the newest save");
  });

  it("should never put a character's key in the DOM, nor a name in a title", async () => {
    await openMuster();
    await pickFromMenu("data008Slot");
    await until(() => line().includes("pinned"), "the pin");
    slotButton().click();
    await until(() => entry("data009Slot") !== null, "the menu");
    const keys = Object.values(MOCK_CHARACTERS);
    const names = ["Odalric", "Wren", "Bramble", "Sable", "Aldith", "Corvin"];
    const titles: string[] = [];
    for (const el of document.querySelectorAll("*")) {
      for (const attr of el.attributes) {
        for (const key of keys) expect(attr.value).not.toContain(key);
        if (attr.name === "title") titles.push(attr.value);
      }
    }
    for (const name of names) expect(titles.join("\n")).not.toContain(name);
    expect(document.body.innerHTML).not.toMatch(new RegExp(keys.join("|")));
  });

  it("should set the slots that won't open apart, under their problem, in the fixed order", async () => {
    await openMuster();
    expect(textAt(".roster__apart")).toBe("2 WON'T ANSWER");
    const groups = [...document.querySelectorAll('[aria-label="Saves that won\'t open"] [role="group"]')].map((g) =>
      textOf(g).replace(/\s+/g, " ").trim(),
    );
    expect(groups).toHaveLength(2);
    expect(groups[0]).toMatch(/^won't open\s*Slot 3\s*12 Oct 2026, 18:40$/);
    expect(groups[1]).toMatch(/^another account's save\s*Slot 1\s*9 Oct 2026, 11:02$/);
    const set = vi.spyOn(spine(), "setSettings");
    document.querySelector<HTMLElement>('[data-slot="data003Slot"]')?.click();
    await until(() => text().includes("THIS SAVE WON'T OPEN"), "Slot 3's state");
    // a slot apart is never a character: the head has no menu, the memory is untouched
    expect(document.querySelector('[role="combobox"]')).toBeNull();
    expect(line()).toMatch(/^Slot 3 · data003Slot · saved 12 Oct 2026, 18:40/);
    expect(set).not.toHaveBeenCalled();
  });

  it("should cross from the last character into the slots apart, and back", async () => {
    await openMuster();
    characterRow("Odalric").focus();
    press("ArrowDown");
    expect(document.activeElement).toBe(characterRow("Aldith"));
    press("ArrowDown");
    expect(document.activeElement?.getAttribute("data-slot")).toBe("data003Slot");
    press("ArrowUp");
    expect(document.activeElement).toBe(characterRow("Aldith"));
    // two tab stops, the characters' first
    expect(characterRows().filter((r) => r.tabIndex === 0)).toHaveLength(1);
    expect(document.querySelectorAll('[data-slot][tabindex="0"]')).toHaveLength(1);
  });

  it("should keep the sheet and the frame on Odalric when Aldith's save moves her row to the top", async () => {
    await openMuster();
    spine().musterMock.saveSlot("data007Slot");
    await until(() => textOf(characterRows()[0] ?? document.body).includes("Aldith"), "Aldith's row on top");
    expect(textOf(characterRows()[0] ?? document.body)).toMatch(/Lv 20\s*Fighter/);
    expect(characterRow("Odalric").getAttribute("aria-selected")).toBe("true");
    expect(characterRow("Aldith").getAttribute("aria-selected")).toBe("false");
    expect(sheet()).toContain("Odalric");
    expect(buttonSlot()).toBe("Slot 9");
  });

  it("should carry THE RE-CARVING's mark on the shown character's row only", async () => {
    await openMuster();
    spine().gameSaved();
    await until(() => text().includes("5 CHANGED"), "the marks");
    expect(textOf(characterRow("Odalric"))).toContain("•");
    expect(textOf(characterRow("Aldith"))).not.toContain("•");
  });
});

describe("THE MUSTER by character — the remembered pick (R18.18, R18.26, R18.27)", () => {
  it("should write musterCharacter once per pick, after the read starts, and never on an arrow", async () => {
    await openMuster();
    const set = vi.spyOn(spine(), "setSettings");
    const read = vi.spyOn(spine(), "getMuster");
    characterRow("Odalric").focus();
    press("ArrowDown");
    press("ArrowUp");
    press("ArrowDown");
    await flushPromises();
    expect(set).not.toHaveBeenCalled();
    expect(read).not.toHaveBeenCalled();
    press("Enter");
    await until(() => sheet().includes("Aldith"), "Aldith's party");
    expect(read.mock.calls).toStrictEqual([["data007Slot"]]);
    expect(writesOf(set)).toStrictEqual([MOCK_CHARACTERS.aldith]);
    expect(read.mock.invocationCallOrder[0]).toBeLessThan(set.mock.invocationCallOrder[0] ?? 0);
    // the character already shown, on its newest save: no read, no write
    characterRow("Aldith").click();
    await flushPromises();
    expect(read).toHaveBeenCalledTimes(1);
    expect(set).toHaveBeenCalledTimes(1);
    expect(useSettings().settings.value.musterCharacter).toBe(MOCK_CHARACTERS.aldith);
  });

  it("should open the remembered character at their newest save, following, on 5", async () => {
    await wakeOn("musterchars" as unknown as Scene);
    await spine().setSettings({ ...useSettings().settings.value, musterCharacter: MOCK_CHARACTERS.aldith });
    await useSettings().loadSettings();
    const set = vi.spyOn(spine(), "setSettings");
    press("5", {}, window);
    await until(() => sheet().includes("Aldith"), "Aldith's party");
    expect(buttonSlot()).toBe("Slot 7");
    expect(line()).toContain("following the newest save");
    await until(() => document.activeElement === characterRow("Aldith"), "the focus on Aldith's row");
    expect(set).not.toHaveBeenCalled();
  });

  it("should not keep a pin across a room switch: 1 then 5 shows the newest save, following", async () => {
    await openMuster();
    await pickFromMenu("data004Slot");
    await until(() => buttonSlot() === "Slot 4" && sheet().includes("Lv 33"), "Slot 4 pinned");
    expect(line()).toContain("· pinned");
    press("1", {}, window);
    await until(() => !document.querySelector(".muster"), "the Rift");
    press("5", {}, window);
    await until(() => !!document.querySelector('[role="combobox"]') && buttonSlot() === "Slot 9", "Slot 9 again");
    await until(() => sheet().includes("Lv 41"), "Slot 9's party");
    expect(line()).toContain("following the newest save");
  });

  it("should open on the newest save with the gone line, and write nothing until a pick", async () => {
    await wakeOn("mustergone" as unknown as Scene);
    const set = vi.spyOn(spine(), "setSettings");
    expect(useSettings().settings.value.musterCharacter).toBe(MOCK_CHARACTERS.gone);
    press("5", {}, window);
    await until(() => sheet().includes("Odalric"), "Odalric's party");
    expect(textAt(".roster__gone")).toBe(GONE_LINE);
    expect(buttonSlot()).toBe("Slot 9");
    await new Promise((resolve) => setTimeout(resolve, 150));
    expect(set).not.toHaveBeenCalled();
    characterRow("Aldith").click();
    await until(() => sheet().includes("Aldith"), "Aldith's party");
    expect(document.querySelector(".roster__gone")).toBeNull();
    expect(writesOf(set)).toStrictEqual([MOCK_CHARACTERS.aldith]);
  });

  it("should tell the gone character once per session", async () => {
    await wakeOn("mustergone" as unknown as Scene);
    press("5", {}, window);
    await until(() => text().includes(GONE_LINE), "the gone line");
    characterRow("Odalric").click();
    await until(() => document.querySelector(".roster__gone") === null, "the line cleared by a pick");
    press("1", {}, window);
    await until(() => !document.querySelector(".muster"), "the Rift");
    press("5", {}, window);
    await until(() => sheet().includes("Odalric"), "the Muster again");
    expect(text()).not.toContain(GONE_LINE);
  });
});

describe("THE MUSTER by character — the slot menu (R18.20–R18.22)", () => {
  it("should be the head's line: the button, the file name and the state words", async () => {
    await openMuster();
    const button = slotButton();
    expect(button.getAttribute("aria-haspopup")).toBe("listbox");
    expect(button.getAttribute("aria-expanded")).toBe("false");
    expect(button.getAttribute("aria-label")).toBe("Save slot of Odalric");
    expect(line()).toMatch(/^Slot 9 · today 21:04\s*▾\s*data009Slot\s*· following the newest save$/);
    expect(document.querySelectorAll('.muster__head [role="combobox"]')).toHaveLength(1);
  });

  it("should list the character's slots newest first, the shown one checked, NEWEST on the first", async () => {
    await openMuster();
    slotButton().click();
    await until(() => entry("data009Slot") !== null, "the menu");
    const entries = [...document.querySelectorAll('#muster-slotmenu [role="option"]')];
    const shown = entries.map((e) => textOf(e).replace(/\s+/g, " ").trim());
    expect(shown).toHaveLength(3);
    expect(shown[0]).toMatch(/^✓\s*Slot 9\s*today 21:04\s*Lv 41\s*NEWEST$/);
    expect(shown[1]).toMatch(/^Slot 8\s*yesterday 20:12\s*Lv 40$/);
    expect(shown[2]).toMatch(/^Slot 4\s*11 Oct 2026, 19:30\s*Lv 33$/);
    expect(entry("data009Slot")?.getAttribute("aria-selected")).toBe("true");
    expect(entry("data009Slot")?.getAttribute("aria-label")).toBe("Slot 9, saved today 21:04, level 41, newest");
  });

  it("should open on the shown slot, move without reading, pick on Enter and say pinned", async () => {
    await openMuster();
    const read = vi.spyOn(spine(), "getMuster");
    const button = slotButton();
    button.focus();
    press("ArrowDown");
    await flushPromises();
    expect(button.getAttribute("aria-expanded")).toBe("true");
    expect(button.getAttribute("aria-activedescendant")).toBe("muster-slotopt-data009Slot");
    press("ArrowDown");
    await flushPromises();
    expect(button.getAttribute("aria-activedescendant")).toBe("muster-slotopt-data008Slot");
    press("End");
    press("Home");
    press("ArrowDown");
    await flushPromises();
    expect(read).not.toHaveBeenCalled();
    press("Enter");
    await until(() => buttonSlot() === "Slot 8" && sheet().includes("Lv 40"), "Slot 8 on the sheet");
    expect(read.mock.calls).toStrictEqual([["data008Slot"]]);
    expect(button.getAttribute("aria-expanded")).toBe("false");
    expect(document.activeElement).toBe(button);
    expect(line()).toMatch(/data008Slot\s*· pinned$/);
  });

  it("should open on the last entry by ↑, close on Esc and Tab without picking, and let a digit through", async () => {
    await openMuster();
    const read = vi.spyOn(spine(), "getMuster");
    const button = slotButton();
    button.focus();
    press("ArrowUp");
    await flushPromises();
    expect(button.getAttribute("aria-activedescendant")).toBe("muster-slotopt-data004Slot");
    press("Escape");
    await flushPromises();
    expect(button.getAttribute("aria-expanded")).toBe("false");
    expect(document.activeElement).toBe(button);
    press("Enter");
    await flushPromises();
    expect(button.getAttribute("aria-expanded")).toBe("true");
    const tab = press("Tab");
    await flushPromises();
    expect(tab.defaultPrevented).toBe(false);
    expect(button.getAttribute("aria-expanded")).toBe("false");
    press(" ");
    await flushPromises();
    expect(button.getAttribute("aria-expanded")).toBe("true");
    // no type-ahead: a digit is a room key, it closes the menu and goes through
    press("1");
    await until(() => !document.querySelector(".muster"), "the Rift by its key");
    expect(read).not.toHaveBeenCalled();
  });

  it("should close on a click outside", async () => {
    await openMuster();
    slotButton().click();
    await until(() => entry("data009Slot") !== null, "the menu");
    document.querySelector(".muster__sheet")?.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
    await flushPromises();
    expect(entry("data009Slot")).toBeNull();
  });

  it("should put the slot button first in the page's tab order, before the characters", async () => {
    await openMuster();
    const order = slotButton().compareDocumentPosition(characterRow("Odalric"));
    expect((order & Node.DOCUMENT_POSITION_FOLLOWING) !== 0).toBe(true);
    const tabs = document.querySelector('[role="tablist"]');
    expect(tabs).not.toBeNull();
    const after = tabs ? characterRow("Odalric").compareDocumentPosition(tabs) : 0;
    expect((after & Node.DOCUMENT_POSITION_FOLLOWING) !== 0).toBe(true);
  });
});

describe("THE MUSTER by character — following and pinned (R18.23–R18.25)", () => {
  it("should stay on a pinned slot, mark the newer save, say it once, and follow again on its own save", async () => {
    await openMuster("musterpinned");
    await pickFromMenu("data008Slot");
    await until(() => buttonSlot() === "Slot 8" && sheet().includes("Lv 40"), "Slot 8 pinned");
    const read = vi.spyOn(spine(), "getMuster");
    spine().musterMock.saveSlot("data009Slot");
    await until(() => line().includes("the game saved again in Slot 9"), "the newer save marked");
    expect(read).not.toHaveBeenCalled();
    expect(buttonSlot()).toBe("Slot 8");
    expect(line()).toContain("· pinned · the game saved again in Slot 9");
    expect(textOf(slotButton())).toMatch(/▾\s*•$/);
    expect(textAt("#muster-slotmenu-newer")).toBe("a newer save arrived");
    expect(live()).toBe(PINNED_NEWER_SENTENCE);
    slotButton().click();
    await until(() => entry("data009Slot") !== null, "the menu");
    expect(textOf(entry("data009Slot") ?? document.body)).toMatch(/^\s*•\s*Slot 9\s*today 21:34\s*Lv 42\s*NEWEST$/);
    expect(entry("data009Slot")?.getAttribute("aria-label")).toBe(
      "Slot 9, saved today 21:34, level 42, newest, saved since you picked slot 8",
    );
    slotButton().click();
    // a save into the pinned slot itself: re-carved, and being the newest, followed again
    spine().musterMock.saveSlot("data008Slot");
    await until(() => text().includes("CHANGED"), "Slot 8 re-carved");
    expect(buttonSlot()).toBe("Slot 8");
    // being the newest now, the pinned slot is followed again, and the re-read says when
    expect(line()).toContain("· following the newest save");
    expect(line()).not.toContain("pinned");
    expect(line()).toMatch(/· read again \d{2}:\d{2}/);
    expect(textOf(slotButton())).not.toContain("•");
  });

  it("should clear the newer marks when any entry is picked", async () => {
    await openMuster("musterpinned");
    await pickFromMenu("data008Slot");
    await until(() => line().includes("pinned"), "the pin");
    spine().musterMock.saveSlot("data009Slot");
    await until(() => line().includes("saved again"), "the mark");
    await pickFromMenu("data009Slot");
    await until(() => buttonSlot() === "Slot 9" && sheet().includes("Lv 42"), "Slot 9 followed");
    expect(line()).toContain("following the newest save");
    expect(textOf(slotButton())).not.toContain("•");
  });

  it("should follow into the newer slot, re-carve against the party it showed, by memberKey", async () => {
    await openMuster("musterfollow");
    spine().musterMock.saveSlot("data008Slot");
    await until(() => buttonSlot() === "Slot 8", "the sheet moved");
    await until(() => text().includes("CHANGED"), "the carved count");
    expect(textAt(".muster__changed")).toBe("5 CHANGED");
    expect(line()).toMatch(/· following the newest save · moved from Slot 9 at \d\d:\d\d/);
    expect(line()).not.toContain("read again");
    expect(textOf(document.querySelector("#muster-sheet") ?? document.body)).toMatch(/Lv\s*42\s*▴1/);
    expect(live()).toBe("The game saved in another slot. The stone moved the sheet there: 5 things changed.");
    // Thistle joined in Bramble's place: `new` on the hired tab, never forty marks
    const hired1 = document.querySelector('[role="tab"][data-place="hired1"]');
    expect(textOf(hired1 ?? document.body)).toContain("Thistle");
    expect(textOf(hired1 ?? document.body)).toContain("new");
    expect(textOf(hired1 ?? document.body)).not.toContain("•");
    // Sable keeps her place, Wren her marks-free sheet
    expect(textOf(document.querySelector('[data-place="hired2"]') ?? document.body)).toContain("Sable");
  });

  it("should never blank the sheet while the newer slot is being written, then move at the read", async () => {
    await openMuster("musterfollow");
    spine().musterMock.saveSlot("data008Slot", true);
    await until(() => line().includes("the game is saving in Slot 8"), "the saving words");
    expect(document.querySelector(".muster__state.is-warn")).not.toBeNull();
    expect(buttonSlot()).toBe("Slot 9");
    expect(document.querySelector('[role="tablist"]')).not.toBeNull();
    expect(text()).not.toContain("THE GAME IS SAVING");
    expect(text()).not.toContain("READING THE PARTY");
    await until(() => buttonSlot() === "Slot 8", "the move once the save settled", 4000);
    expect(line()).toMatch(/moved from Slot 9/);
  });

  it("should light the slot number for a frame after a move", async () => {
    await openMuster("musterfollow");
    let litSlot = false;
    const watcher = new MutationObserver(() => {
      if (document.querySelector(".smenu__slot.is-lit")) litSlot = true;
    });
    watcher.observe(document.body, { subtree: true, attributes: true, childList: true });
    spine().musterMock.saveSlot("data008Slot");
    await until(() => litSlot, "the slot's light");
    await until(() => !document.querySelector(".smenu__slot.is-lit"), "the settle");
    watcher.disconnect();
    expect(litSlot).toBe(true);
    expect(buttonSlot()).toBe("Slot 8");
  });
});

describe("THE MUSTER by character — a slot the game is writing keeps its character (R18.19)", () => {
  it("should keep Odalric's three saves and turn his rail warn, the apart group unchanged", async () => {
    await openMuster("musterwriting");
    await until(() => text().includes("Odalric") && !text().includes("THE GAME IS SAVING"), "the first settle");
    expect(textAt(".roster__apart")).toBe("2 WON'T ANSWER");
    spine().musterMock.saveSlot("data009Slot", true);
    await until(() => characterRow("Odalric").classList.contains("is-writing"), "the warn rail");
    expect(textOf(characterRow("Odalric"))).toContain("3 saves");
    expect(textAt(".roster__apart")).toBe("2 WON'T ANSWER");
    expect(line()).toContain("the game is saving in Slot 9");
    expect(document.querySelector('[role="tablist"]')).not.toBeNull();
    slotButton().click();
    await until(() => entry("data009Slot") !== null, "the menu");
    expect(textOf(entry("data009Slot") ?? document.body)).toContain("the game is writing it");
    expect(entry("data009Slot")?.getAttribute("aria-label")).toMatch(/, newest, the game is writing it$/);
    slotButton().click();
    await until(() => !characterRow("Odalric").classList.contains("is-writing"), "the save settled", 4000);
    await until(() => text().includes("CHANGED"), "re-carved");
    expect(line()).toContain("following the newest save");
  });
});
