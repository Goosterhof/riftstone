<script setup lang="ts">
// The RESERVED remainder row's words (FR-D2): Other and Unknown, never pushed off.
import { computed } from "vue";
import { remainderParts, type Remainder } from "../reading";

const { rem } = defineProps<{ rem: Remainder }>();
const parts = computed(() => remainderParts(rem));
</script>

<template>
  <template v-for="(p, i) in parts" :key="p.word">
    <span v-if="i > 0">{{ "  ·  " }}</span
    >{{ `${p.word} ` }}<span class="v">{{ p.v }}</span>
  </template>
</template>
