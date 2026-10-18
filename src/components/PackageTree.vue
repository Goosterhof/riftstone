<script setup lang="ts">
// §8.3 tree mode — a collection's addonfor chain flattened into rows. Data,
// not inscription: Alegreya rows, menu nodes muted, leaves with their state
// and kind badges. A crumb click lands here, scrolled to that node.
// v0.3 (R16.3-R16.5): the library tree's grammar. Rows lie on the void, 32 px,
// a leaf carries a 2 px rail in its state colour, each level hangs under a
// 1 px guide at its parent's chevron, and the focus ring sits inside the row.
import { nextTick, onMounted } from "vue";
import type { TreeNode } from "@/types/rift";
import { kindsPresent } from "@/services/format";
import KindBadge from "./KindBadge.vue";
import StateChip from "./StateChip.vue";

const {
  nodes,
  depth = 0,
  focusName = null,
  selectedId = null,
} = defineProps<{
  nodes: TreeNode[];
  depth?: number;
  focusName?: string | null;
  selectedId?: string | null;
}>();
const emit = defineEmits<{ open: [optionId: string] }>();

/** The text start of a row at this depth: 12 px inset, 16 px a level (R16.4). */
const INSET = 12;
const STEP = 16;
/** The guide runs down the middle of the parent row's 10 px chevron. */
const GUIDE_AT = INSET + 5;

/** R16.3: a leaf's rail says its state; a menu or a bundle installs nothing. */
function railOf(n: TreeNode): "rift" | "world" | "ember" | null {
  if (n.kind === "bundle" || n.menuOnly || !n.optionId) return null;
  if (n.conflicted) return "ember";
  return n.installed ? "world" : "rift";
}

onMounted(async () => {
  if (depth !== 0 || !focusName) return;
  await nextTick();
  const el = document.querySelector<HTMLElement>(`[data-node="${CSS.escape(focusName)}"]`);
  el?.scrollIntoView({ block: "center" });
  el?.focus();
});
</script>

<template>
  <ul
    class="ptr"
    :class="{ 'ptr--root': depth === 0 }"
    :role="depth === 0 ? 'tree' : 'group'"
    :style="depth === 0 ? undefined : { '--guide-x': `${GUIDE_AT + (depth - 1) * STEP}px` }"
  >
    <li
      v-for="n in nodes"
      :key="n.optionId ?? n.label"
      role="treeitem"
      :aria-expanded="n.children.length ? true : undefined"
    >
      <button
        type="button"
        class="ptr__row"
        :class="[
          railOf(n) ? `rail--${railOf(n)}` : null,
          {
            'is-menu': n.menuOnly || n.kind === 'bundle',
            'is-sel': n.optionId !== null && n.optionId === selectedId,
            'is-focus': n.label === focusName,
          },
        ]"
        :style="{ paddingLeft: `${depth * STEP + INSET}px` }"
        :data-node="n.label"
        :disabled="!n.optionId"
        @click="n.optionId && emit('open', n.optionId)"
      >
        <span class="ptr__glyph" aria-hidden="true">{{ n.children.length ? "▾" : "·" }}</span>
        <span class="ptr__label" :title="n.label">{{ n.label }}</span>
        <!-- a menu or a bundle node installs nothing: no chips (R16.3) -->
        <span v-if="n.kind !== 'bundle' && !(n.menuOnly && n.children.length)" class="ptr__meta">
          <KindBadge v-for="k in kindsPresent(n.kinds)" :key="k" :kind="k" />
          <StateChip v-if="n.menuOnly" state="menu" />
          <StateChip v-else-if="n.conflicted && n.optionId" state="conflict" />
          <StateChip v-else :state="n.installed ? 'installed' : 'notInstalled'" />
        </span>
      </button>
      <PackageTree
        v-if="n.children.length"
        :nodes="n.children"
        :depth="depth + 1"
        :selected-id="selectedId"
        :focus-name="focusName"
        @open="(id) => emit('open', id)"
      />
    </li>
  </ul>
</template>

<style scoped>
.ptr {
  position: relative;
  margin: 0;
  padding: 0;
  list-style: none;
}
/* the level's guide under its parent's chevron: decorative, the indent is the boundary (R16.4) */
.ptr:not(.ptr--root)::before {
  content: "";
  position: absolute;
  top: 0;
  bottom: 4px;
  left: var(--guide-x);
  width: 1px;
  background: var(--rs-edge);
  pointer-events: none;
}
/* an option on the void: no ground, no gap, 32 px (R16.3, R16.4) */
.ptr__row {
  position: relative;
  display: flex;
  align-items: center;
  gap: var(--rs-space-2);
  width: 100%;
  height: 32px;
  padding: 0 var(--rs-space-3) 0 12px;
  border: 1px solid transparent;
  background: none;
  color: var(--rs-text);
  font: 400 var(--rs-text-sm) var(--rs-font-sans);
  text-align: left;
  white-space: nowrap;
  cursor: pointer;
}
/* R16.5: the hover is the only time an option row has a ground */
.ptr__row:hover {
  background: var(--rs-misthi);
}
.ptr__row:disabled {
  cursor: default;
}
.ptr__row.is-menu {
  color: var(--rs-muted);
  font-weight: 500;
}
.ptr__row.is-sel {
  background: var(--rs-misthi);
  border-color: var(--rs-edgehi);
}
/* R16.5: the ring sits INSIDE the row; the column clips an outside one. The
   crumb's landing holds the focus, so it wears the same ring */
.ptr__row:focus-visible,
.ptr__row.is-focus:focus {
  outline: 2px solid var(--rs-rift);
  outline-offset: -2px;
}
/* a leaf's 2 px rail, half the package's (R16.3); drawn, not a border, so
   every row keeps the same box and the indent reads from one text start */
.ptr__row.rail--rift::before,
.ptr__row.rail--world::before,
.ptr__row.rail--ember::before {
  content: "";
  position: absolute;
  top: 0;
  bottom: 0;
  left: 0;
  width: 2px;
  background: var(--rs-rift);
}
.ptr__row.rail--world::before {
  background: var(--rs-world);
}
.ptr__row.rail--ember::before {
  background: var(--rs-ember);
}
.ptr__glyph {
  flex: none;
  width: 10px;
  color: var(--rs-faint);
}
.ptr__label {
  flex: 1 1 auto;
  min-width: 12ch;
  overflow: hidden;
  text-overflow: ellipsis;
}
.ptr__meta {
  flex: none;
  display: flex;
  align-items: center;
  gap: 4px;
}
</style>
