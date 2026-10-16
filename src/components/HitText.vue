<script setup lang="ts">
// A field with every query token lit (colour + underline, §7.3).
import { computed } from "vue";
import { highlightRuns } from "@/services/highlight";

const { text, tokens = [] } = defineProps<{ text: string; tokens?: string[] }>();
const runs = computed(() => highlightRuns(text, tokens));
</script>

<template>
  <template v-for="(run, i) in runs" :key="i"
    ><mark v-if="run.hit" class="rs-hit">{{ run.text }}</mark
    ><template v-else>{{ run.text }}</template></template
  >
</template>
