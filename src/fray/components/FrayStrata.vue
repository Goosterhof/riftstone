<script setup lang="ts">
// THE STRATA (D5, RF.26): the settled fights as thin carved layers, newest
// on top; only the newest and the browsed one are labelled. In a fight it is
// inert; after it (and in a sleeping screen) it is the door to the shelf.
import { computed } from "vue";
import type { FrayFight } from "@/types/fray";
import { isWarm, share, tagText } from "../reading";

const { shelf, framedId, door, rows } = defineProps<{
  shelf: FrayFight[];
  /** The fight framed on the shelf (browsed, or kept), by id. */
  framedId: string | null;
  door: boolean;
  rows: number;
}>();
const emit = defineEmits<{ pick: [n: number | null] }>();

const shown = computed(() => {
  const list = shelf.slice(0, Math.max(1, rows));
  const framed = shelf.find((x) => x.id === framedId);
  if (framed && !list.includes(framed)) list[list.length - 1] = framed;
  return list;
});

function segs(f: FrayFight): { k: string; c: string; v: number }[] {
  const out: { k: string; c: string; v: number }[] = [];
  for (const m of f.members) {
    if (m && m.damage > 0) out.push({ k: `m${m.slot}`, c: isWarm(m) ? "warm" : "cold", v: share(m.damage, f.hAll) });
  }
  if (f.world && f.world.damage > 0) out.push({ k: "w", c: "wseg", v: share(f.world.damage, f.hAll) });
  out.push({ k: "u", c: "nob", v: share(f.unattributed.damage, f.hAll) });
  return out;
}

const labelled = (f: FrayFight, i: number): boolean => (framedId ? f.id === framedId : i === 0);

function pick(event: MouseEvent): void {
  const layer = (event.target as HTMLElement | null)?.closest<HTMLElement>("[data-fight]");
  emit("pick", layer ? Number(layer.dataset.fight) : null);
}
</script>

<template>
  <div v-if="!shelf.length" class="strata">
    <span class="lbl">RECENT FIGHTS</span><span class="empty">The strata fill as fights settle.</span>
  </div>
  <component
    :is="door ? 'button' : 'div'"
    v-else
    class="strata"
    :type="door ? 'button' : undefined"
    :inert="door ? undefined : true"
    :data-act="door ? 'shelf' : undefined"
    :aria-label="door ? 'Recent fights' : undefined"
    @click="door && pick($event)"
  >
    <template v-for="(f, i) in shown" :key="f.id">
      <span class="lbl">{{ i === 0 ? "RECENT FIGHTS" : "" }}</span>
      <span class="layer" :class="{ on: f.id === framedId }" :data-fight="f.n"
        ><span v-for="s in segs(f)" :key="s.k" class="seg" :class="s.c" :style="{ flexGrow: String(s.v) }"
      /></span>
      <span class="tag" :class="{ on: f.id === framedId }">{{ labelled(f, i) ? tagText(f) : "" }}</span>
    </template>
  </component>
</template>
