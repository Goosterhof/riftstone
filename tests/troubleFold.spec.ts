// §8.1.1 — the trouble fold (Chaos #00129 D5). The Rift's first sentence is
// creations, and its trouble is the carved line above them: `3 WON'T ANSWER`,
// ember while a package can't be opened, one press from its cards. Folded at
// every launch, remembered for the session, never a wall of cards again.

import { afterEach, describe, expect, it } from "vitest";
import { useStone } from "@/composables/useStone";
import { button, press, sleep, text, textOf, typeQuery, until, wakeOn } from "./harness";

afterEach(() => sleep());

const fold = (): HTMLElement | null => document.querySelector<HTMLElement>(".trouble-fold");
const troubleNames = (): string[] => [...document.querySelectorAll(".trouble__name")].map((n) => textOf(n).trim());

function must<T extends Element>(el: T | null, what: string): T {
  if (!el) throw new Error(`The stone carved no ${what}`);
  return el;
}

/** The first card's own `Hide it`: the last RiftButton in the card (TroubledCard.vue). */
const firstHide = (): HTMLButtonElement =>
  must(document.querySelector<HTMLButtonElement>("#rift-trouble .trouble .rs-btn:last-child"), "Hide it");

/** Hide the first card the way a keyboard user does: focus on its Hide it, then press. */
function hideFirst(): void {
  const b = firstHide();
  b.focus();
  b.click();
}

/** The card that holds focus now, by its package name. */
const focusedCard = (): string | null => {
  const card = document.activeElement?.closest(".trouble");
  return card ? textOf(must(card.querySelector(".trouble__name"), "card name")).trim() : null;
};

describe("the trouble fold (§8.1.1)", () => {
  it("should carve the trouble as one line above the first creation, folded", async () => {
    await wakeOn();
    await until(() => text().includes("3 WON'T ANSWER"), "the carved trouble line");
    expect(troubleNames()).toStrictEqual([]);
    const show = button("Show ▸");
    expect(show.getAttribute("aria-expanded")).toBe("false");
    expect(show.getAttribute("aria-controls")).toBe("rift-trouble");
    expect(show.getAttribute("aria-describedby")).toBe("rift-trouble-count");
    expect(must(document.getElementById("rift-trouble"), "cards region").hidden).toBe(true);
    // the count is an inscription, never a button label (§10)
    expect(must(document.getElementById("rift-trouble-count"), "count").tagName).toBe("P");
    // v0.3: the idle Rift is the library tree; the fold sits above it, outside its role (R16.8)
    const tree = must(document.getElementById("rift-tree"), "tree");
    expect(tree.contains(must(fold(), "fold"))).toBe(false);
    const order = must(fold(), "fold").compareDocumentPosition(tree);
    expect(Boolean(order & Node.DOCUMENT_POSITION_FOLLOWING)).toBe(true);
  });

  it("should open the cards in place on one press, outside the answers' listbox", async () => {
    await wakeOn();
    await until(() => text().includes("3 WON'T ANSWER"), "the line");
    button("Show ▸").click();
    await until(() => troubleNames().length === 3, "the cards");
    expect(troubleNames()).toStrictEqual(["Corrupted Download", "Readme Only", "Old Road Armor Pack"]);
    expect(button("Close ▾").getAttribute("aria-expanded")).toBe("true");
    expect(document.querySelectorAll("#rift-answers .trouble")).toHaveLength(0);
    const labels = [...document.querySelectorAll("#rift-trouble .trouble")].map((card) =>
      [...card.querySelectorAll(".rs-btn__label")].map((l) => textOf(l).trim()).join(" · "),
    );
    expect(labels).toStrictEqual(Array.from({ length: 3 }, () => "Show in folder · Rescan this one · Hide it"));
    // the count reads the same open or closed
    expect(text()).toContain("3 WON'T ANSWER");
  });

  it("should keep focus on the button as it opens and closes the fold", async () => {
    await wakeOn();
    await until(() => !!fold(), "the line");
    const toggle = button("Show ▸");
    toggle.focus();
    toggle.click();
    await until(() => troubleNames().length === 3, "the cards");
    expect(document.activeElement === toggle).toBe(true);
    button("Close ▾").click();
    await until(() => troubleNames().length === 0, "the cards folded");
    expect(document.activeElement === toggle).toBe(true);
  });

  it("should carve the line ember while a package can't be opened, and not once it is hidden", async () => {
    await wakeOn();
    await until(() => !!fold(), "the line");
    expect(must(fold(), "fold").classList.contains("is-ember")).toBe(true);
    button("Show ▸").click();
    await until(() => troubleNames().length === 3, "the cards");
    firstHide().click(); // Corrupted Download, the one that can't be opened
    await until(() => text().includes("2 WON'T ANSWER"), "two left");
    expect(must(fold(), "fold").classList.contains("is-ember")).toBe(false);
  });

  it("should hand focus on as cards leave, and to the library tree with the last", async () => {
    await wakeOn();
    await until(() => !!fold(), "the line");
    button("Show ▸").click();
    await until(() => troubleNames().length === 3, "the cards");
    hideFirst();
    await until(() => troubleNames().length === 2, "one gone");
    await until(() => focusedCard() === "Readme Only", "focus on the card that took its place");
    hideFirst();
    await until(() => troubleNames().length === 1, "two gone");
    await until(() => focusedCard() === "Old Road Armor Pack", "focus on the last card");
    hideFirst();
    await until(() => !fold(), "the line gone with the last card");
    await until(() => document.activeElement?.matches('#rift-tree [role="treeitem"]') === true, "focus on the tree");
    expect(document.activeElement?.getAttribute("tabindex")).toBe("0");
  });

  it("should hand focus to the card above when the last card in the list leaves", async () => {
    await wakeOn();
    await until(() => !!fold(), "the line");
    button("Show ▸").click();
    await until(() => troubleNames().length === 3, "the cards");
    const hides = document.querySelectorAll<HTMLButtonElement>("#rift-trouble .trouble .rs-btn:last-child");
    const last = must(hides.item(2), "the last card's Hide it");
    last.focus();
    last.click();
    await until(() => troubleNames().length === 2, "the last card gone");
    await until(() => focusedCard() === "Readme Only", "focus on the card above");
    expect(troubleNames()).toStrictEqual(["Corrupted Download", "Readme Only"]);
  });

  it("should close the fold when the last card leaves, so the next trouble arrives folded", async () => {
    await wakeOn();
    await until(() => !!fold(), "the line");
    button("Show ▸").click();
    await until(() => troubleNames().length === 3, "the cards");
    for (let left = 2; left >= 0; left -= 1) {
      firstHide().click();
      await until(() => troubleNames().length === left, `${left} left`);
    }
    await until(() => !fold(), "the line gone");
    expect(useStone().troubleOpen.value).toBe(false);
  });

  it("should keep the trouble off a standing query and remember the fold when it clears", async () => {
    await wakeOn();
    await until(() => !!fold(), "the line");
    button("Show ▸").click();
    await until(() => troubleNames().length === 3, "the cards");
    await typeQuery("cloak");
    expect(fold()).toBeNull();
    expect(troubleNames()).toStrictEqual([]);
    await typeQuery("");
    await until(() => !!fold(), "the line back");
    expect(button("Close ▾").getAttribute("aria-expanded")).toBe("true");
    expect(troubleNames()).toHaveLength(3);
  });

  it("should start folded at every launch", async () => {
    await wakeOn();
    await until(() => !!fold(), "the line");
    button("Show ▸").click();
    await until(() => troubleNames().length === 3, "the cards");
    sleep();
    await wakeOn();
    await until(() => !!fold(), "the line again");
    expect(button("Show ▸").getAttribute("aria-expanded")).toBe("false");
    expect(troubleNames()).toStrictEqual([]);
  });

  it("should leave the cards open with no button when the library holds no creation", async () => {
    await wakeOn("nocreations");
    await until(() => troubleNames().length === 3, "the cards, open");
    // the fold itself carries no door (THE UNDERCROFT's own `Show ▸` sits at the tree's foot, R18.11)
    const doors = [...(fold()?.querySelectorAll("button") ?? [])].map((b) => textOf(b).trim());
    expect(doors).not.toContain("Show ▸");
    expect(doors).not.toContain("Close ▾");
    expect(text()).toContain("3 WON'T ANSWER");
  });

  it("should still send ↓ from the well to the tree's first row, past the fold", async () => {
    await wakeOn();
    await until(() => !!fold() && !!document.querySelector('#rift-tree [role="treeitem"]'), "the line and the tree");
    must(document.getElementById("rift-search"), "search well").focus();
    press("ArrowDown");
    await until(
      () => document.activeElement?.matches('#rift-tree [role="treeitem"][aria-posinset="1"]') === true,
      "the tree's first row",
    );
    expect(document.activeElement?.closest(".trouble-fold")).toBeNull();
  });
});
