<script lang="ts">
/** Packages whose first open this session already drew its rails (§16.7). */
const seenOpen = new Set<string>();
</script>

<script setup lang="ts">
// §16.1 — THE LIBRARY TREE, the Rift's idle view: every package of the mods
// folder as a collapsed slab, its options nested the way their menus nest
// them. The WAI-ARIA tree (R16.6): one tab stop with a roving tabindex,
// ↑ ↓ ← → Home End, Enter opens an option in Your World or toggles a branch,
// Space zooms an option's screenshot (else acts as Enter), * opens every
// sibling branch (WINDOW-CONTRACT § v0.3 "The library tree"). No type-ahead:
// EVERY letter typed here goes to the stone's well and becomes a search
// (Bold Choice 10) — `l` and `i` too, or "lantern" would open a lookbook and
// "iron" plan an install. Expanding never fires the seam.
import { computed, nextTick, onMounted, ref } from "vue";
import { useLibrary, type LibraryRow } from "@/composables/useLibrary";
import { useSearch } from "@/composables/useSearch";
import { useStone } from "@/composables/useStone";
import { useWorld } from "@/composables/useWorld";
import { useNavigation } from "@/composables/useNavigation";
import { useVersions } from "@/composables/useVersions";
import { troubleProblem } from "@/services/voice";
import LibraryBranch from "./LibraryBranch.vue";

const library = useLibrary();
const search = useSearch();
const stone = useStone();
const world = useWorld();
const nav = useNavigation();
const versions = useVersions();

const root = ref<HTMLElement | null>(null);
const drawn = ref<ReadonlySet<string>>(new Set());
/** Longer than --rs-dur-quick: the rails' first draw is over by then. */
const DRAWN_FOR_MS = 600;

/** Visible rows by parent key; the top level under "". */
const kids = computed(() => {
  const map = new Map<string, LibraryRow[]>();
  for (const row of library.rows.value) {
    const at = row.parent ?? "";
    const list = map.get(at);
    if (list) list.push(row);
    else map.set(at, [row]);
  }
  return map;
});
const top = computed(() => kids.value.get("") ?? []);

/** The roving tab stop: the focused row while it is visible, else the first. */
const stop = computed(() => library.focusedRow.value?.key ?? library.rows.value[0]?.key ?? null);

function rowEl(key: string): HTMLElement | null {
  return root.value?.querySelector<HTMLElement>(`[data-key="${CSS.escape(key)}"]`) ?? null;
}

async function focusKey(key: string | null, scroll = true): Promise<void> {
  if (!key) return;
  library.focusedKey.value = key;
  await nextTick();
  const el = rowEl(key);
  el?.focus({ preventScroll: true });
  if (scroll) el?.scrollIntoView?.({ block: "nearest" });
}

/** Opening a package by hand: the first time this session its rails draw in. */
function opened(row: LibraryRow): void {
  if (row.kind !== "package" || seenOpen.has(row.key)) return;
  seenOpen.add(row.key);
  drawn.value = new Set([...drawn.value, row.key]);
  // §16.7: the light runs once. Past the draw (--rs-dur-quick) the package
  // leaves `drawn`, so a later re-open finds its rails already lit.
  const key = row.key;
  setTimeout(() => {
    drawn.value = new Set([...drawn.value].filter((k) => k !== key));
  }, DRAWN_FOR_MS);
}

function setOpen(row: LibraryRow, open: boolean): void {
  if (!row.branch || row.expanded === open) return;
  if (open) {
    library.expand(row.key);
    opened(row);
  } else library.collapse(row.key);
}

/** A troubled package with nothing to open: its card in the trouble fold. */
async function toTrouble(row: LibraryRow): Promise<void> {
  const index = stone.troubled.value.findIndex((p) => p.id === row.packageId);
  if (index < 0) return;
  if (!stone.troubleOpen.value) stone.toggleTrouble();
  await nextTick();
  document.querySelector<HTMLElement>(`#rift-trouble [data-trouble-index="${index}"] button`)?.focus();
}

function activate(row: LibraryRow): void {
  // R18.18.1: the question row opens the question card in Your World (R18.4's door form)
  if (row.kind === "question") {
    versions.openQuestion(row.packageId);
    return;
  }
  // R18.4: a version row opens THE VERSIONS card in Your World and expands, as a leaf with children does
  if (row.kind === "version") {
    versions.open(row.packageId);
    setOpen(row, true);
    return;
  }
  if (row.optionId) {
    void world.open(row.optionId);
    setOpen(row, true);
    return;
  }
  if (row.branch) {
    setOpen(row, !row.expanded);
    return;
  }
  if (troubleProblem(row.pkg.package)) void toTrouble(row);
}

function press(row: LibraryRow): void {
  library.focusedKey.value = row.key;
  activate(row);
}

function chevron(row: LibraryRow): void {
  library.focusedKey.value = row.key;
  setOpen(row, !row.expanded);
}

/** The only printable keys the tree lets through to the window's hotkeys: the
 *  rooms and "/" (search). Every letter starts a word, so a letter is never a
 *  hotkey here: the lookbook stays on Ctrl+L, and Install… on the open option. */
const PASS_THROUGH = new Set(["1", "2", "3", "4", "/"]);

async function typeIntoWell(char: string): Promise<void> {
  const input = document.getElementById("rift-search");
  search.setText(search.text.value + char);
  if (!(input instanceof HTMLInputElement)) return;
  input.focus();
  await nextTick();
  const end = input.value.length;
  input.setSelectionRange(end, end);
}

type Move = (i: number, rows: LibraryRow[]) => void;

const MOVES: Partial<Record<string, Move>> = {
  ArrowDown: (i, rows) => void focusKey(rows[Math.min(rows.length - 1, i + 1)]?.key ?? null),
  ArrowUp: (i, rows) =>
    i <= 0 ? document.getElementById("rift-search")?.focus() : void focusKey(rows[i - 1]?.key ?? null),
  Home: (_i, rows) => void focusKey(rows[0]?.key ?? null),
  End: (_i, rows) => void focusKey(rows.at(-1)?.key ?? null),
  ArrowRight: (i, rows) => {
    const row = rows[i];
    if (!row?.branch) return;
    if (!row.expanded) setOpen(row, true);
    else void focusKey(rows[i + 1]?.key ?? null);
  },
  ArrowLeft: (i, rows) => {
    const row = rows[i];
    if (!row) return;
    if (row.branch && row.expanded) setOpen(row, false);
    else void focusKey(row.parent);
  },
  Enter: (i, rows) => {
    const row = rows[i];
    if (row) activate(row);
  },
  " ": (i, rows) => {
    const row = rows[i];
    if (!row) return;
    if (row.optionId && row.node?.hasScreenshot) nav.openLightbox({ optionId: row.optionId, name: row.label });
    else activate(row);
  },
  "*": (i, rows) => {
    const row = rows[i];
    if (!row) return;
    for (const sibling of rows.filter((r) => r.parent === row.parent)) setOpen(sibling, true);
  },
};

function onKey(event: KeyboardEvent): void {
  if (event.ctrlKey || event.metaKey || event.altKey) return;
  const rows = library.rows.value;
  const move = MOVES[event.key];
  if (move) {
    event.preventDefault();
    move(
      Math.max(
        0,
        rows.findIndex((r) => r.key === stop.value),
      ),
      rows,
    );
    return;
  }
  if (event.key.length === 1 && !PASS_THROUGH.has(event.key.toLowerCase())) {
    event.preventDefault();
    void typeIntoWell(event.key);
  }
}

/** Track the focus a click or a Tab put on a row, so the tab stop follows it. */
function onFocusIn(event: FocusEvent): void {
  const el = event.target instanceof Element ? event.target.closest<HTMLElement>("[data-key]") : null;
  if (el?.dataset.key) library.focusedKey.value = el.dataset.key;
}

/** Expand all opens every branch at once: no light runs (§16.7). */
function expandAll(): void {
  for (const pkg of library.packages.value) seenOpen.add(pkg.package.id);
  drawn.value = new Set();
  library.expandAll();
}

onMounted(async () => {
  // back from the answers: the option the player opened there was revealed
  // (RiftPage.open), so it is visible, selected and in view (R16.7)
  const selected = world.selectedId.value;
  if (selected && library.rows.value.some((r) => r.key === selected)) library.focusedKey.value = selected;
  await nextTick();
  if (stop.value) rowEl(stop.value)?.scrollIntoView?.({ block: "nearest" });
});

defineExpose({ focusFirst: () => focusKey(stop.value), expandAll });
</script>

<template>
  <ul id="rift-tree" ref="root" class="ltr" role="tree" aria-label="Library" @keydown="onKey" @focusin="onFocusIn">
    <LibraryBranch
      :items="top"
      :kids="kids"
      :stop="stop"
      :selected-id="world.selectedId.value"
      :drawn="drawn"
      @press="press"
      @chevron="chevron"
    />
  </ul>
</template>

<style scoped>
.ltr {
  margin: 0;
  padding: var(--rs-space-4) var(--rs-space-6) var(--rs-space-12);
  list-style: none;
  min-width: 0;
  overflow-x: hidden;
}
.ltr.is-after-fold {
  padding-top: var(--rs-space-2);
}
</style>
