<script setup lang="ts">
// THE MUSTER's slot list (#00015 R17.5): a listbox of slabs, newest first in
// the spine's order (the window never re-sorts). Manual activation: the
// arrows move the focus and never read; Enter, Space or a click picks, and
// picking reads the file. One tab stop (roving tabindex).
import { ref, watch } from "vue";
import type { MusterSlot } from "@/types/rift";
import { SLOT_PROBLEM_WORDS, personName, whenWords } from "@/services/muster";
import MusterNamed from "./MusterNamed.vue";

const { slots, pickedId, names } = defineProps<{
  slots: MusterSlot[];
  pickedId: string | null;
  names: "game" | "missing";
}>();

const emit = defineEmits<{ pick: [slotId: string]; toTabs: [] }>();

const focusIndex = ref(0);
const rows = ref<HTMLElement[]>([]);

watch(
  () => [pickedId, slots] as const,
  () => {
    const at = slots.findIndex((s) => s.slotId === pickedId);
    if (at >= 0) focusIndex.value = at;
    else focusIndex.value = Math.min(focusIndex.value, Math.max(0, slots.length - 1));
  },
  { immediate: true },
);

function focusAt(index: number): void {
  if (!slots.length) return;
  focusIndex.value = Math.max(0, Math.min(slots.length - 1, index));
  rows.value[focusIndex.value]?.focus();
}

const MOVES: Record<string, (i: number) => number> = {
  ArrowDown: (i) => i + 1,
  ArrowUp: (i) => i - 1,
  Home: () => 0,
  End: () => slots.length - 1,
};

function onKey(event: KeyboardEvent): void {
  const move = MOVES[event.key];
  if (move) {
    event.preventDefault();
    focusAt(move(focusIndex.value));
    return;
  }
  if (event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    const slot = slots[focusIndex.value];
    if (slot) emit("pick", slot.slotId);
    return;
  }
  if (event.key === "ArrowRight") {
    event.preventDefault();
    emit("toTabs");
  }
}

function choose(index: number, slot: MusterSlot): void {
  focusIndex.value = index;
  emit("pick", slot.slotId);
}

/** R17.5 line 2's title: the summary without the person's name (Privacy). */
function summaryTitle(slot: MusterSlot): string | undefined {
  const a = slot.arisen;
  if (!a) return undefined;
  return `Lv ${a.level} ${a.vocation.name ?? `vocation ${a.vocation.id}`}`;
}

defineExpose({
  focusPicked: () =>
    focusAt(
      Math.max(
        0,
        slots.findIndex((s) => s.slotId === pickedId),
      ),
    ),
  /** `Pick another slot`: the newest slot that has no problem. */
  focusNewestOpen: () =>
    focusAt(
      Math.max(
        0,
        slots.findIndex((s) => s.problem === null),
      ),
    ),
});
</script>

<template>
  <ul class="slots" role="listbox" aria-label="Save slots" @keydown="onKey">
    <li
      v-for="(slot, i) in slots"
      :key="slot.slotId"
      ref="rows"
      role="option"
      class="slot"
      :class="[{ 'is-picked': slot.slotId === pickedId }, slot.problem ? `slot--${slot.problem}` : null]"
      :aria-selected="slot.slotId === pickedId ? 'true' : 'false'"
      :tabindex="i === focusIndex ? 0 : -1"
      :data-slot="slot.slotId"
      @click="choose(i, slot)"
      @focus="focusIndex = i"
    >
      <span class="slot__line">
        <span class="slot__name">Slot {{ slot.slot }}</span>
        <span v-if="slot.account !== null" class="slot__account rs-mono">ACCOUNT {{ slot.account }}</span>
        <time class="slot__when" :datetime="slot.savedAt">{{ whenWords(slot.savedAt) }}</time>
      </span>
      <span
        v-if="slot.problem"
        class="slot__sum slot__problem"
        :class="`is-${SLOT_PROBLEM_WORDS[slot.problem].tone}`"
        >{{ SLOT_PROBLEM_WORDS[slot.problem].words }}</span
      >
      <span v-else-if="slot.arisen" class="slot__sum" :title="summaryTitle(slot)"
        ><span :class="{ 'is-nameless-person': slot.arisen.name === null }">{{
          personName({ ...slot.arisen, role: "arisen" })
        }}</span>
        · Lv {{ slot.arisen.level }} <MusterNamed :named="slot.arisen.vocation" kind="vocation" :names="names"
      /></span>
    </li>
  </ul>
</template>

<style scoped>
.slots {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin: 0;
  padding: 0;
  list-style: none;
}
.slot {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 12px 16px 12px 20px;
  border-left: 4px solid transparent;
  background: var(--rs-hearth);
  cursor: pointer;
  transition: background-color var(--rs-dur-quick) var(--rs-ease-carve);
}
.slot:hover {
  background: var(--rs-hearthhi);
}
.slot.is-picked {
  background: var(--rs-hearthhi);
  /* the frame is the selected mark; the rail stays free for state */
  box-shadow: inset 0 0 0 1px var(--rs-edgehi);
}
.slot--damaged {
  border-left-color: var(--rs-ember);
}
.slot--beingWritten {
  border-left-color: var(--rs-warn);
}
.slot:focus-visible {
  outline: 2px solid var(--rs-rift);
  outline-offset: -2px;
}
.slot__line {
  display: flex;
  align-items: baseline;
  gap: var(--rs-space-2);
  min-width: 0;
}
.slot__name {
  flex: none;
  color: var(--rs-text);
  font-weight: 700;
}
.slot__account {
  flex: none;
  padding: 0 4px;
  border: 1px solid var(--rs-edgehi);
  border-radius: var(--rs-radius-chip);
  color: var(--rs-muted);
  font-size: var(--rs-text-xs);
  font-weight: 600;
}
.slot__when {
  margin-left: auto;
  color: var(--rs-faint);
  white-space: nowrap;
}
.slot__sum {
  display: block;
  overflow: hidden;
  color: var(--rs-muted);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.slot__sum :deep(.named) {
  display: inline;
}
.is-nameless-person,
.is-muted {
  color: var(--rs-muted);
}
.is-warn {
  color: var(--rs-warn);
}
.is-ember {
  color: var(--rs-ember);
}
.is-faint {
  color: var(--rs-faint);
}
</style>
