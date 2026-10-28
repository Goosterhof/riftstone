<script setup lang="ts">
// R20.7 — BRING IT THROUGH, in THE CROSSING's idiom with the Rift's
// temperature: the rows sit on mist, because they land in the Rift, and the
// confirm is Primary, never gold. The digest is frozen when the card opens and
// it is the one the press carries; it is carved whole, sixteen fours in a
// tablet, because those exact bytes are the promise. THE RECEIPT SAYS is the
// pawn's word about itself: muted, attributed, and never ticked (Bold Choice 26).
import { computed, nextTick, onMounted, ref } from "vue";
import type { BringThroughPlan } from "@/types/workshop";
import RiftButton from "@/components/RiftButton.vue";
import type { RiftFailure } from "@/services/rift";
import { useToasts } from "@/composables/useToasts";
import { useWorkshop } from "./useWorkshop";
import { clock, quads, size } from "./words";

const { plan: opened } = defineProps<{ plan: BringThroughPlan }>();
const emit = defineEmits<{ close: [] }>();

const w = useWorkshop();
const plan = ref<BringThroughPlan>(opened);
type Phase = "review" | "bringing" | "landed" | "alreadyHad" | "bytesChanged" | "nameTaken" | "refused";
const phase = ref<Phase>(opened.conflict === "nameTaken" ? "nameTaken" : "review");
/** The stone's own sentence for the last refusal, verbatim. */
const said = ref<string | null>(null);
const card = ref<HTMLElement | null>(null);

const name = computed(() => plan.value.artifact);
const carved = computed<string | null>(() => {
  if (phase.value === "landed") return "IN THE RIFT";
  if (phase.value === "bytesChanged") return "THE BYTES CHANGED";
  if (phase.value === "nameTaken") return "THE NAME IS TAKEN";
  return null;
});
const conflictWord = computed(() => {
  if (plan.value.conflict === "alreadyHad") return "already there";
  if (plan.value.conflict === "nameTaken") return "name taken";
  return "new";
});

async function press(): Promise<void> {
  phase.value = "bringing";
  said.value = null;
  const outcome = await w.bring(plan.value);
  if (outcome === "landed") {
    phase.value = "landed";
    useToasts().push("rift", `${name.value} is in your mods folder. Install it from the Rift.`);
    return;
  }
  if (outcome === "alreadyHad") {
    phase.value = "alreadyHad";
    return;
  }
  refusedBy(w.failure.value);
}

const REFUSED_PHASE: Partial<Record<string, Phase>> = { bytesChanged: "bytesChanged", nameTaken: "nameTaken" };

function refusedBy(failure: RiftFailure | null): void {
  said.value = failure?.detail.detail ?? failure?.message ?? null;
  const reason = failure?.kind === "bring" ? failure.detail.reason : "";
  phase.value = REFUSED_PHASE[reason] ?? "refused";
}

/** Review it again: a new card on the bytes on disk now, frozen anew. */
async function again(): Promise<void> {
  const next = await w.planBring(name.value);
  if (!next) {
    said.value = w.failure.value?.detail.detail ?? w.failure.value?.message ?? null;
    phase.value = "refused";
    return;
  }
  plan.value = next;
  said.value = null;
  phase.value = next.conflict === "nameTaken" ? "nameTaken" : "review";
}

onMounted(async () => {
  await nextTick();
  card.value?.focus({ preventScroll: false });
});
</script>

<template>
  <section
    ref="card"
    class="bring"
    :class="`is-${phase}`"
    tabindex="-1"
    aria-label="Bring it through"
    @keydown.esc.stop="emit('close')"
  >
    <p class="bring__inscription rs-inscription">BRING IT THROUGH</p>
    <p v-if="carved" class="bring__state rs-inscription" role="status">{{ carved }}</p>
    <p v-else class="bring__promise">The stone will add exactly these bytes to your mods folder, and nothing else.</p>

    <div class="bring__rows">
      <div class="bring__row">
        <p class="bring__lands">
          <span class="bring__plus" aria-hidden="true">+</span>
          <span class="bring__path">{{ plan.landsAs }}</span>
        </p>
        <p class="bring__meta rs-mono">
          <span :class="{ 'is-ember': plan.conflict === 'nameTaken' }">{{ conflictWord }}</span>
          · {{ size(plan.bytes) }}
        </p>
        <div class="bring__tablet rs-mono" :aria-label="`sha256 ${plan.sha256}`" data-digest>
          <span v-for="(q, i) in quads(plan.sha256)" :key="i">{{ q }}</span>
        </div>
        <p class="bring__stays rs-mono">· the workshop copy stays where it is</p>
      </div>
      <div class="bring__receipt">
        <p class="bring__says rs-mono">THE RECEIPT SAYS</p>
        <dl class="bring__claims rs-mono">
          <dt>built</dt>
          <dd>{{ clock(plan.receipt.builtAt) }}</dd>
          <template v-for="input in plan.receipt.inputs" :key="input.path">
            <dt>input</dt>
            <dd>{{ input.path }} {{ input.sha256.slice(0, 16) }}…</dd>
          </template>
          <template v-for="check in plan.receipt.checks" :key="check.name">
            <dt>check</dt>
            <dd>{{ check.name }} {{ check.passed }}/{{ check.total }}</dd>
          </template>
        </dl>
      </div>
    </div>

    <p v-if="phase === 'alreadyHad'" class="bring__note">
      The same bytes are already in your mods folder as {{ name }}. The stone added nothing new.
    </p>
    <p v-else-if="phase === 'nameTaken'" class="bring__note bring__note--ember">
      Your mods folder already holds a different {{ name }}. The stone never writes over a file there, so nothing was
      added. Rename the build in the workshop, or put the older one away first.
    </p>
    <template v-else-if="phase === 'bytesChanged'">
      <p class="bring__note bring__note--ember">{{ said }}</p>
      <p class="bring__note">Nothing was added.</p>
    </template>
    <p v-else-if="phase === 'refused'" class="bring__note bring__note--ember">{{ said }}</p>
    <p v-else-if="phase === 'landed'" class="bring__note">Install it from the Rift.</p>

    <div class="bring__doors">
      <template v-if="phase === 'review' || phase === 'bringing'">
        <RiftButton variant="primary" size="sm" :busy="phase === 'bringing'" @click="press"
          >Add to library ▶</RiftButton
        >
        <RiftButton size="sm" @click="emit('close')">Cancel</RiftButton>
      </template>
      <template v-else-if="phase === 'bytesChanged'">
        <RiftButton variant="primary" size="sm" @click="again">Review it again</RiftButton>
        <RiftButton size="sm" @click="emit('close')">Cancel</RiftButton>
      </template>
      <RiftButton v-else-if="phase === 'nameTaken' || phase === 'refused'" size="sm" @click="emit('close')">
        Cancel
      </RiftButton>
      <RiftButton v-else size="sm" @click="emit('close')">Close</RiftButton>
    </div>
  </section>
</template>

<style scoped>
.bring {
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-2);
  margin-top: var(--rs-space-2);
  padding: var(--rs-space-3);
  border: 1px solid var(--rs-edgehi);
  background: var(--rs-basalt);
  outline-offset: 0;
}
.bring__inscription {
  margin: 0;
  color: var(--rs-text);
  font-size: var(--rs-text-md);
  line-height: 1.1;
}
.bring__state {
  margin: 0;
  color: var(--rs-ember);
  font-size: var(--rs-text-sm);
}
.bring.is-landed .bring__state {
  color: var(--rs-rift);
}
.bring__promise,
.bring__note {
  margin: 0;
  color: var(--rs-muted);
  font: 400 var(--rs-text-sm) / 1.4 var(--rs-font-sans);
}
.bring__note--ember {
  color: var(--rs-ember);
}
.bring__rows {
  display: flex;
  flex-direction: column;
  background: var(--rs-mist);
}
.bring__row {
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-1);
  padding: var(--rs-space-3);
}
.bring__lands {
  display: flex;
  gap: var(--rs-space-2);
  margin: 0;
  color: var(--rs-text);
  font: 500 var(--rs-text-sm) var(--rs-font-sans);
  overflow-wrap: anywhere;
}
.bring__plus {
  color: var(--rs-rift);
  font-family: var(--rs-font-mono);
}
.bring__meta,
.bring__stays {
  margin: 0;
  color: var(--rs-faint);
}
.is-ember {
  color: var(--rs-ember);
}
/* the digest tablet: the exact bytes, carved whole in fours, four by four */
.bring__tablet {
  display: grid;
  grid-template-columns: repeat(4, max-content);
  gap: 2px var(--rs-space-3);
  margin: var(--rs-space-1) 0;
  padding: var(--rs-space-2) var(--rs-space-3);
  width: max-content;
  background: var(--rs-void);
  box-shadow: inset 0 2px 0 rgba(0, 0, 0, 0.45);
  color: var(--rs-text);
  letter-spacing: 0.06em;
}
.bring__receipt {
  padding: var(--rs-space-3);
  border-top: 1px solid var(--rs-edge);
}
.bring__says {
  margin: 0 0 var(--rs-space-1);
  color: var(--rs-muted);
  letter-spacing: 0.12em;
}
.bring__claims {
  display: grid;
  grid-template-columns: max-content minmax(0, 1fr);
  gap: 2px var(--rs-space-3);
  margin: 0;
  color: var(--rs-muted);
}
.bring__claims dt {
  color: var(--rs-faint);
}
.bring__claims dd {
  margin: 0;
  overflow-wrap: anywhere;
}
.bring__doors {
  display: flex;
  flex-wrap: wrap;
  gap: var(--rs-space-2);
}
</style>
