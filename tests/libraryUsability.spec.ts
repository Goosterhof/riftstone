import { afterEach, expect, it, vi } from "vitest";
import { useNavigation } from "@/composables/useNavigation";
import { useSearch } from "@/composables/useSearch";
import { button, press, sleep, spine, text, typeQuery, until, wakeOn } from "./harness";

afterEach(() => {
  vi.restoreAllMocks();
  sleep();
});

it("explains an empty state filter and returns to the library", async () => {
  await wakeOn("familyfresh");
  useSearch().setStateFacet("update");
  await until(() => text().includes("NO MODS MATCH THESE FILTERS"), "an empty-filter explanation");
  expect(document.querySelector(".rift__echo")?.textContent).toContain("0 options");
  button("Clear filters").click();
  await until(() => document.querySelector('[role="tree"]') !== null, "the library restored");
});

it("clears the filters while keeping the words the player typed", async () => {
  await wakeOn();
  await typeQuery("ermine");
  useSearch().setStateFacet("update");
  await until(() => text().includes("NO MODS MATCH THESE FILTERS"), "the empty filter");
  button("Clear filters").click();
  await until(() => useSearch().results.value.length > 0, "the word's answers");
  expect(useSearch().text.value).toBe("ermine");
  expect((document.getElementById("rift-search") as HTMLInputElement).value).toBe("ermine");
});

it("lets the mouse clear a search and keeps typing focus in the well", async () => {
  await wakeOn();
  await typeQuery("ermine");
  const clear = document.querySelector<HTMLButtonElement>('[aria-label="Clear search"]');
  expect(clear).not.toBeNull();
  clear?.click();
  await until(() => document.querySelector('[role="tree"]') !== null, "the library restored");
  expect(document.activeElement?.id).toBe("rift-search");
});

it("opens the mods folder from a populated library", async () => {
  await wakeOn();
  const open = vi.spyOn(spine(), "openModsFolder");
  button("Open mods folder").click();
  await until(() => open.mock.calls.length === 1, "the folder opened");
  expect(open).toHaveBeenCalledTimes(1);
});

it("keeps tab and room shortcuts inside the screenshot dialog", async () => {
  await wakeOn();
  const nav = useNavigation();
  nav.openLightbox({ optionId: "ermine-var-1", name: "Ermine Cape" });
  await until(() => document.activeElement?.classList.contains("lightbox__x") === true, "dialog focus");
  const tab = press("Tab");
  expect(tab.defaultPrevented).toBe(true);
  expect(document.activeElement?.closest(".lightbox")).not.toBeNull();
  press("4");
  expect(nav.page.value).toBe("rift");
  press("Escape");
  await until(() => document.querySelector(".lightbox") === null, "the screenshot closed");
});
