<script setup lang="ts">
// THE RIFTSTONE — the window. Stone left, Rift centre, Your World right
// (Design System #00015, direction A "The Monolith — Rift-Lit Basalt").
// One shell: the pillar never moves; the rooms right of it change.
import { onBeforeUnmount, onMounted, ref } from "vue";
import { sleepRiftstone, wakeRiftstone } from "@/composables/boot";
import { useStone } from "@/composables/useStone";
import { useNavigation } from "@/composables/useNavigation";
import { useSearch } from "@/composables/useSearch";
import { useWorld } from "@/composables/useWorld";
import { useHotkeys } from "@/composables/useHotkeys";
import RiftstonePillar from "@/components/RiftstonePillar.vue";
import WakeScreen from "@/components/WakeScreen.vue";
import ToastShelf from "@/components/ToastShelf.vue";
import RiftLightbox from "@/components/RiftLightbox.vue";
import RiftPage from "@/pages/RiftPage.vue";
import InstalledPage from "@/pages/InstalledPage.vue";
import NexusPage from "@/pages/NexusPage.vue";
import PreflightPage from "@/pages/PreflightPage.vue";

const stone = useStone();
const nav = useNavigation();
const search = useSearch();
const world = useWorld();
const riftPage = ref<InstanceType<typeof RiftPage> | null>(null);

useHotkeys();

async function descend(): Promise<void> {
  if (nav.page.value !== "rift" || !search.results.value.length) return;
  await riftPage.value?.focusCard(Math.max(0, search.activeIndex.value));
}

function submit(): void {
  const hit = search.results.value.at(Math.max(0, search.activeIndex.value));
  if (hit && !hit.menuOnly) void world.open(hit.optionId);
}

onMounted(() => void wakeRiftstone());
onBeforeUnmount(() => sleepRiftstone());
</script>

<template>
  <div class="riftstone">
    <RiftstonePillar @descend="descend" @submit="submit" />
    <WakeScreen v-if="stone.phase.value !== 'ready'" :phase="stone.phase.value" />
    <template v-else>
      <RiftPage v-if="nav.page.value === 'rift'" ref="riftPage" />
      <InstalledPage v-else-if="nav.page.value === 'world'" />
      <NexusPage v-else-if="nav.page.value === 'nexus'" />
      <PreflightPage v-else />
    </template>
    <ToastShelf />
    <RiftLightbox />
  </div>
</template>

<style scoped>
.riftstone {
  display: flex;
  width: 100%;
  height: 100%;
  overflow: hidden;
  background: var(--rs-void);
}
</style>
