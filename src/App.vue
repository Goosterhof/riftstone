<script setup lang="ts">
// THE RIFTSTONE — the window. Stone left, Rift centre, Your World right
// (Design System #00015, direction A "The Monolith — Rift-Lit Basalt").
// One shell: the pillar never moves; the rooms right of it change.
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { sleepRiftstone, wakeRiftstone } from "@/composables/boot";
import { useStone } from "@/composables/useStone";
import { useNavigation } from "@/composables/useNavigation";
import { useSearch } from "@/composables/useSearch";
import { useWorld } from "@/composables/useWorld";
import { useHotkeys } from "@/composables/useHotkeys";
import { useLibrary } from "@/composables/useLibrary";
import { useRooms } from "@/composables/useRooms";
import RiftstonePillar from "@/components/RiftstonePillar.vue";
import WakeScreen from "@/components/WakeScreen.vue";
import ToastShelf from "@/components/ToastShelf.vue";
import RiftLightbox from "@/components/RiftLightbox.vue";
import RiftPage from "@/pages/RiftPage.vue";
import InstalledPage from "@/pages/InstalledPage.vue";
import NexusPage from "@/pages/NexusPage.vue";
import PreflightPage from "@/pages/PreflightPage.vue";
import MusterPage from "@/pages/MusterPage.vue";

const stone = useStone();
const nav = useNavigation();
const search = useSearch();
const world = useWorld();
const riftPage = ref<InstanceType<typeof RiftPage> | null>(null);

/** #00015 R17.2: saves live under Steam, not in the game folder, so THE MUSTER
 *  answers while no world does. During THE STONE WAKES it stays dark with the rest. */
const musterAwake = computed(() => nav.page.value === "muster" && stone.phase.value === "nogame");

useHotkeys();

const library = useLibrary();

/** v0.7: the gated rooms (the investor's build only). They stay mounted once loaded,
 *  so THE WORKSHOP's terminal keeps its conversation while other rooms show (§20.12). */
const rooms = useRooms();
const inExtraRoom = computed(() => rooms.extra.value.some((r) => r.id === nav.page.value));

/** ↓ from the well: into the library tree when the Rift is idle (§16.1),
 *  else into the answers. */
async function descend(): Promise<void> {
  if (nav.page.value !== "rift") return;
  if (search.idle.value && nav.riftMode.value === "results") {
    await riftPage.value?.focusTree();
    return;
  }
  if (!search.results.value.length) return;
  await riftPage.value?.focusCard(Math.max(0, search.activeIndex.value));
}

/** Enter in the well opens the active answer. With the tree on screen there
 *  is no answer to open: an empty well never opens an option nobody sees. */
function submit(): void {
  if (search.idle.value) return;
  const hit = search.results.value.at(Math.max(0, search.activeIndex.value));
  if (!hit || hit.menuOnly) return;
  if (library.reveal(hit.optionId)) library.focusedKey.value = hit.optionId;
  void world.open(hit.optionId);
}

onMounted(() => void wakeRiftstone());
onBeforeUnmount(() => sleepRiftstone());
</script>

<template>
  <div class="riftstone">
    <RiftstonePillar @descend="descend" @submit="submit" />
    <MusterPage v-if="musterAwake" />
    <WakeScreen v-else-if="stone.phase.value !== 'ready'" :phase="stone.phase.value" />
    <template v-else>
      <RiftPage v-if="nav.page.value === 'rift'" ref="riftPage" />
      <InstalledPage v-else-if="nav.page.value === 'world'" />
      <NexusPage v-else-if="nav.page.value === 'nexus'" />
      <MusterPage v-else-if="nav.page.value === 'muster'" />
      <PreflightPage v-else-if="!inExtraRoom" />
    </template>
    <component
      :is="r.page"
      v-for="r in rooms.extra.value"
      v-show="stone.phase.value === 'ready' && !musterAwake && nav.page.value === r.id"
      :key="r.id"
    />
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
