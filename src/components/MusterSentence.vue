<script setup lang="ts">
// One of the stone's sentences, its `file` spans set in mono (the slot's own
// file name, which the failure sentences use so the user can match them).
import { computed } from "vue";

const { text } = defineProps<{ text: string }>();
const parts = computed(() => text.split("`").map((part, i) => ({ part, code: i % 2 === 1 })));
</script>

<template>
  <span
    ><template v-for="(p, i) in parts" :key="i"
      ><code v-if="p.code" class="rs-mono sentence__file">{{ p.part }}</code
      ><template v-else>{{ p.part }}</template></template
    ></span
  >
</template>

<style scoped>
.sentence__file {
  font-size: var(--rs-text-xs);
  color: var(--rs-text);
}
</style>
