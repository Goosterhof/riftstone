// The headline path, end to end on the mock spine: search → select → plan →
// install, and THE CROSSING (§11.2): the card's rail turns from Rift cyan to
// Your World gold and NOT INSTALLED becomes INSTALLED.

import { afterEach, describe, expect, it } from "vitest";
import { button, hasButton, sleep, text, textOf, typeQuery, until, wakeOn, idle } from "./harness";

afterEach(() => sleep());

const card = (name: string): HTMLElement | null =>
  [...document.querySelectorAll<HTMLElement>("#rift-answers .card")].find((c) =>
    (c.querySelector(".card__title")?.textContent ?? "").includes(name),
  ) ?? null;

const chipsOf = (el: HTMLElement | null): string =>
  [...(el?.querySelectorAll(".card__chips > *") ?? [])].map((c) => textOf(c).trim()).join(" ");

describe("the crossing", () => {
  it("should carry a searched mod from the Rift into your world, and turn its card gold", async () => {
    await wakeOn();
    await typeQuery("lantern");
    // the card is on the shelf before the search answers: wait for its lit hit,
    // or a loaded bench reads the card from the unsearched frame
    await until(
      () => Boolean(card("Lantern Glow Recolor")?.querySelector("mark.rs-hit")),
      "the Lantern Glow card, answered",
    );

    const before = card("Lantern Glow Recolor");
    expect(before?.classList.contains("rail-world")).toBe(false);
    expect(chipsOf(before)).toContain("NOT INSTALLED");
    expect(textOf(before?.querySelector("mark.rs-hit") ?? document.createTextNode("")).toLowerCase()).toBe("lantern");

    before?.click();
    await until(() => hasButton("Install…"), "the option detail");
    button("Install…").click();
    await until(() => hasButton("Install ▶") && idle(button("Install ▶")), "the install plan");
    expect(text()).toContain("nothing is written until you press Install");

    button("Install ▶").click();
    await until(() => text().includes("Lantern Glow Recolor is in your world."), "the landed note");
    expect(text()).toMatch(/\d+ files? placed\./);

    await until(() => card("Lantern Glow Recolor")?.classList.contains("rail-world") === true, "the gold rail");
    const after = card("Lantern Glow Recolor");
    expect(after?.classList.contains("is-installed")).toBe(true);
    expect(chipsOf(after)).toContain("INSTALLED");
    expect(chipsOf(after)).not.toContain("NOT INSTALLED");
  });

  it("should plan before it writes: opening the plan installs nothing", async () => {
    await wakeOn();
    await typeQuery("lantern");
    await until(() => card("Lantern Glow Recolor") !== null, "the Lantern Glow card");
    card("Lantern Glow Recolor")?.click();
    await until(() => hasButton("Install…"), "the option detail");
    button("Install…").click();
    await until(() => hasButton("Install ▶"), "the install plan");
    await new Promise((resolve) => setTimeout(resolve, 300));
    expect(card("Lantern Glow Recolor")?.classList.contains("rail-world")).toBe(false);
    expect(text()).not.toContain("is in your world.");
  });
});
