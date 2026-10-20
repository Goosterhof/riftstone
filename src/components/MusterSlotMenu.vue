<script setup lang="ts">
// v0.5 — the slot menu (#00015 R18.20–R18.22, R18.24): the head's open-slot
// line IS the button, fused, never a second control. The WAI-ARIA select-only
// combobox: the focus stays on the button and the active entry is told by
// aria-activedescendant. Moving through the menu reads nothing; Enter picks.
// No type-ahead: the digits are the room keys, so they close the menu and go
// through. The popup has a frame and no shadow (§6.3's shadows are closed at
// three) and no motion: a control appears where it is asked for (R18.31).
import { computed, onBeforeUnmount, ref, watch } from "vue";
import type { MusterSlot } from "@/types/rift";
import type { MusterCharacter } from "@/composables/useMuster";
import {
  NEWER_SAVE_NOTE,
  NEWEST_BADGE,
  WRITING_ENTRY,
  personName,
  slotButtonLabel,
  slotEntryLabel,
  whenWords,
} from "@/services/muster";

const { who, shownSlot, shownAt, newerIds, lit } = defineProps<{
  who: MusterCharacter;
  /** The slot on the sheet. */
  shownSlot: MusterSlot;
  /** When the slot on the sheet was saved (the read's own time, else the listing's). */
  shownAt: string;
  /** Pinned: the slots saved since the pin (R18.24). */
  newerIds: ReadonlySet<string>;
  /** R18.25 step 3: the slot number in the re-carving light, for one frame. */
  lit: boolean;
}>();

const emit = defineEmits<{ pick: [slotId: string] }>();

const MENU_ID = "muster-slotmenu";
const optionId = (slot: MusterSlot): string => `muster-slotopt-${slot.slotId}`;

const root = ref<HTMLElement | null>(null);
const button = ref<HTMLElement | null>(null);
const open = ref(false);
const active = ref(0);
/** Q18.8's fallback: the popup never runs past the window's bottom − 16 px. */
const maxHeight = ref<string | undefined>(undefined);

const entries = computed(() => who.slots);
const shownIndex = computed(() =>
  Math.max(
    0,
    entries.value.findIndex((s) => s.slotId === shownSlot.slotId),
  ),
);
const newer = computed(() => entries.value.some((s) => newerIds.has(s.slotId)));
const arisen = computed(() => personName({ name: who.face?.arisen?.name ?? null, role: "arisen" }));
/** The slot the pin was made on, for the newer entry's accessible name. */
const pinnedSlot = computed(() => (newer.value ? shownSlot.slot : null));

function entryLabel(slot: MusterSlot, i: number): string {
  return slotEntryLabel(slot.slot, whenWords(slot.savedAt), slot.arisen?.level ?? null, {
    newest: i === 0,
    writing: slot.problem === "beingWritten",
    since: newerIds.has(slot.slotId) ? pinnedSlot.value : null,
  });
}

function onOutside(event: PointerEvent): void {
  if (root.value && event.target instanceof Node && !root.value.contains(event.target)) close();
}

function show(at: number): void {
  active.value = Math.max(0, Math.min(entries.value.length - 1, at));
  const rect = button.value?.getBoundingClientRect();
  maxHeight.value = rect ? `${Math.max(120, window.innerHeight - rect.bottom - 16)}px` : undefined;
  open.value = true;
  document.addEventListener("pointerdown", onOutside);
}

function close(): void {
  if (!open.value) return;
  open.value = false;
  document.removeEventListener("pointerdown", onOutside);
}

function choose(at: number): void {
  const slot = entries.value.at(at);
  close();
  button.value?.focus();
  if (slot) emit("pick", slot.slotId);
}

function move(at: number): void {
  active.value = Math.max(0, Math.min(entries.value.length - 1, at));
  const slot = entries.value.at(active.value);
  if (slot) document.getElementById(optionId(slot))?.scrollIntoView?.({ block: "nearest" });
}

function onClosedKey(event: KeyboardEvent): boolean {
  if (event.altKey && event.key !== "ArrowDown") return false;
  if (["Enter", " ", "ArrowDown"].includes(event.key)) show(shownIndex.value);
  else if (event.key === "ArrowUp") show(entries.value.length - 1);
  else return false;
  return true;
}

function onOpenKey(event: KeyboardEvent): boolean {
  const moves: Record<string, number> = {
    ArrowDown: active.value + 1,
    ArrowUp: active.value - 1,
    Home: 0,
    End: entries.value.length - 1,
  };
  const to = moves[event.key];
  if (to !== undefined) move(to);
  else if (event.key === "Enter" || event.key === " ") choose(active.value);
  else if (event.key === "Escape") close();
  else {
    // Tab moves on, a digit is a room key: both close without picking and go through
    if (event.key === "Tab" || /^\d$/.test(event.key)) close();
    return false;
  }
  return true;
}

function onKey(event: KeyboardEvent): void {
  if (event.ctrlKey || event.metaKey) return;
  const handled = open.value ? onOpenKey(event) : onClosedKey(event);
  if (handled) event.preventDefault();
}

function toggle(): void {
  if (open.value) close();
  else show(shownIndex.value);
}

// a re-listing that drops entries keeps the active one in range
watch(entries, (list) => {
  if (active.value > list.length - 1) active.value = Math.max(0, list.length - 1);
});

onBeforeUnmount(() => document.removeEventListener("pointerdown", onOutside));

defineExpose({ focus: () => button.value?.focus() });
</script>

<template>
  <span ref="root" class="smenu">
    <button
      ref="button"
      type="button"
      class="smenu__btn"
      role="combobox"
      aria-haspopup="listbox"
      :aria-expanded="open ? 'true' : 'false'"
      :aria-controls="MENU_ID"
      :aria-label="slotButtonLabel(arisen)"
      :aria-activedescendant="open && entries[active] ? optionId(entries[active]) : undefined"
      aria-describedby="muster-slotmenu-file muster-slotmenu-newer"
      @click="toggle"
      @keydown="onKey"
    >
      <span class="smenu__slot rc" :class="{ 'is-lit': lit }">Slot {{ shownSlot.slot }}</span
      ><span class="smenu__when">
        · <time :datetime="shownAt">{{ whenWords(shownAt) }}</time></span
      ><span class="smenu__caret" aria-hidden="true">▾</span
      ><span v-if="newer" class="smenu__dot" aria-hidden="true">•</span>
    </button>
    <span id="muster-slotmenu-file" class="rs-sr-only">{{ shownSlot.slotId }}</span>
    <span id="muster-slotmenu-newer" class="rs-sr-only">{{ newer ? NEWER_SAVE_NOTE : "" }}</span>
    <ul
      v-if="open"
      :id="MENU_ID"
      class="smenu__pop"
      role="listbox"
      :aria-label="`Saves of ${arisen}`"
      :style="{ maxHeight }"
    >
      <li
        v-for="(slot, i) in entries"
        :id="optionId(slot)"
        :key="slot.slotId"
        role="option"
        class="smenu__entry"
        :class="{ 'is-active': i === active }"
        :aria-selected="slot.slotId === shownSlot.slotId ? 'true' : 'false'"
        :aria-label="entryLabel(slot, i)"
        @mousedown.prevent
        @click="choose(i)"
      >
        <span class="smenu__check" aria-hidden="true">{{ slot.slotId === shownSlot.slotId ? "✓" : "" }}</span>
        <span class="smenu__new" aria-hidden="true">{{ newerIds.has(slot.slotId) ? "•" : "" }}</span>
        <span class="smenu__eslot">Slot {{ slot.slot }}</span>
        <time class="smenu__etime" :datetime="slot.savedAt">{{ whenWords(slot.savedAt) }}</time>
        <span v-if="slot.problem === 'beingWritten'" class="smenu__writing">{{ WRITING_ENTRY }}</span>
        <span v-else class="smenu__lv rs-mono">{{ slot.arisen ? `Lv ${slot.arisen.level}` : "" }}</span>
        <span class="smenu__badge rs-mono">{{ i === 0 ? NEWEST_BADGE : "" }}</span>
      </li>
    </ul>
  </span>
</template>

<style scoped>
.smenu {
  position: relative;
  flex: none;
  display: inline-flex;
}
/* a carved well, as every select in the window (§4.1) */
.smenu__btn {
  display: inline-flex;
  align-items: baseline;
  height: 28px;
  padding: 0 12px;
  border: 1px solid var(--rs-edgehi);
  border-radius: 0;
  background: var(--rs-well);
  color: var(--rs-text);
  font: inherit;
  line-height: 26px;
  white-space: nowrap;
  cursor: pointer;
  transition: background-color var(--rs-dur-quick) var(--rs-ease-carve);
}
.smenu__btn:hover {
  background: var(--rs-hearthhi);
}
.smenu__btn:focus-visible {
  outline: 2px solid var(--rs-rift);
  outline-offset: 2px;
}
.smenu__slot {
  font-weight: 500;
}
.smenu__slot.rc.is-lit {
  color: var(--rs-world);
}
.smenu__when,
.smenu__caret {
  color: var(--rs-faint);
}
/* the button is a flex row, whose items drop a leading space: the gap before ` · {when}` is a margin */
.smenu__when {
  margin-left: 0.3em;
}
.smenu__caret {
  margin-left: 0.5em;
}
.smenu__dot {
  margin-left: 0.35em;
  color: var(--rs-world);
}
/* R18.21: 320 px, a frame and no shadow, above the sheet, never above a toast */
.smenu__pop {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  z-index: var(--rs-z-world-overlay);
  box-sizing: border-box;
  width: 320px;
  margin: 0;
  padding: 0;
  overflow-y: auto;
  border: 1px solid var(--rs-edgehi);
  background: var(--rs-hearth);
  list-style: none;
}
.smenu__entry {
  display: grid;
  grid-template-columns: 1ch 1ch auto 1fr auto auto;
  align-items: baseline;
  column-gap: 8px;
  height: 40px;
  padding: 0 12px;
  line-height: 40px;
  background: var(--rs-hearth);
  cursor: pointer;
}
.smenu__entry.is-active,
.smenu__entry:hover {
  background: var(--rs-hearthhi);
}
.smenu__check,
.smenu__eslot {
  color: var(--rs-text);
}
.smenu__eslot {
  font-weight: 700;
}
.smenu__new {
  color: var(--rs-world);
}
.smenu__etime {
  overflow: hidden;
  color: var(--rs-faint);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.smenu__lv {
  color: var(--rs-text);
  font-size: var(--rs-text-xs);
  font-weight: 600;
  text-align: right;
}
.smenu__writing {
  color: var(--rs-warn);
  white-space: nowrap;
}
/* R16.21's NEWEST: the neutral outlined kind badge, a fact about your saves */
.smenu__badge:not(:empty) {
  padding: 0 4px;
  border: 1px solid var(--rs-edgehi);
  border-radius: var(--rs-radius-chip);
  color: var(--rs-muted);
  font-size: var(--rs-text-xs);
  font-weight: 600;
  line-height: 1.4;
}
</style>
