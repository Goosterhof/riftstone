// Your World — Installed: what is in the game now, the pak load order (last
// wins), who overrides whom, REFramework's proof that the paks loaded, and
// strays. Nothing touches disk until the user presses Apply order. Every row,
// a load-order row too, has an uninstall door: an install whose package left
// the mods folder (or an adopted one that never had one) still uninstalls,
// because the journal knows every file it placed.
// v0.2: PARKED — files the stone kept aside rather than delete or force back,
// each with Put back (restore_parked, saying what was swapped out) and Discard
// (discard_parked, behind a confirm: it is the one act here that loses bytes).

import { computed, ref } from "vue";
import type { InstalledRow, InstalledView, UninstallPlanView } from "@/types/rift";
import { asRiftFailure, rift } from "@/services/rift";
import { voice, voicedLine, type Voiced } from "@/services/voice";
import { useToasts } from "./useToasts";
import { parkedRestoreSummary, returnedSummary } from "@/services/ledger";
import { restoredSentence } from "./useWorld";

const view = ref<InstalledView | null>(null);
const loadFailure = ref<Voiced | null>(null);
const pendingOrder = ref<string[] | null>(null);
const applying = ref(false);
const announcement = ref("");
const returning = ref<UninstallPlanView | null>(null);
const returnBusy = ref(false);
/** The store handle whose Put back / Discard is running. */
const parkedBusy = ref<string | null>(null);
/** The store handle waiting on "Discard for good?". */
const discarding = ref<string | null>(null);
/** The last Put back's sentence, shown under the list until the next act. */
const parkedNote = ref<string | null>(null);
/** Bumped when Preflight's reviewParked asks for the list; the page scrolls to it. */
const parkedSummons = ref(0);

const order = computed(() => pendingOrder.value ?? view.value?.pakOrder ?? []);
const orderDirty = computed(() => pendingOrder.value !== null);

const rowsById = computed(() => new Map((view.value?.installs ?? []).map((r) => [r.optionId, r])));

/** Pak-bearing installs in load order (lowest priority first). */
const pakRows = computed(() =>
  order.value.map((id) => rowsById.value.get(id)).filter((r): r is InstalledRow => r !== undefined),
);

/** Everything else: loose, script and root installs with no pak. */
const otherRows = computed(() => (view.value?.installs ?? []).filter((r) => r.kinds.pak === 0));

async function load(): Promise<void> {
  try {
    const backend = await rift();
    view.value = await backend.getInstalled();
    loadFailure.value = null;
  } catch (raw) {
    loadFailure.value = voice(asRiftFailure(raw), "reorder");
  }
}

function nameOf(id: string): string {
  return rowsById.value.get(id)?.name ?? "That mod";
}

function moveTo(id: string, target: number): void {
  const list = [...order.value];
  const from = list.indexOf(id);
  if (from < 0) return;
  const to = Math.max(0, Math.min(list.length - 1, target));
  if (to === from) return;
  list.splice(from, 1);
  list.splice(to, 0, id);
  const same = list.every((x, i) => x === view.value?.pakOrder[i]);
  pendingOrder.value = same ? null : list;
  announcement.value = `${nameOf(id)} moved to position ${to + 1} of ${list.length}`;
}

function moveBy(id: string, delta: number): void {
  moveTo(id, order.value.indexOf(id) + delta);
}

async function applyOrder(): Promise<void> {
  if (!pendingOrder.value) return;
  applying.value = true;
  try {
    const backend = await rift();
    await backend.reorderPaks(pendingOrder.value);
    pendingOrder.value = null;
    await load();
    useToasts().push("world", "The load order is set. Later still wins; the prefixes were rewritten.");
  } catch (raw) {
    useToasts().push("ember", voicedLine(voice(asRiftFailure(raw), "reorder")));
  } finally {
    applying.value = false;
  }
}

function resetOrder(): void {
  pendingOrder.value = null;
  announcement.value = "Load order put back as it is on disk";
}

async function planReturn(optionId: string): Promise<void> {
  const backend = await rift();
  try {
    returning.value = await backend.planUninstall(optionId);
  } catch (raw) {
    useToasts().push("ember", voicedLine(voice(asRiftFailure(raw), "uninstall")));
  }
}

async function confirmReturn(): Promise<void> {
  const plan = returning.value;
  if (!plan || plan.refused) return;
  returnBusy.value = true;
  try {
    const backend = await rift();
    const result = await backend.uninstall(plan.optionId);
    returning.value = null;
    useToasts().push(
      "rift",
      `${plan.name} has returned to the Rift.${restoredSentence(plan.removals)} ${returnedSummary(result)}`,
    );
    await load();
  } catch (raw) {
    useToasts().push("ember", voicedLine(voice(asRiftFailure(raw), "uninstall")));
  } finally {
    returnBusy.value = false;
  }
}

/** A failed Put back / Discard: voiced in its own register (never the
 *  uninstall's), the list read again so a stale row cannot linger, and the
 *  confirm closed. */
async function parkedFailed(raw: unknown, store: string, act: "putBack" | "discard"): Promise<void> {
  const failure = asRiftFailure(raw);
  const gamePath = view.value?.parked.find((f) => f.store === store)?.gamePath ?? "";
  const voiced = voice({ kind: failure.kind, detail: { ...failure.detail, gamePath, act } }, "parked");
  discarding.value = null;
  parkedNote.value = voicedLine(voiced);
  useToasts().push("ember", parkedNote.value);
  await load();
}

async function putBack(store: string): Promise<void> {
  parkedBusy.value = store;
  discarding.value = null;
  try {
    const backend = await rift();
    const report = await backend.restoreParked(store);
    parkedNote.value = parkedRestoreSummary(report);
    useToasts().push(report.restored ? "world" : "ember", parkedNote.value);
    await load();
  } catch (raw) {
    await parkedFailed(raw, store, "putBack");
  } finally {
    parkedBusy.value = null;
  }
}

async function discard(store: string): Promise<void> {
  if (discarding.value !== store) {
    discarding.value = store;
    return;
  }
  const file = view.value?.parked.find((f) => f.store === store);
  parkedBusy.value = store;
  try {
    const backend = await rift();
    await backend.discardParked(store);
    discarding.value = null;
    parkedNote.value = `The kept copy of ${file?.gamePath ?? "that file"} is gone for good. Your game folder was not touched.`;
    useToasts().push("rift", parkedNote.value);
    await load();
  } catch (raw) {
    await parkedFailed(raw, store, "discard");
  } finally {
    parkedBusy.value = null;
  }
}

export function useInstalled() {
  return {
    view,
    loadFailure,
    order,
    orderDirty,
    pakRows,
    otherRows,
    applying,
    announcement,
    returning,
    returnBusy,
    load,
    moveTo,
    moveBy,
    applyOrder,
    resetOrder,
    planReturn,
    cancelReturn: () => (returning.value = null),
    confirmReturn,
    parkedBusy,
    discarding,
    parkedNote,
    parkedSummons,
    putBack,
    discard,
    keepParked: () => (discarding.value = null),
    summonParked: () => (parkedSummons.value += 1),
    /** The page took the summons: the next ordinary visit is not pulled down. */
    consumeParkedSummons: () => (parkedSummons.value = 0),
  };
}

export function resetInstalled(): void {
  view.value = null;
  loadFailure.value = null;
  pendingOrder.value = null;
  applying.value = false;
  announcement.value = "";
  returning.value = null;
  returnBusy.value = false;
  parkedBusy.value = null;
  discarding.value = null;
  parkedNote.value = null;
  parkedSummons.value = 0;
}
