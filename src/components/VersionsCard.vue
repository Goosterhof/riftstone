<script setup lang="ts">
// v0.5 — THE VERSIONS card (#00015 R18.4) and the keep-newest plan it opens
// (R18.10, THE RETURN — OLDER VERSIONS), in Your World on the hearth: the card
// is about what is in your game. One row per version, newest first; the doors
// that clean up live here, never on a tree row (Bold Choice 11). Esc or Cancel
// returns the pane to what it held before. Nothing moves, nothing is drawn for
// a move: the row is simply not there at the next render (Gold #2).
//
// v0.5 (#00015 R18.18, Bold Choice #23): the question is asked in the shape of
// its answer. When the stone asks whether a package is a version of a pinned
// file, this card draws the file's rows as a yes would make them, the
// questioned package beneath them with the evidence, and two doors; a yes takes
// the `?` away at the library's next answer and nothing else moves. A joined
// member carries `by your word` and `Take it back`.
import { computed, onBeforeUnmount, onMounted } from "vue";
import { useNavigation } from "@/composables/useNavigation";
import { useVersions } from "@/composables/useVersions";
import { useUndercroft } from "@/composables/useUndercroft";
import { plural } from "@/services/format";
import type { RecoveryKind } from "@/services/voice";
import {
  ASKING_HEAD,
  BY_YOUR_WORD,
  DECLINING_HELD,
  JOINING_HELD,
  JOIN_DOOR,
  KEEP_APART_DOOR,
  MANUAL_NOTE,
  NEXUS_CANT_TELL,
  PICK_HELD,
  PICK_LEGEND,
  TAKE_IT_BACK,
  askedKind,
  askedOf,
  askedQuestion,
  askedQuestionOpen,
  askedRowLabel,
  candidateLabel,
  evidenceLine,
  KEEP_NEWEST_HEAD,
  NEWEST_BADGE,
  PUT_AWAY_ROW,
  VERSIONS_HEAD,
  backLine,
  keepButton,
  keepsLine,
  keptSummary,
  memberWorldLine,
  newestNotInstalled,
  putAwayFailedTail,
  putAwayLine,
  removalsText,
  versionLabel,
  versionName,
} from "@/services/versions";
import FailureNotice from "./FailureNotice.vue";
import RiftButton from "./RiftButton.vue";

const versions = useVersions();
const undercroft = useUndercroft();
const nav = useNavigation();

interface MemberRow {
  packageId: string;
  version: string;
  fileName: string;
  newest: boolean;
  installed: number;
  under: string | null;
  canPutAway: boolean;
  /** R18.18.3: joined by the player's word (only such a row offers `Take it back`). */
  joined: boolean;
}

const members = computed<MemberRow[]>(() => {
  const entry = versions.entry.value;
  if (!entry) return [];
  const list = [
    // R18.18.2: a file asked about may stand alone, in no family yet: its link's version
    {
      pkg: entry.lead,
      version: entry.lead.family?.version ?? entry.lead.package.nexus?.version ?? null,
      installed: entry.lead.installed,
    },
    ...entry.older.map((o) => ({ pkg: o.pkg, version: o.version.version, installed: o.version.installed })),
  ];
  return list.map((m, i) => {
    const newerInstalled = list.slice(0, i).find((x) => x.installed > 0);
    return {
      packageId: m.pkg.package.id,
      version: versionName(m.version, m.pkg.package.fileName),
      fileName: m.pkg.package.fileName,
      newest: i === 0,
      installed: m.installed,
      under: newerInstalled ? versionName(newerInstalled.version, newerInstalled.pkg.package.fileName) : null,
      canPutAway: i > 0 && m.installed === 0,
      joined: m.pkg.package.nexus?.source === "joined",
    };
  });
});

/** The version keep-newest would keep: the newest with an install, else the lead. */
const keptIndex = computed(() =>
  Math.max(
    0,
    members.value.findIndex((m) => m.installed > 0),
  ),
);
/** The contract's refusal reads the same: something older than the kept version must be there. */
const canKeepNewest = computed(() => members.value.length > 1 && keptIndex.value < members.value.length - 1);
const notInstalledLine = computed(() => {
  const [lead, ...older] = members.value;
  const installedOlder = older.find((m) => m.installed > 0);
  return lead && lead.installed === 0 && installedOlder
    ? newestNotInstalled(lead.version, installedOlder.version)
    : null;
});

const inPlan = computed(() => versions.stage.value !== "card");

// --- R18.18.2: the question form ------------------------------------------------
const asking = computed(() => versions.asking.value !== null);
const manual = computed(() => versions.asking.value?.manual ?? false);
const asked = computed(() => versions.askedPackage.value);
const question = computed(() => asked.value?.versionQuestion ?? null);
const kind = computed(() => (asked.value ? askedKind(asked.value) : "folder"));
/** The file the question names: `v1.3`, else its file name in mono; the manual pick's own. */
const of = computed<{ text: string; mono: boolean } | null>(() => {
  if (question.value) return askedOf(question.value);
  const chosen = versions.candidates.value.find((c) => c.packageId === versions.picked.value);
  if (!chosen) return null;
  return askedOf({ ofVersion: chosen.version, ofFileName: chosen.fileName });
});
/** The questioned package's row: drawn last, claiming no order (the spine places it after a yes). */
const askedRow = computed(() => {
  const pkg = asked.value;
  if (!pkg) return null;
  const above = members.value.find((m) => m.installed > 0);
  return {
    label: askedRowLabel(kind.value),
    fileName: pkg.package.fileName,
    installed: pkg.installed,
    under: pkg.installed > 0 && above ? above.version : null,
  };
});
const evidence = computed(() =>
  question.value && of.value ? `${evidenceLine(question.value, of.value.text)} ${NEXUS_CANT_TELL}` : MANUAL_NOTE,
);
const choosing = computed(() => manual.value && versions.candidates.value.length > 1);
const yesHeld = computed<string | null>(() => {
  if (versions.word.value === "declining") return DECLINING_HELD;
  if (manual.value && !versions.picked.value) return PICK_HELD;
  return null;
});
const name = computed(() => versions.entry.value?.lead.package.displayName ?? asked.value?.package.displayName ?? "");
const plan = computed(() => versions.keepPlan.value);
const keeps = computed(() => (plan.value?.keeps ? versions.nameOfFile(plan.value.keeps) : null));
const removing = computed(() => (plan.value?.uninstalls ?? []).reduce((n, u) => n + u.removals, 0));
const keptRows = computed(() =>
  (versions.kept.value?.putAway ?? []).map((view) => ({
    view,
    name: versions.keptNames.value.get(view.fileName) ?? view.fileName,
  })),
);

function cancel(): void {
  if (versions.stage.value === "keepPlan" || versions.stage.value === "failed") versions.backToCard();
  else versions.close();
}

/** Esc returns the pane to what it held before (R18.4), wherever the focus is
 *  (`Enter` on a version row leaves it on the tree). Caught ahead of the
 *  window's own Esc, which would close the option under the card too; the
 *  lightbox, when open, is the innermost thing and keeps its Esc. */
function onEsc(event: KeyboardEvent): void {
  if (event.key !== "Escape" || event.defaultPrevented || nav.lightbox.value) return;
  event.preventDefault();
  event.stopPropagation();
  if (versions.stage.value === "keeping") return;
  cancel();
}
onMounted(() => window.addEventListener("keydown", onEsc, { capture: true }));
onBeforeUnmount(() => window.removeEventListener("keydown", onEsc, { capture: true }));

async function recover(kind: RecoveryKind): Promise<void> {
  if (kind === "retry") await versions.beginKeepNewest();
  else if (kind === "copyDetails") await versions.copy(versions.failure.value?.details ?? "");
  else versions.backToCard();
}

/** R18.5's tail for keep-newest: the versions it meant to put away and could not, named together. */
const stayedNames = computed(() => {
  const landed = new Set((versions.kept.value?.putAway ?? []).map((v) => v.fileName));
  const left = (plan.value?.putsAway ?? []).filter((f) => !landed.has(f));
  const names = left.map((f) => versions.keptNames.value.get(f) ?? f);
  return names.length ? names.join(" and ") : "the older version";
});

const lineFailure = (landedAs: string) => {
  const state = versions.lineState(landedAs);
  return typeof state === "object" ? state : null;
};
</script>

<template>
  <section class="vc" :aria-label="inPlan ? KEEP_NEWEST_HEAD : VERSIONS_HEAD">
    <template v-if="!inPlan">
      <header class="vc__head">
        <h3 class="vc__title rs-inscription">{{ asking ? ASKING_HEAD : VERSIONS_HEAD }}</h3>
        <RiftButton size="sm" @click="versions.close()">Close</RiftButton>
      </header>
      <p class="vc__name">{{ name }}</p>
      <!-- R18.18.3: the yes, drawn; its words reach the polite live region below too -->
      <p v-if="versions.joinedLine.value && !asking" class="vc__note vc__joined">{{ versions.joinedLine.value }}</p>
      <ol v-if="members.length" class="vc__rows">
        <li
          v-for="m in members"
          :key="m.packageId"
          class="vc__row"
          :class="{ 'has-world': m.installed > 0 }"
          :data-package="m.packageId"
        >
          <p class="vc__line1">
            <span class="vc__ver">{{ m.version }}</span>
            <span v-if="m.newest" class="vc__badge">{{ NEWEST_BADGE }}</span>
            <!-- R18.18.3: a fact about the folder, set beside Nexus's fact -->
            <span v-if="m.joined" class="vc__badge">{{ BY_YOUR_WORD }}</span>
            <span class="vc__file rs-tail" :title="m.fileName"
              ><span class="rs-tail__text">{{ m.fileName }}</span></span
            >
          </p>
          <p class="vc__line2">
            <!-- one flex item: the warn words run straight on (`…in your world, under v1.3`) -->
            <span
              ><span :class="m.installed > 0 ? 'is-world' : 'is-faint'">{{
                memberWorldLine(m.installed, m.under).text
              }}</span
              ><span v-if="memberWorldLine(m.installed, m.under).under" class="is-warn">{{
                memberWorldLine(m.installed, m.under).under
              }}</span></span
            >
            <span v-if="(m.canPutAway || m.joined) && !asking" class="vc__rowdoors">
              <!-- R18.18.3: only on a row joined by your word, and only in the mods folder -->
              <RiftButton
                v-if="m.joined"
                size="sm"
                :busy="versions.parting.value === m.packageId"
                @click="versions.takeBack(m.packageId)"
                >{{ TAKE_IT_BACK }}</RiftButton
              >
              <RiftButton
                v-if="m.canPutAway"
                size="sm"
                class="vc__away"
                :busy="versions.puttingAway.value === m.packageId"
                @click="versions.putAway(m.packageId)"
                >Put it away</RiftButton
              >
            </span>
          </p>
        </li>
      </ol>

      <!-- R18.18.2, Bold Choice #23: the question in the shape of its answer -->
      <template v-if="asking && askedRow">
        <p class="vc__ask">
          <template v-if="of"
            >{{ askedQuestion(kind).before }}<span :class="{ 'vc__ask-file': of.mono }">{{ of.text }}</span
            >{{ askedQuestion(kind).after }}</template
          >
          <template v-else>{{ askedQuestionOpen(kind) }}</template>
        </p>
        <div class="vc__row is-asked" :class="{ 'has-world': askedRow.installed > 0 }">
          <p class="vc__line1">
            <span class="vc__asklabel">{{ askedRow.label }}</span>
            <span class="vc__file rs-tail" :title="askedRow.fileName"
              ><span class="rs-tail__text">{{ askedRow.fileName }}</span></span
            >
          </p>
          <p class="vc__line2">
            <span
              ><span :class="askedRow.installed > 0 ? 'is-world' : 'is-faint'">{{
                memberWorldLine(askedRow.installed, askedRow.under).text
              }}</span
              ><span v-if="memberWorldLine(askedRow.installed, askedRow.under).under" class="is-warn">{{
                memberWorldLine(askedRow.installed, askedRow.under).under
              }}</span></span
            >
          </p>
        </div>
        <p class="vc__note vc__evidence">{{ evidence }}</p>
        <!-- R18.18.6: two or more files qualify; none is picked for the player -->
        <fieldset v-if="choosing" class="vc__pick">
          <legend class="vc__pick-legend">{{ PICK_LEGEND }}</legend>
          <label v-for="c in versions.candidates.value" :key="c.packageId" class="vc__pick-row">
            <input
              type="radio"
              name="rs-join-file"
              :value="c.packageId"
              :checked="versions.picked.value === c.packageId"
              @change="versions.pick(c.packageId)"
            />
            <span class="vc__pick-text">{{ candidateLabel(c) }}</span>
          </label>
        </fieldset>
        <div class="vc__worddoors">
          <RiftButton
            variant="primary"
            :busy="versions.word.value === 'joining'"
            :refused="yesHeld !== null"
            :reason="yesHeld"
            @click="versions.join()"
            >{{ JOIN_DOOR }}</RiftButton
          >
          <RiftButton
            v-if="!manual"
            :busy="versions.word.value === 'declining'"
            :refused="versions.word.value === 'joining'"
            :reason="versions.word.value === 'joining' ? JOINING_HELD : null"
            @click="versions.decline()"
            >{{ KEEP_APART_DOOR }}</RiftButton
          >
        </div>
      </template>

      <p v-if="notInstalledLine && !asking" class="vc__note">{{ notInstalledLine }}</p>
      <div v-if="canKeepNewest && !asking" class="vc__doors">
        <RiftButton variant="primary" @click="versions.beginKeepNewest()">Keep only the newest…</RiftButton>
      </div>
      <!-- R18.18.5: a word that did not land, ember, inside the card -->
      <FailureNotice
        v-if="versions.wordFailure.value"
        :voiced="versions.wordFailure.value.voiced"
        @recover="versions.recoverWord"
      />
      <p class="rs-sr-only" aria-live="polite">{{ asking ? "" : (versions.joinedLine.value ?? "") }}</p>
    </template>

    <!-- R18.10: THE RETURN — OLDER VERSIONS, in the plan's register -->
    <template v-else>
      <header class="vc__head">
        <h3 class="vc__title rs-inscription">{{ KEEP_NEWEST_HEAD }}</h3>
        <div class="vc__buttons">
          <RiftButton
            v-if="versions.stage.value === 'keepPlan' || versions.stage.value === 'keeping'"
            variant="danger"
            :busy="versions.stage.value === 'keeping'"
            :refused="plan?.refused != null"
            :reason="plan?.refused ?? null"
            @click="versions.confirmKeepNewest()"
            >{{ keepButton(keeps) }}</RiftButton
          >
          <RiftButton v-if="versions.stage.value !== 'keeping'" @click="cancel()">{{
            versions.stage.value === "kept" ? "Done" : "Cancel"
          }}</RiftButton>
        </div>
      </header>
      <template v-if="plan && versions.stage.value !== 'kept'">
        <p v-if="plan.refused" class="vc__note is-ember" role="alert">{{ plan.refused }}</p>
        <p v-else class="vc__note">{{ keepsLine(keeps) }}</p>
        <p v-if="versions.stage.value === 'keeping'" class="vc__note" aria-live="polite">
          Removing {{ plural(removing, "file") }}…
        </p>
        <ul class="vc__plan">
          <li v-for="u in plan.uninstalls" :key="u.optionId" class="vc__planrow">
            <span class="vc__glyph" aria-hidden="true">↺</span>
            <span class="vc__optname">{{ u.name }}</span>
            <span v-if="u.version" class="vc__mono is-muted">{{ versionLabel(u.version) }}</span>
            <span class="vc__mono is-faint vc__end">{{ removalsText(u.removals) }}</span>
          </li>
          <li v-for="f in plan.putsAway" :key="f" class="vc__planrow">
            <span class="vc__glyph" aria-hidden="true">↧</span>
            <span class="vc__mono vc__planfile rs-tail" :title="f"
              ><span class="rs-tail__text">{{ f }}</span></span
            >
            <span class="vc__mono is-faint vc__end">{{ PUT_AWAY_ROW }}</span>
          </li>
        </ul>
      </template>
      <div v-if="versions.stage.value === 'kept' && versions.kept.value" class="vc__done" role="status">
        <span class="vc__carved rs-inscription">RETURNED TO THE RIFT</span>
        <p class="vc__note">{{ keptSummary(versions.kept.value.uninstalled) }}</p>
      </div>
      <template v-if="versions.stage.value === 'kept' && versions.kept.value">
        <div v-for="row in keptRows" :key="row.view.landedAs" class="vc__away-line">
          <p>
            <template v-if="versions.lineState(row.view.landedAs) === 'back'">{{ backLine(row.name) }}</template>
            <template v-else
              ><span aria-hidden="true">↧ </span
              >{{ putAwayLine(row.name, row.view.fileName, row.view.landedAs) }}</template
            >
          </p>
          <RiftButton
            v-if="versions.lineState(row.view.landedAs) !== 'back'"
            size="sm"
            :busy="versions.lineState(row.view.landedAs) === 'busy' || undercroft.moving.value === row.view.landedAs"
            @click="versions.bringBackLine(row.view)"
            >Bring it back</RiftButton
          >
          <p v-if="lineFailure(row.view.landedAs)" class="vc__fail" role="alert">
            {{ lineFailure(row.view.landedAs)?.voiced.voice }} {{ lineFailure(row.view.landedAs)?.voiced.empathy }}
          </p>
        </div>
        <p v-if="versions.kept.value.putAwayFailed" class="vc__warn">
          {{ versions.kept.value.putAwayFailed }} {{ putAwayFailedTail(stayedNames, "uninstalls") }}
        </p>
      </template>
      <FailureNotice
        v-if="versions.stage.value === 'failed' && versions.failure.value"
        :voiced="versions.failure.value.voiced"
        @recover="recover"
      />
    </template>
  </section>
</template>

<style scoped>
.vc {
  display: flex;
  flex-direction: column;
  gap: var(--rs-space-2);
}
.vc__head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: var(--rs-space-3);
  flex-wrap: wrap;
}
.vc__title {
  margin: 0;
  font-size: 14px;
  color: var(--rs-muted);
  letter-spacing: 0.1em;
}
.vc__buttons {
  display: flex;
  gap: var(--rs-space-2);
}
.vc__name {
  margin: 0 0 var(--rs-space-1);
  color: var(--rs-text);
  font: 700 var(--rs-text-lg) var(--rs-font-sans);
}
.vc__rows {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin: 0;
  padding: 0;
  list-style: none;
}
/* one row per member on the hearth's high step, a 4 px rail gold when it has installs */
.vc__row {
  position: relative;
  padding: var(--rs-space-3) var(--rs-space-4) var(--rs-space-3) var(--rs-space-5);
  background: var(--rs-hearthhi);
}
.vc__row.has-world::before {
  content: "";
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  width: 4px;
  background: var(--rs-world);
}
.vc__line1,
.vc__line2 {
  display: flex;
  align-items: center;
  gap: var(--rs-space-2);
  min-width: 0;
  margin: 0;
}
/* Q18.2's fallback (not measured in a browser): the file name wraps to its own
   line when the version, the badge and the name cannot share 392 px */
.vc__line1 {
  flex-wrap: wrap;
}
.vc__line2 {
  margin-top: var(--rs-space-1);
  font-size: var(--rs-text-sm);
}
.vc__ver {
  flex: none;
  color: var(--rs-text);
  font: 600 12px var(--rs-font-mono);
}
.vc__badge {
  flex: none;
  padding: 1px 5px;
  border: 1px solid var(--rs-edgehi);
  border-radius: var(--rs-radius-chip);
  color: var(--rs-muted);
  font: 600 12px / 1.3 var(--rs-font-mono);
}
.vc__file {
  flex: 1 1 12ch;
  color: var(--rs-faint);
  font: 400 12px var(--rs-font-mono);
}
.vc__rowdoors {
  display: flex;
  flex-wrap: wrap;
  gap: var(--rs-space-2);
  margin-left: auto;
}
/* R18.18.2: the question, Alegreya 700 14 */
.vc__ask {
  margin: var(--rs-space-2) 0 0;
  color: var(--rs-text);
  font: 700 14px / 1.45 var(--rs-font-sans);
}
.vc__ask-file {
  font: 400 12px var(--rs-font-mono);
  overflow-wrap: anywhere;
}
.vc__asklabel {
  flex: none;
  color: var(--rs-rift);
  font: 500 14px var(--rs-font-sans);
}
/* the doors wrap on a narrow card and never truncate */
.vc__worddoors {
  display: flex;
  flex-wrap: wrap;
  gap: var(--rs-space-2);
  margin-top: var(--rs-space-2);
}
.vc__pick {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin: var(--rs-space-2) 0 0;
  padding: 0;
  border: 0;
}
.vc__pick-legend {
  margin-bottom: var(--rs-space-1);
  padding: 0;
  color: var(--rs-muted);
  font-size: var(--rs-text-sm);
}
.vc__pick-row {
  display: flex;
  align-items: center;
  gap: var(--rs-space-2);
  min-height: 32px;
  padding: 0 var(--rs-space-2);
  background: var(--rs-hearthhi);
  cursor: pointer;
}
.vc__pick-row input {
  accent-color: var(--rs-rift);
}
.vc__pick-row input:focus-visible {
  outline: 2px solid var(--rs-rift);
  outline-offset: 2px;
}
.vc__pick-text {
  min-width: 0;
  color: var(--rs-text);
  font: 400 12px var(--rs-font-mono);
  overflow-wrap: anywhere;
}
.vc__note {
  margin: var(--rs-space-1) 0 0;
  color: var(--rs-text);
  font-size: var(--rs-text-sm);
  line-height: 1.45;
}
.vc__note.is-ember {
  color: var(--rs-ember);
}
.vc__doors {
  margin-top: var(--rs-space-2);
}
.vc__plan {
  margin: var(--rs-space-1) 0 0;
  padding: 0;
  list-style: none;
}
.vc__planrow {
  display: flex;
  align-items: baseline;
  gap: var(--rs-space-2);
  margin-bottom: 2px;
  padding: 6px var(--rs-space-2);
  background: var(--rs-hearthhi);
}
.vc__glyph {
  flex: none;
  width: 12px;
  color: var(--rs-muted);
  font: 400 12px var(--rs-font-mono);
}
.vc__optname {
  color: var(--rs-text);
  font: 500 var(--rs-text-sm) var(--rs-font-sans);
}
.vc__mono {
  flex: none;
  color: var(--rs-text);
  font: 400 12px var(--rs-font-mono);
}
.vc__planfile {
  flex: 1 1 0;
}
.vc__end {
  margin-left: auto;
}
.vc__done {
  padding: var(--rs-space-2) 10px;
  border-left: 3px solid var(--rs-rift);
  background: var(--rs-hearthhi);
  color: var(--rs-rift);
}
.vc__done .vc__note {
  color: var(--rs-text);
}
.vc__carved {
  font-size: var(--rs-text-sm);
}
.vc__away-line {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--rs-space-2) var(--rs-space-3);
}
.vc__away-line p {
  flex: 1 1 0;
  margin: 0;
  color: var(--rs-muted);
  font-size: var(--rs-text-sm);
}
.vc__fail {
  flex-basis: 100%;
  color: var(--rs-ember) !important;
}
.vc__warn {
  margin: 0;
  padding: var(--rs-space-2) 10px;
  border-left: 3px solid var(--rs-warn);
  background: var(--rs-hearthhi);
  color: var(--rs-text);
  font-size: var(--rs-text-sm);
}
.is-world {
  color: var(--rs-world);
}
.is-warn {
  color: var(--rs-warn);
}
.is-faint {
  color: var(--rs-faint);
}
.is-muted {
  color: var(--rs-muted);
}
</style>
