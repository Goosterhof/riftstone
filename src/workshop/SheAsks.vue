<script setup lang="ts">
// 0.8 — SHE ASKS (#00015 R20.36, Q20.33 ruled A by the investor on
// 2026-11-01): every ask of Tamsin's that waits on the Arisen is pinned above
// the hands, in the order she asked, oldest first, a lone held Play included.
// One rule: what waits on you is always in one place. Each block is R20.10's
// held block. Play runs §16.2's plan exactly (its warnings and refusals worded
// as the Play slab words them); Not now answers declined. Update runs the
// card's press on the digest frozen at her ask, replacing the earlier build
// shown here; a build that changed since she asked is not updated.
import { computed, nextTick, ref, watch } from "vue";
import type { BringThroughPlan } from "@/types/workshop";
import RiftButton from "@/components/RiftButton.vue";
import { useToasts } from "@/composables/useToasts";
import { launchWarningLine } from "@/services/ledger";
import { useWorkshop } from "./useWorkshop";
import { PAWN_NAME, clock, quads, swapLine, updatedLine } from "./words";

const w = useWorkshop();
const anyway = ref<InstanceType<typeof RiftButton> | null>(null);
const pressing = ref(false);

interface Ask {
  kind: "play" | "update";
  since: string;
  /** Her held row's number: the order the stone answered her asks, finer than `since`'s seconds. */
  seq: number;
}
/** The newest held row of `tool`, or none (a row the window has not heard yet). */
function heldRow(tool: string): number {
  return w.hands.value.find((h) => h.tool === tool && h.outcome === "held")?.seq ?? Number.MAX_SAFE_INTEGER;
}
const asks = computed<Ask[]>(() => {
  const out: Ask[] = [];
  if (w.held.value) out.push({ kind: "play", since: w.held.value.since, seq: heldRow("play") });
  if (w.heldUpdate.value) out.push({ kind: "update", since: w.heldUpdate.value.since, seq: heldRow("ask_update") });
  // a fresh array of at most two: sorted in place, in the order she asked
  out.sort((a, b) => a.seq - b.seq || a.since.localeCompare(b.since));
  return out;
});
/** Her update waits above her Play: playing first loads it at the next launch. */
const updateAbovePlay = computed(() => asks.value.length === 2 && asks.value[0].kind === "update");

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

/** The update ask's card, read when the ask appears: what the press would do now. */
const updatePlan = ref<BringThroughPlan | null>(null);
const planRefusal = ref<string | null>(null);
watch(
  () => w.heldUpdate.value?.heldRequestId ?? null,
  async (id) => {
    updatePlan.value = null;
    planRefusal.value = null;
    const ask = w.heldUpdate.value;
    if (!id || !ask) return;
    const plan = await w.planBring(ask.artifact);
    if (w.heldUpdate.value?.heldRequestId !== id) return;
    if (plan) updatePlan.value = plan;
    else planRefusal.value = w.failure.value?.detail.detail ?? w.failure.value?.message ?? null;
  },
  { immediate: true },
);
/** The bytes on disk now are not the ones she asked about. */
const changedSince = computed(
  () => !!updatePlan.value && !!w.heldUpdate.value && updatePlan.value.sha256 !== w.heldUpdate.value.sha256,
);
const swaps = computed(() => updatePlan.value?.earlier?.swaps ?? []);
const updateAsks = computed(() => {
  const artifact = w.heldUpdate.value?.artifact ?? "";
  return updatePlan.value?.earlier
    ? `${PAWN_NAME} asks to update to ${artifact}.`
    : `${PAWN_NAME} asks to add ${artifact} to your mods folder.`;
});

async function answerUpdate(accept: boolean): Promise<void> {
  const ask = w.heldUpdate.value;
  if (!ask) return;
  const answered = await w.answerUpdate(accept, accept ? (updatePlan.value?.earlier?.packageId ?? null) : null);
  const result = answered?.result;
  if (!result) return;
  if (result.failed) {
    useToasts().push("ember", `${ask.artifact} is in your mods folder, but the swap didn't happen: ${result.failed}`);
  } else {
    useToasts().push(result.swapped.length ? "world" : "rift", updatedLine(ask.artifact, result));
  }
}
</script>

<template>
  <section v-if="asks.length" class="asks" aria-labelledby="ws-she-asks">
    <h3 id="ws-she-asks" class="asks__head rs-inscription">SHE ASKS</h3>
    <template v-for="ask in asks" :key="ask.kind">
      <div v-if="ask.kind === 'play'" class="hand__held" data-ask="play">
        <p class="hand__asks">
          <span class="asks__glyph" aria-hidden="true">▶ </span>{{ PAWN_NAME }} asks to start the game.
        </p>
        <p v-if="updateAbovePlay" class="asks__note">
          Her update is still waiting above. If you play first, it loads at the next launch.
        </p>
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

      <div v-else-if="w.heldUpdate.value" class="hand__held" data-ask="update">
        <p class="hand__asks"><span class="asks__glyph" aria-hidden="true">↻ </span>{{ updateAsks }}</p>
        <ul v-if="updatePlan?.earlier" class="asks__rows rs-mono">
          <li v-for="swap in swaps" :key="swap.fromOptionId">↻ {{ swapLine(swap) }}</li>
          <li v-for="stay in updatePlan.earlier.stays" :key="stay.optionId">= {{ stay.name }} stays</li>
          <li v-if="updatePlan.earlier.putsAway">↧ {{ updatePlan.earlier.fileName }} put away</li>
          <li v-else>{{ updatePlan.earlier.fileName }} stays in your mods folder</li>
        </ul>
        <p v-else-if="updatePlan" class="asks__rows rs-mono">
          + mods\{{ w.heldUpdate.value.artifact }} · nothing is installed
        </p>
        <p class="asks__digest rs-mono" data-digest>
          sha256 {{ quads(w.heldUpdate.value.sha256).slice(0, 4).join(" ") }} … · frozen at her ask,
          {{ clock(w.heldUpdate.value.since) }}
        </p>
        <p v-if="changedSince" class="hand__refused" role="alert">
          {{ w.heldUpdate.value.artifact }} changed after {{ PAWN_NAME }} asked: you were shown
          {{ w.heldUpdate.value.sha256.slice(0, 16) }}…, the file is now {{ updatePlan?.sha256.slice(0, 16) }}…. Nothing
          was added. Answer Not now, and she can ask again.
        </p>
        <p v-if="planRefusal" class="hand__refused" role="alert">{{ planRefusal }}</p>
        <div class="hand__doors">
          <RiftButton
            :variant="swaps.length ? 'world' : 'primary'"
            size="sm"
            :busy="w.answering.value"
            :refused="!updatePlan || changedSince"
            :reason="
              changedSince
                ? 'The build changed after she asked.'
                : !updatePlan
                  ? 'The stone is reading the build.'
                  : null
            "
            @click="answerUpdate(true)"
            >Update</RiftButton
          >
          <RiftButton size="sm" @click="answerUpdate(false)">Not now</RiftButton>
        </div>
        <p v-if="w.updateRefusal.value" class="hand__refused" role="alert">{{ w.updateRefusal.value }}</p>
      </div>
    </template>
  </section>
</template>

<style scoped>
.asks {
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-3);
  padding-bottom: var(--rs-space-3);
  border-bottom: 1px solid var(--rs-edge);
}
.asks__head {
  margin: 0;
  color: var(--rs-muted);
  font-size: 14px;
}
.asks__glyph {
  color: var(--rs-warn);
  font-family: var(--rs-font-mono);
}
.asks__note,
.asks__digest {
  margin: 0;
  color: var(--rs-faint);
  font-size: var(--rs-text-xs);
}
.asks__rows {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: 0;
  padding: 0;
  list-style: none;
  color: var(--rs-text);
  font-size: 12px;
}
.hand__held {
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-2);
}
.hand__asks {
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
.hand__refused {
  margin: 0;
  color: var(--rs-ember);
  font: 400 var(--rs-text-sm) var(--rs-font-sans);
}
</style>
