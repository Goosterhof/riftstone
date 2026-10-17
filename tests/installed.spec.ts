// Your World — Installed: the pak load order is "later wins", so moving the
// winner above the loser and applying the order must hand the contested files
// to the other mod — in the conflict map and in each row's ⚔ tally.
// v0.3: every load-order row has an uninstall door, so a pak install whose
// package is gone (here the adopted stand-in, which never had one) still
// leaves the game; while the game runs, the engine's own sentence holds it.

import { afterEach, describe, expect, it } from "vitest";
import { useNavigation } from "@/composables/useNavigation";
import { button, hasButton, press, sleep, text, textAt, textOf, until, wakeOn, idle } from "./harness";

afterEach(() => sleep());

const winners = (): string[] => [...document.querySelectorAll(".cmap__w")].map((w) => textOf(w).trim());

const row = (name: string): HTMLElement | null =>
  [...document.querySelectorAll<HTMLElement>("[aria-label]")].find((el) =>
    (el.getAttribute("aria-label") ?? "").startsWith(`${name}, position`),
  ) ?? null;

const door = (name: string): HTMLButtonElement | null =>
  document.querySelector<HTMLButtonElement>(`.lrow [aria-label="Uninstall ${name}"]`);

const tallyOf = (name: string): string =>
  (row(name)?.querySelector(".lrow__fight")?.textContent ?? "").replace(/\s+/g, " ").trim();

describe("the load order", () => {
  it("should flip the conflict winner when the later mod is moved above the other and the order applied", async () => {
    await wakeOn();
    useNavigation().go("world");
    await until(() => winners().length === 1, "the conflict map");

    const first = winners()[0] ?? "";
    expect(["Brine Water Clarity", "Sacred Arbor Fog Removal"]).toContain(first);
    const other = first === "Brine Water Clarity" ? "Sacred Arbor Fog Removal" : "Brine Water Clarity";
    expect(tallyOf(first)).toMatch(/wins ×3/);

    const winnerRow = row(first);
    if (!winnerRow) throw new Error(`No load-order row for ${first}`);
    winnerRow.focus();
    press("ArrowUp", { altKey: true }, winnerRow);
    await until(() => hasButton("Apply order") && idle(button("Apply order")), "a dirty order");
    // Nothing is written until Apply: the map still names the old winner.
    expect(winners()).toStrictEqual([first]);

    button("Apply order").click();
    await until(() => winners()[0] === other, "the flipped winner");
    expect(winners()).toStrictEqual([other]);
    expect(tallyOf(other)).toMatch(/wins ×3/);
    expect(tallyOf(first)).not.toMatch(/wins/);
  });

  it("should uninstall a pak install that has no package through its load-order door", async () => {
    await wakeOn();
    useNavigation().go("world");
    await until(() => row("Pawn Banter Quieter") !== null, "the adopted stand-in in the load order");
    const stand = row("Pawn Banter Quieter");
    expect(textOf(stand ?? document.body)).toContain("adopted");
    expect(textOf(stand ?? document.body)).toContain("no package");

    door("Pawn Banter Quieter")?.click();
    await until(() => textAt(".home__return .home__h") !== null, "THE RETURN");
    expect(textAt(".home__return .home__h")).toBe("THE RETURN — 1 file leaves your game");
    // the plan takes the keyboard: its Uninstall is the next press
    expect(document.activeElement?.classList.contains("home__return")).toBe(true);

    button("Uninstall Pawn Banter Quieter").click();
    await until(() => row("Pawn Banter Quieter") === null, "the stand-in gone from the load order", 4000);
  });

  it("should hold a pak's door shut while the game runs, in the engine's own words", async () => {
    await wakeOn("running");
    useNavigation().go("world");
    await until(() => door("Brine Water Clarity") !== null, "the load order");
    door("Brine Water Clarity")?.click();
    await until(() => text().includes("is running and holds"), "the refusal");
    expect(text()).toContain(
      `Dragon's Dogma 2 is running and holds "Brine Water Clarity"'s paks open. Close the game, then uninstall.`,
    );
    expect(button("Uninstall Brine Water Clarity").disabled).toBe(true);
  });
});
