// THE MUSTER's words and its small pure rules (#00015 §17). Components never
// compose this copy; the failures of a slot are voiced in ./voice.ts (kind
// `save`, context `muster`). One narrator, the stone, in the third person.
//
// Privacy (#00015 §17 Privacy): a name the save gives is shown and never
// logged, never in a `title`, a toast, or a live-region sentence. Nothing in
// this module puts one anywhere but the text a caller renders.

import type { MusterCreator, MusterGains, MusterGearSlot, MusterMember, MusterRole, Named } from "@/types/rift";
import { formatClock, formatDate } from "./format";

export const MUSTER_INSCRIPTION = "THE MUSTER";

/** R17.21: the plain word for each kind of named thing. */
export type NamedKind = "item" | "vocation" | "augment" | "skill" | "inclination" | "specialization";

/** A Named's text: the game's own name, else its kind and its number. */
export function namedText(named: Named, kind: NamedKind): string {
  return named.name ?? `${kind} ${named.id}`;
}

export const UNNAMED_NOTE = "· unnamed in the game's text";
export const UNNAMED_TITLE =
  "The game's text has no name for this one. The stone shows its number so you can still tell it apart.";

/** R17.8 / R17.9: the roles in plain words. */
export const ROLE_WORD: Record<MusterRole, string> = {
  arisen: "Arisen",
  mainPawn: "Main pawn",
  hiredPawn: "Hired pawn",
};

/** R17.21: a person's name the save did not give reads as the role in its place. */
export const NAMELESS_PERSON: Record<MusterRole, string> = {
  arisen: "The Arisen",
  mainPawn: "Your pawn",
  hiredPawn: "A hired pawn",
};

export const personName = (member: Pick<MusterMember, "name" | "role">): string =>
  member.name ?? NAMELESS_PERSON[member.role];

/** R17.11: the eight plain labels; the two rings are never numbered. */
export const GEAR_LABEL: Record<MusterGearSlot, string> = {
  mainWeapon: "Main weapon",
  subWeapon: "Sub weapon",
  head: "Head",
  body: "Body",
  legs: "Legs",
  cloak: "Cloak",
  ring1: "Ring",
  ring2: "Ring",
};

export type GainKey = keyof MusterGains;

/** R17.12: the contract's order, fixed. */
export const GAIN_ROWS: readonly { key: GainKey; label: string }[] = [
  { key: "health", label: "Health" },
  { key: "stamina", label: "Stamina" },
  { key: "strength", label: "Strength" },
  { key: "defence", label: "Defence" },
  { key: "magick", label: "Magick" },
  { key: "magickDefence", label: "Magick Defence" },
  { key: "weight", label: "Weight" },
  { key: "knockdown", label: "Knockdown" },
];

export const GAINS_SCOPE = "What the levels gave. Gear adds its own on top.";
export const MOST_IN_PARTY = "most in the party";

/** R17.12's party mark, one formula for the window and its spec: this
 *  member's value over the party's highest of that stat, 0 when that is 0. */
export function partyMark(value: number, max: number): number {
  return max > 0 ? Math.max(0, Math.min(1, value / max)) : 0;
}

export const count = (n: number): string => n.toLocaleString("en-GB");

/** R17.12's accessible row: the number, then the party mark in words. */
export function gainWords(label: string, value: number, ratio: number | null, most: boolean): string {
  const base = `${label}, plus ${count(value)} from levels`;
  if (ratio === null) return base;
  if (most) return `${base}, the most in the party`;
  return `${base}, ${Math.round(ratio * 100)} percent of the party's most`;
}

const dayOf = (d: Date): number => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();

/** R17.7: today 21:04 · yesterday 21:04 · 16 Oct 2026, 20:12. */
export function whenWords(iso: string, now: number = Date.now()): string {
  const at = new Date(iso);
  if (Number.isNaN(at.getTime())) return iso;
  const clock = formatClock(at.getTime());
  const today = dayOf(new Date(now));
  const then = dayOf(at);
  if (then === today) return `today ${clock}`;
  const yesterday = new Date(now);
  yesterday.setDate(yesterday.getDate() - 1);
  if (then === dayOf(yesterday)) return `yesterday ${clock}`;
  return `${formatDate(iso)}, ${clock}`;
}

/** R17.17: where a hired pawn came from, described by what the contract gives. */
export function creatorLead(): string {
  return "From another Arisen's world";
}

export function creatorTail(creator: MusterCreator | null): { level: string; vocation: Named } | null {
  return creator ? { level: `Lv ${creator.level}`, vocation: creator.vocation } : null;
}

/** R17.8: a tab's accessible name. The person's name sits in the tab's own
 *  text already; this is that text, said in order. */
export function tabName(member: MusterMember): string {
  return `${ROLE_WORD[member.role]}, ${personName(member)}, level ${member.level} ${namedText(member.vocation, "vocation")}`;
}

/** R17.23 page-level states, R17.22's copy. */
export const MUSTER_FOLDER_STATES = {
  noSteam: {
    inscription: "NO PARTY ANSWERS",
    body: "The stone found no Steam folder on this PC, and Dragon's Dogma 2 keeps its saves under Steam. The stone only reads saves; nothing on this PC was changed. If Steam is installed somewhere unusual, start it once, then look again.",
  },
  noSaves: {
    inscription: "THE PARTY HAS NOT GATHERED",
    body: "Steam is here, but no account on this PC has a Dragon's Dogma 2 save yet. Play until the game saves once, and the stone reads your party from it.",
  },
} as const;

export const LOOK_AGAIN = "Look again";
export const LISTING_SLOTS = "Reading your saves…";
export const READING_PARTY = {
  inscription: "READING THE PARTY",
  body: "The first read after a game update takes a moment: the stone is learning the game's names. Later reads are quick.",
} as const;

/** R17.20: the names did not answer. */
export const NAMES_NOTICE = {
  inscription: "THE NAMES DID NOT ANSWER",
  line: "The party below is whole. Until the stone can read the game's own text, each thing it names shows its number.",
  door: "Open Preflight",
} as const;

/** R17.5 line 2 for a slot with a problem, and the colour that says it. */
export const SLOT_PROBLEM_WORDS = {
  otherAccount: { words: "another account's save", tone: "muted" },
  beingWritten: { words: "the game is writing it", tone: "warn" },
  damaged: { words: "won't open", tone: "ember" },
  noParty: { words: "holds no party", tone: "muted" },
  gone: { words: "gone from the folder", tone: "faint" },
} as const;

/** R17.25 step 5 + R17.22: what the polite live region says once. Counts only. */
export function recarvedSentence(n: number): string {
  if (n === 0) return "The game saved. The stone read the party again. Nothing changed.";
  return `The game saved. The stone read the party again: ${n} ${n === 1 ? "thing" : "things"} changed.`;
}

export const readAgainWords = (ms: number): string => `· read again ${formatClock(ms)}`;
