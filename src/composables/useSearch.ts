// The Riftstone is the search (Bold Choice 1). The query, the facets on the
// stone, the answers, and the seam's state — rest, answered, silent — which
// THE SEAM ANSWERS (§11.1) reads. Facets re-run the search but never fire the
// seam: it answers words, not filters.
// v0.2: the NEXUS facets — categories ANY-of (a mod has one), tags ALL-of
// (each tag narrows), both sent to the spine, which compares them
// case-insensitively. Each chip PREDICTS its answers: a second, label-free
// answer set (the same words and filters, no labels) is counted per chip in
// the window, so a chip's number is what pressing it would show right now —
// not the library total, which stops predicting the moment anything narrows.

import { computed, ref } from "vue";
import type { SearchFilters, SearchQuery, SearchResult } from "@/types/rift";
import { asRiftFailure, rift, type RiftBackend } from "@/services/rift";
import { searchTokens } from "@/services/highlight";
import { voice, type Voiced } from "@/services/voice";
import { useStone } from "./useStone";

export type StateFacet = "all" | "notInstalled" | "installed" | "conflict" | "update";
export type SortKey = "best" | "name" | "package";
export type SeamState = "rest" | "answered" | "silent";

export const DEBOUNCE_MS = 120;
/** The spine's default answer cap (SearchQuery.limit 0). */
export const ANSWER_LIMIT = 200;
/** The label-free count query asks for this many; a reply this long was cut,
 *  so its counts would lie and the chips fall back to the library totals. */
export const COUNT_LIMIT = 5000;

const text = ref("");
const settledText = ref("");
const stateFacet = ref<StateFacet>("all");
const offRoots = ref<string[]>([]);
const includeMenuOnly = ref(false);
const categories = ref<string[]>([]);
const tags = ref<string[]>([]);
/** The same query without its labels: what each chip's count is read from.
 *  null when nothing narrows the library (the spine's totals then predict
 *  exactly) or when the label-free answers were cut at COUNT_LIMIT. */
const unlabeledBase = ref<SearchResult[] | null>(null);
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
  return {
    installed: installedFilter(),
    rootIds: rootFilter(),
    includeMenuOnly: includeMenuOnly.value,
    categories: [...categories.value],
    tags: [...tags.value],
  };
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

/** Anything at all narrows the answers below the whole library. */
function narrows(query: SearchQuery): boolean {
  const f = query.filters;
  return Boolean(
    query.text.trim() ||
    f.installed !== null ||
    f.rootIds.length ||
    f.categories.length ||
    f.tags.length ||
    stateFacet.value !== "all",
  );
}

/** The label-free answers the chips count, or null when the totals predict. */
async function countBase(
  backend: RiftBackend,
  query: SearchQuery,
  hits: SearchResult[],
): Promise<SearchResult[] | null> {
  if (!narrows(query)) return null;
  const labelled = query.filters.categories.length > 0 || query.filters.tags.length > 0;
  if (!labelled && hits.length < ANSWER_LIMIT) return hits;
  const base = await backend.search({
    text: query.text,
    filters: { ...query.filters, categories: [], tags: [] },
    limit: COUNT_LIMIT,
  });
  return base.length >= COUNT_LIMIT ? null : base;
}

async function run(answer = false): Promise<void> {
  const mine = ++requestSeq;
  const query: SearchQuery = { text: text.value, filters: filters(), limit: 0 };
  try {
    const backend = await rift();
    const hits = await backend.search(query);
    const base = await countBase(backend, query, hits);
    if (mine !== requestSeq) return;
    raw.value = hits;
    unlabeledBase.value = base;
    settledText.value = query.text;
    failure.value = null;
    loaded.value = true;
    activeIndex.value = Math.min(activeIndex.value, results.value.length - 1);
    if (answer && query.text.trim() && results.value.length) answerSeq.value += 1;
  } catch (rawFailure) {
    if (mine === requestSeq) failure.value = voice(asRiftFailure(rawFailure), "search");
  }
}

const sameLabel = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();
const keepKnown = (chosen: string[], names: string[]) => chosen.filter((c) => names.some((n) => sameLabel(n, c)));
const carries = (hit: SearchResult, tag: string) => hit.nexusTags.some((t) => sameLabel(t, tag));
const inCategories = (hit: SearchResult, names: string[]) =>
  !names.length || names.some((c) => sameLabel(c, hit.nexusCategory ?? ""));

/** What pressing (or keeping) a category chip shows: the label-free answers
 *  in that category that carry every chosen tag. null: the totals predict. */
function categoryPrediction(name: string): number | null {
  const base = unlabeledBase.value;
  if (!base) return null;
  return base.filter((h) => facetKeeps(h) && inCategories(h, [name]) && tags.value.every((t) => carries(h, t))).length;
}

/** What a tag chip shows: the chosen categories, every chosen tag, and this one. */
function tagPrediction(name: string): number | null {
  const base = unlabeledBase.value;
  if (!base) return null;
  const wanted = tags.value.some((t) => sameLabel(t, name)) ? tags.value : [...tags.value, name];
  return base.filter((h) => facetKeeps(h) && inCategories(h, categories.value) && wanted.every((t) => carries(h, t)))
    .length;
}

/** How many answers the words (and the other facets) found before the labels hid them. */
const beforeLabels = computed(() => (unlabeledBase.value ?? raw.value).filter(facetKeeps).length);

const andList = (names: string[], word: string) =>
  names.length < 2 ? (names[0] ?? "") : `${names.slice(0, -1).join(", ")} ${word} ${names.at(-1) ?? ""}`;

/** The label filter in words: "Armour or Visuals, carrying Fashion and Retexture". */
const labelReading = computed(() => {
  const cats = categories.value;
  const chosen = tags.value;
  if (!cats.length && !chosen.length) return null;
  const head = cats.length ? andList(cats, "or") : "Any mod";
  return chosen.length ? `${head}, carrying ${andList(chosen, "and")}` : head;
});

function toggleIn(list: string[], name: string): string[] {
  return list.some((x) => sameLabel(x, name)) ? list.filter((x) => !sameLabel(x, name)) : [...list, name];
}

const labelFiltered = computed(() => categories.value.length > 0 || tags.value.length > 0);

/** Drop chosen labels the library no longer carries (a chip that is gone
 *  would hide everything with nothing on screen to undo it). */
function pruneLabels(known: { categories: string[]; tags: string[] }): void {
  const nextCategories = keepKnown(categories.value, known.categories);
  const nextTags = keepKnown(tags.value, known.tags);
  if (nextCategories.length === categories.value.length && nextTags.length === tags.value.length) return;
  refacet(() => {
    categories.value = nextCategories;
    tags.value = nextTags;
  });
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
    categories,
    tags,
    labelFiltered,
    labelReading,
    beforeLabels,
    categoryPrediction,
    tagPrediction,
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
    toggleCategory: (name: string) => refacet(() => (categories.value = toggleIn(categories.value, name))),
    toggleTag: (name: string) => refacet(() => (tags.value = toggleIn(tags.value, name))),
    hasCategory: (name: string) => categories.value.some((c) => sameLabel(c, name)),
    hasTag: (name: string) => tags.value.some((t) => sameLabel(t, name)),
    clearLabels: () =>
      refacet(() => {
        categories.value = [];
        tags.value = [];
      }),
    pruneLabels,
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
  categories.value = [];
  tags.value = [];
  unlabeledBase.value = null;
  sort.value = "best";
  grouped.value = false;
  raw.value = [];
  failure.value = null;
  loaded.value = false;
  answerSeq.value = 0;
  activeIndex.value = -1;
}
