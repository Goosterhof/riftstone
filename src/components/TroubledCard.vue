<script setup lang="ts">
// §9 trouble cards — a card in the Rift with a rail and no picture: ember for
// a package that can't be opened, faint for one that opens with a warning
// (nothing to install, pre-TU 3.2 files, skipped files). The copy is the
// voice module's, worded for the problem that put the package on the list.
import { computed } from "vue";
import type { PackageSummary } from "@/types/rift";
import { useStone } from "@/composables/useStone";
import { usePreflight } from "@/composables/usePreflight";
import { troubleLine } from "@/services/voice";
import RiftButton from "./RiftButton.vue";

const { pkg } = defineProps<{ pkg: PackageSummary }>();
const stone = useStone();
const preflight = usePreflight();

const others = computed(() => Math.max(0, (stone.overview.value?.library.packages ?? 1) - 1));
const trouble = computed(() => troubleLine(pkg, others.value));
</script>

<template>
  <article class="trouble" :class="{ 'is-ember': trouble.ember }" role="note">
    <div class="trouble__body">
      <h3 class="trouble__name">{{ pkg.displayName }}</h3>
      <p class="trouble__line">{{ trouble.line }}</p>
      <div class="trouble__actions">
        <RiftButton size="sm" @click="preflight.reveal(pkg.path)">Show in folder</RiftButton>
        <RiftButton size="sm" @click="preflight.rescan()">Rescan this one</RiftButton>
        <RiftButton size="sm" @click="stone.hideTroubled(pkg.id)">Hide it</RiftButton>
      </div>
    </div>
  </article>
</template>

<style scoped>
.trouble {
  position: relative;
  padding: var(--rs-space-4) var(--rs-space-4) var(--rs-space-4) var(--rs-space-5);
  background: var(--rs-mist);
}
.trouble::before {
  content: "";
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 4px;
  background: var(--rs-faint);
}
.trouble.is-ember::before {
  background: var(--rs-ember);
}
.trouble__name {
  margin: 0;
  font: 700 var(--rs-text-md) / 1.25 var(--rs-font-sans);
  color: var(--rs-muted);
}
.trouble__line {
  margin: 4px 0 0;
  color: var(--rs-muted);
  line-height: 1.45;
}
.trouble.is-ember .trouble__line {
  color: var(--rs-text);
}
.trouble__actions {
  display: flex;
  gap: var(--rs-space-2);
  margin-top: var(--rs-space-3);
}
</style>
