// Keyboard-first (§8.1). "/" or Ctrl+K: search. 1–4: the rooms. L or Ctrl+L:
// the lookbook for the selected card's package. I: plan the install of the
// selected card (never installs blind). Esc: close the innermost thing.
// Inside the answers list the page handles ↑/↓, Enter and Space itself.

import { onBeforeUnmount, onMounted } from "vue";
import type { SearchResult } from "@/types/rift";
import { useNavigation, type Page } from "./useNavigation";
import { useSearch } from "./useSearch";
import { useWorld } from "./useWorld";

const ROOMS: Partial<Record<string, Page>> = { "1": "rift", "2": "world", "3": "nexus", "4": "preflight" };

function typing(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName);
}

function focusSearch(): void {
  const input = document.getElementById("rift-search");
  if (input instanceof HTMLInputElement) {
    input.focus();
    input.select();
  }
}

/** The card the keyboard is on: the active answer, else the open option. */
function currentHit(): SearchResult | null {
  const search = useSearch();
  const active = search.results.value.at(search.activeIndex.value);
  if (search.activeIndex.value >= 0 && active) return active;
  const selected = useWorld().selectedId.value;
  return search.results.value.find((h) => h.optionId === selected) ?? null;
}

function toggleLookbook(): boolean {
  const nav = useNavigation();
  if (nav.page.value !== "rift") return false;
  if (nav.riftMode.value === "lookbook") {
    nav.backToResults();
    return true;
  }
  const packageId = currentHit()?.packageId ?? useWorld().detail.value?.package.id ?? nav.packageId.value;
  if (!packageId) return false;
  nav.openLookbook(packageId);
  return true;
}

function planInstall(): boolean {
  const nav = useNavigation();
  const hit = currentHit();
  if (nav.page.value !== "rift" || !hit || hit.menuOnly) return false;
  void useWorld().beginInstall(hit.optionId);
  return true;
}

/** Your World drawn OVER the Rift (the narrow posture): it is the innermost thing. */
function worldIsOverlay(): boolean {
  const nav = useNavigation();
  return (
    useWorld().selectedId.value !== null &&
    nav.page.value === "rift" &&
    nav.riftMode.value !== "lookbook" &&
    window.matchMedia("(max-width: 1199px)").matches
  );
}

function escape(): boolean {
  const nav = useNavigation();
  const world = useWorld();
  if (nav.lightbox.value) nav.closeLightbox();
  else if (worldIsOverlay()) world.close();
  else if (nav.page.value === "rift" && nav.riftMode.value !== "results") nav.backToResults();
  else if (world.selectedId.value) world.close();
  else return false;
  return true;
}

function plainKey(event: KeyboardEvent): boolean {
  const room = ROOMS[event.key];
  if (room) {
    useNavigation().go(room);
    return true;
  }
  const key = event.key.toLowerCase();
  if (key === "l") return toggleLookbook();
  if (key === "i") return planInstall();
  if (event.key === "Escape") return escape();
  return false;
}

function wantsSearch(event: KeyboardEvent, ctrl: boolean): boolean {
  if (ctrl && event.key.toLowerCase() === "k") return true;
  return event.key === "/" && !typing(event.target);
}

export function handleHotkey(event: KeyboardEvent): boolean {
  const ctrl = event.ctrlKey || event.metaKey;
  if (wantsSearch(event, ctrl)) {
    focusSearch();
    return true;
  }
  if (ctrl && event.key.toLowerCase() === "l") return toggleLookbook();
  if (ctrl || event.altKey || typing(event.target)) return false;
  return plainKey(event);
}

function onKeydown(event: KeyboardEvent): void {
  if (event.defaultPrevented) return;
  if (handleHotkey(event)) event.preventDefault();
}

export function useHotkeys(): void {
  onMounted(() => window.addEventListener("keydown", onKeydown));
  onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown));
}
