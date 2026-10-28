<script setup lang="ts">
// R20.6 — one built ZIP on the bench. The digest is the stone's own hash of the
// bytes on disk now; the 4 px rail is edgehi until a file with those bytes is
// in the mods folder, then rift (Bold Choice 24: stone, then cold, then warm).
// A build with no receipt, an unreadable one or one naming other bytes is
// listed, faint, and gets no button.
import { computed, ref } from "vue";
import type { ArtifactView, BringThroughPlan } from "@/types/workshop";
import RiftButton from "@/components/RiftButton.vue";
import { useToasts } from "@/composables/useToasts";
import BringThroughCard from "./BringThroughCard.vue";
import { useWorkshop } from "./useWorkshop";
import { NO_RECEIPT_TITLE, RECEIPT_PROBLEM, clock, quads, size } from "./words";

const { artifact } = defineProps<{ artifact: ArtifactView }>();

const w = useWorkshop();
const plan = ref<BringThroughPlan | null>(null);
const opening = ref(false);
const refusal = ref<string | null>(null);

const hashing = computed(() => artifact.sha256 === null);
const problem = computed(() => (artifact.receiptProblem ? RECEIPT_PROBLEM[artifact.receiptProblem] : null));
const bringable = computed(() => !problem.value && !artifact.inLibrary && artifact.receipt !== null);

async function open(): Promise<void> {
  opening.value = true;
  refusal.value = null;
  const next = await w.planBring(artifact.name);
  opening.value = false;
  if (next) plan.value = next;
  else refusal.value = w.failure.value?.detail.detail ?? w.failure.value?.message ?? null;
}

async function copy(): Promise<void> {
  if (!artifact.sha256) return;
  try {
    await navigator.clipboard.writeText(artifact.sha256);
    useToasts().push("rift", "The digest is on your clipboard.");
  } catch {
    useToasts().push("warn", "The clipboard refused the digest.");
  }
}
</script>

<template>
  <li
    class="built"
    :class="{ 'is-faint': problem, 'is-in-rift': artifact.inLibrary }"
    :title="artifact.receiptProblem === 'missing' ? NO_RECEIPT_TITLE : undefined"
    :data-artifact="artifact.name"
  >
    <p class="built__name">
      <span class="built__file">{{ artifact.name }}</span>
      <span class="built__size rs-mono">{{ size(artifact.bytes) }}</span>
    </p>
    <p v-if="hashing" class="built__sha rs-mono" aria-busy="true">sha256 ····</p>
    <p v-else class="built__sha rs-mono">
      <span
        >sha256
        {{
          quads(artifact.sha256 ?? "")
            .slice(0, 4)
            .join(" ")
        }}…</span
      >
      <button type="button" class="built__copy" :aria-label="`Copy the full digest of ${artifact.name}`" @click="copy">
        <span class="built__copy-glyph" aria-hidden="true" />
      </button>
    </p>
    <p class="built__receipt rs-mono">
      <template v-if="problem">{{ problem }}</template>
      <template v-else-if="artifact.receipt">receipt · built {{ clock(artifact.receipt.builtAt) }}</template>
    </p>
    <p v-if="artifact.inLibrary" class="built__where">in your mods folder</p>
    <template v-else-if="bringable && !plan">
      <div class="built__door">
        <RiftButton
          variant="primary"
          size="sm"
          :busy="opening"
          :refused="hashing"
          :reason="hashing ? 'The stone is still reading these bytes.' : null"
          @click="open"
        >
          Add to library…
        </RiftButton>
      </div>
      <p v-if="refusal" class="built__refused">{{ refusal }}</p>
    </template>
    <BringThroughCard v-if="plan" :plan="plan" @close="plan = null" />
  </li>
</template>

<style scoped>
.built {
  position: relative;
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: var(--rs-space-3) var(--rs-space-3) var(--rs-space-3) var(--rs-space-4);
  background: var(--rs-basalt);
}
/* the rail: it has not crossed */
.built::before {
  content: "";
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 4px;
  background: var(--rs-edgehi);
}
.built.is-in-rift::before {
  background: var(--rs-rift);
}
.built__name {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  gap: var(--rs-space-2);
  margin: 0;
}
.built__file {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--rs-text);
  font: 500 var(--rs-text-sm) var(--rs-font-sans);
}
.built__size {
  flex: none;
  color: var(--rs-faint);
}
.built__sha {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin: 0;
  color: var(--rs-text);
}
.built__copy {
  display: grid;
  place-items: center;
  width: 24px;
  height: 24px;
  border: 0;
  background: none;
  color: var(--rs-muted);
  font: 400 14px var(--rs-font-mono);
  cursor: pointer;
}
.built__copy:hover {
  color: var(--rs-text);
}
/* ⧉ drawn, not set: the two carved squares read the same on every font */
.built__copy-glyph {
  position: relative;
  width: 9px;
  height: 9px;
  border: 1px solid currentColor;
  transform: translate(-2px, 2px);
}
.built__copy-glyph::after {
  content: "";
  position: absolute;
  top: -4px;
  left: 2px;
  width: 9px;
  height: 9px;
  border: 1px solid currentColor;
  border-left-color: transparent;
  border-bottom-color: transparent;
}
.built__receipt {
  margin: 0;
  color: var(--rs-faint);
}
.built__where {
  margin: 0;
  color: var(--rs-rift);
  font: 400 var(--rs-text-sm) var(--rs-font-sans);
}
.built__door {
  margin-top: var(--rs-space-2);
}
.built__refused {
  margin: var(--rs-space-1) 0 0;
  color: var(--rs-ember);
  font: 400 var(--rs-text-sm) var(--rs-font-sans);
}
/* listed under the built ones, faint: no receipt, no door */
.built.is-faint .built__file,
.built.is-faint .built__sha {
  color: var(--rs-faint);
}
</style>
