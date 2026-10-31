<script setup lang="ts">
// R20.5 — THE BENCH, top to bottom: SUMMONED WITH, INPUTS, BUILT, THE PAWN'S HANDS.
// SUMMONED WITH (0.7.2) replaced VOICE: she always speaks in the full voice
// (investor, 2026-10-31), and the bench now carries what her Claude Code
// forgot between summons: its mode, effort and model.
// Sections separated by space and hairlines, never nested cards. INPUTS
// replaces WORKSHOP FILES (the contract lists no workshop files): the
// read-only folders Tamsin may read, each checked by the stone on set.
import { computed, ref } from "vue";
import RiftButton from "@/components/RiftButton.vue";
import { rift } from "@/services/rift";
import BenchDial from "./BenchDial.vue";
import BuiltArtifact from "./BuiltArtifact.vue";
import PawnHands from "./PawnHands.vue";
import { useWorkshop } from "./useWorkshop";
import { EFFORT_CHOICES, MODE_CHOICES, MODEL_CHOICES, PAWN_NAME, SESSION_LATER, SESSION_NOTE } from "./words";

const w = useWorkshop();

const inputRefusal = ref<string | null>(null);
const sessionRefusal = ref<string | null>(null);
const session = computed(() => w.view.value?.session);
const later = computed(() => (w.summoned.value ? SESSION_LATER : undefined));

async function setSession(part: Parameters<typeof w.setSession>[0]): Promise<void> {
  sessionRefusal.value = null;
  const ok = await w.setSession(part);
  if (!ok) sessionRefusal.value = w.failure.value?.detail.detail ?? w.failure.value?.message ?? null;
}
const picking = ref(false);

/** Receipted builds first; the ones the stone will not bring are listed under them, faint. */
const built = computed(() => {
  const all = w.view.value?.artifacts ?? [];
  return [...all.filter((a) => !a.receiptProblem), ...all.filter((a) => a.receiptProblem)];
});
const inputs = computed(() => w.view.value?.inputs ?? []);

async function setInputs(next: string[]): Promise<void> {
  inputRefusal.value = null;
  const ok = await w.setInputs(next);
  if (!ok) inputRefusal.value = w.failure.value?.detail.detail ?? w.failure.value?.message ?? null;
}

async function addFolder(): Promise<void> {
  picking.value = true;
  try {
    const path = await (await rift()).pickFolder(`A folder ${PAWN_NAME} may read`);
    if (path && !inputs.value.includes(path)) await setInputs([...inputs.value, path]);
  } finally {
    picking.value = false;
  }
}
</script>

<template>
  <div class="bench">
    <section class="bench__section" aria-labelledby="ws-session">
      <h2 id="ws-session" class="bench__head rs-inscription">SUMMONED WITH</h2>
      <BenchDial
        id="ws-mode"
        name="MODE"
        :choices="MODE_CHOICES"
        :chosen="session?.mode"
        :title="later"
        @pick="setSession({ mode: $event })"
      />
      <BenchDial
        id="ws-effort"
        name="EFFORT"
        :choices="EFFORT_CHOICES"
        :chosen="session?.effort"
        :title="later"
        @pick="setSession({ effort: $event })"
      />
      <BenchDial
        id="ws-model"
        name="MODEL"
        :choices="MODEL_CHOICES"
        :chosen="session?.model"
        :title="later"
        @pick="setSession({ model: $event })"
      />
      <p v-if="sessionRefusal" class="bench__refused" role="alert">{{ sessionRefusal }}</p>
      <p v-if="w.summoned.value" class="bench__note bench__note--now">{{ SESSION_LATER }}</p>
      <p class="bench__note">{{ SESSION_NOTE }}</p>
    </section>

    <section class="bench__section" aria-labelledby="ws-inputs">
      <h2 id="ws-inputs" class="bench__head rs-inscription">INPUTS</h2>
      <p v-if="!inputs.length" class="bench__note">{{ PAWN_NAME }} reads no folder outside the workshop.</p>
      <ul v-else class="inputs">
        <li v-for="path in inputs" :key="path" class="inputs__row">
          <span class="inputs__path rs-mono" :title="path">{{ path }}</span>
          <button
            type="button"
            class="inputs__remove"
            :aria-label="`Stop ${PAWN_NAME} reading ${path}`"
            @click="setInputs(inputs.filter((p) => p !== path))"
          >
            ✕
          </button>
        </li>
      </ul>
      <div>
        <RiftButton size="sm" :busy="picking" @click="addFolder">Add a folder…</RiftButton>
      </div>
      <p v-if="inputRefusal" class="bench__refused" role="alert">{{ inputRefusal }}</p>
    </section>

    <section class="bench__section" aria-labelledby="ws-built">
      <h2 id="ws-built" class="bench__head rs-inscription">BUILT</h2>
      <p v-if="!built.length" class="bench__note">
        Nothing is built yet. A finished build is a ZIP with its receipt beside it.
      </p>
      <ul v-else class="built-list">
        <BuiltArtifact v-for="a in built" :key="a.name" :artifact="a" />
      </ul>
    </section>

    <section class="bench__section" aria-labelledby="ws-hands">
      <h2 id="ws-hands" class="bench__head rs-inscription">THE PAWN'S HANDS</h2>
      <p v-if="!w.hands.value.length" class="bench__note">The stone has answered nothing for {{ PAWN_NAME }} yet.</p>
      <PawnHands v-else />
    </section>
  </div>
</template>

<style scoped>
.bench {
  display: flex;
  flex-direction: column;
}
.bench__section {
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-2);
  padding: var(--rs-space-4) var(--rs-space-5);
  border-bottom: 1px solid var(--rs-edge);
}
.bench__section:last-child {
  border-bottom: 0;
}
.bench__head {
  margin: 0;
  color: var(--rs-muted);
  font-size: 14px;
}
.bench__note {
  margin: 0;
  color: var(--rs-faint);
  font: 400 var(--rs-text-sm) / 1.4 var(--rs-font-sans);
}
.bench__note--now {
  color: var(--rs-muted);
}
.bench__refused {
  margin: 0;
  color: var(--rs-ember);
  font: 400 var(--rs-text-sm) / 1.4 var(--rs-font-sans);
}
.inputs,
.built-list {
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-2);
  margin: 0;
  padding: 0;
  list-style: none;
}
.inputs__row {
  display: flex;
  align-items: center;
  gap: var(--rs-space-2);
}
.inputs__path {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--rs-text);
}
.inputs__remove {
  flex: none;
  width: 28px;
  height: 28px;
  border: 1px solid var(--rs-edgehi);
  background: none;
  color: var(--rs-muted);
  cursor: pointer;
}
.inputs__remove:hover {
  background: var(--rs-misthi);
  color: var(--rs-text);
}
</style>
