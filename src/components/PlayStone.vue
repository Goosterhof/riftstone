<script setup lang="ts">
// §16.2 — PLAY, carved as its own slab of the stone between the rooms and
// the foot (R16.11). A Primary cyan button, never gold: Play writes nothing
// into your world; the gold is spent on THE WORLD IS AWAKE while the game
// runs (R16.12, Bold Choice 9). Pressing reads plan_launch first: a refusal
// is said, warnings turn the slab into an inline confirm (R16.14). No hotkey:
// one slipped key must never start Dragon's Dogma 2 (R16.15).
import { computed, nextTick, onMounted, ref, watch } from "vue";
import type { LaunchWarning } from "@/types/rift";
import { useLaunch } from "@/composables/useLaunch";
import { useStone } from "@/composables/useStone";
import { useNavigation } from "@/composables/useNavigation";
import { launchWarningLine } from "@/services/ledger";
import { voicedLine } from "@/services/voice";
import RiftButton from "./RiftButton.vue";

const launch = useLaunch();
const stone = useStone();
const nav = useNavigation();

const playButton = ref<HTMLButtonElement | null>(null);
const anyway = ref<InstanceType<typeof RiftButton> | null>(null);
/** From the press until launch_game answered (R16.13 Launching). */
const pressing = ref(false);
/** The warnings the player is being asked to read before playing. */
const confirming = ref<LaunchWarning[] | null>(null);

const game = computed(() => stone.overview.value?.game ?? null);
/** The sentence that refuses Play now; the running game is its own state. */
const refused = computed(() => {
  if (launch.running.value) return null;
  const sentence = launch.plan.value?.refused ?? null;
  if (sentence) return sentence;
  // the plan carries the stone's own sentence; until it is read, a failed read speaks
  if (!game.value && launch.failure.value) return voicedLine(launch.failure.value);
  return null;
});
/** launch_game answered, the watcher has not seen the game yet. */
const stirring = computed(() => launch.launching.value && !pressing.value && !launch.running.value);
const since = computed(() => {
  const ms = launch.startedMs.value;
  if (ms === null) return null;
  return new Date(ms).toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit", hour12: false });
});

async function go(): Promise<void> {
  pressing.value = true;
  try {
    await launch.play();
  } finally {
    pressing.value = false;
  }
}

async function press(): Promise<void> {
  if (refused.value || pressing.value || launch.running.value) return;
  pressing.value = true;
  const plan = await launch.refreshPlan();
  pressing.value = false;
  if (!plan || plan.refused) return;
  if (plan.warnings.length) {
    confirming.value = plan.warnings;
    await nextTick();
    (anyway.value?.$el as HTMLElement | undefined)?.focus();
    return;
  }
  await go();
}

async function playAnyway(): Promise<void> {
  confirming.value = null;
  await go();
  await nextTick();
  playButton.value?.focus();
}

/** The Preflight row each warning answers to (PreflightPage's own checks). */
const CHECK_OF: Record<LaunchWarning["kind"], string> = {
  reframeworkMissing: "reframework",
  pakFolderOff: "pakdir",
  looseLoaderOff: "loose",
  buildChanged: "build",
};

async function fixIt(): Promise<void> {
  const first = confirming.value?.[0];
  confirming.value = null;
  nav.go("preflight");
  if (!first) return;
  await nextTick();
  const row = document.querySelector<HTMLElement>(`[data-check="${CHECK_OF[first.kind]}"]`);
  row?.scrollIntoView?.({ block: "center" });
}

async function backOut(event: KeyboardEvent): Promise<void> {
  if (event.key !== "Escape") return;
  event.preventDefault();
  event.stopPropagation();
  confirming.value = null;
  await nextTick();
  playButton.value?.focus();
}

onMounted(() => void launch.refreshPlan());
// a game found, lost or started elsewhere: what Play would do is read again
watch(
  () => [game.value === null, stone.phase.value] as const,
  () => void launch.refreshPlan(),
);
watch(
  () => launch.running.value,
  (on) => {
    if (on) confirming.value = null;
  },
);
</script>

<template>
  <section class="play" :class="{ 'is-awake': launch.running.value }" aria-label="Play" aria-live="polite">
    <template v-if="launch.running.value">
      <p class="play__carved play__carved--awake rs-inscription">THE WORLD IS AWAKE</p>
      <p v-if="since" class="play__since rs-mono">since {{ since }}</p>
    </template>

    <div v-else-if="confirming" class="play__confirm" role="group" aria-label="Before you play" @keydown="backOut">
      <p v-for="w in confirming" :key="w.kind" class="play__warn">
        <span aria-hidden="true">⚠ </span>{{ launchWarningLine(w) }}
      </p>
      <div class="play__choices">
        <RiftButton ref="anyway" variant="primary" size="sm" @click="playAnyway">Play anyway</RiftButton>
        <RiftButton size="sm" @click="fixIt">Fix it</RiftButton>
      </div>
    </div>

    <template v-else>
      <template v-if="stirring">
        <p class="play__carved rs-inscription">THE WORLD STIRS</p>
        <p class="play__note">
          {{ launch.route.value === "steam" ? "Steam is starting the game." : "The game is starting." }}
        </p>
      </template>
      <button
        ref="playButton"
        type="button"
        class="play__btn"
        :class="{ 'is-refused': refused, 'is-busy': pressing }"
        :aria-disabled="refused || pressing ? 'true' : undefined"
        :aria-busy="pressing ? 'true' : undefined"
        :aria-describedby="refused ? 'play-refused' : undefined"
        @click="press"
      >
        <span class="play__label">Play</span>
        <span v-if="pressing" class="play__dots rs-mono" aria-hidden="true">···</span>
      </button>
      <p v-if="refused" id="play-refused" class="play__refused" :title="refused">{{ refused }}</p>
    </template>
  </section>
</template>

<style scoped>
/* its own slab, pinned: 12 px above, a decorative hairline, full width (R16.11) */
.play {
  flex: none;
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-2);
  padding: var(--rs-space-3) 0;
  border-top: 1px solid var(--rs-edge);
}
.play__btn {
  position: relative;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 36px;
  padding: 0 var(--rs-space-4);
  border: 1px solid var(--rs-rift);
  border-radius: 0;
  background: var(--rs-riftdeep);
  color: var(--rs-text);
  font: 500 var(--rs-text-sm) var(--rs-font-sans);
  cursor: pointer;
  transition: background var(--rs-dur-quick);
}
/* RiftButton's Primary, kept here because a refused Play stays focusable
   (aria-disabled, never disabled: its reason must be reachable, R16.13) */
.play__btn:hover:not(.is-refused, .is-busy) {
  background: #256673;
}
.play__btn:active:not(.is-refused, .is-busy) {
  transform: translateY(1px);
}
.play__btn.is-refused {
  opacity: 0.45;
  cursor: not-allowed;
}
.play__btn.is-busy .play__label {
  visibility: hidden;
}
.play__btn.is-busy {
  cursor: progress;
}
.play__dots {
  position: absolute;
  font-weight: 600;
  letter-spacing: 0.2em;
  inset: 0;
  display: grid;
  place-items: center;
}
.play__refused {
  display: -webkit-box;
  margin: 0;
  overflow: hidden;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 3;
  line-clamp: 3;
  color: var(--rs-muted);
  font: 400 var(--rs-text-sm) var(--rs-font-sans);
}
.play__carved {
  margin: 0;
  color: var(--rs-muted);
  font: 600 14px var(--rs-font-inscription);
  letter-spacing: 0.08em;
}
/* gold is the state "your world is live": it fades in once, never pulses (§16.7) */
.play__carved--awake {
  color: var(--rs-world);
  animation: play-wake var(--rs-dur-settle) ease-out both;
}
@keyframes play-wake {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}
.play__since {
  margin: 0;
  color: var(--rs-faint);
  font-size: 12px;
}
.play__note {
  margin: 0;
  color: var(--rs-faint);
  font: 400 var(--rs-text-sm) var(--rs-font-sans);
}
.play__confirm {
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-2);
}
.play__warn {
  margin: 0;
  color: var(--rs-warn);
  font: 400 var(--rs-text-sm) var(--rs-font-sans);
}
.play__choices {
  display: flex;
  flex-wrap: wrap;
  gap: var(--rs-space-2);
}
</style>
