// NEXUS — the far world. Cached update state, the anonymous check (no key
// needed), linking a package by URL or mod id, identifying by MD5, finding a
// package on Nexus by its name (v0.2), and the optional personal key.
//
// Find never links. It asks Nexus about ONE package's name (the press is the
// consent: the name leaves the machine only then), keeps what Nexus offered
// under that row, and waits for the player to pick. Picking links the package
// and runs the update check at once, so its labels arrive with it. A link made
// by mod id names no file: the check pins one only where the spine can (a file
// id, sqid, upload stamp or version to match), and the toast says which.

import { computed, ref } from "vue";
import type {
  Md5Result,
  NexusCandidate,
  NexusFindings,
  UpdateRow,
  UpdatesProgressEvent,
  UpdatesView,
} from "@/types/rift";
import { asRiftFailure, rift } from "@/services/rift";
import { quotaProbe, type NexusQuota } from "@/services/pending";
import { plural } from "@/services/format";
import { voice, voicedLine, type Voiced } from "@/services/voice";
import { useToasts } from "./useToasts";
import { useStone } from "./useStone";

const updates = ref<UpdatesView | null>(null);
const progress = ref<UpdatesProgressEvent | null>(null);
const checking = ref(false);
const checkFailure = ref<Voiced | null>(null);
const keyFailure = ref<Voiced | null>(null);
const keyBusy = ref(false);
const quota = ref<NexusQuota | null>(null);
const linkingId = ref<string | null>(null);
const linkFailure = ref<Voiced | null>(null);
const identifying = ref(false);
const identifyNote = ref<string | null>(null);

/** What a Find… press left under one row. `linkedAtAsk` is how many LINKED
 *  rows each candidate's mod had when Nexus answered, so the count shown can
 *  follow later links and unlinks without asking Nexus again. */
export type FindState =
  | { status: "asking" }
  | { status: "found"; findings: NexusFindings; linkedAtAsk: Record<number, number> }
  | { status: "failed"; failure: Voiced };

const finds = ref<Record<string, FindState>>({});
/** "Find all by name" walking the rows: 1-based row being asked, of total. */
const findAllProgress = ref<{ done: number; total: number } | null>(null);
/** The candidate being linked, `<packageId>:<modId>`, while its link + check run. */
const linkingCandidate = ref<string | null>(null);
/** The one live line the finds speak through (a screen reader hears this, not
 *  every row's list): a single find's answer, or Find all's sum. */
const findNote = ref<string | null>(null);

const linked = computed(() => (updates.value?.rows ?? []).filter((r) => r.link !== null));
const unlinked = computed(() => (updates.value?.rows ?? []).filter((r) => r.link === null));
const updateCount = computed(() => linked.value.filter((r) => r.state.kind === "updateAvailable").length);
const findingAll = computed(() => findAllProgress.value !== null);
const offline = computed(() => checkFailure.value?.inscription === "THE FAR WORLD IS OUT OF REACH");

async function refreshQuota(): Promise<void> {
  const backend = await rift();
  const probe = quotaProbe(backend);
  quota.value = probe ? await probe() : null;
}

async function load(): Promise<void> {
  const backend = await rift();
  updates.value = await backend.getUpdates();
  await refreshQuota();
}

async function check(): Promise<void> {
  checking.value = true;
  checkFailure.value = null;
  try {
    const backend = await rift();
    updates.value = await backend.checkUpdates();
  } catch (raw) {
    checkFailure.value = voice(asRiftFailure(raw), "nexusCheck");
    await load();
  } finally {
    checking.value = false;
    progress.value = null;
    await refreshQuota();
  }
}

async function link(packageId: string, urlOrId: string): Promise<boolean> {
  linkFailure.value = null;
  try {
    const backend = await rift();
    const row: UpdateRow = await backend.linkPackage(packageId, urlOrId.trim());
    linkingId.value = null;
    setFind(packageId, null);
    await load();
    useToasts().push("rift", `${row.packageName} is linked to Nexus mod ${row.link?.modId ?? ""}.`.replace(" .", "."));
    return true;
  } catch (raw) {
    linkFailure.value = voice(asRiftFailure(raw), "nexusLink");
    return false;
  }
}

async function unlink(packageId: string): Promise<void> {
  const backend = await rift();
  await backend.unlinkPackage(packageId);
  setFind(packageId, null);
  await load();
}

/** Identify by MD5; returns what Nexus said per package ([] on a failure). */
async function identify(packageIds: string[]): Promise<Md5Result[]> {
  identifying.value = true;
  identifyNote.value = null;
  try {
    const backend = await rift();
    const found = await backend.identifyPackages(packageIds);
    const matched = found.filter((r) => r.matched).length;
    for (const r of found) if (r.matched) setFind(r.packageId, null);
    identifyNote.value =
      matched > 0
        ? `Nexus recognised ${matched} of ${found.length} by their MD5 and linked them.`
        : `Nexus didn't recognise any of the ${found.length} by MD5 — they may be repacks. Find them by name, or link them by URL.`;
    await load();
    return found;
  } catch (raw) {
    identifyNote.value = voicedLine(voice(asRiftFailure(raw), "nexusCheck"));
    return [];
  } finally {
    identifying.value = false;
  }
}

function setFind(packageId: string, state: FindState | null): void {
  const next = { ...finds.value };
  if (state) next[packageId] = state;
  else delete next[packageId];
  finds.value = next;
}

/** How many LINKED rows point at each mod, per the cached update state. */
function linkedCounts(): Record<number, number> {
  const counts: Record<number, number> = {};
  for (const r of linked.value) if (r.link) counts[r.link.modId] = (counts[r.link.modId] ?? 0) + 1;
  return counts;
}

/** Packages linked to a candidate's mod NOW: the spine's count at the time of
 *  asking, moved by every link and unlink made since. */
export function linkedNow(state: Extract<FindState, { status: "found" }>, c: NexusCandidate): number {
  const now = linkedCounts()[c.modId] ?? 0;
  return Math.max(0, c.linkedPackages + now - (state.linkedAtAsk[c.modId] ?? 0));
}

function nameOf(packageId: string): string {
  return unlinked.value.find((r) => r.packageId === packageId)?.packageName ?? "this package";
}

function answerLine(packageId: string, state: FindState): string {
  const name = `“${nameOf(packageId)}”`;
  if (state.status === "failed") return `${name}: ${voicedLine(state.failure)}`;
  if (state.status !== "found") return "";
  const n = state.findings.candidates.slice(0, 5).length;
  if (!state.findings.terms) return `${name} holds nothing to search for.`;
  return n ? `Nexus offered ${plural(n, "candidate")} for ${name}.` : `Nexus knows no mod named like ${name}.`;
}

/** Ask Nexus about one package's name. Returns the findings, or null when the
 *  ask failed (the failure is voiced under the row). Never links. */
async function find(packageId: string, announce = true): Promise<NexusFindings | null> {
  setFind(packageId, { status: "asking" });
  if (announce) findNote.value = null;
  let state: FindState;
  try {
    const backend = await rift();
    const findings = await backend.findOnNexus(packageId);
    state = { status: "found", findings, linkedAtAsk: linkedCounts() };
  } catch (raw) {
    state = { status: "failed", failure: voice(asRiftFailure(raw), "nexusFind") };
  }
  setFind(packageId, state);
  if (announce) findNote.value = answerLine(packageId, state);
  return state.status === "found" ? state.findings : null;
}

/** Find every unlinked row by name, one at a time, and link none of them.
 *  A failure of the far world (offline, quota, refused) stops the walk, since
 *  every later ask would fail the same way; a failure of one package (it left
 *  the library) is marked under its row and the walk goes on. */
async function findAll(): Promise<void> {
  if (findAllProgress.value) return;
  const ids = unlinked.value.map((r) => r.packageId);
  findNote.value = null;
  let stopped: number | null = null;
  try {
    for (const [i, id] of ids.entries()) {
      findAllProgress.value = { done: i + 1, total: ids.length };
      if ((await find(id, false)) === null && farWorldFailed(id)) {
        stopped = i + 1;
        break;
      }
    }
  } finally {
    findAllProgress.value = null;
  }
  findNote.value = walkSum(ids, stopped);
}

/** The ask failed because the far world did (not because of the package). */
function farWorldFailed(packageId: string): boolean {
  const state = finds.value[packageId] as FindState | undefined;
  return state?.status === "failed" && state.failure.kind === "nexus";
}

/** Find all's one line for the status region: how the walk ended. */
function walkSum(ids: string[], stopped: number | null): string {
  if (stopped !== null) {
    const left = ids.length - stopped;
    const tail = left ? ` ${plural(left, "package")} not asked.` : "";
    return `Find all stopped at ${stopped} of ${ids.length}: Nexus could not be asked (the reason is under that row).${tail}`;
  }
  const states = ids.map((id) => finds.value[id] as FindState | undefined);
  const offered = states.filter((st) => st?.status === "found" && st.findings.candidates.length > 0).length;
  const none = states.filter((st) => st?.status === "found" && st.findings.candidates.length === 0).length;
  const failed = states.filter((st) => st?.status === "failed").length;
  const parts = [`Nexus offered candidates for ${offered} of ${ids.length}`];
  if (none) parts.push(`${none} found nothing`);
  if (failed) parts.push(`${failed} could not be asked`);
  return `${parts.join("; ")}.`;
}

/** How the toast ends after Link this: what the check really did. A link by
 *  mod id names no file, so unless the spine could pin one the file stays
 *  unpinned, and the toast says so rather than promise a pin. */
function afterLink(packageId: string): string {
  if (checkFailure.value) return "Its labels follow at the next check that reaches Nexus.";
  const row = linked.value.find((r) => r.packageId === packageId);
  const pinned = row?.link && (row.link.fileId !== null || row.link.sqid !== null) && row.state.kind !== "unpinned";
  return pinned
    ? "Its labels and file pin came home with it."
    : "Its labels came home with it. No file is pinned yet, so the stone can't tell whether your copy is the newest; link its file's Nexus URL to pin one.";
}

/** The player picked a candidate: link it, then check, so its labels arrive
 *  at once. Returns whether the link took. */
async function linkCandidate(packageId: string, candidate: NexusCandidate): Promise<boolean> {
  linkingCandidate.value = `${packageId}:${candidate.modId}`;
  try {
    const backend = await rift();
    const row = await backend.linkPackage(packageId, String(candidate.modId));
    setFind(packageId, null);
    if (linkingId.value === packageId) linkingId.value = null;
    await load();
    await check();
    useToasts().push(
      "rift",
      `${row.packageName} came through: linked to ${candidate.name}, Nexus mod ${candidate.modId}. ${afterLink(packageId)}`,
    );
    return true;
  } catch (raw) {
    const failure = voice(asRiftFailure(raw), "nexusLink");
    const offered = finds.value[packageId]?.status === "found";
    useToasts().push("ember", voicedLine(failure));
    if (!offered) setFind(packageId, { status: "failed", failure });
    return false;
  } finally {
    linkingCandidate.value = null;
  }
}

/** The quiet line under a candidate's name: the parts Nexus gave, in order. */
export function candidateLine(c: NexusCandidate): string {
  const parts: string[] = [];
  if (c.author) parts.push(`by ${c.author}`);
  if (c.category) parts.push(c.category);
  if (c.downloads !== null) parts.push(plural(c.downloads, "download"));
  if (c.version) parts.push(`v${c.version.replace(/^v/i, "")}`);
  return parts.join(" · ");
}

async function setKey(key: string | null): Promise<boolean> {
  keyBusy.value = true;
  keyFailure.value = null;
  try {
    const backend = await rift();
    await backend.setNexusKey(key);
    await useStone().refreshOverview();
    // the quota is the key's v1 budget: it appears with a key, goes with it
    await refreshQuota();
    return true;
  } catch (raw) {
    keyFailure.value = voice(asRiftFailure(raw), "nexusKey");
    return false;
  } finally {
    keyBusy.value = false;
  }
}

export function useFarWorld() {
  return {
    updates,
    progress,
    checking,
    checkFailure,
    keyFailure,
    keyBusy,
    quota,
    linkingId,
    linkFailure,
    identifying,
    identifyNote,
    linked,
    unlinked,
    updateCount,
    offline,
    load,
    check,
    link,
    unlink,
    identify,
    finds,
    findAllProgress,
    findingAll,
    linkingCandidate,
    findNote,
    setFind,
    find,
    findAll,
    linkCandidate,
    setKey,
    onProgress: (event: UpdatesProgressEvent) => (progress.value = event),
  };
}

export function resetFarWorld(): void {
  updates.value = null;
  progress.value = null;
  checking.value = false;
  checkFailure.value = null;
  keyFailure.value = null;
  keyBusy.value = false;
  quota.value = null;
  linkingId.value = null;
  linkFailure.value = null;
  identifying.value = false;
  identifyNote.value = null;
  finds.value = {};
  findAllProgress.value = null;
  linkingCandidate.value = null;
  findNote.value = null;
}
