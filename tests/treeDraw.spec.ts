// §16.7 — the first open of a package this session draws its rails in; a
// re-open never replays it. Its own file: the session's first-open memory is
// the tree module's, and tree.spec's Expand all marks every package as seen.

import { afterEach, describe, expect, it, vi } from "vitest";
import { sleep, textOf, until, wakeOn } from "./harness";

afterEach(() => {
  vi.restoreAllMocks();
  sleep();
});

const topRows = (): HTMLElement[] => [
  ...document.querySelectorAll<HTMLElement>('#rift-tree [role="treeitem"][aria-level="1"]'),
];

function pkgRow(label: string): HTMLElement {
  const found = topRows().find((r) => textOf(r.querySelector(".ltr__label") ?? r).trim() === label);
  if (!found) throw new Error(`no package row ${label}`);
  return found;
}

const lineOf = (item: HTMLElement): HTMLElement => {
  const line = item.querySelector<HTMLElement>(":scope > .ltr__row");
  if (!line) throw new Error("no row line");
  return line;
};

describe("the library tree's first open (§16.7)", () => {
  it("should draw a package's rails in the first time it opens this session, and never again on a re-open", async () => {
    await wakeOn();
    await until(() => topRows().length > 0, "the library tree");
    const wardrobe = pkgRow("Vernworth Wardrobe Collection");
    const kids = (): number => wardrobe.querySelectorAll('[role="group"] .ltr__row').length;
    const drawnKids = (): number => wardrobe.querySelectorAll('[role="group"] .ltr__row.is-drawn').length;

    lineOf(wardrobe).click();
    await until(() => kids() > 0, "Vernworth open");
    expect(drawnKids()).toBe(kids());

    await new Promise((resolve) => setTimeout(resolve, 700));
    expect(drawnKids()).toBe(0);
    lineOf(wardrobe).click();
    await until(() => kids() === 0, "Vernworth closed");
    lineOf(wardrobe).click();
    await until(() => kids() > 0, "Vernworth open again");
    expect(drawnKids()).toBe(0);
  });
});
