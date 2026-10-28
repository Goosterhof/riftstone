<script setup lang="ts">
// §20.13, narrowed by the contract — the Gift: put back what Tamsin placed.
// Planned once per departed run. The sentence is the stone's own promise,
// shown verbatim; `remove` is what the press will uninstall, newest first;
// `review` is what the run touched that was not its own, worded by what the
// ledger can say, and never acted on here.
import { computed, ref, watch } from "vue";
import type { PutBackPlan } from "@/types/workshop";
import RiftButton from "@/components/RiftButton.vue";
import { useToasts } from "@/composables/useToasts";
import { useWorkshop } from "./useWorkshop";
import { KNOWN_WORD, PAWN_NAME, clock } from "./words";

const w = useWorkshop();
const plan = ref<PutBackPlan | null>(null);
const kept = ref(false);
const putting = ref(false);
const refusal = ref<string | null>(null);
let plannedFor: string | null = null;

const ended = computed(() => w.pawn.value?.state === "departed" || w.pawn.value?.state === "torn");
const shown = computed(
  () => !kept.value && !!plan.value && (plan.value.remove.length > 0 || plan.value.review.length > 0),
);

async function replan(): Promise<void> {
  plan.value = await w.planPutBack();
}

watch(
  () => [ended.value, w.runId.value ?? w.pawn.value?.runId ?? null] as const,
  ([gone, run]) => {
    if (!gone || !run || plannedFor === run) return;
    plannedFor = run;
    kept.value = false;
    refusal.value = null;
    void replan();
  },
  { immediate: true },
);

async function putBack(): Promise<void> {
  const current = plan.value;
  if (!current) return;
  putting.value = true;
  refusal.value = null;
  const done = await w.putBack(current);
  putting.value = false;
  if (!done) {
    refusal.value = w.failure.value?.detail.detail ?? w.failure.value?.message ?? null;
    return;
  }
  const n = current.remove.length;
  useToasts().push(
    "rift",
    `${n === 1 ? "The install" : `The ${n} installs`} ${PAWN_NAME} made ${n === 1 ? "is" : "are"} out of your world.`,
  );
  await replan();
}
</script>

<template>
  <section v-if="shown && plan" class="putback" aria-label="Put back what Tamsin placed">
    <p class="putback__sentence">{{ plan.sentence }}</p>
    <ul v-if="plan.remove.length" class="putback__list">
      <li v-for="r in plan.remove" :key="r.optionId" class="putback__remove">
        <span class="putback__glyph rs-mono" aria-hidden="true">↺</span>{{ r.name }}
      </li>
    </ul>
    <ul v-if="plan.review.length" class="putback__list">
      <li v-for="r in plan.review" :key="r.installId" class="putback__review">
        <span class="putback__name">{{ r.name }}</span>
        <span class="putback__whose rs-mono">
          {{ KNOWN_WORD[r.known] }} · {{ clock(r.at) }} ·
          {{ r.undo === "available" ? "undo available" : "reinstall it from the Rift" }}
        </span>
      </li>
    </ul>
    <div class="putback__doors">
      <RiftButton v-if="plan.remove.length" variant="danger" size="sm" :busy="putting" @click="putBack">
        Put them back
      </RiftButton>
      <RiftButton size="sm" @click="kept = true">Keep them</RiftButton>
    </div>
    <p v-if="refusal" class="putback__refused" role="alert">{{ refusal }}</p>
  </section>
</template>

<style scoped>
.putback {
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-2);
  padding-top: var(--rs-space-3);
  border-top: 1px solid var(--rs-edge);
}
.putback__sentence {
  margin: 0;
  max-width: 64ch;
  color: var(--rs-text);
  font: 400 var(--rs-text-sm) / 1.45 var(--rs-font-sans);
}
.putback__list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: 0;
  padding: 0;
  list-style: none;
}
.putback__remove {
  color: var(--rs-world);
  font: 500 var(--rs-text-sm) var(--rs-font-sans);
}
.putback__glyph {
  display: inline-block;
  width: 2ch;
  color: var(--rs-muted);
  font-size: var(--rs-text-sm);
}
.putback__review {
  display: flex;
  flex-wrap: wrap;
  gap: 0 var(--rs-space-2);
  padding-left: 2ch;
}
.putback__name {
  color: var(--rs-muted);
  font: 500 var(--rs-text-sm) var(--rs-font-sans);
}
.putback__whose {
  color: var(--rs-faint);
}
.putback__doors {
  display: flex;
  gap: var(--rs-space-2);
}
.putback__refused {
  margin: 0;
  color: var(--rs-ember);
  font: 400 var(--rs-text-sm) var(--rs-font-sans);
}
</style>
