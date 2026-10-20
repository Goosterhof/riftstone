// Your World pane: the selected option, its install plan, and the crossing.
// stage walks:  detail → plan → crossing → landed | failed
//               detail → returnPlan → returning → returned | failed
// THE CROSSING (§11.2) keys off `crossingSeq`; the card's rail turns gold on
// its own because the option's state changed, not because anything is drawn.
// v0.5 (#00015 R18.5, R18.6, R18.9): an install that replaces another
// version's install lands with no Undo, its put-away told once in the plan,
// and a failed replace names the version the rollback put back.

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
import { failureDetails, pick, voice, type VoiceContext, type Voiced } from "@/services/voice";
import { plural } from "@/services/format";
import { familyNames, replacedRollback, replacedToast, versionName } from "@/services/versions";
import { useLibrary } from "./useLibrary";
import { useNavigation } from "./useNavigation";
import { useToasts } from "./useToasts";
import { resetUndercroft, useUndercroft } from "./useUndercroft";
import { resetVersions } from "./useVersions";

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
/** Writes outlive their card selection. Keep its door busy through the
 *  completion refresh, so returning to the card cannot start another one. */
const writesInFlight = ref(new Set<string>());
const writing = computed(() => selectedId.value !== null && writesInFlight.value.has(selectedId.value));
const crossingSeq = ref(0);
/** v0.5: the landed install's family, file name → version, read before the scan moves one away. */
const landedNames = ref<ReadonlyMap<string, string>>(new Map());
/** A response belongs to the selection that asked, even if the player
 *  leaves and comes back to the same option before it answers. */
let selectionSeq = 0;
let planningSeq = 0;
let detailSeq = 0;

const isInstalled = computed(() => Boolean(detail.value?.installed));

function canPlan(optionId: string, selection: number): boolean {
  return selection === selectionSeq && selectedId.value === optionId && !writesInFlight.value.has(optionId);
}

function currentPlanRequest(request: number, selection: number): boolean {
  return request === planningSeq && selection === selectionSeq && !writing.value;
}

function installCanStart(current: InstallPlanView | null): current is InstallPlanView {
  return Boolean(
    current && !current.refused && current.optionId === selectedId.value && stage.value === "plan" && !writing.value,
  );
}

function uninstallCanStart(current: UninstallPlanView | null): current is UninstallPlanView {
  return Boolean(
    current &&
    !current.refused &&
    current.optionId === selectedId.value &&
    stage.value === "returnPlan" &&
    !writing.value,
  );
}

async function loadDetail(optionId: string): Promise<void> {
  const selection = selectionSeq;
  const mine = ++detailSeq;
  try {
    const backend = await rift();
    const next = await backend.getOption(optionId);
    if (mine !== detailSeq || selection !== selectionSeq || selectedId.value !== optionId) return;
    detail.value = next;
    detailFailure.value = null;
  } catch (raw) {
    if (mine === detailSeq && selection === selectionSeq && selectedId.value === optionId)
      detailFailure.value = voice(asRiftFailure(raw), "package");
  }
}

/** A completed write still changes the option if the player left and came
 *  back. Refresh its data without reviving the earlier selection's plan. */
async function refreshRevisited(optionId: string): Promise<void> {
  if (selectedId.value === optionId) await loadDetail(optionId);
}

async function open(optionId: string): Promise<void> {
  if (selectedId.value !== optionId) {
    selectionSeq += 1;
    planningSeq += 1;
    planning.value = false;
    selectedId.value = optionId;
    detail.value = null;
    stage.value = "detail";
    plan.value = null;
    returnPlan.value = null;
    failure.value = null;
    detailFailure.value = null;
  }
  await loadDetail(optionId);
}

function close(): void {
  selectionSeq += 1;
  planningSeq += 1;
  detailSeq += 1;
  planning.value = false;
  selectedId.value = null;
  detail.value = null;
  stage.value = "detail";
  plan.value = null;
  returnPlan.value = null;
  failure.value = null;
}

function fail(raw: unknown, context: VoiceContext): void {
  const f: RiftFailure = asRiftFailure(raw);
  const voiced = voice(f, context);
  // R18.30 / Q18.13: a failed replace names the version the rollback put back, read from the plan
  const replaced = context === "install" ? plan.value?.replaces[0] : undefined;
  if (replaced && voiced.inscription === "THE CROSSING FAILED") {
    const placed = Number(pick(f.detail, "rolledBack", "restored", "placed", "filesRolledBack"));
    const old = versionName(replaced.version, replaced.name);
    voiced.empathy = `${replacedRollback(old, Number.isFinite(placed) ? placed : null)} Close the game if it's running.`;
  }
  failure.value = { voiced, details: failureDetails(f, context), context };
  stage.value = "failed";
}

async function beginInstall(optionId: string | null = selectedId.value): Promise<void> {
  if (!optionId) return;
  const opening = optionId !== selectedId.value ? open(optionId) : null;
  const selection = selectionSeq;
  await opening;
  if (!canPlan(optionId, selection)) return;
  const mine = ++planningSeq;
  planning.value = true;
  try {
    const backend = await rift();
    const next = await backend.planInstall(optionId);
    if (!currentPlanRequest(mine, selection)) return;
    plan.value = next;
    stage.value = "plan";
  } catch (raw) {
    if (currentPlanRequest(mine, selection)) fail(raw, "install");
  } finally {
    if (mine === planningSeq) planning.value = false;
  }
}

/** The version a package is, as the copy names it (`v1.3`), from the library the tree draws. */
function versionOfPackage(packageId: string | undefined): { pkgName: string; version: string } | null {
  const pkg = useLibrary().packages.value.find((p) => p.package.id === packageId);
  if (!pkg?.family) return null;
  return { pkgName: pkg.package.displayName, version: versionName(pkg.family.version, pkg.package.fileName) };
}

function landedToast(result: InstallResultView, name: string, landedPlan: InstallPlanView, packageId?: string): void {
  const { push } = useToasts();
  const replaced = landedPlan.replaces.at(0);
  const mine = versionOfPackage(packageId);
  if (replaced) {
    // R18.6, Bold Choice 18: a replace has no Undo; going back is installing the older version
    const old = versionName(replaced.version, replaced.name);
    const line = mine ? replacedToast(mine.pkgName, mine.version, old) : `${name} is in your world, in ${old}'s place.`;
    push("world", line, [{ label: "Show in load order", run: () => useNavigation().go("world") }]);
    return;
  }
  const restart = result.restartNeeded ? " Restart the game to load it." : "";
  push("world", `${name} is in your world. ${plural(result.filesWritten, "file")} placed.${restart}`, [
    { label: "Undo", run: () => quietUninstall(result.optionId, name) },
    { label: "Show in load order", run: () => useNavigation().go("world") },
  ]);
}

async function confirmInstall(): Promise<void> {
  const current = plan.value;
  if (!installCanStart(current)) return;
  const selection = selectionSeq;
  const packageId = detail.value?.package.id;
  writesInFlight.value.add(current.optionId);
  stage.value = "crossing";
  landedNames.value = familyNames(useLibrary().packages.value, detail.value?.package.id ?? "");
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
    landedToast(result, current.name, current, packageId);
    if (result.putAway.length) void useUndercroft().load();
    if (selection !== selectionSeq) {
      await refreshRevisited(current.optionId);
      return;
    }
    landed.value = result;
    stage.value = "landed";
    crossingSeq.value += 1;
    await loadDetail(current.optionId);
  } catch (raw) {
    if (selection === selectionSeq) fail(raw, "install");
    else useToasts().push("ember", voice(asRiftFailure(raw), "install").voice);
  } finally {
    writesInFlight.value.delete(current.optionId);
    if (selection === selectionSeq) progress.value = null;
  }
}

async function beginUninstall(optionId: string | null = selectedId.value): Promise<void> {
  if (!optionId) return;
  const opening = optionId !== selectedId.value ? open(optionId) : null;
  const selection = selectionSeq;
  await opening;
  if (!canPlan(optionId, selection)) return;
  const mine = ++planningSeq;
  planning.value = true;
  try {
    const backend = await rift();
    const next = await backend.planUninstall(optionId);
    if (!currentPlanRequest(mine, selection)) return;
    returnPlan.value = next;
    stage.value = "returnPlan";
  } catch (raw) {
    if (currentPlanRequest(mine, selection)) fail(raw, "uninstall");
  } finally {
    if (mine === planningSeq) planning.value = false;
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
  if (!uninstallCanStart(current)) return;
  const selection = selectionSeq;
  writesInFlight.value.add(current.optionId);
  stage.value = "returning";
  try {
    const backend = await rift();
    const result = await backend.uninstall(current.optionId);
    useToasts().push("rift", returnedLine(current), [
      { label: "Undo", run: () => quietInstall(current.optionId, current.name) },
    ]);
    if (selection !== selectionSeq) {
      await refreshRevisited(current.optionId);
      return;
    }
    returned.value = result;
    stage.value = "returned";
    await loadDetail(current.optionId);
  } catch (raw) {
    if (selection === selectionSeq) fail(raw, "uninstall");
    else useToasts().push("ember", voice(asRiftFailure(raw), "uninstall").voice);
  } finally {
    writesInFlight.value.delete(current.optionId);
  }
}

/** Undo paths: straight through, no plan (the user just saw it). */
async function quietUninstall(optionId: string, name: string): Promise<void> {
  if (writesInFlight.value.has(optionId)) return;
  writesInFlight.value.add(optionId);
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
  } finally {
    writesInFlight.value.delete(optionId);
  }
}

async function quietInstall(optionId: string, name: string): Promise<void> {
  if (writesInFlight.value.has(optionId)) return;
  writesInFlight.value.add(optionId);
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
  } finally {
    writesInFlight.value.delete(optionId);
  }
}

/** R18.5's `Try again` on a failed put-away: the spine makes the install's
 *  put-away again (WINDOW-CONTRACT `retry_put_away`). It works out what goes,
 *  moves what it can and names what would not move, so a move that worked is
 *  never lost to a later one that failed (General, PR #2). */
const retryingPutAway = ref(false);
async function retryPutAway(): Promise<void> {
  const result = landed.value;
  const packageId = detail.value?.package.id;
  if (!result || !packageId || retryingPutAway.value) return;
  retryingPutAway.value = true;
  try {
    const backend = await rift();
    const again = await backend.retryPutAway(packageId);
    if (again.putAway.length) void useUndercroft().load();
    // the landing the press came from, still on screen: a later one keeps its own lines
    if (landed.value !== result) return;
    landed.value = { ...result, putAway: [...result.putAway, ...again.putAway], putAwayFailed: again.putAwayFailed };
  } catch (raw) {
    if (landed.value !== result) return;
    const f = asRiftFailure(raw);
    const voiced = voice({ kind: f.kind, detail: { ...f.detail, act: "putAway" } }, "olderVersions");
    landed.value = { ...result, putAwayFailed: voiced.voice };
  } finally {
    retryingPutAway.value = false;
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
  const selection = selectionSeq;
  const request = planningSeq;
  try {
    const backend = await rift();
    const next = await backend.planInstall(optionId);
    if (currentPlanRequest(request, selection) && stage.value === "plan" && selectedId.value === optionId)
      plan.value = next;
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
    writing,
    crossingSeq,
    landedNames,
    retryingPutAway,
    retryPutAway,
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
  writesInFlight.value.clear();
  crossingSeq.value = 0;
  detailFailure.value = null;
  retryingPutAway.value = false;
  landedNames.value = new Map();
  resetVersions();
  resetUndercroft();
}
