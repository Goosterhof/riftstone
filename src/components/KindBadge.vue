<script setup lang="ts">
// §7.5 — kind badges are neutral (bone outline). Colour is reserved for state.
import type { DeployKind } from "@/types/rift";
import { KIND_DESTINATION, KIND_LABEL } from "@/services/format";

const { kind, count = null } = defineProps<{ kind: DeployKind; count?: number | null }>();
</script>

<template>
  <span class="rs-kind" :title="KIND_DESTINATION[kind]"
    >{{ KIND_LABEL[kind] }}<span v-if="count !== null" class="rs-kind__n">{{ count }}</span></span
  >
</template>

<style scoped>
.rs-kind {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 1px 5px;
  border: 1px solid var(--rs-edgehi);
  border-radius: var(--rs-radius-chip);
  color: var(--rs-muted);
  font: 600 var(--rs-text-xs) / 1.4 var(--rs-font-mono);
  letter-spacing: 0.04em;
  white-space: nowrap;
}
.rs-kind__n {
  color: var(--rs-faint);
  font-weight: 400;
}
</style>
