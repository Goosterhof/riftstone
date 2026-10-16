<script setup lang="ts">
// §8.1.1 — the trouble fold: the Rift's trouble carved as one line above the
// first creation, ember when a package can't be opened. One press opens the
// cards in place; the count never hides (trouble-first triage, kept). The
// count is an inscription and the verb beside it is the button (§10's Rule).
// No motion: the cards appear where they stand, and the frame is the state.
import { computed, nextTick, watch } from "vue";
import { useStone } from "@/composables/useStone";
import { troubleLine } from "@/services/voice";
import RiftButton from "./RiftButton.vue";
import TroubledCard from "./TroubledCard.vue";

const emit = defineEmits<{ emptied: [] }>();
const stone = useStone();

const others = computed(() => Math.max(0, (stone.overview.value?.library.packages ?? 1) - 1));
const count = computed(() => stone.troubled.value.length);
const ember = computed(() => stone.troubled.value.some((p) => troubleLine(p, others.value).ember));
/** Nothing to fold over: a library with packages but no creation shows its cards. */
const foldable = computed(() => (stone.overview.value?.library.options ?? 0) > 0);
const open = computed(() => !foldable.value || stone.troubleOpen.value);
const carved = computed(() => `${count.value.toLocaleString("en-GB")} WON'T ANSWER`);

const cardsOf = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>("#rift-trouble [data-trouble-index]")];

/** The card that held focus, so a card that leaves can hand it on. */
let held = -1;
function onFocusIn(event: FocusEvent): void {
  const card = event.target instanceof Element ? event.target.closest<HTMLElement>("[data-trouble-index]") : null;
  held = card ? Number(card.dataset.troubleIndex) : -1;
}
function onFocusOut(event: FocusEvent): void {
  // A real move elsewhere clears the hold; a removal (no relatedTarget) keeps it.
  const to = event.relatedTarget;
  if (to instanceof Node && !document.getElementById("rift-trouble")?.contains(to)) held = -1;
}

/** Focus fell with its card: nothing holds it, or what held it left the page. */
function focusLost(): boolean {
  const active = document.activeElement;
  return !active || active === document.body || !active.isConnected;
}

watch(
  () => stone.troubled.value.map((p) => p.id),
  async () => {
    await nextTick();
    if (held < 0 || !focusLost()) return;
    const left = cardsOf();
    const next = left.at(held) ?? left.at(held - 1);
    held = next ? Number(next.dataset.troubleIndex) : -1;
    if (next) next.querySelector<HTMLButtonElement>("button")?.focus();
    else emit("emptied");
  },
);
</script>

<template>
  <section v-if="count > 0" class="trouble-fold" :class="{ 'is-ember': ember }" aria-label="Packages with trouble">
    <div class="trouble-fold__line">
      <p id="rift-trouble-count" class="trouble-fold__count" aria-live="polite">{{ carved }}</p>
      <RiftButton
        v-if="foldable"
        size="sm"
        :aria-expanded="open ? 'true' : 'false'"
        aria-controls="rift-trouble"
        aria-describedby="rift-trouble-count"
        @click="stone.toggleTrouble()"
      >
        {{ open ? "Close" : "Show" }} <span aria-hidden="true">{{ open ? "▾" : "▸" }}</span>
      </RiftButton>
    </div>
    <div id="rift-trouble" class="trouble-fold__cards" :hidden="!open" @focusin="onFocusIn" @focusout="onFocusOut">
      <template v-if="open">
        <TroubledCard v-for="(p, i) in stone.troubled.value" :key="p.id" :pkg="p" :data-trouble-index="i" />
      </template>
    </div>
  </section>
</template>

<style scoped>
.trouble-fold {
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-2);
  padding: var(--rs-space-4) var(--rs-space-6) 0; /* the listbox's own side padding */
}
/* the cards' own anatomy: mist ground, a 4 px rail, radius 0, no shadow */
.trouble-fold__line {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--rs-space-4);
  padding: var(--rs-space-3) var(--rs-space-4) var(--rs-space-3) var(--rs-space-5);
  background: var(--rs-mist);
}
.trouble-fold__line::before {
  content: "";
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 4px;
  background: var(--rs-faint);
}
.trouble-fold.is-ember .trouble-fold__line::before {
  background: var(--rs-ember);
}
/* the pillar count's idiom: mono 600 12, tracked; tabular, so the figure
   holds its place as cards leave */
.trouble-fold__count {
  margin: 0;
  color: var(--rs-muted);
  font: 600 var(--rs-text-xs) / 1.4 var(--rs-font-mono);
  font-variant-numeric: tabular-nums;
  letter-spacing: 0.12em;
  white-space: nowrap;
}
.trouble-fold.is-ember .trouble-fold__count {
  color: var(--rs-ember);
}
.trouble-fold__cards {
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-2);
}
.trouble-fold__cards[hidden] {
  display: none;
}
</style>
