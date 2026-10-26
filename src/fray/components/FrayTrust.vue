<script setup lang="ts">
// The trust band (D4, RF.14), two lines always: the honesty line (hParty
// against the game's own party ledger, nothing else), then one slot by
// priority.
import { computed } from "vue";
import type { FrayFight } from "@/types/fray";
import { trustLines } from "../reading";

const { fight } = defineProps<{ fight: FrayFight }>();
const lines = computed(() => trustLines(fight));
</script>

<template>
  <div class="trust">
    <div v-for="(line, i) in lines" :key="i">
      <span v-for="(p, j) in line" :key="j" :class="{ n: p.num, w: p.warn }">{{ p.text }}</span>
    </div>
  </div>
</template>
