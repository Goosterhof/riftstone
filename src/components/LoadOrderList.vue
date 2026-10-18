<script setup lang="ts">
// §7.10 — THE LOAD ORDER, later wins. Drag a grip (pointer events: Tauri's
// file-drop handler swallows HTML5 drag-and-drop in WebView2) and a 2 px gold
// line shows where it lands. Keyboard: focus a row, Alt+↑/↓ moves it, Home/End
// sends it to the ends; every move is announced. The prefix column is the real
// file prefix; nothing touches disk until Apply order.
// v0.3: every row has an uninstall door (−), so a pak install whose package
// left the mods folder, or an adopted one that never had a package, still
// leaves your game: THE RETURN plans it from the journal, which knows every
// file. Such a row says so ("no package"), and an adopted one says "adopted".
import { computed, nextTick, ref } from "vue";
import type { InstalledRow, PakLoadState } from "@/types/rift";
import { useInstalled } from "@/composables/useInstalled";
import { pakPrefix, plural } from "@/services/format";
import { ADOPTED_TITLE, PACKAGE_GONE_LINE, missingPackageTitle } from "@/services/voice";
import KindBadge from "./KindBadge.vue";
import RiftButton from "./RiftButton.vue";

const installed = useInstalled();
const listEl = ref<HTMLElement | null>(null);
const dragId = ref<string | null>(null);
const dropAt = ref<number | null>(null);

const rows = computed(() => installed.pakRows.value);

/** ⚔ wins ×N / overridden ×N per row, from the conflict list. */
const tally = computed(() => {
  const t = new Map<string, { wins: number; lost: number }>();
  for (const c of installed.view.value?.conflicts ?? []) {
    for (const l of c.layers) {
      const entry = t.get(l.optionId) ?? { wins: 0, lost: 0 };
      if (l.optionId === c.winner.optionId) entry.wins += 1;
      else entry.lost += 1;
      t.set(l.optionId, entry);
    }
  }
  return t;
});

function prefixOf(row: InstalledRow): string {
  const pak = row.paks.at(0);
  return pak ? pakPrefix(pak.fileName) : "—";
}

const PROOF: Record<PakLoadState["kind"], { glyph: string; tone: string }> = {
  loaded: { glyph: "✓", tone: "is-world" },
  pending: { glyph: "↻", tone: "is-rift" },
  notLoaded: { glyph: "✗", tone: "is-ember" },
  unknown: { glyph: "?", tone: "is-faint" },
};

function proofText(load: PakLoadState): string {
  if (load.kind === "loaded") return `loaded · slot ${load.slot}`;
  if (load.kind === "pending") return "loads next launch";
  if (load.kind === "notLoaded") return "not loaded at last launch";
  return "no REFramework log";
}

function proofTitle(load: PakLoadState): string {
  if (load.kind === "loaded") return `REFramework injected it as ${load.nativeName}`;
  return proofText(load);
}

async function refocus(id: string): Promise<void> {
  await nextTick();
  listEl.value?.querySelector<HTMLElement>(`[data-row="${id}"]`)?.focus();
}

const ALT_STEPS: Partial<Record<string, number>> = { ArrowUp: -1, ArrowDown: 1 };

function onKey(event: KeyboardEvent, id: string): void {
  // the row's own keys: a key pressed on its door is the door's
  if (event.target !== event.currentTarget) return;
  const step = ALT_STEPS[event.key];
  if (event.altKey && step !== undefined) installed.moveBy(id, step);
  else if (event.key === "Home") installed.moveTo(id, 0);
  else if (event.key === "End") installed.moveTo(id, Number.MAX_SAFE_INTEGER);
  else return;
  event.preventDefault();
  void refocus(id);
}

function slotFor(clientY: number): number {
  const items = [...(listEl.value?.querySelectorAll<HTMLElement>("[data-row]") ?? [])];
  const index = items.findIndex((el) => {
    const box = el.getBoundingClientRect();
    return clientY < box.top + box.height / 2;
  });
  return index < 0 ? items.length : index;
}

function onPointerDown(event: PointerEvent, id: string): void {
  if (event.button !== 0) return;
  dragId.value = id;
  dropAt.value = rows.value.findIndex((r) => r.optionId === id);
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  event.preventDefault();
}

function onPointerMove(event: PointerEvent): void {
  if (dragId.value) dropAt.value = slotFor(event.clientY);
}

function onPointerUp(): void {
  const id = dragId.value;
  const at = dropAt.value;
  dragId.value = null;
  dropAt.value = null;
  if (!id || at === null) return;
  const from = rows.value.findIndex((r) => r.optionId === id);
  installed.moveTo(id, at > from ? at - 1 : at);
}
</script>

<template>
  <div class="lorder">
    <p v-if="!rows.length" class="lorder__none">
      No PAK mods in your game yet. When there are two, the later one wins.
    </p>
    <ol
      ref="listEl"
      class="lorder__list"
      aria-label="Load order, later wins"
      @pointermove="onPointerMove"
      @pointerup="onPointerUp"
    >
      <li
        v-for="(row, i) in rows"
        :key="row.optionId"
        class="lrow"
        :class="{
          'is-dragging': dragId === row.optionId,
          'drop-before': dropAt === i && dragId,
          'drop-after': dropAt === rows.length && i === rows.length - 1 && dragId,
        }"
        :data-row="row.optionId"
        tabindex="0"
        :aria-label="`${row.name}, position ${i + 1} of ${rows.length}. Alt plus arrow keys move it.`"
        @keydown="onKey($event, row.optionId)"
      >
        <span
          class="lrow__grip"
          aria-hidden="true"
          title="Drag to reorder"
          @pointerdown="onPointerDown($event, row.optionId)"
          >⋮⋮</span
        >
        <span class="lrow__pos rs-mono">{{ i + 1 }}</span>
        <span class="lrow__prefix rs-mono">{{ prefixOf(row) }}</span>
        <span class="lrow__name">{{ row.name }}</span>
        <KindBadge kind="pak" />
        <span class="lrow__files rs-mono">{{ plural(row.fileCount, "file") }}</span>
        <span v-if="tally.get(row.optionId)?.wins" class="lrow__fight is-win"
          >⚔ wins ×{{ tally.get(row.optionId)?.wins }}</span
        >
        <span v-else-if="tally.get(row.optionId)?.lost" class="lrow__fight is-lost"
          >⚔ overridden ×{{ tally.get(row.optionId)?.lost }}</span
        >
        <span class="lrow__proofs">
          <span
            v-for="pak in row.paks"
            :key="pak.fileName"
            class="lrow__proof rs-mono"
            :class="PROOF[pak.load.kind].tone"
            :title="proofTitle(pak.load)"
            >{{ PROOF[pak.load.kind].glyph }} {{ proofText(pak.load) }}</span
          >
          <!-- R16.26: provenance, not state: words and a title, no chip, no colour -->
          <span v-if="row.origin === 'adopted'" class="lrow__mark rs-mono" :title="ADOPTED_TITLE">adopted</span>
          <span v-if="row.packageMissing" class="lrow__mark lrow__gone" :title="missingPackageTitle(row)">{{
            PACKAGE_GONE_LINE
          }}</span>
        </span>
        <RiftButton
          class="lrow__door"
          size="sm"
          variant="danger"
          :aria-label="`Uninstall ${row.name}`"
          @click="installed.planReturn(row.optionId)"
          >−</RiftButton
        >
      </li>
    </ol>
    <p class="rs-sr-only" aria-live="polite">{{ installed.announcement.value }}</p>
  </div>
</template>

<style scoped>
.lorder__none {
  margin: 0;
  color: var(--rs-faint);
}
.lorder__list {
  margin: 0;
  padding: 0;
  list-style: none;
}
.lrow {
  position: relative;
  display: grid;
  grid-template-columns: 22px 18px 11ch minmax(0, 1fr) auto auto auto;
  grid-template-areas:
    "grip pos prefix name kind files door"
    "grip . . proof proof fight door";
  align-items: center;
  column-gap: var(--rs-space-2);
  min-height: 40px;
  padding: 4px var(--rs-space-3) 4px 6px;
  margin-bottom: 2px;
  background: var(--rs-hearthhi);
  border: 1px solid transparent;
}
.lrow:focus-visible {
  border-color: var(--rs-edgehi);
}
.lrow.is-dragging {
  opacity: 0.55;
}
.lrow.drop-before::before,
.lrow.drop-after::after {
  content: "";
  position: absolute;
  left: 0;
  right: 0;
  height: 2px;
  background: var(--rs-world);
}
.lrow.drop-before::before {
  top: -2px;
}
.lrow.drop-after::after {
  bottom: -2px;
}
.lrow__grip {
  grid-area: grip;
  color: var(--rs-faint);
  cursor: grab;
  user-select: none;
  touch-action: none;
  text-align: center;
}
.lrow__pos {
  grid-area: pos;
  color: var(--rs-faint);
}
.lrow__prefix {
  grid-area: prefix;
  color: var(--rs-muted);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.lrow__name {
  grid-area: name;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 500;
}
.lrow :deep(.rs-kind) {
  grid-area: kind;
}
.lrow__files {
  grid-area: files;
  color: var(--rs-faint);
  white-space: nowrap;
}
.lrow__fight {
  grid-area: fight;
  font-size: var(--rs-text-sm);
  white-space: nowrap;
  justify-self: end;
}
.lrow__fight.is-win {
  color: var(--rs-world);
}
.lrow__fight.is-lost {
  color: var(--rs-ember);
}
.lrow__proofs {
  grid-area: proof;
  display: flex;
  flex-wrap: wrap;
  gap: 2px var(--rs-space-2);
  min-width: 0;
}
.lrow__proof,
.lrow__mark {
  font-size: var(--rs-text-xs);
}
.lrow__mark {
  color: var(--rs-faint);
}
.lrow__door {
  grid-area: door;
  align-self: center;
}
/* R16.26: the door is 28 × 28, the loose rows' door */
.lrow__door.rs-btn {
  width: 28px;
  min-width: 28px;
  height: 28px;
  min-height: 28px;
  padding: 0;
}
.lrow__gone {
  font: 400 var(--rs-text-sm) var(--rs-font-sans);
}
.is-world {
  color: var(--rs-world);
}
.is-rift {
  color: var(--rs-rift);
}
.is-ember {
  color: var(--rs-ember);
}
.is-faint {
  color: var(--rs-faint);
}
</style>
