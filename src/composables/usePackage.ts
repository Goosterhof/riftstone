// One package opened in the Rift: its addonfor tree (tree mode) and the
// lookbook sections built from it — every menu node that holds variants
// becomes a section of screenshot tiles.

import { computed, ref, watch } from "vue";
import type { PackageDetail, TreeNode } from "@/types/rift";
import { asRiftFailure, rift } from "@/services/rift";
import { voice, type Voiced } from "@/services/voice";
import { useNavigation } from "./useNavigation";

export interface LookbookTile {
  optionId: string;
  name: string;
  short: string;
  crumb: string;
  installed: boolean;
  hasScreenshot: boolean;
}

export interface LookbookSection {
  heading: string;
  tiles: LookbookTile[];
}

const detail = ref<PackageDetail | null>(null);
const failure = ref<Voiced | null>(null);
const filter = ref("");

/** `fresh`: the world changed (an install, a rescan) — read the open package
 *  again past the id cache, keeping the old tree on screen until it lands so
 *  the grid (and the keyboard's place in it) never unmounts. */
async function load(id: string | null, fresh = false): Promise<void> {
  if (!id) {
    detail.value = null;
    return;
  }
  if (detail.value?.package.id === id && !fresh) return;
  if (detail.value?.package.id !== id) detail.value = null;
  try {
    const backend = await rift();
    detail.value = await backend.getPackage(id);
    failure.value = null;
  } catch (raw) {
    failure.value = voice(asRiftFailure(raw), "package");
  }
}

/** "Ermine Cape - Variation 3" under "Ermine Cape (Menu Option)" → "Variation 3". */
export function shortName(name: string, parent: string): string {
  const base = parent.replace(/\s*\(menu option\)\s*$/i, "").trim();
  if (!base || !name.toLowerCase().startsWith(base.toLowerCase())) return name;
  const rest = name.slice(base.length).replace(/^[\s\-—–:·]+/, "");
  return rest || name;
}

function tileOf(node: TreeNode, parent: string): LookbookTile | null {
  if (!node.optionId || node.menuOnly) return null;
  return {
    optionId: node.optionId,
    name: node.label,
    short: shortName(node.label, parent),
    crumb: parent,
    installed: node.installed,
    hasScreenshot: node.hasScreenshot,
  };
}

function collect(nodes: TreeNode[], heading: string, out: LookbookSection[]): void {
  const tiles = nodes.map((n) => tileOf(n, heading)).filter((t): t is LookbookTile => t !== null);
  if (tiles.length) out.push({ heading, tiles });
  for (const n of nodes) if (n.children.length) collect(n.children, n.label, out);
}

const sections = computed<LookbookSection[]>(() => {
  const d = detail.value;
  if (!d) return [];
  const out: LookbookSection[] = [];
  collect(d.tree, d.package.displayName, out);
  return out;
});

const tiles = computed(() => sections.value.flatMap((s) => s.tiles));

function matchesFilter(node: TreeNode, needle: string): boolean {
  return node.label.toLowerCase().includes(needle) || node.children.some((c) => matchesFilter(c, needle));
}

function prune(nodes: TreeNode[], needle: string): TreeNode[] {
  return nodes
    .filter((n) => matchesFilter(n, needle))
    .map((n) => ({ ...n, children: n.label.toLowerCase().includes(needle) ? n.children : prune(n.children, needle) }));
}

const tree = computed(() => {
  const nodes = detail.value?.tree ?? [];
  const needle = filter.value.trim().toLowerCase();
  return needle ? prune(nodes, needle) : nodes;
});

const { packageId } = useNavigation();
watch(packageId, (id) => void load(id), { immediate: true });

export function usePackage() {
  return { detail, failure, filter, sections, tiles, tree, reload: () => load(packageId.value, true) };
}

export function resetPackage(): void {
  detail.value = null;
  failure.value = null;
  filter.value = "";
}
