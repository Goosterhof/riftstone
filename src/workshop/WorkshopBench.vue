<script setup lang="ts">
// R20.5 — THE BENCH, top to bottom: VOICE, INPUTS, BUILT, THE PAWN'S HANDS.
// Sections separated by space and hairlines, never nested cards. INPUTS
// replaces WORKSHOP FILES (the contract lists no workshop files): the
// read-only folders Tamsin may read, each checked by the stone on set.
import { computed, ref } from "vue";
import type { Voice } from "@/types/workshop";
import RiftButton from "@/components/RiftButton.vue";
import { rift } from "@/services/rift";
import BuiltArtifact from "./BuiltArtifact.vue";
import PawnHands from "./PawnHands.vue";
import { useWorkshop } from "./useWorkshop";
import { PAWN_NAME, VOICE_LATER } from "./words";

const w = useWorkshop();

const VOICES: { value: Voice; label: string }[] = [
  { value: "full", label: "Full" },
  { value: "sparing", label: "Sparing" },
  { value: "plain", label: "Plain" },
];

const inputRefusal = ref<string | null>(null);
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

const STEP: Partial<Record<string, number>> = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 };

function onVoiceKey(event: KeyboardEvent, index: number): void {
  const step = STEP[event.key];
  if (!step) return;
  event.preventDefault();
  const next = VOICES[(index + step + VOICES.length) % VOICES.length];
  void w.setVoice(next.value);
  const group = (event.currentTarget as HTMLElement).parentElement;
  group?.querySelectorAll<HTMLElement>("[role=radio]")[VOICES.indexOf(next)]?.focus();
}
</script>

<template>
  <div class="bench">
    <section class="bench__section" aria-labelledby="ws-voice">
      <h2 id="ws-voice" class="bench__head rs-inscription">VOICE</h2>
      <div
        class="dial"
        role="radiogroup"
        aria-labelledby="ws-voice"
        :title="w.summoned.value ? VOICE_LATER : undefined"
      >
        <button
          v-for="(v, i) in VOICES"
          :key="v.value"
          type="button"
          role="radio"
          class="dial__seg"
          :aria-checked="w.view.value?.voice === v.value"
          :tabindex="w.view.value?.voice === v.value ? 0 : -1"
          @click="w.setVoice(v.value)"
          @keydown="onVoiceKey($event, i)"
        >
          {{ v.label }}
        </button>
      </div>
      <p v-if="w.summoned.value" class="bench__note">{{ VOICE_LATER }}</p>
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
.bench__refused {
  margin: 0;
  color: var(--rs-ember);
  font: 400 var(--rs-text-sm) / 1.4 var(--rs-font-sans);
}
.dial {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  border: 1px solid var(--rs-edgehi);
}
.dial__seg {
  height: 32px;
  border: 0;
  border-left: 1px solid var(--rs-edge);
  background: transparent;
  color: var(--rs-muted);
  font: 500 var(--rs-text-sm) var(--rs-font-sans);
  cursor: pointer;
}
.dial__seg:first-child {
  border-left: 0;
}
.dial__seg:hover {
  background: var(--rs-misthi);
}
.dial__seg[aria-checked="true"] {
  background: var(--rs-well);
  box-shadow: inset 0 2px 0 rgba(0, 0, 0, 0.45);
  color: var(--rs-text);
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
