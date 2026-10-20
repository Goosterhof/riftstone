<script setup lang="ts">
// §16.1 — one level of THE LIBRARY TREE: the rows of one parent, each a
// treeitem, each open branch carrying its children in a role="group" under a
// 1 px guide. Data, not inscription: Alegreya rows, never Cinzel (R16.3).
// Package rows are slabs on --rs-mist with the card's 4 px state rail; option
// rows lie on the void with a 2 px rail (R16.2–R16.4). Labels are not drawn
// on a row: a row is a line, not a card (Bold Choice 11).
//
// A treeitem is NAMED by its own label (aria-labelledby) and DESCRIBED by the
// state its row shows (aria-describedby): the format, the counts or the
// verdict, the UPDATE chip, the hidden "conflict", a leaf's kinds and state
// chip. Never aria-label, which would replace the state with the label, and
// never a name from the content, which would pull in the nested group.
//
// v0.5 (#00015 R18.1–R18.3): a family's lead carries its version and the older
// mark, its rail warn while an older version is still installed; each older
// version is a menu-node row inside it (no ground, its file name beside the
// label), and a lead leaf in your world only from an older version reads
// `INSTALLED v1.2`. The words carry every state (WCAG 1.4.1).
//
// v0.5 (#00015 R18.18.1): a package the stone asks about carries
// `a version of v1.3?` in the older mark's slot, cyan, and its entry opens on
// one question row (a leaf, the menu-node look, no button: Enter opens the
// question card in Your World). A joined version row says `· by your word`.
import { useId } from "vue";
import type { LibraryRow } from "@/composables/useLibrary";
import { FORMAT_LABEL, kindsPresent, plural } from "@/services/format";
import { troubleProblem } from "@/services/voice";
import {
  askMark,
  chipVersion,
  installedFromOlder,
  olderMark,
  versionLabel,
  versionRowState,
  type AskMark,
  type OlderMark,
} from "@/services/versions";
import KindBadge from "./KindBadge.vue";
import StateChip from "./StateChip.vue";

const {
  items,
  kids,
  stop,
  selectedId = null,
  drawn,
} = defineProps<{
  items: LibraryRow[];
  /** Visible children by parent key, in order. */
  kids: ReadonlyMap<string, LibraryRow[]>;
  /** The key of the one row in the tab order (roving tabindex). */
  stop: string | null;
  selectedId?: string | null;
  /** Packages opened for the first time this session: their rails draw in. */
  drawn: ReadonlySet<string>;
}>();
const emit = defineEmits<{ press: [row: LibraryRow]; chevron: [row: LibraryRow] }>();

/** R16.4: the level guide sits under its parent's chevron centre: the row's
 *  1 px border, 12 px padding, half the 10 px chevron. Every row has the same
 *  1 px border (a package's 4 px rail is drawn, not a border), so one indent
 *  step is 16 px from the package's text start. */
const GUIDE_AT = 1 + 12 + 5;

/** R16.4 + R16.10: 16 px per level, 8 px from the fifth level down. */
function indentOf(level: number): number {
  let px = 0;
  for (let l = 2; l <= level; l++) px += l >= 5 ? 8 : 16;
  return px;
}

type Tone = "ember" | "warn" | "world" | "rift" | null;

/** The lead's older mark (R18.1), or null for a package with no older version. */
const markOf = (row: LibraryRow): OlderMark | null =>
  row.kind === "package" ? olderMark(row.pkg.family?.older ?? []) : null;

/** R18.3: the older version a lead leaf is in your world from, its own leaf not installed. */
function fromOlder(row: LibraryRow): string | null {
  const node = row.node;
  if (!node || !row.optionId || node.kind === "bundle" || node.menuOnly) return null;
  const older = installedFromOlder(row.pkg, row.optionId, node.installed);
  // R18.18.7: a version Nexus never named reads `INSTALLED older`, never a file name in a chip
  return older ? chipVersion(older.version) : null;
}

/** R18.18.1: the question's mark on the questioned package's own row (never on the other's). */
const askOf = (row: LibraryRow): AskMark | null =>
  row.kind === "package" && row.pkg.versionQuestion ? askMark(row.pkg.versionQuestion) : null;

/** The state rail's colour (§7.3, R18.1): ember > warn (an older version still
 *  installed) > gold > cyan. A version row's rail is gold when it has installs. */
function packageRail(row: LibraryRow): Tone {
  if (row.pkg.conflicted) return "ember";
  if (markOf(row)?.warn) return "warn";
  return row.pkg.installed > 0 ? "world" : "rift";
}

function railOf(row: LibraryRow): Tone {
  if (row.kind === "package") return packageRail(row);
  if (row.kind === "version") return (row.version?.installed ?? 0) > 0 ? "world" : null;
  return nodeRail(row);
}

function nodeRail(row: LibraryRow): Tone {
  const node = row.node;
  if (!node || node.kind === "bundle" || node.menuOnly || !row.optionId) return null;
  if (node.conflicted) return "ember";
  return node.installed || fromOlder(row) ? "world" : "rift";
}

/** The lead's own version (R18.1): on a family's lead only. */
function leadVersion(row: LibraryRow): string | null {
  const family = row.pkg.family;
  if (row.kind !== "package" || !family || family.lead !== row.pkg.package.id || !family.version) return null;
  return versionLabel(family.version);
}

/** A row that installs something: a leaf option, never a menu or a bundle. */
const installsOn = (row: LibraryRow): boolean =>
  row.optionId !== null && !!row.node && row.node.kind !== "bundle" && !row.node.menuOnly;

function chipOf(row: LibraryRow): "menu" | "conflict" | "installed" | "notInstalled" | null {
  const node = row.node;
  if (!node || node.kind === "bundle") return null;
  if (node.menuOnly) return row.branch ? null : "menu";
  if (node.conflicted) return "conflict";
  return node.installed || fromOlder(row) ? "installed" : "notInstalled";
}

/** The trouble fold's own verdict (R16.2): only for a trouble the fold lists
 *  (services/voice.ts TROUBLE_KINDS), read in its order, never problems[0]
 *  blindly. A package whose problems the fold does not list keeps its counts. */
function problemOf(row: LibraryRow): { text: string; ember: boolean } | null {
  const lead = troubleProblem(row.pkg.package);
  if (!lead) return null;
  return lead.kind === "unreadable"
    ? { text: "won't open", ember: true }
    : { text: "opens with a warning", ember: false };
}

/** Ids for one row's parts: one LibraryBranch per level, so its own prefix and
 *  the row's place in it make them unique in the document. */
const uid = useId();
const idOf = (i: number, part: string): string => `${uid}-${i}-${part}`;

/** What describes a row: the parts of its line that carry its state. */
function stateIds(row: LibraryRow, i: number): string | undefined {
  if (row.kind === "version") return `${idOf(i, "file")} ${idOf(i, "count")}`;
  if (row.kind === "question") return undefined;
  if (row.kind !== "package") return row.node ? idOf(i, "meta") : undefined;
  const parts = leadVersion(row) ? ["ver", "fmt", "count"] : ["fmt", "count"];
  if (markOf(row)) parts.push("older");
  if (askOf(row)) parts.push("ask");
  if (row.pkg.update === "available") parts.push("update");
  if (row.pkg.conflicted) parts.push("conflict");
  return parts.map((part) => idOf(i, part)).join(" ");
}
</script>

<template>
  <li
    v-for="(row, i) in items"
    :key="row.key"
    role="treeitem"
    class="ltr__item"
    :class="`ltr__item--l${Math.min(row.level, 2)}`"
    :data-key="row.key"
    :aria-level="row.level"
    :aria-setsize="row.setSize"
    :aria-posinset="row.position"
    :aria-expanded="row.branch ? (row.expanded ? 'true' : 'false') : undefined"
    :aria-selected="row.optionId ? (row.optionId === selectedId ? 'true' : 'false') : undefined"
    :aria-labelledby="idOf(i, 'name')"
    :aria-describedby="stateIds(row, i)"
    :tabindex="row.key === stop ? 0 : -1"
  >
    <div
      class="ltr__row"
      :class="[
        row.kind === 'package' ? 'is-pkg' : 'is-node',
        railOf(row) ? `rail--${railOf(row)}` : null,
        {
          'is-open': row.expanded,
          'is-quiet': row.kind === 'bundle' || row.kind === 'version' || row.kind === 'question' || row.node?.menuOnly,
          'is-sel': row.optionId !== null && row.optionId === selectedId,
          'is-drawn': row.parent !== null && drawn.has(row.packageId),
        },
      ]"
      :style="{ paddingLeft: `${12 + indentOf(row.level)}px` }"
      @click="emit('press', row)"
    >
      <span class="ltr__chev" aria-hidden="true" @click.stop="row.branch ? emit('chevron', row) : emit('press', row)">{{
        row.branch ? (row.expanded ? "▾" : "▸") : "·"
      }}</span>
      <span
        :id="idOf(i, 'name')"
        class="ltr__label"
        :class="{ 'is-version': row.kind === 'version', 'is-ask': row.kind === 'question' }"
        :title="row.label"
        >{{ row.label }}</span
      >
      <template v-if="row.kind === 'version'">
        <!-- R18.2: the older package's own file, ellipsized from the left, whole in its title -->
        <span :id="idOf(i, 'file')" class="ltr__file rs-tail" :title="row.pkg.package.fileName"
          ><span class="rs-tail__text">{{ row.pkg.package.fileName }}</span></span
        >
        <span :id="idOf(i, 'count')" class="ltr__count" :class="{ 'is-world': (row.version?.installed ?? 0) > 0 }">{{
          versionRowState(row.version?.installed ?? 0)
        }}</span>
      </template>
      <template v-else-if="row.kind === 'package'">
        <span v-if="leadVersion(row)" :id="idOf(i, 'ver')" class="ltr__ver">{{ leadVersion(row) }}</span>
        <span :id="idOf(i, 'fmt')" class="ltr__fmt">{{ FORMAT_LABEL[row.pkg.package.format] }}</span>
        <span
          v-if="problemOf(row)"
          :id="idOf(i, 'count')"
          class="ltr__count"
          :class="problemOf(row)?.ember ? 'is-ember' : 'is-muted'"
          >{{ problemOf(row)?.text }}</span
        >
        <span v-else :id="idOf(i, 'count')" class="ltr__count"
          >{{ plural(row.pkg.installable, "option")
          }}<span v-if="row.pkg.installed > 0" class="is-world"> · {{ row.pkg.installed }} in your world</span></span
        >
        <span
          v-if="markOf(row)"
          :id="idOf(i, 'older')"
          class="ltr__older"
          :class="{ 'is-warn': markOf(row)?.warn }"
          :title="markOf(row)?.hidden"
          ><span class="ltr__older-full" aria-hidden="true">{{ markOf(row)?.text }}</span
          ><span class="ltr__older-short" aria-hidden="true">{{ markOf(row)?.short }}</span
          ><span class="rs-sr-only">{{ markOf(row)?.hidden }}</span></span
        >
        <!-- R18.18.1: the question, cyan (it is about the mods folder, not your game); no rail change -->
        <span v-if="askOf(row)" :id="idOf(i, 'ask')" class="ltr__older ltr__ask" :title="askOf(row)?.hidden"
          ><span class="ltr__older-full" aria-hidden="true">{{ askOf(row)?.text }}</span
          ><span class="ltr__older-short" aria-hidden="true">{{ askOf(row)?.short }}</span
          ><span class="rs-sr-only">{{ askOf(row)?.hidden }}</span></span
        >
        <StateChip v-if="row.pkg.update === 'available'" :id="idOf(i, 'update')" state="update" />
        <span v-if="row.pkg.conflicted" :id="idOf(i, 'conflict')" class="ltr__sword"
          ><span aria-hidden="true">⚔</span><span class="rs-sr-only">conflict</span></span
        >
      </template>
      <span v-else-if="row.node" :id="idOf(i, 'meta')" class="ltr__meta">
        <!-- R16.3: menu and bundle nodes install nothing, so they carry no badges -->
        <span
          v-if="installsOn(row) && kindsPresent(row.node.kinds).length"
          class="ltr__badges"
          :data-kinds="kindsPresent(row.node.kinds).length"
        >
          <KindBadge v-for="k in kindsPresent(row.node.kinds)" :key="k" :kind="k" />
        </span>
        <span
          v-if="installsOn(row) && kindsPresent(row.node.kinds).length > 1"
          class="ltr__kinds"
          :title="kindsPresent(row.node.kinds).join(', ')"
          >+{{ kindsPresent(row.node.kinds).length }} kinds</span
        >
        <!-- R18.3: in your world, at the older version: the gold chip with the version appended -->
        <span v-if="fromOlder(row)" class="ltr__chip-older">INSTALLED {{ fromOlder(row) }}</span>
        <StateChip v-else-if="chipOf(row)" :state="chipOf(row) ?? 'menu'" />
      </span>
    </div>
    <ul
      v-if="row.expanded && kids.get(row.key)?.length"
      role="group"
      class="ltr__group"
      :style="{ '--guide-x': `${GUIDE_AT + indentOf(row.level)}px` }"
    >
      <LibraryBranch
        :items="kids.get(row.key) ?? []"
        :kids="kids"
        :stop="stop"
        :selected-id="selectedId"
        :drawn="drawn"
        @press="(r) => emit('press', r)"
        @chevron="(r) => emit('chevron', r)"
      />
    </ul>
  </li>
</template>

<style scoped>
.ltr__item {
  outline: none;
}
.ltr__item--l1 + .ltr__item--l1 {
  margin-top: var(--rs-space-1);
}
/* an expanded package: 8 px of the slab rhythm before the next slab (R16.4) */
.ltr__item--l1[aria-expanded="true"] {
  margin-bottom: var(--rs-space-2);
}
/* R16.5: the ring sits INSIDE the row; the Rift column clips an outside one */
.ltr__item:focus-visible > .ltr__row {
  outline: 2px solid var(--rs-rift);
  outline-offset: -2px;
}
.ltr__row {
  position: relative;
  display: flex;
  align-items: center;
  gap: var(--rs-space-2);
  min-width: 0;
  padding-right: var(--rs-space-3);
  border: 1px solid transparent;
  color: var(--rs-text);
  font: 400 var(--rs-text-sm) var(--rs-font-sans);
  white-space: nowrap;
  cursor: pointer;
  container: ltr-row / inline-size;
}
.ltr__row:hover {
  background: var(--rs-misthi);
}
.ltr__row.is-sel {
  background: var(--rs-misthi);
  border-color: var(--rs-edgehi);
}
/* the package: one slab on --rs-mist, 40 px, its 4 px state rail */
/* the rail is drawn, not a border: every row keeps the same 1 px box, so the
   indent is measured from the same text start (R16.4) */
.ltr__row.is-pkg {
  height: 40px;
  background: var(--rs-mist);
}
.ltr__row.is-pkg::before {
  content: "";
  position: absolute;
  top: -1px;
  bottom: -1px;
  left: -1px;
  width: 4px;
  background: var(--rs-rift);
}
.ltr__row.is-pkg:hover {
  background: var(--rs-misthi);
}
.ltr__row.is-pkg.is-open {
  box-shadow: 0 1px 0 var(--rs-edge);
}
.ltr__row.is-pkg .ltr__label {
  font-weight: 700;
}
.ltr__row.is-pkg.rail--world::before {
  background: var(--rs-world);
}
.ltr__row.is-pkg.rail--ember::before {
  background: var(--rs-ember);
}
/* R18.1: an older version still installed under the lead; the rail's colour
   takes --rs-dur-quick, the investor's moment when it goes out (§18.13) */
.ltr__row.is-pkg.rail--warn::before {
  background: var(--rs-warn);
}
.ltr__row.is-pkg::before {
  transition: background-color var(--rs-dur-quick);
}
/* an option: on the void, 32 px, a 2 px rail half the package's */
.ltr__row.is-node {
  height: 32px;
}
.ltr__row.is-node.rail--rift::before,
.ltr__row.is-node.rail--world::before,
.ltr__row.is-node.rail--ember::before {
  content: "";
  position: absolute;
  top: 0;
  bottom: 0;
  left: 0;
  width: 2px;
  background: var(--rs-rift);
  transform-origin: top;
}
.ltr__row.is-node.rail--world::before {
  background: var(--rs-world);
}
.ltr__row.is-node.rail--ember::before {
  background: var(--rs-ember);
}
/* §16.7: the first open of a package this session — light runs down the channels */
.ltr__row.is-drawn::before {
  animation: ltr-draw var(--rs-dur-quick) ease-out both;
}
@keyframes ltr-draw {
  from {
    transform: scaleY(0);
  }
  to {
    transform: scaleY(1);
  }
}
.ltr__row.is-quiet {
  color: var(--rs-muted);
  font-weight: 500;
}
.ltr__chev {
  flex: none;
  width: 10px;
  color: var(--rs-faint);
}
.ltr__label {
  flex: 1 1 auto;
  min-width: 12ch;
  overflow: hidden;
  text-overflow: ellipsis;
}
.ltr__ver {
  flex: none;
  color: var(--rs-muted);
  font: 400 12px var(--rs-font-mono);
}
.ltr__fmt {
  flex: none;
  color: var(--rs-faint);
  font: 400 12px var(--rs-font-mono);
}
.ltr__count {
  flex: none;
  color: var(--rs-faint);
}
.ltr__count .is-world {
  color: var(--rs-world);
}
.ltr__count.is-ember {
  color: var(--rs-ember);
}
.ltr__count.is-muted {
  color: var(--rs-muted);
}
.ltr__older {
  flex: none;
  color: var(--rs-faint);
}
.ltr__older.is-warn {
  color: var(--rs-warn);
}
.ltr__older-short {
  display: none;
}
/* R18.18.1: the question, in the older mark's slot, cyan (rift on mist 12.27) */
.ltr__older.ltr__ask {
  color: var(--rs-rift);
  font: 400 14px var(--rs-font-sans);
}
/* R18.18.1: the question row, the menu-node look in Alegreya 500 cyan */
.ltr__label.is-ask {
  flex: 0 1 auto;
  color: var(--rs-rift);
  font: 500 14px var(--rs-font-sans);
}
/* R18.2: the version row's label keeps the menu-node look */
.ltr__label.is-version {
  flex: 0 1 auto;
}
.ltr__file {
  flex: 1 1 0;
  min-width: 6ch;
  color: var(--rs-faint);
  font: 400 12px var(--rs-font-mono);
}
.ltr__count.is-world {
  color: var(--rs-world);
}
.ltr__chip-older {
  display: inline-block;
  padding: 2px 6px;
  border-radius: var(--rs-radius-chip);
  background: var(--rs-world);
  color: var(--rs-ongold);
  font: 600 var(--rs-text-xs) / 1.3 var(--rs-font-mono);
  letter-spacing: 0.04em;
  white-space: nowrap;
}
.ltr__sword {
  flex: none;
  color: var(--rs-ember);
}
.ltr__meta {
  flex: none;
  display: flex;
  align-items: center;
  gap: 4px;
}
.ltr__badges {
  display: flex;
  gap: 4px;
}
.ltr__kinds {
  display: none;
  color: var(--rs-faint);
  font: 400 12px var(--rs-font-mono);
}
/* R16.10: the label keeps its 12ch; the kind badges fold to one count first */
/* R18.1 / Q18.1's fallback (not measured in a browser yet): below a 480 px row
   the older mark folds to its count, its words in the hidden text and the title */
@container ltr-row (max-width: 480px) {
  .ltr__older-full {
    display: none;
  }
  .ltr__older-short {
    display: inline;
  }
}
@container ltr-row (max-width: 420px) {
  .ltr__badges[data-kinds]:not([data-kinds="1"]) {
    display: none;
  }
  .ltr__kinds {
    display: inline;
  }
}
.ltr__group {
  position: relative;
  margin: 0;
  padding: 0;
  list-style: none;
}
/* the level's guide under its parent's chevron — decorative, the indent is the boundary */
.ltr__group::before {
  content: "";
  position: absolute;
  top: 0;
  bottom: 4px;
  left: var(--guide-x);
  width: 1px;
  background: var(--rs-edge);
  pointer-events: none;
}
</style>
