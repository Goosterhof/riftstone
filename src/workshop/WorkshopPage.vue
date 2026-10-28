<script setup lang="ts">
// THE WORKSHOP (#00015 §20, WINDOW-CONTRACT § v0.7) — a hollow carved into the
// stone, with a crack in its back wall where Tamsin's voice comes through from
// the Rift. The room ground is the well; the hollow is the void seen through
// the crack; its left edge, the fissure, is the room's only light. The pawn is
// never drawn: a name carved in the head and a voice in the hollow (Bold
// Choice 23). THE PARTING runs once, on the first bytes, never on the press
// (R20.17): the text is already there when the stone opens.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import RiftButton from "@/components/RiftButton.vue";
import { useNavigation } from "@/composables/useNavigation";
import { useToasts } from "@/composables/useToasts";
import FenceRecord from "./FenceRecord.vue";
import PutBackBlock from "./PutBackBlock.vue";
import WorkshopBench from "./WorkshopBench.vue";
import WorkshopHollow from "./WorkshopHollow.vue";
import { useWorkshop } from "./useWorkshop";
import { CHIP_WORD, PAWN_NAME, STEP_WORD, clock } from "./words";

const w = useWorkshop();
const nav = useNavigation();

const root = ref<HTMLElement | null>(null);
const hollow = ref<InstanceType<typeof WorkshopHollow> | null>(null);
const fenceOpen = ref(false);
const benchOpen = ref(false);
const narrow = ref(false);
const parting = ref(false);
const summoning = ref(false);
/** The survey's sentence when Summon found no fence to build (it names the install command). */
const noFence = ref<string | null>(null);
/** A refusal from a press on this page that no other place in the room says. */
const said = ref<string | null>(null);

const pawn = computed(() => w.pawn.value);
const state = computed(() => pawn.value?.state ?? "absent");
const fence = computed(() => w.fence.value);
const breached = computed(() => fence.value?.chip === "breached");
const ended = computed(() => state.value === "departed" || state.value === "torn");
const folderMade = computed(() => w.view.value?.folder.made ?? false);
const folderPath = computed(() => w.view.value?.folder.path ?? "<mods folder> (workshop)");

/** The hollow's four places (R20.16), each legible at zero frames per second. */
const place = computed<"closed" | "summoning" | "live" | "departed">(() => {
  if (noFence.value) return "closed";
  if (state.value === "here") return "live";
  if (state.value === "summoning") return "summoning";
  if (ended.value && w.firstBytes.value) return "departed";
  return "closed";
});

const WHERE: Record<string, string> = {
  summoning: "being called",
  departed: "gone back",
  torn: "torn away",
  absent: "not here",
};

const nameLine = computed(() => {
  const p = pawn.value;
  if (state.value !== "here" && state.value !== "summoning") return [WHERE[state.value] ?? "not here"];
  const parts = [state.value === "here" ? `here since ${clock(p?.since)}` : WHERE.summoning];
  if (p?.host) parts.push(`claude ${p.host.claudeVersion}`, `WSL ${p.host.distro}`);
  return parts;
});

/** The steps the last probe saw the fence fail, as the record says them. */
const broken = computed(() =>
  (fence.value?.steps ?? [])
    .filter((s) => s.verdict === "breached")
    .map((s) => `${STEP_WORD[s.step] ?? s.step} (${s.observed})`),
);

const sliverMarks = computed(() => ({
  built: w.view.value?.artifacts.length ?? 0,
  hands: w.hands.value.length,
}));

function failureSentence(): string | null {
  const f = w.failure.value;
  return f ? (f.detail.detail ?? f.message) : null;
}

async function summon(): Promise<void> {
  if (summoning.value) return;
  summoning.value = true;
  said.value = null;
  const { cols, rows } = hollow.value?.size() ?? { cols: 80, rows: 24 };
  try {
    await w.summon(cols, rows);
  } finally {
    summoning.value = false;
  }
  const f = w.failure.value;
  if (f?.kind === "pawn" && f.detail.reason === "noFence") noFence.value = failureSentence();
  else if (f) said.value = failureSentence();
  else noFence.value = null;
}

async function lookAgain(): Promise<void> {
  noFence.value = null;
  said.value = null;
  w.failure.value = null;
  await w.load();
}

async function copyDetails(lines: string[]): Promise<void> {
  try {
    await navigator.clipboard.writeText(lines.join("\n"));
    useToasts().push("rift", "The details are on your clipboard.");
  } catch {
    useToasts().push("warn", "The clipboard refused the details.");
  }
}

async function press(work: () => Promise<void>): Promise<void> {
  said.value = null;
  await work();
  said.value = failureSentence();
}

async function showRecord(): Promise<void> {
  fenceOpen.value = true;
  await nextTick();
  document.getElementById("ws-fence-record")?.scrollIntoView?.({ block: "nearest" });
}

function visible(): boolean {
  return (root.value?.getClientRects().length ?? 0) > 0;
}

function reducedMotion(): boolean {
  return window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
}

let partingTimer: ReturnType<typeof setTimeout> | null = null;
function part(): void {
  parting.value = true;
  if (partingTimer) clearTimeout(partingTimer);
  partingTimer = setTimeout(() => (parting.value = false), 1000);
}

function focusInRoom(): boolean {
  const active = document.activeElement;
  return active === null || active === document.body || (root.value?.contains(active) ?? false);
}

// THE PARTING: once, on the run's first bytes, and only where it can be seen;
// under reduced motion the hollow lands on its live place with no frames.
watch(
  () => w.firstBytes.value,
  async (now, before) => {
    if (!now || before) return;
    const inRoom = focusInRoom();
    if (visible() && !reducedMotion()) part();
    // the Summon button left under the focus: the conversation takes it
    await nextTick();
    if (visible() && inRoom) hollow.value?.focus();
  },
);

let media: MediaQueryList | null = null;
function onMedia(): void {
  narrow.value = media?.matches ?? false;
  if (!narrow.value) benchOpen.value = false;
}

function typing(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName);
}

/** B unfolds the bench at 1024 (R20.3) when the keyboard is not in the terminal; Esc folds it. */
function onKey(event: KeyboardEvent): void {
  if (nav.page.value !== "workshop" || !narrow.value || event.defaultPrevented) return;
  const fold = event.key === "Escape" && benchOpen.value;
  if (fold || bareB(event)) {
    benchOpen.value = !fold && !benchOpen.value;
    event.preventDefault();
  }
}

function bareB(event: KeyboardEvent): boolean {
  if (event.ctrlKey || event.metaKey || event.altKey || typing(event.target)) return false;
  return event.key.toLowerCase() === "b";
}

onMounted(() => {
  media = window.matchMedia?.("(max-width: 1199px)") ?? null;
  onMedia();
  media?.addEventListener("change", onMedia);
  window.addEventListener("keydown", onKey);
  void w.subscribe().then(() => w.load());
});
onBeforeUnmount(() => {
  media?.removeEventListener("change", onMedia);
  window.removeEventListener("keydown", onKey);
  if (partingTimer) clearTimeout(partingTimer);
});
</script>

<template>
  <main ref="root" class="workshop" :class="{ 'is-narrow': narrow }" aria-labelledby="ws-title">
    <div class="ws-room">
      <header class="ws-head">
        <div class="ws-head__top">
          <h1 id="ws-title" class="ws-head__title rs-inscription">THE WORKSHOP</h1>
          <div v-if="fence" class="ws-fence">
            <span class="ws-fence__label rs-mono">FENCE</span>
            <span class="ws-chip rs-mono" :class="`is-${fence.chip}`" data-chip>{{ CHIP_WORD[fence.chip] }}</span>
            <button
              type="button"
              class="ws-fence__fold"
              :aria-expanded="fenceOpen"
              aria-controls="ws-fence-record"
              :aria-label="fenceOpen ? 'Hide the probe record' : 'Show the probe record'"
              @click="fenceOpen = !fenceOpen"
            >
              <span aria-hidden="true">{{ fenceOpen ? "▴" : "▾" }}</span>
            </button>
          </div>
        </div>
        <p class="ws-head__name">
          <span class="ws-head__pawn">{{ PAWN_NAME }}</span>
          <span v-for="part in nameLine" :key="part" class="ws-head__part rs-mono">{{ " · " }}{{ part }}</span>
        </p>
      </header>

      <FenceRecord
        v-if="fence && fenceOpen"
        :fence="fence"
        :probing="w.probing.value"
        :summoned="w.summoned.value"
        @run="press(w.runProbe)"
      />

      <div class="ws-hollow" :class="`is-${place}`">
        <span class="ws-fissure" :class="{ 'is-flaring': parting }" aria-hidden="true" />
        <div class="ws-void" :class="{ 'is-parting': parting }">
          <WorkshopHollow ref="hollow" :live="state === 'here' || state === 'summoning'" />
        </div>

        <div v-if="place === 'closed' || place === 'summoning'" class="ws-stone">
          <template v-if="noFence">
            <p class="ws-stone__carved ws-stone__carved--ember rs-inscription">NO PAWN CAN CROSS</p>
            <p class="ws-stone__body">{{ noFence }}</p>
            <div class="ws-stone__doors">
              <RiftButton size="sm" @click="lookAgain">Look again</RiftButton>
              <RiftButton
                size="sm"
                @click="copyDetails(['Riftstone: no pawn can cross', 'reason: noFence', `detail: ${noFence}`])"
              >
                Copy details
              </RiftButton>
            </div>
          </template>
          <template v-else-if="place === 'summoning'">
            <p class="ws-stone__body">The stone is calling a pawn…</p>
            <div class="ws-stone__doors">
              <RiftButton variant="primary" busy>Summon</RiftButton>
            </div>
          </template>
          <template v-else-if="breached">
            <p class="ws-stone__carved ws-stone__carved--ember rs-inscription">THE FENCE DID NOT HOLD</p>
            <p class="ws-stone__body">
              The last probe reached a place the fence should close<template v-if="broken.length"
                >: {{ broken.join("; ") }}</template
              >. The stone won't summon a pawn until a probe holds.
            </p>
            <div class="ws-stone__doors">
              <RiftButton variant="primary" refused reason="The fence did not hold. Run the probe until it holds.">
                Summon
              </RiftButton>
              <RiftButton size="sm" @click="showRecord">Show the probe record</RiftButton>
              <RiftButton size="sm" :busy="w.probing.value" @click="press(w.runProbe)">Run the probe</RiftButton>
            </div>
          </template>
          <template v-else-if="!ended">
            <p class="ws-stone__carved rs-inscription">NO PAWN IS HERE</p>
            <p class="ws-stone__body">
              Summon a pawn through the stone to build mods with. It works in your workshop folder,
              <span class="ws-stone__path rs-mono">{{ folderPath }}</span
              >, and the stone's own hands install, uninstall and read the REFramework log for it.
            </p>
            <p v-if="!folderMade" class="ws-stone__body">
              The stone makes the workshop folder the first time you summon.
            </p>
            <div class="ws-stone__doors">
              <RiftButton variant="primary" :busy="summoning" @click="summon">Summon</RiftButton>
              <RiftButton v-if="folderMade" @click="press(w.openFolder)">Open workshop folder</RiftButton>
            </div>
            <p v-if="fence?.chip === 'unprobed'" class="ws-stone__note rs-mono">
              FENCE UNPROBED: no probe record matches this ground, so the stone claims nothing about it.
            </p>
          </template>
        </div>
      </div>

      <section v-if="ended && !noFence" class="ws-ended" :class="{ 'is-torn': state === 'torn' }">
        <p class="ws-ended__carved rs-inscription">
          {{ state === "torn" ? "THE PAWN WAS TORN AWAY" : "THE PAWN HAS GONE BACK" }}
        </p>
        <p class="ws-ended__body">
          <template v-if="state === 'torn'">
            {{ PAWN_NAME }}'s session ended without a goodbye (exit code {{ pawn?.exitCode ?? "unknown" }}). The
            workshop keeps everything written before it ended.
          </template>
          <template v-else>
            {{ PAWN_NAME }} went back through the Rift. The workshop keeps everything made here.
          </template>
        </p>
        <div class="ws-ended__doors">
          <RiftButton
            variant="primary"
            size="sm"
            :busy="summoning"
            :refused="breached"
            :reason="breached ? 'The fence did not hold. Run the probe until it holds.' : null"
            @click="summon"
          >
            Summon again
          </RiftButton>
          <RiftButton
            v-if="state === 'torn'"
            size="sm"
            @click="
              copyDetails([
                `Riftstone: ${PAWN_NAME} was torn away`,
                `run: ${pawn?.runId ?? ''}`,
                `exit code: ${pawn?.exitCode ?? 'unknown'}`,
              ])
            "
          >
            Copy details
          </RiftButton>
        </div>
        <PutBackBlock />
      </section>

      <footer class="ws-foot">
        <RiftButton v-if="w.summoned.value" @click="press(w.depart)">Depart</RiftButton>
        <RiftButton v-if="folderMade && place !== 'closed'" @click="press(w.openFolder)"
          >Open workshop folder</RiftButton
        >
        <p v-if="said" class="ws-foot__said" role="alert">{{ said }}</p>
        <p v-else-if="folderMade" class="ws-foot__path rs-mono" :title="w.view.value?.folder.path">
          {{ w.view.value?.folder.path }}
        </p>
      </footer>
    </div>

    <button
      v-if="narrow"
      type="button"
      class="ws-sliver"
      :aria-expanded="benchOpen"
      aria-controls="ws-bench"
      aria-label="The bench (B)"
      @click="benchOpen = !benchOpen"
    >
      <span class="ws-sliver__mark rs-mono">{{ sliverMarks.built }} BUILT</span>
      <span class="ws-sliver__mark rs-mono">{{ sliverMarks.hands }} HANDS</span>
      <span v-if="w.held.value" class="ws-sliver__mark ws-sliver__mark--asks rs-mono">ASKS</span>
    </button>
    <aside
      id="ws-bench"
      class="ws-bench"
      :class="{ 'is-overlay': narrow, 'is-folded': narrow && !benchOpen }"
      aria-label="The bench"
    >
      <WorkshopBench />
    </aside>
  </main>
</template>

<style scoped>
.workshop {
  position: relative;
  flex: 1;
  min-width: 0;
  height: 100%;
  display: flex;
  background: var(--rs-well);
}
.ws-room {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  overflow-y: auto;
}

/* the head: inscription, the carved name, the fence chip on the baseline */
.ws-head {
  flex: none;
  padding: var(--rs-space-5) var(--rs-space-6) var(--rs-space-4);
}
.ws-head__top {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--rs-space-4);
}
.ws-head__title {
  margin: 0;
  color: var(--rs-text);
  font-size: var(--rs-text-xl);
  line-height: 1.15;
}
.ws-head__name {
  margin: var(--rs-space-1) 0 0;
  color: var(--rs-muted);
  font: 400 14px var(--rs-font-sans);
}
.ws-head__pawn {
  color: var(--rs-text);
  font-weight: 500;
}
.ws-head__part {
  color: var(--rs-faint);
  font-size: 12px;
  white-space: pre;
}
.ws-fence {
  display: flex;
  align-items: center;
  gap: var(--rs-space-2);
}
.ws-fence__label {
  color: var(--rs-faint);
}
/* the struck chip: a word and a colour, never colour alone, never green, never a shield */
.ws-chip {
  padding: 2px 6px;
  border: 1px solid currentColor;
  border-radius: var(--rs-radius-chip);
  font-weight: 600;
  letter-spacing: 0.06em;
}
.ws-chip.is-contained {
  color: var(--rs-text);
}
.ws-chip.is-unprobed {
  color: var(--rs-faint);
}
.ws-chip.is-breached {
  border-color: var(--rs-ember);
  background: var(--rs-ember);
  color: var(--rs-ongold);
}
.ws-fence__fold {
  display: grid;
  place-items: center;
  width: 28px;
  height: 28px;
  border: 1px solid transparent;
  background: none;
  color: var(--rs-muted);
  font: 400 14px var(--rs-font-mono);
  cursor: pointer;
}
.ws-fence__fold:hover {
  border-color: var(--rs-edgehi);
  color: var(--rs-text);
}

/* the hollow: a recess in the stone, lit only by its fissure */
.ws-hollow {
  position: relative;
  flex: 1;
  min-height: 240px;
  margin: 0 var(--rs-space-6);
  background: var(--rs-well);
}
.ws-fissure {
  position: absolute;
  z-index: 2;
  top: 0;
  bottom: 0;
  left: 0;
  width: 2px;
  background: var(--rs-rift);
  opacity: 0.18;
}
.ws-hollow.is-summoning .ws-fissure,
.ws-hollow.is-live .ws-fissure {
  opacity: 0.6;
}
.ws-void {
  position: absolute;
  inset: 0;
  padding: var(--rs-space-3) var(--rs-space-4) var(--rs-space-3) var(--rs-space-5);
  background: var(--rs-void);
  box-shadow: inset 0 2px 0 rgba(0, 0, 0, 0.45);
  clip-path: inset(0 100% 0 0);
}
.ws-hollow.is-live .ws-void,
.ws-hollow.is-departed .ws-void {
  clip-path: inset(0);
}
/* departed: open but dimmed, read-only and still scrollable; no transition in either mode */
.ws-hollow.is-departed .ws-void {
  opacity: 0.55;
}
/* THE PARTING (R20.17): the fissure flares (quick), the void opens from it
 * rightward (answer), the fissure decays to its live rest. Resting frames:
 * the live place itself, so the clamp lands exactly there. */
.ws-fissure.is-flaring {
  animation: ws-flare calc(var(--rs-dur-quick) * 2 + var(--rs-dur-answer)) linear both;
}
.ws-void.is-parting {
  animation: ws-part var(--rs-dur-answer) var(--rs-ease-answer) var(--rs-dur-quick) both;
}
@keyframes ws-flare {
  0% {
    opacity: 0.6;
    box-shadow: none;
  }
  14%,
  86% {
    opacity: 1;
    box-shadow: var(--rs-glow-rift);
  }
  100% {
    opacity: 0.6;
    box-shadow: none;
  }
}
@keyframes ws-part {
  from {
    clip-path: inset(0 100% 0 0);
  }
  to {
    clip-path: inset(0);
  }
}
@media (prefers-reduced-motion: reduce) {
  .ws-fissure.is-flaring,
  .ws-void.is-parting {
    animation: none;
  }
}

/* the closed stone: no recess carve, the carved words centred */
.ws-stone {
  position: absolute;
  z-index: 1;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--rs-space-3);
  padding: var(--rs-space-8) var(--rs-space-12);
  background: var(--rs-well);
  text-align: center;
}
.ws-stone__carved {
  margin: 0;
  color: var(--rs-muted);
  font-size: var(--rs-text-2xl);
  line-height: 1.1;
}
.ws-stone__carved--ember {
  color: var(--rs-ember);
}
.ws-stone__body {
  margin: 0;
  max-width: 52ch;
  color: var(--rs-muted);
  font: 400 var(--rs-text-sm) / 1.5 var(--rs-font-sans);
}
.ws-stone__path {
  color: var(--rs-text);
  font-size: 12px;
}
.ws-stone__doors {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  align-items: center;
  gap: var(--rs-space-2);
  margin-top: var(--rs-space-2);
}
.ws-stone__note {
  margin: 0;
  max-width: 52ch;
  color: var(--rs-faint);
}

/* departed: the carved line above the foot */
.ws-ended {
  position: relative;
  flex: none;
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-2);
  margin: var(--rs-space-4) var(--rs-space-6) 0;
  padding-left: var(--rs-space-4);
}
.ws-ended::before {
  content: "";
  position: absolute;
  left: 0;
  top: 2px;
  bottom: 2px;
  width: 4px;
  background: var(--rs-edgehi);
}
.ws-ended.is-torn::before {
  background: var(--rs-ember);
}
.ws-ended__carved {
  margin: 0;
  color: var(--rs-text);
  font-size: var(--rs-text-md);
}
.ws-ended.is-torn .ws-ended__carved {
  color: var(--rs-ember);
}
.ws-ended__body {
  margin: 0;
  color: var(--rs-muted);
  font: 400 var(--rs-text-sm) / 1.45 var(--rs-font-sans);
}
.ws-ended__doors {
  display: flex;
  gap: var(--rs-space-2);
}

.ws-foot {
  flex: none;
  display: flex;
  align-items: center;
  gap: var(--rs-space-2);
  min-height: 44px;
  margin: var(--rs-space-3) var(--rs-space-6) var(--rs-space-4);
}
.ws-foot__path {
  min-width: 0;
  margin: 0 0 0 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--rs-faint);
}
.ws-foot__said {
  margin: 0 0 0 var(--rs-space-2);
  color: var(--rs-ember);
  font: 400 var(--rs-text-sm) var(--rs-font-sans);
}

/* the bench: basalt, a hairline on its left */
.ws-bench {
  flex: none;
  width: 320px;
  height: 100%;
  overflow-y: auto;
  background: var(--rs-basalt);
  border-left: 1px solid var(--rs-edge);
}
/* 1024: the bench folds to a sliver and unfolds over the hollow (R20.3) */
.ws-bench.is-overlay {
  position: absolute;
  z-index: var(--rs-z-world-overlay);
  top: 0;
  right: 0;
  bottom: 0;
  box-shadow: -6px 0 0 rgba(0, 0, 0, 0.55);
}
.ws-bench.is-folded {
  display: none;
}
.ws-sliver {
  flex: none;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--rs-space-5);
  width: 56px;
  padding: var(--rs-space-5) 0;
  border: 0;
  border-left: 1px solid var(--rs-edge);
  background: var(--rs-basalt);
  cursor: pointer;
  transition: background-color var(--rs-dur-settle) var(--rs-ease-carve);
}
.ws-sliver:hover {
  background: var(--rs-misthi);
}
.ws-sliver__mark {
  writing-mode: vertical-rl;
  letter-spacing: 0.2em;
  color: var(--rs-muted);
}
.ws-sliver__mark--asks {
  color: var(--rs-warn);
}
</style>
