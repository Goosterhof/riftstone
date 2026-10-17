<script setup lang="ts">
// R16.19 — FIND ON NEXUS. A carved well, a Sort select and Search. The
// words go to Nexus only on Enter or Search, never as you type (the
// contract's privacy rule: nothing from the library, and nothing half-typed).
// The answers are the v0.2 candidate cards, drawn at once; "Show 20 more"
// appends the next page and disappears when Nexus has no more.
import { computed } from "vue";
import type { NexusBrowseSort, NexusCandidate } from "@/types/rift";
import { BROWSE_PAGE, useFarWorld } from "@/composables/useFarWorld";
import FailureNotice from "./FailureNotice.vue";
import FarModCard from "./FarModCard.vue";
import RiftButton from "./RiftButton.vue";
import StandingControls from "./StandingControls.vue";

const emit = defineEmits<{ files: [mod: NexusCandidate] }>();

const far = useFarWorld();

const SORTS: { value: NexusBrowseSort; label: string }[] = [
  { value: "endorsements", label: "Endorsements" },
  { value: "downloads", label: "Downloads" },
  { value: "updated", label: "Updated" },
  { value: "relevance", label: "Relevance" },
];
const hasText = computed(() => far.browseText.value.trim().length > 0);
const sorts = computed(() => SORTS.filter((s) => s.value !== "relevance" || hasText.value));

function onText(event: Event): void {
  far.browseText.value = (event.target as HTMLInputElement).value;
  if (!far.browseText.value.trim() && far.browseSort.value === "relevance") far.browseSort.value = "endorsements";
}

const totalLine = computed(() => {
  const b = far.browsed.value;
  if (!b) return null;
  const n = b.total.toLocaleString("en-GB");
  return b.total === 1 ? "1 mod on Nexus" : `${n} mods on Nexus`;
});
</script>

<template>
  <section id="far-find" class="far-sec browse" aria-labelledby="far-h-find">
    <h2 id="far-h-find" class="browse__h rs-inscription">FIND ON NEXUS</h2>
    <form class="browse__bar" role="search" @submit.prevent="far.browse()">
      <label class="browse__well">
        <span class="rs-sr-only">Search Nexus for Dragon's Dogma 2 mods</span>
        <input
          :value="far.browseText.value"
          type="search"
          placeholder="Search Nexus for Dragon's Dogma 2 mods"
          autocomplete="off"
          spellcheck="false"
          data-browse-field
          @input="onText"
        />
      </label>
      <label class="browse__sort">
        <span class="browse__sortk">Sort</span>
        <select v-model="far.browseSort.value" data-browse-sort>
          <option v-for="s in sorts" :key="s.value" :value="s.value">{{ s.label }}</option>
        </select>
      </label>
      <RiftButton size="sm" variant="primary" type="submit" :busy="far.browsing.value === 'new'">Search</RiftButton>
    </form>

    <FailureNotice v-if="far.browseFailure.value" :voiced="far.browseFailure.value" @recover="far.browse()" />

    <template v-if="far.browsed.value">
      <p class="browse__total" role="status">{{ totalLine }}</p>
      <ul class="browse__cards" aria-label="Mods on Nexus">
        <FarModCard v-for="m in far.browsed.value.mods" :key="m.modId" :mod="m" @files="emit('files', m)">
          <RiftButton size="sm" @click="emit('files', m)">Files</RiftButton>
          <StandingControls :mod-id="m.modId" :name="m.name" :endorse="false" />
        </FarModCard>
      </ul>
      <RiftButton
        v-if="far.browseHasMore.value"
        class="browse__more"
        :busy="far.browsing.value === 'more'"
        @click="far.browse(true)"
        >Show {{ BROWSE_PAGE }} more</RiftButton
      >
    </template>
  </section>
</template>

<style scoped>
.browse__h {
  margin: 0 0 var(--rs-space-2);
  font-size: 14px;
  color: var(--rs-muted);
  letter-spacing: 0.1em;
}
.browse__bar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--rs-space-2);
}
.browse__well {
  flex: 1;
  min-width: 240px;
}
.browse__well input {
  width: 100%;
  height: 36px;
  padding: 0 var(--rs-space-3);
  border: 1px solid var(--rs-edgehi);
  background: var(--rs-well);
  color: var(--rs-text);
  font: var(--rs-text-sm) var(--rs-font-sans);
}
.browse__well input:focus-visible,
.browse__sort select:focus-visible {
  outline: 2px solid var(--rs-rift);
  outline-offset: -2px;
}
.browse__sort {
  display: flex;
  align-items: center;
  gap: var(--rs-space-2);
  color: var(--rs-muted);
  font-size: var(--rs-text-sm);
}
.browse__sort select {
  height: 28px;
  padding: 0 var(--rs-space-2);
  border: 1px solid var(--rs-edgehi);
  background: var(--rs-well);
  color: var(--rs-text);
  font: var(--rs-text-sm) var(--rs-font-sans);
}
.browse__total {
  margin: var(--rs-space-3) 0 var(--rs-space-2);
  color: var(--rs-muted);
}
.browse__cards {
  margin: 0;
  padding: 0;
  list-style: none;
}
.browse__more {
  width: 100%;
  margin-top: var(--rs-space-2);
}
</style>
