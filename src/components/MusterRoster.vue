<script setup lang="ts">
// v0.5 — THE MUSTER's left column (#00015 R18.15–R18.17, R18.27): the
// characters, each the Arisen with their main pawn, newest save first in the
// spine's order; then the slots that won't open, apart, under their problem.
// Two listboxes, two tab stops, manual activation as R17.5's: the arrows move
// the focus and read nothing; Enter, Space or a click picks. `↓` from the last
// character crosses into the slots apart, `↑` from their first crosses back.
//
// Privacy: a character's key never reaches the DOM. Rows are told apart in
// memory, and the focus is held by key, so a reorder after a save never moves
// it to another person (R18.17).
import { computed, nextTick, ref, watch } from "vue";
import type { MusterSlot } from "@/types/rift";
import type { ApartGroup, MusterCharacter } from "@/composables/useMuster";
import {
  GONE_LINE,
  SLOT_PROBLEM_WORDS,
  apartHead,
  characterLabel,
  namedText,
  personName,
  savesWords,
  whenWords,
} from "@/services/muster";
import MusterNamed from "./MusterNamed.vue";

const { characters, apart, shown, pickedId, names, marked, gone } = defineProps<{
  characters: MusterCharacter[];
  apart: ApartGroup[];
  /** The character on the sheet, or null while a slot apart is shown. */
  shown: MusterCharacter | null;
  pickedId: string | null;
  names: "game" | "missing";
  /** THE RE-CARVING has marks on the sheet (R18.15's `•`, the shown character only). */
  marked: boolean;
  /** R18.27: the remembered character is gone. */
  gone: boolean;
}>();

const emit = defineEmits<{ pickCharacter: [key: string]; pickApart: [slotId: string]; toTabs: [] }>();

/** The roving tab stops: a character by key, a slot apart by its id. */
const focusKey = ref<string | null>(null);
const focusSlot = ref<string | null>(null);
/** The rows by key and by slot id, in memory only (a v-for ref array keeps no order). */
const charRows = new Map<string, HTMLElement>();
const apartRows = new Map<string, HTMLElement>();

function holdRow(rows: Map<string, HTMLElement>, id: string, el: unknown): void {
  if (el instanceof HTMLElement) rows.set(id, el);
  else rows.delete(id);
}

const apartSlots = computed<MusterSlot[]>(() => apart.flatMap((g) => g.slots));
const apartCount = computed(() => apartSlots.value.length);

/** The character stop follows the sheet's character, held by key (R18.17). */
function syncCharacterStop(): void {
  const keys = characters.map((c) => c.key);
  if (shown && keys.includes(shown.key)) focusKey.value = shown.key;
  else if (focusKey.value === null || !keys.includes(focusKey.value)) focusKey.value = keys.at(0) ?? null;
}

function syncApartStop(): void {
  const ids = apartSlots.value.map((s) => s.slotId);
  if (!shown && pickedId && ids.includes(pickedId)) focusSlot.value = pickedId;
  else if (focusSlot.value === null || !ids.includes(focusSlot.value)) focusSlot.value = ids.at(0) ?? null;
}

watch(
  () => [characters, shown, apartSlots.value, pickedId] as const,
  () => {
    syncCharacterStop();
    syncApartStop();
  },
  { immediate: true },
);

const rowOfKey = (key: string | null): HTMLElement | undefined => (key === null ? undefined : charRows.get(key));
const rowOfSlot = (slotId: string | null): HTMLElement | undefined =>
  slotId === null ? undefined : apartRows.get(slotId);

function focusCharacter(index: number): void {
  const who = characters.at(Math.max(0, Math.min(characters.length - 1, index)));
  if (!who) return;
  focusKey.value = who.key;
  rowOfKey(who.key)?.focus();
}

function focusApart(index: number): void {
  const slot = apartSlots.value.at(Math.max(0, Math.min(apartSlots.value.length - 1, index)));
  if (!slot) return;
  focusSlot.value = slot.slotId;
  rowOfSlot(slot.slotId)?.focus();
}

/** One listbox's roving keys; `cross` moves into the other listbox at an edge. */
interface Roving {
  index: number;
  last: number;
  focus: (index: number) => void;
  pick: () => void;
  crossDown?: () => void;
  crossUp?: () => void;
}

function roveWithin(key: string, r: Roving): boolean {
  const moves: Record<string, number> = { ArrowDown: r.index + 1, ArrowUp: r.index - 1, Home: 0, End: r.last };
  const to = moves[key];
  if (to !== undefined) r.focus(to);
  else if (key === "Enter" || key === " ") r.pick();
  else if (key === "ArrowRight") emit("toTabs");
  else return false;
  return true;
}

function rove(event: KeyboardEvent, r: Roving): void {
  let handled = true;
  if (event.key === "ArrowDown" && r.index === r.last && r.crossDown) r.crossDown();
  else if (event.key === "ArrowUp" && r.index === 0 && r.crossUp) r.crossUp();
  else handled = roveWithin(event.key, r);
  if (handled) event.preventDefault();
}

function onCharacterKey(event: KeyboardEvent, index: number, who: MusterCharacter): void {
  rove(event, {
    index,
    last: characters.length - 1,
    focus: focusCharacter,
    pick: () => emit("pickCharacter", who.key),
    crossDown: apartSlots.value.length ? () => focusApart(0) : undefined,
  });
}

function onApartKey(event: KeyboardEvent, slot: MusterSlot): void {
  rove(event, {
    index: apartIndex(slot),
    last: apartSlots.value.length - 1,
    focus: focusApart,
    pick: () => emit("pickApart", slot.slotId),
    crossUp: characters.length ? () => focusCharacter(characters.length - 1) : undefined,
  });
}

function chooseCharacter(who: MusterCharacter): void {
  focusKey.value = who.key;
  emit("pickCharacter", who.key);
}

function chooseApart(slot: MusterSlot): void {
  focusSlot.value = slot.slotId;
  emit("pickApart", slot.slotId);
}

const arisenOf = (who: MusterCharacter): string => personName({ name: who.face?.arisen?.name ?? null, role: "arisen" });
const pawnOf = (who: MusterCharacter): string | null => who.face?.mainPawnName ?? null;
const newestAt = (who: MusterCharacter): string => who.slots.at(0)?.savedAt ?? "";
const isShown = (who: MusterCharacter): boolean => shown?.key === who.key;

function labelOf(who: MusterCharacter): string {
  const a = who.face?.arisen ?? null;
  return characterLabel({
    arisen: arisenOf(who),
    pawn: pawnOf(who),
    level: a?.level ?? null,
    vocation: a ? namedText(a.vocation, "vocation") : null,
    saves: who.slots.length,
    when: whenWords(newestAt(who)),
  });
}

/** The index of a slot apart across every group, for the roving stop. */
const apartIndex = (slot: MusterSlot): number => apartSlots.value.findIndex((s) => s.slotId === slot.slotId);

defineExpose({
  /** `5` (R18.26): the shown character's row, else the slot apart on the sheet. */
  focusShown: () => {
    void nextTick(() => {
      if (shown) rowOfKey(shown.key)?.focus();
      else if (pickedId && rowOfSlot(pickedId)) rowOfSlot(pickedId)?.focus();
      else focusCharacter(0);
    });
  },
  /** `Pick another slot` (§18.11): the newest character, else the first slot apart. */
  focusNewestOpen: () => {
    if (characters.length) focusCharacter(0);
    else focusApart(0);
  },
});
</script>

<template>
  <div class="roster">
    <p v-if="gone" class="roster__gone">{{ GONE_LINE }}</p>
    <ul v-if="characters.length" class="roster__list" role="listbox" aria-label="Characters">
      <li
        v-for="(who, i) in characters"
        :key="who.key"
        :ref="(el) => holdRow(charRows, who.key, el)"
        role="option"
        class="who"
        :class="{ 'is-picked': isShown(who), 'is-writing': who.writing }"
        :aria-selected="isShown(who) ? 'true' : 'false'"
        :aria-label="labelOf(who)"
        :tabindex="who.key === focusKey ? 0 : -1"
        @click="chooseCharacter(who)"
        @focus="focusKey = who.key"
        @keydown="onCharacterKey($event, i, who)"
      >
        <span class="who__line">
          <span class="who__names"
            ><span class="who__arisen" :class="{ 'is-nameless-person': !who.face?.arisen?.name }">{{
              arisenOf(who)
            }}</span
            ><span v-if="pawnOf(who) !== null" class="who__pawn"> &amp; {{ pawnOf(who) }}</span></span
          >
          <span v-if="who.face && who.face.account !== null" class="who__account rs-mono"
            >ACCOUNT {{ who.face.account }}</span
          >
        </span>
        <span v-if="who.face?.arisen" class="who__sum"
          ><span class="rs-mono who__lv">Lv {{ who.face.arisen.level }}</span
          >{{ " "
          }}<MusterNamed class="who__voc" :named="who.face.arisen.vocation" kind="vocation" :names="names" /><span
            v-if="marked && isShown(who)"
            class="who__dot"
            aria-hidden="true"
            >•</span
          ></span
        >
        <span class="who__saves"
          >{{ savesWords(who.slots.length) }} ·
          <time :datetime="newestAt(who)">{{ whenWords(newestAt(who)) }}</time></span
        >
      </li>
    </ul>

    <template v-if="apartCount">
      <p class="roster__apart rs-mono" aria-hidden="true">{{ apartHead(apartCount) }}</p>
      <div class="roster__groups" role="listbox" aria-label="Saves that won't open">
        <div
          v-for="group in apart"
          :key="group.reason"
          role="group"
          :aria-label="SLOT_PROBLEM_WORDS[group.reason].words"
        >
          <p class="roster__why" :class="`is-${SLOT_PROBLEM_WORDS[group.reason].tone}`" aria-hidden="true">
            {{ SLOT_PROBLEM_WORDS[group.reason].words }}
          </p>
          <div
            v-for="slot in group.slots"
            :key="slot.slotId"
            :ref="(el) => holdRow(apartRows, slot.slotId, el)"
            role="option"
            class="slot"
            :class="[{ 'is-picked': !shown && slot.slotId === pickedId }, `slot--${group.reason}`]"
            :aria-selected="!shown && slot.slotId === pickedId ? 'true' : 'false'"
            :tabindex="slot.slotId === focusSlot ? 0 : -1"
            :data-slot="slot.slotId"
            @click="chooseApart(slot)"
            @focus="focusSlot = slot.slotId"
            @keydown="onApartKey($event, slot)"
          >
            <span class="slot__name">Slot {{ slot.slot }}</span>
            <span v-if="slot.account !== null" class="who__account rs-mono">ACCOUNT {{ slot.account }}</span>
            <time class="slot__when" :datetime="slot.savedAt">{{ whenWords(slot.savedAt) }}</time>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.roster__list,
.roster__groups {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin: 0;
  padding: 0;
  list-style: none;
}
.roster__gone {
  margin: 0 0 var(--rs-space-3);
  color: var(--rs-faint);
}
/* R17.5's slab, one line taller (R18.15) */
.who,
.slot {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
  padding: 12px 16px 12px 20px;
  border-left: 4px solid transparent;
  background: var(--rs-hearth);
  cursor: pointer;
  transition:
    background-color var(--rs-dur-quick) var(--rs-ease-carve),
    border-color var(--rs-dur-quick) var(--rs-ease-carve);
}
.slot {
  flex-direction: row;
  align-items: baseline;
  gap: var(--rs-space-2);
}
.who:hover,
.slot:hover {
  background: var(--rs-hearthhi);
}
.who.is-picked,
.slot.is-picked {
  background: var(--rs-hearthhi);
  /* the frame is the selected mark; the rail stays free for state */
  box-shadow: inset 0 0 0 1px var(--rs-edgehi);
}
.who.is-writing,
.slot--beingWritten {
  border-left-color: var(--rs-warn);
}
.slot--damaged {
  border-left-color: var(--rs-ember);
}
.who:focus-visible,
.slot:focus-visible {
  outline: 2px solid var(--rs-rift);
  outline-offset: -2px;
}
.who__line {
  display: flex;
  align-items: baseline;
  gap: var(--rs-space-2);
  min-width: 0;
}
.who__names {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.who__arisen {
  color: var(--rs-text);
  font-weight: 700;
}
.who__pawn {
  color: var(--rs-muted);
}
.who__account {
  flex: none;
  margin-left: auto;
  padding: 0 4px;
  border: 1px solid var(--rs-edgehi);
  border-radius: var(--rs-radius-chip);
  color: var(--rs-muted);
  font-size: var(--rs-text-xs);
  font-weight: 600;
}
.who__sum {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.who__lv {
  color: var(--rs-text);
  font-size: var(--rs-text-xs);
  font-weight: 600;
}
.who__voc {
  color: var(--rs-faint);
}
.who__sum :deep(.named) {
  display: inline;
}
.who__dot {
  margin-left: 0.4em;
  color: var(--rs-world);
}
.who__saves {
  color: var(--rs-faint);
  white-space: nowrap;
}
/* R18.16: a carved line, the Rift's trouble count — an inscription, never a button */
.roster__apart {
  margin: var(--rs-space-4) 0 var(--rs-space-2);
  color: var(--rs-muted);
  font-size: var(--rs-text-xs);
  font-weight: 600;
  letter-spacing: 0.12em;
}
.roster__why {
  margin: var(--rs-space-2) 0 4px;
  font-weight: 500;
}
.slot__name {
  flex: none;
  color: var(--rs-text);
  font-weight: 700;
}
.slot__when {
  margin-left: auto;
  color: var(--rs-faint);
  white-space: nowrap;
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
