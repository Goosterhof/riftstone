<script setup lang="ts">
// §8.3 tree mode — a collection's addonfor chain flattened into rows. Data,
// not inscription: Alegreya rows, menu nodes muted, leaves with their state
// and kind badges. A crumb click lands here, scrolled to that node.
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

onMounted(async () => {
  if (depth !== 0 || !focusName) return;
  await nextTick();
  const el = document.querySelector<HTMLElement>(`[data-node="${CSS.escape(focusName)}"]`);
  el?.scrollIntoView({ block: "center" });
  el?.focus();
});
</script>

<template>
  <ul class="ptr" :class="{ 'ptr--root': depth === 0 }" :role="depth === 0 ? 'tree' : 'group'">
    <li
      v-for="n in nodes"
      :key="n.optionId ?? n.label"
      role="treeitem"
      :aria-expanded="n.children.length ? true : undefined"
    >
      <button
        type="button"
        class="ptr__row"
        :class="{
          'is-menu': n.menuOnly || n.kind === 'bundle',
          'is-sel': n.optionId === selectedId,
          'is-focus': n.label === focusName,
        }"
        :style="{ paddingLeft: `${depth * 16 + 12}px` }"
        :data-node="n.label"
        :disabled="!n.optionId"
        @click="n.optionId && emit('open', n.optionId)"
      >
        <span class="ptr__glyph" aria-hidden="true">{{ n.children.length ? "▾" : "·" }}</span>
        <span class="ptr__label">{{ n.label }}</span>
        <span class="ptr__meta">
          <KindBadge v-for="k in kindsPresent(n.kinds)" :key="k" :kind="k" />
          <StateChip v-if="n.menuOnly" state="menu" />
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
  margin: 0;
  padding: 0;
  list-style: none;
}
.ptr__row {
  display: flex;
  align-items: center;
  gap: var(--rs-space-2);
  width: 100%;
  min-height: 34px;
  padding: 4px var(--rs-space-3) 4px 12px;
  margin-bottom: 2px;
  border: 1px solid transparent;
  background: var(--rs-mist);
  color: var(--rs-text);
  font: 400 var(--rs-text-sm) var(--rs-font-sans);
  text-align: left;
  cursor: pointer;
}
.ptr__row:hover {
  background: var(--rs-misthi);
}
.ptr__row:disabled {
  cursor: default;
}
.ptr__row.is-menu {
  background: none;
  color: var(--rs-muted);
  font-weight: 500;
}
.ptr__row.is-sel {
  background: var(--rs-misthi);
  border-color: var(--rs-edgehi);
}
.ptr__row.is-focus {
  box-shadow: inset 2px 0 0 var(--rs-rift);
}
.ptr__glyph {
  flex: none;
  width: 10px;
  color: var(--rs-faint);
}
.ptr__label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ptr__meta {
  flex: none;
  display: flex;
  gap: 4px;
}
</style>
