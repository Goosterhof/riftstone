// THE WORKSHOP's state (WINDOW-CONTRACT § v0.7): the view the stone sends, the
// pawn's hands this summoning, the PTY's bytes in order, and the presses. One
// store for the room, so the terminal keeps its conversation across rooms
// (#00015 §20.12). The window never shows a chip, a digest or a state the stone
// did not send.

import { computed, ref, shallowRef } from "vue";
import type { LaunchWarning } from "@/types/rift";
import {
  WORKSHOP_EVENTS,
  type BringThroughPlan,
  type FenceView,
  type PawnHand,
  type PawnOutput,
  type PawnStateView,
  type PutBackPlan,
  type Voice,
  type WorkshopView,
} from "@/types/workshop";
import { asRiftFailure, rift, type RiftFailure } from "@/services/rift";
import { useLaunch } from "@/composables/useLaunch";
import { workshop } from "./backend";

export type OutputSink = (bytes: Uint8Array) => void;

const view = ref<WorkshopView | null>(null);
/** The run on show: its hands and its bytes. Only the pawn's state moves it. */
const runId = ref<string | null>(null);
/** Every run's hands as they arrived, so a late row from another run never
 *  displaces the one on show (the Heretic's note on PR #11). */
const handsByRun = ref<Record<string, PawnHand[]>>({});
/** The run on show's hands, newest first. */
const hands = computed<PawnHand[]>(() =>
  [...(runId.value ? (handsByRun.value[runId.value] ?? []) : [])].sort((a, b) => b.seq - a.seq),
);
/** The PTY's first bytes of this run have arrived: THE PARTING may run (R20.17). */
const firstBytes = ref(false);
const probing = ref(false);
const failure = shallowRef<RiftFailure | null>(null);

/** The Play press on a held request: its warnings while the investor decides,
 *  the refusal that left it waiting, and what the answer did. */
const playWarnings = ref<LaunchWarning[] | null>(null);
const playRefusal = ref<string | null>(null);
const playAnswered = ref<{ decision: "played" | "declined"; persisted: boolean } | null>(null);
const answering = ref(false);

/** The request a Play press was made on, frozen at the click (the Heretic's WR3):
 *  a plan that comes back after the waiting request changed answers nothing. */
interface PlayTarget {
  runId: string;
  heldRequestId: string;
}
let playFor: PlayTarget | null = null;

function pressOwner(): PlayTarget | null {
  return playFor;
}

let sink: OutputSink | null = null;
/** Bytes that arrived before the terminal listened, in order. */
let waiting: Uint8Array[] = [];
/** One run's chunks: the spine's pump numbers them from 0 (summon.rs). A run
 *  that is not on show yet keeps its in-order bytes until the pawn's state
 *  puts it there (the Heretic's WR6: output may come before that state). */
interface RunOutput {
  nextSeq: number;
  /** Out-of-order chunks, by seq, until the gap fills. */
  early: Map<number, Uint8Array>;
  /** In order, not yet shown. */
  held: Uint8Array[];
}
const outputs = new Map<string, RunOutput>();
/** Runs another took the place of: their late bytes are dropped. */
const retired = new Set<string>();
let subscribed: Promise<void> | null = null;

const pawn = computed<PawnStateView | null>(() => view.value?.pawn ?? null);
const here = computed(() => pawn.value?.state === "here");
const summoned = computed(() => pawn.value?.state === "here" || pawn.value?.state === "summoning");
const held = computed(() => pawn.value?.held ?? null);
const fence = computed<FenceView | null>(() => view.value?.fence ?? null);

function decode(data: string): Uint8Array {
  const binary = atob(data);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

function deliver(bytes: Uint8Array): void {
  if (!firstBytes.value) firstBytes.value = true;
  if (sink) sink(bytes);
  else waiting.push(bytes);
}

function outputOf(run: string): RunOutput {
  let found = outputs.get(run);
  if (!found) {
    found = { nextSeq: 0, early: new Map(), held: [] };
    outputs.set(run, found);
  }
  return found;
}

/** `rift://pawn-output`: the window drops nothing of the run on show and
 *  writes it in order; a run not on show yet waits for its state. */
function onOutput(out: PawnOutput): void {
  if (retired.has(out.runId)) return;
  const run = outputOf(out.runId);
  if (out.seq < run.nextSeq) return;
  run.early.set(out.seq, decode(out.data));
  for (let next = run.early.get(run.nextSeq); next; next = run.early.get(run.nextSeq)) {
    run.early.delete(run.nextSeq);
    run.nextSeq += 1;
    if (out.runId === runId.value) deliver(next);
    else run.held.push(next);
  }
}

function newRun(id: string | null): void {
  const previous = runId.value;
  if (previous && previous !== id) {
    retired.add(previous);
    outputs.delete(previous);
  }
  // The terminal resets on this change (synchronously), before the new
  // run's bytes that arrived early are handed over below.
  runId.value = id;
  firstBytes.value = false;
  waiting = [];
  playWarnings.value = null;
  playRefusal.value = null;
  playAnswered.value = null;
  playFor = null;
  const early = id ? outputs.get(id) : undefined;
  if (early) {
    const shown = early.held;
    early.held = [];
    for (const bytes of shown) deliver(bytes);
  }
}

function onState(next: PawnStateView): void {
  if (next.runId !== runId.value && (next.state === "summoning" || next.state === "here")) newRun(next.runId);
  if (!next.held) {
    playWarnings.value = null;
    playRefusal.value = null;
    playFor = null;
  }
  if (view.value) view.value = { ...view.value, pawn: next };
}

function onHand(hand: PawnHand): void {
  const rows = handsByRun.value[hand.runId] ?? [];
  if (rows.some((h) => h.seq === hand.seq)) return;
  handsByRun.value = { ...handsByRun.value, [hand.runId]: [...rows, hand] };
}

async function load(): Promise<void> {
  const backend = await workshop();
  const next = await backend.getWorkshop();
  if (runId.value === null && next.pawn.runId) newRun(next.pawn.runId);
  view.value = next;
}

async function subscribe(): Promise<void> {
  subscribed ??= (async () => {
    const backend = await workshop();
    await backend.on<PawnOutput>(WORKSHOP_EVENTS.pawnOutput, onOutput);
    await backend.on<PawnStateView>(WORKSHOP_EVENTS.pawnState, onState);
    await backend.on<PawnHand>(WORKSHOP_EVENTS.pawnHands, onHand);
    await backend.on(WORKSHOP_EVENTS.workshopChanged, () => void load());
  })();
  return subscribed;
}

/** The terminal listens: it gets every byte that waited, then the rest as they come. */
function attach(next: OutputSink): () => void {
  sink = next;
  for (const bytes of waiting) next(bytes);
  waiting = [];
  return () => {
    if (sink === next) sink = null;
  };
}

async function attempt<T>(work: () => Promise<T>): Promise<T | null> {
  failure.value = null;
  try {
    return await work();
  } catch (raw) {
    failure.value = asRiftFailure(raw);
    return null;
  }
}

async function summon(cols: number, rows: number): Promise<void> {
  await attempt(async () => (await workshop()).summonPawn(cols, rows));
  await load();
}

async function write(data: string): Promise<void> {
  if (!here.value) return;
  await (await workshop()).pawnWrite(data).catch(() => undefined);
}

async function resize(cols: number, rows: number): Promise<void> {
  await (await workshop()).pawnResize(cols, rows).catch(() => undefined);
}

async function depart(): Promise<void> {
  await attempt(async () => (await workshop()).departPawn());
}

async function runProbe(): Promise<void> {
  if (probing.value) return;
  probing.value = true;
  try {
    const next = await attempt(async () => (await workshop()).runFenceProbe());
    if (next && view.value) view.value = { ...view.value, fence: next };
  } finally {
    probing.value = false;
  }
}

async function setVoice(next: Voice): Promise<void> {
  const done = await attempt(async () => (await workshop()).setPawnVoice(next));
  if (done !== null && view.value) view.value = { ...view.value, voice: next };
}

async function setInputs(next: string[]): Promise<boolean> {
  const done = await attempt(async () => (await workshop()).setPawnInputs(next));
  if (done === null) return false;
  if (view.value) view.value = { ...view.value, inputs: [...next] };
  return true;
}

async function openFolder(): Promise<void> {
  await attempt(async () => (await workshop()).openWorkshopFolder());
  await load();
}

/** The request waiting now, as the stone last said it. */
function waitingNow(): PlayTarget | null {
  const run = runId.value;
  const waiting = held.value;
  return run && waiting ? { runId: run, heldRequestId: waiting.heldRequestId } : null;
}

/** Is `target` still the request waiting now? */
function stillWaiting(target: PlayTarget): boolean {
  const now = waitingNow();
  return now !== null && now.runId === target.runId && now.heldRequestId === target.heldRequestId;
}

const MOVED_ON = "That request is no longer the one waiting, so the press answered nothing.";

async function answer(target: PlayTarget, accept: boolean): Promise<void> {
  if (answering.value) return;
  if (!stillWaiting(target)) {
    playFor = null;
    playWarnings.value = null;
    playRefusal.value = MOVED_ON;
    return;
  }
  answering.value = true;
  try {
    const result = await attempt(async () =>
      (await workshop()).answerPawnPlay(target.runId, target.heldRequestId, accept),
    );
    if (result) {
      playAnswered.value = result;
      playWarnings.value = null;
      playRefusal.value = null;
      playFor = null;
      // The stone's own word on what is waiting now.
      await load();
    } else if (failure.value) {
      // A refusal or a failure records nothing: the request stays waiting with it shown.
      playRefusal.value = failure.value.message;
    }
  } finally {
    answering.value = false;
  }
}

/** `Play` on the held request: §16.2's plan first. The request is the one on
 *  screen at the click. A refusal is shown and it stays waiting; warnings wait
 *  for `Play anyway` (confirmPlay), on that same request. */
async function pressPlay(): Promise<void> {
  const target = waitingNow();
  if (!target) return;
  playFor = target;
  playRefusal.value = null;
  const plan = await useLaunch().refreshPlan();
  // A later press owns the answer now (read fresh: the await may have moved it).
  const owner = pressOwner();
  if (owner !== null && owner !== target) return;
  if (!stillWaiting(target)) {
    playFor = null;
    playRefusal.value = MOVED_ON;
    return;
  }
  if (!plan) {
    playRefusal.value = "The stone could not read what Play would do. Try again.";
    return;
  }
  if (plan.refused) {
    playRefusal.value = plan.refused;
    return;
  }
  if (plan.warnings.length) {
    playWarnings.value = plan.warnings;
    return;
  }
  await answer(target, true);
}

async function confirmPlay(): Promise<void> {
  if (playFor) await answer(playFor, true);
}

function cancelWarnings(): void {
  // A cancel at a warning records nothing; the request stays waiting.
  playWarnings.value = null;
  playFor = null;
}

async function declinePlay(): Promise<void> {
  const target = waitingNow();
  if (target) await answer(target, false);
}

async function planBring(artifact: string): Promise<BringThroughPlan | null> {
  return attempt(async () => (await workshop()).planBringThrough(artifact));
}

/** The press refers to the plan's frozen digest, never the file's digest now. */
async function bring(plan: BringThroughPlan): Promise<"landed" | "alreadyHad" | null> {
  const result = await attempt(async () => (await workshop()).bringThrough(plan.artifact, plan.sha256));
  if (result) {
    await load();
    // The scan the press requests names the landed zip as a package; the card
    // does not wait on it (the window reads it from rift://library-changed).
    void rescanQuietly();
  }
  return result?.outcome ?? null;
}

/** Ask for a rescan and let it run; its answer arrives as rift://library-changed. */
async function rescanQuietly(): Promise<void> {
  try {
    await (await rift()).rescan();
  } catch {
    // A scan that fails says so through the stone's own events.
  }
}

async function planPutBack(): Promise<PutBackPlan | null> {
  const run = runId.value ?? pawn.value?.runId ?? null;
  if (!run) return null;
  return attempt(async () => (await workshop()).planPutBack(run));
}

async function putBack(plan: PutBackPlan): Promise<boolean> {
  // The journal keys an install by its option: a remove item's optionId is the
  // install id the spine planned it from (plan_put_back_impl).
  const ids = plan.remove.map((r) => r.optionId);
  const done = await attempt(async () => (await workshop()).putBack(plan.runId, ids));
  return done !== null;
}

/** For specs: forget everything, as a fresh window would. */
export function resetWorkshop(): void {
  view.value = null;
  handsByRun.value = {};
  outputs.clear();
  retired.clear();
  newRun(null);
  retired.clear();
  probing.value = false;
  failure.value = null;
  answering.value = false;
  sink = null;
  subscribed = null;
}

export function useWorkshop() {
  return {
    view,
    pawn,
    here,
    summoned,
    held,
    fence,
    hands,
    runId,
    firstBytes,
    probing,
    failure,
    playWarnings,
    playRefusal,
    playAnswered,
    answering,
    load,
    subscribe,
    attach,
    summon,
    write,
    resize,
    depart,
    runProbe,
    setVoice,
    setInputs,
    openFolder,
    pressPlay,
    confirmPlay,
    cancelWarnings,
    declinePlay,
    planBring,
    bring,
    planPutBack,
    putBack,
  };
}
