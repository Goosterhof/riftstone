// v0.5 — versions of one file (#00015 §18, WINDOW-CONTRACT § v0.5): the one
// fold every surface reads, and every sentence the versions say. Components
// never compose this copy (R18.30); the failures of a move are voiced in
// ./voice.ts (context `olderVersions`). One narrator, the stone, in the third
// person. A folder is named `mods (older versions)`, never a path.
//
// The spine decides which package leads (`family.lead`) and which option of one
// version is the same option as which of another (`OlderVersionView.matches`):
// this module only groups by the one and reads the other, never works either out.

import type { LibraryPackage, LinkSource, OlderVersionView, ReplacesView, VersionQuestion } from "@/types/rift";
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
  /** `+1 older` / `+1 older, installed`; `+2 older, 1 installed` when only some are. */
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
  // count the installed ones: two older versions with only v1.2 installed are
  // never `+2 older, installed` (General, PR #2)
  const installed = older.filter((o) => o.installed > 0).length;
  const warn = installed > 0;
  const versions = n === 1 ? "1 older version" : `${n} older versions`;
  const some = installed > 0 && installed < n;
  let text = `+${n} older`;
  let hidden = `${versions} of this file in your mods folder`;
  if (some) {
    text = `+${n} older, ${installed} installed`;
    hidden = `${versions} of this file, ${installed} still installed`;
  } else if (warn) {
    text = `+${n} older, installed`;
    hidden = `${versions} of this file, still installed`;
  }
  return { text, short: `+${n}`, warn, hidden };
}

/** R18.2: a version row's label; R18.18.3: a joined member says whose word it is. */
export const versionRowLabel = (version: string | null, joined = false): string =>
  `${version ? `${versionLabel(version)} · older version` : "older version"}${joined ? ` · ${BY_YOUR_WORD}` : ""}`;

export const versionRowState = (installed: number): string =>
  installed > 0 ? `${installed} in your world` : "nothing installed";

/** R18.3: the lead leaf's chip suffix when its option is in your world only from
 *  an older version (a match whose `leadOptionId` is the leaf's is installed). */
/** R18.3 / R18.18.7: the chip's version: `v1.2`, or `older` for a version
 *  Nexus never named (never a file name in a chip). */
export const chipVersion = (version: string | null): string => (version ? versionLabel(version) : "older");

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

// --- R18.18: the player's word (a question Nexus can't answer) ---------------
//
// The spine sets `versionQuestion` and decides which file it asks about; the
// window reads `ofPackageId`, `sharedFiles` and `files` as given and never
// counts a file name or picks a lead itself (Pattern 013).

export const BY_YOUR_WORD = "by your word";
export const ASKING_HEAD = "VERSIONS OF ONE FILE?";

/** A link source as THE FAR WORLD's row prints it (R18.18.3): never the raw `joined`. */
export const linkSourceText = (source: LinkSource): string => {
  if (source === "md5") return "by MD5";
  if (source === "joined") return BY_YOUR_WORD;
  return source;
};

/** What the questioned package is called in the question: a folder or an archive. */
export type AskedKind = "folder" | "archive";
export const askedKind = (pkg: Pick<LibraryPackage, "package">): AskedKind =>
  pkg.package.format === "folder" ? "folder" : "archive";

/** The file the question is about: `v1.3`, else its file name (drawn in mono). */
export interface AskedOf {
  text: string;
  mono: boolean;
}
export const askedOf = (q: Pick<VersionQuestion, "ofVersion" | "ofFileName">): AskedOf =>
  q.ofVersion ? { text: versionLabel(q.ofVersion), mono: false } : { text: q.ofFileName, mono: true };

/** R18.18.1: the tree's mark after the counts, its folded form and its hidden words. */
export interface AskMark {
  text: string;
  short: string;
  hidden: string;
}
export function askMark(q: VersionQuestion): AskMark {
  const of = askedOf(q).text;
  return {
    text: `a version of ${of}?`,
    short: "?",
    hidden: `The stone asks whether this is a version of the same file as ${of}`,
  };
}

/** R18.18.1: the question row, the entry's first child. */
export const askRowLabel = (q: VersionQuestion): string => `Is this a version of ${askedOf(q).text}?`;

/** R18.18.1: the World pane's line under an option of the questioned package. */
export const askDetailLine = (q: VersionQuestion): string =>
  `The stone thinks this may be a version of the same file as ${askedOf(q).text}.`;
export const ASK_DETAIL_DOOR = "Show both versions…";

/** R18.18.2: the evidence, read only from the spine's two counts. `of` is the
 *  file's name in the sentence (`v1.3`, or `it` in THE FAR WORLD's row). */
export function evidenceLine(q: Pick<VersionQuestion, "sharedFiles" | "files">, of: string): string {
  if (q.files === 1) return `Its one file name is in ${of} too.`;
  if (q.sharedFiles === q.files) return `All ${q.files} of its file names are in ${of} too.`;
  return `${q.sharedFiles} of its ${q.files} file names are in ${of} too.`;
}
export const NEXUS_CANT_TELL = "Nexus can't tell: its link names the mod, not the file. Your word can.";

/** R18.18.1: THE FAR WORLD's sub-row under the questioned package's own row. */
export function farAskLine(q: VersionQuestion): string {
  const evidence = evidenceLine(q, "it");
  return `The stone thinks this may be a version of the same file as ${askedOf(q).text}: ${evidence.charAt(0).toLowerCase()}${evidence.slice(1)}`;
}
export const FAR_ASK_DOOR = "Show both versions in the Rift";

/** R18.18.2: the question, around the file's name (drawn in mono when it is a file name). */
export const askedQuestion = (kind: AskedKind): { before: string; after: string } => ({
  before: `Is this ${kind} a version of the same file as `,
  after: "?",
});
/** R18.18.6: the question before the player picks among two or more files. */
export const askedQuestionOpen = (kind: AskedKind): string => `Is this ${kind} a version of one of these files?`;
/** R18.18.6: the radio list's name. */
export const PICK_LEGEND = "Which file is it a version of?";
/** R18.18.6: one file in the radio list: `v1.3 · Lantern_Glow_Recolor-….zip`. */
export const candidateLabel = (c: JoinCandidate): string =>
  c.version ? `${versionLabel(c.version)} · ${c.fileName}` : c.fileName;
/** R18.18.2: the questioned package's row label: never a version. */
export const askedRowLabel = (kind: AskedKind): string => `this ${kind}`;

export const JOIN_DOOR = "Yes, join them";
export const KEEP_APART_DOOR = "No, keep them apart";
export const JOINING_HELD = "The stone is joining them.";
export const DECLINING_HELD = "The stone is remembering your no.";
export const PICK_HELD = "Pick the file this is a version of first.";
export const TAKE_IT_BACK = "Take it back";

/** R18.18.3: the one line above the rows once the join is drawn (and the live region's). */
export const JOINED_LINE = "Joined by your word. The Rift shows one entry for this file now.";

/** R18.18.3: `Take it back` landed. */
export const partedToast = (fileName: string): string =>
  `${fileName} is its own entry in the Rift again. The stone won't ask about it again.`;

/** R18.18.4: the no landed. */
export const keptApartToast = (kind: AskedKind): string => `Kept apart. The stone won't ask about this ${kind} again.`;

// --- R18.18.6: join without the question, one door -----------------------------

export const MANUAL_LINE = "Nexus links this to its mod but not to one of its files.";
export const MANUAL_DOOR = "This is a version of…";
export const MANUAL_NOTE =
  "The stone isn't asking: their file names don't say so. Join them only if you know they are.";

/** One file the manual join may name: its group's newest package, read from the spine's `family.lead`. */
export interface JoinCandidate {
  packageId: string;
  version: string | null;
  fileName: string;
}

/** R18.18.6's predicate, on the contract's own fields only: the package's link
 *  names a mod and no file group, it is in no family and asked nothing, and the
 *  library holds a package of the same mod with a file group. Each such group
 *  is offered once, by its lead (the spine's `family.lead`, else the package). */
/** The modId of a package the manual door may offer for: linked to a mod, to no file group, in no family, asked nothing. */
function unpinnedMod(pkg: LibraryPackage): number | null {
  const link = pkg.package.nexus;
  if (link?.groupId !== null || pkg.family !== null || pkg.versionQuestion !== null) return null;
  return link.modId;
}

/** Another package pinned to a file group of the same mod. */
const pinnedOf = (other: LibraryPackage, modId: number): boolean =>
  other.package.nexus?.modId === modId && other.package.nexus.groupId !== null;

export function joinCandidates(pkg: LibraryPackage, packages: readonly LibraryPackage[]): JoinCandidate[] {
  const modId = unpinnedMod(pkg);
  if (modId === null) return [];
  const byId = new Map(packages.map((p) => [p.package.id, p]));
  const leads = new Map<string, LibraryPackage>();
  for (const other of packages) {
    if (other === pkg || !pinnedOf(other, modId)) continue;
    const lead = byId.get(other.family?.lead ?? other.package.id) ?? other;
    leads.set(lead.package.id, lead);
  }
  return [...leads.values()].map((lead) => ({
    packageId: lead.package.id,
    version: lead.package.nexus?.version ?? null,
    fileName: lead.package.fileName,
  }));
}
