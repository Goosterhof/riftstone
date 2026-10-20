// v0.5 — the versions of one file in Your World (#00015 R18.4, R18.5, R18.10):
// THE VERSIONS card, the keep-newest plan (THE RETURN — OLDER VERSIONS), the
// one-press put-away, and the put-away lines a landed plan carries with their
// `Bring it back`. The card stands in the World pane over whatever it held, so
// closing it returns the pane to that (the option's detail, or the summary).
// stage walks:  card → keepPlan → keeping → kept | failed

import { computed, nextTick, ref, watch } from "vue";
import type { KeepNewestPlanView, KeepNewestResultView, LibraryPackage, PutAwayView } from "@/types/rift";
import { asRiftFailure, rift } from "@/services/rift";
import { failureDetails, voice, voicedLine, type RecoveryKind, type Voiced } from "@/services/voice";
import {
  JOINED_LINE,
  askedKind,
  askedOf,
  broughtBackToast,
  familyNames,
  joinCandidates,
  keptApartToast,
  keptToast,
  olderPlace,
  partedToast,
  putAwayToast,
  versionName,
  type FamilyEntry,
  type JoinCandidate,
} from "@/services/versions";
import { questionKey, useLibrary } from "./useLibrary";
import { useStone } from "./useStone";
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

// --- R18.18: the player's word --------------------------------------------------

/** The package the card asks about, when it stands in its question form; `manual`
 *  for R18.18.6's door (no question, the player names the file). */
const asking = ref<{ packageId: string; manual: boolean } | null>(null);
/** R18.18.6: the file the player picked when two or more qualify; else the one. */
const picked = ref<string | null>(null);
/** The word in flight: the yes (until the library draws the join) or the no. */
const word = ref<"idle" | "joining" | "declining">("idle");
/** The package `Take it back` is parting now. */
const parting = ref<string | null>(null);
/** R18.18.5: a word that did not land, voiced inside the card. */
type WordAct = "join" | "decline" | "part";
const wordFailure = ref<{ voiced: Voiced; details: string; act: WordAct; packageId: string } | null>(null);
/** R18.18.3: `Joined by your word…`, above the rows once the join is drawn. */
const joinedLine = ref<string | null>(null);
/** The one watch waiting for the library's next answer (a join or a take-back landing). */
let landing: (() => void) | null = null;
/** Each card the pane shows gets its own number. Every press that awaits the
 *  spine takes the number first, and an answer that lands after it moved belongs
 *  to a card the player has left: it never draws on the card open now (General,
 *  PR #2). Closing counts, so a card closed and opened again is a new card. */
let shownCard = 0;
const takeCard = (): number => shownCard;
const stillShown = (card: number): boolean => card === shownCard;

function stopLanding(): void {
  landing?.();
  landing = null;
}

/** The open card's entry, read from the library the tree draws. */
const entry = computed<FamilyEntry | null>(
  () => useLibrary().entries.value.find((e) => e.lead.package.id === leadId.value) ?? null,
);

/** Opens THE VERSIONS card for any member of a family. */
function open(packageId: string): void {
  const packages = useLibrary().packages.value;
  const place = olderPlace(packages, packageId);
  shownCard += 1;
  leadId.value = place ? place.lead.package.id : packageId;
  stage.value = "card";
  keepPlan.value = null;
  kept.value = null;
  failure.value = null;
  forgetWord();
}

function close(): void {
  shownCard += 1;
  leadId.value = null;
  stage.value = "card";
  keepPlan.value = null;
  kept.value = null;
  failure.value = null;
  forgetWord();
}

function forgetWord(): void {
  stopLanding();
  asking.value = null;
  picked.value = null;
  word.value = "idle";
  wordFailure.value = null;
  joinedLine.value = null;
}

const libraryPackage = (packageId: string): LibraryPackage | null =>
  useLibrary().packages.value.find((p) => p.package.id === packageId) ?? null;

/** The questioned package, read from the library as it is now. */
const askedPackage = computed<LibraryPackage | null>(() =>
  asking.value ? libraryPackage(asking.value.packageId) : null,
);

/** R18.18.6: the files the manual join may name (read from the contract's fields). */
const candidates = computed<JoinCandidate[]>(() => {
  const pkg = askedPackage.value;
  return pkg && asking.value?.manual ? joinCandidates(pkg, useLibrary().packages.value) : [];
});

/** R18.18.1 / R18.18.6: THE VERSIONS card in its question form. With a question
 *  the card draws the file it names; without one (the manual door) the one file
 *  that qualifies, or none until the player picks among two or more. */
function openQuestion(packageId: string): void {
  close();
  const pkg = libraryPackage(packageId);
  if (!pkg) return;
  const q = pkg.versionQuestion;
  if (q) {
    asking.value = { packageId, manual: false };
    leadId.value = q.ofPackageId;
    return;
  }
  asking.value = { packageId, manual: true };
  const offered = candidates.value;
  if (offered.length === 1) pick(offered[0]?.packageId ?? null);
}

/** R18.18.6: the radio list's pick; the card draws that file's versions above the folder. */
function pick(packageId: string | null): void {
  picked.value = packageId;
  leadId.value = packageId;
}

/** The file the yes or the no names: always `versionQuestion`'s own id, never
 *  recomputed; the manual door's pick when there is no question. */
function ofIdNow(): string | null {
  const a = asking.value;
  if (!a) return null;
  return a.manual ? picked.value : (askedPackage.value?.versionQuestion?.ofPackageId ?? null);
}

/** The file asked about as the copy names it (`v1.3`, else the file's name). */
function ofNameNow(): string {
  const q = askedPackage.value?.versionQuestion;
  if (q) return askedOf(q).text;
  const chosen = candidates.value.find((c) => c.packageId === picked.value);
  return chosen ? versionName(chosen.version, chosen.fileName) : "that file";
}

function wordFailed(raw: unknown, act: WordAct, packageId: string, extra: Record<string, string>): void {
  const f = asRiftFailure(raw);
  const failed = { kind: f.kind, detail: { ...f.detail, act, ...extra } };
  wordFailure.value = {
    voiced: voice(failed, "olderVersions"),
    details: failureDetails(failed, "olderVersions"),
    act,
    packageId,
  };
}

/** Waits for the library's next answer to show `done`, then runs `then` once. */
function whenLibrary(done: () => boolean, then: () => void): void {
  stopLanding();
  const stop = watch(
    () => useLibrary().packages.value,
    () => {
      if (!done()) return;
      stopLanding();
      then();
    },
    { immediate: false },
  );
  landing = stop;
}

/** R18.18.3: the yes. Both ids from `versionQuestion` (or the manual pick). The
 *  card changes at the library's next answer, never when the command resolves. */
async function join(): Promise<void> {
  const a = asking.value;
  const ofId = ofIdNow();
  const pkg = askedPackage.value;
  if (!a || !ofId || !pkg || word.value !== "idle") return;
  const packageId = a.packageId;
  const of = ofNameNow();
  const card = takeCard();
  word.value = "joining";
  wordFailure.value = null;
  try {
    const backend = await rift();
    await backend.joinVersions(packageId, ofId);
  } catch (raw) {
    if (!stillShown(card)) return;
    word.value = "idle";
    wordFailed(raw, "join", packageId, { of, what: askedKind(pkg), file: pkg.package.fileName });
    return;
  }
  // closed, or another card opened, while the yes was in flight: the join stands,
  // and the Rift draws it at the library's next answer, but no card reopens for it
  if (!stillShown(card)) return;
  whenLibrary(
    () => {
      const now = libraryPackage(packageId);
      return now?.package.nexus?.source === "joined" && !!now.family;
    },
    () => {
      const now = libraryPackage(packageId);
      const lead = now?.family?.lead ?? ofId;
      asking.value = null;
      picked.value = null;
      word.value = "idle";
      leadId.value = lead;
      joinedLine.value = JOINED_LINE;
      // the lead's entry opens in the expansion memory; the roving stop moves to the joined row
      const library = useLibrary();
      library.expand(lead);
      library.focusedKey.value = packageId;
    },
  );
}

/** R18.18.4: the no. The card closes once the spine remembers it. */
async function decline(): Promise<void> {
  const a = asking.value;
  const ofId = ofIdNow();
  const pkg = askedPackage.value;
  if (!a || a.manual || !ofId || !pkg || word.value !== "idle") return;
  const card = takeCard();
  const of = ofNameNow();
  const kind = askedKind(pkg);
  word.value = "declining";
  wordFailure.value = null;
  try {
    const backend = await rift();
    await backend.declineVersionQuestion(a.packageId, ofId);
  } catch (raw) {
    if (!stillShown(card)) return;
    word.value = "idle";
    wordFailed(raw, "decline", a.packageId, { of, what: kind, file: pkg.package.fileName });
    return;
  }
  // the no is remembered either way; only the card that asked closes
  if (stillShown(card)) close();
  useToasts().push("rift", keptApartToast(kind));
}

/** R18.18.3: `Take it back`, on a joined row of THE VERSIONS card only. No
 *  confirm (R18.18.6 joins it again). A family left with one member closes the card. */
async function takeBack(packageId: string): Promise<void> {
  const pkg = libraryPackage(packageId);
  if (pkg?.package.nexus?.source !== "joined" || parting.value) return;
  const fileName = pkg.package.fileName;
  const card = takeCard();
  parting.value = packageId;
  wordFailure.value = null;
  joinedLine.value = null;
  try {
    const backend = await rift();
    await backend.partVersions(packageId);
  } catch (raw) {
    if (stillShown(card)) wordFailed(raw, "part", packageId, { file: fileName });
    return;
  } finally {
    parting.value = null;
  }
  useToasts().push("rift", partedToast(fileName));
  if (!stillShown(card)) return;
  whenLibrary(
    () => libraryPackage(packageId)?.package.nexus?.source !== "joined",
    () => {
      if (!entry.value || entry.value.older.length === 0) close();
    },
  );
}

/** R18.18.5's doors: `Scan again`, `Try again`, `Copy details`. */
async function recoverWord(kind: RecoveryKind): Promise<void> {
  const failed = wordFailure.value;
  if (!failed) return;
  if (kind === "copyDetails") {
    await copy(failed.details);
    return;
  }
  wordFailure.value = null;
  if (kind === "rescan") {
    await useStone().startScan();
    return;
  }
  if (kind !== "retry") return;
  if (failed.act === "join") await join();
  else if (failed.act === "decline") await decline();
  else await takeBack(failed.packageId);
}

/** THE FAR WORLD's door (R18.18.1): the Rift, the package open, its question row
 *  focused, and the question card. */
async function showQuestion(packageId: string): Promise<void> {
  await showInRift(packageId, questionKey(packageId));
  openQuestion(packageId);
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
  const card = takeCard();
  try {
    const backend = await rift();
    const plan = await backend.planKeepNewest(packageId);
    // a plan only reads: one for a card the player left is dropped, nothing moved
    if (!stillShown(card)) return;
    keepPlan.value = plan;
    stage.value = "keepPlan";
  } catch (raw) {
    if (!stillShown(card)) return;
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
  const card = takeCard();
  stage.value = "keeping";
  try {
    const backend = await rift();
    const result = await backend.keepNewest(plan.packageId);
    // the toast tells it wherever the player is now; the landing draws only on its own card
    useToasts().push("world", keptToast(keeps, name));
    void useUndercroft().load();
    if (!stillShown(card)) return;
    kept.value = result;
    stage.value = "kept";
  } catch (raw) {
    const f = asRiftFailure(raw);
    const voiced = voice(f, "uninstall");
    const details = failureDetails(f, "uninstall");
    if (stillShown(card)) {
      failure.value = { voiced, details };
      stage.value = "failed";
      return;
    }
    // the card is gone: the failure is still told, never dropped
    useToasts().push("ember", voicedLine(voiced), [{ label: "Copy details", run: () => copy(details) }]);
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
    asking,
    askedPackage,
    candidates,
    picked,
    word,
    parting,
    wordFailure,
    joinedLine,
    openQuestion,
    pick,
    join,
    decline,
    takeBack,
    recoverWord,
    showQuestion,
  };
}

export function resetVersions(): void {
  close();
  puttingAway.value = null;
  parting.value = null;
  lines.value = new Map();
  keptNames.value = new Map();
}
