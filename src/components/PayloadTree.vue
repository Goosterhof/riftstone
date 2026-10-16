<script setup lang="ts">
// §7.6 — what an option carries. Mono 12, 22 px rows, 16 px indent. A matched
// file gets the 2 px rift rule and its hit lit; ancestors of a match open by
// themselves. Skipped files close the tree, struck, with the reason.
import { computed, ref, watch } from "vue";
import type { ModOption } from "@/types/rift";
import { buildFileTree, skippedRows, type FileNode } from "@/services/payloadTree";
import { formatBytes } from "@/services/format";
import HitText from "./HitText.vue";

const { option, tokens = [] } = defineProps<{ option: ModOption; tokens?: string[] }>();

const tree = computed(() => buildFileTree(option.payload, option.root, tokens));
const skipped = computed(() => skippedRows(option.skipped, option.root));
const open = ref<Set<string>>(new Set());

interface Row {
  node: FileNode;
  depth: number;
}

function defaultOpen(nodes: FileNode[], small: boolean, into: Set<string>): void {
  for (const n of nodes) {
    if (!n.folder) continue;
    if (small || n.matched) into.add(n.path);
    defaultOpen(n.children, small, into);
  }
}

watch(
  tree,
  (nodes) => {
    const next = new Set<string>();
    defaultOpen(nodes, option.payload.length <= 24, next);
    open.value = next;
  },
  { immediate: true },
);

function flatten(nodes: FileNode[], depth: number, out: Row[]): Row[] {
  for (const node of nodes) {
    out.push({ node, depth });
    if (node.folder && open.value.has(node.path)) flatten(node.children, depth + 1, out);
  }
  return out;
}

const rows = computed(() => flatten(tree.value, 0, []));

function toggle(path: string): void {
  const next = new Set(open.value);
  if (next.has(path)) next.delete(path);
  else next.add(path);
  open.value = next;
}
</script>

<template>
  <div class="ptree" role="tree" aria-label="Files this option installs">
    <p v-if="!rows.length" class="ptree__none">This option installs no files: it is a menu entry.</p>
    <div
      v-for="row in rows"
      :key="row.node.path"
      class="ptree__row"
      :class="{ 'is-folder': row.node.folder, 'is-hit': row.node.matched && !row.node.folder }"
      role="treeitem"
      :aria-expanded="row.node.folder ? open.has(row.node.path) : undefined"
      :style="{ paddingLeft: `${row.depth * 16 + 6}px` }"
    >
      <button v-if="row.node.folder" type="button" class="ptree__toggle" @click="toggle(row.node.path)">
        <span aria-hidden="true">{{ open.has(row.node.path) ? "▾" : "▸" }}</span> {{ row.node.name }}/
      </button>
      <span v-else class="ptree__name"><HitText :text="row.node.name" :tokens="tokens" /></span>
      <span class="ptree__size">{{ formatBytes(row.node.size) }}</span>
    </div>
    <template v-if="skipped.length">
      <div class="ptree__group rs-mono">SKIPPED</div>
      <div v-for="s in skipped" :key="s.path" class="ptree__row is-skipped">
        <span class="ptree__name"
          ><s>{{ s.path }}</s></span
        >
        <span class="ptree__size">{{ s.reason }}</span>
      </div>
    </template>
  </div>
</template>

<style scoped>
.ptree {
  font: 400 var(--rs-text-xs) / 22px var(--rs-font-mono);
}
.ptree__none {
  margin: 0;
  color: var(--rs-faint);
  font-family: var(--rs-font-sans);
  font-size: var(--rs-text-sm);
}
.ptree__row {
  display: flex;
  align-items: center;
  gap: var(--rs-space-2);
  min-height: 22px;
  padding-right: 6px;
  border-left: 2px solid transparent;
  color: var(--rs-text);
}
.ptree__row.is-hit {
  border-left-color: var(--rs-rift);
  background: rgba(143, 227, 242, 0.05);
}
.ptree__toggle {
  flex: 1;
  min-width: 0;
  padding: 0;
  border: 0;
  background: none;
  color: var(--rs-muted);
  font: inherit;
  text-align: left;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: pointer;
}
.ptree__name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ptree__size {
  flex: none;
  color: var(--rs-faint);
}
.ptree__group {
  margin-top: var(--rs-space-3);
  padding-left: 6px;
  color: var(--rs-faint);
  letter-spacing: 0.1em;
}
.is-skipped {
  color: var(--rs-faint);
}
</style>
