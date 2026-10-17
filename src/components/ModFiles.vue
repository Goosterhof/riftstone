<script setup lang="ts">
// R16.21 — a mod's FILES, a sub-view of the far world: it replaces the page's
// scroll, and "‹ The far world" or Esc hands the same scroll back. One group
// per category in the wire's order; the retired ones (old versions, removed,
// archived) fold closed under one "Older files" disclosure. Every row offers
// Download, even a file already in the mods folder (an update may be wanted):
// cyan, because it brings the file into the Rift, never into your world.
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import type { NexusFileCategory, NexusFileEntry, NexusModFiles } from "@/types/rift";
import { asRiftFailure, rift } from "@/services/rift";
import { voice, type Voiced } from "@/services/voice";
import { formatBytes, formatDate } from "@/services/format";
import { useDownloads } from "@/composables/useDownloads";
import { useNexusAccount } from "@/composables/useNexusAccount";
import { usePreflight } from "@/composables/usePreflight";
import FailureNotice from "./FailureNotice.vue";
import RiftButton from "./RiftButton.vue";
import StandingControls from "./StandingControls.vue";

const {
  modId,
  name = null,
  forPackageId = null,
} = defineProps<{ modId: number; name?: string | null; forPackageId?: string | null }>();
const emit = defineEmits<{ back: [] }>();

const dl = useDownloads();
const nexus = useNexusAccount();
const preflight = usePreflight();

const files = ref<NexusModFiles | null>(null);
const failure = ref<Voiced | null>(null);
const loading = ref(false);
const olderOpen = ref(false);
const notesOpen = ref<Set<number>>(new Set());
const backButton = ref<HTMLElement | null>(null);

const LIVE: NexusFileCategory[] = ["main", "update", "optional", "miscellaneous", "unknown"];
const RETIRED: NexusFileCategory[] = ["oldVersion", "removed", "archived"];
const HEAD: Record<NexusFileCategory, string> = {
  main: "MAIN FILES",
  update: "UPDATES",
  optional: "OPTIONAL FILES",
  miscellaneous: "MISCELLANEOUS",
  unknown: "OTHER FILES",
  oldVersion: "OLD VERSIONS",
  removed: "REMOVED",
  archived: "ARCHIVED",
};

function groupsOf(categories: NexusFileCategory[]): { category: NexusFileCategory; files: NexusFileEntry[] }[] {
  const all = files.value?.files ?? [];
  return categories
    .map((category) => ({ category, files: all.filter((f) => f.category === category) }))
    .filter((g) => g.files.length > 0);
}
const live = computed(() => groupsOf(LIVE));
const retired = computed(() => groupsOf(RETIRED));
const title = computed(() => files.value?.modName ?? name ?? `Mod ${modId}`);
const keyless = computed(() => nexus.standing.value === "none");
const canDownload = computed(() => nexus.connected.value);

async function load(): Promise<void> {
  loading.value = true;
  failure.value = null;
  try {
    const backend = await rift();
    files.value = await backend.getModFiles(modId);
  } catch (raw) {
    failure.value = voice(asRiftFailure(raw), "browse");
  } finally {
    loading.value = false;
  }
}

function toggleNotes(fileId: number): void {
  const next = new Set(notesOpen.value);
  if (next.has(fileId)) next.delete(fileId);
  else next.add(fileId);
  notesOpen.value = next;
}

function onKey(event: KeyboardEvent): void {
  if (event.key !== "Escape" || event.defaultPrevented) return;
  event.preventDefault();
  emit("back");
}

/** R16.21: "Download" on every row, for both tiers; a free account's press
 *  starts a waiting row and opens the file's page (useDownloads.start). */
const downloadLabel = "Download";
const busyFile = (f: NexusFileEntry): boolean => dl.busy.value === `start:${modId}:${f.fileId}`;

onMounted(async () => {
  window.addEventListener("keydown", onKey);
  await nextTick();
  backButton.value?.querySelector("button")?.focus();
  await load();
});
onBeforeUnmount(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <section class="files" :aria-label="`Files of ${title} on Nexus`" data-mod-files>
    <span ref="backButton" class="files__back">
      <RiftButton size="sm" @click="emit('back')">‹ The far world</RiftButton>
    </span>
    <header class="files__head">
      <h2 class="files__name">{{ title }}</h2>
      <span v-if="files?.adult" class="files__badge rs-mono" title="Nexus marks this mod as adult content">ADULT</span>
      <RiftButton v-if="files" size="sm" @click="preflight.openUrl(files.pageUrl)">Open on Nexus</RiftButton>
    </header>
    <StandingControls :mod-id="modId" :name="title" class="files__standing" />

    <FailureNotice v-if="failure" :voiced="failure" :busy="loading" @recover="load()" />
    <p v-else-if="loading && !files" class="files__dim rs-mono" role="status">asking Nexus for the files…</p>

    <template v-if="files">
      <p v-if="keyless" class="files__nokey">Add your Nexus key in PREFLIGHT to download from the stone.</p>
      <p v-if="!files.files.length" class="files__dim">Nexus lists no files for this mod.</p>

      <template v-for="group in [...live, ...(olderOpen ? retired : [])]" :key="group.category">
        <h3 class="files__h rs-inscription">{{ HEAD[group.category] }}</h3>
        <ul class="files__rows" :aria-label="HEAD[group.category]">
          <li v-for="f in group.files" :key="f.fileId" class="files__row" :data-file="f.fileId">
            <div class="files__line">
              <span class="files__fname">{{ f.name }}</span>
              <span v-if="f.version" class="files__mono rs-mono">v{{ f.version.replace(/^v/i, "") }}</span>
              <span v-if="f.size !== null" class="files__mono rs-mono">{{ formatBytes(f.size) }}</span>
              <span class="files__date">uploaded {{ formatDate(f.uploadedMs) }}</span>
              <span v-if="f.newest" class="files__badge rs-mono">NEWEST</span>
              <span v-if="f.inLibrary" class="files__have rs-mono">IN YOUR MODS FOLDER</span>
              <RiftButton
                class="files__dl"
                size="sm"
                variant="primary"
                :busy="busyFile(f)"
                :disabled="!canDownload"
                :reason="
                  keyless
                    ? 'Add your Nexus key in PREFLIGHT to download from the stone.'
                    : 'Nexus has not accepted the key this session.'
                "
                :aria-label="`${downloadLabel}: ${f.name}`"
                @click="dl.start(modId, f.fileId, forPackageId)"
                >{{ downloadLabel }}</RiftButton
              >
            </div>
            <p v-if="f.description" class="files__desc" :title="f.description">{{ f.description }}</p>
            <template v-if="f.changelog.length">
              <button
                type="button"
                class="files__notes"
                :aria-expanded="notesOpen.has(f.fileId)"
                :aria-controls="`files-notes-${f.fileId}`"
                @click="toggleNotes(f.fileId)"
              >
                Patch notes {{ notesOpen.has(f.fileId) ? "▾" : "▸" }}
              </button>
              <ul v-if="notesOpen.has(f.fileId)" :id="`files-notes-${f.fileId}`" class="files__log">
                <li v-for="(line, i) in f.changelog" :key="i">{{ line }}</li>
              </ul>
            </template>
          </li>
        </ul>
      </template>

      <button
        v-if="retired.length"
        type="button"
        class="files__older"
        :aria-expanded="olderOpen"
        @click="olderOpen = !olderOpen"
      >
        Older files {{ olderOpen ? "▾" : "▸" }}
        <span class="files__dim">{{ retired.reduce((n, g) => n + g.files.length, 0) }}</span>
      </button>
    </template>
  </section>
</template>

<style scoped>
.files {
  max-width: 880px;
  margin-top: var(--rs-space-5);
}
.files__head {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: var(--rs-space-3);
  margin-top: var(--rs-space-4);
}
.files__name {
  margin: 0;
  color: var(--rs-text);
  font: 700 var(--rs-text-lg) var(--rs-font-sans);
  overflow-wrap: anywhere;
}
.files__standing {
  margin-top: var(--rs-space-3);
}
.files__h {
  margin: var(--rs-space-5) 0 var(--rs-space-2);
  font-size: 14px;
  color: var(--rs-muted);
  letter-spacing: 0.1em;
}
.files__rows {
  margin: 0;
  padding: 0;
  list-style: none;
}
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
.files__date,
.files__dim {
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
.files__notes,
.files__older {
  min-height: 28px;
  margin-top: 4px;
  padding: 0;
  border: 0;
  background: none;
  color: var(--rs-muted);
  font: 500 var(--rs-text-sm) var(--rs-font-sans);
  cursor: pointer;
}
.files__older {
  margin-top: var(--rs-space-4);
}
.files__notes:focus-visible,
.files__older:focus-visible {
  outline: 2px solid var(--rs-rift);
  outline-offset: 2px;
}
.files__log {
  margin: 4px 0 0;
  padding-left: var(--rs-space-4);
  color: var(--rs-muted);
  font-size: var(--rs-text-sm);
}
.files__nokey {
  margin: var(--rs-space-3) 0 0;
  color: var(--rs-text);
}
</style>
