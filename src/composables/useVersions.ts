// v0.5 — the versions of one file in Your World (#00015 R18.4, R18.5, R18.10):
// THE VERSIONS card, the keep-newest plan (THE RETURN — OLDER VERSIONS), the
// one-press put-away, and the put-away lines a landed plan carries with their
// `Bring it back`. The card stands in the World pane over whatever it held, so
// closing it returns the pane to that (the option's detail, or the summary).
// stage walks:  card → keepPlan → keeping → kept | failed

import { computed, nextTick, ref } from "vue";
import type { KeepNewestPlanView, KeepNewestResultView, PutAwayView } from "@/types/rift";
import { asRiftFailure, rift } from "@/services/rift";
import { failureDetails, voice, voicedLine, type Voiced } from "@/services/voice";
import {
  broughtBackToast,
  familyNames,
  keptToast,
  olderPlace,
  putAwayToast,
  versionName,
  type FamilyEntry,
} from "@/services/versions";
import { useLibrary } from "./useLibrary";
import { useNavigation } from "./useNavigation";
import { useToasts } from "./useToasts";
import { useUndercroft } from "./useUndercroft";

export type VersionsStage = "card" | "keepPlan" | "keeping" | "kept" | "failed";

/** A put-away line's own state (R18.5): there, moving back, back, or a failed move. */
export type PutAwayLineState = "idle" | "busy" | "back" | { voiced: Voiced; details: string };

const leadId = ref<string | null>(null);
const stage = ref<VersionsStage>("card");
const keepPlan = ref<KeepNewestPlanView | null>(null);
const kept = ref<KeepNewestResultView | null>(null);
const failure = ref<{ voiced: Voiced; details: string } | null>(null);
/** The package a one-press put-away moves now. */
const puttingAway = ref<string | null>(null);
/** The family's file names → versions, read before a keep-newest's scan moves one away. */
const keptNames = ref<ReadonlyMap<string, string>>(new Map());
/** Put-away lines by their landed name. */
const lines = ref<ReadonlyMap<string, PutAwayLineState>>(new Map());

/** The open card's entry, read from the library the tree draws. */
const entry = computed<FamilyEntry | null>(
  () => useLibrary().entries.value.find((e) => e.lead.package.id === leadId.value) ?? null,
);

/** Opens THE VERSIONS card for any member of a family. */
function open(packageId: string): void {
  const packages = useLibrary().packages.value;
  const place = olderPlace(packages, packageId);
  leadId.value = place ? place.lead.package.id : packageId;
  stage.value = "card";
  keepPlan.value = null;
  kept.value = null;
  failure.value = null;
}

function close(): void {
  leadId.value = null;
  stage.value = "card";
  keepPlan.value = null;
  kept.value = null;
  failure.value = null;
}

const familyName = (): string => entry.value?.lead.package.displayName ?? "this file";

/** A file name in the library's family as its version (`v1.2`), else the name itself. */
function nameOfFile(fileName: string): string {
  const e = entry.value;
  if (!e) return fileName;
  if (e.lead.package.fileName === fileName) return versionName(e.lead.family?.version ?? null, fileName);
  const older = e.older.find((o) => o.pkg.package.fileName === fileName);
  return older ? versionName(older.version.version, fileName) : fileName;
}

async function beginKeepNewest(packageId: string | null = leadId.value): Promise<void> {
  if (!packageId) return;
  try {
    const backend = await rift();
    keepPlan.value = await backend.planKeepNewest(packageId);
    stage.value = "keepPlan";
  } catch (raw) {
    const f = asRiftFailure(raw);
    failure.value = { voiced: voice(f, "uninstall"), details: failureDetails(f, "uninstall") };
    stage.value = "failed";
  }
}

async function confirmKeepNewest(): Promise<void> {
  const plan = keepPlan.value;
  if (!plan || plan.refused) return;
  const keeps = plan.keeps ? nameOfFile(plan.keeps) : "the newest version";
  const name = familyName();
  keptNames.value = familyNames(useLibrary().packages.value, plan.packageId);
  stage.value = "keeping";
  try {
    const backend = await rift();
    const result = await backend.keepNewest(plan.packageId);
    kept.value = result;
    stage.value = "kept";
    useToasts().push("world", keptToast(keeps, name));
    void useUndercroft().load();
  } catch (raw) {
    const f = asRiftFailure(raw);
    failure.value = { voiced: voice(f, "uninstall"), details: failureDetails(f, "uninstall") };
    stage.value = "failed";
  }
}

function backToCard(): void {
  stage.value = "card";
  keepPlan.value = null;
  kept.value = null;
  failure.value = null;
}

function setLine(landedAs: string, state: PutAwayLineState): void {
  const next = new Map(lines.value);
  next.set(landedAs, state);
  lines.value = next;
}

const lineState = (landedAs: string): PutAwayLineState => lines.value.get(landedAs) ?? "idle";

/** R18.5: `Bring it back` on a put-away line of a landed plan. */
async function bringBackLine(view: PutAwayView): Promise<void> {
  if (lineState(view.landedAs) === "busy") return;
  setLine(view.landedAs, "busy");
  const landed = await useUndercroft().bringBack(view.landedAs);
  if (landed) {
    setLine(view.landedAs, "back");
    return;
  }
  const failed = useUndercroft().rowFailures.value.get(view.landedAs);
  setLine(view.landedAs, failed ?? "idle");
}

/** Expands the lead and focuses the row the toast names (R18.13, R18.8). */
async function showInRift(lead: string, focus: string | null): Promise<void> {
  const nav = useNavigation();
  const library = useLibrary();
  nav.go("rift");
  nav.backToResults();
  library.expand(lead);
  library.focusedKey.value = focus ?? lead;
  await nextTick();
  await nextTick();
  const el = document.querySelector<HTMLElement>(`#rift-tree [data-key="${CSS.escape(focus ?? lead)}"]`);
  el?.focus({ preventScroll: true });
  el?.scrollIntoView({ block: "nearest" });
}

/** The Rift's door from THE FAR WORLD: the lead's first version row and THE VERSIONS card. */
async function showVersions(lead: string): Promise<void> {
  const first = useLibrary().entries.value.find((e) => e.lead.package.id === lead)?.older[0]?.pkg.package.id ?? null;
  await showInRift(lead, first);
  open(lead);
}

/** R18.13: after a bring-back the next scan folds it under its lead again. */
/** Where a package brought back sits now: under its lead, or as its own entry. */
function landedPlace(landedAs: string): { lead: string | null; target: string | null; focus: string | null } {
  const packages = useLibrary().packages.value;
  const pkg = packages.find((p) => p.package.fileName === landedAs);
  if (!pkg) return { lead: null, target: null, focus: null };
  const place = olderPlace(packages, pkg.package.id);
  if (!place) return { lead: null, target: pkg.package.id, focus: null };
  return {
    lead: versionName(place.lead.family?.version ?? null, place.lead.package.fileName),
    target: place.lead.package.id,
    focus: pkg.package.id,
  };
}

async function broughtBack(fileName: string, landedAs: string, old: string, pkgName: string | null): Promise<void> {
  await useLibrary().load();
  const at = landedPlace(landedAs);
  const taken = landedAs !== fileName ? landedAs : null;
  const target = at.target;
  const doors = target ? [{ label: "Show in the Rift", run: () => showInRift(target, at.focus) }] : [];
  useToasts().push("rift", broughtBackToast(old, pkgName, at.lead, taken), doors);
}

/** R18.4 / R18.5: `Put it away` on THE VERSIONS card, one move, told once. */
async function putAway(packageId: string): Promise<void> {
  if (puttingAway.value) return;
  const e = entry.value;
  const older = e?.older.find((o) => o.pkg.package.id === packageId);
  const old = older ? versionName(older.version.version, older.version.fileName) : "That version";
  const name = familyName();
  puttingAway.value = packageId;
  try {
    const backend = await rift();
    const moved = await backend.putAway(packageId);
    void useUndercroft().load();
    useToasts().push("rift", putAwayToast(old, name), [
      {
        label: "Bring it back",
        run: async () => {
          const landed = await useUndercroft().bringBack(moved.landedAs);
          if (landed) await broughtBack(moved.landedAs, landed, old, name);
        },
      },
    ]);
  } catch (raw) {
    const f = asRiftFailure(raw);
    const failed = { kind: f.kind, detail: { ...f.detail, act: "putAway", file: older?.version.fileName ?? "" } };
    const voiced = voice(failed, "olderVersions");
    const details = failureDetails(failed, "olderVersions");
    const doors =
      f.kind === "refused"
        ? []
        : [
            { label: "Try again", run: () => putAway(packageId) },
            { label: "Copy details", run: () => copy(details) },
          ];
    useToasts().push("ember", voicedLine(voiced), doors);
  } finally {
    puttingAway.value = null;
  }
}

async function copy(details: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(details);
    useToasts().push("rift", "The details are on your clipboard.");
  } catch {
    useToasts().push("warn", "The clipboard refused; the details are in the log instead.");
  }
}

export function useVersions() {
  return {
    leadId,
    entry,
    stage,
    keepPlan,
    kept,
    keptNames,
    failure,
    puttingAway,
    lines,
    lineState,
    nameOfFile,
    open,
    close,
    beginKeepNewest,
    confirmKeepNewest,
    backToCard,
    putAway,
    bringBackLine,
    broughtBack,
    showInRift,
    showVersions,
    copy,
  };
}

export function resetVersions(): void {
  close();
  puttingAway.value = null;
  lines.value = new Map();
  keptNames.value = new Map();
}
