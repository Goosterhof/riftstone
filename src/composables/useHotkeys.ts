// Keyboard-first (§8.1). "/" or Ctrl+K: search. 1–5: the rooms (5, v0.4: THE MUSTER). L or Ctrl+L:
// the lookbook for the selected card's package. I: plan the install of the
// selected card (never installs blind). Esc: close the innermost thing.
// Inside the answers list the page handles ↑/↓, Enter and Space itself.
// v0.3: with the library tree on screen (the Rift idle), I and L read the
// tree's focused row, never a stale answer index from an earlier search.

import { onBeforeUnmount, onMounted } from "vue";
import { useNavigation, type Page } from "./useNavigation";
import { useSearch } from "./useSearch";
import { useWorld } from "./useWorld";
import { useLibrary } from "./useLibrary";
import { useMuster } from "./useMuster";
import { useRooms } from "./useRooms";

const ROOMS: Partial<Record<string, Page>> = {
  "1": "rift",
  "2": "world",
  "3": "nexus",
  "4": "preflight",
  "5": "muster",
};

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

/** What the keyboard is on: an option (null for a package or bundle row),
 *  its package, and whether it is menu-only. */
interface KeyTarget {
  optionId: string | null;
  packageId: string;
  menuOnly: boolean;
}

/** The tree's focused row while the tree is on screen, else the open option's. */
function treeTarget(): KeyTarget | null {
  const row = useLibrary().focusedRow.value;
  if (row) return { optionId: row.optionId, packageId: row.packageId, menuOnly: row.node?.menuOnly ?? false };
  const detail = useWorld().detail.value;
  return detail ? { optionId: detail.option.id, packageId: detail.package.id, menuOnly: detail.option.menuOnly } : null;
}

/** The card the keyboard is on: the active answer, else the open option. */
function currentTarget(): KeyTarget | null {
  const search = useSearch();
  if (search.idle.value) return treeTarget();
  const active = search.results.value.at(search.activeIndex.value);
  const selected = useWorld().selectedId.value;
  const hit =
    search.activeIndex.value >= 0 && active ? active : search.results.value.find((h) => h.optionId === selected);
  return hit ? { optionId: hit.optionId, packageId: hit.packageId, menuOnly: hit.menuOnly } : null;
}

function toggleLookbook(): boolean {
  const nav = useNavigation();
  if (nav.page.value !== "rift") return false;
  if (nav.riftMode.value === "lookbook") {
    nav.backToResults();
    return true;
  }
  const packageId = currentTarget()?.packageId ?? useWorld().detail.value?.package.id ?? nav.packageId.value;
  if (!packageId) return false;
  nav.openLookbook(packageId);
  return true;
}

function planInstall(): boolean {
  const nav = useNavigation();
  const hit = currentTarget();
  if (nav.page.value !== "rift" || !hit?.optionId || hit.menuOnly) return false;
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

/** Esc closes the innermost thing ON SCREEN. Your World's pane is drawn on THE
 *  RIFT only: elsewhere its selection is not a thing on screen, so Esc leaves
 *  it alone and goes to the page's own Esc (a mod's files returns, R16.21). */
function escape(): boolean {
  const nav = useNavigation();
  const world = useWorld();
  const inRift = nav.page.value === "rift";
  if (nav.lightbox.value) nav.closeLightbox();
  else if (worldIsOverlay()) world.close();
  else if (inRift && nav.riftMode.value !== "results") nav.backToResults();
  else if (inRift && world.selectedId.value) world.close();
  else return false;
  return true;
}

function plainKey(event: KeyboardEvent): boolean {
  const room = ROOMS[event.key] ?? useRooms().extra.value.find((r) => r.key === event.key)?.id;
  if (room) {
    // R17.27: entering THE MUSTER by its key puts the focus on the picked slot
    if (room === "muster") useMuster().summon();
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
