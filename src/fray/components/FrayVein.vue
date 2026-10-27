<script setup lang="ts">
// THE VEIN: every source on one axis, hAll (FQ.23). Segments in fixed party
// order, then the world, then the unattributed. Lit while the fight runs;
// carved in the record, the shares cut into it (D10). The change from lit to
// carved on the same element is THE SETTLING: one property, --fr-lit 1 -> 0.
import { computed } from "vue";
import type { FrayFight } from "@/types/fray";
import { isWarm, share } from "../reading";

const { fight, mode } = defineProps<{ fight: FrayFight; mode: "lit" | "stale" | "carved" }>();

interface Seg {
  k: string;
  c: string;
  v: number;
  insc: string;
}

const segs = computed<Seg[]>(() => {
  const f = fight;
  const out: Seg[] = [];
  const carved = mode === "carved";
  for (const m of f.members) {
    if (!m || m.damage <= 0) continue;
    const v = share(m.damage, f.hAll);
    out.push({
      k: `m${m.slot}`,
      c: isWarm(m) ? "warm" : "cold",
      v,
      insc: carved && v >= 0.06 ? String(Math.round(v * 100)) : "",
    });
  }
  if (f.world && f.world.damage > 0) out.push({ k: "w", c: "wseg", v: share(f.world.damage, f.hAll), insc: "" });
  if (f.unattributed.damage > 0) out.push({ k: "u", c: "nob", v: share(f.unattributed.damage, f.hAll), insc: "" });
  return out;
});
</script>

<template>
  <div class="vein" :class="mode === 'lit' ? '' : mode" aria-hidden="true">
    <span v-for="s in segs" :key="s.k" class="seg" :class="s.c" :data-k="s.k" :style="{ flexGrow: s.v.toFixed(4) }"
      ><i v-if="s.insc">{{ s.insc }}</i></span
    >
  </div>
</template>
