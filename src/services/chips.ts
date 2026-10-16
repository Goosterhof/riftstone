// Which state chips a card shows (§7.3): at most two, CONFLICT > UPDATE >
// INSTALLED / NOT INSTALLED. Temperature is state; the word always rides along.

import type { SearchResult } from "@/types/rift";

export type ChipState = "installed" | "notInstalled" | "update" | "conflict" | "switchOff" | "fluffy" | "menu";

/** The rail's temperature: ember beats gold beats cyan. */
export type RailState = "rift" | "world" | "ember" | "menu";

export function chipsFor(hit: Pick<SearchResult, "conflicted" | "update" | "installed" | "menuOnly">): ChipState[] {
  if (hit.menuOnly) return ["menu"];
  const chips: ChipState[] = [];
  if (hit.conflicted) chips.push("conflict");
  if (hit.update === "available") chips.push("update");
  if (hit.installed?.origin === "fluffyUnadopted") chips.push("fluffy");
  else chips.push(hit.installed ? "installed" : "notInstalled");
  return chips.slice(0, 2);
}

export function railFor(hit: Pick<SearchResult, "conflicted" | "installed" | "menuOnly">): RailState {
  if (hit.menuOnly) return "menu";
  if (hit.conflicted) return "ember";
  return hit.installed ? "world" : "rift";
}
