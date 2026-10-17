<script setup lang="ts">
// R16.22 — Track / Untrack, Endorse / Abstain: plain words, toggle buttons
// with aria-pressed, no hearts, no stars, no counts. A refusal Nexus gives
// (endorseNotDownloaded, endorseTooSoon, endorseOwnMod …) is the §9 sentence
// under the pair, never a toast. Shown only with a connected key.
import { computed } from "vue";
import { useNexusAccount } from "@/composables/useNexusAccount";
import RiftButton from "./RiftButton.vue";

const {
  modId,
  name,
  track = true,
  endorse = true,
} = defineProps<{
  modId: number;
  name: string;
  track?: boolean;
  endorse?: boolean;
}>();

const nexus = useNexusAccount();

const tracked = computed(() => nexus.isTracked(modId));
const decided = computed(() => nexus.endorsementOf(modId));
const busy = computed(() => nexus.standingBusy.value === modId);
const refusal = computed(() => nexus.refusalOf(modId));
const decidedLine = computed(() => {
  if (decided.value === "endorsed") return "You endorsed this mod.";
  if (decided.value === "abstained") return "You abstained from endorsing this mod.";
  return null;
});
</script>

<template>
  <div v-if="nexus.connected.value" class="stand" :data-standing="modId">
    <div class="stand__acts" role="group" :aria-label="`Your standing on Nexus for ${name}`">
      <RiftButton
        v-if="track"
        size="sm"
        :aria-pressed="tracked"
        :busy="busy"
        :title="tracked ? 'Stop tracking' : `Track ${name} on Nexus`"
        @click="nexus.setTracked(modId, !tracked)"
        >{{ tracked ? "Tracked" : "Track" }}</RiftButton
      >
      <template v-if="endorse">
        <RiftButton
          size="sm"
          :aria-pressed="decided === 'endorsed'"
          :disabled="busy"
          reason="Nexus is answering your last press."
          @click="nexus.setEndorsement(modId, 'endorsed')"
          >Endorse</RiftButton
        >
        <RiftButton
          size="sm"
          :aria-pressed="decided === 'abstained'"
          :disabled="busy"
          reason="Nexus is answering your last press."
          @click="nexus.setEndorsement(modId, 'abstained')"
          >Abstain</RiftButton
        >
      </template>
    </div>
    <p v-if="refusal" class="stand__refusal" role="status">{{ refusal.voice }} {{ refusal.empathy }}</p>
    <p v-else-if="endorse && decidedLine" class="stand__line">{{ decidedLine }}</p>
  </div>
</template>

<style scoped>
.stand__acts {
  display: flex;
  flex-wrap: wrap;
  gap: var(--rs-space-2);
}
.stand__acts :deep(.rs-btn[aria-pressed="true"]) {
  border-color: var(--rs-rift);
  background: var(--rs-misthi);
  color: var(--rs-text);
}
.stand__line,
.stand__refusal {
  margin: 4px 0 0;
  font-size: var(--rs-text-sm);
}
.stand__line {
  color: var(--rs-faint);
}
.stand__refusal {
  color: var(--rs-muted);
}
</style>
