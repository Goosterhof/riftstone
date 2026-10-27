<script setup lang="ts">
// S3c, a member's seven parts (D3, RF.20): ADDS UP draws bars and figures only
// where additivity is established; a mix (or unknown) is a framed well with
// muted percentages of these hits' parts, never an amount.
import { computed } from "vue";
import type { FrayMember, FrayTypes } from "@/types/fray";
import { nf, typeCells, verdictOf } from "../reading";

const { member, types } = defineProps<{ member: FrayMember; types: FrayTypes }>();
const add = computed(() => verdictOf(types) === "additive" && member.parts !== null);
const cells = computed(() => typeCells(member, add.value));
const label = computed(() =>
  add.value
    ? "damage types, adding up to the member's damage"
    : "component mix: each part's share of these hits' parts, not of the damage",
);
</script>

<template>
  <div class="types" :class="{ mixwell: !add }" :aria-label="label">
    <div v-for="c in cells" :key="c.label" class="ty" :class="{ dash: c.dash, mix: !add }">
      <span class="t">{{ c.label }}</span>
      <span v-if="add" class="mini" aria-hidden="true"
        ><i v-if="!c.dash" class="fill" :style="{ '--s': c.s.toFixed(4) }"
      /></span>
      <span class="p">{{ c.pct }}</span>
      <span v-if="add && !c.dash" class="d">{{ c.damage }}</span>
    </div>
    <div v-if="add" class="ty say"><b>ADDS UP</b>{{ ` · ${nf(member.damage)}` }}</div>
    <div v-else class="ty say"><b>component mix</b></div>
  </div>
</template>
