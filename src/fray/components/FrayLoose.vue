<script setup lang="ts">
// What no member is credited with, on the same axis: the unattributed row
// (no one's credit) and, only on evidence, the world (P3.8).
import { computed } from "vue";
import type { FrayFight } from "@/types/fray";
import { nf, pct, share } from "../reading";
import FrayShare from "./FrayShare.vue";

const { fight } = defineProps<{ fight: FrayFight }>();
const u = computed(() => fight.unattributed.damage);
const uLabel = computed(
  () =>
    `Unattributed: ${fight.hAll > 0 ? `${pct(share(u.value, fight.hAll))}, ` : ""}${nf(u.value)} damage, no one's credit`,
);
const w = computed(() => fight.world?.damage ?? 0);
const wv = computed(() => share(w.value, fight.hAll));
</script>

<template>
  <div class="unrow" :aria-label="uLabel">
    <div class="t">
      Unattributed<span class="faint">{{ " · no one's credit" }}</span>
    </div>
    <FrayShare :part="u" :whole="fight.hAll" bar-class="nob" />
  </div>
  <div
    v-if="w > 0"
    class="worldrow"
    :aria-label="`The world: ${Math.round(wv * 100)} percent, ${nf(w)} damage, from traps and the place itself`"
  >
    <div class="wslab">
      <div class="wl">
        <div class="t">
          The world<span class="faint">{{ " · traps and the place itself" }}</span>
        </div>
        <span class="bar wld thin" aria-hidden="true"><i class="fill" :style="{ '--s': wv.toFixed(4) }" /></span>
      </div>
      <span class="lab"
        ><span class="pc">{{ pct(wv) }}</span
        >{{ " " }}<span class="dm">{{ `· ${nf(w)}` }}</span></span
      >
    </div>
  </div>
</template>
