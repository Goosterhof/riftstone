// v0.5 — versions of one file (#00015 §18, WINDOW-CONTRACT § v0.5): the one
// fold every surface reads, and every sentence the versions say. Components
// never compose this copy (R18.30); the failures of a move are voiced in
// ./voice.ts (context `olderVersions`). One narrator, the stone, in the third
// person. A folder is named `mods (older versions)`, never a path.
//
// The spine decides which package leads (`family.lead`) and which option of one
// version is the same option as which of another (`OlderVersionView.matches`):
// this module only groups by the one and reads the other, never works either out.

import type { LibraryPackage, OlderVersionView, ReplacesView } from "@/types/rift";
import { plural } from "./format";

/** The folder's plain name, in every label and sentence (the inscription is THE UNDERCROFT). */
export const OLDER_FOLDER = "mods (older versions)";
export const UNDERCROFT = "THE UNDERCROFT";
export const VERSIONS_HEAD = "VERSIONS OF THIS FILE";
export const REPLACES_HEAD = "TAKES THE PLACE OF";
export const KEEP_NEWEST_HEAD = "THE RETURN — OLDER VERSIONS";

/** One entry of the Rift's tree: the lead, with its older versions inside it, newest first. */
export interface FamilyEntry {
  lead: LibraryPackage;
  older: { pkg: LibraryPackage; version: OlderVersionView }[];
}

/** R18.1–R18.2 (§18.13): every package in library order, a family folded into
 *  its lead's entry. A package whose `family.lead` is another package that the
 *  library lists is drawn inside that lead, never as an entry of its own. */
export function foldFamilies(packages: readonly LibraryPackage[]): FamilyEntry[] {
  const byId = new Map(packages.map((p) => [p.package.id, p]));
  const entries: FamilyEntry[] = [];
  for (const pkg of packages) {
    const lead = pkg.family?.lead;
    if (lead && lead !== pkg.package.id && byId.has(lead)) continue;
    const older = (pkg.family?.older ?? []).flatMap((version) => {
      const found = byId.get(version.packageId);
      return found ? [{ pkg: found, version }] : [];
    });
    entries.push({ lead: pkg, older });
  }
  return entries;
}

/** An older version's place: its lead and its own entry in `family.older`, or
 *  null for a lead or a package in no family. */
export function olderPlace(
  packages: readonly LibraryPackage[],
  packageId: string,
): { lead: LibraryPackage; version: OlderVersionView } | null {
  const pkg = packages.find((p) => p.package.id === packageId);
  const leadId = pkg?.family?.lead;
  if (!pkg || !leadId || leadId === packageId) return null;
  const lead = packages.find((p) => p.package.id === leadId);
  const version = lead?.family?.older.find((o) => o.packageId === packageId);
  return lead && version ? { lead, version } : null;
}

/** The members of the family a package is in, newest first, and its place among
 *  them; null for a package in no family the library folds. */
export function familyMembers(
  packages: readonly LibraryPackage[],
  packageId: string,
): { members: LibraryPackage[]; index: number } | null {
  const entry = foldFamilies(packages).find(
    (e) => e.older.length > 0 && [e.lead, ...e.older.map((o) => o.pkg)].some((m) => m.package.id === packageId),
  );
  if (!entry) return null;
  const members = [entry.lead, ...entry.older.map((o) => o.pkg)];
  return { members, index: members.findIndex((m) => m.package.id === packageId) };
}

const holdsOption = (nodes: readonly LibraryPackage["tree"][number][], optionId: string): boolean =>
  nodes.some((n) => n.optionId === optionId || holdsOption(n.children, optionId));

/** R18.9: what THE CROSSING says about a family: the newer version a rollback
 *  takes out of your game (null when the install is not a rollback), and whether
 *  any member is older than the package it installs (the put-away sentence). */
export function crossingFamily(
  packages: readonly LibraryPackage[],
  packageId: string,
  replaces: readonly ReplacesView[],
): { rollbackFrom: string | null; olderRemain: boolean } {
  const place = familyMembers(packages, packageId);
  if (!place) return { rollbackFrom: null, olderRemain: false };
  const newer = place.members.slice(0, Math.max(0, place.index));
  const back = replaces.find((r) => newer.some((m) => holdsOption(m.tree, r.optionId)));
  return {
    rollbackFrom: back ? versionName(back.version, back.name) : null,
    olderRemain: place.index >= 0 && place.index < place.members.length - 1,
  };
}

/** Every member's file name → its version as the copy names it, for lines told after a scan. */
export function familyNames(packages: readonly LibraryPackage[], packageId: string): Map<string, string> {
  const place = familyMembers(packages, packageId);
  return new Map(
    (place?.members ?? []).map((m) => [m.package.fileName, versionName(m.family?.version ?? null, m.package.fileName)]),
  );
}

/** `v1.3`; a leading `v` is never doubled. */
export const versionLabel = (version: string): string => `v${version.replace(/^v/i, "")}`;

/** A version as the copy names it: `v1.2`, else the file's own name (a version Nexus never named). */
export const versionName = (version: string | null, fileName: string): string =>
  version ? versionLabel(version) : fileName;

/** The older versions named together: `v1.2`, `v1.2 and v1.1`, `3 older versions`. */
export function olderNames(older: readonly Pick<OlderVersionView, "version" | "fileName">[]): string {
  const names = older.map((o) => versionName(o.version, o.fileName));
  if (names.length >= 3) return `${names.length} older versions`;
  return names.join(" and ");
}

const isAre = (n: number): string => (n === 1 ? "is" : "are");

// --- R18.1: the lead's row ---------------------------------------------------

export interface OlderMark {
  /** `+1 older` / `+1 older, installed`. */
  text: string;
  /** The folded form below the threshold (Q18.1's fallback): `+1`. */
  short: string;
  warn: boolean;
  /** The words a screen reader hears, and the folded mark's title. */
  hidden: string;
}

export function olderMark(older: readonly OlderVersionView[]): OlderMark | null {
  if (!older.length) return null;
  const n = older.length;
  const warn = older.some((o) => o.installed > 0);
  const versions = n === 1 ? "1 older version" : `${n} older versions`;
  return {
    text: warn ? `+${n} older, installed` : `+${n} older`,
    short: `+${n}`,
    warn,
    hidden: warn ? `${versions} of this file, still installed` : `${versions} of this file in your mods folder`,
  };
}

/** R18.2: a version row's label. */
export const versionRowLabel = (version: string | null): string =>
  version ? `${versionLabel(version)} · older version` : "older version";

export const versionRowState = (installed: number): string =>
  installed > 0 ? `${installed} in your world` : "nothing installed";

/** R18.3: the lead leaf's chip suffix when its option is in your world only from
 *  an older version (a match whose `leadOptionId` is the leaf's is installed). */
export function installedFromOlder(
  lead: Pick<LibraryPackage, "family">,
  leadOptionId: string,
  leafInstalled: boolean,
): OlderVersionView | null {
  if (leafInstalled) return null;
  return lead.family?.older.find((o) => o.matches.some((m) => m.leadOptionId === leadOptionId && m.installed)) ?? null;
}

/** A lead leaf installed itself while an older version of the same option is still installed. */
export function stackedUnder(
  lead: Pick<LibraryPackage, "family">,
  leadOptionId: string,
  leafInstalled: boolean,
): OlderVersionView | null {
  if (!leafInstalled) return null;
  return lead.family?.older.find((o) => o.matches.some((m) => m.leadOptionId === leadOptionId && m.installed)) ?? null;
}

// --- R18.4: the option detail's line -----------------------------------------

export interface DetailLine {
  text: string;
  door: "install" | "keepNewest";
  doorLabel: string;
}

export function leadOptionLine(old: string, lead: string, stacked: boolean): DetailLine {
  return stacked
    ? {
        text: `${old} of this option is still installed under it, so any of its files ${lead} lacks are still in your game.`,
        door: "keepNewest",
        doorLabel: "Keep only the newest…",
      }
    : {
        text: `${old} of this option is in your world. Installing ${lead} takes its place in one step.`,
        door: "install",
        doorLabel: `Install ${lead}…`,
      };
}

export const olderOptionLine = (old: string, lead: string): string =>
  `This option is from ${old}, an older version. ${lead} is in your mods folder.`;

// --- R18.4: THE VERSIONS card ----------------------------------------------

export function memberWorldLine(installed: number, under: string | null): { text: string; under: string | null } {
  if (installed <= 0) return { text: "nothing installed", under: null };
  return {
    text: `${plural(installed, "option")} in your world`,
    under: under ? `, under ${under}` : null,
  };
}

export const NEWEST_BADGE = "newest";

export const newestNotInstalled = (lead: string, old: string): string =>
  `${lead} is in your mods folder but not installed. Open its options in the Rift and install the one you use: it takes ${old}'s place in one step.`;

// --- R18.9: THE CROSSING's replace block ------------------------------------

export const removalsText = (n: number): string => `${n} ${n === 1 ? "file" : "files"} removed or handed back`;

export function stepSentence(replaces: readonly ReplacesView[], newName: string): string {
  const leave =
    replaces.length === 1
      ? `${versionName(replaces[0]?.version ?? null, replaces[0]?.name ?? "")}'s files leave first`
      : "the older versions' files leave first";
  return `In one step: ${leave}, then ${newName}'s are placed. If anything fails, both are put back.`;
}

export const PUT_AWAY_SENTENCE = `After the install, an older version with nothing left installed is put away in ${OLDER_FOLDER}.`;

export const rollbackLine = (newer: string): string =>
  `You are installing an older version: ${newer} leaves your game, and stays in your mods folder.`;

/** R18.30 / Q18.13: the install rollback's empathy names the version it put back. */
export const replacedRollback = (old: string, placed: number | null): string =>
  placed && placed > 0
    ? `Your game folder is exactly as it was: ${old} is installed again and the stone put back the ${placed} ${placed === 1 ? "file" : "files"} already placed.`
    : `Your game folder is exactly as it was: ${old} is installed again.`;

// --- R18.5: the put-away line, R18.6 the replace's toast ---------------------

export function putAwayLine(name: string, fileName: string, landedAs: string): string {
  const taken = landedAs !== fileName ? `, as ${landedAs} (a file of that name was already there)` : "";
  return `${name} is put away in ${OLDER_FOLDER}, beside your mods folder${taken}.`;
}

export const backLine = (name: string): string => `${name} is back in your mods folder.`;

export const putAwayFailedTail = (name: string, after: "install" | "uninstalls"): string =>
  `The ${after === "install" ? "install stands" : "uninstalls stand"}; ${name} stays in your mods folder.`;

export const replacedToast = (pkgName: string, lead: string, old: string): string =>
  `${pkgName} ${lead} is in your world, in ${old}'s place.`;

export const putAwayToast = (old: string, pkgName: string): string =>
  `${old} of ${pkgName} is put away in ${OLDER_FOLDER}. Nothing in your game changed.`;

// --- R18.10: the keep-newest plan --------------------------------------------

export const keepsLine = (keeps: string | null): string =>
  keeps ? `Keeps ${keeps}, the newest version with an install.` : "Keeps the newest version.";

export const keepButton = (keeps: string | null): string => (keeps ? `Keep only ${keeps}` : "Keep only the newest");

export const PUT_AWAY_ROW = `put away in ${OLDER_FOLDER}`;

export const keptSummary = (n: number): string => `${plural(n, "install")} of older versions removed.`;

export const keptToast = (keeps: string, pkgName: string): string =>
  `Only ${keeps} of ${pkgName} is in your world now.`;

// --- R18.7: an older version's answer ------------------------------------------

export const answerMark = (old: string, matched: boolean): string =>
  matched ? ` · in ${old}, an older version` : ` · only in ${old}, an older version`;

// --- R18.8: THE FAR WORLD's sub-row -------------------------------------------

export interface FarSubRow {
  text: string;
  tone: "warn" | "text" | "faint";
  door: "versions" | "rift";
  doorLabel: string;
}

export function farSubRow(older: readonly OlderVersionView[], lead: string, leadInstalled: boolean): FarSubRow | null {
  if (!older.length) return null;
  const names = olderNames(older);
  const verb = isAre(older.length);
  if (older.some((o) => o.installed > 0)) {
    if (leadInstalled) {
      return {
        text: `${names} ${verb} in your mods folder too, still installed under ${lead}.`,
        tone: "warn",
        door: "versions",
        doorLabel: "Show versions in the Rift",
      };
    }
    return {
      text: `${names} ${verb} installed. Install ${lead} from the Rift and it takes ${older.length === 1 ? `${names}'s` : "their"} place.`,
      tone: "text",
      door: "rift",
      doorLabel: "Show in the Rift",
    };
  }
  return {
    text: `${names} ${verb} in your mods folder too, with nothing installed.`,
    tone: "faint",
    door: "versions",
    doorLabel: "Show versions in the Rift",
  };
}

// --- R18.11–R18.14: THE UNDERCROFT -------------------------------------------

export const undercroftCount = (n: number): string =>
  n > 0 ? `${n.toLocaleString("en-GB")} PUT AWAY` : "NOTHING PUT AWAY";

export const UNDERCROFT_BODY = [
  `Older versions the stone put away, in ${OLDER_FOLDER} beside your mods folder.`,
  "Nothing here is read or installed. Bring one back and it joins its file in the Rift again.",
] as const;

export const UNDERCROFT_UNMADE = `The stone hasn't put anything away yet. When an install replaces an older version of a file, the older archive waits here, in a folder named ${OLDER_FOLDER} beside your mods folder, until you bring it back. The stone makes the folder the first time it needs it.`;

export const UNDERCROFT_EMPTY = `Nothing is put away. The folder ${OLDER_FOLDER} beside your mods folder is empty. An archive you move into it by hand shows here too, the next time the stone looks.`;

export const shelfNewer = (newer: boolean): string =>
  newer ? "A newer version is in your mods folder." : "No other version of it is in your mods folder.";

export function broughtBackToast(
  old: string,
  pkgName: string | null,
  lead: string | null,
  landedAs: string | null,
): string {
  const what = pkgName ? `${old} of ${pkgName}` : old;
  if (landedAs) return `${what} is back in your mods folder, as ${landedAs}: a file of that name was already there.`;
  return lead
    ? `${what} is back in your mods folder. It sits under ${lead} in the Rift.`
    : `${what} is back in your mods folder.`;
}
