<script setup lang="ts">
// R16.21 — a mod's FILES, a sub-view of the far world: it replaces the page's
// scroll, and "‹ The far world" or Esc hands the same scroll back. One group
// per category in the wire's order; the retired ones (old versions, removed,
// archived) fold closed under one "Older files" disclosure, which opens them
// below itself, in the region it controls. Each file is a ModFileRow, with
// Download on every row (held back but reachable without a key).
import { computed, nextTick, onBeforeUnmount, onMounted, ref, useId } from "vue";
import type { NexusFileCategory, NexusFileEntry, NexusModFiles } from "@/types/rift";
import { asRiftFailure, rift } from "@/services/rift";
import { voice, type Voiced } from "@/services/voice";
import { useNexusAccount } from "@/composables/useNexusAccount";
import { usePreflight } from "@/composables/usePreflight";
import FailureNotice from "./FailureNotice.vue";
import ModFileRow from "./ModFileRow.vue";
import RiftButton from "./RiftButton.vue";
import StandingControls from "./StandingControls.vue";

const {
  modId,
  name = null,
  forPackageId = null,
} = defineProps<{ modId: number; name?: string | null; forPackageId?: string | null }>();
const emit = defineEmits<{ back: [] }>();

const nexus = useNexusAccount();
const preflight = usePreflight();

const files = ref<NexusModFiles | null>(null);
const failure = ref<Voiced | null>(null);
const loading = ref(false);
const olderOpen = ref(false);
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
/** The region "Older files" opens and controls. */
const olderId = `files-older-${useId()}`;
const title = computed(() => files.value?.modName ?? name ?? `Mod ${modId}`);
const keyless = computed(() => nexus.standing.value === "none");

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

function onKey(event: KeyboardEvent): void {
  if (event.key !== "Escape" || event.defaultPrevented) return;
  event.preventDefault();
  emit("back");
}

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

      <template v-for="group in live" :key="group.category">
        <h3 class="files__h rs-inscription">{{ HEAD[group.category] }}</h3>
        <ul class="files__rows" :aria-label="HEAD[group.category]">
          <ModFileRow
            v-for="f in group.files"
            :key="f.fileId"
            :file="f"
            :mod-id="modId"
            :for-package-id="forPackageId"
          />
        </ul>
      </template>

      <template v-if="retired.length">
        <button
          type="button"
          class="files__older"
          :aria-expanded="olderOpen"
          :aria-controls="olderId"
          @click="olderOpen = !olderOpen"
        >
          Older files {{ olderOpen ? "▾" : "▸" }}
          <span class="files__dim">{{ retired.reduce((n, g) => n + g.files.length, 0) }}</span>
        </button>
        <div :id="olderId" class="files__older-groups">
          <template v-if="olderOpen">
            <template v-for="group in retired" :key="group.category">
              <h3 class="files__h rs-inscription">{{ HEAD[group.category] }}</h3>
              <ul class="files__rows" :aria-label="HEAD[group.category]">
                <ModFileRow
                  v-for="f in group.files"
                  :key="f.fileId"
                  :file="f"
                  :mod-id="modId"
                  :for-package-id="forPackageId"
                />
              </ul>
            </template>
          </template>
        </div>
      </template>
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
.files__dim {
  color: var(--rs-faint);
  font-size: var(--rs-text-sm);
}
.files__badge {
  padding: 2px 6px;
  border: 1px solid var(--rs-edgehi);
  border-radius: var(--rs-radius-chip);
  color: var(--rs-muted);
  font-size: var(--rs-text-xs);
  font-weight: 600;
  letter-spacing: 0.04em;
}
.files__older {
  min-height: 28px;
  margin-top: var(--rs-space-4);
  padding: 0;
  border: 0;
  background: none;
  color: var(--rs-muted);
  font: 500 var(--rs-text-sm) var(--rs-font-sans);
  cursor: pointer;
}
.files__older:focus-visible {
  outline: 2px solid var(--rs-rift);
  outline-offset: 2px;
}
.files__nokey {
  margin: var(--rs-space-3) 0 0;
  color: var(--rs-text);
}
</style>
