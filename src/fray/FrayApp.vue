<script setup lang="ts">
// THE FRAY — the Riftstone's second window (#00015 §19): the party's damage
// in a fight, for the portrait monitor beside the game. It pulls one frame
// every 250 ms (src/services/fray.ts) and draws from that frame alone. Its own
// state is only the view's: the 4 s beat's reading, which card is open, which
// breakdown is pressed, and which shelf fight is browsed.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, shallowRef } from "vue";
import type { FrayFight, FrayFrame } from "@/types/fray";
import { fraySource, mockScene, startFrayPoll } from "@/services/fray";
import {
  clockAgo,
  freshBeat,
  minutesAgoOf,
  settledWords,
  startedAgoS,
  shelfOf,
  takeBeat,
  type BeatState,
} from "./reading";
import { freshened, isNewcomer, modeOf, placesOf, type FrayDoor } from "./stage";
import FrayCard, { type FrayCardKind } from "./components/FrayCard.vue";
import FrayHead from "./components/FrayHead.vue";
import FrayIdle from "./components/FrayIdle.vue";
import FrayLoose from "./components/FrayLoose.vue";
import FrayOpenCard from "./components/FrayOpenCard.vue";
import FrayStateHead from "./components/FrayStateHead.vue";
import FrayStrata from "./components/FrayStrata.vue";
import FrayTrust from "./components/FrayTrust.vue";

/** The last frame a pull answered with, and when (ms). */
const pulled = shallowRef<FrayFrame | null>(null);
const pulledAt = ref<number | null>(null);
const beat = shallowRef<BeatState>(freshBeat());
const sub = ref<"skills" | "types">("skills");
const openSlot = ref<number | null>(null);
const browseN = ref<number | null>(null);
const announce = ref("");
const now = ref(new Date());
const mocked = mockScene() !== null || !("__TAURI_INTERNALS__" in window);
// Pulls that fail leave the last frame standing; it may not stand for live (stage.ts).
const frame = computed(() => freshened(pulled.value, pulledAt.value, now.value.getTime()));

const mode = computed(() => (frame.value ? modeOf(frame.value) : null));
const shelf = computed(() => (frame.value ? shelfOf(frame.value.fight, frame.value.recent) : []));
const browsing = computed(() => mode.value?.kind === "record" || mode.value?.kind === "sleep");
const browsed = computed(() =>
  browsing.value && browseN.value !== null ? (shelf.value.find((f) => f.n === browseN.value) ?? null) : null,
);

type View = "live" | "stale" | "record" | "kept" | "fresh" | "idle" | "none";
const view = computed<View>(() => {
  const m = mode.value;
  if (!m) return "none";
  if (browsed.value) return "record";
  if (m.kind === "live") return m.stale ? "stale" : "live";
  if (m.kind === "sleep") return m.kept ? "kept" : "fresh";
  return m.kind;
});
const stageFight = computed<FrayFight | null>(() => {
  const m = mode.value;
  if (browsed.value) return browsed.value;
  if (m?.kind === "live" || m?.kind === "record") return m.fight;
  return m?.kind === "sleep" ? m.kept : null;
});
const sleep = computed(() => (mode.value?.kind === "sleep" ? mode.value.sleep : "silent"));
const places = computed(() => (frame.value ? placesOf(frame.value) : []));
const fighting = computed(() => view.value === "live" || view.value === "stale");
const staleAge = computed(() => frame.value?.feed.lastBeatAgoS ?? 0);
const minutesAgo = computed(() => minutesAgoOf(stageFight.value, now.value));
const startedAgo = computed(() => startedAgoS(frame.value?.feed.gameStartedMs ?? null, now.value));
const framedId = computed(() => (browsed.value ?? (view.value === "kept" ? stageFight.value : null))?.id ?? null);
const stageClass = computed(() => ({
  muteall: view.value === "stale" || view.value === "kept",
  "stale-all": view.value === "stale",
  carvedbars: view.value === "record" || view.value === "kept",
}));

function cardKind(slot: number): FrayCardKind {
  if (fighting.value) return "live";
  return openSlot.value !== null && view.value === "record" && openSlot.value !== slot ? "strip" : "record";
}
const isOpen = (slot: number): boolean => view.value === "record" && openSlot.value === slot;

/** THE SETTLING: the same fight, open in the last frame, settled in this one. */
function settles(prev: FrayFight | null, fight: FrayFight): boolean {
  return prev?.id === fight.id && prev.state === "open" && fight.state === "settled";
}

/** A new fight opens: the stage is the fight's, so the record's view closes. */
function freshFight(prev: FrayFight | null, fight: FrayFight): void {
  if (fight.state !== "open" || prev?.id === fight.id) return;
  openSlot.value = null;
  browseN.value = null;
  sub.value = "skills";
}

function onFrame(next: FrayFrame): void {
  const prev = pulled.value?.fight ?? null;
  const fight = next.fight;
  if (fight) {
    beat.value = takeBeat(beat.value, fight);
    if (settles(prev, fight)) announce.value = settledWords(fight);
    freshFight(prev, fight);
  }
  pulled.value = next;
  now.value = new Date();
  pulledAt.value = now.value.getTime();
}

/** A pull that failed: the clock still moves, so a frame left standing can go stale. */
function onPullFailed(): void {
  now.value = new Date();
}

const shelfIdx = (): number => shelf.value.findIndex((f) => f.id === stageFight.value?.id);
function browseTo(n: number | undefined): void {
  if (n === undefined) return;
  openSlot.value = null;
  browseN.value = n;
}
/** A step along the shelf rebuilds the head: the door pressed keeps the keyboard (PR #3). */
async function step(door: "older" | "newer", n: number | undefined): Promise<void> {
  const held = document.activeElement?.getAttribute("data-act") === door;
  browseTo(n);
  if (!held) return;
  await nextTick();
  document.querySelector<HTMLElement>(`[data-act="${door}"]`)?.focus();
}
const older = (): Promise<void> => step("older", shelf.value[shelfIdx() + 1]?.n);
const newer = (): Promise<void> => step("newer", shelfIdx() > 0 ? shelf.value[shelfIdx() - 1]?.n : undefined);
function pick(n: number | null): void {
  const s = shelf.value;
  browseTo(n ?? (s[shelfIdx() + 1] ?? s[1] ?? s[0])?.n);
}
function door(d: FrayDoor): void {
  if (d === "Copy details")
    void navigator.clipboard?.writeText(frame.value?.feed.eyeReason ?? "The eye is shut and gave no reason.");
  // Open the Riftstone / Open Preflight: the contract has no command for them yet (see the build report).
}
function onKey(e: KeyboardEvent): void {
  if (e.key !== "Escape") return;
  if (openSlot.value !== null) openSlot.value = null;
  else browseN.value = null;
}

let stop: (() => void) | null = null;
onMounted(async () => {
  window.addEventListener("keydown", onKey);
  const source = await fraySource();
  sub.value = source.view.sub ?? "skills";
  openSlot.value = source.view.open ?? null;
  browseN.value = source.view.browse ?? null;
  stop = startFrayPoll(onFrame, onPullFailed);
});
onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKey);
  stop?.();
});
</script>

<template>
  <div id="fray" class="win">
    <main
      class="stage"
      :class="stageClass"
      :inert="fighting ? true : undefined"
      role="main"
      aria-label="The party in the fight"
    >
      <span v-if="mocked" class="invented" aria-hidden="true">INVENTED DATA · MOCK</span>
      <template v-if="view === 'idle'">
        <FrayIdle :places="places" />
        <div class="grow" />
        <FrayStrata :shelf="[]" :framed-id="null" :door="false" :rows="3" />
      </template>
      <template v-else-if="view === 'fresh'">
        <FrayStateHead :started-ago-s="startedAgo" :kind="sleep" :kept="null" :minutes-ago="null" @door="door" />
        <section class="cards" aria-label="The party, as the last save has it">
          <FrayCard v-for="(m, slot) in places" :key="slot" :member="m" :fight="null" kind="save" />
        </section>
        <div class="grow" />
        <FrayStrata :shelf="[]" :framed-id="null" :door="false" :rows="3" />
      </template>
      <template v-else-if="stageFight">
        <FrayStateHead
          :started-ago-s="startedAgo"
          v-if="view === 'kept'"
          :kind="sleep"
          :kept="stageFight"
          :minutes-ago="minutesAgo"
          @door="door"
        />
        <FrayHead
          v-else
          :key="browsed ? `browse-${stageFight.id}` : stageFight.id"
          :fight="stageFight"
          :mode="view === 'record' ? 'record' : view === 'stale' ? 'stale' : 'live'"
          :browse="browsed !== null"
          :sub="sub"
          :open-slot="openSlot"
          :stale-age-s="staleAge"
          :last-beat-at="clockAgo(staleAge, now)"
          :minutes-ago="minutesAgo"
          @sub="sub = $event"
          @older="older"
          @newer="newer"
        />
        <section :key="browsed ? `browse-${stageFight.id}` : stageFight.id" class="cards" aria-label="The party">
          <template v-for="(m, slot) in stageFight.members" :key="slot">
            <FrayOpenCard v-if="m && isOpen(slot)" :member="m" :fight="stageFight" @close="openSlot = null" />
            <FrayCard
              v-else
              :member="m"
              :fight="stageFight"
              :kind="cardKind(slot)"
              :block="m ? beat.blocks[m.slot] : null"
              :sub="sub"
              :door="view === 'record'"
              :is-new="m ? isNewcomer(m, shelf) : false"
              @open="openSlot = $event"
            />
          </template>
        </section>
        <FrayLoose :fight="stageFight" />
        <FrayTrust :fight="stageFight" />
        <FrayStrata
          :shelf="fighting ? (frame?.recent ?? []) : shelf"
          :framed-id="framedId"
          :door="!fighting"
          :rows="stageFight.world ? 1 : 3"
          @pick="pick"
        />
      </template>
      <div class="sr" aria-live="polite">{{ announce }}</div>
    </main>
  </div>
</template>
