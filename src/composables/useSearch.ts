// The Riftstone is the search (Bold Choice 1). The query, the facets on the
// stone, the answers, and the seam's state — rest, answered, silent — which
// THE SEAM ANSWERS (§11.1) reads. Facets re-run the search but never fire the
// seam: it answers words, not filters.

import { computed, ref } from "vue";
import type { SearchFilters, SearchResult } from "@/types/rift";
import { asRiftFailure, rift } from "@/services/rift";
import { searchTokens } from "@/services/highlight";
import { voice, type Voiced } from "@/services/voice";
import { useStone } from "./useStone";

export type StateFacet = "all" | "notInstalled" | "installed" | "conflict" | "update";
export type SortKey = "best" | "name" | "package";
export type SeamState = "rest" | "answered" | "silent";

export const DEBOUNCE_MS = 120;

const text = ref("");
const settledText = ref("");
const stateFacet = ref<StateFacet>("all");
const offRoots = ref<string[]>([]);
const includeMenuOnly = ref(false);
const sort = ref<SortKey>("best");
const grouped = ref(false);
const raw = ref<SearchResult[]>([]);
const failure = ref<Voiced | null>(null);
const loaded = ref(false);
/** Bumps once per settled query that answered — the seam's flare key. */
const answerSeq = ref(0);
const activeIndex = ref(-1);

let timer: ReturnType<typeof setTimeout> | null = null;
let requestSeq = 0;

function installedFilter(): boolean | null {
  if (stateFacet.value === "installed") return true;
  if (stateFacet.value === "notInstalled") return false;
  return null;
}

function rootFilter(): string[] {
  if (!offRoots.value.length) return [];
  const roots = useStone().overview.value?.roots ?? [];
  return roots.map((r) => r.id).filter((id) => !offRoots.value.includes(id));
}

function filters(): SearchFilters {
  return { installed: installedFilter(), rootIds: rootFilter(), includeMenuOnly: includeMenuOnly.value };
}

function facetKeeps(hit: SearchResult): boolean {
  if (stateFacet.value === "conflict") return hit.conflicted;
  if (stateFacet.value === "update") return hit.update === "available";
  return true;
}

const SORTERS: Record<SortKey, ((a: SearchResult, b: SearchResult) => number) | null> = {
  best: null,
  name: (a, b) => a.name.localeCompare(b.name),
  package: (a, b) => a.packageName.localeCompare(b.packageName) || a.name.localeCompare(b.name),
};

const results = computed(() => {
  const kept = raw.value.filter(facetKeeps);
  const sorter = SORTERS[sort.value];
  return sorter ? [...kept].sort(sorter) : kept;
});

const tokens = computed(() => searchTokens(settledText.value));

const seam = computed<SeamState>(() => {
  if (!settledText.value.trim()) return "rest";
  return results.value.length ? "answered" : "silent";
});

/** Package groups for "Group by package", in result order. */
const groups = computed(() => {
  const order: string[] = [];
  const byPackage = new Map<string, SearchResult[]>();
  for (const hit of results.value) {
    const list = byPackage.get(hit.packageId);
    if (list) list.push(hit);
    else {
      byPackage.set(hit.packageId, [hit]);
      order.push(hit.packageId);
    }
  }
  return order.map((id) => ({ packageId: id, hits: byPackage.get(id) ?? [] }));
});

async function run(answer = false): Promise<void> {
  const mine = ++requestSeq;
  const query = { text: text.value, filters: filters(), limit: 0 };
  try {
    const backend = await rift();
    const hits = await backend.search(query);
    if (mine !== requestSeq) return;
    raw.value = hits;
    settledText.value = query.text;
    failure.value = null;
    loaded.value = true;
    activeIndex.value = Math.min(activeIndex.value, results.value.length - 1);
    if (answer && query.text.trim() && results.value.length) answerSeq.value += 1;
  } catch (rawFailure) {
    if (mine === requestSeq) failure.value = voice(asRiftFailure(rawFailure), "search");
  }
}

function setText(next: string): void {
  text.value = next;
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => {
    timer = null;
    void run(true);
  }, DEBOUNCE_MS);
}

function clearText(): void {
  if (timer) clearTimeout(timer);
  text.value = "";
  activeIndex.value = -1;
  void run(false);
}

function refacet(apply: () => void): void {
  apply();
  void run(false);
}

export function useSearch() {
  return {
    text,
    settledText,
    tokens,
    stateFacet,
    offRoots,
    includeMenuOnly,
    sort,
    grouped,
    results,
    groups,
    failure,
    loaded,
    seam,
    answerSeq,
    activeIndex,
    setText,
    clearText,
    run,
    setStateFacet: (facet: StateFacet) => refacet(() => (stateFacet.value = facet)),
    toggleRoot: (rootId: string) =>
      refacet(() => {
        offRoots.value = offRoots.value.includes(rootId)
          ? offRoots.value.filter((r) => r !== rootId)
          : [...offRoots.value, rootId];
      }),
    allRoots: () => refacet(() => (offRoots.value = [])),
    setMenuOnly: (on: boolean) => refacet(() => (includeMenuOnly.value = on)),
  };
}

export function resetSearch(): void {
  if (timer) clearTimeout(timer);
  timer = null;
  requestSeq = 0;
  text.value = "";
  settledText.value = "";
  stateFacet.value = "all";
  offRoots.value = [];
  includeMenuOnly.value = false;
  sort.value = "best";
  grouped.value = false;
  raw.value = [];
  failure.value = null;
  loaded.value = false;
  answerSeq.value = 0;
  activeIndex.value = -1;
}
