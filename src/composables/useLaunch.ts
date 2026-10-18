// PLAY (v0.3): what pressing Play would do now (plan_launch: its route, its
// warnings, or the one sentence that refuses it), the press itself
// (launch_game), and whether Dragon's Dogma 2 runs. The running state is the
// spine's: its game watcher samples the process list and emits
// rift://game-changed when the game starts or stops, and the overview's
// GameSummary carries `running` and `startedMs` (WINDOW-CONTRACT § v0.3 Play).
//
// Warnings never block: PlayStone holds them in its inline confirm on the
// stone before the press (R16.14: Play anyway starts the game, Fix it opens
// PREFLIGHT), and no toast repeats them. `launching` holds from the press until the watcher sees the game
// (or a failure, or LAUNCH_WAIT_MS without a sighting): THE WORLD STIRS. Play
// stays pressable under it (R16.13): a start that never comes must leave no
// dead stone, so a second press plans again and asks Steam or Windows again
// (which bring an already-starting game forward), and the wait starts over.
// Only a press whose plan and launch_game are still in flight swallows the next.

import { computed, ref } from "vue";
import type { LaunchPlanView, LaunchReport } from "@/types/rift";
import { asRiftFailure, rift } from "@/services/rift";
import { voice, voicedLine, type Voiced } from "@/services/voice";
import { useStone } from "./useStone";
import { useToasts } from "./useToasts";

/** How long the stone waits for the watcher to see the game after a press:
 *  the spine samples every second for up to 120 s after a launch. */
export const LAUNCH_WAIT_MS = 120_000;

const plan = ref<LaunchPlanView | null>(null);
/** The plan on hand was asked for or answered while the game ran: its
 *  refusal ("already running") belongs to that run. */
const planUnderRun = ref(false);
const planning = ref(false);
const launching = ref(false);
const lastLaunch = ref<LaunchReport | null>(null);
const failure = ref<Voiced | null>(null);
let waitTimer: ReturnType<typeof setTimeout> | null = null;
/** A press's plan_launch and launch_game are on their way: the next press waits for them. */
let asking = false;

const game = computed(() => useStone().overview.value?.game ?? null);
const running = computed(() => game.value?.running ?? false);
const startedMs = computed(() => game.value?.startedMs ?? null);
/** The route Play takes: the plan's once read, else the overview's. */
const route = computed(() => plan.value?.launch ?? game.value?.launch ?? null);
/** Play can be pressed: a game is known, it is not running, no press is in flight. */
const ready = computed(() => game.value !== null && !running.value && !launching.value);
/** A plan read while the game ran, now that the overview says it stopped: void
 *  until it is read again, so Play never says "already running" for a game
 *  that is not (the overview lands before the plan's re-read). */
const planStale = computed(() => planUnderRun.value && !running.value);

/** Ask the spine for Play's plan, and remember whether the game ran meanwhile. */
async function askPlan(): Promise<LaunchPlanView> {
  const ranAtAsk = running.value;
  const backend = await rift();
  const next = await backend.planLaunch();
  plan.value = next;
  planUnderRun.value = ranAtAsk || running.value;
  return next;
}

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
    const next = await askPlan();
    failure.value = null;
    return next;
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
 *  the game. Resolves true once Steam or Windows was asked to start it. A
 *  press while the world stirs asks again and starts the wait over; only a
 *  press whose own asks are still on their way is not answered twice. */
async function play(): Promise<boolean> {
  if (asking || running.value) return false;
  asking = true;
  launching.value = true;
  failure.value = null;
  try {
    const next = await askPlan();
    if (next.refused) return failed(voice({ kind: "refused", detail: { reason: next.refused } }, "launch"));
    const backend = await rift();
    const report = await backend.launchGame();
    lastLaunch.value = report;
    // §9 / §16: the slab says it (THE WORLD STIRS), and the warnings were the
    // confirm's: no toast repeats either, and no gold is spent on a press.
    // the watcher's sighting (onGameChanged) ends the press; this only bounds the wait
    armWait();
    return true;
  } catch (raw) {
    return failed(voice(asRiftFailure(raw), "launch"));
  } finally {
    asking = false;
  }
}

/** rift://game-changed, after the overview was read again: the watcher saw
 *  the game start (the press is over) or stop, or a change landed; the plan is
 *  read again so no warning or refusal outlives its cause. */
async function onGameChanged(): Promise<void> {
  if (launching.value && running.value) settle();
  await refreshPlan();
}

export function useLaunch() {
  return {
    plan,
    planStale,
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
  asking = false;
  plan.value = null;
  planUnderRun.value = false;
  planning.value = false;
  lastLaunch.value = null;
  failure.value = null;
}
