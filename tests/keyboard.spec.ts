// Keyboard-first (§8.1): "/" and Ctrl+K reach the search well from anywhere,
// ↓ steps from the well into the answers, ↑/↓ walk them, Enter opens one,
// Esc closes what Enter opened.

import { afterEach, describe, expect, it } from "vitest";
import { useSearch } from "@/composables/useSearch";
import { useWorld } from "@/composables/useWorld";
import { press, sleep, typeQuery, until, wakeOn } from "./harness";

afterEach(() => sleep());

const searchWell = (): HTMLInputElement => {
  const input = document.getElementById("rift-search");
  if (!(input instanceof HTMLInputElement)) throw new Error("The search well is missing");
  return input;
};

const focusedIndex = (): string | null =>
  (document.activeElement as HTMLElement | null)?.closest("[data-index]")?.getAttribute("data-index") ?? null;

describe("the keyboard", () => {
  it('should focus the search well on "/" from anywhere outside a field', async () => {
    await wakeOn();
    document.body.focus();
    expect(document.activeElement).not.toBe(searchWell());
    const event = press("/", {}, document.body);
    expect(document.activeElement).toBe(searchWell());
    expect(event.defaultPrevented).toBe(true);
  });

  it('should leave "/" alone while the user types it into a field', async () => {
    await wakeOn();
    const other = document.createElement("input");
    document.body.append(other);
    other.focus();
    const event = press("/", {}, other);
    expect(document.activeElement).toBe(other);
    expect(event.defaultPrevented).toBe(false);
  });

  it("should focus the search well on Ctrl+K, even from inside another field", async () => {
    await wakeOn();
    const other = document.createElement("input");
    document.body.append(other);
    other.focus();
    press("k", { ctrlKey: true }, other);
    expect(document.activeElement).toBe(searchWell());
  });

  it("should walk the answers with the arrows, open one with Enter and close it with Esc", async () => {
    await wakeOn();
    await typeQuery("ermine");
    await until(() => useSearch().results.value.length >= 3, "three ermine answers");

    searchWell().focus();
    press("ArrowDown", {}, searchWell());
    await until(() => focusedIndex() === "0", "the first answer focused");

    press("ArrowDown");
    await until(() => focusedIndex() === "1", "the second answer focused");
    press("ArrowDown");
    await until(() => focusedIndex() === "2", "the third answer focused");
    press("ArrowUp");
    await until(() => focusedIndex() === "1", "back on the second answer");
    expect(useSearch().activeIndex.value).toBe(1);

    const second = useSearch().results.value[1];
    expect(useWorld().selectedId.value).toBeNull();
    press("Enter");
    await until(() => useWorld().detail.value?.option.id === second.optionId, "the second answer opened");
    expect(document.querySelector(`[data-option="${second.optionId}"]`)?.getAttribute("aria-selected")).toBe("true");

    press("Escape");
    await until(() => useWorld().selectedId.value === null, "the detail closed");
  });

  it("should climb from the first answer back into the search well on ↑", async () => {
    await wakeOn();
    await typeQuery("ermine");
    await until(() => useSearch().results.value.length > 0, "an answer");
    searchWell().focus();
    press("ArrowDown", {}, searchWell());
    await until(() => focusedIndex() === "0", "the first answer focused");
    press("ArrowUp");
    expect(document.activeElement).toBe(searchWell());
  });
});
