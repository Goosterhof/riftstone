// v0.2 — the NEXUS facets. What Nexus says each linked mod is (its category,
// its tags), counted in installable options; and how many packages carry no
// labels yet, with the one press that fixes it: identify the unlinked ones by
// MD5, then ask Nexus about everything. Refetched after rift://updates-changed
// and rift://library-changed (boot.ts).
//
// A press re-hashes only what has not already failed to match: an archive
// Nexus did not recognise once will not be recognised by the same bytes
// again. And after a press that learned nothing, the button gives way to a
// door to the NEXUS page's Link… rows — until the library itself changes.

import { computed, ref } from "vue";
import type { LabelFacets } from "@/types/rift";
import { asRiftFailure, rift } from "@/services/rift";
import { voice } from "@/services/voice";
import { askNexusToast } from "@/services/ledger";
import { useFarWorld } from "./useFarWorld";
import { useSearch } from "./useSearch";
import { useToasts } from "./useToasts";

const facets = ref<LabelFacets | null>(null);
const asking = ref(false);
/** Packages whose archive Nexus already failed to recognise by MD5. */
const md5Misses = ref<Set<string>>(new Set());
/** The unlabeled count a fruitless press left behind; null after a press that learned. */
const fruitlessAt = ref<number | null>(null);

/** Labels exist at all: false on a first launch before any update check. */
const carved = computed(() => Boolean(facets.value && (facets.value.categories.length || facets.value.tags.length)));

/** Pressing again would only hash the same bytes: offer the NEXUS page instead. */
const exhausted = computed(
  () => fruitlessAt.value !== null && fruitlessAt.value === (facets.value?.unlabeledPackages ?? 0),
);

async function load(): Promise<void> {
  try {
    const backend = await rift();
    const next = await backend.getLabels();
    facets.value = next;
    useSearch().pruneLabels({
      categories: next.categories.map((c) => c.name),
      tags: next.tags.map((t) => t.name),
    });
  } catch {
    // The facets are a convenience over search: when they cannot be read the
    // group stays as it was, and the search itself still answers.
  }
}

/** Identify the unlinked packages not already missed, then check Nexus: the
 *  check is what brings the labels home (same batched call, zero extra requests). */
async function identifyAndCheck(): Promise<void> {
  const farWorld = useFarWorld();
  if (!farWorld.updates.value) await farWorld.load();
  const unlinked = farWorld.unlinked.value.map((r) => r.packageId).filter((id) => !md5Misses.value.has(id));
  if (unlinked.length) {
    const found = await farWorld.identify(unlinked);
    const misses = new Set(md5Misses.value);
    for (const r of found) if (!r.matched) misses.add(r.packageId);
    md5Misses.value = misses;
  }
  await farWorld.check();
  await load();
}

async function askNexus(): Promise<void> {
  if (asking.value) return;
  asking.value = true;
  const before = facets.value?.unlabeledPackages ?? 0;
  const hadLabels = carved.value;
  try {
    await identifyAndCheck();
    const after = facets.value?.unlabeledPackages ?? 0;
    const failure = useFarWorld().checkFailure.value;
    const toast = askNexusToast({ before, after, hadLabels, failure });
    useToasts().push(toast.tone, toast.line);
    fruitlessAt.value = !failure && toast.learned === 0 ? after : null;
  } catch (raw) {
    const voiced = voice(asRiftFailure(raw), "nexusCheck");
    const toast = askNexusToast({ before, after: before, hadLabels, failure: voiced });
    useToasts().push(toast.tone, toast.line);
  } finally {
    asking.value = false;
  }
}

export function useLabels() {
  return { facets, carved, asking, exhausted, load, askNexus };
}

export function resetLabels(): void {
  facets.value = null;
  asking.value = false;
  md5Misses.value = new Set();
  fruitlessAt.value = null;
}
