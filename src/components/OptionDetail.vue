<script setup lang="ts">
// §8.2 — the selected option in Your World: screenshot (click → lightbox),
// name, trail, by-line, description, the three doors (Install…, Open in
// package, Show in archive), conflicts, payload and skipped files. Install…
// opens THE CROSSING in place; the screenshot shrinks to a strip for it.
import { computed } from "vue";
import type { OptionDetail } from "@/types/rift";
import { useWorld } from "@/composables/useWorld";
import { useNavigation } from "@/composables/useNavigation";
import { useSearch } from "@/composables/useSearch";
import { usePreflight } from "@/composables/usePreflight";
import { useSettings } from "@/composables/useSettings";
import { kindsPresent, formatDate, plural } from "@/services/format";
import ConflictMap from "./ConflictMap.vue";
import InstallPlan from "./InstallPlan.vue";
import KindBadge from "./KindBadge.vue";
import MenuTrail from "./MenuTrail.vue";
import PayloadTree from "./PayloadTree.vue";
import RiftButton from "./RiftButton.vue";
import RiftThumb from "./RiftThumb.vue";

const { detail } = defineProps<{ detail: OptionDetail }>();

const world = useWorld();
const nav = useNavigation();
const search = useSearch();
const preflight = usePreflight();
const { settings } = useSettings();

const option = computed(() => detail.option);
const planning = computed(() => world.stage.value !== "detail");
const installedHere = computed(() => detail.installed !== null);

const byLine = computed(() => {
  const info = option.value.info;
  const bits: string[] = [];
  if (info?.author) bits.push(`by ${info.author}`);
  if (info?.version) bits.push(`v${info.version.replace(/^v/i, "")}`);
  if (info?.category) bits.push(info.category);
  if (detail.package.nexus) bits.push(`Nexus #${detail.package.nexus.modId}`);
  return bits.join(" · ");
});

const update = computed(() => (detail.update?.state.kind === "updateAvailable" ? detail.update.state.newer : null));

const conflictSentence = computed(() => {
  const own = option.value.id;
  const wins = detail.conflicts.filter((c) => c.winner.optionId === own);
  const losses = detail.conflicts.filter((c) => c.winner.optionId !== own);
  const parts: string[] = [];
  if (wins.length) parts.push(`overwrites ${plural(wins.length, "file")} of ${otherNames(wins)}`);
  if (losses.length) parts.push(`is overridden on ${plural(losses.length, "file")} by ${otherNames(losses)}`);
  return parts.join("; ");
});

function otherNames(list: OptionDetail["conflicts"]): string {
  const names = new Set<string>();
  for (const c of list) for (const l of c.layers) if (l.optionId !== option.value.id) names.add(l.name);
  return [...names].join(", ");
}

function zoom(): void {
  if (option.value.screenshot) nav.openLightbox({ optionId: option.value.id, name: option.value.name });
}

function crumb(name: string): void {
  nav.openPackage(detail.package.id, name);
}
</script>

<template>
  <article class="odetail" :class="{ 'is-veiled': settings.veilThumbnails }">
    <button
      type="button"
      class="odetail__shot"
      :class="{ 'is-strip': planning }"
      :disabled="!option.screenshot"
      :aria-label="option.screenshot ? `Open the screenshot of ${option.name}` : 'No screenshot'"
      @click="zoom"
    >
      <RiftThumb
        :option-id="option.id"
        :has-screenshot="option.screenshot !== null"
        size="full"
        :alt="`Screenshot of ${option.name}`"
      />
    </button>

    <h2 class="odetail__name">{{ option.name }}</h2>
    <MenuTrail :trail="option.trail" :tokens="search.tokens.value" @crumb="crumb" />
    <p class="odetail__by">
      {{ byLine }}
      <button v-if="update" type="button" class="odetail__upd" @click="nav.go('nexus')">
        · UPDATE v{{ update.version ?? "?" }} ▸
      </button>
    </p>
    <p v-if="installedHere && detail.installed" class="odetail__in">
      In your world since {{ formatDate(detail.installed.installedAt)
      }}<template v-if="detail.installed.origin === 'fluffy'"> · adopted from Fluffy</template
      ><template v-if="detail.installed.gameBuild"> · build {{ detail.installed.gameBuild }}</template>
    </p>
    <p v-else-if="detail.fluffyInstalled" class="odetail__in">
      Fluffy installed this. Adopt it on Your World to uninstall or reorder it here.
    </p>
    <p v-if="option.info?.description" class="odetail__desc">{{ option.info.description }}</p>
    <div class="odetail__kinds">
      <KindBadge v-for="k in kindsPresent(option.kinds)" :key="k" :kind="k" :count="option.kinds[k]" />
    </div>

    <div v-if="!planning" class="odetail__doors">
      <RiftButton
        v-if="!installedHere"
        variant="primary"
        :busy="world.planning.value"
        :disabled="option.menuOnly"
        reason="A menu entry installs nothing. Pick one of its variants."
        @click="world.beginInstall()"
        >Install…</RiftButton
      >
      <RiftButton v-else variant="danger" :busy="world.planning.value" @click="world.beginUninstall()"
        >Uninstall…</RiftButton
      >
      <RiftButton @click="nav.openPackage(detail.package.id, option.name)">Open in package</RiftButton>
      <RiftButton @click="preflight.reveal(detail.package.path)">Show in archive</RiftButton>
    </div>

    <InstallPlan v-if="planning" class="odetail__plan" />

    <template v-else>
      <section v-if="detail.conflicts.length" class="odetail__sec">
        <h3 class="odetail__h rs-inscription">CONFLICTS</h3>
        <p class="odetail__conf">⚔ {{ option.name }} {{ conflictSentence }}.</p>
        <ConflictMap :conflicts="detail.conflicts" />
      </section>
      <section v-if="detail.children.length" class="odetail__sec">
        <h3 class="odetail__h rs-inscription">VARIANTS BEHIND THIS MENU</h3>
        <ul class="odetail__kids">
          <li v-for="c in detail.children" :key="c.optionId">
            <button type="button" class="odetail__kid" @click="world.open(c.optionId)">
              <span>{{ c.name }}</span>
              <span class="rs-mono" :class="c.installed ? 'is-world' : 'is-rift'">{{
                c.installed ? "INSTALLED" : c.menuOnly ? "MENU" : ""
              }}</span>
            </button>
          </li>
        </ul>
        <RiftButton size="sm" @click="nav.openLookbook(detail.package.id)">Compare in the lookbook</RiftButton>
      </section>
      <section class="odetail__sec">
        <h3 class="odetail__h rs-inscription">PAYLOAD</h3>
        <PayloadTree :option="option" :tokens="search.tokens.value" />
      </section>
    </template>
  </article>
</template>

<style scoped>
.odetail__shot {
  display: block;
  width: 100%;
  aspect-ratio: 4 / 3;
  padding: 0;
  border: 0;
  background: none;
  cursor: zoom-in;
  transition: aspect-ratio var(--rs-dur-settle) var(--rs-ease-carve);
}
.odetail__shot:disabled {
  cursor: default;
}
.odetail__shot.is-strip {
  aspect-ratio: auto;
  height: 96px;
}
.odetail__shot :deep(.rs-thumb) {
  width: 100%;
  height: 100%;
}
.odetail__name {
  margin: var(--rs-space-4) 0 2px;
  font: 700 var(--rs-text-lg) / 1.2 var(--rs-font-sans);
  overflow-wrap: anywhere;
}
.odetail__by,
.odetail__in {
  margin: 6px 0 0;
  color: var(--rs-faint);
  font-size: var(--rs-text-sm);
}
.odetail__in {
  color: var(--rs-world);
}
.odetail__upd {
  padding: 0;
  border: 0;
  background: none;
  color: var(--rs-verdant);
  font: inherit;
  cursor: pointer;
}
.odetail__desc {
  margin: var(--rs-space-2) 0 0;
  line-height: 1.45;
}
.odetail__kinds {
  display: flex;
  gap: 6px;
  margin-top: var(--rs-space-3);
}
.odetail__doors {
  display: flex;
  flex-wrap: wrap;
  gap: var(--rs-space-2);
  margin-top: var(--rs-space-4);
}
.odetail__plan {
  margin-top: var(--rs-space-5);
}
.odetail__sec {
  margin-top: var(--rs-space-6);
}
.odetail__h {
  margin: 0 0 var(--rs-space-2);
  font-size: var(--rs-text-sm);
  color: var(--rs-muted);
  letter-spacing: 0.1em;
}
.odetail__conf {
  margin: 0 0 var(--rs-space-3);
  padding: var(--rs-space-2) 10px;
  border-left: 3px solid var(--rs-ember);
  background: var(--rs-hearthhi);
  font-size: var(--rs-text-sm);
}
.odetail__kids {
  margin: 0 0 var(--rs-space-3);
  padding: 0;
  list-style: none;
}
.odetail__kid {
  display: flex;
  justify-content: space-between;
  width: 100%;
  padding: 6px var(--rs-space-2);
  margin-bottom: 2px;
  border: 0;
  background: var(--rs-hearthhi);
  color: var(--rs-text);
  font: inherit;
  text-align: left;
  cursor: pointer;
}
.odetail__kid:hover {
  background: #2b2318;
}
.is-world {
  color: var(--rs-world);
}
.is-rift {
  color: var(--rs-faint);
}
</style>
