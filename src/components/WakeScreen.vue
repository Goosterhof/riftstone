<script setup lang="ts">
// §9 — the first frames, full window, only the pillar lit: the stone wakes,
// finds your world (or doesn't), and offers the first scan.
import { computed } from "vue";
import { useStone, type WakePhase } from "@/composables/useStone";
import { usePreflight } from "@/composables/usePreflight";
import { voice, voicedLine } from "@/services/voice";
import RiftButton from "./RiftButton.vue";
import RiftInscription from "./RiftInscription.vue";

const { phase } = defineProps<{ phase: Exclude<WakePhase, "ready"> }>();
const stone = useStone();
const preflight = usePreflight();

/** No world answered: the voice's own line, so the wake and a later failure
 *  speak with one narrator (components never compose failure copy). */
const noWorld = voice({ kind: "gameNotFound", detail: {} }, "game");

const foundLine = computed(() => {
  const o = stone.overview.value;
  const build = o?.game?.buildId ? ` · build ${o.game.buildId}` : "";
  const version = o?.reframework?.version?.replace(/^v/, "").replace(/\+.*$/, "");
  const ref = version ? ` · REFramework ${version}` : " · REFramework not found";
  return `Dragon's Dogma 2${build}${ref}`;
});
</script>

<template>
  <main class="wake" :data-phase="phase">
    <RiftInscription
      v-if="phase === 'waking'"
      inscription="THE STONE WAKES"
      body="Looking for Dragon's Dogma 2 in your Steam libraries…"
    >
      <RiftButton @click="preflight.pickGameFolder()">Choose folder manually</RiftButton>
    </RiftInscription>
    <RiftInscription
      v-else-if="phase === 'nogame'"
      :inscription="noWorld.inscription ?? 'NO WORLD ANSWERS'"
      :body="voicedLine(noWorld)"
    >
      <RiftButton variant="primary" :busy="preflight.busy.value === 'game'" @click="preflight.pickGameFolder()"
        >Choose game folder…</RiftButton
      >
      <RiftButton @click="stone.refreshOverview()">Retry search</RiftButton>
    </RiftInscription>
    <RiftInscription v-else inscription="YOUR WORLD IS FOUND" tone="world" :body="foundLine">
      <RiftButton variant="primary" @click="stone.startScan()">Scan my mods</RiftButton>
    </RiftInscription>
  </main>
</template>

<style scoped>
.wake {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  padding-left: var(--rs-space-12);
  background: radial-gradient(ellipse 60% 70% at 0% 50%, rgba(143, 227, 242, 0.05), transparent 70%), var(--rs-void);
}
.wake[data-phase="found"] {
  background: radial-gradient(ellipse 60% 70% at 100% 60%, rgba(235, 196, 111, 0.06), transparent 70%), var(--rs-void);
}
.wake :deep(.rs-ins__carve) {
  font-size: var(--rs-text-2xl);
}
.wake[data-phase="found"] :deep(.rs-ins__carve) {
  color: var(--rs-world);
}
</style>
