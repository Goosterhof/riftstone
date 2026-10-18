<script setup lang="ts">
// R16.18 — the DOWNLOADS shelf. Each download a mist slab on a cyan rail
// (ember once failed). Never gold: landing in the mods folder is not
// installing; the file came out of the far world into the Rift, and only an
// install crosses into your world. A landing only swaps the phase word and the
// button (§16.7: a cousin of THE CROSSING, never its twin). The bar is flat
// and determinate, drawn only while the total is known: no indeterminate loop.
// A failed row offers the ways out its reason's voice offers (§9, R16.18):
// Try again only where a second try can land, the key's door where the key is
// what failed, and the file's Nexus page on every failed row.
import { computed } from "vue";
import type { DownloadPhase, DownloadView } from "@/types/rift";
import { filePageUrl, landedPath, shareDone, useDownloads } from "@/composables/useDownloads";
import { usePreflight } from "@/composables/usePreflight";
import { useNxmHandler } from "@/composables/useNxmHandler";
import { useToasts } from "@/composables/useToasts";
import { asRiftFailure } from "@/services/rift";
import { failureDetails, voice, type Recovery, type RecoveryKind } from "@/services/voice";
import { formatBytes } from "@/services/format";
import RiftButton from "./RiftButton.vue";

const emit = defineEmits<{ replaceKey: [] }>();

const dl = useDownloads();
const preflight = usePreflight();
const nxm = useNxmHandler();

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
  // a file named like its mod is said once: never "Lantern Glow Recolor · Lantern Glow Recolor"
  if (d.fileName && d.fileName.toLowerCase() !== (d.modName ?? "").toLowerCase()) parts.push(d.fileName);
  if (d.version) parts.push(`v${d.version.replace(/^v/i, "")}`);
  return parts;
}

/** "4.1 / 12.6 MB" in the total's unit; "4.1 MB so far" without a total. */
function bytesLine(d: DownloadView): string | null {
  if (d.phase !== "downloading" && d.phase !== "verifying") return null;
  if (d.bytesTotal === null || d.bytesTotal <= 0) return `${formatBytes(d.bytesDone)} so far`;
  // both numbers in the total's unit at one decimal, so done never reads past the total
  const [, unit = "B"] = formatBytes(d.bytesTotal).split(" ");
  const scale = 1024 ** Math.max(0, ["B", "KB", "MB", "GB", "TB"].indexOf(unit));
  const done = Math.min(d.bytesDone, d.bytesTotal);
  return `${(done / scale).toFixed(1)} / ${(d.bytesTotal / scale).toFixed(1)} ${unit}`;
}

const barOf = (d: DownloadView): number | null =>
  d.phase === "downloading" || d.phase === "verifying" ? shareDone(d) : null;

const failureOf = (d: DownloadView) => (d.failure ? voice(asRiftFailure(d.failure), "download") : null);

/** §9's door for a failed row, named as §9 names it. */
const OPEN_ON_NEXUS: Recovery = { kind: "openNexusPage", label: "Open on Nexus" };
/** The recoveries a shelf row can act on; Dismiss is the header's Clear finished. */
const ON_THE_SHELF: ReadonlySet<RecoveryKind> = new Set(["retry", "replaceKey", "openNexusPage", "copyDetails"]);

/** A failed row's ways out: its voice's recoveries in their order (the first is
 *  primary), the Nexus page among them once, under §9's name. */
function waysOut(d: DownloadView): Recovery[] {
  const offered = (failureOf(d)?.recovery ?? [])
    .filter((r) => ON_THE_SHELF.has(r.kind))
    .map((r) => (r.kind === "openNexusPage" ? OPEN_ON_NEXUS : r));
  return offered.some((r) => r.kind === "openNexusPage") ? offered : [...offered, OPEN_ON_NEXUS];
}

async function copyDetails(d: DownloadView): Promise<void> {
  const text = d.failure ? failureDetails(asRiftFailure(d.failure), "download") : "";
  try {
    await navigator.clipboard.writeText(text);
    useToasts().push("rift", "The details are on your clipboard.");
  } catch {
    useToasts().push("warn", "The clipboard refused; the details are in the log instead.");
  }
}

async function recover(d: DownloadView, kind: RecoveryKind): Promise<void> {
  if (kind === "retry") await dl.start(d.modId, d.fileId, d.forPackageId);
  else if (kind === "openNexusPage") await preflight.openUrl(filePageUrl(d));
  else if (kind === "replaceKey") emit("replaceKey");
  else if (kind === "copyDetails") await copyDetails(d);
}

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
            <!-- a landed file that is no archive has no package: Explorer finds it, the Rift never lists it -->
            <RiftButton v-else-if="d.phase === 'landed' && landedPath(d)" size="sm" @click="dl.showInFolder(d)"
              >Show in folder</RiftButton
            >
            <template v-if="d.phase === 'failed'">
              <RiftButton
                v-for="(r, i) in waysOut(d)"
                :key="r.kind"
                size="sm"
                :variant="i === 0 ? 'primary' : 'secondary'"
                :busy="r.kind === 'retry' && dl.busy.value === `start:${d.modId}:${d.fileId}`"
                @click="recover(d, r.kind)"
                >{{ r.label }}</RiftButton
              >
            </template>
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
        <p v-if="d.phase === 'waiting' && !nxm.silent.value" class="shelf__press">
          Press Mod Manager Download on Nexus. Nexus sends a free account's downloads through its own button, and the
          stone picks them up from there.
        </p>
        <!-- the site's button reaches the stone only while the stone answers it:
             say so where the press is asked for, with the switch's own act -->
        <p v-else-if="d.phase === 'waiting'" class="shelf__press shelf__press--off">
          Nothing hands Nexus's button to the stone now. Let the stone answer it first, then press Mod Manager Download
          on Nexus.
          <RiftButton size="sm" variant="primary" :busy="nxm.busy.value" @click="nxm.set(true)"
            >Answer Nexus's button</RiftButton
          >
        </p>
        <p v-if="d.phase === 'landed' && d.alreadyHad" class="shelf__gone">
          The same file was already in your mods folder. The stone wrote nothing new.
        </p>
        <p v-else-if="d.phase === 'landed' && !d.packageId" class="shelf__gone">
          It is no archive, so the Rift does not list it.
        </p>
        <p v-if="d.phase === 'failed' && failureOf(d)" class="shelf__fail">
          The download of {{ d.modName ?? d.fileName ?? "this file" }} stopped. {{ failureOf(d)?.voice }}
          {{ failureOf(d)?.empathy }}
        </p>
        <p v-if="d.phase === 'cancelled'" class="shelf__gone">Cancelled. The partial file is gone.</p>
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
.shelf__fail,
.shelf__gone {
  margin: var(--rs-space-1) 0 0;
  font-size: var(--rs-text-sm);
}
.shelf__press {
  color: var(--rs-text);
}
.shelf__fail {
  color: var(--rs-muted);
}
.shelf__gone {
  color: var(--rs-faint);
}
</style>
