<script setup lang="ts">
// §7.11 — who overrides whom. One row per contested file; the winner in world
// gold, the overridden struck. Grouped by winner, a count per pair.
import { computed } from "vue";
import type { ConflictView } from "@/types/rift";

const { conflicts, focusId = null } = defineProps<{ conflicts: ConflictView[]; focusId?: string | null }>();

const hex = (n: number) => (n >>> 0).toString(16).padStart(8, "0");

function keyLabel(c: ConflictView): string {
  if (c.key.kind === "path") return c.key.path;
  if (c.key.path) return c.key.path;
  return `pak entry ${hex(c.key.hash.lower)}·${hex(c.key.hash.upper)}`;
}

const groups = computed(() => {
  const byWinner = new Map<string, { winner: string; rows: { path: string; losers: string[] }[] }>();
  for (const c of conflicts) {
    const g = byWinner.get(c.winner.optionId) ?? { winner: c.winner.name, rows: [] };
    g.rows.push({
      path: keyLabel(c),
      losers: c.layers.filter((l) => l.optionId !== c.winner.optionId).map((l) => l.name),
    });
    byWinner.set(c.winner.optionId, g);
  }
  return [...byWinner.entries()].map(([id, g]) => ({ id, ...g }));
});
</script>

<template>
  <div class="cmap">
    <p v-if="!conflicts.length" class="cmap__none">
      No two mods in your game touch the same file. Nobody overrides anybody.
    </p>
    <section v-for="g in groups" :key="g.id" class="cmap__group" :class="{ 'is-focus': focusId === g.id }">
      <h4 class="cmap__winner">
        <span class="cmap__w">{{ g.winner }}</span> wins <span class="cmap__n rs-mono">×{{ g.rows.length }}</span>
      </h4>
      <ul class="cmap__rows">
        <li v-for="(r, i) in g.rows" :key="i" class="cmap__row">
          <span class="cmap__path rs-mono">{{ r.path }}</span>
          <span class="cmap__losers"
            >over <s v-for="l in r.losers" :key="l">{{ l }}</s></span
          >
        </li>
      </ul>
    </section>
  </div>
</template>

<style scoped>
.cmap__none {
  margin: 0;
  color: var(--rs-faint);
}
.cmap__group + .cmap__group {
  margin-top: var(--rs-space-4);
}
.cmap__winner {
  margin: 0 0 var(--rs-space-2);
  font: 500 var(--rs-text-sm) var(--rs-font-sans);
  color: var(--rs-muted);
}
.cmap__w {
  color: var(--rs-world);
  font-weight: 700;
}
.cmap__n {
  margin-left: 4px;
  padding: 0 4px;
  border: 1px solid var(--rs-edgehi);
  border-radius: var(--rs-radius-chip);
  color: var(--rs-muted);
}
.cmap__rows {
  margin: 0;
  padding: 0;
  list-style: none;
}
.cmap__row {
  display: flex;
  flex-wrap: wrap;
  gap: 2px var(--rs-space-3);
  padding: 6px var(--rs-space-2);
  margin-bottom: 2px;
  background: var(--rs-hearthhi);
}
.cmap__path {
  flex: 1 1 100%;
  min-width: 0;
  color: var(--rs-text);
  word-break: break-all;
}
.cmap__losers {
  color: var(--rs-faint);
  font-size: var(--rs-text-sm);
}
.cmap__losers s {
  margin-left: 4px;
}
</style>
