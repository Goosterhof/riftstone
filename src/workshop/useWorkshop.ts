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
/** This summoning's hands, newest first. */
const hands = ref<PawnHand[]>([]);
/** The run the hands and the bytes belong to. */
const runId = ref<string | null>(null);
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

let sink: OutputSink | null = null;
/** Bytes that arrived before the terminal listened, in order. */
let waiting: Uint8Array[] = [];
let nextSeq = 1;
/** Out-of-order chunks, by seq, until the gap fills. */
const early = new Map<number, Uint8Array>();
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

/** `rift://pawn-output`: the window drops nothing and writes in order. */
function onOutput(out: PawnOutput): void {
  if (out.seq < nextSeq) return;
  early.set(out.seq, decode(out.data));
  for (let next = early.get(nextSeq); next; next = early.get(nextSeq)) {
    early.delete(nextSeq);
    nextSeq += 1;
    deliver(next);
  }
}

function newRun(id: string | null): void {
  runId.value = id;
  hands.value = [];
  firstBytes.value = false;
  nextSeq = 1;
  early.clear();
  waiting = [];
  playWarnings.value = null;
  playRefusal.value = null;
  playAnswered.value = null;
}

function onState(next: PawnStateView): void {
  if (next.runId !== runId.value && (next.state === "summoning" || next.state === "here")) newRun(next.runId);
  if (!next.held) {
    playWarnings.value = null;
    playRefusal.value = null;
  }
  if (view.value) view.value = { ...view.value, pawn: next };
}

function onHand(hand: PawnHand): void {
  if (hand.runId !== runId.value) newRun(hand.runId);
  if (hands.value.some((h) => h.seq === hand.seq)) return;
  hands.value = [hand, ...hands.value].sort((a, b) => b.seq - a.seq);
}

async function load(): Promise<void> {
  const backend = await workshop();
  const next = await backend.getWorkshop();
  if (runId.value === null && next.pawn.runId) runId.value = next.pawn.runId;
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

async function answer(accept: boolean): Promise<void> {
  const run = runId.value;
  const waitingOn = held.value;
  if (!run || !waitingOn || answering.value) return;
  answering.value = true;
  try {
    const result = await attempt(async () => (await workshop()).answerPawnPlay(run, waitingOn.heldRequestId, accept));
    if (result) {
      playAnswered.value = result;
      playWarnings.value = null;
      playRefusal.value = null;
    } else if (failure.value) {
      // A refusal or a failure records nothing: the request stays waiting with it shown.
      playRefusal.value = failure.value.message;
    }
  } finally {
    answering.value = false;
  }
}

/** `Play` on the held request: §16.2's plan first. A refusal is shown and the
 *  request stays waiting; warnings wait for `Play anyway` (confirmPlay). */
async function pressPlay(): Promise<void> {
  if (!held.value) return;
  playRefusal.value = null;
  const plan = await useLaunch().refreshPlan();
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
  await answer(true);
}

async function confirmPlay(): Promise<void> {
  await answer(true);
}

function cancelWarnings(): void {
  // A cancel at a warning records nothing; the request stays waiting.
  playWarnings.value = null;
}

async function declinePlay(): Promise<void> {
  await answer(false);
}

async function planBring(artifact: string): Promise<BringThroughPlan | null> {
  return attempt(async () => (await workshop()).planBringThrough(artifact));
}

/** The press refers to the plan's frozen digest, never the file's digest now. */
async function bring(plan: BringThroughPlan): Promise<"landed" | "alreadyHad" | null> {
  const result = await attempt(async () => (await workshop()).bringThrough(plan.artifact, plan.sha256));
  if (result) {
    await load();
    await (await rift()).rescan().catch(() => undefined);
  }
  return result?.outcome ?? null;
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
  newRun(null);
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
