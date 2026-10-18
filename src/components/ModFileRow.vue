<script setup lang="ts">
// R16.21 — one file of a mod on Nexus, a row of its FILES view: its name,
// version, size, upload date, NEWEST, IN YOUR MODS FOLDER, its description,
// its own patch notes behind a disclosure, and Download on every row, even a
// file already in the mods folder (an update may be wanted): cyan, because it
// brings the file into the Rift, never into your world. Without a key that
// works this session, Download is held back but reachable: aria-disabled,
// still in the tab order, its reason as its description (RiftButton refused).
// So it is while this very file's download is on its way (Chaos D2): the
// spine would answer a second press with that same row.
import { computed, ref } from "vue";
import type { NexusFileEntry } from "@/types/rift";
import { formatBytes, formatDate } from "@/services/format";
import { HELD_WHILE_DOWNLOADING, useDownloads } from "@/composables/useDownloads";
import { useNexusAccount } from "@/composables/useNexusAccount";
import RiftButton from "./RiftButton.vue";

const {
  file,
  modId,
  forPackageId = null,
} = defineProps<{ file: NexusFileEntry; modId: number; forPackageId?: string | null }>();

const dl = useDownloads();
const nexus = useNexusAccount();
const notesOpen = ref(false);

/** The key works now: a free account's press starts a waiting row and opens
 *  the file's page, a premium one's downloads in the stone (useDownloads.start). */
const canDownload = computed(() => nexus.connected.value);
const busy = computed(() => dl.busy.value === `start:${modId}:${file.fileId}`);
/** This very file of this mod is on its way (another file of the mod is another download). */
const onItsWay = computed(() => dl.unfinishedFor(null, modId, file.fileId) !== null);
const heldBack = computed(() => {
  if (!canDownload.value)
    return nexus.standing.value === "none"
      ? "Add your Nexus key to download from the stone."
      : "Nexus has not accepted the key this session.";
  return onItsWay.value ? HELD_WHILE_DOWNLOADING : null;
});
</script>

<template>
  <li class="files__row" :data-file="file.fileId">
    <div class="files__line">
      <span class="files__fname">{{ file.name }}</span>
      <span v-if="file.version" class="files__mono rs-mono">v{{ file.version.replace(/^v/i, "") }}</span>
      <span v-if="file.size !== null" class="files__mono rs-mono">{{ formatBytes(file.size) }}</span>
      <span class="files__date">uploaded {{ formatDate(file.uploadedMs) }}</span>
      <span v-if="file.newest" class="files__badge rs-mono">NEWEST</span>
      <span v-if="file.inLibrary" class="files__have rs-mono">IN YOUR MODS FOLDER</span>
      <RiftButton
        class="files__dl"
        size="sm"
        variant="primary"
        :busy="busy"
        :refused="heldBack !== null"
        :reason="heldBack"
        :aria-label="`Download: ${file.name}`"
        @click="dl.start(modId, file.fileId, forPackageId)"
        >Download</RiftButton
      >
    </div>
    <p v-if="file.description" class="files__desc" :title="file.description">{{ file.description }}</p>
    <template v-if="file.changelog.length">
      <button
        type="button"
        class="files__notes"
        :aria-expanded="notesOpen"
        :aria-controls="`files-notes-${file.fileId}`"
        @click="notesOpen = !notesOpen"
      >
        Patch notes {{ notesOpen ? "▾" : "▸" }}
      </button>
      <ul v-if="notesOpen" :id="`files-notes-${file.fileId}`" class="files__log">
        <li v-for="(line, i) in file.changelog" :key="i">{{ line }}</li>
      </ul>
    </template>
  </li>
</template>

<style scoped>
.files__row {
  padding: 8px var(--rs-space-3);
  margin-bottom: 2px;
  background: var(--rs-mist);
}
.files__line {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: var(--rs-space-3);
}
.files__fname {
  color: var(--rs-text);
  font: 700 var(--rs-text-sm) var(--rs-font-sans);
  overflow-wrap: anywhere;
}
.files__mono {
  color: var(--rs-faint);
  font-size: var(--rs-text-xs);
}
.files__date {
  color: var(--rs-faint);
  font-size: var(--rs-text-sm);
}
.files__badge,
.files__have {
  padding: 2px 6px;
  border: 1px solid var(--rs-edgehi);
  border-radius: var(--rs-radius-chip);
  color: var(--rs-muted);
  font-size: var(--rs-text-xs);
  font-weight: 600;
  letter-spacing: 0.04em;
}
.files__have {
  border-color: var(--rs-rift);
  color: var(--rs-rift);
}
.files__dl {
  margin-left: auto;
  align-self: center;
}
.files__desc {
  display: -webkit-box;
  margin: 4px 0 0;
  overflow: hidden;
  color: var(--rs-muted);
  font-size: var(--rs-text-sm);
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  line-clamp: 2;
}
.files__notes {
  min-height: 28px;
  margin-top: 4px;
  padding: 0;
  border: 0;
  background: none;
  color: var(--rs-muted);
  font: 500 var(--rs-text-sm) var(--rs-font-sans);
  cursor: pointer;
}
.files__notes:focus-visible {
  outline: 2px solid var(--rs-rift);
  outline-offset: 2px;
}
.files__log {
  margin: 4px 0 0;
  padding-left: var(--rs-space-4);
  color: var(--rs-muted);
  font-size: var(--rs-text-sm);
}
</style>
