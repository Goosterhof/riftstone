<script setup lang="ts" generic="T extends string">
// One row of THE BENCH's SUMMONED WITH (0.7.2): a named radiogroup in the
// retired voice dial's idiom (segments, a sunken chosen one, arrows rove), with
// the line under it saying what the chosen setting does.
import { computed } from "vue";
import type { DialChoice } from "./words";

const { id, name, choices, chosen, title } = defineProps<{
  id: string;
  name: string;
  choices: DialChoice<T>[];
  chosen: T | undefined;
  title?: string;
}>();
const emit = defineEmits<{ pick: [value: T] }>();

const says = computed(() => choices.find((c) => c.value === chosen)?.says ?? "");

const STEP: Partial<Record<string, number>> = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 };

function onKey(event: KeyboardEvent, index: number): void {
  const step = STEP[event.key];
  if (!step) return;
  event.preventDefault();
  const count = choices.length;
  const next = choices[(index + step + count) % count];
  emit("pick", next.value);
  const group = (event.currentTarget as HTMLElement).parentElement;
  group?.querySelectorAll<HTMLElement>("[role=radio]")[choices.indexOf(next)]?.focus();
}
</script>

<template>
  <div class="row">
    <span :id="id" class="row__name rs-mono">{{ name }}</span>
    <div
      class="dial"
      role="radiogroup"
      :aria-labelledby="id"
      :aria-describedby="`${id}-says`"
      :title="title"
      :style="{ gridTemplateColumns: `repeat(${choices.length}, 1fr)` }"
    >
      <button
        v-for="(c, i) in choices"
        :key="c.value"
        type="button"
        role="radio"
        class="dial__seg"
        :aria-checked="chosen === c.value"
        :tabindex="chosen === c.value ? 0 : -1"
        @click="emit('pick', c.value)"
        @keydown="onKey($event, i)"
      >
        {{ c.label }}
      </button>
    </div>
    <p :id="`${id}-says`" class="row__says">{{ says }}</p>
  </div>
</template>

<style scoped>
.row {
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-1);
}
.row__name {
  color: var(--rs-faint);
  font-size: var(--rs-text-xs);
  letter-spacing: 0.08em;
}
.row__says {
  min-height: 2.8em;
  margin: 0;
  color: var(--rs-muted);
  font: 400 var(--rs-text-xs) / 1.4 var(--rs-font-sans);
}
.dial {
  display: grid;
  border: 1px solid var(--rs-edgehi);
}
.dial__seg {
  min-width: 0;
  height: 30px;
  padding: 0 2px;
  border: 0;
  border-left: 1px solid var(--rs-edge);
  background: transparent;
  color: var(--rs-muted);
  font: 500 var(--rs-text-sm) var(--rs-font-sans);
  white-space: nowrap;
  cursor: pointer;
}
.dial__seg:first-child {
  border-left: 0;
}
.dial__seg:hover {
  background: var(--rs-misthi);
}
.dial__seg[aria-checked="true"] {
  background: var(--rs-well);
  box-shadow: inset 0 2px 0 rgba(0, 0, 0, 0.45);
  color: var(--rs-text);
}
</style>
