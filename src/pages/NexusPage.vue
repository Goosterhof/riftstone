<script setup lang="ts">
// §8.5 / §16.3 — THE FAR WORLD, cold: these live in another world. One scroll,
// six lean sections in the order R16.16 rules: ACCOUNT (what the key is and
// does), DOWNLOADS (only while a download row exists), UPDATES, UNLINKED,
// TRACKED (only with a connected key), FIND ON NEXUS. Update checks, browsing
// and a mod's files are anonymous; the key adds the account, downloads,
// tracking and endorsing. A mod's files replace the scroll (R16.21) and hand
// the same scroll back.
//
// v0.2 Find on Nexus: an unlinked row can ask Nexus about its name. What Nexus
// offers descends from a cold seam under the row (the Rift's own answer, §11.1,
// told small, drawn at once: §11.1 renders results without a staggered
// entrance): ranked as Nexus ranked them, the words that matched lit (colour
// AND underline, HitText). The player picks; Link this links and runs the
// check, so labels arrive at once. Nothing here moves the focus onto a button
// that sends: a press the player did not make must never send a name.
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { LibraryPackage, NexusCandidate, NexusFileView, UpdateRow } from "@/types/rift";
import { HELD_WHILE_DOWNLOADING, landedPath, useDownloads } from "@/composables/useDownloads";
import { useNexusAccount } from "@/composables/useNexusAccount";
import { candidateLine, linkedNow, useFarWorld, type FindState } from "@/composables/useFarWorld";
import { useStone } from "@/composables/useStone";
import { usePreflight, NEXUS_KEY_URL } from "@/composables/usePreflight";
import { keyStateOf, type NexusKeyState } from "@/services/pending";
import type { RecoveryKind } from "@/services/voice";
import { formatAgo, formatDate, plural } from "@/services/format";
import { FAR_ASK_DOOR, farAskLine, farSubRow, linkSourceText, versionName, type FarSubRow } from "@/services/versions";
import { nodeKey, useLibrary } from "@/composables/useLibrary";
import { useVersions } from "@/composables/useVersions";
import FailureNotice from "@/components/FailureNotice.vue";
import HitText from "@/components/HitText.vue";
import RiftButton from "@/components/RiftButton.vue";
import RiftInscription from "@/components/RiftInscription.vue";
import StateChip from "@/components/StateChip.vue";
import AccountCard from "@/components/AccountCard.vue";
import DownloadShelf from "@/components/DownloadShelf.vue";
import FarModCard from "@/components/FarModCard.vue";
import ModFiles from "@/components/ModFiles.vue";
import NexusBrowse from "@/components/NexusBrowse.vue";
import NxmSwitch from "@/components/NxmSwitch.vue";
import StandingControls from "@/components/StandingControls.vue";

const far = useFarWorld();
const stone = useStone();
const preflight = usePreflight();

const linkText = ref("");
const keyText = ref("");
/** The ACCOUNT section's key form: the far world's own, which every key door
 *  in the window opens (useFarWorld().openKeyForm). */
const keyOpen = far.keyFormOpen;

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
  return ms === null ? "updates never checked" : `updates checked ${formatAgo(ms)}`;
});

const summary = computed(() => stone.overview.value?.nexus ?? null);
/** What the chip says: the spine's standing, and on top of it what Nexus said
 *  about this session's paste. Only a paste Nexus refused reads REJECTED; one
 *  that never reached Nexus reads OFFLINE, and a rationed one changes nothing
 *  (the spine stored nothing and kept the key it held). */
const keyState = computed<NexusKeyState>(() =>
  summary.value ? keyStateOf(summary.value, far.keyRefused.value, far.offline.value || far.keyUnreached.value) : "none",
);
/** The spine holds a key (whatever Nexus last said of it): there is one to check, replace or remove. */
const keyStored = computed(() => (summary.value?.key ?? "none") !== "none");

const KEY_CHIP: Record<NexusKeyState, string> = {
  connected: "CONNECTED",
  unchecked: "NOT CHECKED YET",
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

/** The key's v1 budget, running low. It governs only the key's own calls:
 *  the anonymous update check never waits on it (WINDOW-CONTRACT
 *  get_nexus_quota), so nothing here gates Check now. */
const quotaLow = computed(() => {
  const q = far.quota.value;
  if (!q || q.hourlyRemaining > 20) return null;
  const left = q.hourlyRemaining === 0 ? "No requests" : plural(q.hourlyRemaining, "request");
  return `${left} left this hour for downloads, tracking and endorsing; update checks don't use them.`;
});

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
  } else if (kind === "replaceKey") await far.openKeyForm();
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

// --- v0.3 ----------------------------------------------------------------
const dl = useDownloads();

// --- v0.5 (#00015 R18.8): one row per family, the older versions on a line under it
const library = useLibrary();
const versions = useVersions();
/** The older versions' installs, read from the library as the lead's are: an
 *  install or an uninstall reads the library again (rift://game-changed), never
 *  get_updates, so the row's own `older` would keep the counts of the last check. */
function olderOf(row: UpdateRow, lead: LibraryPackage | undefined): UpdateRow["older"] {
  const family = lead?.family;
  return family && family.lead === row.packageId && family.older.length ? family.older : row.older;
}
function olderLine(row: UpdateRow): FarSubRow | null {
  if (!row.older.length) return null;
  const lead = library.packages.value.find((p) => p.package.id === row.packageId);
  const leadName = versionName(lead?.family?.version ?? row.localVersion, lead?.package.fileName ?? row.packageName);
  return farSubRow(olderOf(row, lead), leadName, (lead?.installed ?? 0) > 0);
}
function olderDoor(row: UpdateRow, line: FarSubRow): void {
  if (line.door === "versions") {
    void versions.showVersions(row.packageId);
    return;
  }
  const first = library.packages.value.find((p) => p.package.id === row.packageId)?.tree[0];
  void versions.showInRift(row.packageId, first ? nodeKey(first, row.packageId) : null);
}
/** R18.18.1: the stone's question under the questioned package's own row (no yes or no here). */
function askLine(row: UpdateRow): string | null {
  const q = library.packages.value.find((p) => p.package.id === row.packageId)?.versionQuestion;
  return q ? farAskLine(q) : null;
}
const nexus = useNexusAccount();
const scroller = ref<HTMLElement | null>(null);

/** R16.16's index line: the sections on screen now, in the ruled order. */
const sections = computed(() =>
  [
    { id: "far-account", label: "Account", on: true },
    { id: "far-downloads", label: "Downloads", on: dl.downloads.value.length > 0 },
    { id: "far-updates", label: "Updates", on: true },
    { id: "far-unlinked", label: "Unlinked", on: true },
    { id: "far-tracked", label: "Tracked", on: nexus.connected.value },
    { id: "far-find", label: "Find", on: true },
  ].filter((s) => s.on),
);
const current = ref("far-account");
/** The section a jump went to: kept marked while its head is on screen, even
 *  when the scroll cannot bring that head to the top (the last sections). */
let jumpedTo: string | null = null;

function jump(id: string): void {
  current.value = id;
  jumpedTo = id;
  document.getElementById(id)?.scrollIntoView?.({ block: "start" });
}

/** A section's head is inside the scroll's window. */
function headInView(id: string, view: DOMRect): boolean {
  const top = document.getElementById(id)?.getBoundingClientRect().top;
  return top !== undefined && top >= view.top - 1 && top < view.bottom;
}

/** The scroll stands at its very end: the last section is the one being read. */
const atBottom = (box: HTMLElement): boolean =>
  box.scrollHeight > box.clientHeight + 1 && box.scrollTop + box.clientHeight >= box.scrollHeight - 1;

/** The last section whose head has passed `line`. */
function passed(line: number): string {
  let at = sections.value[0]?.id ?? "far-account";
  for (const s of sections.value) {
    const el = document.getElementById(s.id);
    if (el && el.getBoundingClientRect().top <= line) at = s.id;
  }
  return at;
}

/** The current section: the one a jump went to while its head shows; at the
 *  end of the scroll, the last; else the last whose head passed the top. */
function onScroll(): void {
  const box = scroller.value;
  if (!box) return;
  const view = box.getBoundingClientRect();
  if (jumpedTo && headInView(jumpedTo, view)) {
    current.value = jumpedTo;
    return;
  }
  jumpedTo = null;
  current.value = atBottom(box) ? (sections.value.at(-1)?.id ?? "far-account") : passed(view.top + 48);
}

/** R16.21: a mod's files replace the scroll; back returns to the exact place. */
const filesOf = ref<{ modId: number; name: string | null; forPackageId: string | null } | null>(null);
let savedScroll = 0;
let opener: HTMLElement | null = null;

function openFiles(modId: number, name: string | null, forPackageId: string | null = null): void {
  savedScroll = scroller.value?.scrollTop ?? 0;
  opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  filesOf.value = { modId, name, forPackageId };
  if (scroller.value) scroller.value.scrollTop = 0;
}

async function closeFiles(): Promise<void> {
  filesOf.value = null;
  await nextTick();
  if (scroller.value) scroller.value.scrollTop = savedScroll;
  if (opener?.isConnected) opener.focus({ preventScroll: true });
  opener = null;
}

const newerOf = (row: UpdateRow): NexusFileView | null =>
  row.state.kind === "updateAvailable" ? row.state.newer : null;

/** R16.23: premium downloads in the stone; free goes through the file's page. */
function downloadLabel(newer: NexusFileView): string {
  if (!nexus.premium.value) return "Get it on Nexus";
  return newer.version ? `Download v${newer.version.replace(/^v/i, "")}` : "Download";
}

async function downloadUpdate(row: UpdateRow, newer: NexusFileView): Promise<void> {
  if (!row.link) return;
  await dl.start(row.link.modId, newer.fileId, row.packageId);
}

/** Chaos D2: the row's own download (or one of the same file) is on its way.
 *  The spine would answer a press with that same row, so the door is held:
 *  drawn, reachable, its reason said (RiftButton refused), never hidden. */
function heldFor(row: UpdateRow, newer: NexusFileView): boolean {
  return row.link !== null && dl.unfinishedFor(row.packageId, row.link.modId, newer.fileId) !== null;
}

/** The update row's own download, once landed: its line and its door. */
function landedFor(row: UpdateRow) {
  const d = dl.forPackage(row.packageId);
  return d && d.phase === "landed" ? d : null;
}

/** R16.23's landed line; a file that is no archive (no package) is in the
 *  mods folder too, but the Rift never lists it, so it is never sent there. */
function landedLine(row: UpdateRow): string {
  const d = landedFor(row);
  const v = d?.version ?? newerOf(row)?.version ?? null;
  const what = v ? `v${v.replace(/^v/i, "")}` : "The new file";
  if (d && d.packageId === null)
    return `${what} is in your mods folder, but it is no archive, so the Rift does not list it.`;
  return `${what} is in your mods folder. Install it from the Rift.`;
}

async function removeKey(): Promise<void> {
  await far.setKey(null);
  keyOpen.value = false;
}

/** Every key door on this page (Paste key…, Replace key…, a notice's Add key…,
 *  the shelf's) is the window's one key door: the ACCOUNT form open, its
 *  password field focused. */
async function replaceKey(): Promise<void> {
  await far.openKeyForm();
}

// A key door pressed while a mod's FILES fills the page (its own Add key…, or
// a refused link's toast): the files give way to the far world's scroll, so
// the form the door opens is there to take the focus.
watch(
  () => far.keyDoorPresses.value,
  () => {
    filesOf.value = null;
  },
);

// The standings are the connected account's: a key that changes hands (even
// from one connected account to another) forgets the old account's and reads
// the new one's; none are shown without a connected key.
watch(
  () => nexus.owner.value,
  () => void nexus.followAccount(),
);

/** The ACCOUNT card is on the page (it carries the nxm switch as its last row). */
const cardShown = computed(
  () => !far.keyFailure.value && keyState.value === "connected" && nexus.account.value !== null,
);

// R16.22: an untrack in TRACKED takes its card away under the focus. The focus
// goes on to the next card's toggle, else the previous card's, else the
// section's head; a focus that was anywhere else stays where it was.
watch(
  () => nexus.standings.value?.tracked.map((c) => c.modId) ?? [],
  async (now, before) => {
    const gone = before.findIndex((id) => !now.includes(id));
    if (gone < 0) return;
    const active = document.activeElement;
    const held = active instanceof HTMLElement && active.closest(`#far-tracked [data-mod="${before[gone]}"]`) !== null;
    await nextTick();
    if (!held) return;
    const cards = [...document.querySelectorAll<HTMLElement>("#far-tracked [data-mod]")];
    const card = cards.at(gone) ?? (gone > 0 ? cards.at(gone - 1) : undefined);
    const toggle = card?.querySelector<HTMLElement>("[aria-pressed]");
    (toggle ?? document.getElementById("far-h-tracked"))?.focus({ preventScroll: true });
  },
);

onMounted(() => {
  void far.load();
  void dl.load();
  void nexus.followAccount();
});
// The form is the page's: leaving the far world closes it, as it always did.
onBeforeUnmount(() => {
  keyOpen.value = false;
});
</script>

<template>
  <main ref="scroller" class="far" aria-label="The far world — Nexus Mods" @scroll.passive="onScroll">
    <header class="far__head">
      <div>
        <h1 class="far__title rs-inscription">THE FAR WORLD</h1>
        <p class="far__sub">
          Your account, downloads, updates, tracked mods and Find on Nexus ·
          <span class="rs-mono">{{ lastCheck }}</span>
          <template v-if="far.checking.value">
            · <span class="rs-mono far__busy">{{ progressLine }}</span></template
          >
        </p>
      </div>
      <div class="far__acts">
        <StateChip v-if="far.updateCount.value" state="update" />
        <!-- anonymous: never gated by the key's v1 quota (WINDOW-CONTRACT get_nexus_quota) -->
        <RiftButton variant="primary" :busy="far.checking.value" @click="far.check()">Check now</RiftButton>
      </div>
    </header>

    <ModFiles
      v-if="filesOf"
      :key="filesOf.modId"
      :mod-id="filesOf.modId"
      :name="filesOf.name"
      :for-package-id="filesOf.forPackageId"
      @back="closeFiles"
    />

    <div v-show="!filesOf" class="far__body">
      <nav class="far__index" aria-label="Sections of the far world">
        <template v-for="(sec, i) in sections" :key="sec.id"
          ><span v-if="i" aria-hidden="true"> · </span
          ><a
            :href="`#${sec.id}`"
            :aria-current="current === sec.id ? 'location' : undefined"
            @click.prevent="jump(sec.id)"
            >{{ sec.label }}</a
          ></template
        >
      </nav>

      <FailureNotice
        v-if="far.checkFailure.value"
        :voiced="checkNotice ?? far.checkFailure.value"
        :busy="far.checking.value"
        @recover="far.check()"
      />

      <section id="far-account" class="far__sec far__key">
        <header class="far__sechead">
          <h2 class="far__h rs-inscription">ACCOUNT</h2>
          <span class="far__chip" :class="`is-${keyState}`">{{ KEY_CHIP[keyState] }}</span>
          <span v-if="quotaLine && keyState !== 'connected'" class="far__dim rs-mono">{{ quotaLine }}</span>
        </header>

        <p v-if="quotaLow" class="far__warn">{{ quotaLow }}</p>

        <FailureNotice v-if="far.keyFailure.value" :voiced="far.keyFailure.value" @recover="replaceKey()" />
        <AccountCard v-else-if="keyState === 'connected'" @replace-key="replaceKey()" @remove-key="removeKey()" />
        <RiftInscription
          v-else-if="keyState === 'none'"
          inscription="THE FAR WORLD IS SILENT"
          size="md"
          body="Add your Nexus Mods personal API key and the stone will show your account, and download, track and endorse mods for you. It stays on this PC. Update checks and browsing Nexus work without it."
        />
        <RiftInscription
          v-else-if="keyState === 'rejected'"
          inscription="THE FAR WORLD REFUSES"
          tone="ember"
          size="md"
          body="Nexus didn't accept that key: it may have been regenerated. Nothing else changed."
        />
        <p v-else class="far__note" data-key-unchecked>
          The stone holds a key but hasn't checked it with Nexus this session.
        </p>
        <FailureNotice
          v-if="keyState !== 'connected' && nexus.accountFailure.value"
          :voiced="nexus.accountFailure.value"
          :busy="nexus.refreshing.value"
          @recover="(kind) => (kind === 'replaceKey' ? replaceKey() : void nexus.refreshAccount())"
        />

        <!-- what the spine STORES decides the doors: a paste that never reached
             Nexus stored nothing, so there is nothing to check, replace or remove -->
        <div v-if="keyState !== 'connected'" class="far__acts">
          <RiftButton
            v-if="keyStored && (keyState === 'unchecked' || keyState === 'offline')"
            size="sm"
            variant="primary"
            :busy="nexus.refreshing.value"
            @click="nexus.refreshAccount()"
            >Check again</RiftButton
          >
          <RiftButton
            v-if="!keyOpen"
            size="sm"
            :variant="!keyStored || keyState === 'rejected' ? 'primary' : 'secondary'"
            @click="replaceKey()"
            >{{ keyStored ? "Replace key…" : "Paste key…" }}</RiftButton
          >
          <RiftButton size="sm" @click="preflight.openUrl(NEXUS_KEY_URL)">Where do I find it?</RiftButton>
          <RiftButton v-if="keyStored" size="sm" variant="danger" @click="removeKey()">Remove key</RiftButton>
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
        <!-- Without the ACCOUNT card the switch stays in the section: while the
             stone answers nxm:// links, turning it off must stay within reach
             whatever the key's standing (ruling 5: the protocol is given back). -->
        <NxmSwitch v-if="!cardShown" />
      </section>

      <DownloadShelf class="far__sec" @replace-key="replaceKey()" />

      <section id="far-updates" class="far__sec">
        <h2 class="far__h rs-inscription">UPDATES</h2>
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
            <template v-for="row in far.linked.value" :key="row.packageId">
              <tr :id="`far-linked-${row.packageId}`" tabindex="-1" class="far__linkedrow">
                <td>
                  <span class="far__name">{{ row.packageName }}</span>
                  <span class="far__dim rs-mono">
                    mod {{ row.link?.modId }} · {{ row.link ? linkSourceText(row.link.source) : "" }}</span
                  >
                </td>
                <td class="rs-mono">{{ row.localVersion ? `v${row.localVersion}` : "—" }}</td>
                <td class="rs-mono" :class="{ 'is-update': row.state.kind === 'updateAvailable' }">
                  {{ latest(row) }}
                </td>
                <td><StateChip v-if="row.state.kind === 'updateAvailable'" state="update" /></td>
                <td class="far__end">
                  <div class="far__doors">
                    <template v-for="newer in [newerOf(row)]" :key="newer?.fileId ?? 0">
                      <RiftButton
                        v-if="newer && row.link && nexus.connected.value && !landedFor(row)"
                        size="sm"
                        variant="primary"
                        :busy="dl.busy.value === `start:${row.link.modId}:${newer.fileId}`"
                        :refused="heldFor(row, newer)"
                        :reason="heldFor(row, newer) ? HELD_WHILE_DOWNLOADING : null"
                        :data-update-download="row.packageId"
                        @click="downloadUpdate(row, newer)"
                        >{{ downloadLabel(newer) }}</RiftButton
                      >
                    </template>
                    <RiftButton
                      v-if="row.link"
                      size="sm"
                      :aria-label="`Files of ${row.packageName} on Nexus`"
                      @click="openFiles(row.link.modId, row.modName ?? row.packageName, row.packageId)"
                      >Files</RiftButton
                    >
                    <RiftButton v-if="row.pageUrl" size="sm" @click="preflight.openUrl(row.pageUrl)"
                      >Open on Nexus</RiftButton
                    >
                    <RiftButton size="sm" @click="far.unlink(row.packageId)">Unlink</RiftButton>
                  </div>
                </td>
              </tr>
              <!-- R16.22: Endorse / Abstain live in the mod's FILES, never on its update row -->
              <tr v-if="landedFor(row)" class="far__subrow">
                <td colspan="5">
                  <p v-if="landedFor(row)" class="far__landed">
                    {{ landedLine(row) }}
                    <RiftButton
                      v-if="landedFor(row)?.packageId"
                      size="sm"
                      variant="primary"
                      @click="dl.showInRift(landedFor(row)?.packageId ?? '')"
                      >Show in the Rift</RiftButton
                    >
                    <RiftButton
                      v-else-if="landedPath({ landedAs: landedFor(row)?.landedAs ?? null })"
                      size="sm"
                      @click="dl.showInFolder({ landedAs: landedFor(row)?.landedAs ?? null })"
                      >Show in folder</RiftButton
                    >
                  </p>
                </td>
              </tr>
              <tr
                v-for="line in [olderLine(row)].filter((l): l is FarSubRow => l !== null)"
                :key="`older-${row.packageId}`"
                class="far__subrow"
              >
                <td colspan="5">
                  <p class="far__older" :class="`is-${line.tone}`">
                    <span class="far__older-glyph" aria-hidden="true">↳</span>
                    <span>{{ line.text }}</span>
                    <RiftButton
                      size="sm"
                      :variant="line.door === 'rift' ? 'primary' : 'secondary'"
                      @click="olderDoor(row, line)"
                      >{{ line.doorLabel }}</RiftButton
                    >
                  </p>
                </td>
              </tr>
              <tr
                v-for="line in [askLine(row)].filter((l): l is string => l !== null)"
                :key="`ask-${row.packageId}`"
                class="far__subrow"
              >
                <td colspan="5">
                  <p class="far__older far__ask">
                    <span class="far__older-glyph" aria-hidden="true">↳</span>
                    <span>{{ line }}</span>
                    <RiftButton size="sm" @click="versions.showQuestion(row.packageId)">{{ FAR_ASK_DOOR }}</RiftButton>
                  </p>
                </td>
              </tr>
            </template>
          </tbody>
        </table>
      </section>

      <section id="far-unlinked" class="far__sec">
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
                  far.findingAll.value
                    ? 'Find all by name is asking Nexus about every row.'
                    : findTitle(row.packageName)
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
                    <template v-if="far.linkingId.value === row.packageId"
                      >Paste its Nexus page below instead.</template
                    >
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

      <section v-if="nexus.connected.value" id="far-tracked" class="far__sec">
        <h2 id="far-h-tracked" class="far__h rs-inscription" tabindex="-1">TRACKED ON NEXUS</h2>
        <FailureNotice
          v-if="nexus.standingsFailure.value"
          :voiced="nexus.standingsFailure.value"
          :busy="nexus.standingsLoading.value"
          @recover="nexus.loadStandings(true)"
        />
        <p v-else-if="nexus.standingsLoading.value && !nexus.standings.value" class="far__dim rs-mono" role="status">
          asking Nexus what you track…
        </p>
        <p v-else-if="nexus.standings.value && !nexus.standings.value.tracked.length" class="far__dim">
          You don't track any Dragon's Dogma 2 mods on Nexus yet.
        </p>
        <ul v-if="nexus.standings.value?.tracked.length" class="far__rows" aria-label="Mods you track on Nexus">
          <FarModCard
            v-for="m in nexus.standings.value.tracked"
            :key="m.modId"
            :mod="m"
            @files="openFiles(m.modId, m.name)"
          >
            <RiftButton size="sm" @click="openFiles(m.modId, m.name)">Files</RiftButton>
            <StandingControls :mod-id="m.modId" :name="m.name" :endorse="false" />
          </FarModCard>
        </ul>
      </section>

      <NexusBrowse class="far__sec" @files="(m) => openFiles(m.modId, m.name)" />
    </div>
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
}
/* the row's doors: one line at the wide posture, wrapping at the narrow one
   (1024x680) so the table gives its width to the names, never sideways */
.far__doors {
  display: flex;
  justify-content: flex-end;
  gap: var(--rs-space-2);
}
.far__doors :deep(.rs-btn) {
  white-space: nowrap;
}
@media (max-width: 1199px) {
  .far__doors {
    flex-wrap: wrap;
  }
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
/* v0.3: a stored key Nexus has not answered for yet this session */
.far__chip.is-unchecked {
  border: 1px solid var(--rs-rift);
  color: var(--rs-rift);
}

/* --- v0.3: the index line, the update row's second line --------------- */
.far__index {
  margin-top: var(--rs-space-3);
  color: var(--rs-faint);
  font-size: var(--rs-text-sm);
}
.far__index a {
  color: var(--rs-muted);
  text-decoration: none;
}
.far__index a:hover,
.far__index a[aria-current] {
  text-decoration: underline;
  text-underline-offset: 3px;
}
.far__index a[aria-current] {
  color: var(--rs-text);
}
.far__index a:focus-visible {
  outline: 2px solid var(--rs-rift);
  outline-offset: 2px;
}
.far__table .far__subrow td {
  padding-top: 0;
}
.far__landed {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--rs-space-3);
  margin: 0 0 var(--rs-space-2);
  color: var(--rs-text);
}
.far__linkedrow:has(+ .far__subrow) td,
.far__subrow:has(+ .far__subrow) td {
  border-bottom: 0;
}
/* R18.18.1: the stone's question, Alegreya 14 muted (8.84 on void) */
.far__older.far__ask {
  color: var(--rs-muted);
}
/* R18.8: the older versions' line, the row's own (↳), never a column */
.far__older {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--rs-space-2) var(--rs-space-3);
  margin: 0 0 var(--rs-space-2);
  padding-left: var(--rs-space-4);
  font-size: var(--rs-text-sm);
}
.far__older-glyph {
  color: var(--rs-faint);
}
.far__older.is-warn {
  color: var(--rs-warn);
}
.far__older.is-text {
  color: var(--rs-text);
}
.far__older.is-faint {
  color: var(--rs-faint);
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
