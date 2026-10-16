<script setup lang="ts">
// The IN FILE line's path: the proof of why a card answered (Bold Choice 3).
// A card is narrower than most archive paths, and the match usually sits at
// the END of one (`…/mantle_310_f_alba.tex.251211553`), so plain end-ellipsis
// cut away exactly the proof. Here the path is four flex runs that give way
// in a fixed order — the elided directories first, then the tail, then the
// start of the matched segment (clipped at its START) — and the lit fragment
// never shrinks: at any card width the hit stays on screen, lit.
import { computed } from "vue";
import { pathProof } from "@/services/highlight";
import HitText from "./HitText.vue";

const { path, tokens } = defineProps<{ path: string; tokens: string[] }>();
const proof = computed(() => pathProof(path, tokens));
</script>

<template>
  <span class="proof" :aria-label="path"
    ><span v-if="proof.lead" class="proof__lead" aria-hidden="true">{{ proof.lead }}</span
    ><span v-if="proof.before" class="proof__before" aria-hidden="true"
      ><span class="proof__ltr">{{ proof.before }}</span></span
    ><span v-if="proof.fragment" class="proof__hit" aria-hidden="true"
      ><HitText :text="proof.fragment" :tokens="tokens" /></span
    ><span v-if="proof.after" class="proof__after" aria-hidden="true">{{ proof.after }}</span></span
  >
</template>

<style scoped>
.proof {
  display: inline-flex;
  flex: 0 1 auto;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
}
.proof__lead,
.proof__before,
.proof__after {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}
/* Shrink order: the elided directories give way first, then the tail, and
   last the start of the matched segment. */
.proof__lead {
  flex: 0 1000 auto;
  min-width: 2ch;
}
.proof__after {
  flex: 0 10 auto;
  min-width: 2ch;
}
/* Clipped at its START: "…_310_f_" keeps the characters that touch the hit. */
.proof__before {
  flex: 0 1 auto;
  direction: rtl;
}
.proof__ltr {
  direction: ltr;
  unicode-bidi: isolate;
}
.proof__hit {
  flex: none;
}
</style>
