<script setup lang="ts">
// The lintel: health over stamina, live only. A reading the feed could not
// take is a dashed track (no reading, never an empty bar); the record keeps
// the lintel's 10 as padding (RF.19).
import { computed } from "vue";
import type { FrayMember } from "@/types/fray";

const LOW_HP = 0.25;
const { member, record } = defineProps<{ member: FrayMember | null; record: boolean }>();
const hp = computed(() => (member?.down ? 0 : (member?.hp ?? null)));
const st = computed(() => member?.st ?? null);
const low = computed(() => hp.value !== null && hp.value > 0 && hp.value < LOW_HP);
const label = computed(() =>
  hp.value === null ? "health didn't answer" : `health ${Math.round(hp.value * 100)} percent`,
);
</script>

<template>
  <div v-if="record || !member" class="lintel rec" />
  <div v-else class="lintel" :aria-label="label">
    <span class="track hp" :class="{ low, null: hp === null }"
      ><i v-if="hp !== null" class="fill" :style="{ '--s': hp.toFixed(4) }"
    /></span>
    <span class="track stm" :class="{ null: st === null }"
      ><i v-if="st !== null" class="fill" :style="{ '--s': st.toFixed(4) }"
    /></span>
  </div>
</template>
