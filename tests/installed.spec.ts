// Your World — Installed: the pak load order is "later wins", so moving the
// winner above the loser and applying the order must hand the contested files
// to the other mod — in the conflict map and in each row's ⚔ tally.

import { afterEach, describe, expect, it } from "vitest";
import { useNavigation } from "@/composables/useNavigation";
import { button, hasButton, press, sleep, textOf, until, wakeOn, idle } from "./harness";

afterEach(() => sleep());

const winners = (): string[] => [...document.querySelectorAll(".cmap__w")].map((w) => textOf(w).trim());

const row = (name: string): HTMLElement | null =>
  [...document.querySelectorAll<HTMLElement>("[aria-label]")].find((el) =>
    (el.getAttribute("aria-label") ?? "").startsWith(`${name}, position`),
  ) ?? null;

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
});
