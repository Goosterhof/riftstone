<script setup lang="ts">
// R16.18 — the DOWNLOADS shelf. Each download a mist slab on a cyan rail
// (ember once failed). Never gold: landing in the mods folder is not
// installing; the file came out of the far world into the Rift, and only an
// install crosses into your world. A landing only swaps the phase word and the
// button (§16.7: a cousin of THE CROSSING, never its twin). The bar is flat
// and determinate, drawn only while the total is known: no indeterminate loop.
import { computed } from "vue";
import type { DownloadPhase, DownloadView } from "@/types/rift";
import { shareDone, useDownloads } from "@/composables/useDownloads";
import { asRiftFailure } from "@/services/rift";
import { voice } from "@/services/voice";
import { formatBytes } from "@/services/format";
import RiftButton from "./RiftButton.vue";

const dl = useDownloads();

const PHASE: Record<DownloadPhase, string> = {
  waiting: "Waiting for Nexus",
  queued: "Queued",
  linking: "Asking Nexus",
  downloading: "Downloading",
  verifying: "Checking the file",
  landed: "In your mods folder",
  failed: "Failed",
  cancelled: "Cancelled",
};

const phaseWord = (d: DownloadView): string =>
  d.phase === "landed" && d.alreadyHad ? "Already in your mods folder" : PHASE[d.phase];

/** Line 1: mod · file · v{version}; unknown parts are omitted, never "null". */
function title(d: DownloadView): string[] {
  const parts: string[] = [];
  if (d.fileName) parts.push(d.fileName);
  if (d.version) parts.push(`v${d.version.replace(/^v/i, "")}`);
  return parts;
}

/** "4.1 / 12.6 MB" in the total's unit; "4.1 MB so far" without a total. */
function bytesLine(d: DownloadView): string | null {
  if (d.phase !== "downloading" && d.phase !== "verifying") return null;
  if (d.bytesTotal === null || d.bytesTotal <= 0) return `${formatBytes(d.bytesDone)} so far`;
  const total = formatBytes(d.bytesTotal);
  const [, unit = "B"] = total.split(" ");
  const scale = 1024 ** Math.max(0, ["B", "KB", "MB", "GB", "TB"].indexOf(unit));
  return `${(d.bytesDone / scale).toFixed(1)} / ${total}`;
}

const barOf = (d: DownloadView): number | null =>
  d.phase === "downloading" || d.phase === "verifying" ? shareDone(d) : null;

const failureOf = (d: DownloadView) => (d.failure ? voice(asRiftFailure(d.failure), "download") : null);

const anyFinished = computed(() => dl.finished.value.length > 0);
</script>

<template>
  <section v-if="dl.downloads.value.length" id="far-downloads" class="shelf far-sec" aria-labelledby="far-h-downloads">
    <header class="shelf__head">
      <h2 id="far-h-downloads" class="shelf__h rs-inscription">DOWNLOADS</h2>
      <RiftButton v-if="anyFinished" size="sm" :busy="dl.busy.value === 'clear'" @click="dl.clear()"
        >Clear finished</RiftButton
      >
    </header>
    <ul class="shelf__rows" aria-label="Downloads into your mods folder">
      <li
        v-for="d in dl.downloads.value"
        :key="d.id"
        class="shelf__row"
        :class="`is-${d.phase}`"
        :data-download="d.id"
        :data-phase="d.phase"
      >
        <div class="shelf__line">
          <span class="shelf__what">
            <b v-if="d.modName">{{ d.modName }}</b
            ><template v-for="(p, i) in title(d)" :key="i"
              ><template v-if="d.modName || i > 0"> · </template>{{ p }}</template
            ><template v-if="!d.modName && !title(d).length">Mod {{ d.modId }}</template>
          </span>
          <span class="shelf__phase" role="status">{{ phaseWord(d) }}</span>
          <span v-if="bytesLine(d)" class="shelf__bytes rs-mono">{{ bytesLine(d) }}</span>
          <span class="shelf__acts">
            <RiftButton
              v-if="d.phase === 'landed' && d.packageId"
              size="sm"
              variant="primary"
              @click="dl.showInRift(d.packageId)"
              >Show in the Rift</RiftButton
            >
            <RiftButton
              v-if="d.phase === 'failed'"
              size="sm"
              variant="primary"
              :busy="dl.busy.value === `start:${d.modId}:${d.fileId}`"
              @click="dl.start(d.modId, d.fileId, d.forPackageId)"
              >Try again</RiftButton
            >
            <RiftButton
              v-if="d.phase === 'waiting' && d.nexusPageUrl"
              size="sm"
              variant="primary"
              @click="dl.openPage(d.id)"
              >Open on Nexus</RiftButton
            >
            <RiftButton
              v-if="dl.canCancel(d)"
              size="sm"
              :busy="dl.busy.value === `cancel:${d.id}`"
              :aria-label="`Cancel the download of ${d.modName ?? d.fileName ?? `mod ${d.modId}`}`"
              @click="dl.cancel(d.id)"
              >Cancel</RiftButton
            >
          </span>
        </div>
        <div
          v-if="barOf(d) !== null"
          class="shelf__bar"
          role="progressbar"
          :aria-label="`${d.modName ?? 'Download'} progress`"
          aria-valuemin="0"
          :aria-valuenow="d.bytesDone"
          :aria-valuemax="d.bytesTotal ?? undefined"
          :aria-valuetext="bytesLine(d) ?? undefined"
        >
          <span class="shelf__fill" :style="{ width: `${((barOf(d) ?? 0) * 100).toFixed(2)}%` }" />
        </div>
        <p v-if="d.phase === 'waiting'" class="shelf__press">Press Mod Manager Download on Nexus.</p>
        <p v-if="d.phase === 'failed' && failureOf(d)" class="shelf__fail">
          {{ failureOf(d)?.voice }} {{ failureOf(d)?.empathy }}
        </p>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.shelf__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--rs-space-3);
  margin-bottom: var(--rs-space-2);
}
.shelf__h {
  margin: 0;
  font-size: 14px;
  color: var(--rs-muted);
  letter-spacing: 0.1em;
}
.shelf__rows {
  margin: 0;
  padding: 0;
  list-style: none;
}
.shelf__row {
  padding: var(--rs-space-3) var(--rs-space-4);
  margin-bottom: var(--rs-space-2);
  background: var(--rs-mist);
  border-left: 4px solid var(--rs-rift);
}
.shelf__row.is-failed {
  border-left-color: var(--rs-ember);
}
.shelf__line {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--rs-space-3);
}
.shelf__what {
  flex: 1;
  min-width: 12ch;
  color: var(--rs-text);
  font-size: var(--rs-text-sm);
  overflow-wrap: anywhere;
}
.shelf__what b {
  font-weight: 700;
}
.shelf__phase {
  color: var(--rs-muted);
  font: 500 var(--rs-text-sm) var(--rs-font-sans);
}
.is-failed .shelf__phase {
  color: var(--rs-ember);
}
.is-cancelled .shelf__phase {
  color: var(--rs-faint);
}
.shelf__bytes {
  color: var(--rs-faint);
  font-size: var(--rs-text-xs);
}
.shelf__acts {
  display: flex;
  gap: var(--rs-space-2);
}
.shelf__bar {
  height: 4px;
  margin-top: var(--rs-space-2);
  background: var(--rs-edge);
}
.shelf__fill {
  display: block;
  height: 100%;
  background: var(--rs-rift);
  transition: width var(--rs-dur-quick) ease-out;
}
.shelf__press,
.shelf__fail {
  margin: var(--rs-space-1) 0 0;
  font-size: var(--rs-text-sm);
}
.shelf__press {
  color: var(--rs-text);
}
.shelf__fail {
  color: var(--rs-muted);
}
</style>
