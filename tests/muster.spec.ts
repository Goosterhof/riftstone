// v0.4 — THE MUSTER (#00015 §17): every state of R17.22 with its copy and its
// doors, the nameless things of R17.21, the fixed tab line of R17.8, the
// keyboard of R17.27, and THE RE-CARVING (R17.25) — exactly the changed
// values marked after rift://muster-changed, nothing when nothing changed,
// and no light under reduced motion. The window talks to the mock spine only.

import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { RiftFailure } from "@/services/rift";
import { useMuster } from "@/composables/useMuster";
import { button, hasButton, press, sleep, spine, text, textAt, textOf, until, wakeOn, type Scene } from "./harness";

afterEach(() => {
  vi.restoreAllMocks();
  sleep();
});

/** Wake on a scene and open THE MUSTER by its key. */
async function openMuster(scene: Scene = "", what: string | null = null): Promise<void> {
  await wakeOn(scene);
  press("5", {}, window);
  if (what) await until(() => text().includes(what), `THE MUSTER showing ${what}`);
  else await until(() => !!document.querySelector('[role="tablist"]'), "THE MUSTER's party");
  await flushPromises();
}

const saveFailure = (reason: string) =>
  new RiftFailure({ kind: "save", detail: { detail: `data009Slot: ${reason}.`, reason } });

function slotRow(slotId: string): HTMLElement {
  const row = document.querySelector<HTMLElement>(`[role="option"][data-slot="${slotId}"]`);
  if (!row) throw new Error(`No slot row ${slotId}`);
  return row;
}

function tab(place: string): HTMLElement {
  const el = document.querySelector<HTMLElement>(`[role="tab"][data-place="${place}"]`);
  if (!el) throw new Error(`No tab at ${place}`);
  return el;
}

async function pickSlot(slotId: string, what: string): Promise<void> {
  slotRow(slotId).click();
  await until(() => text().includes(what), what);
}

const sheetText = (): string => textOf(document.querySelector("#muster-sheet") ?? document.body).replace(/\s+/g, " ");
function gearRow(slot: string): string {
  const row = document.querySelector(`[data-gear="${slot}"]`);
  const part = (sel: string) => textOf(row?.querySelector(sel) ?? document.createElement("i")).trim();
  return `${part("dt")} ${part("dd")}`.replace(/\s+/g, " ").trim();
}

describe("THE MUSTER — the fifth room", () => {
  it("should open on 5 from any room, marked as the current page, the focus on the picked slot", async () => {
    await openMuster();
    const entry = [...document.querySelectorAll(".navlink")].find((b) => textOf(b).includes("THE MUSTER"));
    expect(entry?.getAttribute("aria-current")).toBe("page");
    expect(textAt(".muster__title")).toBe("THE MUSTER");
    await until(() => document.activeElement === slotRow("data009Slot"), "the focus on Slot 9");
  });

  it("should stay live while no world answers, and 1 should return to NO WORLD ANSWERS (R17.2)", async () => {
    await wakeOn("nogame");
    expect(text()).toContain("NO WORLD ANSWERS");
    press("5", {}, window);
    await until(() => text().includes("THE NAMES DID NOT ANSWER"), "the names notice");
    expect(document.querySelectorAll(".muster__notice")).toHaveLength(1);
    expect(gearRow("mainWeapon")).toMatch(/^Main weapon item \d+$/);
    press("1", {}, window);
    await until(() => text().includes("NO WORLD ANSWERS"), "the wake screen again");
    expect(document.querySelector(".muster")).toBeNull();
  });
});

describe("THE MUSTER — every state of R17.22", () => {
  it("should say NO PARTY ANSWERS with Look again when there is no Steam", async () => {
    await openMuster("musternosteam", "NO PARTY ANSWERS");
    expect(text()).toContain(
      "The stone found no Steam folder on this PC, and Dragon's Dogma 2 keeps its saves under Steam. The stone only reads saves; nothing on this PC was changed.",
    );
    expect(document.querySelector('[role="listbox"]')).toBeNull();
    const look = vi.spyOn(spine(), "getMusterSlots");
    button("Look again").click();
    await until(() => look.mock.calls.length === 1, "a second look");
  });

  it("should say THE PARTY HAS NOT GATHERED when Steam holds no save", async () => {
    await openMuster("musternosaves", "THE PARTY HAS NOT GATHERED");
    expect(text()).toContain("Play until the game saves once, and the stone reads your party from it.");
    expect(hasButton("Look again")).toBe(true);
  });

  it("should show THIS SAVE WON'T OPEN on a damaged slot, its rail ember, the slot list still there", async () => {
    await openMuster();
    expect(textOf(slotRow("data003Slot"))).toContain("won't open");
    await pickSlot("data003Slot", "THIS SAVE WON'T OPEN");
    expect(text()).toContain("The stone couldn't read data003Slot: the file is damaged inside.");
    expect(text()).toContain("The stone only reads saves, so it is exactly as it was.");
    for (const door of ["Show in folder", "Read again", "Copy details"]) expect(hasButton(door)).toBe(true);
    expect(document.querySelector(".mstate--ember")).not.toBeNull();
    expect(document.querySelector('[role="listbox"]')).not.toBeNull();
    expect(document.querySelector('[role="tablist"]')).toBeNull();
  });

  it("should show ANOTHER ARISEN'S SAVE with Show in folder and Pick another slot", async () => {
    await openMuster();
    await pickSlot("data001Slot", "ANOTHER ARISEN'S SAVE");
    expect(text()).toContain("data001Slot belongs to a Steam account the stone didn't find on this PC");
    const folder = vi.spyOn(spine(), "openSavesFolder");
    button("Show in folder").click();
    await until(() => folder.mock.calls.length === 1, "the folder opened");
    expect(folder.mock.calls[0]).toStrictEqual(["data001Slot"]);
    button("Pick another slot").click();
    await until(() => document.activeElement === slotRow("data009Slot"), "the newest slot that opens");
  });

  it("should show THE GAME IS SAVING once, then the party when the save settles", async () => {
    await openMuster("musterwriting", "THE GAME IS SAVING");
    expect(text()).toContain(
      "The game is writing data009Slot right now, or another program is holding it for a moment.",
    );
    expect(hasButton("Read again")).toBe(true);
    expect(document.querySelector(".mstate--warn")).not.toBeNull();
    await until(() => text().includes("Odalric") && !!document.querySelector('[role="tablist"]'), "the party read");
    expect(document.querySelector(".muster__changed")).toBeNull();
  });

  it("should voice noParty, gone and any other failure with their doors", async () => {
    await openMuster();
    const read = vi.spyOn(spine(), "getMuster");
    read.mockRejectedValueOnce(saveFailure("noParty"));
    await pickSlot("data008Slot", "NO PARTY IN THIS SAVE");
    expect(text()).toContain("data008Slot opens, but holds no Arisen and no pawns to read.");
    expect(hasButton("Pick another slot")).toBe(true);

    const list = vi.spyOn(spine(), "getMusterSlots");
    read.mockRejectedValueOnce(saveFailure("gone"));
    await pickSlot("data009Slot", "THIS SAVE IS GONE");
    expect(text()).toContain(
      "data009Slot left the save folder after the stone listed it. The stone is reading the list again.",
    );
    await until(() => list.mock.calls.length >= 1, "the list read again");

    read.mockRejectedValueOnce(new RiftFailure({ kind: "io", detail: { reason: "the disk refused" } }));
    await pickSlot("data008Slot", "THE STONE COULDN'T READ THE PARTY");
    expect(text()).toContain("The stone only reads saves; nothing was changed.");
    expect(hasButton("Read again")).toBe(true);
    expect(hasButton("Copy details")).toBe(true);
  });

  it("should say READING THE PARTY only after 200 ms of a read", async () => {
    await wakeOn("musterslow");
    press("5", {}, window);
    await until(() => !!document.querySelector('[role="listbox"]'), "the slot list");
    expect(text()).not.toContain("READING THE PARTY");
    await until(() => text().includes("READING THE PARTY"), "the reading state");
    expect(text()).toContain("the stone is learning the game's names");
    await until(() => text().includes("GEAR"), "the party", 4000);
  });

  it("should say Reading your saves… only after 200 ms of listing", async () => {
    await wakeOn();
    const backend = spine();
    const real = backend.getMusterSlots.bind(backend);
    vi.spyOn(backend, "getMusterSlots").mockImplementation(
      () => new Promise((resolve) => setTimeout(() => void real().then(resolve), 400)),
    );
    press("5", {}, window);
    await flushPromises();
    expect(text()).not.toContain("Reading your saves…");
    await until(() => text().includes("Reading your saves…"), "the listing line");
    await until(() => text().includes("Odalric"), "the party");
  });

  it("should carve the names notice once above the tabs when the names did not answer", async () => {
    await openMuster("musternames", "THE NAMES DID NOT ANSWER");
    expect(document.querySelectorAll(".muster__notice")).toHaveLength(1);
    expect(text()).toContain("The stone found no Dragon's Dogma 2 install, and the names live in the game's own text.");
    expect(text()).toContain("The party below is whole.");
    expect(document.querySelectorAll('[role="tab"]')).toHaveLength(4);
    expect(document.querySelectorAll("[data-gear]")).toHaveLength(8);
    expect(document.querySelectorAll("[data-gain]")).toHaveLength(8);
    expect(text()).not.toContain("unnamed in the game's text");
    button("Open Preflight").click();
    await until(() => !document.querySelector(".muster"), "PREFLIGHT");
  });
});

const health = () => document.querySelector('[data-gain="health"]');

describe("THE MUSTER — the sheet", () => {
  it("should name an unnamed thing by its kind and number, never blank (R17.21)", async () => {
    await openMuster();
    tab("hired2").click();
    await until(() => sheetText().includes("Sable"), "Sable's sheet");
    const ring = document.querySelector('[data-gear="ring1"] dd');
    expect(textOf(ring?.querySelector(".named__text") ?? document.body)).toBe("item 40917");
    expect(ring?.querySelector(".named__text")?.classList.contains("rs-mono")).toBe(true);
    expect(textOf(ring?.querySelector(".named__note") ?? document.body)).toContain("· unnamed in the game's text");
    // the explanation is the note's title and its accessible text, never a blank cell
    expect(ring?.querySelector(".named__note")?.getAttribute("title")).toContain(
      "The game's text has no name for this one.",
    );
  });

  it("should read a person the save did not name as the role in its place", async () => {
    await wakeOn();
    const backend = spine();
    const real = backend.getMuster.bind(backend);
    vi.spyOn(backend, "getMuster").mockImplementation(async (id) => {
      const view = await real(id);
      for (const m of [view.arisen, view.mainPawn, ...view.hired]) if (m) m.name = null;
      return view;
    });
    press("5", {}, window);
    await until(() => text().includes("The Arisen"), "the nameless Arisen");
    expect(text()).toContain("Your pawn");
    expect(text()).toContain("A hired pawn");
  });

  it("should keep eight gear rows with empty slots kept, and the creator described, never named", async () => {
    await openMuster();
    tab("mainPawn").click();
    await until(() => sheetText().includes("Wren"), "Wren's sheet");
    expect(document.querySelectorAll("[data-gear]")).toHaveLength(8);
    expect(gearRow("subWeapon")).toBe("Sub weapon — empty");
    expect(gearRow("ring2")).toBe("Ring — empty");
    expect(sheetText()).toMatch(/Inclination\s*Calm\s*·\s*Specialization\s*Healer/);
    tab("hired1").click();
    await until(() => sheetText().includes("Bramble"), "Bramble's sheet");
    expect(sheetText()).toContain("From another Arisen's world · their Arisen is a Lv 52 Warrior");
    expect(sheetText()).toMatch(/Specialization\s*none/);
    expect(document.querySelector(".muster__sheetcol")?.classList.contains("is-cold")).toBe(true);
  });

  it("should mark the party's most on the holder and scale the bars to it", async () => {
    await openMuster();
    expect(textOf(health() ?? document.body)).toContain("most in the party");
    expect(health()?.querySelector<HTMLElement>(".gains__bar")?.style.width).toBe("100%");
    tab("hired1").click();
    await until(() => sheetText().includes("Bramble"), "Bramble");
    const magick = document.querySelector('[data-gain="magick"]');
    expect(textOf(magick ?? document.body)).toContain("most in the party");
    expect(textOf(magick ?? document.body)).toContain("Magick, plus 486 from levels, the most in the party");
  });
});

describe("THE MUSTER — the tab line (R17.8)", () => {
  it("should keep the places fixed and leave a missing member's place empty", async () => {
    await wakeOn();
    const backend = spine();
    const real = backend.getMuster.bind(backend);
    vi.spyOn(backend, "getMuster").mockImplementation(async (id) => {
      const view = await real(id);
      view.hired = view.hired.slice(0, 1);
      return view;
    });
    press("5", {}, window);
    await until(() => text().includes("Bramble"), "the party");
    const cells = [...document.querySelectorAll<HTMLElement>(".tabs > *")].map((c) => c.dataset.place);
    expect(cells).toStrictEqual(["arisen", "mainPawn", "hired1", "hired2"]);
    expect(document.querySelectorAll('[role="tab"]')).toHaveLength(3);
    expect(document.querySelector('[data-place="hired2"]')?.classList.contains("tab--empty")).toBe(true);
  });

  it("should keep Sable where Sable stood when the other hired pawn leaves", async () => {
    await openMuster();
    expect(textOf(tab("hired2"))).toContain("Sable");
    const backend = spine();
    const real = backend.getMuster.bind(backend);
    vi.spyOn(backend, "getMuster").mockImplementation(async (id) => {
      const view = await real(id);
      view.hired = view.hired.slice(1);
      return view;
    });
    backend.gameSaved();
    await until(() => !text().includes("Bramble"), "Bramble gone");
    expect(textOf(tab("hired2"))).toContain("Sable");
    expect(document.querySelector('[data-place="hired1"]')?.classList.contains("tab--empty")).toBe(true);
  });
});

describe("THE MUSTER — the keyboard (R17.27)", () => {
  it("should move through the slots without reading, and read on Enter or Space", async () => {
    await openMuster();
    await until(() => document.activeElement === slotRow("data009Slot"), "the focus on Slot 9");
    const read = vi.spyOn(spine(), "getMuster");
    for (let i = 0; i < 3; i += 1) press("ArrowDown");
    await flushPromises();
    expect(document.activeElement).toBe(slotRow("data001Slot"));
    expect(read).not.toHaveBeenCalled();
    press("Home");
    expect(document.activeElement).toBe(slotRow("data009Slot"));
    press("ArrowDown");
    press("Enter");
    await until(() => read.mock.calls.length === 1, "one read");
    expect(read.mock.calls[0]).toStrictEqual(["data008Slot"]);
    await until(() => slotRow("data008Slot").getAttribute("aria-selected") === "true", "Slot 8 picked");
    press("End");
    press(" ");
    await until(() => read.mock.calls.length === 2, "a second read");
    expect(read.mock.calls[1]).toStrictEqual(["data001Slot"]);
  });

  it("should reach Sable's gear by →, End and ↓, and step members with [ and ]", async () => {
    await openMuster();
    await until(() => document.activeElement === slotRow("data009Slot"), "the focus on Slot 9");
    press("ArrowRight");
    await until(() => document.activeElement === tab("arisen"), "the Arisen's tab");
    press("ArrowRight");
    await flushPromises();
    expect(tab("mainPawn").getAttribute("aria-selected")).toBe("true");
    await flushPromises();
    press("End");
    await flushPromises();
    expect(tab("hired2").getAttribute("aria-selected")).toBe("true");
    expect(document.activeElement).toBe(tab("hired2"));
    press("ArrowDown");
    await flushPromises();
    expect(document.activeElement?.id).toBe("muster-sheet");
    expect(sheetText()).toContain("Wand of Quiet Rain");
    press("Home", {}, tab("hired2"));
    await flushPromises();
    expect(tab("arisen").getAttribute("aria-selected")).toBe("true");
    press("]", {}, window);
    await flushPromises();
    expect(tab("mainPawn").getAttribute("aria-selected")).toBe("true");
    press("[", {}, window);
    press("[", {}, window);
    await flushPromises();
    expect(tab("hired2").getAttribute("aria-selected")).toBe("true");
  });
});

describe("THE MUSTER — THE RE-CARVING (R17.25)", () => {
  it("should mark exactly the values the save changed, carve the count, and say it once", async () => {
    await openMuster();
    spine().gameSaved();
    await until(() => text().includes("CHANGED"), "the carved count");
    expect(textAt(".muster__changed")).toBe("5 CHANGED");
    expect(textOf(tab("arisen"))).toContain("•");
    for (const place of ["mainPawn", "hired1", "hired2"]) expect(textOf(tab(place))).not.toContain("•");
    expect(sheetText()).toMatch(/Lv\s*42\s*▴1/);
    expect(sheetText()).toMatch(/1,306,900 experience\s*▴22,350/);
    expect(gearRow("ring1")).toMatch(/^Ring\s*•\s*Ring of Embers, changed at the last save, was Ring of Low Tides$/);
    const marks = [...document.querySelectorAll(".muster .rc-mark")].map((m) => textOf(m).trim()).filter(Boolean);
    expect(marks).toStrictEqual(["▴6,730", "▴1", "▴22,350", "▴22"]);
    expect(document.querySelectorAll("#muster-sheet .named__dot")).toHaveLength(1);
    expect(text()).toMatch(/· read again \d\d:\d\d/);
    expect(textAt('[aria-live="polite"].rs-sr-only')).toBe(
      "The game saved. The stone read the party again: 5 things changed.",
    );
    // Wren's sheet holds no mark
    tab("mainPawn").click();
    await flushPromises();
    const wrenMarks = [...document.querySelectorAll("#muster-sheet .rc-mark")].filter((m) => textOf(m).trim());
    expect(wrenMarks).toHaveLength(0);
  });

  it("should read the slot list again too, so the saved slot's row tells the new level and time", async () => {
    await openMuster();
    expect(textAt('[role="option"]')).toMatch(/Lv 41/);
    spine().gameSaved();
    await until(() => text().includes("CHANGED"), "the carved count");
    await until(() => /Lv 42/.test(textAt('[role="option"]') ?? ""), "the slot row's new level");
    expect(textAt('[role="option"]')).toMatch(/21:34/);
  });

  it("should keep a gap between a value and its change mark, and between the purse's labels and numbers", async () => {
    await openMuster();
    spine().gameSaved();
    await until(() => text().includes("CHANGED"), "the carved count");
    expect(textAt(".muster__purse")).toMatch(/^Gold 291,040\s*▴6,730 · Rift Crystals 6,420$/);
  });

  it("should carve nothing when the save changed nothing", async () => {
    await openMuster();
    spine().gameSaved();
    await until(() => text().includes("5 CHANGED"), "the first save's marks");
    spine().gameSaved();
    await until(() => textAt('[aria-live="polite"].rs-sr-only')?.includes("Nothing changed") === true, "silence");
    expect(document.querySelector(".muster__changed")).toBeNull();
    expect([...document.querySelectorAll(".rc-mark")].map((m) => textOf(m).trim()).filter(Boolean)).toStrictEqual([]);
    expect(document.querySelectorAll(".named__dot")).toHaveLength(0);
  });

  it("should never re-carve on a pick or a Read again, and clear the marks on another pick", async () => {
    await openMuster();
    spine().gameSaved();
    await until(() => text().includes("5 CHANGED"), "the marks");
    await pickSlot("data008Slot", "Slot 8");
    await until(() => !text().includes("CHANGED"), "the marks cleared");
    await pickSlot("data009Slot", "Lv 42");
    expect(text()).not.toContain("CHANGED");
  });

  it("should keep its ground through a re-read that failed, and carve the next one", async () => {
    // The General's review of PR #1: a failed `changed` read blanked the sheet,
    // and the next good read landed with nothing to carve against.
    await openMuster();
    const backend = spine();
    const read = backend.getMuster.bind(backend);
    let refuse = true;
    vi.spyOn(backend, "getMuster").mockImplementation(async (slotId) => {
      if (refuse) {
        refuse = false;
        throw new RiftFailure({
          kind: "save",
          detail: {
            detail: `The game is writing ${slotId} right now, or another program is holding it for a moment.`,
            reason: "beingWritten",
          },
        });
      }
      return read(slotId);
    });
    backend.gameSaved();
    await until(() => text().includes("THE GAME IS SAVING"), "the refused re-read");
    backend.gameSaved();
    await until(() => text().includes("CHANGED"), "the carved count");
    expect(textAt(".muster__changed")).toBe("5 CHANGED");
    expect(gearRow("ring1")).toMatch(/•\s*Ring of Embers, changed at the last save, was Ring of Low Tides$/);
  });

  it("should show the marks without the light under reduced motion", async () => {
    vi.spyOn(window, "matchMedia").mockImplementation(
      (query: string) =>
        ({
          matches: query.includes("reduce"),
          media: query,
          onchange: null,
          addEventListener: () => undefined,
          removeEventListener: () => undefined,
          dispatchEvent: () => false,
        }) as unknown as MediaQueryList,
    );
    await openMuster();
    let litSeen = false;
    const watcher = new MutationObserver(() => {
      if (document.querySelector(".is-lit")) litSeen = true;
    });
    watcher.observe(document.body, { subtree: true, attributes: true, childList: true });
    spine().gameSaved();
    await until(() => text().includes("5 CHANGED"), "the marks");
    await flushPromises();
    watcher.disconnect();
    expect(litSeen).toBe(false);
    expect(useMuster().lit.value).toBe(false);
    expect(gearRow("ring1")).toMatch(/•\s*Ring of Embers/);
  });

  it("should light the changed values for a frame when motion is welcome", async () => {
    await openMuster();
    // the light lasts one frame: record what it touched the moment it lands
    let lit: string[] | null = null;
    const watcher = new MutationObserver(() => {
      const now = [...document.querySelectorAll(".is-lit")].map((el) => textOf(el).trim());
      if (!lit && now.length) lit = now;
    });
    watcher.observe(document.body, { subtree: true, attributes: true, childList: true });
    spine().gameSaved();
    await until(() => lit !== null, "the light");
    await until(() => !useMuster().lit.value, "the settle");
    watcher.disconnect();
    expect(lit).toStrictEqual(["291,040", "42", "1,306,900", "Ring of Embers", "+1,262"]);
    expect(document.querySelector(".is-lit")).toBeNull();
  });
});

describe("THE MUSTER — its voice", () => {
  it("should never say we in any state it shows", async () => {
    const seen: string[] = [];
    for (const scene of ["musternosteam", "musternosaves", "musternames", "musterwriting"] as const) {
      await wakeOn(scene);
      press("5", {}, window);
      await until(() => !!document.querySelector(".muster__head + *"), scene);
      await new Promise((resolve) => setTimeout(resolve, 260));
      await flushPromises();
      seen.push(textOf(document.querySelector(".muster") ?? document.body));
      sleep();
    }
    await openMuster();
    for (const slot of ["data003Slot", "data001Slot"]) {
      slotRow(slot).click();
      await until(() => !!document.querySelector(".mstate"), slot);
      seen.push(textOf(document.querySelector(".muster") ?? document.body));
    }
    for (const page of seen) expect(page).not.toMatch(/\bwe\b|\bwe'|\bour\b|\bus\b/i);
  });
});
