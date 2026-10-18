// v0.5 — THE UNDERCROFT (#00015 R18.11–R18.14): what the folder beside the
// mods folder, `mods (older versions)`, holds, and the one door back. The
// folder is outside every watcher (WINDOW-CONTRACT § v0.5), so the count is
// read at the moments R18.11 names, never live: entering THE RIFT, each open
// of the fold, and after an install that put something away, a keep-newest, a
// put-away and a bring-back (their callers ask for it).

import { ref } from "vue";
import type { OlderShelfView } from "@/types/rift";
import { asRiftFailure, rift } from "@/services/rift";
import { failureDetails, voice, type Voiced } from "@/services/voice";

export interface ShelfRowFailure {
  voiced: Voiced;
  details: string;
}

const view = ref<OlderShelfView | null>(null);
const failure = ref<Voiced | null>(null);
const open = ref(false);
/** The row whose move runs now, by file name. */
const moving = ref<string | null>(null);
/** A row's own failed move, by file name (R18.13). */
const rowFailures = ref<ReadonlyMap<string, ShelfRowFailure>>(new Map());
let requestSeq = 0;

async function load(): Promise<void> {
  const mine = ++requestSeq;
  try {
    const backend = await rift();
    const next = await backend.getOlderVersions();
    if (mine !== requestSeq) return;
    view.value = next;
    failure.value = null;
  } catch (raw) {
    if (mine === requestSeq) failure.value = voice(asRiftFailure(raw), "olderVersions");
  }
}

function toggle(): void {
  open.value = !open.value;
  if (open.value) void load();
}

function setRowFailure(fileName: string, value: ShelfRowFailure | null): void {
  const next = new Map(rowFailures.value);
  if (value) next.set(fileName, value);
  else next.delete(fileName);
  rowFailures.value = next;
}

/** Moves one item back into the mods folder; its landed name, or null when it failed. */
async function bringBack(fileName: string): Promise<string | null> {
  if (moving.value) return null;
  moving.value = fileName;
  setRowFailure(fileName, null);
  try {
    const backend = await rift();
    const result = await backend.bringBack(fileName);
    await load();
    return result.landedAs;
  } catch (raw) {
    const f = asRiftFailure(raw);
    const detail = { ...f.detail, act: "bringBack", file: fileName };
    const failed = { kind: f.kind, detail };
    setRowFailure(fileName, {
      voiced: voice(failed, "olderVersions"),
      details: failureDetails(failed, "olderVersions"),
    });
    return null;
  } finally {
    moving.value = null;
  }
}

/** Q18.6 (ruled): `Show in Explorer`, only once the folder exists. */
async function showInExplorer(): Promise<Voiced | null> {
  try {
    const backend = await rift();
    await backend.openOlderVersionsFolder();
    return null;
  } catch (raw) {
    const f = asRiftFailure(raw);
    return voice({ kind: f.kind, detail: { ...f.detail, act: "open" } }, "olderVersions");
  }
}

export function useUndercroft() {
  return { view, failure, open, moving, rowFailures, load, toggle, bringBack, showInExplorer };
}

export function resetUndercroft(): void {
  requestSeq = 0;
  view.value = null;
  failure.value = null;
  open.value = false;
  moving.value = null;
  rowFailures.value = new Map();
}
