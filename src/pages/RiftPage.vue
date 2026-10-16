<script setup lang="ts">
// §8.1 — THE RIFT, the search-first library: a cold column of answers and,
// beside it, the warm pane of Your World. The Rift also holds one package's
// tree and the lookbook, which folds Your World to its sliver.
import { computed, nextTick, ref, watch } from "vue";
import { useSearch, type SortKey } from "@/composables/useSearch";
import { useStone } from "@/composables/useStone";
import { useWorld } from "@/composables/useWorld";
import { useNavigation } from "@/composables/useNavigation";
import { useSettings } from "@/composables/useSettings";
import { usePackage } from "@/composables/usePackage";
import { usePreflight } from "@/composables/usePreflight";
import { formatBytes, plural } from "@/services/format";
import { adultnessOf, veils } from "@/services/ledger";
import OptionCard from "@/components/OptionCard.vue";
import TroubledCard from "@/components/TroubledCard.vue";
import RiftInscription from "@/components/RiftInscription.vue";
import RiftButton from "@/components/RiftButton.vue";
import FailureNotice from "@/components/FailureNotice.vue";
import YourWorldPane from "@/components/YourWorldPane.vue";
import PackageTree from "@/components/PackageTree.vue";
import LookbookGrid from "@/components/LookbookGrid.vue";

const search = useSearch();
const stone = useStone();
const world = useWorld();
const nav = useNavigation();
const pkg = usePackage();
const preflight = usePreflight();
const { settings } = useSettings();

const list = ref<HTMLElement | null>(null);
const lookbook = ref<InstanceType<typeof LookbookGrid> | null>(null);
const worldFolded = ref(true);

const query = computed(() => search.settledText.value.trim());
const library = computed(() => stone.overview.value?.library ?? null);
const empty = computed(() => library.value !== null && library.value.packages === 0 && !stone.scanning.value);
const rootLabels = computed(() => new Map((stone.overview.value?.roots ?? []).map((r) => [r.id, r.label])));
const anyRootOff = computed(() => search.offRoots.value.length > 0);
/** Trouble cards carry no labels: under a label filter they would lead a
 *  column they do not belong to. */
const showTroubled = computed(() => !query.value && !search.labelFiltered.value && stone.troubled.value.length > 0);
/** A label filter hid every answer: say which filter, and how to undo it. */
const labelsHidAll = computed(() => search.labelFiltered.value && search.loaded.value && !search.results.value.length);
const lowerFirst = (t: string) => t.charAt(0).toLowerCase() + t.slice(1);
const labelsBody = computed(() => {
  const n = search.beforeLabels.value;
  const found = query.value
    ? `The words found ${plural(n, "answer")}; the labels hid every one.`
    : `None of the ${plural(n, "option")} in view ${n === 1 ? "carries" : "carry"} that.`;
  return `${search.labelReading.value ?? ""}: ${lowerFirst(found)} Labels come from Nexus: an unlinked package carries none, so a label filter hides it too.`;
});
const lookbookMode = computed(() => nav.riftMode.value === "lookbook");

const SORTS: { id: SortKey; label: string }[] = [
  { id: "best", label: "Best match" },
  { id: "name", label: "Name" },
  { id: "package", label: "Package" },
];

const scanLine = computed(() => {
  const s = stone.scanReading.value;
  if (!s) return "";
  const entries = library.value?.entries ?? 0;
  const now = s.current ? ` · now: ${s.current}` : "";
  return `Opened ${s.done} of ${s.total} packages · ${entries.toLocaleString("en-GB")} files indexed${now}`;
});

const nothingBody = computed(() => {
  const packages = library.value?.packages ?? 0;
  const hidden = search.includeMenuOnly.value ? "" : " Menu-only entries are hidden.";
  return `No option name, menu, description or file path in ${plural(packages, "package")} contains that.${hidden}`;
});

async function focusCard(index: number): Promise<void> {
  search.activeIndex.value = index;
  await nextTick();
  list.value?.querySelector<HTMLElement>(`[data-index="${index}"]`)?.focus();
}

function open(optionId: string, index: number): void {
  search.activeIndex.value = index;
  void world.open(optionId);
}

function zoom(optionId: string, name: string): void {
  nav.openLightbox({ optionId, name });
}

const LIST_KEYS: Partial<Record<string, (i: number, count: number) => void>> = {
  ArrowDown: (i, count) => void focusCard(Math.min(count - 1, i + 1)),
  ArrowUp: (i) => (i <= 0 ? document.getElementById("rift-search")?.focus() : void focusCard(i - 1)),
  Enter: (i) => {
    const hit = search.results.value.at(i);
    if (hit) open(hit.optionId, i);
  },
  " ": (i) => {
    const hit = search.results.value.at(i);
    if (hit?.hasScreenshot) zoom(hit.optionId, hit.name);
  },
  Escape: () => document.getElementById("rift-search")?.focus(),
};

function onListKey(event: KeyboardEvent): void {
  const count = search.results.value.length;
  const handler = LIST_KEYS[event.key];
  if (!count || !handler) return;
  if (event.key === "Escape" && world.selectedId.value) return;
  event.preventDefault();
  handler(Math.max(0, search.activeIndex.value), count);
}

function crumb(packageId: string, name: string): void {
  nav.openPackage(packageId, name);
}

function onChosen(): void {
  worldFolded.value = false;
}

watch(lookbookMode, (on) => {
  worldFolded.value = on;
});

/** The lookbook takes the keyboard once its package has actually arrived:
 *  pressing L on a package not yet loaded must not leave focus on <body>. */
const lookbookReady = computed(() =>
  lookbookMode.value && pkg.detail.value?.package.id === nav.packageId.value ? nav.packageId.value : null,
);
watch(lookbookReady, async (id) => {
  if (!id) return;
  await nextTick();
  await lookbook.value?.focusFirst();
});

/** At the narrow posture Your World is an overlay over the Rift: a door that
 *  moves the Rift to a package (Open in package, a crumb) closes it, or the
 *  package would open behind the scrim. */
const NARROW = "(max-width: 1199px)";
watch([() => nav.riftMode.value, () => nav.packageId.value], ([mode]) => {
  if (mode === "package" && world.selectedId.value && window.matchMedia(NARROW).matches) world.close();
});

defineExpose({ focusCard });
</script>

<template>
  <div class="room" :class="{ 'is-lookbook': lookbookMode, 'has-overlay': world.selectedId.value && !lookbookMode }">
    <main class="rift" aria-label="The Rift">
      <!-- results header, 48 px, sticky -->
      <header class="rift__head">
        <template v-if="nav.riftMode.value === 'results'">
          <p class="rift__echo" aria-live="polite">
            <template v-if="query"
              ><b>{{ search.results.value.length }}</b> answered “{{ query }}”<span
                v-if="search.labelReading.value"
                class="rift__dim"
              >
                · {{ search.labelReading.value }}</span
              ></template
            >
            <!-- the space before each "·" lives inside its span: whitespace between
                 two elements that carries a newline is dropped by Vue's compiler -->
            <template v-else-if="search.labelReading.value"
              ><b>{{ plural(search.results.value.length, "option") }}</b
              ><span class="rift__dim"> · {{ search.labelReading.value }}</span></template
            >
            <template v-else
              ><b>Library</b
              ><span v-if="library" class="rift__dim">
                · {{ plural(library.packages, "package") }} · {{ plural(library.options, "option") }}</span
              ></template
            >
          </p>
          <div class="rift__tools">
            <label class="rift__sort">
              <span class="rs-sr-only">Sort</span>
              <select v-model="search.sort.value">
                <option v-for="s in SORTS" :key="s.id" :value="s.id">{{ s.label }}</option>
              </select>
            </label>
            <label class="rift__group"><input v-model="search.grouped.value" type="checkbox" /> Group by package</label>
          </div>
        </template>
        <template v-else>
          <p class="rift__echo">
            <button type="button" class="rift__back" @click="nav.backToResults()">‹ Answers</button>
            <b>{{ pkg.detail.value?.package.displayName ?? "Package" }}</b>
            <span v-if="pkg.detail.value" class="rift__dim"
              >· {{ plural(pkg.detail.value.package.optionCount, "option") }} ·
              {{ formatBytes(pkg.detail.value.package.size) }}</span
            >
          </p>
          <div class="rift__tools">
            <button
              type="button"
              class="rift__mode"
              :class="{ 'is-on': nav.riftMode.value === 'package' }"
              @click="nav.packageId.value && nav.openPackage(nav.packageId.value)"
            >
              TREE
            </button>
            <button
              type="button"
              class="rift__mode"
              :class="{ 'is-on': lookbookMode }"
              title="Lookbook (L)"
              @click="nav.packageId.value && nav.openLookbook(nav.packageId.value)"
            >
              LOOKBOOK <kbd>L</kbd>
            </button>
          </div>
        </template>
      </header>

      <!-- the answers -->
      <div v-if="nav.riftMode.value === 'results'" class="rift__scroll">
        <RiftInscription
          v-if="stone.scanning.value"
          size="md"
          class="rift__scan"
          inscription="READING THE RIFT"
          :body="scanLine"
        >
          <!-- §9's Pause waits on a spine command (contract request): no dead button meanwhile. -->
        </RiftInscription>

        <RiftInscription
          v-if="empty"
          inscription="THE RIFT IS QUIET"
          body="No mods in your library roots yet. Add the folder where your downloads land, or adopt what Fluffy already has."
        >
          <RiftButton variant="primary" @click="preflight.addRoot()">Add library folder…</RiftButton>
          <RiftButton @click="nav.go('world')">Adopt Fluffy's library</RiftButton>
        </RiftInscription>

        <FailureNotice
          v-else-if="search.failure.value"
          size="lg"
          :voiced="search.failure.value"
          @recover="search.run()"
        />

        <RiftInscription v-else-if="labelsHidAll" inscription="THE LABELS HIDE EVERY ANSWER" :body="labelsBody">
          <RiftButton variant="primary" @click="search.clearLabels()">Clear labels</RiftButton>
          <RiftButton v-if="query" @click="search.clearText()">Clear search</RiftButton>
        </RiftInscription>

        <RiftInscription
          v-else-if="query && !search.results.value.length && search.loaded.value"
          :inscription="`NOTHING ANSWERS “${query}”`"
          :body="nothingBody"
        >
          <RiftButton v-if="!search.includeMenuOnly.value" variant="primary" @click="search.setMenuOnly(true)"
            >Include menu-only</RiftButton
          >
          <RiftButton v-if="anyRootOff" @click="search.allRoots()">Search all roots</RiftButton>
          <RiftButton @click="search.clearText()">Clear search</RiftButton>
        </RiftInscription>

        <div
          v-else
          id="rift-answers"
          ref="list"
          class="rift__list"
          role="listbox"
          :aria-label="query ? `Answers for ${query}` : 'Library'"
          @keydown="onListKey"
        >
          <template v-if="showTroubled">
            <TroubledCard v-for="p in stone.troubled.value" :key="p.id" :pkg="p" />
          </template>
          <template v-if="search.grouped.value">
            <section v-for="g in search.groups.value" :key="g.packageId" class="rift__groupbox">
              <header class="rift__pkg">
                <span class="rift__pkgname">{{ g.hits[0]?.packageName }}</span>
                <span class="rift__dim">{{ plural(g.hits.length, "match", "matches") }}</span>
                <button type="button" class="rift__open" @click="nav.openPackage(g.packageId)">Open package ▸</button>
              </header>
              <OptionCard
                v-for="hit in g.hits"
                :key="hit.optionId"
                :hit="hit"
                :index="search.results.value.indexOf(hit)"
                :tokens="search.tokens.value"
                :selected="world.selectedId.value === hit.optionId"
                :active="search.activeIndex.value === search.results.value.indexOf(hit)"
                :veiled="veils(settings.veil, adultnessOf(hit))"
                :root-label="rootLabels.get(hit.rootId) ?? ''"
                @open="open(hit.optionId, search.results.value.indexOf(hit))"
                @crumb="crumb"
                @zoom="zoom(hit.optionId, hit.name)"
              />
            </section>
          </template>
          <template v-else>
            <OptionCard
              v-for="(hit, i) in search.results.value"
              :key="hit.optionId"
              :hit="hit"
              :index="i"
              :tokens="search.tokens.value"
              :selected="world.selectedId.value === hit.optionId"
              :active="search.activeIndex.value === i || (search.activeIndex.value < 0 && i === 0)"
              :veiled="veils(settings.veil, adultnessOf(hit))"
              :root-label="rootLabels.get(hit.rootId) ?? ''"
              @open="open(hit.optionId, i)"
              @crumb="crumb"
              @zoom="zoom(hit.optionId, hit.name)"
            />
          </template>
        </div>
      </div>

      <!-- one package -->
      <div v-else-if="nav.riftMode.value === 'package'" class="rift__scroll rift__pad">
        <p v-if="pkg.failure.value" class="rift__dim">{{ pkg.failure.value.voice }}</p>
        <template v-else-if="pkg.detail.value">
          <p class="rift__file rs-mono">
            {{ pkg.detail.value.package.fileName }} · {{ plural(pkg.detail.value.package.entryCount, "file") }} inside
          </p>
          <label class="rift__filter">
            <span class="rs-sr-only">Filter this package</span>
            <input v-model="pkg.filter.value" type="search" placeholder="Filter this package…" />
          </label>
          <PackageTree
            :nodes="pkg.tree.value"
            :focus-name="nav.focusNodeName.value"
            :selected-id="world.selectedId.value"
            @open="(id) => world.open(id)"
          />
        </template>
        <p v-else class="rift__dim rs-mono">Opening the package…</p>
      </div>

      <div v-else class="rift__scroll">
        <LookbookGrid ref="lookbook" @chosen="onChosen" />
      </div>
    </main>
    <YourWorldPane :folded="lookbookMode && worldFolded" @unfold="worldFolded = false" />
    <div v-if="world.selectedId.value && !lookbookMode" class="room__scrim" aria-hidden="true" @click="world.close()" />
  </div>
</template>

<style scoped>
.room {
  position: relative;
  flex: 1;
  min-width: 0;
  display: flex;
  height: 100%;
}
.rift {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  background: var(--rs-void);
}
.rift__head {
  flex: none;
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: var(--rs-space-3);
  height: 48px;
  padding: 0 var(--rs-space-6);
  border-bottom: 1px solid var(--rs-edge);
  color: var(--rs-muted);
}
.rift__echo {
  margin: 0;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.rift__echo b {
  color: var(--rs-text);
  font-weight: 500;
}
.rift__dim {
  color: var(--rs-faint);
}
.rift__tools {
  flex: none;
  display: flex;
  align-items: center;
  gap: var(--rs-space-3);
  color: var(--rs-faint);
  font: 12px var(--rs-font-mono);
}
.rift__sort select {
  height: 28px;
  padding: 0 6px;
  border: 1px solid var(--rs-edgehi);
  background: var(--rs-well);
  color: var(--rs-muted);
  font: 12px var(--rs-font-mono);
}
.rift__group {
  display: flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
}
.rift__group input {
  accent-color: var(--rs-rift);
}
.rift__back,
.rift__open {
  margin-right: var(--rs-space-2);
  padding: 0;
  border: 0;
  background: none;
  color: var(--rs-rift);
  font: inherit;
  cursor: pointer;
}
.rift__mode {
  height: 28px;
  padding: 0 10px;
  border: 1px solid var(--rs-edgehi);
  background: none;
  color: var(--rs-muted);
  font: 600 var(--rs-text-xs) var(--rs-font-mono);
  letter-spacing: 0.06em;
  cursor: pointer;
}
.rift__mode.is-on {
  border-color: var(--rs-rift);
  color: var(--rs-text);
  background: var(--rs-riftdeep);
}
.rift__mode kbd {
  margin-left: 4px;
  color: var(--rs-faint);
}
/* the Rift column: one scroll region (§8.0) */
.rift__scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
}
.rift__pad {
  padding: var(--rs-space-4) var(--rs-space-6) var(--rs-space-8);
}
.rift__scan {
  padding: var(--rs-space-4) var(--rs-space-6) 0;
}
.rift__list {
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-2);
  padding: var(--rs-space-4) var(--rs-space-6) var(--rs-space-12);
}
.rift__groupbox {
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-2);
}
.rift__pkg {
  display: flex;
  align-items: center;
  gap: var(--rs-space-3);
  height: 32px;
  margin-top: var(--rs-space-2);
  border-bottom: 1px solid var(--rs-edge);
}
.rift__pkgname {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--rs-text);
  font-weight: 500;
}
.rift__file {
  margin: 0 0 var(--rs-space-3);
  color: var(--rs-faint);
}
.rift__filter input {
  width: 100%;
  height: 36px;
  margin-bottom: var(--rs-space-3);
  padding: 0 10px;
  border: 1px solid var(--rs-edgehi);
  background: var(--rs-well);
  color: var(--rs-text);
  font: inherit;
}
.room__scrim {
  display: none;
}
@media (max-width: 1199px) {
  .room__scrim {
    display: block;
    position: absolute;
    inset: 0;
    z-index: calc(var(--rs-z-world-overlay) - 1);
    background: rgba(10, 14, 19, 0.45);
  }
}
</style>
