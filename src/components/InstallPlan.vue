<script setup lang="ts">
// §7.9 — THE CROSSING: every file, where it lands, and what it does to what is
// already there, before a byte moves. Warnings stack above the rows, each with
// its one-click recovery; only a hard refusal disables Install. Mirrors as
// THE RETURN for uninstall. The plan shows the user's own data — the spine's
// writes and removals — never a guess.
// v0.5 (#00015 R18.5, R18.9): an install that takes another version's place
// shows TAKES THE PLACE OF between the warnings and the rows, muted, never
// ember; once landed, each older version put away is told here, once, with
// `Bring it back`. The plan never names which versions will go: the landed
// state does, from `putAway`.
import { computed } from "vue";
import type { PlanWarning, PlannedRemoval, PlannedWrite } from "@/types/rift";
import { useWorld } from "@/composables/useWorld";
import { usePreflight, REFRAMEWORK_URL } from "@/composables/usePreflight";
import { useNavigation } from "@/composables/useNavigation";
import { useSearch } from "@/composables/useSearch";
import { useToasts } from "@/composables/useToasts";
import { useLibrary } from "@/composables/useLibrary";
import { useVersions } from "@/composables/useVersions";
import { formatBytes, plural } from "@/services/format";
import { skippedRows } from "@/services/payloadTree";
import { removalWords, returnedSummary, returnHeading } from "@/services/ledger";
import {
  PUT_AWAY_SENTENCE,
  REPLACES_HEAD,
  backLine,
  crossingFamily,
  familyMembers,
  putAwayFailedTail,
  putAwayLine,
  removalsText,
  rollbackLine,
  stepSentence,
  versionLabel,
  versionName,
} from "@/services/versions";
import type { RecoveryKind } from "@/services/voice";
import FailureNotice from "./FailureNotice.vue";
import RiftButton from "./RiftButton.vue";

const world = useWorld();
const preflight = usePreflight();
const nav = useNavigation();
const library = useLibrary();
const versions = useVersions();

interface Line {
  glyph: string;
  path: string;
  verb: string;
  tone: "new" | "over" | "own" | "skip" | "gone" | "back";
  size: number | null;
}

function writeLine(w: PlannedWrite): Line {
  const base = { path: w.gamePath, size: w.size };
  const a = w.action;
  if (a.kind === "create") return { ...base, glyph: "+", verb: "new", tone: "new" };
  if (a.kind === "reinstall") return { ...base, glyph: "↺", verb: "replaces own older file", tone: "own" };
  if (a.kind === "overwriteRiftstone") return { ...base, glyph: "!", verb: `OVERWRITES ${a.optionName}`, tone: "over" };
  return { ...base, glyph: "!", verb: "OVERWRITES a file no install owns (backed up first)", tone: "over" };
}

function removalLine(r: PlannedRemoval): Line {
  return { path: r.gamePath, size: null, ...removalWords(r.action) };
}

const returning = computed(
  () =>
    ["returnPlan", "returning", "returned"].includes(world.stage.value) || world.failure.value?.context === "uninstall",
);

const lines = computed<Line[]>(() => {
  if (returning.value) return (world.returnPlan.value?.removals ?? []).map(removalLine);
  const writes = (world.plan.value?.writes ?? []).map(writeLine);
  const option = world.detail.value?.option;
  const skips: Line[] = option
    ? skippedRows(option.skipped, option.root).map((s) => ({
        glyph: "·",
        path: s.path,
        verb: `skipped: ${s.reason}`,
        tone: "skip",
        size: null,
      }))
    : [];
  return [...writes, ...skips];
});

const fileCount = computed(() =>
  returning.value ? (world.returnPlan.value?.removals.length ?? 0) : (world.plan.value?.writes.length ?? 0),
);

const heading = computed(() =>
  returning.value
    ? `THE RETURN — ${returnHeading(world.returnPlan.value?.removals ?? [])}`
    : `THE CROSSING — ${plural(fileCount.value, "file")}`,
);

const refused = computed(() => (returning.value ? world.returnPlan.value?.refused : world.plan.value?.refused) ?? null);

interface WarningView {
  key: string;
  tone: "warn" | "ember" | "info";
  text: string;
  action: { label: string; run: () => void | Promise<void> } | null;
}

function warningView(w: PlanWarning): WarningView {
  switch (w.kind) {
    case "pakFolderOff":
      return {
        key: w.kind,
        tone: "warn",
        text: "Your PAK mods won't load: REFramework's Alternate PAK Folder is off.",
        action: { label: "Turn it on", run: () => preflight.turnOn("pakDirectory") },
      };
    case "looseLoaderOff":
      return {
        key: w.kind,
        tone: "warn",
        text: "Loose-file mods won't load: REFramework's Loose File Loader is off.",
        action: { label: "Turn it on", run: () => preflight.turnOn("looseFileLoader") },
      };
    case "reframeworkMissing":
      return {
        key: w.kind,
        tone: "warn",
        text: "REFramework isn't in your game folder, so this mod won't load. PAK mods need it too: the Alternate PAK Folder lives in REFramework.",
        action: { label: "Open REFramework on Nexus", run: () => preflight.openUrl(REFRAMEWORK_URL) },
      };
    case "gameRunning":
      return {
        key: w.kind,
        tone: "info",
        text: "Dragon's Dogma 2 is running. New paks and loose files load at the next launch.",
        action: null,
      };
    case "requiresMissing":
      return {
        key: `${w.kind}:${w.name}`,
        tone: "warn",
        text: `This mod needs ${w.name}, which isn't installed.`,
        action: { label: `Find ${w.name}`, run: () => findRequirement(w.name) },
      };
    case "conflicts":
      return {
        key: w.kind,
        tone: "ember",
        text: `⚔ Contests ${plural(w.paths, "file")} with ${w.optionNames.join(", ")}. Later wins: installing now puts this mod on top.`,
        action: { label: "See who overrides whom", run: () => nav.go("world") },
      };
    case "outdatedFormats":
      return {
        key: w.kind,
        tone: "warn",
        text: `${plural(w.count, "file")} use asset formats from before the current engine update; this mod may need an update.`,
        action: null,
      };
  }
}

const warnings = computed(() => (returning.value ? [] : (world.plan.value?.warnings ?? []).map(warningView)));

function findRequirement(name: string): void {
  world.backToDetail();
  useSearch().setText(`"${name}"`);
}

const progressLine = computed(() => {
  const p = world.progress.value;
  if (!p) return "Placing the files…";
  return `Placing ${p.filesDone} of ${p.filesTotal} files…`;
});

// --- v0.5: the replace block and the put-away lines -----------------------
const replaces = computed(() => (returning.value ? [] : (world.plan.value?.replaces ?? [])));
const planPackageId = computed(() => world.detail.value?.package.id ?? "");
const familyAt = computed(() => crossingFamily(library.packages.value, planPackageId.value, replaces.value));
/** The version the plan installs, as the copy names it. */
const newName = computed(() => {
  const pkg = library.packages.value.find((p) => p.package.id === planPackageId.value);
  return versionName(pkg?.family?.version ?? null, pkg?.package.fileName ?? world.plan.value?.name ?? "");
});
const putAwayRows = computed(() =>
  (world.landed.value?.putAway ?? []).map((view) => ({
    view,
    name: world.landedNames.value.get(view.fileName) ?? view.fileName,
  })),
);
/** The older versions a failed put-away left in the mods folder, named together. */
const stayingNames = computed(() => {
  const place = familyMembers(library.packages.value, planPackageId.value);
  const left = (place?.members ?? []).slice((place?.index ?? 0) + 1).filter((m) => m.installed === 0);
  const names = left.map((m) => versionName(m.family?.version ?? null, m.package.fileName));
  return names.length ? names.join(" and ") : "the older version";
});
const lineFailure = (landedAs: string) => {
  const state = versions.lineState(landedAs);
  return typeof state === "object" ? state : null;
};

async function recover(kind: RecoveryKind): Promise<void> {
  if (kind === "retry") await world.retry();
  else if (kind === "copyDetails") await copyDetails();
  else world.backToDetail();
}

async function copyDetails(): Promise<void> {
  const text = world.failure.value?.details ?? "";
  try {
    await navigator.clipboard.writeText(text);
    useToasts().push("rift", "The details are on your clipboard.");
  } catch {
    useToasts().push("warn", "The clipboard refused; the details are in the log instead.");
  }
}
</script>

<template>
  <section class="plan" :aria-busy="world.stage.value === 'crossing' || world.stage.value === 'returning'">
    <header class="plan__head">
      <h3 class="plan__title rs-inscription">{{ heading }}</h3>
      <div class="plan__buttons">
        <template v-if="!returning">
          <RiftButton
            v-if="world.stage.value === 'plan' || world.stage.value === 'crossing'"
            variant="world"
            :busy="world.stage.value === 'crossing'"
            :disabled="refused !== null"
            :reason="refused"
            @click="world.confirmInstall()"
            >Install ▶</RiftButton
          >
        </template>
        <RiftButton
          v-else-if="world.stage.value === 'returnPlan' || world.stage.value === 'returning'"
          variant="danger"
          :busy="world.stage.value === 'returning'"
          :disabled="refused !== null"
          :reason="refused"
          @click="world.confirmUninstall()"
          >Uninstall</RiftButton
        >
        <RiftButton
          v-if="world.stage.value !== 'crossing' && world.stage.value !== 'returning'"
          @click="world.backToDetail()"
          >{{ world.stage.value === "landed" || world.stage.value === "returned" ? "Done" : "Cancel" }}</RiftButton
        >
      </div>
    </header>

    <p v-if="refused" class="plan__note is-ember" role="alert">{{ refused }}</p>

    <div v-for="w in warnings" :key="w.key" class="plan__warn" :class="`is-${w.tone}`">
      <span>{{ w.tone === "ember" ? "" : "⚠ " }}{{ w.text }}</span>
      <RiftButton
        v-if="w.action"
        size="sm"
        variant="secondary"
        :busy="preflight.busy.value !== null"
        @click="w.action.run()"
        >{{ w.action.label }}</RiftButton
      >
    </div>

    <!-- R18.9: the replace, between the warnings and the files; muted, never ember -->
    <section
      v-if="replaces.length && (world.stage.value === 'plan' || world.stage.value === 'crossing')"
      class="plan__replaces"
      :aria-label="REPLACES_HEAD"
    >
      <h4 class="plan__subhead rs-inscription">{{ REPLACES_HEAD }}</h4>
      <ul class="plan__replace-rows">
        <li v-for="r in replaces" :key="r.optionId" class="plan__replace">
          <span class="plan__replace-glyph" aria-hidden="true">↺</span>
          <span class="plan__replace-name">{{ r.name }}</span>
          <span v-if="r.version" class="plan__replace-ver">{{ versionLabel(r.version) }}</span>
          <span class="plan__replace-count">{{ removalsText(r.removals) }}</span>
        </li>
      </ul>
      <p v-if="familyAt.rollbackFrom" class="plan__step is-text">{{ rollbackLine(familyAt.rollbackFrom) }}</p>
      <p class="plan__step">{{ stepSentence(replaces, newName) }}</p>
      <!-- §18.17: whenever a member is older than the package installed, a rollback's too -->
      <p v-if="familyAt.olderRemain" class="plan__step is-faint">{{ PUT_AWAY_SENTENCE }}</p>
    </section>

    <p v-if="world.stage.value === 'crossing'" class="plan__note" aria-live="polite">{{ progressLine }}</p>
    <p v-else-if="world.stage.value === 'returning'" class="plan__note" aria-live="polite">
      Removing {{ plural(fileCount, "file") }}…
    </p>
    <!-- The crossing lands once (Chaos #00129 D4): the toast says the sentence and
         owns Undo; the plan carves the state. THE RETURN mirrors it. -->
    <div v-else-if="world.stage.value === 'landed' && world.landed.value" class="plan__landed" role="status">
      <span class="plan__carved rs-inscription">IN YOUR WORLD</span>
      <RiftButton size="sm" @click="nav.go('world')">Show in load order</RiftButton>
    </div>
    <!-- R18.5: each older version put away, told once, here and never in the toast -->
    <template v-if="world.stage.value === 'landed' && world.landed.value">
      <div v-for="row in putAwayRows" :key="row.view.landedAs" class="plan__away">
        <p class="plan__away-line">
          <template v-if="versions.lineState(row.view.landedAs) === 'back'">{{ backLine(row.name) }}</template>
          <template v-else
            ><span aria-hidden="true">↧ </span
            >{{ putAwayLine(row.name, row.view.fileName, row.view.landedAs) }}</template
          >
        </p>
        <RiftButton
          v-if="versions.lineState(row.view.landedAs) !== 'back'"
          size="sm"
          :busy="versions.lineState(row.view.landedAs) === 'busy'"
          @click="versions.bringBackLine(row.view)"
          >Bring it back</RiftButton
        >
        <p v-if="lineFailure(row.view.landedAs)" class="plan__away-fail" role="alert">
          {{ lineFailure(row.view.landedAs)?.voiced.voice }} {{ lineFailure(row.view.landedAs)?.voiced.empathy }}
        </p>
      </div>
      <div v-if="world.landed.value.putAwayFailed" class="plan__warn plan__away-warn">
        <span>{{ world.landed.value.putAwayFailed }} {{ putAwayFailedTail(stayingNames, "install") }}</span>
        <RiftButton size="sm" :busy="world.retryingPutAway.value" @click="world.retryPutAway()">Try again</RiftButton>
      </div>
    </template>
    <div v-else-if="world.stage.value === 'returned'" class="plan__returned" role="status">
      <span class="plan__carved rs-inscription">RETURNED TO THE RIFT</span>
      <p v-if="world.returned.value" class="plan__note">{{ returnedSummary(world.returned.value) }}</p>
    </div>

    <FailureNotice
      v-if="world.stage.value === 'failed' && world.failure.value"
      :voiced="world.failure.value.voiced"
      @recover="recover"
    />

    <ol class="plan__rows" :aria-label="heading">
      <li v-for="(l, i) in lines" :key="`${i}-${l.path}`" class="plan__row" :class="`is-${l.tone}`">
        <span class="plan__glyph" aria-hidden="true">{{ l.glyph }}</span>
        <span class="plan__path">{{ l.path }}</span>
        <span class="plan__verb">{{ l.verb }}</span>
        <span v-if="l.size !== null" class="plan__size">{{ formatBytes(l.size) }}</span>
      </li>
    </ol>
    <p v-if="!returning && world.plan.value && world.stage.value === 'plan'" class="plan__total rs-mono">
      {{ formatBytes(world.plan.value.totalBytes) }} into your game folder · nothing is written until you press Install
    </p>
  </section>
</template>

<style scoped>
.plan__head {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: var(--rs-space-3);
  flex-wrap: wrap;
  margin-bottom: var(--rs-space-3);
}
.plan__title {
  margin: 0;
  font-size: 14px;
  color: var(--rs-muted);
  letter-spacing: 0.1em;
}
.plan__buttons {
  display: flex;
  gap: var(--rs-space-2);
}
.plan__warn {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--rs-space-3);
  margin-bottom: var(--rs-space-2);
  padding: var(--rs-space-2) 10px;
  border-left: 3px solid var(--rs-warn);
  background: var(--rs-hearthhi);
  color: var(--rs-text);
  font-size: var(--rs-text-sm);
  line-height: 1.4;
}
.plan__warn.is-ember {
  border-left-color: var(--rs-ember);
}
.plan__warn.is-info {
  border-left-color: var(--rs-edgehi);
}
/* R18.9: TAKES THE PLACE OF — a §7.8 section head, then plan-like rows */
.plan__replaces {
  margin: 0 0 var(--rs-space-3);
}
.plan__subhead {
  margin: 0 0 var(--rs-space-2);
  font-size: 14px;
  color: var(--rs-muted);
  letter-spacing: 0.1em;
}
.plan__replace-rows {
  margin: 0;
  padding: 0;
  list-style: none;
}
.plan__replace {
  display: flex;
  align-items: baseline;
  gap: var(--rs-space-2);
  margin-bottom: 2px;
  padding: 6px var(--rs-space-2);
  background: var(--rs-hearthhi);
}
.plan__replace-glyph {
  flex: none;
  width: 12px;
  color: var(--rs-muted);
  font: 400 var(--rs-text-xs) var(--rs-font-mono);
}
.plan__replace-name {
  flex: 0 1 auto;
  min-width: 0;
  color: var(--rs-text);
  font: 500 var(--rs-text-sm) var(--rs-font-sans);
}
.plan__replace-ver {
  flex: none;
  color: var(--rs-muted);
  font: 400 12px var(--rs-font-mono);
}
.plan__replace-count {
  flex: none;
  margin-left: auto;
  color: var(--rs-faint);
  font: 400 12px var(--rs-font-mono);
}
.plan__step {
  margin: var(--rs-space-2) 0 0;
  color: var(--rs-muted);
  font-size: var(--rs-text-sm);
  line-height: 1.4;
}
.plan__step.is-text {
  color: var(--rs-text);
}
.plan__step.is-faint {
  color: var(--rs-faint);
}
/* R18.5: the put-away line under IN YOUR WORLD */
.plan__away {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: space-between;
  gap: var(--rs-space-2) var(--rs-space-3);
  margin-bottom: var(--rs-space-2);
}
.plan__away-line {
  flex: 1 1 0;
  min-width: 0;
  margin: 0;
  color: var(--rs-muted);
  font-size: var(--rs-text-sm);
  line-height: 1.4;
}
.plan__away-fail {
  flex-basis: 100%;
  margin: 0;
  color: var(--rs-ember);
  font-size: var(--rs-text-sm);
}
.plan__note {
  margin: 0 0 var(--rs-space-2);
  color: var(--rs-muted);
}
.plan__note.is-ember {
  color: var(--rs-ember);
}
.plan__landed {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--rs-space-3);
  margin-bottom: var(--rs-space-2);
  padding: var(--rs-space-2) 10px;
  border-left: 3px solid var(--rs-world);
  background: var(--rs-hearthhi);
  color: var(--rs-world);
}
/* Back to the Rift is back to the cold: the return's state carves in rift light. */
.plan__returned {
  margin-bottom: var(--rs-space-2);
  padding: var(--rs-space-2) 10px;
  border-left: 3px solid var(--rs-rift);
  background: var(--rs-hearthhi);
  color: var(--rs-rift);
}
.plan__returned .plan__note {
  margin: var(--rs-space-1) 0 0;
}
.plan__carved {
  font-size: var(--rs-text-sm);
}
.plan__rows {
  margin: var(--rs-space-2) 0 0;
  padding: 0;
  list-style: none;
  font: 400 var(--rs-text-xs) / 1.35 var(--rs-font-mono);
}
.plan__row {
  display: flex;
  flex-wrap: wrap;
  gap: 4px var(--rs-space-2);
  margin-bottom: 2px;
  padding: 6px var(--rs-space-2);
  background: var(--rs-hearthhi);
}
.plan__glyph {
  flex: none;
  width: 12px;
}
.plan__path {
  flex: 1;
  min-width: 0;
  color: var(--rs-text);
  word-break: break-all;
}
.plan__verb {
  flex: none;
}
.plan__size {
  flex: none;
  color: var(--rs-faint);
}
.is-new .plan__glyph,
.is-new .plan__verb {
  color: var(--rs-rift);
}
.is-over .plan__glyph,
.is-over .plan__verb {
  color: var(--rs-ember);
  font-weight: 600;
}
.is-over .plan__verb {
  flex-basis: 100%;
  padding-left: 20px;
}
.is-own .plan__glyph,
.is-own .plan__verb,
.is-gone .plan__glyph,
.is-gone .plan__verb {
  color: var(--rs-muted);
}
.is-back .plan__glyph,
.is-back .plan__verb {
  color: var(--rs-world);
}
.is-skip .plan__glyph,
.is-skip .plan__path,
.is-skip .plan__verb {
  color: var(--rs-faint);
}
.plan__total {
  margin: var(--rs-space-2) 0 0;
  color: var(--rs-faint);
}
</style>
