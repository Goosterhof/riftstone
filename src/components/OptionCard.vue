<script setup lang="ts">
// §7.3 — a creation waiting in the Rift. The card shows WHY it answered:
// name hits light the title, trail hits light the crumb, a path hit adds the
// match line (the archive path, the hit lit), a description hit quotes it.
// When ONLY file paths matched, the reason comes first: the match line sits
// above the title (§13's collision). The 4 px rail is the card's temperature —
// cyan in the Rift, gold in your world, ember in conflict — and THE CROSSING
// (§11.2) is that rail sweeping gold from the bottom up.
import { computed } from "vue";
import type { MatchedField, SearchResult } from "@/types/rift";
import { chipsFor, railFor } from "@/services/chips";
import { carriesToken, containsAnyToken, SHORT_TOKEN } from "@/services/highlight";
import { kindsPresent } from "@/services/format";
import HitText from "./HitText.vue";
import KindBadge from "./KindBadge.vue";
import MenuTrail from "./MenuTrail.vue";
import PathProof from "./PathProof.vue";
import RiftThumb from "./RiftThumb.vue";
import StateChip from "./StateChip.vue";

const {
  hit,
  tokens,
  index,
  selected = false,
  active = false,
  veiled = false,
} = defineProps<{
  hit: SearchResult;
  tokens: string[];
  index: number;
  selected?: boolean;
  active?: boolean;
  veiled?: boolean;
}>();

const emit = defineEmits<{
  open: [optionId: string];
  crumb: [packageId: string, name: string];
  zoom: [optionId: string];
}>();

const rail = computed(() => railFor(hit));
const chips = computed(() => chipsFor(hit));
const kinds = computed(() => kindsPresent(hit.kinds));
const hasQuery = computed(() => tokens.length > 0);

/** The reason line for a field that is not otherwise visible on the card. */
const OTHER_FIELDS: { field: MatchedField; label: string; value: (h: SearchResult) => string | null }[] = [
  { field: "author", label: "author", value: (h) => h.author },
  { field: "category", label: "category", value: (h) => h.category },
  { field: "bundle", label: "bundle", value: (h) => h.bundle },
  { field: "packageName", label: "package", value: (h) => h.packageName },
  { field: "version", label: "version", value: (h) => h.version },
];

/** Tokens the path line may prove: long enough to mean something (a lone
 *  "1" lights noise inside every hash) and not already lit on the title or
 *  the trail, where the card already shows them. */
const proofTokens = computed(() => {
  const visible = [hit.name, ...hit.trail].join(" / ");
  return tokens.filter((t) => t.length >= SHORT_TOKEN && !carriesToken(visible, t));
});

const pathLine = computed(() => {
  if (!hasQuery.value || !hit.fileMatches.length) return null;
  const carrying = hit.fileMatches.filter((p) => proofTokens.value.some((t) => carriesToken(p, t)));
  // A card that answered on file paths alone always shows its reason, even
  // a weak one: hiding it would leave the card unexplained.
  const fileOnly = hit.matched.every((m) => m === "file");
  let paths: string[] = [];
  if (carrying.length) paths = carrying;
  else if (fileOnly) paths = hit.fileMatches;
  const first = paths.at(0);
  if (!first) return null;
  const more = paths.length - 1;
  const capped = hit.fileMatches.length >= 5 ? "\n…and possibly more" : "";
  return { path: first, more: more > 0 ? `+${more}` : null, all: paths.join("\n") + capped };
});

const quoteLine = computed(() => {
  if (!hasQuery.value || pathLine.value || !hit.matched.includes("description")) return null;
  return containsAnyToken(hit.descriptionSnippet, tokens) ? hit.descriptionSnippet : null;
});

/** v0.2: a nexusLabel hit on a tag says which tag (the category is already lit
 *  on its own chip, so a category hit needs no line of its own). */
const tagHit = computed(() => {
  if (!hasQuery.value || !hit.matched.includes("nexusLabel")) return null;
  if (containsAnyToken(hit.nexusCategory, tokens)) return null;
  return hit.nexusTags.find((t) => containsAnyToken(t, tokens)) ?? null;
});

const fieldLine = computed(() => {
  if (!hasQuery.value || pathLine.value || quoteLine.value) return null;
  if (tagHit.value) return { label: "nexus tag", value: tagHit.value };
  for (const f of OTHER_FIELDS) {
    const value = f.value(hit);
    if (hit.matched.includes(f.field) && value && containsAnyToken(value, tokens)) return { label: f.label, value };
  }
  return null;
});

const MATCHED_WORD: Partial<Record<MatchedField, string>> = { file: "file path", nexusLabel: "Nexus label" };

const reasonFirst = computed(() => pathLine.value !== null && hit.matched.every((m) => m === "file"));

const metaLine = computed(() => {
  const bits: string[] = [];
  if (hit.version) bits.push(`v${hit.version.replace(/^v/i, "")}`);
  if (hit.author) bits.push(`by ${hit.author}`);
  return bits.join(" · ");
});

const matchedLabel = computed(() =>
  hit.matched.length ? `Matched in ${hit.matched.map((m) => MATCHED_WORD[m] ?? m).join(", ")}` : "",
);
</script>

<template>
  <article
    v-if="hit.menuOnly"
    class="card card--menu"
    role="option"
    :aria-selected="selected"
    :tabindex="active ? 0 : -1"
    :data-index="index"
    :data-option="hit.optionId"
    @click="emit('crumb', hit.packageId, hit.name)"
  >
    <MenuTrail :trail="[...hit.trail, hit.name]" :tokens="tokens" :interactive="false" />
    <StateChip state="menu" />
  </article>
  <article
    v-else
    class="card"
    :class="[
      `rail-${rail}`,
      { 'is-selected': selected, 'is-active': active, 'is-veiled': veiled, 'is-installed': rail === 'world' },
    ]"
    role="option"
    :aria-selected="selected"
    :aria-label="`${hit.name}. ${matchedLabel}`"
    :tabindex="active ? 0 : -1"
    :data-index="index"
    :data-option="hit.optionId"
    @click="emit('open', hit.optionId)"
  >
    <RiftThumb
      class="card__thumb"
      :option-id="hit.optionId"
      :has-screenshot="hit.hasScreenshot"
      :alt="`Screenshot of ${hit.name}`"
      @click.stop="hit.hasScreenshot ? emit('zoom', hit.optionId) : emit('open', hit.optionId)"
    />
    <div class="card__body">
      <p v-if="reasonFirst && pathLine" class="card__match card__match--path is-first" :title="pathLine.all">
        <span class="card__why">in file</span><PathProof :path="pathLine.path" :tokens="tokens" /><span
          v-if="pathLine.more"
          class="card__more"
          >{{ pathLine.more }}</span
        >
      </p>
      <div class="card__top">
        <h3 class="card__title"><HitText :text="hit.name" :tokens="hasQuery ? tokens : []" /></h3>
        <div class="card__chips">
          <StateChip v-for="c in chips" :key="c" :state="c" />
        </div>
      </div>
      <MenuTrail
        v-if="hit.trail.length"
        class="card__trail"
        :trail="hit.trail"
        :tokens="hasQuery ? tokens : []"
        :tabbable="false"
        @crumb="(name) => emit('crumb', hit.packageId, name)"
      />
      <div class="card__meta">
        <KindBadge v-for="k in kinds" :key="k" :kind="k" />
        <span v-if="hit.nexusCategory" class="card__cat" :title="`Nexus category: ${hit.nexusCategory}`"
          ><HitText :text="hit.nexusCategory" :tokens="hasQuery ? tokens : []"
        /></span>
        <span class="card__metatext">{{ metaLine }}</span>
      </div>
      <p v-if="pathLine && !reasonFirst" class="card__match card__match--path" :title="pathLine.all">
        <span class="card__glyph" aria-hidden="true">⌕</span><PathProof :path="pathLine.path" :tokens="tokens" /><span
          v-if="pathLine.more"
          class="card__more"
          >{{ pathLine.more }}</span
        >
      </p>
      <p v-else-if="quoteLine" class="card__match card__match--quote">
        “<HitText :text="quoteLine" :tokens="tokens" />”
      </p>
      <p v-else-if="fieldLine" class="card__match">
        <span class="card__why">{{ fieldLine.label }}</span
        ><HitText :text="fieldLine.value" :tokens="tokens" />
      </p>
    </div>
  </article>
</template>

<style scoped>
.card {
  --rail-world: 0%;
  position: relative;
  display: flex;
  gap: var(--rs-space-4);
  padding: var(--rs-space-4) var(--rs-space-4) var(--rs-space-4) var(--rs-space-5);
  background: var(--rs-mist);
  border: 1px solid transparent;
  cursor: pointer;
  transition:
    background-color var(--rs-dur-quick),
    border-color var(--rs-dur-quick);
}
.card:hover {
  background: var(--rs-misthi);
}
.card.is-selected,
.card:focus-visible {
  background: var(--rs-misthi);
  border-color: var(--rs-edgehi);
}
.card.is-selected {
  outline: 2px solid var(--rs-rift);
  outline-offset: 2px;
}
.card:focus-visible {
  outline: 2px solid var(--rs-rift);
  outline-offset: 2px;
}
/* The rail: a two-colour column twice the card's height. Cyan shows by
 * default; installed slides it so gold fills from the bottom up — THE
 * CROSSING, 420 ms. Clamped, it lands solid gold. Ember wins over both. */
.card::before {
  content: "";
  position: absolute;
  left: -1px;
  top: -1px;
  bottom: -1px;
  width: 4px;
  background: linear-gradient(to bottom, var(--rs-rift) 50%, var(--rs-world) 50%);
  background-size: 100% 200%;
  background-position: 0 0;
  transition: background-position var(--rs-dur-crossing) var(--rs-ease-carve);
}
.card.rail-world::before {
  background-position: 0 100%;
}
.card.rail-ember::before {
  background: var(--rs-ember);
}
.card__thumb {
  flex: none;
  width: 112px;
  height: 84px;
}
.card__body {
  flex: 1;
  min-width: 0;
}
.card__top {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: var(--rs-space-2);
}
.card__title {
  margin: 0;
  min-width: 0;
  color: var(--rs-text);
  font: 700 var(--rs-text-md) / 1.25 var(--rs-font-sans);
  overflow-wrap: anywhere;
}
.card__chips {
  flex: none;
  display: flex;
  gap: 4px;
  padding-top: 2px;
}
.card__trail {
  /* The trail tucks under the title (identity) and the meta line stands off
     (facts). On a thumb-height card that leaves the card's centre — where a
     pointer aimed at "the card" lands — on inert ground between the two
     rows, not on a crumb that leaves for the package view (§7.4). */
  line-height: 1.2;
}
.card__meta {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 10px;
  min-width: 0;
  color: var(--rs-faint);
  font-size: var(--rs-text-sm);
}
/* v0.2 — the Nexus category, quiet: a hairline chip with a rift-cyan notch
   on its left edge (it came from the far world), never louder than the
   kind badges beside it */
.card__cat {
  flex: none;
  max-width: 132px;
  overflow: hidden;
  padding: 1px 6px 1px 8px;
  border: 1px solid var(--rs-edge);
  border-left: 2px solid rgba(143, 227, 242, 0.55);
  border-radius: var(--rs-radius-chip);
  color: var(--rs-muted);
  font: 400 var(--rs-text-xs) / 1.3 var(--rs-font-mono);
  text-overflow: ellipsis;
  white-space: nowrap;
}
.card__metatext {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.card__match {
  margin: 6px 0 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--rs-muted);
  font: 400 var(--rs-text-xs) / 1.4 var(--rs-font-mono);
}
/* The path line is a flex row: label and "+N more" hold their width, the
   path (PathProof) gives way around its lit fragment instead of being cut
   at the end, where the match usually is. */
.card__match--path {
  display: flex;
  align-items: baseline;
  min-width: 0;
}
.card__match--path > .card__why,
.card__match--path > .card__glyph,
.card__match--path > .card__more {
  flex: none;
}
.card__match.is-first {
  margin: 0 0 4px;
}
.card__match--quote {
  font-family: var(--rs-font-sans);
  font-size: var(--rs-text-sm);
  font-style: italic;
  white-space: normal;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
}
.card__glyph {
  margin-right: 6px;
  color: var(--rs-rift);
}
.card__why {
  margin-right: 6px;
  padding: 0 4px;
  border: 1px solid var(--rs-edgehi);
  border-radius: var(--rs-radius-chip);
  color: var(--rs-faint);
  font: 600 var(--rs-text-xs) var(--rs-font-mono);
  letter-spacing: 0.06em;
  text-transform: uppercase;
}
.card__more {
  margin-left: 8px;
  color: var(--rs-faint);
}
.card--menu {
  align-items: center;
  justify-content: space-between;
  min-height: 36px;
  padding: 6px var(--rs-space-4) 6px var(--rs-space-5);
  background: none;
  color: var(--rs-faint);
}
.card--menu::before {
  background: var(--rs-edge);
}
.card--menu :deep(.rs-trail) {
  color: var(--rs-faint);
}
</style>
