// THE LIBRARY TREE (v0.3): every package in the mods folder with its options
// nested the way their menus nest them (get_library) — the Rift's idle view,
// shown while the well holds no words and no facet narrows (useSearch.idle).
// Read again after rift://library-changed, rift://game-changed and
// rift://updates-changed (boot.ts).
//
// Expansion belongs to the player for the session: a Set of package ids and
// node keys, never stored, kept across refetches and searches, so clearing the
// well returns to the tree exactly as it was (WINDOW-CONTRACT § v0.3 The
// library tree). `rows` is the tree flattened to what is visible now, in
// order, with what a WAI-ARIA tree needs per row (level, set size, position,
// parent): one tab stop, ↑ ↓ ← → Home End, built by the screen that draws it.

import { computed, ref } from "vue";
import type { LibraryPackage, LibraryView, TreeNode } from "@/types/rift";
import { asRiftFailure, rift } from "@/services/rift";
import { voice, type Voiced } from "@/services/voice";

export type LibraryRowKind = "package" | "option" | "bundle";

export interface LibraryRow {
  /** The expansion key: a package's id, an option's id, or a bundle's key (nodeKey). */
  key: string;
  kind: LibraryRowKind;
  label: string;
  /** 1 for a package, 2 for its top nodes, and so on (aria-level). */
  level: number;
  /** aria-setsize and aria-posinset (1-based) among the row's siblings. */
  setSize: number;
  position: number;
  /** The key of the row this one sits under; null for a package. */
  parent: string | null;
  /** It has children to show or hide. */
  branch: boolean;
  expanded: boolean;
  packageId: string;
  /** Set for an option row; null for a package or a bundle. Opaque: never parsed. */
  optionId: string | null;
  /** The package row's own data, carried on every row of that package. */
  pkg: LibraryPackage;
  /** null for a package row. */
  node: TreeNode | null;
}

const view = ref<LibraryView | null>(null);
const failure = ref<Voiced | null>(null);
const loading = ref(false);
/** Package ids and node keys the player opened this session. */
const expanded = ref<ReadonlySet<string>>(new Set());
/** The tree's one active row (the roving tab stop), by key; the keyboard's
 *  I and L read it while the tree is on screen. Per session, like expansion. */
const focusedKey = ref<string | null>(null);
let requestSeq = 0;

/** A tree node's expansion key. An option is its own id (unique across the
 *  library); a bundle has no id, so it is keyed by where it sits — its
 *  parent's key and its label, case-blind, as the spine gathers bundles. */
export function nodeKey(node: TreeNode, parentKey: string): string {
  return node.optionId ?? `bundle|${parentKey}|${node.label.toLowerCase()}`;
}

async function load(): Promise<void> {
  const mine = ++requestSeq;
  loading.value = true;
  try {
    const backend = await rift();
    const next = await backend.getLibrary();
    if (mine !== requestSeq) return;
    view.value = next;
    failure.value = null;
  } catch (raw) {
    if (mine === requestSeq) failure.value = voice(asRiftFailure(raw), "modsFolder");
  } finally {
    if (mine === requestSeq) loading.value = false;
  }
}

const packages = computed(() => view.value?.packages ?? []);
const mods = computed(() => view.value?.mods ?? null);
/** The mods folder answered and holds no package at all. */
const empty = computed(() => view.value?.packages.length === 0);

function nodeRows(
  nodes: TreeNode[],
  at: { pkg: LibraryPackage; parent: string; level: number },
  open: ReadonlySet<string>,
): LibraryRow[] {
  const out: LibraryRow[] = [];
  for (const [i, node] of nodes.entries()) {
    const key = nodeKey(node, at.parent);
    const branch = node.children.length > 0;
    const isOpen = branch && open.has(key);
    out.push({
      key,
      kind: node.kind,
      label: node.label,
      level: at.level,
      setSize: nodes.length,
      position: i + 1,
      parent: at.parent,
      branch,
      expanded: isOpen,
      packageId: at.pkg.package.id,
      optionId: node.optionId,
      pkg: at.pkg,
      node,
    });
    if (isOpen) out.push(...nodeRows(node.children, { pkg: at.pkg, parent: key, level: at.level + 1 }, open));
  }
  return out;
}

/** The tree as it is visible now: every package, and under each open one its
 *  open branches, in order. */
const rows = computed<LibraryRow[]>(() => {
  const open = expanded.value;
  const list = packages.value;
  const out: LibraryRow[] = [];
  for (const [i, pkg] of list.entries()) {
    const key = pkg.package.id;
    const branch = pkg.tree.length > 0;
    const isOpen = branch && open.has(key);
    out.push({
      key,
      kind: "package",
      label: pkg.package.displayName,
      level: 1,
      setSize: list.length,
      position: i + 1,
      parent: null,
      branch,
      expanded: isOpen,
      packageId: key,
      optionId: null,
      pkg,
      node: null,
    });
    if (isOpen) out.push(...nodeRows(pkg.tree, { pkg, parent: key, level: 2 }, open));
  }
  return out;
});

function setOpen(key: string, open: boolean): void {
  if (expanded.value.has(key) === open) return;
  const next = new Set(expanded.value);
  if (open) next.add(key);
  else next.delete(key);
  expanded.value = next;
}

/** The keys from a package down to the node holding `optionId`, or null. */
function pathTo(optionId: string): string[] | null {
  const walk = (nodes: TreeNode[], parent: string, trail: string[]): string[] | null => {
    for (const node of nodes) {
      const key = nodeKey(node, parent);
      if (node.optionId === optionId) return trail;
      const deeper = walk(node.children, key, [...trail, key]);
      if (deeper) return deeper;
    }
    return null;
  };
  for (const pkg of packages.value) {
    const found = walk(pkg.tree, pkg.package.id, [pkg.package.id]);
    if (found) return found;
  }
  return null;
}

/** Every package and node key that has children: what Expand all opens. */
function branchKeys(): string[] {
  const out: string[] = [];
  const walk = (nodes: TreeNode[], parent: string): void => {
    for (const node of nodes) {
      if (!node.children.length) continue;
      const key = nodeKey(node, parent);
      out.push(key);
      walk(node.children, key);
    }
  };
  for (const pkg of packages.value) {
    if (!pkg.tree.length) continue;
    out.push(pkg.package.id);
    walk(pkg.tree, pkg.package.id);
  }
  return out;
}

/** The visible row the tree's focus sits on, or null. */
const focusedRow = computed(() => rows.value.find((r) => r.key === focusedKey.value) ?? null);

/** Open every branch above an option so its row is visible (a search answer
 *  the player opened, say). Returns whether the option is in the tree. */
function reveal(optionId: string): boolean {
  const path = pathTo(optionId);
  if (!path) return false;
  const next = new Set(expanded.value);
  for (const key of path) next.add(key);
  expanded.value = next;
  return true;
}

export function useLibrary() {
  return {
    view,
    failure,
    loading,
    packages,
    mods,
    empty,
    rows,
    expanded,
    load,
    isExpanded: (key: string) => expanded.value.has(key),
    toggle: (key: string) => setOpen(key, !expanded.value.has(key)),
    expand: (key: string) => setOpen(key, true),
    collapse: (key: string) => setOpen(key, false),
    collapseAll: () => (expanded.value = new Set()),
    expandAll: () => (expanded.value = new Set(branchKeys())),
    reveal,
    focusedKey,
    focusedRow,
  };
}

export function resetLibrary(): void {
  requestSeq = 0;
  view.value = null;
  failure.value = null;
  loading.value = false;
  expanded.value = new Set();
  focusedKey.value = null;
}
