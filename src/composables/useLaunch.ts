// PLAY (v0.3): what pressing Play would do now (plan_launch: its route, its
// warnings, or the one sentence that refuses it), the press itself
// (launch_game), and whether Dragon's Dogma 2 runs. The running state is the
// spine's: its game watcher samples the process list and emits
// rift://game-changed when the game starts or stops, and the overview's
// GameSummary carries `running` and `startedMs` (WINDOW-CONTRACT § v0.3 Play).
//
// Warnings never block: they ride along in the toast that says the game is
// starting. `launching` holds from the press until the watcher sees the game
// (or a failure, or LAUNCH_WAIT_MS without a sighting), so a second press in
// between cannot start a second game.

import { computed, ref } from "vue";
import type { LaunchPlanView, LaunchReport } from "@/types/rift";
import { asRiftFailure, rift } from "@/services/rift";
import { launchedLine } from "@/services/ledger";
import { voice, voicedLine, type Voiced } from "@/services/voice";
import { useStone } from "./useStone";
import { useToasts } from "./useToasts";

/** How long the stone waits for the watcher to see the game after a press:
 *  the spine samples every second for up to 120 s after a launch. */
export const LAUNCH_WAIT_MS = 120_000;

const plan = ref<LaunchPlanView | null>(null);
const planning = ref(false);
const launching = ref(false);
const lastLaunch = ref<LaunchReport | null>(null);
const failure = ref<Voiced | null>(null);
let waitTimer: ReturnType<typeof setTimeout> | null = null;

const game = computed(() => useStone().overview.value?.game ?? null);
const running = computed(() => game.value?.running ?? false);
const startedMs = computed(() => game.value?.startedMs ?? null);
/** The route Play takes: the plan's once read, else the overview's. */
const route = computed(() => plan.value?.launch ?? game.value?.launch ?? null);
/** Play can be pressed: a game is known, it is not running, no press is in flight. */
const ready = computed(() => game.value !== null && !running.value && !launching.value);

function settle(): void {
  if (waitTimer) clearTimeout(waitTimer);
  waitTimer = null;
  launching.value = false;
}

function armWait(): void {
  if (waitTimer) clearTimeout(waitTimer);
  waitTimer = setTimeout(() => {
    waitTimer = null;
    if (!launching.value || running.value) return;
    launching.value = false;
    useToasts().push(
      "warn",
      "The stone has not seen Dragon's Dogma 2 start yet. If Steam is asking something, answer it there.",
    );
  }, LAUNCH_WAIT_MS);
}

/** Read what Play would do now, without pressing it. */
async function refreshPlan(): Promise<LaunchPlanView | null> {
  planning.value = true;
  try {
    const backend = await rift();
    plan.value = await backend.planLaunch();
    failure.value = null;
    return plan.value;
  } catch (raw) {
    failure.value = voice(asRiftFailure(raw), "launch");
    return null;
  } finally {
    planning.value = false;
  }
}

function failed(voiced: Voiced): false {
  settle();
  failure.value = voiced;
  useToasts().push("ember", voicedLine(voiced));
  return false;
}

/** Press Play: plan first (a refusal is said, never pressed past), then start
 *  the game. Resolves true once Steam or Windows was asked to start it. */
async function play(): Promise<boolean> {
  if (launching.value || running.value) return false;
  launching.value = true;
  failure.value = null;
  try {
    const backend = await rift();
    const next = await backend.planLaunch();
    plan.value = next;
    if (next.refused) return failed(voice({ kind: "refused", detail: { reason: next.refused } }, "launch"));
    const report = await backend.launchGame();
    lastLaunch.value = report;
    useToasts().push(next.warnings.length ? "warn" : "world", launchedLine(report.launch, next.warnings));
    // the watcher's sighting (onGameChanged) ends the press; this only bounds the wait
    armWait();
    return true;
  } catch (raw) {
    return failed(voice(asRiftFailure(raw), "launch"));
  }
}

/** rift://game-changed, after the overview was read again: the watcher saw
 *  the game start (the press is over) or stop; an open plan is read again so
 *  no warning or refusal outlives its cause. */
async function onGameChanged(): Promise<void> {
  if (launching.value && running.value) settle();
  if (plan.value) await refreshPlan();
}

export function useLaunch() {
  return {
    plan,
    planning,
    launching,
    lastLaunch,
    failure,
    running,
    startedMs,
    route,
    ready,
    refreshPlan,
    play,
    onGameChanged,
  };
}

export function resetLaunch(): void {
  settle();
  plan.value = null;
  planning.value = false;
  lastLaunch.value = null;
  failure.value = null;
}
