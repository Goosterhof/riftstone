<script setup lang="ts">
// "Used without damage": the eye's evidence of a support member's casts
// (P2.6), never a share.
import { computed } from "vue";

const { casts, max, record } = defineProps<{
  casts: { name: string; count: number }[];
  max: number;
  record?: boolean;
}>();
const shown = computed(() => casts.slice(0, max));
const more = computed(() => casts.length - max);
</script>

<template>
  <span>{{ "Used without damage: " }}</span>
  <template v-for="(c, i) in shown" :key="c.name">
    <span v-if="i > 0">{{ " · " }}</span>
    <span :class="record ? 'txt' : 's'">{{ c.name }}</span
    >{{ " " }}<span :class="record ? 'v mono b6 txt' : 'v'">{{ `×${c.count}` }}</span>
  </template>
  <span v-if="more > 0">{{ ` · +${more} more` }}</span>
</template>
