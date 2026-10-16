<script setup lang="ts">
// §7.1 — the stone. Search, facets, nav and the game's state are carved into
// one basalt pillar (v0.2: the NEXUS labels join the facets, below STATE); its right edge is THE SEAM, where rift-light leaks out.
// THE SEAM ANSWERS (§11.1) lives here and nowhere else: the flare is a CSS
// animation keyed on each answered query, the rest states are PLACES (rest
// 18%, answered 60%, silent 8%) so the reduced-motion clamp lands on them.
import { computed, onMounted, ref } from "vue";
import { useSearch, type StateFacet } from "@/composables/useSearch";
import { useStone } from "@/composables/useStone";
import { useNavigation, type Page } from "@/composables/useNavigation";
import { useFarWorld } from "@/composables/useFarWorld";
import RiftSearchWell from "./RiftSearchWell.vue";
import FarWorldLabels from "./FarWorldLabels.vue";
import { useLabels } from "@/composables/useLabels";
import { seamReach } from "@/services/seam";

const search = useSearch();
const stone = useStone();
const nav = useNavigation();
const farWorld = useFarWorld();

const labels = useLabels();

const emit = defineEmits<{ descend: []; submit: [] }>();

/** Bring the NEXUS LABELS group into view and hand it the keyboard. */
function toLabels(): void {
  const group = document.getElementById("nexus-labels");
  const reduce = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
  group?.scrollIntoView?.({ block: "start", behavior: reduce ? "auto" : "smooth" });
  group?.querySelector<HTMLElement>(".lchip")?.focus({ preventScroll: true });
}

const well = ref<InstanceType<typeof RiftSearchWell> | null>(null);
const seamTop = ref(118);

const STATES: { id: StateFacet; label: string }[] = [
  { id: "all", label: "All" },
  { id: "notInstalled", label: "Not installed" },
  { id: "installed", label: "Installed" },
  { id: "conflict", label: "Conflict" },
  { id: "update", label: "Update" },
];

const NAV: { id: Page; label: string; key: string }[] = [
  { id: "rift", label: "THE RIFT", key: "1" },
  { id: "world", label: "YOUR WORLD", key: "2" },
  { id: "nexus", label: "NEXUS", key: "3" },
  { id: "preflight", label: "PREFLIGHT", key: "4" },
];

const seamState = computed(() => (stone.scanning.value ? "scanning" : search.seam.value));

/** §11.1 step 3 — the gauge: n of the library's N options answered, as a
 *  length. 0 whenever the seam is not answered (rest, silent, scanning). */
const reach = computed(() =>
  seamState.value === "answered"
    ? seamReach(search.results.value.length, stone.overview.value?.library.options ?? 0)
    : 0,
);

/** Until the stone is awake (no game, first launch), the facets and the rooms
 *  are carved but dark: only the search well and the foot speak (§9 wake). */
const awake = computed(() => stone.phase.value === "ready");

const carvedCount = computed(() => {
  if (stone.phase.value === "nogame") return "NO WORLD";
  if (search.settledText.value.trim()) return `${search.results.value.length.toLocaleString("en-GB")} ANSWERED`;
  const lib = stone.overview.value?.library;
  if (!lib) return "THE STONE WAKES";
  return `${lib.packages.toLocaleString("en-GB")} PACKAGES · ${lib.options.toLocaleString("en-GB")} OPTIONS`;
});

const ref_ = computed(() => stone.overview.value?.reframework ?? null);
const game = computed(() => stone.overview.value?.game ?? null);

const navCount = computed<Record<Page, string | null>>(() => ({
  rift: null,
  world: stone.overview.value?.game ? String(stone.overview.value.library.installedOptions) : null,
  nexus: farWorld.updateCount.value ? String(farWorld.updateCount.value) : null,
  preflight: null,
}));

function onType(value: string): void {
  if (nav.page.value !== "rift") nav.go("rift");
  if (nav.riftMode.value !== "results") nav.backToResults();
  search.setText(value);
}

function rootLabel(label: string, path: string): string {
  return label || path;
}

onMounted(() => {
  const el = well.value?.el;
  if (el?.parentElement) {
    const box = el.parentElement;
    seamTop.value = box.offsetTop + box.offsetHeight / 2;
  }
});

defineExpose({ focusSearch: () => well.value?.focus() });
</script>

<template>
  <aside
    class="pillar"
    :data-seam="seamState"
    :style="{
      '--seam-top': `${seamTop}px`,
      '--seam-answer': seamState === 'answered' ? 1 : 0,
      '--seam-reach': reach,
    }"
    aria-label="The Riftstone"
  >
    <header class="pillar__mark">
      <h1 class="pillar__word">RIFTSTONE</h1>
      <p class="pillar__sub">DRAGON'S DOGMA 2</p>
    </header>

    <RiftSearchWell
      ref="well"
      class="pillar__well"
      :model-value="search.text.value"
      @update:model-value="onType"
      @clear="search.clearText()"
      @descend="emit('descend')"
      @submit="emit('submit')"
    />
    <p class="pillar__count" :class="{ 'is-zero': search.seam.value === 'silent' }" aria-live="polite">
      {{ carvedCount }}
    </p>
    <!-- v0.2: at the narrow posture the NEXUS LABELS group sits below the
         facet block's fold; the chosen labels are carved here too, and a
         door brings the group into view. -->
    <p v-if="search.labelReading.value" class="pillar__labels rs-mono">
      <span class="pillar__labeltext">{{ search.labelReading.value }}</span>
    </p>
    <button v-else-if="labels.carved.value" type="button" class="pillar__tolabels" @click="toLabels">
      Nexus labels ↓
    </button>

    <div class="pillar__facets" :class="{ 'is-dark': !awake }" :inert="!awake">
      <h2 class="pillar__head rs-inscription">LIBRARY ROOTS</h2>
      <label v-for="root in stone.overview.value?.roots ?? []" :key="root.id" class="facet" :title="root.path">
        <input
          type="checkbox"
          :checked="!search.offRoots.value.includes(root.id)"
          @change="search.toggleRoot(root.id)"
        />
        <span class="facet__label">{{ rootLabel(root.label, root.path) }}</span>
        <span class="facet__n">{{ stone.rootCounts.value[root.id] ?? 0 }}</span>
      </label>

      <h2 class="pillar__head rs-inscription">STATE</h2>
      <div role="radiogroup" aria-label="State">
        <label v-for="s in STATES" :key="s.id" class="facet">
          <input
            type="radio"
            name="rift-state"
            :checked="search.stateFacet.value === s.id"
            @change="search.setStateFacet(s.id)"
          />
          <span class="facet__label">{{ s.label }}</span>
        </label>
      </div>
      <label class="facet facet--gap">
        <input
          type="checkbox"
          :checked="search.includeMenuOnly.value"
          @change="search.setMenuOnly(!search.includeMenuOnly.value)"
        />
        <span class="facet__label">Show menu-only</span>
      </label>

      <FarWorldLabels />
    </div>

    <nav class="pillar__nav" :class="{ 'is-dark': !awake }" :inert="!awake" aria-label="Rooms">
      <button
        v-for="n in NAV"
        :key="n.id"
        type="button"
        class="navlink"
        :class="{ 'is-on': nav.page.value === n.id }"
        :aria-current="nav.page.value === n.id ? 'page' : undefined"
        @click="nav.go(n.id)"
      >
        <span>{{ n.label }}</span>
        <span class="navlink__meta">
          <span v-if="navCount[n.id]" class="navlink__n" :class="{ 'is-update': n.id === 'nexus' }">{{
            navCount[n.id]
          }}</span>
          <kbd class="navlink__k">{{ n.key }}</kbd>
        </span>
      </button>
    </nav>

    <footer class="pillar__foot" aria-label="Game state">
      <span v-if="game">build {{ game.buildId ?? "?" }}</span>
      <span v-else class="is-warn">no game</span>
      <span v-if="game?.buildChanged" class="is-warn" title="The game updated since your mods were installed">⚠</span>
      <span v-if="ref_ && !ref_.installed" class="is-warn" title="REFramework is missing">REF ⚠</span>
      <span v-else-if="ref_" class="is-ok" :title="`REFramework ${ref_.version ?? ''}`">REF ✓</span>
      <span v-if="ref_?.pakDirectory === false" class="is-warn" title="The Alternate PAK Folder is off">PAK ✗</span>
      <span v-if="ref_?.looseFileLoader === false" class="is-warn" title="The Loose File Loader is off">LOOSE ✗</span>
    </footer>

    <i class="seam" aria-hidden="true" />
    <i class="seam-answered" aria-hidden="true" />
    <i
      v-if="seamState === 'answered' && search.answerSeq.value > 0"
      :key="search.answerSeq.value"
      class="seam-flare"
      aria-hidden="true"
    />
    <i v-if="seamState === 'scanning'" class="seam-scan" aria-hidden="true" />
  </aside>
</template>

<style scoped>
.pillar {
  position: relative;
  z-index: var(--rs-z-pillar);
  flex: none;
  display: flex;
  flex-direction: column;
  width: var(--rs-pillar-w);
  height: 100%;
  padding: var(--rs-space-5) var(--rs-space-5) 0;
  /* a faint chisel: two stops, nothing more (§7.1) */
  background: linear-gradient(#1f2226, #1b1e21);
  box-shadow: var(--rs-shadow-stone);
}
@media (max-width: 1199px) {
  .pillar {
    width: var(--rs-pillar-w-narrow);
    padding: 18px var(--rs-space-4) 0;
  }
}
.pillar__mark {
  flex: none;
}
.pillar__word {
  margin: 0;
  font: 700 30px / 1 var(--rs-font-inscription);
  letter-spacing: 0.1em;
  color: var(--rs-text);
  text-shadow:
    0 1px 0 rgba(255, 255, 255, 0.05),
    0 -1px 0 rgba(0, 0, 0, 0.7);
}
@media (max-width: 1199px) {
  .pillar__word {
    font-size: 26px;
  }
}
.pillar__sub {
  margin: 6px 0 0;
  color: var(--rs-faint);
  font: 400 12px var(--rs-font-mono);
  letter-spacing: 0.06em;
}
.pillar__well {
  flex: none;
  margin-top: 22px;
}
.pillar__count {
  flex: none;
  margin: 10px 0 0;
  min-height: 17px;
  color: var(--rs-rift);
  font: 600 12px var(--rs-font-mono);
  letter-spacing: 0.12em;
}
.pillar__count.is-zero {
  color: var(--rs-faint);
}
.pillar__labels {
  flex: none;
  margin: 4px 0 0;
  padding-left: var(--rs-space-2);
  border-left: 2px solid var(--rs-rift);
  color: var(--rs-rift);
  font-size: var(--rs-text-xs);
  line-height: 1.4;
}
.pillar__labeltext {
  display: -webkit-box;
  overflow: hidden;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}
/* only where the group falls below the fold: the short window */
.pillar__tolabels {
  display: none;
  flex: none;
  align-self: flex-start;
  min-height: 24px;
  margin: 2px 0 0;
  padding: 0;
  border: 0;
  background: none;
  color: var(--rs-muted);
  font: 400 var(--rs-text-xs) var(--rs-font-mono);
  text-decoration: underline;
  text-underline-offset: 3px;
  cursor: pointer;
}
.pillar__tolabels:focus-visible {
  outline: 2px solid var(--rs-rift);
  outline-offset: 2px;
}
@media (max-height: 900px) {
  .pillar__tolabels {
    display: block;
  }
}
/* the one internal scroll region of the stone (§8.0) */
.pillar__facets {
  flex: 1;
  min-height: 0;
  margin-top: var(--rs-space-3);
  overflow-y: auto;
  padding: 0 4px 8px 2px;
}
.pillar__head {
  margin: 14px 0 6px;
  font-size: 14px;
  color: var(--rs-muted);
}
.facet {
  display: flex;
  align-items: center;
  gap: var(--rs-space-2);
  min-height: 28px;
  color: var(--rs-text);
  cursor: pointer;
}
.facet--gap {
  margin-top: var(--rs-space-2);
}
.facet input {
  flex: none;
  width: 14px;
  height: 14px;
  accent-color: var(--rs-rift);
}
.facet__label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.facet__n {
  color: var(--rs-faint);
  font: 12px var(--rs-font-mono);
}
.pillar__facets.is-dark,
.pillar__nav.is-dark {
  opacity: 0.38;
}
.pillar__nav {
  flex: none;
  padding: 6px 0;
  border-top: 1px solid var(--rs-edge);
}
.navlink {
  display: flex;
  justify-content: space-between;
  align-items: center;
  width: 100%;
  height: 38px;
  padding: 0 2px 0 12px;
  border: 0;
  border-left: 3px solid transparent;
  background: none;
  color: var(--rs-muted);
  font: 600 14px var(--rs-font-inscription);
  letter-spacing: 0.08em;
  text-align: left;
  cursor: pointer;
  transition:
    color var(--rs-dur-quick),
    border-color var(--rs-dur-quick);
}
.navlink:hover {
  color: var(--rs-text);
}
.navlink.is-on {
  color: var(--rs-text);
  border-left-color: var(--rs-rift);
}
.navlink__meta {
  display: flex;
  align-items: center;
  gap: var(--rs-space-2);
}
.navlink__n {
  color: var(--rs-faint);
  font: 600 var(--rs-text-xs) var(--rs-font-mono);
}
.navlink__n.is-update {
  color: var(--rs-verdant);
}
.navlink__k {
  min-width: 18px;
  padding: 0 4px;
  border: 1px solid var(--rs-edgehi);
  border-radius: var(--rs-radius-chip);
  color: var(--rs-faint);
  font: var(--rs-text-xs) / 16px var(--rs-font-mono);
  text-align: center;
}
.pillar__foot {
  flex: none;
  display: flex;
  align-items: center;
  gap: 10px;
  height: 46px;
  border-top: 1px solid var(--rs-edge);
  color: var(--rs-faint);
  font: 12px var(--rs-font-mono);
  white-space: nowrap;
}
.is-ok {
  color: var(--rs-world);
}
.is-warn {
  color: var(--rs-warn);
}

/* ---- THE SEAM (§11) ---------------------------------------------------- */
.seam,
.seam-answered,
.seam-flare,
.seam-scan {
  position: absolute;
  right: 0;
  width: 2px;
  pointer-events: none;
}
.seam {
  top: 0;
  bottom: 0;
  background: var(--rs-rift);
  opacity: 0.18;
  transition: opacity var(--rs-dur-settle) var(--rs-ease-carve);
}
.pillar[data-seam="silent"] .seam {
  opacity: 0.08;
}
/* The answered rest holds at the gauge length (§11.1 step 3); a facet SNAPS
   it, so opacity is the only thing that ever transitions here. */
.seam-answered {
  top: var(--seam-top);
  height: calc((100% - var(--seam-top)) * var(--seam-reach));
  background: var(--rs-rift);
  opacity: calc(var(--seam-answer) * 0.6);
  transition: opacity var(--rs-dur-settle) var(--rs-ease-carve);
}
/* The flare's scaleY 0 → 1 grows to the gauge length, not to the foot. */
.seam-flare {
  top: var(--seam-top);
  height: calc((100% - var(--seam-top)) * var(--seam-reach));
  background: linear-gradient(var(--rs-rift), rgba(143, 227, 242, 0.85) 80%, rgba(143, 227, 242, 0));
  box-shadow: var(--rs-glow-rift);
  transform-origin: top;
  opacity: 0;
  animation: rs-seam-answer var(--rs-dur-answer) var(--rs-ease-answer) forwards;
}
.seam-scan {
  top: 60%;
  height: 25%;
  background: linear-gradient(rgba(143, 227, 242, 0), var(--rs-rift) 40%, var(--rs-rift) 60%, rgba(143, 227, 242, 0));
  animation: rs-seam-scan 1.6s linear infinite;
}
/* Reduced motion: the flare's resting frame is opacity 0, the same as no
   flare at all, so it is not drawn (and leaves no finished animation behind). */
@media (prefers-reduced-motion: reduce) {
  .seam-flare {
    display: none;
  }
}
</style>
