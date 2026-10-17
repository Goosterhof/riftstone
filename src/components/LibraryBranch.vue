<script setup lang="ts">
// §16.1 — one level of THE LIBRARY TREE: the rows of one parent, each a
// treeitem, each open branch carrying its children in a role="group" under a
// 1 px guide. Data, not inscription: Alegreya rows, never Cinzel (R16.3).
// Package rows are slabs on --rs-mist with the card's 4 px state rail; option
// rows lie on the void with a 2 px rail (R16.2–R16.4). Labels are not drawn
// on a row: a row is a line, not a card (Bold Choice 11).
import type { LibraryRow } from "@/composables/useLibrary";
import { kindsPresent, plural } from "@/services/format";
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

/** R16.4 + R16.10: 16 px per level, 8 px from the fifth level down. */
function indentOf(level: number): number {
  let px = 0;
  for (let l = 2; l <= level; l++) px += l >= 5 ? 8 : 16;
  return px;
}

type Tone = "ember" | "world" | "rift" | null;

/** The state rail's colour (§7.3): ember wins over gold, gold over cyan. */
function railOf(row: LibraryRow): Tone {
  if (row.kind === "package") {
    if (row.pkg.conflicted) return "ember";
    return row.pkg.installed > 0 ? "world" : "rift";
  }
  const node = row.node;
  if (!node || node.kind === "bundle" || node.menuOnly || !row.optionId) return null;
  if (node.conflicted) return "ember";
  return node.installed ? "world" : "rift";
}

function chipOf(row: LibraryRow): "menu" | "conflict" | "installed" | "notInstalled" | null {
  const node = row.node;
  if (!node || node.kind === "bundle") return null;
  if (node.menuOnly) return row.branch ? null : "menu";
  if (node.conflicted) return "conflict";
  return node.installed ? "installed" : "notInstalled";
}

/** The package's lead problem, in the trouble fold's own verdict (R16.2). */
function problemOf(row: LibraryRow): { text: string; ember: boolean } | null {
  const lead = row.pkg.package.problems[0];
  if (!lead) return null;
  return lead.kind === "unreadable"
    ? { text: "won't open", ember: true }
    : { text: "opens with a warning", ember: false };
}
</script>

<template>
  <li
    v-for="row in items"
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
    :aria-label="row.label"
    :tabindex="row.key === stop ? 0 : -1"
  >
    <div
      class="ltr__row"
      :class="[
        row.kind === 'package' ? 'is-pkg' : 'is-node',
        railOf(row) ? `rail--${railOf(row)}` : null,
        {
          'is-open': row.expanded,
          'is-quiet': row.kind === 'bundle' || row.node?.menuOnly,
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
      <span class="ltr__label" :title="row.label">{{ row.label }}</span>
      <template v-if="row.kind === 'package'">
        <span class="ltr__fmt">{{ row.pkg.package.format.toUpperCase() }}</span>
        <span v-if="problemOf(row)" class="ltr__count" :class="problemOf(row)?.ember ? 'is-ember' : 'is-muted'">{{
          problemOf(row)?.text
        }}</span>
        <span v-else class="ltr__count"
          >{{ plural(row.pkg.installable, "option")
          }}<span v-if="row.pkg.installed > 0" class="is-world"> · {{ row.pkg.installed }} in your world</span></span
        >
        <StateChip v-if="row.pkg.update === 'available'" state="update" />
        <span v-if="row.pkg.conflicted" class="ltr__sword"
          ><span aria-hidden="true">⚔</span><span class="rs-sr-only">conflict</span></span
        >
      </template>
      <span v-else-if="row.node" class="ltr__meta">
        <span
          v-if="kindsPresent(row.node.kinds).length"
          class="ltr__badges"
          :data-kinds="kindsPresent(row.node.kinds).length"
        >
          <KindBadge v-for="k in kindsPresent(row.node.kinds)" :key="k" :kind="k" />
        </span>
        <span
          v-if="kindsPresent(row.node.kinds).length > 1"
          class="ltr__kinds"
          :title="kindsPresent(row.node.kinds).join(', ')"
          >+{{ kindsPresent(row.node.kinds).length }} kinds</span
        >
        <StateChip v-if="chipOf(row)" :state="chipOf(row) ?? 'menu'" />
      </span>
    </div>
    <ul
      v-if="row.expanded && kids.get(row.key)?.length"
      role="group"
      class="ltr__group"
      :style="{ '--guide-x': `${12 + indentOf(row.level) + 5}px` }"
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
.ltr__row.is-pkg {
  height: 40px;
  background: var(--rs-mist);
  border-left: 4px solid var(--rs-rift);
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
.ltr__row.is-pkg.rail--world {
  border-left-color: var(--rs-world);
}
.ltr__row.is-pkg.rail--ember {
  border-left-color: var(--rs-ember);
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
