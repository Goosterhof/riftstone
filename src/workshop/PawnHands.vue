<script setup lang="ts">
// R20.8 with the contract's PawnHand — THE PAWN'S HANDS: every call the stone
// answered for Tamsin, newest first, two lines each, the text the stone's own.
// No per-row Undo and no log viewer: the contract carries neither, and Put
// back is the undo. The held Play (R20.10, WD5, WD6) is the newest play row
// still waiting: Play runs §16.2's plan exactly, warnings and refusals worded
// as the Play slab words them; Not now answers declined.
import { computed, nextTick, ref } from "vue";
import type { PawnHand } from "@/types/workshop";
import RiftButton from "@/components/RiftButton.vue";
import { useLaunch } from "@/composables/useLaunch";
import { launchWarningLine } from "@/services/ledger";
import { useWorkshop } from "./useWorkshop";
import { PAWN_NAME, handFace } from "./words";

const w = useWorkshop();
const launch = useLaunch();
const anyway = ref<InstanceType<typeof RiftButton> | null>(null);
const pressing = ref(false);

/** The row the investor answers: the newest play/held while one is held. */
const heldSeq = computed(() => {
  if (!w.held.value) return null;
  return w.hands.value.find((h) => h.tool === "play" && h.outcome === "held")?.seq ?? null;
});
/** Where the answer's lines sit: the newest answered play row, else the newest
 *  play row at all (the held call's own answer stays `held` on the spine). */
const answeredSeq = computed(() => {
  if (!w.playAnswered.value) return null;
  const plays = w.hands.value.filter((h) => h.tool === "play");
  return (plays.find((h) => h.outcome === "done" || h.outcome === "declined") ?? plays.at(0))?.seq ?? null;
});

async function play(): Promise<void> {
  pressing.value = true;
  try {
    await w.pressPlay();
  } finally {
    pressing.value = false;
  }
  if (w.playWarnings.value) {
    await nextTick();
    (anyway.value?.$el as HTMLElement | undefined)?.focus();
  }
}

/** A held row warns only while it is the request waiting now; once answered
 *  or dropped it is history, muted like the rest. */
function face(hand: PawnHand) {
  const f = handFace(hand);
  const stale = hand.tool === "play" && hand.outcome === "held" && hand.seq !== heldSeq.value;
  return stale ? { ...f, tone: "muted" as const } : f;
}
</script>

<template>
  <ol class="hands" aria-live="polite">
    <li
      v-for="hand in w.hands.value"
      :key="hand.seq"
      class="hand"
      :class="`is-${face(hand).tone}`"
      :data-hand="hand.seq"
    >
      <span class="hand__glyph rs-mono" aria-hidden="true">{{ face(hand).glyph }}</span>
      <p class="hand__line">{{ face(hand).line }}</p>
      <p class="hand__second" :class="{ 'is-sentence': hand.outcome === 'refused' }">{{ face(hand).second }}</p>

      <div v-if="hand.seq === heldSeq" class="hand__held">
        <p class="hand__asks">{{ PAWN_NAME }} asks to start the game.</p>
        <div v-if="w.playWarnings.value" class="hand__confirm" role="group" aria-label="Before you play">
          <p v-for="warning in w.playWarnings.value" :key="warning.kind" class="hand__warn">
            <span aria-hidden="true">⚠ </span>{{ launchWarningLine(warning) }}
          </p>
          <div class="hand__doors">
            <RiftButton ref="anyway" variant="primary" size="sm" :busy="w.answering.value" @click="w.confirmPlay()">
              Play anyway
            </RiftButton>
            <RiftButton size="sm" @click="w.cancelWarnings()">Cancel</RiftButton>
          </div>
        </div>
        <div v-else class="hand__doors">
          <RiftButton variant="primary" size="sm" :busy="pressing || w.answering.value" @click="play">Play</RiftButton>
          <RiftButton size="sm" @click="w.declinePlay()">Not now</RiftButton>
        </div>
        <p v-if="w.playRefusal.value" class="hand__refused" role="alert">{{ w.playRefusal.value }}</p>
      </div>

      <div v-if="hand.seq === answeredSeq && w.playAnswered.value" class="hand__answered" role="status">
        <p class="hand__decision">
          {{
            w.playAnswered.value.decision === "played"
              ? "The stone started the game."
              : `Not now: ${PAWN_NAME} was told.`
          }}
        </p>
        <p v-if="!w.playAnswered.value.persisted" class="hand__unpersisted">
          {{ PAWN_NAME }}'s ledger could not be written, so she may not hear of it.
        </p>
        <!-- the game watcher's own state, never folded into the decision (WD6) -->
        <p v-if="w.playAnswered.value.decision === 'played'" class="hand__watcher rs-mono">
          {{ launch.running.value ? "Dragon's Dogma 2 is running." : "The stone has not seen the game start yet." }}
        </p>
      </div>
    </li>
  </ol>
</template>

<style scoped>
.hands {
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-3);
  margin: 0;
  padding: 0;
  list-style: none;
}
.hand {
  display: grid;
  grid-template-columns: 2ch minmax(0, 1fr);
  column-gap: var(--rs-space-1);
}
.hand > :not(.hand__glyph) {
  grid-column: 2;
}
.hand__glyph {
  grid-row: 1;
  font-size: var(--rs-text-sm);
  line-height: 1.45;
}
.hand__line {
  grid-row: 1;
  margin: 0;
  font: 500 var(--rs-text-sm) / 1.45 var(--rs-font-sans);
  overflow-wrap: anywhere;
}
.hand__second {
  margin: 0;
  color: var(--rs-faint);
  font: 400 var(--rs-text-xs) / 1.4 var(--rs-font-mono);
}
/* the stone's refusal sentence, verbatim: read as a sentence, not a stamp */
.hand__second.is-sentence {
  color: var(--rs-text);
  font: 400 var(--rs-text-sm) / 1.4 var(--rs-font-sans);
}
.is-muted .hand__glyph,
.is-muted .hand__line {
  color: var(--rs-muted);
}
.is-world .hand__glyph,
.is-world .hand__line {
  color: var(--rs-world);
}
.is-warn .hand__glyph,
.is-warn .hand__line {
  color: var(--rs-warn);
}
.is-ember .hand__glyph,
.is-ember .hand__line {
  color: var(--rs-ember);
}
.is-faint .hand__glyph,
.is-faint .hand__line {
  color: var(--rs-faint);
}
.hand__held,
.hand__answered {
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-2);
  margin-top: var(--rs-space-2);
}
.hand__asks,
.hand__decision {
  margin: 0;
  color: var(--rs-text);
  font: 400 var(--rs-text-sm) var(--rs-font-sans);
}
.hand__doors {
  display: flex;
  flex-wrap: wrap;
  gap: var(--rs-space-2);
}
.hand__confirm {
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-2);
}
.hand__warn {
  margin: 0;
  color: var(--rs-warn);
  font: 400 var(--rs-text-sm) var(--rs-font-sans);
}
.hand__refused,
.hand__unpersisted {
  margin: 0;
  color: var(--rs-ember);
  font: 400 var(--rs-text-sm) var(--rs-font-sans);
}
.hand__watcher {
  margin: 0;
  color: var(--rs-faint);
}
</style>
