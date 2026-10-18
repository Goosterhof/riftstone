// THE KEY'S EXTRAS (v0.3): whose key the stone holds, and what it does with
// it besides the anonymous update checks. The account comes from the
// overview's NexusSummary (`key`, `account`), which the spine fills from
// /v1/users/validate.json at launch (no quota spent) and again on
// refresh_nexus_account. The window shows the account's name and whether it
// is premium; it never sees the key or the e-mail.
//
// STANDINGS: the mods this account tracks on Nexus and its endorsements
// (get_nexus_standings), cached by the spine for the session; `refresh` asks
// Nexus again. Tracking and endorsing answer with the new standings. Every
// one of these is a metered v1 call, so nothing here runs without a press or
// a screen that shows it.
//
// Standings belong to ONE account. A key that changes hands (Replace key…
// with another account's key) starts over, as the spine does (account.rs
// key_changed): the old account's tracked mods, endorsements and refusals are
// forgotten, an answer that began under it is never applied, and the new
// account's are read (followAccount).

import { computed, ref } from "vue";
import type { EndorseStatus, NexusKeyStanding, NexusStandings } from "@/types/rift";
import { asRiftFailure, rift } from "@/services/rift";
import { voice, type Voiced } from "@/services/voice";
import { useFarWorld } from "./useFarWorld";
import { useStone } from "./useStone";

const refreshing = ref(false);
const accountFailure = ref<Voiced | null>(null);
const standings = ref<NexusStandings | null>(null);
const standingsLoading = ref(false);
const standingsFailure = ref<Voiced | null>(null);
/** The mod whose tracking or endorsement is being changed. */
const standingBusy = ref<number | null>(null);
/** R16.22: a refusal (endorseTooSoon, endorseNotDownloaded, endorseOwnMod, or
 *  any other failure) is said under the mod's own controls, never as a toast:
 *  the sentence stays beside the press that earned it until the next press. */
const standingRefusals = ref<Record<number, Voiced>>({});
let requestSeq = 0;
/** Bumps when the standings change hands: a press's answer from before never lands. */
let epoch = 0;
/** The account the standings on hand belong to (null: none read yet). */
let heldFor: string | null = null;

const summary = computed(() => useStone().overview.value?.nexus ?? null);
const standing = computed<NexusKeyStanding>(() => summary.value?.key ?? "none");
const account = computed(() => summary.value?.account ?? null);
const premium = computed(() => account.value?.premium ?? false);
/** The key works now: the extras (downloads, tracking, endorsing) can be asked for. */
const connected = computed(() => standing.value === "connected");
/** Whose standings the window may show now: the connected account's name, else nobody. */
const owner = computed(() => (connected.value ? (account.value?.name ?? null) : null));

const trackedIds = computed(() => new Set((standings.value?.tracked ?? []).map((c) => c.modId)));
const endorsements = computed(
  () => new Map((standings.value?.endorsements ?? []).map((e) => [e.modId, e.status] as const)),
);

/** Validate the stored key again (no quota spent). A rejected key comes back
 *  as the summary's `key: "rejected"`, not as a failure; out of reach, the
 *  failure says so and the key stays stored. */
async function refreshAccount(): Promise<boolean> {
  refreshing.value = true;
  accountFailure.value = null;
  try {
    const backend = await rift();
    await backend.refreshNexusAccount();
    return true;
  } catch (raw) {
    accountFailure.value = voice(asRiftFailure(raw), "nexusAccount");
    return false;
  } finally {
    refreshing.value = false;
    await Promise.all([useStone().refreshOverview(), useFarWorld().refreshQuota()]);
  }
}

/** The account's tracked mods and endorsements: the spine's session cache,
 *  or Nexus again with `refresh`. Without a key there is nothing to ask. */
async function loadStandings(refresh = false): Promise<void> {
  if (standing.value === "none") {
    standings.value = null;
    return;
  }
  const mine = ++requestSeq;
  standingsLoading.value = true;
  try {
    const backend = await rift();
    const next = await backend.getNexusStandings(refresh);
    if (mine !== requestSeq) return;
    standings.value = next;
    standingsFailure.value = null;
  } catch (raw) {
    if (mine === requestSeq) standingsFailure.value = voice(asRiftFailure(raw), "standings");
  } finally {
    if (mine === requestSeq) standingsLoading.value = false;
  }
}

function refuse(modId: number, refusal: Voiced | null): void {
  const next = { ...standingRefusals.value };
  if (refusal) next[modId] = refusal;
  else delete next[modId];
  standingRefusals.value = next;
}

/** The key changed hands: forget the old account's standings and the
 *  sentences its presses left, and let no answer from before land. */
function forgetStandings(): void {
  epoch += 1;
  requestSeq += 1;
  standings.value = null;
  standingsLoading.value = false;
  standingsFailure.value = null;
  standingBusy.value = null;
  standingRefusals.value = {};
}

/** Keep the standings on hand the connected account's: a new owner (another
 *  account's key, or none) forgets the old one's first; a connected owner's
 *  are read (the spine's session cache, or Nexus when the key is new). */
async function followAccount(): Promise<void> {
  const now = owner.value;
  if (now !== heldFor) {
    forgetStandings();
    heldFor = now;
  }
  if (now) await loadStandings();
}

async function change(modId: number, act: () => Promise<NexusStandings>): Promise<boolean> {
  const mine = epoch;
  standingBusy.value = modId;
  refuse(modId, null);
  try {
    const next = await act();
    if (mine !== epoch) return false;
    standings.value = next;
    requestSeq += 1;
    standingsFailure.value = null;
    return true;
  } catch (raw) {
    if (mine === epoch) refuse(modId, voice(asRiftFailure(raw), "standings"));
    return false;
  } finally {
    if (mine === epoch) standingBusy.value = null;
    await useFarWorld().refreshQuota();
  }
}

async function setTracked(modId: number, tracked: boolean): Promise<boolean> {
  const backend = await rift();
  return change(modId, () => backend.setTracked(modId, tracked));
}

async function setEndorsement(modId: number, status: EndorseStatus): Promise<boolean> {
  const backend = await rift();
  return change(modId, () => backend.setEndorsement(modId, status));
}

export function useNexusAccount() {
  return {
    summary,
    standing,
    account,
    premium,
    connected,
    refreshing,
    accountFailure,
    refreshAccount,
    standings,
    standingsLoading,
    standingsFailure,
    standingBusy,
    standingRefusals,
    /** The sentence Nexus's last refusal left under this mod's controls. */
    refusalOf: (modId: number): Voiced | null => standingRefusals.value[modId] ?? null,
    dismissRefusal: (modId: number) => refuse(modId, null),
    trackedIds,
    owner,
    loadStandings,
    followAccount,
    setTracked,
    setEndorsement,
    isTracked: (modId: number) => trackedIds.value.has(modId),
    /** This account's endorsement of a mod; null while undecided. */
    endorsementOf: (modId: number): EndorseStatus | null => endorsements.value.get(modId) ?? null,
  };
}

export function resetNexusAccount(): void {
  requestSeq = 0;
  epoch = 0;
  heldFor = null;
  refreshing.value = false;
  accountFailure.value = null;
  standings.value = null;
  standingsLoading.value = false;
  standingsFailure.value = null;
  standingBusy.value = null;
  standingRefusals.value = {};
}
