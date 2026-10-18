// v0.3 PLAY (WINDOW-CONTRACT § v0.3 "Play", design system #00015 §16.2): a
// Primary button carved into the stone that starts Dragon's Dogma 2 through
// Steam (or DD2.exe when the game was picked by hand). Warnings never block:
// they turn the slab into a confirm. A refusal is a sentence under a button
// that stays focusable. While the game runs the stone says THE WORLD IS AWAKE,
// and the engine's own rules bite: the load order and the REFramework
// switches wait, in the engine's own words, until the game closes.

import { afterEach, describe, expect, it, vi } from "vitest";
import { nextTick } from "vue";
import { useLaunch } from "@/composables/useLaunch";
import { useNavigation } from "@/composables/useNavigation";
import { useStone } from "@/composables/useStone";
import { useToasts } from "@/composables/useToasts";
import { button, hasButton, idle, press, sleep, spine, text, textAt, textOf, until, wakeOn } from "./harness";

afterEach(() => {
  vi.restoreAllMocks();
  sleep();
});

const play = (): HTMLButtonElement | null => document.querySelector<HTMLButtonElement>(".play__btn");
/** THE WORLD IS AWAKE, carved where Play stood. */
const awake = (): Element | null => document.querySelector(".play__carved--awake");
const confirm = (): HTMLElement | null =>
  document.querySelector<HTMLElement>('.play [role="group"][aria-label="Before you play"]');
const warnings = (): string[] =>
  [...document.querySelectorAll(".play__warn")].map((w) =>
    textOf(w)
      .replace(/^\s*⚠\s*/, "")
      .trim(),
  );
const toastLines = (): string[] => useToasts().toasts.value.map((t) => t.line);
const toastOf = (line: string): string | undefined => useToasts().toasts.value.find((t) => t.line === line)?.tone;

/** Sacred Arbor Fog Removal's row in THE LOAD ORDER. */
const sacred = (): HTMLElement | undefined =>
  [...document.querySelectorAll<HTMLElement>(".lrow")].find((r) =>
    (r.getAttribute("aria-label") ?? "").startsWith("Sacred Arbor Fog Removal, position"),
  );

/** The start time the slab carves: the window's local clock, HH:mm. */
function since(): string {
  const ms = useStone().overview.value?.game?.startedMs;
  if (typeof ms !== "number") throw new Error("The game has no start time");
  return `since ${new Date(ms).toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit", hour12: false })}`;
}

const RUNNING_REORDER =
  "The stone held back: Dragon's Dogma 2 is running and holds its paks open. Close the game, then change the load order. Nothing was changed. Close Dragon's Dogma 2, then try again.";
const RUNNING_SWITCH =
  "The stone held back: Dragon's Dogma 2 is running, and REFramework rewrites re2_fw_config.txt when the game closes, so the change would be lost. Close the game, then flip the switch. Nothing was changed. Close Dragon's Dogma 2, then try again.";
const PAK_FOLDER_OFF = "Your PAK mods won't load: REFramework's Alternate PAK Folder is off.";

describe("Play on the stone", () => {
  it("should offer Play with a game, read its plan at once, and bind no key that starts it", async () => {
    await wakeOn();
    await until(() => play() !== null && useLaunch().plan.value !== null, "Play and its plan");
    expect(useLaunch().plan.value).toStrictEqual({ launch: "steam", warnings: [], refused: null });
    const ready = play();
    expect(textOf(ready ?? document.body).trim()).toBe("Play");
    expect(ready?.getAttribute("aria-disabled")).toBeNull();
    expect(ready?.querySelector("kbd")).toBeNull();

    const launched = vi.spyOn(spine(), "launchGame");
    for (const key of ["p", "P", "Enter", " ", "g", "s"]) press(key, {}, document.body);
    press("p", { ctrlKey: true }, document.body);
    press("Enter", { altKey: true }, document.body);
    await new Promise((resolve) => setTimeout(resolve, 80));
    expect(launched).not.toHaveBeenCalled();
    expect(useLaunch().launching.value).toBe(false);
  });

  it("should start the game when nothing warns: launching, then THE WORLD STIRS, then THE WORLD IS AWAKE since its start", async () => {
    await wakeOn();
    await until(() => play() !== null, "Play");
    const launched = vi.spyOn(spine(), "launchGame");
    play()?.click();
    await until(() => play()?.getAttribute("aria-busy") === "true", "the launching press");
    expect(textAt(".play__dots")).toBe("···");

    await until(() => text().includes("THE WORLD STIRS"), "the world stirring");
    expect(textAt(".play__note")).toBe("Steam is starting the game.");
    // a start that never comes leaves no dead stone: Play stays under the line
    expect(play()?.getAttribute("aria-disabled")).toBeNull();

    await until(() => text().includes("THE WORLD IS AWAKE"), "the game running");
    expect(play()).toBeNull();
    expect(textAt(".play__since")).toBe(since());
    expect(launched).toHaveBeenCalledTimes(1);
    // the slab says it: no toast repeats THE WORLD STIRS, and no gold is spent on a press
    expect(toastOf("Dragon's Dogma 2 is starting through Steam.")).toBeUndefined();
    expect(useLaunch().launching.value).toBe(false);
  });

  it("should start DD2.exe from its folder once the game folder was picked by hand", async () => {
    await wakeOn();
    useNavigation().go("preflight");
    await until(() => hasButton("Change…"), "the game install row");
    button("Change…").click();
    await until(() => useStone().overview.value?.game?.launch === "exe", "the hand-picked game");
    play()?.click();
    await until(() => text().includes("THE WORLD STIRS"), "the world stirring");
    expect(textAt(".play__note")).toBe("The game is starting.");
    expect(toastOf("Dragon's Dogma 2 is starting from its game folder.")).toBeUndefined();
    await until(() => text().includes("THE WORLD IS AWAKE"), "the game running");
  });

  it("should ask before playing past a warning: Esc keeps the game asleep, Play anyway starts it", async () => {
    await wakeOn("refoff");
    await until(() => play() !== null, "Play");
    const launched = vi.spyOn(spine(), "launchGame");
    play()?.click();
    await until(() => confirm() !== null, "the confirm on the stone");
    expect(warnings()).toStrictEqual([PAK_FOLDER_OFF]);
    expect(document.activeElement).toBe(button("Play anyway"));
    expect(hasButton("Fix it")).toBe(true);
    expect(launched).not.toHaveBeenCalled();

    press("Escape");
    await until(() => confirm() === null && play() !== null, "Play again");
    expect(document.activeElement).toBe(play());
    expect(launched).not.toHaveBeenCalled();

    play()?.click();
    await until(() => confirm() !== null, "the confirm again");
    button("Play anyway").click();
    await until(() => text().includes("THE WORLD IS AWAKE"), "the game running");
    expect(launched).toHaveBeenCalledTimes(1);
    // the warning was the confirm's to say: no toast says it again after Play anyway
    expect(useToasts().toasts.value.some((t) => t.line.includes(PAK_FOLDER_OFF))).toBe(false);
  });

  it("should send Fix it to PREFLIGHT, onto the row of the first warning, with the keyboard on its recovery", async () => {
    await wakeOn("refoff");
    await until(() => play() !== null, "Play");
    const scrolled = vi.spyOn(Element.prototype, "scrollIntoView");
    play()?.click();
    await until(() => confirm() !== null, "the confirm on the stone");
    button("Fix it").focus();
    button("Fix it").click();
    await until(() => useNavigation().page.value === "preflight", "PREFLIGHT");
    const target = (): HTMLElement | undefined =>
      scrolled.mock.contexts
        .filter((c): c is HTMLElement => c instanceof HTMLElement)
        .find((el) => el.dataset.check === "pakdir");
    await until(() => target() !== undefined, "the Alternate PAK Folder row scrolled to");
    expect(textOf(target() ?? document.body)).toContain("Alternate PAK Folder");
    // the focus never drops to <body>: it lands on the row's own way out
    await until(() => document.activeElement === button("Turn it on"), "the focus on Turn it on");
    expect(document.activeElement?.closest("[data-check]")?.getAttribute("data-check")).toBe("pakdir");
    expect(confirm()).toBeNull();
    expect(useLaunch().launching.value).toBe(false);
  });

  it("should answer a second press while the world stirs: Steam is asked again, so a start that never comes leaves no dead stone", async () => {
    // R16.13: Play stays pressable under THE WORLD STIRS
    await wakeOn();
    await until(() => play() !== null && useLaunch().plan.value !== null, "Play");
    // Steam is asked, and the game never comes
    const launched = vi.spyOn(spine(), "launchGame").mockImplementation(async () => {
      await new Promise((resolve) => setTimeout(resolve, 30));
      return { launch: "steam", launchedMs: Date.now() };
    });
    play()?.click();
    await until(() => text().includes("THE WORLD STIRS") && play()?.getAttribute("aria-busy") !== "true", "stirring");
    expect(launched).toHaveBeenCalledTimes(1);
    expect(play()?.getAttribute("aria-disabled")).toBeNull();

    play()?.click();
    await until(() => launched.mock.calls.length === 2, "Steam asked again");
    await until(
      () => text().includes("THE WORLD STIRS") && play()?.getAttribute("aria-busy") !== "true",
      "stirring again",
    );
    expect(toastLines().filter((l) => l.startsWith("Dragon's Dogma 2 is starting"))).toHaveLength(0);
    expect(useLaunch().launching.value).toBe(true);
    expect(text()).not.toContain("THE WORLD IS AWAKE");
  });

  it("should keep the keyboard on the slab as the game starts and stops: the carved line, then Play", async () => {
    await wakeOn();
    await until(() => play() !== null && useLaunch().plan.value !== null, "Play");
    play()?.focus();
    play()?.click();
    await until(() => text().includes("THE WORLD IS AWAKE"), "the game running");
    // Play left under the focus: the carved line takes it, never <body>
    await until(() => document.activeElement === awake(), "the focus on THE WORLD IS AWAKE");
    expect(awake()?.getAttribute("tabindex")).toBe("-1");

    spine().closeGame();
    await until(() => play() !== null && document.activeElement === play(), "the focus back on Play");
  });

  it("should refuse Play without a game: focusable, its sentence linked, and a press starts nothing", async () => {
    await wakeOn("nogame");
    await until(() => play()?.getAttribute("aria-disabled") === "true", "the refused Play");
    const refused = "Dragon's Dogma 2 was not found, so pick the game folder before you play.";
    expect(textAt("#play-refused")).toBe(refused);
    expect(play()?.getAttribute("aria-describedby")).toBe("play-refused");
    expect(play()?.disabled).toBe(false);
    play()?.focus();
    expect(document.activeElement).toBe(play());

    const launched = vi.spyOn(spine(), "launchGame");
    play()?.click();
    await new Promise((resolve) => setTimeout(resolve, 120));
    expect(launched).not.toHaveBeenCalled();
    expect(text()).not.toContain("THE WORLD STIRS");
  });
});

describe("while the world is awake", () => {
  it("should never say the game is already running once it has stopped, not even before the plan is read again", async () => {
    await wakeOn("running");
    await until(() => text().includes("THE WORLD IS AWAKE"), "the running game");
    // the plan on hand was read while the game ran: it refuses for that run
    await until(() => useLaunch().plan.value?.refused?.includes("already running") === true, "the running plan");
    // every plan read the wake started has landed (the mock answers from the state at its answer)
    await new Promise((resolve) => setTimeout(resolve, 150));
    expect(useLaunch().plan.value?.refused).toContain("already running");
    // the next plan takes its time to arrive
    const original = spine().planLaunch.bind(spine());
    vi.spyOn(spine(), "planLaunch").mockImplementation(async () => {
      await new Promise((resolve) => setTimeout(resolve, 400));
      return original();
    });

    spine().closeGame();
    await until(() => useStone().overview.value?.game?.running === false, "the overview: the game stopped");
    await nextTick();
    // the moment the stone knows the game stopped, Play is Ready: no stale sentence is shown or announced
    expect(play()).not.toBeNull();
    expect(play()?.getAttribute("aria-disabled")).toBeNull();
    expect(textAt("#play-refused")).toBeNull();
    expect(text()).not.toContain("already running");
    await until(() => useLaunch().plan.value?.refused === null, "the fresh plan");
    expect(textAt("#play-refused")).toBeNull();
  });

  it("should refuse a reorder and a switch in the engine's own words, and lift both rules when the game stops", async () => {
    await wakeOn("running,refoff");
    await until(() => text().includes("THE WORLD IS AWAKE"), "the running game");
    expect(play()).toBeNull();
    expect(textAt(".play__since")).toBe(since());

    // the load order waits for the game to close
    useNavigation().go("world");
    await until(() => sacred() !== undefined, "the load order");
    const reorder = vi.spyOn(spine(), "reorderPaks");
    sacred()?.focus();
    press("ArrowUp", { altKey: true });
    await until(() => hasButton("Apply order") && idle(button("Apply order")), "a changed order");
    button("Apply order").click();
    await until(() => toastLines().includes(RUNNING_REORDER), "the refused reorder");
    expect(toastOf(RUNNING_REORDER)).toBe("ember");
    expect(reorder).toHaveBeenCalledTimes(1);

    // so does the REFramework switch
    useNavigation().go("preflight");
    await until(() => hasButton("Turn it on"), "the switch row");
    button("Turn it on").click();
    await until(() => toastLines().includes(RUNNING_SWITCH), "the refused switch");
    expect(toastOf(RUNNING_SWITCH)).toBe("ember");

    // the watcher sees the game stop: Play is back, and the order lands
    spine().closeGame();
    await until(() => play() !== null && !text().includes("THE WORLD IS AWAKE"), "Play back");
    // the plan read while the game ran is void once it stops, and is read again: Play is Ready, not refused
    await until(() => play()?.getAttribute("aria-disabled") === null, "Play ready");
    await until(() => !useLaunch().planStale.value, "the plan read again");
    // the switch stayed off while the game ran, so its warning rides along
    expect(useLaunch().plan.value).toStrictEqual({
      launch: "steam",
      warnings: [{ kind: "pakFolderOff", paks: 3 }],
      refused: null,
    });
    expect(textAt("#play-refused")).toBeNull();
    useNavigation().go("world");
    await until(() => hasButton("Apply order") && idle(button("Apply order")), "the order still waiting");
    button("Apply order").click();
    await until(
      () => toastLines().includes("The load order is set. Later still wins; the prefixes were rewritten."),
      "the order set",
    );
    expect(reorder).toHaveBeenCalledTimes(2);
  });
});
