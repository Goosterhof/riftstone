<script setup lang="ts">
// §7.11 — who overrides whom. One row per contested file; the winner in world
// gold, the overridden struck. Grouped by winner, a count per pair.
import { computed, useId } from "vue";
import type { ConflictView } from "@/types/rift";

const { conflicts, focusId = null } = defineProps<{ conflicts: ConflictView[]; focusId?: string | null }>();

const UNCONFIRMED_TITLE =
  "A pak in this clash carries table flags the stone can't read yet, so it can't be sure this hash is the one the game loads. The order above still decides who wins if it is.";

/** The sentence is said once, visibly, under the map; each marker points at it. */
const noteId = useId();
const anyUnconfirmed = computed(() => conflicts.some((c) => c.unconfirmed));

const hex = (n: number) => (n >>> 0).toString(16).padStart(8, "0");

function keyLabel(c: ConflictView): string {
  if (c.key.kind === "path") return c.key.path;
  if (c.key.path) return c.key.path;
  return `pak entry ${hex(c.key.hash.lower)}·${hex(c.key.hash.upper)}`;
}

const groups = computed(() => {
  const byWinner = new Map<
    string,
    { winner: string; rows: { path: string; losers: string[]; unconfirmed: boolean }[] }
  >();
  for (const c of conflicts) {
    const g = byWinner.get(c.winner.optionId) ?? { winner: c.winner.name, rows: [] };
    g.rows.push({
      path: keyLabel(c),
      losers: c.layers.filter((l) => l.optionId !== c.winner.optionId).map((l) => l.name),
      unconfirmed: c.unconfirmed,
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
          ><span v-if="r.unconfirmed" class="cmap__unsure rs-mono" :aria-describedby="noteId">unconfirmed</span>
        </li>
      </ul>
    </section>
    <p v-if="anyUnconfirmed" :id="noteId" class="cmap__note">
      <span class="cmap__unsure rs-mono" aria-hidden="true">unconfirmed</span> {{ UNCONFIRMED_TITLE }}
    </p>
  </div>
</template>

<style scoped>
.cmap__unsure {
  margin-left: var(--rs-space-2);
  padding: 0 4px;
  border: 1px dashed var(--rs-edgehi);
  border-radius: var(--rs-radius-chip);
  color: var(--rs-faint);
  font-size: var(--rs-text-xs);
  white-space: nowrap;
}
.cmap__note {
  margin: var(--rs-space-2) 0 0;
  color: var(--rs-muted);
  font-size: var(--rs-text-sm);
  line-height: 1.45;
}
.cmap__note .cmap__unsure {
  margin: 0 var(--rs-space-1) 0 0;
}
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
