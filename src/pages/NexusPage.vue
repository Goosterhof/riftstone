<script setup lang="ts">
// §8.5 — NEXUS, cold: these live in another world. Three lean sections —
// Linked, Unlinked, Key. Update checks are anonymous (no key needed); a key
// is optional and only adds the account extras.
//
// v0.2 Find on Nexus: an unlinked row can ask Nexus about its name. What Nexus
// offers descends from a cold seam under the row (the Rift's own answer, §11.1,
// told small, drawn at once: §11.1 renders results without a staggered
// entrance): ranked as Nexus ranked them, the words that matched lit (colour
// AND underline, HitText). The player picks; Link this links and runs the
// check, so labels arrive at once. Nothing here moves the focus onto a button
// that sends: a press the player did not make must never send a name.
import { computed, nextTick, onMounted, ref } from "vue";
import type { NexusCandidate, UpdateRow } from "@/types/rift";
import { candidateLine, linkedNow, useFarWorld, type FindState } from "@/composables/useFarWorld";
import { useStone } from "@/composables/useStone";
import { usePreflight, NEXUS_KEY_URL } from "@/composables/usePreflight";
import { keyStateOf, type NexusKeyState } from "@/services/pending";
import type { RecoveryKind } from "@/services/voice";
import { formatAgo, formatClock, formatDate, plural } from "@/services/format";
import FailureNotice from "@/components/FailureNotice.vue";
import HitText from "@/components/HitText.vue";
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
  const meanwhile = q.hourlyResetMs
    ? ` Checks resume at ${formatClock(q.hourlyResetMs)}; until then the stone re-checks only the mods you open.`
    : " Until the hour turns, the stone re-checks only the mods you open.";
  return `${q.hourlyRemaining} requests left this hour.${meanwhile}`;
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

const findOf = (packageId: string): FindState | null => far.finds.value[packageId] ?? null;
const askingRow = (packageId: string): boolean => findOf(packageId)?.status === "asking";

const findAllLine = computed(() => {
  const p = far.findAllProgress.value;
  return p ? `asking Nexus about ${p.done} of ${p.total}…` : null;
});

/** Find…'s disclosure: everything a press may send (the spine's find_plan
 *  falls back to the first option's name when the package's finds nothing). */
const FIND_SENDS = "and, if that finds nothing, its first option's name";
const findTitle = (name: string): string => `Sends the name “${name}” to Nexus Mods (${FIND_SENDS}) to search for it`;
const FIND_ALL_TITLE = `Sends each unlinked package's name to Nexus Mods (${FIND_SENDS}), one at a time. Links nothing by itself.`;

const rowEl = (packageId: string): HTMLElement | null => document.getElementById(`far-row-${packageId}`);
const findButtonOf = (packageId: string): HTMLElement | null =>
  document.querySelector<HTMLElement>(`[data-find="${packageId}"]`);
/** The focus is the player's, on this row (or lost to <body>). */
const focusOnRow = (packageId: string): boolean => {
  const active = document.activeElement;
  return !active || active === document.body || (rowEl(packageId)?.contains(active) ?? false);
};

/** Open the row's Link… field and focus it. A field already open on this row
 *  keeps what the player typed; another row's open field is left alone. */
async function pasteField(packageId: string): Promise<void> {
  if (far.linkingId.value !== packageId) openLink(packageId);
  await nextTick();
  document.getElementById(`far-link-${packageId}`)?.focus();
}

/** One press of Find… (or Try again): when Nexus offers nothing, the Link…
 *  field opens as the way on, unless another row's field is open. The focus
 *  stays on the row's Find… through the ask (a busy button keeps it). */
async function findOne(packageId: string): Promise<void> {
  if (focusOnRow(packageId)) findButtonOf(packageId)?.focus();
  const findings = await far.find(packageId);
  if (!findings || findings.candidates.length) return;
  const other = far.linkingId.value !== null && far.linkingId.value !== packageId;
  if (other) return;
  if (far.linkingId.value !== packageId) openLink(packageId);
  if (!focusOnRow(packageId)) return;
  await nextTick();
  document.getElementById(`far-link-${packageId}`)?.focus();
}

/** A row failure's recovery, by kind: never a label that does something else. */
async function recoverFind(packageId: string, kind: RecoveryKind): Promise<void> {
  if (kind === "rescan") {
    far.setFind(packageId, null);
    await stone.startScan();
  } else if (kind === "replaceKey") keyOpen.value = true;
  else if (kind === "retry") await findOne(packageId);
  else far.setFind(packageId, null);
}

const termsOf = (terms: string): string[] => terms.split(/\s+/).filter(Boolean);

function alsoLinked(state: Extract<FindState, { status: "found" }>, c: NexusCandidate): string {
  return `already linked to ${plural(linkedNow(state, c), "other package")}`;
}

/** Link this: the row leaves UNLINKED, so the focus goes on down the list to
 *  the next unlinked ROW (it sends nothing: a Space or Enter there sends no
 *  name), or to the row's new line under LINKED. */
async function pick(packageId: string, candidate: NexusCandidate): Promise<void> {
  const ids = far.unlinked.value.map((r) => r.packageId);
  const next = ids[ids.indexOf(packageId) + 1] ?? null;
  if (!(await far.linkCandidate(packageId, candidate))) return;
  await nextTick();
  const target = (next && rowEl(next)) || document.getElementById(`far-linked-${packageId}`);
  target?.focus();
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
      <p v-if="!far.linked.value.length" class="far__dim">
        No package is linked yet. Find one below by its name, or link it by its Nexus URL.
      </p>
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
          <tr
            v-for="row in far.linked.value"
            :id="`far-linked-${row.packageId}`"
            :key="row.packageId"
            tabindex="-1"
            class="far__linkedrow"
          >
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
        <RiftButton
          v-if="far.unlinked.value.length"
          size="sm"
          :busy="far.findingAll.value"
          :title="FIND_ALL_TITLE"
          @click="far.findAll()"
          >Find all by name</RiftButton
        >
        <span class="far__busy rs-mono" role="status">{{ findAllLine ?? "" }}</span>
      </header>
      <p v-if="far.identifyNote.value" class="far__note">{{ far.identifyNote.value }}</p>
      <p class="far__note" role="status" data-find-note>{{ findAllLine ? "" : (far.findNote.value ?? "") }}</p>
      <p v-if="!far.unlinked.value.length" class="far__dim">Every package knows where it came from.</p>
      <ul class="far__rows" aria-label="Unlinked packages">
        <li
          v-for="row in far.unlinked.value"
          :id="`far-row-${row.packageId}`"
          :key="row.packageId"
          class="far__row"
          tabindex="-1"
          :aria-label="row.packageName"
        >
          <div class="far__rowtop">
            <span class="far__name">{{ row.packageName }}</span>
            <span class="far__dim rs-mono">{{ guess(row) }}</span>
            <RiftButton
              size="sm"
              :data-find="row.packageId"
              :busy="askingRow(row.packageId)"
              :disabled="far.findingAll.value"
              reason="Find all by name is asking Nexus about every row."
              :title="
                far.findingAll.value ? 'Find all by name is asking Nexus about every row.' : findTitle(row.packageName)
              "
              @click="findOne(row.packageId)"
              >Find…</RiftButton
            >
            <RiftButton
              v-if="far.linkingId.value !== row.packageId"
              size="sm"
              variant="primary"
              @click="openLink(row.packageId)"
              >Link…</RiftButton
            >
          </div>

          <div v-if="findOf(row.packageId)" class="far__find">
            <template v-for="state in [findOf(row.packageId)]" :key="state?.status">
              <p v-if="state?.status === 'asking'" class="far__asked rs-mono">
                asking Nexus about “{{ row.packageName }}”…
              </p>
              <div v-else-if="state?.status === 'failed'" class="far__findfail">
                <FailureNotice
                  :voiced="state.failure"
                  size="md"
                  @recover="(kind) => recoverFind(row.packageId, kind)"
                />
              </div>
              <template v-else-if="state?.status === 'found'">
                <p v-if="!state.findings.terms" class="far__none">
                  This package's name holds nothing to search for.
                  <template v-if="far.linkingId.value === row.packageId">Paste its Nexus page below instead.</template>
                  <RiftButton v-else size="sm" @click="pasteField(row.packageId)">Paste its Nexus page…</RiftButton>
                </p>
                <p v-else-if="!state.findings.candidates.length" class="far__none">
                  Nexus knows no Dragon's Dogma 2 mod named like “{{ state.findings.terms }}”.
                  <span v-if="far.linkingId.value === row.packageId" class="far__dim"
                    >Paste its Nexus page below if you know where it lives.</span
                  >
                  <RiftButton v-else size="sm" @click="pasteField(row.packageId)">Paste its Nexus page…</RiftButton>
                </p>
                <template v-else>
                  <p class="far__asked rs-mono">
                    <span class="far__askedk">ASKED</span> “{{ state.findings.terms }}”
                    <span class="far__answered">· {{ state.findings.candidates.slice(0, 5).length }} ANSWERED</span>
                  </p>
                  <ol class="far__cands" :aria-label="`Nexus candidates for ${row.packageName}`">
                    <li
                      v-for="(c, i) in state.findings.candidates.slice(0, 5)"
                      :key="c.modId"
                      class="far__cand"
                      :class="{ 'is-first': i === 0 }"
                    >
                      <span class="far__rank rs-mono" aria-hidden="true">{{ i + 1 }}</span>
                      <div class="far__cbody">
                        <div class="far__cline">
                          <button
                            type="button"
                            class="far__cname"
                            :title="`Open ${c.name} on Nexus Mods`"
                            @click="preflight.openUrl(c.pageUrl)"
                          >
                            <HitText :text="c.name" :tokens="termsOf(state.findings.terms)" />
                          </button>
                          <span v-if="c.adult" class="far__mark" title="Nexus marks this mod as adult content"
                            >ADULT</span
                          >
                          <span v-if="linkedNow(state, c) > 0" class="far__also">{{ alsoLinked(state, c) }}</span>
                        </div>
                        <p v-if="candidateLine(c)" class="far__meta">{{ candidateLine(c) }}</p>
                        <p v-if="c.summary" class="far__sum" :title="c.summary">{{ c.summary }}</p>
                      </div>
                      <RiftButton
                        size="sm"
                        variant="primary"
                        :busy="far.linkingCandidate.value === `${row.packageId}:${c.modId}`"
                        :disabled="far.linkingCandidate.value !== null"
                        reason="Another link is on its way to Nexus."
                        :aria-label="`Link ${row.packageName} to ${c.name}, Nexus mod ${c.modId}`"
                        @click="pick(row.packageId, c)"
                        >Link this</RiftButton
                      >
                    </li>
                  </ol>
                </template>
              </template>
            </template>
          </div>

          <form v-if="far.linkingId.value === row.packageId" class="far__link" @submit.prevent="link(row.packageId)">
            <label class="far__field">
              <span class="rs-sr-only">Nexus URL or mod id for {{ row.packageName }}</span>
              <input
                :id="`far-link-${row.packageId}`"
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

/* --- v0.2 Find on Nexus: the far world answers under the row ------------- */
.far__linkedrow:focus-visible,
.far__row:focus-visible {
  outline: 2px solid var(--rs-rift);
  outline-offset: -2px;
}
.far__find {
  margin-top: var(--rs-space-2);
}
.far__asked {
  margin: 0;
  color: var(--rs-faint);
  font-size: var(--rs-text-xs);
  letter-spacing: 0.04em;
}
.far__askedk {
  font-weight: 600;
  color: var(--rs-muted);
  letter-spacing: 0.12em;
}
.far__answered {
  color: var(--rs-rift);
  font-weight: 600;
}
.far__none {
  margin: 0;
  color: var(--rs-muted);
}
.far__none :deep(.rs-btn) {
  margin-left: var(--rs-space-2);
}
/* The candidates hang from a cold seam: a rift line drawn down the gutter, the
   ranks carved on it. Drawn at once, never staggered in (§11.1). */
.far__cands {
  position: relative;
  margin: 6px 0 2px;
  padding: 0;
  list-style: none;
}
.far__cands::before {
  content: "";
  position: absolute;
  top: 0;
  bottom: 0;
  left: 11px;
  z-index: 1;
  width: 2px;
  background: var(--rs-rift);
  opacity: 0.5;
  pointer-events: none;
}
.far__cand {
  position: relative;
  display: grid;
  grid-template-columns: 24px minmax(0, 1fr) auto;
  align-items: center;
  gap: var(--rs-space-3);
  padding: 8px var(--rs-space-3) 8px 0;
  background: var(--rs-void);
  border-top: 2px solid var(--rs-mist);
}
.far__rank {
  position: relative;
  z-index: 2;
  justify-self: center;
  min-width: 18px;
  padding: 1px 0;
  background: var(--rs-void);
  border: 1px solid var(--rs-edgehi);
  border-radius: var(--rs-radius-chip);
  color: var(--rs-faint);
  font-size: var(--rs-text-xs);
  font-weight: 600;
  text-align: center;
}
.far__cand.is-first .far__rank {
  border-color: var(--rs-rift);
  color: var(--rs-rift);
}
.far__cbody {
  min-width: 0;
}
.far__cline {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: var(--rs-space-2);
}
.far__cname {
  padding: 0;
  border: 0;
  background: none;
  color: var(--rs-text);
  font: 700 var(--rs-text-md) var(--rs-font-sans);
  line-height: 1.25;
  text-align: left;
  overflow-wrap: anywhere;
  cursor: pointer;
}
.far__cname:hover {
  text-decoration: underline;
  text-decoration-color: var(--rs-edgehi);
  text-underline-offset: 3px;
}
.far__cname:focus-visible {
  outline: 2px solid var(--rs-rift);
  outline-offset: 2px;
}
.far__mark {
  padding: 2px 6px;
  border: 1px solid var(--rs-edgehi);
  border-radius: var(--rs-radius-chip);
  color: var(--rs-muted);
  font: 600 var(--rs-text-xs) var(--rs-font-mono);
  letter-spacing: 0.04em;
}
.far__also {
  color: var(--rs-muted);
  font-size: var(--rs-text-sm);
  font-style: italic;
}
.far__meta,
.far__sum {
  margin: 2px 0 0;
  font-size: var(--rs-text-sm);
}
.far__meta {
  color: var(--rs-faint);
}
.far__sum {
  overflow: hidden;
  color: var(--rs-muted);
  text-overflow: ellipsis;
  white-space: nowrap;
}
@media (max-width: 720px) {
  .far__cand {
    grid-template-columns: 24px minmax(0, 1fr);
  }
  .far__cand > :deep(.rs-btn) {
    grid-column: 2;
    justify-self: start;
  }
  .far__rowtop {
    flex-wrap: wrap;
  }
}
</style>
