<script setup lang="ts">
// GEAR (#00015 R17.11): eight rows, always eight, in the game's order. An
// empty slot keeps its row (`— empty`, faint: a fact, not a fault), so the
// sheet's shape never changes between members.
import type { MusterGear } from "@/types/rift";
import { GEAR_LABEL } from "@/services/muster";
import { wasWords, type Mark } from "@/services/recarving";
import MusterNamed from "./MusterNamed.vue";

const { gear, names, marks, lit } = defineProps<{
  gear: MusterGear[];
  names: "game" | "missing";
  marks: Map<number, Mark>;
  lit: boolean;
}>();
</script>

<template>
  <section class="gear" aria-labelledby="muster-gear-h">
    <h3 id="muster-gear-h" class="msec rs-inscription">GEAR</h3>
    <dl class="gear__rows">
      <div v-for="(g, i) in gear" :key="g.slot" class="gear__row" :data-gear="g.slot">
        <dt class="gear__label">{{ GEAR_LABEL[g.slot] }}</dt>
        <dd class="gear__item">
          <MusterNamed
            v-if="g.item"
            :named="g.item"
            kind="item"
            :names="names"
            :mark="marks.get(i)"
            :lit="lit"
            titled
          />
          <span v-else class="gear__empty" :class="{ 'rc is-lit': lit && marks.has(i) }"
            ><span v-if="marks.has(i)" class="gear__dot" aria-hidden="true">• </span
            ><span aria-hidden="true">— </span>empty<span v-if="marks.has(i)" class="rs-sr-only"
              >, {{ wasWords(marks.get(i)) }}</span
            ></span
          >
        </dd>
      </div>
    </dl>
  </section>
</template>

<style scoped>
.gear__rows {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: 0;
}
.gear__row {
  display: grid;
  grid-template-columns: 104px minmax(0, 1fr);
  align-items: center;
  height: 32px;
  padding: 0 12px;
  background: var(--rs-row);
}
.gear__label {
  color: var(--rs-faint);
  font-weight: 500;
}
.gear__item {
  display: flex;
  min-width: 0;
  margin: 0;
  color: var(--rs-text);
}
.gear__empty {
  color: var(--rs-faint);
}
.gear__dot {
  color: var(--temp);
}
</style>
