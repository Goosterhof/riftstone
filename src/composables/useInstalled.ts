// Your World — Installed: what is in the game now, the pak load order (last
// wins), who overrides whom, REFramework's proof that the paks loaded, strays,
// and Fluffy's installs waiting to be adopted. Nothing touches disk until the
// user presses Apply order / Adopt.

import { computed, ref } from "vue";
import type { AdoptResultView, InstalledRow, InstalledView, UninstallPlanView } from "@/types/rift";
import { asRiftFailure, rift } from "@/services/rift";
import { voice, voicedLine, type Voiced } from "@/services/voice";
import { useToasts } from "./useToasts";
import { restoredSentence } from "./useWorld";

export type AdoptStage = "closed" | "plan" | "adopting" | "done";

const view = ref<InstalledView | null>(null);
const loadFailure = ref<Voiced | null>(null);
const pendingOrder = ref<string[] | null>(null);
const applying = ref(false);
const announcement = ref("");
const adoptStage = ref<AdoptStage>("closed");
const adoptResult = ref<AdoptResultView | null>(null);
const returning = ref<UninstallPlanView | null>(null);
const returnBusy = ref(false);

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

async function confirmAdopt(): Promise<void> {
  adoptStage.value = "adopting";
  try {
    const backend = await rift();
    adoptResult.value = await backend.adoptFluffy();
    adoptStage.value = "done";
    await load();
  } catch (raw) {
    adoptStage.value = "plan";
    useToasts().push("ember", voicedLine(voice(asRiftFailure(raw), "adopt")));
  }
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
    await backend.uninstall(plan.optionId);
    returning.value = null;
    useToasts().push("rift", `${plan.name} has returned to the Rift.${restoredSentence(plan.removals)}`);
    await load();
  } catch (raw) {
    useToasts().push("ember", voicedLine(voice(asRiftFailure(raw), "uninstall")));
  } finally {
    returnBusy.value = false;
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
    adoptStage,
    adoptResult,
    returning,
    returnBusy,
    load,
    moveTo,
    moveBy,
    applyOrder,
    resetOrder,
    openAdoption: () => (adoptStage.value = "plan"),
    closeAdoption: () => (adoptStage.value = "closed"),
    confirmAdopt,
    planReturn,
    cancelReturn: () => (returning.value = null),
    confirmReturn,
  };
}

export function resetInstalled(): void {
  view.value = null;
  loadFailure.value = null;
  pendingOrder.value = null;
  applying.value = false;
  announcement.value = "";
  adoptStage.value = "closed";
  adoptResult.value = null;
  returning.value = null;
  returnBusy.value = false;
}
