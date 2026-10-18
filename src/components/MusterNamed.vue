<script setup lang="ts">
// A thing the game names (#00015 R17.21): its own name, else its kind and its
// number in mono; with the game's names read, a quiet note that the game's
// text simply has none for it. A swapped thing carries THE RE-CARVING's `•`
// and its accessible "was …" (R17.25 step 4), never a title.
import { computed } from "vue";
import type { Named } from "@/types/rift";
import { UNNAMED_NOTE, UNNAMED_TITLE, namedText, type NamedKind } from "@/services/muster";
import { wasWords, type Mark } from "@/services/recarving";

const {
  named,
  kind,
  names,
  mark = undefined,
  lit = false,
  titled = false,
} = defineProps<{
  named: Named;
  kind: NamedKind;
  names: "game" | "missing";
  mark?: Mark;
  lit?: boolean;
  /** A game name may sit in a title when its cell ellipsizes (R17.11); a person's never. */
  titled?: boolean;
}>();

const nameless = computed(() => named.name === null);
const was = computed(() => wasWords(mark));
</script>

<template>
  <span class="named" :class="{ 'is-nameless': nameless }">
    <span v-if="was" class="named__dot" aria-hidden="true">•</span>
    <span
      class="named__text rc"
      :class="{ 'is-lit': lit && was, 'rs-mono': nameless }"
      :title="titled && !nameless ? (named.name ?? undefined) : undefined"
      >{{ namedText(named, kind) }}</span
    >
    <span v-if="nameless && names === 'game'" class="named__note" :title="UNNAMED_TITLE"
      >{{ UNNAMED_NOTE }}<span class="rs-sr-only">. {{ UNNAMED_TITLE }}</span></span
    >
    <span v-if="was" class="rs-sr-only">, {{ was }}</span>
  </span>
</template>

<style scoped>
.named {
  display: inline-flex;
  align-items: baseline;
  gap: 6px;
  min-width: 0;
  max-width: 100%;
}
.named__text {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.is-nameless .named__text {
  color: var(--rs-muted);
  font-size: var(--rs-text-xs);
}
.named__note {
  flex: none;
  color: var(--rs-faint);
  white-space: nowrap;
}
.named__dot {
  flex: none;
  color: var(--temp);
}
</style>
