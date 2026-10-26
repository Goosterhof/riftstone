<script setup lang="ts">
// One share row on the shared axis (§19.13): the bar and its label, every
// share a part of hAll. While hAll is 0 the share has no value, so the label
// is the damage alone (P3c.3).
import { computed } from "vue";
import { nf, pct, share } from "../reading";

const { part, whole, approx, barClass } = defineProps<{
  part: number;
  whole: number;
  approx?: boolean;
  barClass?: string;
}>();
const v = computed(() => share(part, whole));
</script>

<template>
  <div class="share">
    <span class="bar" :class="barClass" aria-hidden="true"><i class="fill" :style="{ '--s': v.toFixed(4) }" /></span>
    <span class="lab">
      <span v-if="approx" class="approx">{{ "≈ " }}</span>
      <template v-if="whole > 0">
        <span class="pc">{{ pct(v) }}</span
        >{{ " " }}<span class="dm">{{ `· ${nf(part)}` }}</span>
      </template>
      <span v-else class="dm">{{ nf(part) }}</span>
    </span>
  </div>
</template>
