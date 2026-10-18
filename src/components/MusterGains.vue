<script setup lang="ts">
// GAINS FROM LEVELS (#00015 R17.12): the number first, then the party mark —
// a flat bar whose length is this member's value over the party's highest of
// that stat (services/muster.ts partyMark), and `most in the party` in words
// on the holder(s). A party of one has no one to compare with: numbers only.
import { computed } from "vue";
import type { MusterMember } from "@/types/rift";
import { GAIN_ROWS, GAINS_SCOPE, MOST_IN_PARTY, count, gainWords, partyMark, type GainKey } from "@/services/muster";
import { deltaText, type Mark } from "@/services/recarving";

const { member, party, marks, lit } = defineProps<{
  member: MusterMember;
  party: MusterMember[];
  marks: Partial<Record<GainKey, Mark>>;
  lit: boolean;
}>();

const compared = computed(() => party.length > 1);

const rows = computed(() =>
  GAIN_ROWS.map(({ key, label }) => {
    const value = member.gains[key];
    const max = Math.max(...party.map((m) => m.gains[key]));
    const ratio = compared.value ? partyMark(value, max) : null;
    const most = compared.value && max > 0 && value === max;
    const mark = marks[key];
    return {
      key,
      label,
      value,
      ratio,
      most,
      mark,
      delta: deltaText(mark),
      words: gainWords(label, value, ratio, most),
    };
  }),
);
</script>

<template>
  <section class="gains" aria-labelledby="muster-gains-h">
    <h3 id="muster-gains-h" class="msec rs-inscription">GAINS FROM LEVELS</h3>
    <p class="gains__scope">{{ GAINS_SCOPE }}</p>
    <ul class="gains__rows" :class="{ 'is-alone': !compared }">
      <li v-for="r in rows" :key="r.key" class="gains__row" :data-gain="r.key">
        <span class="rs-sr-only">{{ r.words }}</span>
        <span class="gains__label" aria-hidden="true">{{ r.label }}</span>
        <span class="gains__value rs-mono" aria-hidden="true"
          ><span class="rc" :class="{ 'is-lit': lit && r.mark }">+{{ count(r.value) }}</span></span
        >
        <span v-if="r.delta" class="rc-mark rs-mono" aria-hidden="true">{{ r.delta }}</span>
        <span v-else class="rc-mark" aria-hidden="true" />
        <template v-if="r.ratio !== null">
          <span class="gains__track" aria-hidden="true"
            ><span class="gains__bar" :style="{ width: `${r.ratio * 100}%` }"
          /></span>
          <span class="gains__most" aria-hidden="true">{{ r.most ? MOST_IN_PARTY : "" }}</span>
        </template>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.gains__scope {
  margin: 0 0 var(--rs-space-2);
  color: var(--rs-faint);
}
.gains__rows {
  margin: 0;
  padding: 0;
  list-style: none;
}
.gains__row {
  display: grid;
  grid-template-columns: 104px 56px 44px minmax(24px, 1fr) auto;
  align-items: center;
  column-gap: 6px;
  min-height: 28px;
}
.is-alone .gains__row {
  grid-template-columns: 104px 56px 44px;
}
.gains__label {
  color: var(--rs-muted);
  font-weight: 500;
}
.gains__value {
  color: var(--rs-text);
  font-size: var(--rs-text-xs);
  font-weight: 600;
  text-align: right;
}
.gains__track {
  height: 4px;
  background: var(--rs-edge);
}
.gains__bar {
  display: block;
  height: 100%;
  background: var(--rs-muted);
}
.gains__most {
  color: var(--rs-faint);
  white-space: nowrap;
}
</style>
