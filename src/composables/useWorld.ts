// Your World pane: the selected option, its install plan, and the crossing.
// stage walks:  detail → plan → crossing → landed | failed
//               detail → returnPlan → returning → returned | failed
// THE CROSSING (§11.2) keys off `crossingSeq`; the card's rail turns gold on
// its own because the option's state changed, not because anything is drawn.

import { computed, ref } from "vue";
import type {
  InstallPlanView,
  InstallProgressEvent,
  InstallResultView,
  OptionDetail,
  UninstallPlanView,
  UninstallResultView,
} from "@/types/rift";
import { asRiftFailure, rift, type RiftFailure } from "@/services/rift";
import { failureDetails, voice, type VoiceContext, type Voiced } from "@/services/voice";
import { plural } from "@/services/format";
import { useNavigation } from "./useNavigation";
import { useToasts } from "./useToasts";

export type WorldStage = "detail" | "plan" | "crossing" | "landed" | "returnPlan" | "returning" | "returned" | "failed";

export interface WorldFailure {
  voiced: Voiced;
  details: string;
  context: VoiceContext;
}

const selectedId = ref<string | null>(null);
const detail = ref<OptionDetail | null>(null);
const detailFailure = ref<Voiced | null>(null);
const stage = ref<WorldStage>("detail");
const plan = ref<InstallPlanView | null>(null);
const returnPlan = ref<UninstallPlanView | null>(null);
const progress = ref<InstallProgressEvent | null>(null);
const landed = ref<InstallResultView | null>(null);
const returned = ref<UninstallResultView | null>(null);
const failure = ref<WorldFailure | null>(null);
const planning = ref(false);
const crossingSeq = ref(0);

const isInstalled = computed(() => Boolean(detail.value?.installed));

async function loadDetail(optionId: string): Promise<void> {
  try {
    const backend = await rift();
    const next = await backend.getOption(optionId);
    if (selectedId.value === optionId) detail.value = next;
    detailFailure.value = null;
  } catch (raw) {
    detailFailure.value = voice(asRiftFailure(raw), "package");
  }
}

async function open(optionId: string): Promise<void> {
  if (selectedId.value !== optionId) {
    selectedId.value = optionId;
    detail.value = null;
    stage.value = "detail";
    plan.value = null;
    returnPlan.value = null;
    failure.value = null;
  }
  await loadDetail(optionId);
}

function close(): void {
  selectedId.value = null;
  detail.value = null;
  stage.value = "detail";
  plan.value = null;
  returnPlan.value = null;
  failure.value = null;
}

function fail(raw: unknown, context: VoiceContext): void {
  const f: RiftFailure = asRiftFailure(raw);
  failure.value = { voiced: voice(f, context), details: failureDetails(f, context), context };
  stage.value = "failed";
}

async function beginInstall(optionId: string | null = selectedId.value): Promise<void> {
  if (!optionId) return;
  if (optionId !== selectedId.value) await open(optionId);
  planning.value = true;
  try {
    const backend = await rift();
    plan.value = await backend.planInstall(optionId);
    stage.value = "plan";
  } catch (raw) {
    fail(raw, "install");
  } finally {
    planning.value = false;
  }
}

function landedToast(result: InstallResultView, name: string): void {
  const { push } = useToasts();
  const restart = result.restartNeeded ? " Restart the game to load it." : "";
  push("world", `${name} is in your world. ${plural(result.filesWritten, "file")} placed.${restart}`, [
    { label: "Undo", run: () => quietUninstall(result.optionId, name) },
    { label: "Show in load order", run: () => useNavigation().go("world") },
  ]);
}

async function confirmInstall(): Promise<void> {
  const current = plan.value;
  if (!current || current.refused) return;
  stage.value = "crossing";
  progress.value = {
    optionId: current.optionId,
    filesDone: 0,
    filesTotal: current.writes.length,
    bytesDone: 0,
    bytesTotal: current.totalBytes,
  };
  try {
    const backend = await rift();
    const result = await backend.install(current.optionId);
    landed.value = result;
    stage.value = "landed";
    crossingSeq.value += 1;
    landedToast(result, current.name);
    await loadDetail(current.optionId);
  } catch (raw) {
    fail(raw, "install");
  } finally {
    progress.value = null;
  }
}

async function beginUninstall(optionId: string | null = selectedId.value): Promise<void> {
  if (!optionId) return;
  if (optionId !== selectedId.value) await open(optionId);
  planning.value = true;
  try {
    const backend = await rift();
    returnPlan.value = await backend.planUninstall(optionId);
    stage.value = "returnPlan";
  } catch (raw) {
    fail(raw, "uninstall");
  } finally {
    planning.value = false;
  }
}

/** "3 of Dragonforged Tops Overhaul's files are back in place." per owner. */
export function restoredSentence(removals: UninstallPlanView["removals"]): string {
  const byOwner = new Map<string, number>();
  for (const r of removals) {
    if (r.action.kind === "restoreBackup") byOwner.set(r.action.owner, (byOwner.get(r.action.owner) ?? 0) + 1);
  }
  return [...byOwner]
    .map(([owner, n]) => ` ${n} of ${owner}'s ${n === 1 ? "file is" : "files are"} back in place.`)
    .join("");
}

function returnedLine(plan: UninstallPlanView): string {
  return `${plan.name} has returned to the Rift.${restoredSentence(plan.removals)}`;
}

async function confirmUninstall(): Promise<void> {
  const current = returnPlan.value;
  if (!current || current.refused) return;
  stage.value = "returning";
  try {
    const backend = await rift();
    const result = await backend.uninstall(current.optionId);
    returned.value = result;
    stage.value = "returned";
    useToasts().push("rift", returnedLine(current), [
      { label: "Undo", run: () => quietInstall(current.optionId, current.name) },
    ]);
    await loadDetail(current.optionId);
  } catch (raw) {
    fail(raw, "uninstall");
  }
}

/** Undo paths: straight through, no plan (the user just saw it). */
async function quietUninstall(optionId: string, name: string): Promise<void> {
  try {
    const backend = await rift();
    await backend.uninstall(optionId);
    useToasts().push("rift", `${name} has returned to the Rift.`);
    if (selectedId.value === optionId) {
      stage.value = "detail";
      await loadDetail(optionId);
    }
  } catch (raw) {
    useToasts().push("ember", voice(asRiftFailure(raw), "uninstall").voice);
  }
}

async function quietInstall(optionId: string, name: string): Promise<void> {
  try {
    const backend = await rift();
    const result = await backend.install(optionId);
    crossingSeq.value += 1;
    useToasts().push("world", `${name} is in your world. ${plural(result.filesWritten, "file")} placed.`);
    if (selectedId.value === optionId) {
      stage.value = "detail";
      await loadDetail(optionId);
    }
  } catch (raw) {
    useToasts().push("ember", voice(asRiftFailure(raw), "install").voice);
  }
}

function backToDetail(): void {
  stage.value = "detail";
  plan.value = null;
  returnPlan.value = null;
  failure.value = null;
}

function onInstallProgress(event: InstallProgressEvent): void {
  if (stage.value === "crossing" && event.optionId === plan.value?.optionId) progress.value = event;
}

async function refresh(): Promise<void> {
  const id = selectedId.value;
  if (!id) return;
  if (stage.value === "detail" || stage.value === "landed" || stage.value === "returned") await loadDetail(id);
  else if (stage.value === "plan" && plan.value?.optionId === id) await replan(id);
}

/** The game changed under an open plan (a recovery button fixed what it
 *  warned about): read the plan again, quietly, so no warning outlives its cause. */
async function replan(optionId: string): Promise<void> {
  try {
    const backend = await rift();
    const next = await backend.planInstall(optionId);
    if (stage.value === "plan" && selectedId.value === optionId) plan.value = next;
  } catch {
    // The plan on screen stays; pressing Install re-checks at the spine anyway.
  }
}

async function retry(): Promise<void> {
  const context = failure.value?.context;
  if (context === "uninstall") await beginUninstall();
  else await beginInstall();
}

export function useWorld() {
  return {
    selectedId,
    detail,
    detailFailure,
    stage,
    plan,
    returnPlan,
    progress,
    landed,
    returned,
    failure,
    planning,
    crossingSeq,
    isInstalled,
    open,
    close,
    beginInstall,
    confirmInstall,
    beginUninstall,
    confirmUninstall,
    backToDetail,
    onInstallProgress,
    refresh,
    retry,
  };
}

export function resetWorld(): void {
  close();
  progress.value = null;
  landed.value = null;
  returned.value = null;
  planning.value = false;
  crossingSeq.value = 0;
  detailFailure.value = null;
}
