<script setup lang="ts">
// A voiced failure (services/voice.ts) with its recovery buttons. The
// component never writes copy; it shows what the voice module said.
import type { RecoveryKind, Voiced } from "@/services/voice";
import RiftButton from "./RiftButton.vue";
import RiftInscription from "./RiftInscription.vue";

const { voiced, size = "md", busy = false } = defineProps<{ voiced: Voiced; size?: "lg" | "md"; busy?: boolean }>();
const emit = defineEmits<{ recover: [kind: RecoveryKind] }>();
</script>

<template>
  <RiftInscription
    :inscription="voiced.inscription"
    :body="`${voiced.voice} ${voiced.empathy}`"
    :tone="voiced.kind === 'nexus' && !voiced.inscription?.includes('REFUSES') ? 'warn' : 'ember'"
    :size="size"
  >
    <RiftButton
      v-for="(r, i) in voiced.recovery"
      :key="r.kind"
      :variant="i === 0 ? 'primary' : 'secondary'"
      :busy="busy && i === 0"
      @click="emit('recover', r.kind)"
      >{{ r.label }}</RiftButton
    >
  </RiftInscription>
</template>
