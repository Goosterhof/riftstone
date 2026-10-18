<script setup lang="ts">
// §7.8 — Your World: the warm pane right of the Rift. A hard temperature cut
// and one hairline separate it from the cold (§13: never a gradient between).
// Nothing selected → what is in your game; an option selected → its detail and
// THE CROSSING. Folds to a 56 px warm sliver in the lookbook (Bold Choice 6).
import { computed, onMounted, watch } from "vue";
import { useWorld } from "@/composables/useWorld";
import { useVersions } from "@/composables/useVersions";
import { useStone } from "@/composables/useStone";
import { useInstalled } from "@/composables/useInstalled";
import { useNavigation } from "@/composables/useNavigation";
import { useFarWorld } from "@/composables/useFarWorld";
import { usePreflight, REFRAMEWORK_URL } from "@/composables/usePreflight";
import { kindsSentence, plural } from "@/services/format";
import { recoverySentence } from "@/services/ledger";
import type { PayloadKinds } from "@/types/rift";
import OptionDetail from "./OptionDetail.vue";
import VersionsCard from "./VersionsCard.vue";
import RiftButton from "./RiftButton.vue";
import RiftInscription from "./RiftInscription.vue";

const { folded = false } = defineProps<{ folded?: boolean }>();
const emit = defineEmits<{ unfold: [] }>();

const world = useWorld();
const versions = useVersions();
const stone = useStone();
const installed = useInstalled();
const nav = useNavigation();
const farWorld = useFarWorld();
const preflight = usePreflight();

/** v0.5 (R18.4): THE VERSIONS card stands over what the pane held; closing it returns there. */
const versionsOpen = computed(() => versions.leadId.value !== null);
const selected = computed(() => world.selectedId.value !== null || versionsOpen.value);
const railWorld = computed(() => !versionsOpen.value && world.isInstalled.value);
// picking an option (or closing it) is a new thing for the pane: the card steps aside
watch(
  () => world.selectedId.value,
  () => versions.close(),
);

function closePane(): void {
  if (versionsOpen.value) versions.close();
  else world.close();
}

const totals = computed<PayloadKinds>(() => {
  const sum: PayloadKinds = { pak: 0, loose: 0, reframework: 0, root: 0 };
  for (const row of installed.view.value?.installs ?? []) {
    for (const k of ["pak", "loose", "reframework", "root"] as const) if (row.kinds[k] > 0) sum[k] += 1;
  }
  return sum;
});

const ref_ = computed(() => stone.overview.value?.reframework ?? null);
const game = computed(() => stone.overview.value?.game ?? null);
const mark = (on: boolean | null | undefined) => (on ? "✓" : "✗");

onMounted(() => {
  if (!installed.view.value) void installed.load();
});
</script>

<template>
  <button
    v-if="folded"
    type="button"
    class="world world--folded"
    aria-label="Unfold Your World"
    @click="emit('unfold')"
  >
    <span class="world__sliver rs-inscription">YOUR WORLD</span>
  </button>
  <section v-else class="world" :class="{ 'is-open': selected }" aria-label="Your World">
    <header class="world__head" :class="{ 'rail-world': railWorld, 'has-rail': selected }">
      <i v-if="world.crossingSeq.value > 0" :key="world.crossingSeq.value" class="world__warm" aria-hidden="true" />
      <h2 class="world__title rs-inscription">YOUR WORLD</h2>
      <button v-if="selected" type="button" class="world__esc" title="Close (Esc)" @click="closePane()">Esc</button>
    </header>

    <div class="world__body">
      <VersionsCard v-if="versionsOpen" />
      <template v-else-if="selected">
        <OptionDetail v-if="world.detail.value" :detail="world.detail.value" />
        <p v-else-if="world.detailFailure.value" class="world__line">{{ world.detailFailure.value.voice }}</p>
        <p v-else class="world__line rs-mono">Reading the option…</p>
      </template>

      <template v-else>
        <div v-if="stone.buildChange.value" class="world__banner">
          <RiftInscription
            size="md"
            tone="warn"
            inscription="YOUR WORLD HAS CHANGED"
            :body="`Dragon's Dogma 2 updated from build ${stone.buildChange.value.recorded} to ${stone.buildChange.value.now} since your mods were installed. Most still work; script mods and REFramework are the first to break.`"
          >
            <RiftButton size="sm" variant="primary" @click="preflight.openUrl(REFRAMEWORK_URL)"
              >Check REFramework</RiftButton
            >
            <RiftButton size="sm" @click="stone.dismissBuildChange()">I've tested it, dismiss</RiftButton>
          </RiftInscription>
        </div>
        <p v-if="stone.overview.value?.recovered" class="world__recovered">
          {{ recoverySentence(stone.overview.value.recovered) }}
        </p>

        <dl class="world__summary">
          <div>
            <dt>{{ plural(stone.overview.value?.library.installedOptions ?? 0, "mod") }} in your game</dt>
            <dd class="rs-mono">{{ kindsSentence(totals) || "nothing yet" }}</dd>
          </div>
          <div v-if="farWorld.updateCount.value">
            <dt class="is-update">⚠ {{ plural(farWorld.updateCount.value, "update") }} waiting</dt>
            <dd><button type="button" class="world__link" @click="nav.go('nexus')">NEXUS ▸</button></dd>
          </div>
          <div v-if="(stone.overview.value?.library.conflicts ?? 0) > 0">
            <dt class="is-ember">⚔ {{ plural(stone.overview.value?.library.conflicts ?? 0, "contested file") }}</dt>
            <dd><button type="button" class="world__link" @click="nav.go('world')">WHO WINS ▸</button></dd>
          </div>
          <div v-if="game">
            <dt>build {{ game.buildId ?? "unknown" }}</dt>
            <dd :class="game.buildChanged ? 'is-warn' : 'is-world'">{{ game.buildChanged ? "⚠ changed" : "✓" }}</dd>
          </div>
          <div v-if="ref_">
            <dt>
              REFramework {{ mark(ref_.installed) }} · PAK {{ mark(ref_.pakDirectory) }} · loose
              {{ mark(ref_.looseFileLoader) }}
            </dt>
            <dd>
              <button type="button" class="world__link" @click="nav.go('preflight')">PREFLIGHT ▸</button>
            </dd>
          </div>
        </dl>
        <p class="world__hint">
          Pick a creation in the Rift and it opens here: its files, where they land, and what they would overwrite.
        </p>
      </template>
    </div>
  </section>
</template>

<style scoped>
.world {
  position: relative;
  flex: none;
  display: flex;
  flex-direction: column;
  width: var(--rs-world-w);
  height: 100%;
  background: var(--rs-hearth);
  border-left: 1px solid var(--rs-edge);
  transition: width var(--rs-dur-settle) var(--rs-ease-carve);
}
/* Narrow posture: Your World arrives as an overlay from the right (§8.0). */
@media (max-width: 1199px) {
  .world:not(.world--folded) {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    z-index: var(--rs-z-world-overlay);
    box-shadow: -6px 0 0 rgba(0, 0, 0, 0.55);
  }
  .world:not(.is-open):not(.world--folded) {
    display: none;
  }
}
.world--folded {
  width: 56px;
  padding: var(--rs-space-5) 0;
  border: 0;
  border-left: 1px solid var(--rs-edge);
  cursor: pointer;
}
.world--folded:hover {
  background: var(--rs-hearthhi);
}
.world__sliver {
  display: block;
  margin: 0 auto;
  writing-mode: vertical-rl;
  letter-spacing: 0.2em;
  color: var(--rs-muted);
  font-size: 14px;
}
.world__head {
  position: relative;
  flex: none;
  display: flex;
  justify-content: space-between;
  align-items: center;
  height: 48px;
  padding: 0 var(--rs-space-6);
  border-bottom: 1px solid var(--rs-edge);
  background: var(--rs-hearth);
}
.world__head.has-rail::before {
  content: "";
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 4px;
  background: linear-gradient(to bottom, var(--rs-rift) 50%, var(--rs-world) 50%);
  background-size: 100% 200%;
  background-position: 0 0;
  transition: background-position var(--rs-dur-crossing) var(--rs-ease-carve);
}
.world__head.has-rail.rail-world::before {
  background-position: 0 100%;
}
/* THE CROSSING warms the header one value step, once (§11.2). Resting
 * frame: --rs-hearth, identical to the header's own ground. */
.world__warm {
  position: absolute;
  inset: 0;
  z-index: 0;
  background: var(--rs-hearth);
  animation: rs-hearth-warm var(--rs-dur-crossing) var(--rs-ease-carve) 1 forwards;
}
/* Reduced motion: rs-hearth-warm rests on the hearth the header already is. */
@media (prefers-reduced-motion: reduce) {
  .world__warm {
    display: none;
  }
}
.world__title,
.world__esc {
  position: relative;
  z-index: 1;
}
.world__title {
  margin: 0;
  font-size: 14px;
  color: var(--rs-muted);
  letter-spacing: 0.1em;
}
.world__esc {
  padding: 1px 6px;
  border: 1px solid var(--rs-edgehi);
  border-radius: var(--rs-radius-chip);
  background: none;
  color: var(--rs-muted);
  font: 600 var(--rs-text-xs) var(--rs-font-mono);
  cursor: pointer;
}
.world__body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: var(--rs-space-5) var(--rs-space-6) var(--rs-space-8);
}
.world__line {
  color: var(--rs-muted);
}
.world__banner {
  margin-bottom: var(--rs-space-4);
}
.world__recovered {
  margin: 0 0 var(--rs-space-4);
  padding: var(--rs-space-2) 10px;
  border-left: 3px solid var(--rs-warn);
  background: var(--rs-hearthhi);
  font-size: var(--rs-text-sm);
}
.world__summary {
  margin: 0;
}
.world__summary > div {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  gap: var(--rs-space-3);
  padding: 10px 0;
  border-bottom: 1px solid var(--rs-edge);
}
.world__summary dt {
  color: var(--rs-text);
}
.world__summary dd {
  margin: 0;
  color: var(--rs-muted);
  text-align: right;
}
.world__link {
  padding: 0;
  border: 0;
  background: none;
  color: var(--rs-muted);
  font: 600 var(--rs-text-xs) var(--rs-font-mono);
  letter-spacing: 0.08em;
  cursor: pointer;
}
.world__link:hover {
  color: var(--rs-text);
}
.world__hint {
  margin: var(--rs-space-6) 0 0;
  color: var(--rs-faint);
  line-height: 1.5;
}
.is-world {
  color: var(--rs-world);
}
.is-warn {
  color: var(--rs-warn);
}
.is-update {
  color: var(--rs-verdant) !important;
}
.is-ember {
  color: var(--rs-ember) !important;
}
</style>
