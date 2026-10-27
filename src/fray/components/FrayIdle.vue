<script setup lang="ts">
// S5, the party rests: the eye is live and no fight has come yet. Names and
// live lintels, no digit in any card.
import type { FrayMember } from "@/types/fray";
import FrayCard from "./FrayCard.vue";

defineProps<{ places: (FrayMember | null)[] }>();
</script>

<template>
  <header class="head">
    <div class="headline"><span class="cz ins muted">THE FRAY</span><span class="watch">watching</span></div>
    <div class="rests">
      <div class="restsins">THE PARTY RESTS</div>
      <p class="restsp">No fight yet since the game started. The next one fills this screen.</p>
    </div>
  </header>
  <section class="cards" aria-label="The party">
    <FrayCard v-for="(m, slot) in places" :key="slot" :member="m" :fight="null" kind="idle" />
  </section>
</template>
