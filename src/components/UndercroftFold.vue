<script setup lang="ts">
// v0.5 — THE UNDERCROFT (#00015 R18.11–R18.14): the trouble fold turned upside
// down, at the Rift's foot, outside role="tree". What won't answer sits above
// the creations; what was put away sits beneath them. The same 40 px line, the
// same carved mono count, the same Show ▸ / Close ▾, the opposite end of the
// column (§18.13). No motion: the body appears where it stands (R18.31).
// The count is read at R18.11's moments, never live (the folder has no watcher).
import { computed, onMounted } from "vue";
import { useUndercroft } from "@/composables/useUndercroft";
import { useVersions } from "@/composables/useVersions";
import { useToasts } from "@/composables/useToasts";
import { formatBytes } from "@/services/format";
import { voicedLine } from "@/services/voice";
import {
  UNDERCROFT,
  UNDERCROFT_BODY,
  UNDERCROFT_EMPTY,
  UNDERCROFT_UNMADE,
  shelfNewer,
  undercroftCount,
  versionLabel,
  versionName,
} from "@/services/versions";
import type { OlderShelfItem } from "@/types/rift";
import RiftButton from "./RiftButton.vue";

const shelf = useUndercroft();
const versions = useVersions();

const items = computed(() => shelf.view.value?.items ?? []);
const exists = computed(() => shelf.view.value?.exists ?? false);
const carved = computed(() => undercroftCount(items.value.length));

async function bringBack(item: OlderShelfItem): Promise<void> {
  const landed = await shelf.bringBack(item.fileName);
  if (!landed) return;
  await versions.broughtBack(item.fileName, landed, versionName(item.version, item.fileName), item.modName);
}

async function showInExplorer(): Promise<void> {
  const failed = await shelf.showInExplorer();
  if (failed) useToasts().push("ember", voicedLine(failed));
}

onMounted(() => void shelf.load());
</script>

<template>
  <section class="crypt" :class="{ 'is-empty': !items.length }" aria-label="Older versions put away">
    <div class="crypt__line">
      <p id="rift-undercroft-count" class="crypt__count" aria-live="polite">{{ carved }}</p>
      <div class="crypt__doors">
        <RiftButton v-if="shelf.open.value && exists && !shelf.failure.value" size="sm" @click="showInExplorer()"
          >Show in Explorer</RiftButton
        >
        <RiftButton
          size="sm"
          :aria-expanded="shelf.open.value ? 'true' : 'false'"
          aria-controls="rift-undercroft"
          aria-describedby="rift-undercroft-count"
          @click="shelf.toggle()"
        >
          {{ shelf.open.value ? "Close" : "Show" }}
          <span aria-hidden="true">{{ shelf.open.value ? "▾" : "▸" }}</span>
        </RiftButton>
      </div>
    </div>
    <div id="rift-undercroft" class="crypt__body" :hidden="!shelf.open.value">
      <template v-if="shelf.open.value">
        <h3 class="crypt__head rs-inscription">{{ UNDERCROFT }}</h3>
        <template v-if="shelf.failure.value">
          <p class="crypt__say is-ember" role="alert">
            {{ shelf.failure.value.voice }} {{ shelf.failure.value.empathy }}
          </p>
          <RiftButton size="sm" @click="shelf.load()">Try again</RiftButton>
        </template>
        <p v-else-if="!exists" class="crypt__say">{{ UNDERCROFT_UNMADE }}</p>
        <p v-else-if="!items.length" class="crypt__say">{{ UNDERCROFT_EMPTY }}</p>
        <template v-else>
          <p class="crypt__say">{{ UNDERCROFT_BODY[0] }} {{ UNDERCROFT_BODY[1] }}</p>
          <ul class="crypt__rows">
            <li v-for="item in items" :key="item.fileName" class="crypt__row" :data-shelf-file="item.fileName">
              <p class="crypt__line1">
                <template v-if="item.modName">
                  <span class="crypt__name">{{ item.modName }}</span>
                  <span v-if="item.version" class="crypt__ver">{{ versionLabel(item.version) }}</span>
                </template>
                <span v-else class="crypt__file is-text rs-tail" :title="item.fileName"
                  ><span class="rs-tail__text">{{ item.fileName }}</span></span
                >
                <span class="crypt__size">{{ formatBytes(item.size) }}</span>
                <RiftButton
                  size="sm"
                  variant="primary"
                  :busy="shelf.moving.value === item.fileName"
                  @click="bringBack(item)"
                  >Bring it back</RiftButton
                >
              </p>
              <p v-if="item.modName" class="crypt__file rs-tail" :title="item.fileName">
                <span class="rs-tail__text">{{ item.fileName }}</span>
              </p>
              <p class="crypt__newer" :class="{ 'is-only': !item.newerInMods }">{{ shelfNewer(item.newerInMods) }}</p>
              <div v-if="shelf.rowFailures.value.get(item.fileName)" class="crypt__fail" role="alert">
                <p>
                  {{ shelf.rowFailures.value.get(item.fileName)?.voiced.voice }}
                  {{ shelf.rowFailures.value.get(item.fileName)?.voiced.empathy }}
                </p>
                <RiftButton size="sm" variant="primary" @click="bringBack(item)">Try again</RiftButton>
                <RiftButton size="sm" @click="versions.copy(shelf.rowFailures.value.get(item.fileName)?.details ?? '')"
                  >Copy details</RiftButton
                >
              </div>
            </li>
          </ul>
        </template>
      </template>
    </div>
  </section>
</template>

<style scoped>
.crypt {
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-2);
  padding: 0 var(--rs-space-6) var(--rs-space-12); /* the tree's own side padding */
  margin-top: calc(-1 * var(--rs-space-12) + var(--rs-space-2));
}
/* the trouble fold's line, upside down: on the void (it is outside the
   library), a hairline above, the quiet 4 px rail */
.crypt__line {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--rs-space-4);
  min-height: 40px;
  padding: 0 var(--rs-space-4) 0 var(--rs-space-5);
  border-top: 1px solid var(--rs-edge);
  background: var(--rs-void);
}
.crypt__line::before {
  content: "";
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 4px;
  background: var(--rs-faint);
}
.crypt__count {
  margin: 0;
  color: var(--rs-muted);
  font: 600 var(--rs-text-xs) / 1.4 var(--rs-font-mono);
  font-variant-numeric: tabular-nums;
  letter-spacing: 0.12em;
  white-space: nowrap;
}
.crypt.is-empty .crypt__count {
  color: var(--rs-faint);
}
.crypt__doors {
  display: flex;
  gap: var(--rs-space-2);
}
.crypt__body[hidden] {
  display: none;
}
.crypt__head {
  margin: var(--rs-space-2) 0 var(--rs-space-1);
  font-size: 14px;
  color: var(--rs-muted);
  letter-spacing: 0.1em;
}
.crypt__say {
  margin: 0 0 var(--rs-space-2);
  color: var(--rs-muted);
  font-size: var(--rs-text-sm);
  line-height: 1.45;
}
.crypt__say.is-ember {
  color: var(--rs-ember);
}
/* beneath the Rift things are set down, not displayed: no slab, no rail */
.crypt__rows {
  margin: 0;
  padding: 0;
  list-style: none;
  border-top: 1px solid var(--rs-edge);
}
.crypt__row {
  padding: var(--rs-space-3) var(--rs-space-4);
  border-bottom: 1px solid var(--rs-edge);
}
.crypt__line1 {
  display: flex;
  align-items: center;
  gap: var(--rs-space-2);
  min-width: 0;
  margin: 0;
}
.crypt__name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--rs-text);
  font: 700 var(--rs-text-sm) var(--rs-font-sans);
}
.crypt__ver {
  flex: none;
  color: var(--rs-muted);
  font: 400 12px var(--rs-font-mono);
}
.crypt__size {
  flex: none;
  margin-left: auto;
  color: var(--rs-faint);
  font: 400 12px var(--rs-font-mono);
}
.crypt__file {
  margin: var(--rs-space-1) 0 0;
  color: var(--rs-faint);
  font: 400 12px var(--rs-font-mono);
}
.crypt__line1 .crypt__file {
  flex: 1 1 0;
  margin: 0;
}
.crypt__file.is-text {
  color: var(--rs-text);
}
.crypt__newer {
  margin: var(--rs-space-1) 0 0;
  color: var(--rs-faint);
  font-size: var(--rs-text-sm);
}
/* one step louder: this is the only copy the player has */
.crypt__newer.is-only {
  color: var(--rs-muted);
}
.crypt__fail {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--rs-space-2);
  margin-top: var(--rs-space-2);
}
.crypt__fail p {
  flex-basis: 100%;
  margin: 0;
  color: var(--rs-ember);
  font-size: var(--rs-text-sm);
}
</style>
