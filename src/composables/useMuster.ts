// THE MUSTER's state (#00015 §17): the slot list, the picked slot and the
// session's memory of it, the party read from it, which member's sheet is
// open, and THE RE-CARVING — the marks a rift://muster-changed re-read
// leaves on exactly the values that changed (R17.25).
//
// Picking reads a file, so only a pick, a door or the save watcher ever
// calls get_muster: moving the focus through the slot list never does.
// Privacy: nothing here logs, toasts or announces a name the save gives.

import { computed, ref, shallowRef } from "vue";
import type { MusterMember, MusterSlotsView, MusterView } from "@/types/rift";
import { asRiftFailure, rift, type RiftFailure } from "@/services/rift";
import { voice, type Voiced } from "@/services/voice";
import { NO_MARKS, recarve, type Recarving } from "@/services/recarving";
import { recarvedSentence } from "@/services/muster";

/** The four places of the tab line (R17.8): position is identity. */
export const PLACES = ["arisen", "mainPawn", "hired1", "hired2"] as const;
export type Place = (typeof PLACES)[number];

/** How long a read may run before its quiet state shows (R17.22: no flash). */
export const SLOW_MS = 200;

type ReadCause = "pick" | "retry" | "changed";

const slotsView = shallowRef<MusterSlotsView | null>(null);
const slotsFailure = shallowRef<RiftFailure | null>(null);
const listingSlow = ref(false);
/** The slot whose party the sheet column is about. */
const pickedId = ref<string | null>(null);
/** The session's memory of the last pick (never written to settings). */
let rememberedId: string | null = null;
const view = shallowRef<MusterView | null>(null);
const viewFailure = shallowRef<RiftFailure | null>(null);
const reading = ref(false);
const readingSlow = ref(false);
const activePlace = ref<Place>("arisen");
const marks = shallowRef<Recarving>(NO_MARKS);
/** True for the one frame the re-carved values paint in their light. */
const lit = ref(false);
const readAgainAt = ref<number | null>(null);
const announcement = ref("");
/** `5` brought the user here: the page puts the focus on the picked slot. */
const focusSummoned = ref(false);
/** The slot list asks the page to bring the focus to the newest slot that opens. */
const pickSlotSummons = ref(0);

/** The last successful read of each slot, the diff's ground. */
const lastRead = new Map<string, MusterView>();
/** Where each hired pawn stood in each slot's tab line (by memberKey), so a
 *  pawn keeps its place when the other leaves (R17.8). */
const hiredPlaces = new Map<string, Map<string, Place>>();
let readSeq = 0;
let listSeq = 0;

function reduced(): boolean {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/** The hired pawns placed: a key that stood somewhere before stays there. */
function placeHired(slotId: string, hired: MusterMember[]): Partial<Record<Place, MusterMember>> {
  const before = hiredPlaces.get(slotId) ?? new Map<string, Place>();
  const placed: Partial<Record<Place, MusterMember>> = {};
  const rest: MusterMember[] = [];
  for (const member of hired) {
    const place = before.get(member.memberKey);
    if (place && !placed[place]) placed[place] = member;
    else rest.push(member);
  }
  for (const member of rest) {
    const free = (["hired1", "hired2"] as const).find((p) => !placed[p]);
    if (free) placed[free] = member;
  }
  const after = new Map<string, Place>();
  for (const p of ["hired1", "hired2"] as const) {
    const m = placed[p];
    if (m) after.set(m.memberKey, p);
  }
  hiredPlaces.set(slotId, after);
  return placed;
}

const places = computed<Record<Place, MusterMember | null>>(() => {
  const v = view.value;
  const empty = { arisen: null, mainPawn: null, hired1: null, hired2: null };
  if (!v) return empty;
  const hired = hiredPlacement.value;
  return { arisen: v.arisen, mainPawn: v.mainPawn, hired1: hired.hired1 ?? null, hired2: hired.hired2 ?? null };
});

const hiredPlacement = shallowRef<Partial<Record<Place, MusterMember>>>({});

const members = computed(() => PLACES.filter((p) => places.value[p] !== null));

const activeMember = computed<MusterMember | null>(() => places.value[activePlace.value]);

const picked = computed(() => slotsView.value?.slots.find((s) => s.slotId === pickedId.value) ?? null);

/** The voiced failure of the open slot, with its file named for the voice. */
const viewVoiced = computed<Voiced | null>(() => {
  const failure = viewFailure.value;
  if (!failure) return null;
  return voice({ kind: failure.kind, detail: { ...failure.detail, file: pickedId.value ?? "" } }, "muster");
});

const slotsVoiced = computed<Voiced | null>(() => (slotsFailure.value ? voice(slotsFailure.value, "muster") : null));

function slowAfter(flag: typeof listingSlow): () => void {
  const timer = setTimeout(() => (flag.value = true), SLOW_MS);
  return () => {
    clearTimeout(timer);
    flag.value = false;
  };
}

/** get_muster_slots. Resolves to the list, or null when it failed. */
async function loadSlots(): Promise<MusterSlotsView | null> {
  const mine = ++listSeq;
  const done = slowAfter(listingSlow);
  try {
    const backend = await rift();
    const next = await backend.getMusterSlots();
    if (mine !== listSeq) return null;
    slotsView.value = next;
    slotsFailure.value = null;
    return next;
  } catch (e) {
    if (mine === listSeq) slotsFailure.value = asRiftFailure(e);
    return null;
  } finally {
    if (mine === listSeq) done();
  }
}

function clearMarks(): void {
  marks.value = NO_MARKS;
  lit.value = false;
}

/** Light the re-carved values for one frame, then let them settle (R17.28).
 *  Under reduced motion they are painted at rest at once: the marks and the
 *  count are places, not phases. */
function light(): void {
  if (reduced() || marks.value.count === 0) return;
  lit.value = true;
  requestAnimationFrame(() => requestAnimationFrame(() => (lit.value = false)));
}

/** A read landed. Only a save-watcher re-read of the party already on screen
 *  is re-carved (R17.25 step 1): never a first read, a pick, or Read again. */
function landed(slotId: string, next: MusterView, cause: ReadCause, sameSlot: boolean): void {
  const before = lastRead.get(slotId);
  const recarving = cause === "changed" && sameSlot && before ? recarve(before, next) : null;
  hiredPlacement.value = placeHired(slotId, next.hired);
  lastRead.set(slotId, next);
  view.value = next;
  viewFailure.value = null;
  marks.value = recarving ?? NO_MARKS;
  if (recarving) {
    readAgainAt.value = Date.now();
    announcement.value = recarvedSentence(recarving.count);
    light();
  }
  if (!places.value[activePlace.value]) activePlace.value = members.value[0] ?? "arisen";
}

function failed(e: unknown): void {
  const failure = asRiftFailure(e);
  view.value = null;
  clearMarks();
  viewFailure.value = failure;
  // THIS SAVE IS GONE: the stone is reading the list again
  if (failure.kind === "save" && failure.detail.reason === "gone") void loadSlots();
}

async function read(slotId: string, cause: ReadCause): Promise<void> {
  const mine = ++readSeq;
  const sameSlot = view.value?.slotId === slotId;
  if (cause !== "changed") {
    clearMarks();
    readAgainAt.value = null;
  }
  if (cause === "pick") viewFailure.value = null;
  reading.value = true;
  const done = slowAfter(readingSlow);
  try {
    const backend = await rift();
    const next = await backend.getMuster(slotId);
    if (mine === readSeq) landed(slotId, next, cause, sameSlot);
  } catch (e) {
    if (mine === readSeq) failed(e);
  } finally {
    if (mine === readSeq) {
      reading.value = false;
      done();
    }
  }
}

/** Pick a slot (Enter, Space or a click): reads it. */
async function pick(slotId: string): Promise<void> {
  pickedId.value = slotId;
  rememberedId = slotId;
  view.value = view.value?.slotId === slotId ? view.value : null;
  await read(slotId, "pick");
}

/** Entering the room: the list, then the slot last picked this session (if
 *  it is still listed), else the newest. A slot already on screen is not
 *  read again: its marks stay until the next read. */
async function enter(): Promise<void> {
  const list = await loadSlots();
  if (!list || list.problem) return;
  const listed = (id: string | null) => (id ? list.slots.some((s) => s.slotId === id) : false);
  const target = listed(rememberedId) ? rememberedId : (list.slots[0]?.slotId ?? null);
  if (!target) return;
  if (view.value?.slotId === target && pickedId.value === target) return;
  await pick(target);
}

/** `Look again` (R17.22 page states). */
async function lookAgain(): Promise<void> {
  await enter();
}

/** `Read again`: the user asked, so nothing is re-carved (R17.25 step 1). */
async function readAgain(): Promise<void> {
  if (pickedId.value) await read(pickedId.value, "retry");
}

/** rift://muster-changed: the list again, then the open slot again. */
async function onChanged(): Promise<void> {
  if (slotsView.value === null && slotsFailure.value === null) return; // the room was never opened
  const list = await loadSlots();
  const open = pickedId.value;
  if (!list || !open) return;
  if (list.problem) {
    view.value = null;
    viewFailure.value = null;
    clearMarks();
    return;
  }
  await read(open, "changed");
}

async function openSavesFolder(): Promise<RiftFailure | null> {
  const slotId = pickedId.value;
  if (!slotId) return null;
  try {
    const backend = await rift();
    await backend.openSavesFolder(slotId);
    return null;
  } catch (e) {
    return asRiftFailure(e);
  }
}

function select(place: Place): void {
  if (places.value[place]) activePlace.value = place;
}

/** `[` / `]`: the previous / next member that exists. */
function step(delta: 1 | -1): Place | null {
  const list = members.value;
  if (!list.length) return null;
  const at = list.indexOf(activePlace.value);
  const next = list[(at + delta + list.length) % list.length];
  activePlace.value = next;
  return next;
}

function summon(): void {
  focusSummoned.value = true;
}

function askPickSlot(): void {
  pickSlotSummons.value += 1;
}

export function useMuster() {
  return {
    slotsView,
    slotsFailure,
    slotsVoiced,
    listingSlow,
    pickedId,
    picked,
    view,
    viewFailure,
    viewVoiced,
    reading,
    readingSlow,
    activePlace,
    activeMember,
    places,
    members,
    marks,
    lit,
    readAgainAt,
    announcement,
    focusSummoned,
    pickSlotSummons,
    enter,
    pick,
    lookAgain,
    readAgain,
    onChanged,
    openSavesFolder,
    select,
    step,
    summon,
    askPickSlot,
  };
}

/** Tests: back to the first frame, the session's memory included. */
export function resetMuster(): void {
  slotsView.value = null;
  slotsFailure.value = null;
  listingSlow.value = false;
  pickedId.value = null;
  rememberedId = null;
  view.value = null;
  viewFailure.value = null;
  reading.value = false;
  readingSlow.value = false;
  activePlace.value = "arisen";
  marks.value = NO_MARKS;
  lit.value = false;
  readAgainAt.value = null;
  announcement.value = "";
  focusSummoned.value = false;
  pickSlotSummons.value = 0;
  hiredPlacement.value = {};
  lastRead.clear();
  hiredPlaces.clear();
  readSeq += 1;
  listSeq += 1;
}
