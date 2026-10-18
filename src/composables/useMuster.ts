// THE MUSTER's state (#00015 §17): the slot list, the picked slot and the
// session's memory of it, the party read from it, which member's sheet is
// open, and THE RE-CARVING — the marks a rift://muster-changed re-read
// leaves on exactly the values that changed (R17.25).
//
// Picking reads a file, so only a pick, a door or the save watcher ever
// calls get_muster: moving the focus through the slot list never does.
// Privacy: nothing here logs, toasts or announces a name the save gives.
//
// v0.5 (#00015 §18.8–§18.10): the list is of characters, held by their key
// (in memory only: never in the DOM, a log or a sentence), with the slots
// that won't open apart. The sheet FOLLOWS the shown character's newest save
// into any slot, or stays PINNED on a slot picked from the head's menu. A
// slot the game is writing keeps the character it held at its last good
// listing (R18.19), and while following it never moves or blanks the sheet
// (Bold Choice 22). Picking a character writes `musterCharacter`; nothing
// else ever does (Q18.12).

import { computed, ref, shallowRef } from "vue";
import type { MusterMember, MusterSlot, MusterSlotsView, MusterView, SaveReason } from "@/types/rift";
import { asRiftFailure, rift, type RiftFailure } from "@/services/rift";
import { voice, type Voiced } from "@/services/voice";
import { NO_MARKS, recarve, type Recarving } from "@/services/recarving";
import {
  APART_ORDER,
  PINNED_NEWER_SENTENCE,
  movedSentence,
  recarvedSentence,
  type MusterMode,
} from "@/services/muster";
import { useSettings } from "./useSettings";

/** The four places of the tab line (R17.8): position is identity. */
export const PLACES = ["arisen", "mainPawn", "hired1", "hired2"] as const;
export type Place = (typeof PLACES)[number];

/** How long a read may run before its quiet state shows (R17.22: no flash). */
export const SLOW_MS = 200;

/** `followed`: the save watcher moved the sheet into the shown character's newer slot (R18.25). */
type ReadCause = "pick" | "retry" | "changed" | "followed";

/** R18.15: one character, the Arisen with their main pawn. */
export interface MusterCharacter {
  /** The private key (`MusterSlot.character`): held in memory, never rendered or logged. */
  key: string;
  /** The character's slots, newest first, a slot the game is writing included (R18.19). */
  slots: MusterSlot[];
  /** The newest slot that opened whole: the row's names, level and vocation come from it. */
  face: MusterSlot | null;
  /** The game is writing the character's newest slot (R18.15's warn rail). */
  writing: boolean;
}

/** R18.16: the slots that will not open, under their problem. */
export interface ApartGroup {
  reason: SaveReason;
  slots: MusterSlot[];
}

const slotsView = shallowRef<MusterSlotsView | null>(null);
const slotsFailure = shallowRef<RiftFailure | null>(null);
const listingSlow = ref(false);
/** The slot whose party the sheet column is about. */
const pickedId = ref<string | null>(null);
/** v0.5: the shown character's key, or null while a slot apart is shown (R18.17: held by key). */
const shownKey = ref<string | null>(null);
const mode = ref<"following" | "pinned">("following");
/** Following, and the newer save still being written: its slot number (R18.23). */
const savingIn = ref<number | null>(null);
/** The last read followed the character from this slot (R18.25 step 3). */
const movedFrom = shallowRef<{ slot: number; at: number } | null>(null);
/** The slot number's light for the one frame after a move (R18.25 step 3). */
const slotLit = ref(false);
/** Pinned: the character's slots as they were saved when the pin was made (R18.24). */
let pinBaseline: Map<string, string> | null = null;
/** Pinned: the slots of the character saved since the pin. */
const newerIds = shallowRef<ReadonlySet<string>>(new Set());
/** R18.27: the remembered character is gone; the line under `CHARACTERS` shows. */
const goneShown = ref(false);
/** R18.27: the line shows once per session. */
let goneTold = false;
/** R18.19: the character each slot held at its last good listing, for this session only. */
const knownCharacter = new Map<string, string>();
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
/** The slot whose party was drawn last. A failed re-read blanks the sheet but
 *  keeps this, so the next re-read of that party is still re-carved against
 *  its last read; a pick of another slot or a folder problem clears it. */
let drawnId: string | null = null;
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

/** R18.19: the character a slot belongs to now, a slot the game is writing keeping its own. */
function characterOf(slot: MusterSlot): string | null {
  if (slot.character !== null) return slot.character;
  return slot.problem === "beingWritten" ? (knownCharacter.get(slot.slotId) ?? null) : null;
}

/** R18.15 + R18.17: grouped by key, in the spine's order (newest save first). */
const characters = computed<MusterCharacter[]>(() => {
  const groups = new Map<string, MusterSlot[]>();
  for (const slot of slotsView.value?.slots ?? []) {
    const key = characterOf(slot);
    if (key === null) continue;
    const group = groups.get(key);
    if (group) group.push(slot);
    else groups.set(key, [slot]);
  }
  return [...groups].map(([key, slots]) => ({
    key,
    slots,
    face: slots.find((s) => s.arisen !== null) ?? null,
    writing: slots[0]?.problem === "beingWritten",
  }));
});

/** R18.16: every slot without a character, one group per reason, in the fixed order. */
const apart = computed<ApartGroup[]>(() => {
  const loose = (slotsView.value?.slots ?? []).filter((s) => characterOf(s) === null);
  return APART_ORDER.map((reason) => ({ reason, slots: loose.filter((s) => s.problem === reason) })).filter(
    (g) => g.slots.length > 0,
  );
});

const shownCharacter = computed(() => characters.value.find((c) => c.key === shownKey.value) ?? null);

/** R18.23–R18.25: what the head's state words say. */
const stateMode = computed<MusterMode>(() => {
  if (mode.value === "pinned") {
    const newer = shownCharacter.value?.slots.find((s) => newerIds.value.has(s.slotId));
    return { kind: "pinned", newerIn: newer?.slot ?? null };
  }
  return { kind: "following", savingIn: savingIn.value, movedFrom: movedFrom.value };
});

/** Where a character opens: its newest slot that opens whole, else its newest. */
function openTarget(who: MusterCharacter): MusterSlot | null {
  return who.slots.find((s) => s.problem === null) ?? who.slots.at(0) ?? null;
}

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
    for (const slot of next.slots) if (slot.character !== null) knownCharacter.set(slot.slotId, slot.character);
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

/** R18.25 step 3: the slot number takes the light after a move, colour only. */
function lightSlot(): void {
  if (reduced()) return;
  slotLit.value = true;
  requestAnimationFrame(() => requestAnimationFrame(() => (slotLit.value = false)));
}

/** A read landed. Only a save-watcher re-read of the party already on screen
 *  is re-carved (R17.25 step 1): never a first read, a pick, or Read again.
 *  v0.5: a read that FOLLOWED the character into another slot is re-carved
 *  against the party it showed, members matched by memberKey (R18.25). */
function landed(slotId: string, next: MusterView, cause: ReadCause, from: string | null): void {
  const ground = groundOf(slotId, cause, from);
  const recarving = ground ? recarve(ground.view, next) : null;
  // a hired pawn keeps the place it held in the party the sheet showed (R17.8)
  if (ground?.moved) hiredPlaces.set(slotId, new Map(hiredPlaces.get(ground.view.slotId)));
  draw(slotId, next, recarving);
  if (cause === "changed" || cause === "followed") movedFrom.value = null;
  if (!ground || !recarving) return;
  told({ slot: ground.moved ? ground.view.slot : null, count: recarving.count });
}

/** What a read is re-carved against, or null: the slot's last read for a
 *  save-watcher re-read of the slot on screen (R17.25 step 1), or the party
 *  the sheet showed for a move into the character's newer slot (R18.25). */
function groundOf(slotId: string, cause: ReadCause, from: string | null): { view: MusterView; moved: boolean } | null {
  const shown = from === null ? undefined : lastRead.get(from);
  if (!shown) return null;
  if (cause === "followed" && from !== slotId) return { view: shown, moved: true };
  return cause === "changed" && from === slotId ? { view: shown, moved: false } : null;
}

/** The read on the sheet, its marks with it. */
function draw(slotId: string, next: MusterView, recarving: Recarving | null): void {
  hiredPlacement.value = placeHired(slotId, next.hired);
  lastRead.set(slotId, next);
  drawnId = slotId;
  pickedId.value = slotId;
  view.value = next;
  viewFailure.value = null;
  savingIn.value = null;
  marks.value = recarving ?? NO_MARKS;
  if (!places.value[activePlace.value]) activePlace.value = members.value[0] ?? "arisen";
}

/** The head's words and the live region after a re-carving: a move names
 *  the slot it came from (R18.25 step 3), a re-read the time (R17.25). */
function told(what: { slot: number | null; count: number }): void {
  if (what.slot === null) {
    readAgainAt.value = Date.now();
    announcement.value = recarvedSentence(what.count);
  } else {
    movedFrom.value = { slot: what.slot, at: Date.now() };
    readAgainAt.value = null;
    announcement.value = movedSentence(what.count);
    lightSlot();
  }
  light();
}

const isWriting = (failure: RiftFailure): boolean =>
  failure.kind === "save" && failure.detail.reason === "beingWritten";

/** Bold Choice 22: while following, a save the game is still writing never
 *  blanks the sheet. The party stays; the head says where the game is saving. */
function heldThrough(failure: RiftFailure, cause: ReadCause, slotId: string): boolean {
  const watcher = cause === "changed" || cause === "followed";
  const following = mode.value === "following" && shownKey.value !== null && view.value !== null;
  if (!watcher || !following || !isWriting(failure)) return false;
  savingIn.value = slotsView.value?.slots.find((s) => s.slotId === slotId)?.slot ?? null;
  return true;
}

function failed(e: unknown, cause: ReadCause, slotId: string): void {
  const failure = asRiftFailure(e);
  if (heldThrough(failure, cause, slotId)) return;
  pickedId.value = slotId;
  view.value = null;
  clearMarks();
  viewFailure.value = failure;
  // THIS SAVE IS GONE: the stone is reading the list again
  if (failure.kind === "save" && failure.detail.reason === "gone") void loadSlots();
}

async function read(slotId: string, cause: ReadCause): Promise<void> {
  const mine = ++readSeq;
  const from = drawnId;
  if (cause !== "changed" && cause !== "followed") {
    clearMarks();
    readAgainAt.value = null;
    movedFrom.value = null;
  }
  if (cause === "pick") viewFailure.value = null;
  reading.value = true;
  const done = slowAfter(readingSlow);
  try {
    const backend = await rift();
    const next = await backend.getMuster(slotId);
    if (mine === readSeq) landed(slotId, next, cause, from);
  } catch (e) {
    if (mine === readSeq) failed(e, cause, slotId);
  } finally {
    if (mine === readSeq) {
      reading.value = false;
      done();
    }
  }
}

function clearPin(): void {
  pinBaseline = null;
  newerIds.value = new Set();
}

/** Show a slot as a pick: the sheet clears unless it already shows that slot. */
async function show(slotId: string): Promise<void> {
  pickedId.value = slotId;
  if (drawnId !== slotId) drawnId = null;
  view.value = view.value?.slotId === slotId ? view.value : null;
  savingIn.value = null;
  await read(slotId, "pick");
}

/** The character opened at its newest save, following (R18.15, R18.26). */
function follow(who: MusterCharacter): MusterSlot | null {
  shownKey.value = who.key;
  mode.value = "following";
  clearPin();
  return openTarget(who);
}

const showing = (who: MusterCharacter, target: MusterSlot): boolean =>
  shownKey.value === who.key &&
  mode.value === "following" &&
  pickedId.value === target.slotId &&
  view.value?.slotId === target.slotId;

/** R18.15 + R18.18: pick a character (Enter, Space or a click). Reads its
 *  newest save, then writes `musterCharacter`, once per pick: the read is
 *  never held behind the settings write. The character already shown on its
 *  newest save reads nothing and writes nothing. */
async function pickCharacter(key: string): Promise<void> {
  const who = characters.value.find((c) => c.key === key);
  if (!who) return;
  goneShown.value = false;
  const target = openTarget(who);
  if (!target || showing(who, target)) return;
  follow(who);
  const reading = show(target.slotId);
  const settings = useSettings();
  if (settings.settings.value.musterCharacter !== key) void settings.setMusterCharacter(key);
  await reading;
}

/** R18.21–R18.23: pick an entry of the slot menu. The newest follows; any
 *  other pins, and remembers the character's saves as they stood. */
async function pickSlot(slotId: string): Promise<void> {
  const who = shownCharacter.value;
  if (!who) return;
  newerIds.value = new Set();
  if (who.slots.at(0)?.slotId === slotId) {
    mode.value = "following";
    pinBaseline = null;
  } else {
    mode.value = "pinned";
    pinBaseline = new Map(who.slots.map((s) => [s.slotId, s.savedAt]));
  }
  movedFrom.value = null;
  if (pickedId.value === slotId && view.value?.slotId === slotId) {
    savingIn.value = null;
    return;
  }
  await show(slotId);
}

/** R18.16: pick a slot that won't open; its state shows. Never a character. */
async function pickApart(slotId: string): Promise<void> {
  shownKey.value = null;
  mode.value = "following";
  clearPin();
  movedFrom.value = null;
  await show(slotId);
}

/** v0.4's door, kept: a slot by its id, as a character's save or a slot apart. */
async function pick(slotId: string): Promise<void> {
  const owner = characters.value.find((c) => c.slots.some((s) => s.slotId === slotId));
  if (!owner) return pickApart(slotId);
  if (owner.key !== shownKey.value) follow(owner);
  await pickSlot(slotId);
}

/** Entering the room (R18.26): the list, then the remembered character at
 *  their newest save, following, else the character of the newest slot. A
 *  pin does not survive leaving the room. A sheet already on that save is
 *  not read again: its marks stay until the next read. The window never
 *  writes `musterCharacter` here (Q18.12). */
async function enter(): Promise<void> {
  const list = await loadSlots();
  if (!list || list.problem) return;
  const who = rememberedOrNewest(list);
  if (!who) {
    const first = list.slots.at(0);
    if (first && !(pickedId.value === first.slotId && shownKey.value === null)) await pickApart(first.slotId);
    return;
  }
  const target = openTarget(who);
  if (!target || showing(who, target)) return;
  follow(who);
  await show(target.slotId);
}

/** R18.26 + R18.27: the remembered character when listed, else the character
 *  of the newest slot; a remembered key no slot carries says so once per
 *  session, unless a slot is being written (R18.19). */
function rememberedOrNewest(list: MusterSlotsView): MusterCharacter | undefined {
  const remembered = useSettings().settings.value.musterCharacter;
  const found = characters.value.find((c) => c.key === remembered);
  const writing = list.slots.some((s) => s.problem === "beingWritten");
  if (remembered !== null && !found && !writing && !goneTold) {
    goneTold = true;
    goneShown.value = true;
  }
  return found ?? characters.value.at(0);
}

/** `Look again` (R17.22 page states). */
async function lookAgain(): Promise<void> {
  await enter();
}

/** `Read again`: the user asked, so nothing is re-carved (R17.25 step 1). */
async function readAgain(): Promise<void> {
  if (pickedId.value) await read(pickedId.value, "retry");
}

/** R18.24: pinned, and the save watcher fired. A save into the pinned slot
 *  itself re-reads it (and, being the newest now, follows again); a save in
 *  another slot marks it and moves nothing. */
async function changedWhilePinned(who: MusterCharacter, open: string): Promise<void> {
  const baseline = pinBaseline ?? new Map<string, string>();
  const since = (s: MusterSlot): boolean => baseline.get(s.slotId) !== s.savedAt;
  const own = who.slots.find((s) => s.slotId === open);
  if (own?.problem === null && since(own)) {
    if (who.slots.at(0)?.slotId === open) {
      mode.value = "following";
      clearPin();
    } else baseline.set(open, own.savedAt);
    await read(open, "changed");
    return;
  }
  const newer = who.slots.filter((s) => s.slotId !== open && since(s)).map((s) => s.slotId);
  if (!newer.length) return;
  if (newerIds.value.size === 0) announcement.value = PINNED_NEWER_SENTENCE;
  newerIds.value = new Set([...newerIds.value, ...newer]);
}

/** rift://muster-changed: the list again, then the open slot again. v0.5:
 *  following, the sheet moves to the character's newest save in any slot
 *  (R18.23, R18.25); pinned, it stays (R18.24). */
async function onChanged(): Promise<void> {
  if (slotsView.value === null && slotsFailure.value === null) return; // the room was never opened
  const list = await loadSlots();
  const open = pickedId.value;
  if (!list || !open) return;
  if (list.problem) {
    view.value = null;
    viewFailure.value = null;
    drawnId = null;
    clearMarks();
    return;
  }
  const who = shownCharacter.value;
  if (!who) await read(open, "changed");
  else if (mode.value === "pinned") await changedWhilePinned(who, open);
  else await changedWhileFollowing(who, open);
}

/** R18.23 + R18.25: following, the sheet moves to the character's newest
 *  save in any slot; a newest save still being written moves nothing. */
async function changedWhileFollowing(who: MusterCharacter, open: string): Promise<void> {
  const newest = who.slots.at(0);
  if (!newest) return;
  if (newest.problem === "beingWritten" && view.value) {
    savingIn.value = newest.slot;
    return;
  }
  savingIn.value = null;
  await read(newest.slotId, newest.slotId === open || !view.value ? "changed" : "followed");
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
    characters,
    apart,
    shownCharacter,
    stateMode,
    newerIds,
    slotLit,
    goneShown,
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
    pickCharacter,
    pickSlot,
    pickApart,
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
  shownKey.value = null;
  mode.value = "following";
  savingIn.value = null;
  movedFrom.value = null;
  slotLit.value = false;
  pinBaseline = null;
  newerIds.value = new Set();
  goneShown.value = false;
  goneTold = false;
  knownCharacter.clear();
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
  drawnId = null;
  hiredPlaces.clear();
  readSeq += 1;
  listSeq += 1;
}
