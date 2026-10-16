<script setup lang="ts">
// §8.5 — NEXUS, cold: these live in another world. Three lean sections —
// Linked, Unlinked, Key. Update checks are anonymous (no key needed); a key
// is optional and only adds the account extras.
import { computed, onMounted, ref } from "vue";
import type { UpdateRow } from "@/types/rift";
import { useFarWorld } from "@/composables/useFarWorld";
import { useStone } from "@/composables/useStone";
import { usePreflight, NEXUS_KEY_URL } from "@/composables/usePreflight";
import { keyStateOf, type NexusKeyState } from "@/services/pending";
import { formatAgo, formatClock, formatDate } from "@/services/format";
import FailureNotice from "@/components/FailureNotice.vue";
import RiftButton from "@/components/RiftButton.vue";
import RiftInscription from "@/components/RiftInscription.vue";
import StateChip from "@/components/StateChip.vue";

const far = useFarWorld();
const stone = useStone();
const preflight = usePreflight();

const linkText = ref("");
const keyText = ref("");
const keyOpen = ref(false);

/** §9: the offline notice says how old the status below is. The voice can't
 *  know (it maps a failure); the page does, so it dates the empathy line. */
const checkNotice = computed(() => {
  const failure = far.checkFailure.value;
  const ms = far.updates.value?.lastCheckMs ?? null;
  if (!failure || !far.offline.value || ms === null) return null;
  return {
    ...failure,
    empathy: `Update status below is from the last successful check, ${formatAgo(ms)}. Everything else works offline.`,
  };
});

const lastCheck = computed(() => {
  const ms = far.updates.value?.lastCheckMs ?? null;
  return ms === null ? "Never checked" : `Last checked ${formatAgo(ms)}`;
});

const summary = computed(() => stone.overview.value?.nexus ?? null);
const keyState = computed<NexusKeyState>(() =>
  summary.value ? keyStateOf(summary.value, far.keyFailure.value !== null, far.offline.value) : "none",
);

const KEY_CHIP: Record<NexusKeyState, string> = {
  connected: "CONNECTED",
  none: "NO KEY",
  rejected: "REJECTED",
  offline: "OFFLINE",
};

const progressLine = computed(() => {
  const p = far.progress.value;
  if (!p) return "Asking Nexus…";
  const verb = { linking: "Linking", hashing: "Hashing", checking: "Checking" }[p.phase];
  return `${verb} ${p.done} of ${p.total}…`;
});

const quotaLine = computed(() => {
  const q = far.quota.value;
  if (!q) return null;
  return `hourly ${q.hourlyRemaining}/${q.hourlyLimit} · daily ${q.dailyRemaining.toLocaleString("en-GB")}/${q.dailyLimit.toLocaleString("en-GB")}`;
});

const quotaLow = computed(() => {
  const q = far.quota.value;
  if (!q || q.hourlyRemaining > 20) return null;
  const at = q.hourlyResetMs ? ` Checks resume at ${formatClock(q.hourlyResetMs)};` : "";
  const then = at ? `${at} we'll` : " We'll";
  return `${q.hourlyRemaining} requests left this hour.${then} only re-check mods you open until then.`;
});

const quotaSpent = computed(() => far.quota.value?.hourlyRemaining === 0);

function latest(row: UpdateRow): string {
  const s = row.state;
  if (s.kind === "updateAvailable") return s.newer.version ? `v${s.newer.version}` : formatDate(s.newer.uploadedMs);
  if (s.kind === "current") return "current";
  if (s.kind === "localArchived") return "archived on Nexus";
  if (s.kind === "modUnavailable") return s.status;
  if (s.kind === "error") return "last check failed";
  if (s.kind === "unpinned") return "no file pinned";
  return "not checked";
}

function guess(row: UpdateRow): string {
  return row.localVersion ? `local version ${row.localVersion}` : "no Nexus name in the file";
}

async function link(packageId: string): Promise<void> {
  if (await far.link(packageId, linkText.value)) linkText.value = "";
}

async function saveKey(): Promise<void> {
  if (await far.setKey(keyText.value.trim() || null)) {
    keyText.value = "";
    keyOpen.value = false;
  }
}

function openLink(packageId: string): void {
  far.linkingId.value = packageId;
  far.linkFailure.value = null;
  linkText.value = "";
}

onMounted(() => void far.load());
</script>

<template>
  <main class="far" aria-label="Nexus — updates">
    <header class="far__head">
      <div>
        <h1 class="far__title rs-inscription">THE FAR WORLD</h1>
        <p class="far__sub">
          Nexus Mods updates · <span class="rs-mono">{{ lastCheck }}</span>
          <template v-if="far.checking.value">
            · <span class="rs-mono far__busy">{{ progressLine }}</span></template
          >
        </p>
      </div>
      <div class="far__acts">
        <StateChip v-if="far.updateCount.value" state="update" />
        <RiftButton
          variant="primary"
          :busy="far.checking.value"
          :disabled="quotaSpent"
          reason="Nexus's hourly quota is spent; checks resume when it refills."
          @click="far.check()"
          >Check now</RiftButton
        >
      </div>
    </header>

    <FailureNotice
      v-if="far.checkFailure.value"
      :voiced="checkNotice ?? far.checkFailure.value"
      :busy="far.checking.value"
      @recover="far.check()"
    />

    <section class="far__sec">
      <h2 class="far__h rs-inscription">LINKED</h2>
      <p v-if="!far.linked.value.length" class="far__dim">No package is linked yet. Link one below by its Nexus URL.</p>
      <table v-else class="far__table">
        <thead>
          <tr>
            <th>Package</th>
            <th>Local</th>
            <th>Latest</th>
            <th />
            <th />
          </tr>
        </thead>
        <tbody>
          <tr v-for="row in far.linked.value" :key="row.packageId">
            <td>
              <span class="far__name">{{ row.packageName }}</span>
              <span class="far__dim rs-mono">
                mod {{ row.link?.modId }} · {{ row.link?.source === "md5" ? "by MD5" : row.link?.source }}</span
              >
            </td>
            <td class="rs-mono">{{ row.localVersion ? `v${row.localVersion}` : "—" }}</td>
            <td class="rs-mono" :class="{ 'is-update': row.state.kind === 'updateAvailable' }">{{ latest(row) }}</td>
            <td><StateChip v-if="row.state.kind === 'updateAvailable'" state="update" /></td>
            <td class="far__end">
              <RiftButton v-if="row.pageUrl" size="sm" @click="preflight.openUrl(row.pageUrl)"
                >Open on Nexus</RiftButton
              >
              <RiftButton size="sm" @click="far.unlink(row.packageId)">Unlink</RiftButton>
            </td>
          </tr>
        </tbody>
      </table>
    </section>

    <section class="far__sec">
      <header class="far__sechead">
        <h2 class="far__h rs-inscription">UNLINKED</h2>
        <RiftButton
          v-if="far.unlinked.value.length"
          size="sm"
          :busy="far.identifying.value"
          @click="far.identify(far.unlinked.value.map((r) => r.packageId))"
          >Identify all by MD5</RiftButton
        >
      </header>
      <p v-if="far.identifyNote.value" class="far__note">{{ far.identifyNote.value }}</p>
      <p v-if="!far.unlinked.value.length" class="far__dim">Every package knows where it came from.</p>
      <ul class="far__rows">
        <li v-for="row in far.unlinked.value" :key="row.packageId" class="far__row">
          <div class="far__rowtop">
            <span class="far__name">{{ row.packageName }}</span>
            <span class="far__dim rs-mono">{{ guess(row) }}</span>
            <RiftButton
              v-if="far.linkingId.value !== row.packageId"
              size="sm"
              variant="primary"
              @click="openLink(row.packageId)"
              >Link…</RiftButton
            >
          </div>
          <form v-if="far.linkingId.value === row.packageId" class="far__link" @submit.prevent="link(row.packageId)">
            <label class="far__field">
              <span class="rs-sr-only">Nexus URL or mod id for {{ row.packageName }}</span>
              <input
                v-model="linkText"
                type="text"
                placeholder="nexusmods.com/dragonsdogma2/mods/1234 or 1234"
                autofocus
              />
            </label>
            <RiftButton
              size="sm"
              variant="primary"
              type="submit"
              :disabled="!linkText.trim()"
              reason="Paste a link or a mod id first."
              >Link</RiftButton
            >
            <RiftButton size="sm" @click="far.linkingId.value = null">Cancel</RiftButton>
            <p v-if="far.linkFailure.value" class="far__err">
              {{ far.linkFailure.value.voice }} {{ far.linkFailure.value.empathy }}
            </p>
          </form>
        </li>
      </ul>
    </section>

    <section class="far__sec far__key">
      <header class="far__sechead">
        <h2 class="far__h rs-inscription">KEY</h2>
        <span class="far__chip" :class="`is-${keyState}`">{{ KEY_CHIP[keyState] }}</span>
        <span v-if="quotaLine" class="far__dim rs-mono">{{ quotaLine }}</span>
      </header>

      <p v-if="quotaLow" class="far__warn">
        {{ quotaLow }}
        <RiftButton size="sm" :disabled="quotaSpent" reason="No requests left this hour." @click="far.check()"
          >Check now anyway</RiftButton
        >
      </p>

      <FailureNotice v-if="far.keyFailure.value" :voiced="far.keyFailure.value" @recover="keyOpen = true" />
      <RiftInscription
        v-else-if="keyState === 'none'"
        size="md"
        body="Update checks run without a key: the stone asks Nexus anonymously. A personal API key is optional and only adds the account extras. It stays on this PC, in the Windows Credential Manager."
      />
      <p v-else-if="keyState === 'connected'" class="far__dim">
        Your key is stored in the Windows Credential Manager, never in a file.
      </p>

      <div class="far__acts">
        <RiftButton
          v-if="!keyOpen"
          size="sm"
          :variant="keyState === 'none' ? 'primary' : 'secondary'"
          @click="keyOpen = true"
          >{{ keyState === "none" ? "Paste key…" : "Replace key…" }}</RiftButton
        >
        <RiftButton size="sm" @click="preflight.openUrl(NEXUS_KEY_URL)">Where do I find it?</RiftButton>
        <RiftButton v-if="keyState === 'connected'" size="sm" variant="danger" @click="far.setKey(null)"
          >Forget key</RiftButton
        >
      </div>
      <form v-if="keyOpen" class="far__link" @submit.prevent="saveKey">
        <label class="far__field">
          <span class="rs-sr-only">Nexus personal API key</span>
          <input v-model="keyText" type="password" autocomplete="off" placeholder="Personal API key" />
        </label>
        <RiftButton
          size="sm"
          variant="primary"
          type="submit"
          :busy="far.keyBusy.value"
          :disabled="!keyText.trim()"
          reason="Paste the key first."
          >Save &amp; test</RiftButton
        >
        <RiftButton size="sm" @click="keyOpen = false">Cancel</RiftButton>
      </form>
    </section>
  </main>
</template>

<style scoped>
.far {
  flex: 1;
  min-width: 0;
  height: 100%;
  overflow-y: auto;
  padding: var(--rs-space-5) var(--rs-space-6) var(--rs-space-12);
  background: var(--rs-void);
}
.far__head {
  display: flex;
  justify-content: space-between;
  align-items: flex-end;
  gap: var(--rs-space-4);
  padding-bottom: var(--rs-space-4);
  border-bottom: 1px solid var(--rs-edge);
}
.far__title {
  margin: 0;
  font-size: var(--rs-text-xl);
  line-height: 1.15;
}
.far__sub {
  margin: 4px 0 0;
  color: var(--rs-muted);
}
.far__busy {
  color: var(--rs-rift);
}
.far__acts {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--rs-space-2);
}
.far__sec {
  margin-top: var(--rs-space-6);
  max-width: 880px;
}
.far__sechead {
  display: flex;
  align-items: center;
  gap: var(--rs-space-3);
  margin-bottom: var(--rs-space-2);
}
.far__sechead .far__h {
  margin: 0;
}
.far__h {
  margin: 0 0 var(--rs-space-2);
  font-size: 14px;
  color: var(--rs-muted);
  letter-spacing: 0.1em;
}
.far__dim {
  color: var(--rs-faint);
}
.far__name {
  color: var(--rs-text);
  font-weight: 500;
}
.far__table {
  width: 100%;
  border-collapse: collapse;
}
.far__table th {
  padding: 6px var(--rs-space-3);
  border-bottom: 1px solid var(--rs-edge);
  color: var(--rs-faint);
  font: 600 var(--rs-text-xs) var(--rs-font-mono);
  letter-spacing: 0.08em;
  text-align: left;
  text-transform: uppercase;
}
.far__table td {
  padding: 8px var(--rs-space-3);
  background: var(--rs-mist);
  border-bottom: 2px solid var(--rs-void);
  vertical-align: middle;
}
.far__end {
  text-align: right;
  white-space: nowrap;
}
.far__end :deep(.rs-btn + .rs-btn) {
  margin-left: var(--rs-space-2);
}
.is-update {
  color: var(--rs-verdant);
}
.far__rows {
  margin: 0;
  padding: 0;
  list-style: none;
}
.far__row {
  padding: 8px var(--rs-space-3);
  margin-bottom: 2px;
  background: var(--rs-mist);
}
.far__rowtop {
  display: flex;
  align-items: center;
  gap: var(--rs-space-3);
}
.far__rowtop .far__dim {
  flex: 1;
}
.far__link {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--rs-space-2);
  margin-top: var(--rs-space-2);
}
.far__field {
  flex: 1;
  min-width: 240px;
}
.far__field input {
  width: 100%;
  height: 28px;
  padding: 0 8px;
  border: 1px solid var(--rs-edgehi);
  background: var(--rs-well);
  color: var(--rs-text);
  font: 12px var(--rs-font-mono);
}
.far__err {
  flex-basis: 100%;
  margin: 0;
  color: var(--rs-ember);
}
.far__note {
  margin: 0 0 var(--rs-space-2);
  color: var(--rs-muted);
}
.far__warn {
  display: flex;
  align-items: center;
  gap: var(--rs-space-3);
  padding: var(--rs-space-2) 10px;
  border-left: 3px solid var(--rs-warn);
  background: var(--rs-mist);
}
.far__chip {
  padding: 2px 6px;
  border-radius: var(--rs-radius-chip);
  font: 600 var(--rs-text-xs) var(--rs-font-mono);
  letter-spacing: 0.04em;
  color: var(--rs-ongold);
}
.far__chip.is-connected {
  background: var(--rs-rift);
}
.far__chip.is-none {
  border: 1px solid var(--rs-edgehi);
  color: var(--rs-muted);
}
.far__chip.is-rejected {
  background: var(--rs-ember);
}
.far__chip.is-offline {
  background: var(--rs-warn);
}
</style>
