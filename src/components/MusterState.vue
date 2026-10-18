<script setup lang="ts">
// A state of THE MUSTER (#00015 R17.22–R17.23): an inscription, the stone's
// body, and its doors (the first Primary sm, the rest Secondary sm; none is
// Danger or World: the Muster changes nothing). The copy comes from
// services/voice.ts or services/muster.ts; this component composes none.
import type { RecoveryKind } from "@/services/voice";
import RiftButton from "./RiftButton.vue";
import MusterSentence from "./MusterSentence.vue";

const {
  inscription,
  body,
  doors,
  rail = "none",
  scale = "slot",
  busy = false,
} = defineProps<{
  inscription: string | null;
  body: string;
  doors: { kind: RecoveryKind | "lookAgain"; label: string }[];
  rail?: "none" | "warn" | "ember" | "faint";
  /** `page`: a page-level problem (Cinzel 2xl); `slot`: a slot's (Cinzel xl). */
  scale?: "page" | "slot";
  busy?: boolean;
}>();

const emit = defineEmits<{ door: [kind: RecoveryKind | "lookAgain"] }>();
</script>

<template>
  <section
    class="mstate"
    :class="[`mstate--${scale}`, `mstate--${rail}`]"
    :role="rail === 'ember' ? 'alert' : 'status'"
    :aria-busy="busy ? 'true' : undefined"
  >
    <h2 v-if="inscription" class="mstate__carve rs-inscription">{{ inscription }}</h2>
    <p class="mstate__body"><MusterSentence :text="body" /></p>
    <div v-if="doors.length" class="mstate__doors">
      <RiftButton
        v-for="(d, i) in doors"
        :key="d.kind"
        size="sm"
        :variant="i === 0 ? 'primary' : 'secondary'"
        @click="emit('door', d.kind)"
        >{{ d.label }}</RiftButton
      >
    </div>
  </section>
</template>

<style scoped>
.mstate {
  max-width: 560px;
  padding: var(--rs-space-1) 0 var(--rs-space-1) var(--rs-space-5);
  border-left: 4px solid transparent;
}
.mstate--warn {
  border-left-color: var(--rs-warn);
}
.mstate--ember {
  border-left-color: var(--rs-ember);
}
.mstate--faint {
  border-left-color: var(--rs-faint);
}
.mstate--page {
  padding-left: 0;
  border-left: 0;
}
.mstate__carve {
  margin: 0 0 var(--rs-space-3);
  font-size: var(--rs-text-xl);
  line-height: 1.15;
  color: var(--rs-text);
}
.mstate--page .mstate__carve {
  font-size: var(--rs-text-2xl);
}
.mstate__body {
  margin: 0;
  color: var(--rs-muted);
  line-height: 1.5;
}
.mstate__doors {
  display: flex;
  flex-wrap: wrap;
  gap: var(--rs-space-2);
  margin-top: var(--rs-space-4);
}
</style>
