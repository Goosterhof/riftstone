// NEXUS — the far world. Cached update state, the anonymous check (no key
// needed), linking a package by URL or mod id, identifying by MD5, and the
// optional personal key.

import { computed, ref } from "vue";
import type { UpdateRow, UpdatesProgressEvent, UpdatesView } from "@/types/rift";
import { asRiftFailure, rift } from "@/services/rift";
import { quotaProbe, type NexusQuota } from "@/services/pending";
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

const linked = computed(() => (updates.value?.rows ?? []).filter((r) => r.link !== null));
const unlinked = computed(() => (updates.value?.rows ?? []).filter((r) => r.link === null));
const updateCount = computed(() => linked.value.filter((r) => r.state.kind === "updateAvailable").length);
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
  await load();
}

async function identify(packageIds: string[]): Promise<void> {
  identifying.value = true;
  identifyNote.value = null;
  try {
    const backend = await rift();
    const found = await backend.identifyPackages(packageIds);
    const matched = found.filter((r) => r.matched).length;
    identifyNote.value =
      matched > 0
        ? `Nexus recognised ${matched} of ${found.length} by their MD5 and linked them.`
        : `Nexus didn't recognise any of the ${found.length} by MD5 — they may be repacks. Link them by URL instead.`;
    await load();
  } catch (raw) {
    identifyNote.value = voicedLine(voice(asRiftFailure(raw), "nexusCheck"));
  } finally {
    identifying.value = false;
  }
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
}
