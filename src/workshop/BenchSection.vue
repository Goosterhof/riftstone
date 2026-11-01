<script setup lang="ts">
// 0.7.4 — one section of THE BENCH, foldable under its own head (R20.5's
// sections, the Undercroft's ▸ / ▾). Folded, the head keeps a one-line summary,
// so a fold hides the controls and never the state. A section the stone needs
// the investor in (`held`: Tamsin waits on a press; a refusal to read) stays
// open, and its toggle says why instead of doing nothing.
import { computed } from "vue";
import { useBenchFolds, type BenchSectionKey } from "./benchFolds";

const { section, title, summary, held } = defineProps<{
  section: BenchSectionKey;
  title: string;
  /** What the folded head says in place of the body. */
  summary: string;
  /** Why the section cannot fold right now, or null. */
  held?: string | null;
}>();

const folds = useBenchFolds();
const open = computed(() => Boolean(held) || !folds.isFolded(section));
const headId = computed(() => `ws-${section}`);
const bodyId = computed(() => `ws-${section}-body`);
</script>

<template>
  <section class="bench__section" :class="{ 'is-folded': !open }" :aria-labelledby="headId">
    <h2 class="bench__head rs-inscription">
      <button
        type="button"
        class="bench__toggle"
        :aria-expanded="open ? 'true' : 'false'"
        :aria-controls="bodyId"
        :aria-disabled="held ? 'true' : undefined"
        :title="held ?? undefined"
        @click="held || folds.toggle(section)"
      >
        <span :id="headId" class="bench__title">{{ title }}</span>
        <span v-if="!open" class="bench__summary rs-mono">{{ summary }}</span>
        <span class="bench__mark" aria-hidden="true">{{ open ? "▾" : "▸" }}</span>
      </button>
    </h2>
    <div v-show="open" :id="bodyId" class="bench__body">
      <slot />
    </div>
  </section>
</template>

<style scoped>
.bench__section {
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-2);
  padding: var(--rs-space-4) var(--rs-space-5);
  border-bottom: 1px solid var(--rs-edge);
}
.bench__section.is-folded {
  padding-block: var(--rs-space-3);
}
.bench__section:last-child {
  border-bottom: 0;
}
.bench__head {
  margin: 0;
  font-size: 14px;
}
/* Title and mark on the first line; a folded head's summary on its own line
   under them, whole (the bench is 320 px: beside the title it was cut). */
.bench__toggle {
  display: grid;
  grid-template-columns: 1fr auto;
  align-items: baseline;
  column-gap: var(--rs-space-2);
  row-gap: 2px;
  width: 100%;
  padding: 0;
  border: 0;
  background: none;
  color: var(--rs-muted);
  font: inherit;
  letter-spacing: inherit;
  text-align: left;
  cursor: pointer;
}
.bench__toggle:hover:not([aria-disabled="true"]) {
  color: var(--rs-text);
}
.bench__toggle[aria-disabled="true"] {
  cursor: default;
}
.bench__toggle:focus-visible {
  outline: 2px solid var(--rs-rift);
  outline-offset: 2px;
}
.bench__title {
  grid-column: 1;
  grid-row: 1;
}
.bench__summary {
  grid-column: 1 / -1;
  grid-row: 2;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--rs-faint);
  font-size: 12px;
  letter-spacing: 0;
  text-transform: none;
}
.bench__mark {
  grid-column: 2;
  grid-row: 1;
  color: var(--rs-faint);
}
.bench__body {
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-2);
}
</style>
