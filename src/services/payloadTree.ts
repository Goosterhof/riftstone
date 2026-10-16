// Fold an option's flat payload into the folder tree PayloadTree.vue draws.

import type { PayloadFile, SkipReason, SkippedFile } from "@/types/rift";
import { containsAnyToken } from "./highlight";

export interface FileNode {
  name: string;
  path: string;
  size: number;
  folder: boolean;
  matched: boolean;
  children: FileNode[];
}

function relative(entry: string, root: string): string {
  if (!root) return entry;
  const prefix = `${root}/`;
  return entry.toLowerCase().startsWith(prefix.toLowerCase()) ? entry.slice(prefix.length) : entry;
}

function child(parent: FileNode, name: string, path: string): FileNode {
  let node = parent.children.find((c) => c.name === name && c.folder);
  if (!node) {
    node = { name, path, size: 0, folder: true, matched: false, children: [] };
    parent.children.push(node);
  }
  return node;
}

function sortTree(node: FileNode): void {
  node.children.sort((a, b) => Number(b.folder) - Number(a.folder) || a.name.localeCompare(b.name));
  for (const c of node.children) sortTree(c);
}

/** Folders roll up their size and whether any file inside matched. */
function rollUp(node: FileNode): void {
  if (!node.folder) return;
  for (const c of node.children) rollUp(c);
  node.size = node.children.reduce((n, c) => n + c.size, 0);
  node.matched = node.children.some((c) => c.matched);
}

/** "natives/" → "stm/" → "appsystem/" each holding one folder is a staircase
 *  of rows that says nothing: a folder whose only child is a folder folds into
 *  one row, "natives/stm/appsystem/mantleWb/mesh". */
function compact(node: FileNode): FileNode {
  if (!node.folder) return node;
  let at = node;
  let name = node.name;
  while (at.children.length === 1 && at.children[0]?.folder) {
    at = at.children[0];
    name = `${name}/${at.name}`;
  }
  return { ...at, name, children: at.children.map(compact) };
}

export function buildFileTree(payload: PayloadFile[], root: string, tokens: string[]): FileNode[] {
  const top: FileNode = { name: "", path: "", size: 0, folder: true, matched: false, children: [] };
  for (const file of payload) {
    const rel = relative(file.entry, root);
    const parts = rel.split("/");
    let cursor = top;
    for (const [i, part] of parts.slice(0, -1).entries()) cursor = child(cursor, part, parts.slice(0, i + 1).join("/"));
    cursor.children.push({
      name: parts.at(-1) ?? rel,
      path: rel,
      size: file.size,
      folder: false,
      matched: tokens.length > 0 && containsAnyToken(rel, tokens),
      children: [],
    });
  }
  rollUp(top);
  sortTree(top);
  return top.children.map(compact);
}

export const SKIP_REASON: Record<SkipReason, string> = {
  modinfo: "Fluffy metadata",
  picture: "preview only",
  document: "documentation",
  unknownPlace: "not a place the game loads from",
};

export function skippedRows(skipped: SkippedFile[], root: string): { path: string; reason: string }[] {
  return skipped.map((s) => ({ path: relative(s.entry, root), reason: SKIP_REASON[s.reason] }));
}
