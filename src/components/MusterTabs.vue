<script setup lang="ts">
// THE MUSTER's tab line (#00015 R17.8): up to four members in a fixed order
// (the Arisen, the main pawn, the two hired places). Position is identity: a
// missing member leaves its place empty, never closes the gap. Automatic
// activation: the party is already read, so an arrow switches the sheet.
// A hired pawn's tab is cold (mist, cyan rail): it came through the Rift.
import { computed, ref } from "vue";
import type { MusterMember } from "@/types/rift";
import { PLACES, type Place } from "@/composables/useMuster";
import { ROLE_WORD, namedText, personName, tabName } from "@/services/muster";
import type { Recarving } from "@/services/recarving";

const { places, active, marks } = defineProps<{
  places: Record<Place, MusterMember | null>;
  active: Place;
  marks: Recarving;
}>();

const emit = defineEmits<{ select: [place: Place]; toSheet: [] }>();

const tabs = ref<Partial<Record<Place, HTMLElement>>>({});
const present = computed(() => PLACES.filter((p) => places[p] !== null));
const cells = computed(() => PLACES.map((place) => ({ place, member: places[place] })));

const tabId = (place: Place) => `muster-tab-${place}`;

function setRef(place: Place, el: unknown): void {
  if (el instanceof HTMLElement) tabs.value[place] = el;
}

function go(place: Place | undefined): void {
  if (!place) return;
  emit("select", place);
  tabs.value[place]?.focus();
}

function onKey(event: KeyboardEvent): void {
  const list = present.value;
  const at = list.indexOf(active);
  const moves: Record<string, () => Place | undefined> = {
    ArrowRight: () => list.at((at + 1) % list.length),
    ArrowLeft: () => list.at((at - 1 + list.length) % list.length),
    Home: () => list[0],
    End: () => list.at(-1),
  };
  const move = moves[event.key];
  if (move) {
    event.preventDefault();
    go(move());
  } else if (event.key === "ArrowDown") {
    event.preventDefault();
    emit("toSheet");
  }
}

const cold = (m: MusterMember) => m.role === "hiredPawn";
const marked = (m: MusterMember) => marks.members.has(m.memberKey);
const joined = (m: MusterMember) => marks.joined.has(m.memberKey);

defineExpose({ focusActive: () => tabs.value[active]?.focus(), tabId });
</script>

<template>
  <div class="tabs" role="tablist" aria-label="The party" @keydown="onKey">
    <template v-for="cell in cells" :key="cell.place">
      <button
        v-if="cell.member"
        :id="tabId(cell.place)"
        :ref="(el) => setRef(cell.place, el)"
        type="button"
        role="tab"
        class="tab"
        :class="{ 'is-cold': cold(cell.member), 'is-on': cell.place === active }"
        :data-place="cell.place"
        :aria-selected="cell.place === active ? 'true' : 'false'"
        aria-controls="muster-sheet"
        :aria-label="tabName(cell.member)"
        :tabindex="cell.place === active ? 0 : -1"
        @click="emit('select', cell.place)"
      >
        <span class="tab__role"
          >{{ ROLE_WORD[cell.member.role] }}<span v-if="joined(cell.member)" class="tab__new"> new</span></span
        >
        <span class="tab__name" :class="{ 'is-nameless': cell.member.name === null }">{{
          personName(cell.member)
        }}</span>
        <span class="tab__line">
          <span class="tab__lv rs-mono">Lv {{ cell.member.level }}</span>
          <span class="tab__voc" :class="{ 'rs-mono': cell.member.vocation.name === null }">{{
            namedText(cell.member.vocation, "vocation")
          }}</span>
          <span v-if="marked(cell.member)" class="tab__mark" aria-hidden="true">•</span>
        </span>
      </button>
      <span v-else class="tab tab--empty" aria-hidden="true" :data-place="cell.place" />
    </template>
  </div>
</template>

<style scoped>
.tabs {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 8px;
}
.tab {
  --temp: var(--rs-world);
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 1px;
  min-width: 0;
  padding: 8px 12px 8px 16px;
  border: 0;
  border-left: 4px solid var(--temp);
  background: var(--rs-hearth);
  color: var(--rs-text);
  font: inherit;
  text-align: left;
  cursor: pointer;
  transition: background-color var(--rs-dur-settle) var(--rs-ease-carve);
}
.tab.is-cold {
  --temp: var(--rs-rift);
  background: var(--rs-mist);
}
.tab.is-on {
  background: var(--rs-hearthhi);
  /* the frame opens into the sheet below: no line on the bottom */
  box-shadow:
    inset 0 1px 0 var(--rs-edgehi),
    inset -1px 0 0 var(--rs-edgehi);
}
.tab.is-cold.is-on {
  background: var(--rs-misthi);
}
.tab:hover:not(.is-on, .tab--empty) {
  background: var(--rs-hearthhi);
}
.tab.is-cold:hover:not(.is-on) {
  background: var(--rs-misthi);
}
.tab:focus-visible {
  outline: 2px solid var(--rs-rift);
  outline-offset: -2px;
}
.tab--empty {
  border: 0;
  background: none;
  cursor: default;
}
.tab__role {
  color: var(--rs-muted);
  font-weight: 500;
}
.tab__new {
  color: var(--rs-faint);
}
.tab__name,
.tab__voc {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.tab__name {
  font-weight: 700;
}
.tab__name.is-nameless {
  color: var(--rs-muted);
  font-weight: 400;
}
.tab__line {
  display: flex;
  align-items: baseline;
  gap: 6px;
  min-width: 0;
}
.tab__lv {
  flex: none;
  font-size: var(--rs-text-xs);
  font-weight: 600;
}
.tab__voc {
  min-width: 0;
  color: var(--rs-faint);
}
.tab__voc.rs-mono {
  font-size: var(--rs-text-xs);
}
.tab__mark {
  flex: none;
  color: var(--temp);
}
</style>
