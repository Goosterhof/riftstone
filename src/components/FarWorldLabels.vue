<script setup lang="ts">
// v0.2 — the NEXUS LABELS facets, carved into the stone below STATE. What the
// far world calls each linked mod: its CATEGORY (one per mod, so the chips
// widen: any of) and its TAGS (each one narrows: all of). Every chip carries
// the count of answers pressing it would show NOW (useSearch's predictions:
// the label-free answers counted in the window), dimmed at 0 so a dead end
// is visible before the press, and a VEIN, a rift-light hairline as long as
// that label's share of the whole library, so the stone shows the library's
// shape. A live reading line says the logic in words ("Armour or Visuals,
// carrying Fashion and Retexture"). Facets never fire the seam (§11.1).
import { computed, nextTick, ref } from "vue";
import type { LabelCount } from "@/types/rift";
import { useLabels } from "@/composables/useLabels";
import { useSearch } from "@/composables/useSearch";
import { useFarWorld } from "@/composables/useFarWorld";
import { useNavigation } from "@/composables/useNavigation";
import { plural } from "@/services/format";
import { unlabeledLine } from "@/services/ledger";
import RiftButton from "./RiftButton.vue";

const labels = useLabels();
const search = useSearch();
const farWorld = useFarWorld();
const nav = useNavigation();

/** Tags past this many fold behind "+N more": a real library carries dozens. */
const TAG_FOLD = 10;
const allTags = ref(false);
const chipBox = ref<HTMLElement | null>(null);

const facets = computed(() => labels.facets.value);
const unlabeled = computed(() => facets.value?.unlabeledPackages ?? 0);
const busy = computed(() => labels.asking.value || farWorld.checking.value || farWorld.identifying.value);
/** The group exists once the stone has asked once and there is anything to say. */
const shown = computed(() => facets.value !== null && (labels.carved.value || unlabeled.value > 0));

const widest = computed(() =>
  Math.max(
    1,
    ...(facets.value?.categories ?? []).map((c) => c.options),
    ...(facets.value?.tags ?? []).map((t) => t.options),
  ),
);

const tags = computed(() => {
  const list = facets.value?.tags ?? [];
  if (allTags.value || list.length <= TAG_FOLD) return list;
  // chosen tags never fold away: what filters the answers stays in sight
  const head = list.slice(0, TAG_FOLD);
  const chosenTail = list.slice(TAG_FOLD).filter((t) => search.hasTag(t.name));
  return [...head, ...chosenTail];
});
const foldable = computed(() => (facets.value?.tags.length ?? 0) > TAG_FOLD);
const folded = computed(() => (facets.value?.tags.length ?? 0) - tags.value.length);

/** One toggle for more/fewer: the button stays, so focus stays on it. */
async function toggleFold(): Promise<void> {
  allTags.value = !allTags.value;
  await nextTick();
  if (allTags.value) chipBox.value?.querySelectorAll<HTMLButtonElement>(".lchip--tag")[TAG_FOLD]?.focus();
}

const vein = (c: LabelCount) => `${Math.max(6, Math.round((c.options / widest.value) * 100))}%`;

/** The count a chip shows: what pressing it gives now, else the library total. */
const catCount = (c: LabelCount) => search.categoryPrediction(c.name) ?? c.options;
const tagCount = (t: LabelCount) => search.tagPrediction(t.name) ?? t.options;

function chipLabel(c: LabelCount, now: number): string {
  const total = plural(c.options, "option");
  return now === c.options ? `${c.name}, ${total}` : `${c.name}, ${plural(now, "answer")} now, ${total} in the library`;
}

const progressLine = computed(() => {
  const p = farWorld.progress.value;
  if (!labels.asking.value || !p) return null;
  const verb = { linking: "linking", hashing: "hashing archives", checking: "asking Nexus" }[p.phase];
  return `${verb} · ${p.done} of ${p.total}`;
});
</script>

<template>
  <section v-if="shown" id="nexus-labels" class="flab" aria-label="Nexus labels">
    <h2 class="flab__head rs-inscription">NEXUS LABELS</h2>

    <template v-if="labels.carved.value">
      <div v-if="facets?.categories.length" class="flab__set">
        <p id="flab-cat" class="flab__sub">category <span class="flab__logic">· any of</span></p>
        <div class="flab__chips" role="group" aria-labelledby="flab-cat">
          <button
            v-for="c in facets.categories"
            :key="c.name"
            type="button"
            class="lchip"
            :class="{ 'is-on': search.hasCategory(c.name), 'is-dead': catCount(c) === 0 }"
            :aria-pressed="search.hasCategory(c.name)"
            :aria-label="chipLabel(c, catCount(c))"
            :title="c.name"
            :style="{ '--vein': vein(c) }"
            @click="search.toggleCategory(c.name)"
          >
            <span class="lchip__name">{{ c.name }}</span
            ><span class="lchip__n">{{ catCount(c) }}</span>
          </button>
        </div>
      </div>

      <div v-if="facets?.tags.length" class="flab__set">
        <p id="flab-tag" class="flab__sub">tags <span class="flab__logic">· all of</span></p>
        <div ref="chipBox" class="flab__chips" role="group" aria-labelledby="flab-tag">
          <button
            v-for="t in tags"
            :key="t.name"
            type="button"
            class="lchip lchip--tag"
            :class="{ 'is-on': search.hasTag(t.name), 'is-dead': tagCount(t) === 0 }"
            :aria-pressed="search.hasTag(t.name)"
            :aria-label="chipLabel(t, tagCount(t))"
            :title="t.name"
            :style="{ '--vein': vein(t) }"
            @click="search.toggleTag(t.name)"
          >
            <span class="lchip__name">{{ t.name }}</span
            ><span class="lchip__n">{{ tagCount(t) }}</span>
          </button>
          <button
            v-if="foldable"
            type="button"
            class="flab__more"
            :aria-expanded="allTags"
            :aria-label="allTags ? 'Show fewer tags' : `Show ${folded} more tags`"
            @click="toggleFold"
          >
            {{ allTags ? "fewer" : `+${folded} more` }}
          </button>
        </div>
      </div>

      <p v-if="search.labelReading.value" class="flab__reading" aria-live="polite">
        <span class="flab__readtext">{{ search.labelReading.value }}</span>
        <button type="button" class="flab__clear" @click="search.clearLabels()">Clear</button>
      </p>

      <p v-if="unlabeled > 0" class="flab__quiet">
        {{ unlabeledLine(unlabeled) }}
        <span class="flab__hint">A label filter hides them.</span>
      </p>
    </template>

    <p v-else class="flab__quiet">
      {{ unlabeledLine(unlabeled) }}
      <span class="flab__hint">The stone learns each mod's category and tags when it asks Nexus about updates.</span>
    </p>

    <p v-if="progressLine" class="flab__progress rs-mono" role="status">{{ progressLine }}</p>

    <template v-if="unlabeled > 0">
      <p v-if="labels.exhausted.value && !busy" class="flab__quiet flab__spent">
        Nexus knows none of the rest by their bytes.
        <button type="button" class="flab__clear" @click="nav.go('nexus')">Link them on the NEXUS page ▸</button>
      </p>
      <RiftButton
        v-else
        size="sm"
        class="flab__ask"
        :busy="busy"
        reason="The stone is already asking Nexus."
        @click="labels.askNexus()"
        >Identify &amp; check</RiftButton
      >
    </template>
  </section>
</template>

<style scoped>
.flab {
  margin-top: 6px;
}
.flab__head {
  margin: 14px 0 2px;
  font-size: 14px;
  color: var(--rs-muted);
}
.flab__set + .flab__set {
  margin-top: var(--rs-space-2);
}
/* the subtitle voice of "later wins": Alegreya 500, lowercase, inside the head */
.flab__sub {
  margin: 4px 0 6px;
  color: var(--rs-muted);
  font: 500 var(--rs-text-sm) var(--rs-font-sans);
}
.flab__logic {
  color: var(--rs-faint);
}
.flab__chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

/* a label chip: plain word + mono count, outlined in the stone's edge; its
   vein (::after) is the label's share of the library, in rift light */
.lchip {
  position: relative;
  display: inline-flex;
  align-items: baseline;
  gap: 6px;
  min-height: 28px;
  max-width: 100%;
  padding: 4px 8px 6px;
  border: 1px solid var(--rs-edgehi);
  border-radius: var(--rs-radius-chip);
  background: transparent;
  color: var(--rs-text);
  font: 500 var(--rs-text-sm) / 1.2 var(--rs-font-sans);
  text-align: left;
  cursor: pointer;
  transition:
    background-color var(--rs-dur-quick),
    border-color var(--rs-dur-quick);
}
.lchip::after {
  content: "";
  position: absolute;
  left: 0;
  bottom: 0;
  width: var(--vein);
  height: 2px;
  background: var(--rs-rift);
  opacity: 0.35;
  transition: opacity var(--rs-dur-quick);
}
.lchip:hover {
  background: rgba(143, 227, 242, 0.06);
  border-color: var(--rs-rift);
}
.lchip:hover::after {
  opacity: 0.6;
}
.lchip.is-on {
  background: var(--rs-riftdeep);
  border-color: var(--rs-rift);
}
.lchip.is-on::after {
  opacity: 1;
}
.lchip__name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.lchip__n {
  flex: none;
  color: var(--rs-faint);
  font: 400 var(--rs-text-xs) var(--rs-font-mono);
}
/* the hover tint lowers the count's ground: lift the count to muted so it
   stays above 4.5:1 anywhere on the pillar's gradient (§4.3) */
.lchip:hover .lchip__n {
  color: var(--rs-muted);
}
.lchip.is-on .lchip__n {
  color: var(--rs-rift);
}
/* pressing it would answer nothing right now: the name recedes, the 0 stays */
.lchip.is-dead:not(.is-on) .lchip__name {
  color: var(--rs-faint);
}
.lchip.is-dead:not(.is-on)::after {
  opacity: 0.15;
}
/* a tag narrows: its chosen chip carries a "+" stub, the mark of all-of */
.lchip--tag.is-on .lchip__name::before {
  content: "+ ";
  color: var(--rs-rift);
  font-family: var(--rs-font-mono);
}
.flab__more,
.flab__clear {
  min-height: 28px;
  padding: 0 6px;
  border: 0;
  background: none;
  color: var(--rs-muted);
  font: 400 var(--rs-text-xs) var(--rs-font-mono);
  text-decoration: underline;
  text-underline-offset: 3px;
  cursor: pointer;
}
.flab__more:hover,
.flab__clear:hover {
  color: var(--rs-text);
}
.flab__reading {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: var(--rs-space-2);
  margin: var(--rs-space-3) 0 0;
  padding-left: var(--rs-space-2);
  border-left: 2px solid var(--rs-rift);
}
.flab__readtext {
  min-width: 0;
  color: var(--rs-rift);
  font: 600 var(--rs-text-xs) / 1.5 var(--rs-font-mono);
}
.flab__quiet {
  margin: var(--rs-space-3) 0 0;
  color: var(--rs-muted);
  font-size: var(--rs-text-sm);
  line-height: 1.45;
}
.flab__hint {
  display: block;
  color: var(--rs-faint);
}
.flab__ask {
  margin-top: var(--rs-space-2);
}
.flab__progress {
  margin: var(--rs-space-2) 0 0;
  color: var(--rs-rift);
  font-size: var(--rs-text-xs);
}
.flab__spent .flab__clear {
  padding: 0;
}
.lchip:focus-visible,
.flab__more:focus-visible,
.flab__clear:focus-visible {
  outline: 2px solid var(--rs-rift);
  outline-offset: 2px;
}
</style>
