// THE RE-CARVING's diff (#00015 R17.25 step 2), a pure function: the party a
// slot showed, the party it shows now, and every value that changed between.
// Members are matched by `memberKey` (the contract's stable per-pawn key); a
// key the previous read did not hold JOINED: it gets no marks, only `new`.

import type { MusterGains, MusterMember, MusterView, Named } from "@/types/rift";
import { GAIN_ROWS, namedText, type NamedKind } from "./muster";

/** A number that rose or fell, or a thing that was swapped (was: its old name). */
export type Mark = { kind: "rose" | "fell"; delta: number } | { kind: "swapped"; was: string };

export interface MemberMarks {
  level?: Mark;
  experience?: Mark;
  vocation?: Mark;
  /** By vocation id. */
  vocations: Map<number, Mark>;
  gains: Partial<Record<keyof MusterGains, Mark>>;
  /** By gear index (0–7). */
  gear: Map<number, Mark>;
  augments?: Mark;
  skills?: Mark;
  inclination?: Mark;
  specialization?: Mark;
}

export interface Recarving {
  /** By memberKey. */
  members: Map<string, MemberMarks>;
  /** memberKeys that were not in the previous read. */
  joined: Set<string>;
  gold?: Mark;
  riftCrystals?: Mark;
  /** Every marked value, the purse included. */
  count: number;
}

export const NO_MARKS: Recarving = { members: new Map(), joined: new Set(), count: 0 };

function numberMark(was: number, now: number): Mark | undefined {
  if (was === now) return undefined;
  return now > was ? { kind: "rose", delta: now - was } : { kind: "fell", delta: was - now };
}

const thingWas = (named: Named | null, kind: NamedKind): string => (named ? namedText(named, kind) : "empty");

function namedMark(was: Named | null, now: Named | null, kind: NamedKind): Mark | undefined {
  if ((was?.id ?? null) === (now?.id ?? null)) return undefined;
  return { kind: "swapped", was: thingWas(was, kind) };
}

function listMark(was: Named[], now: Named[], kind: NamedKind): Mark | undefined {
  const same = was.length === now.length && was.every((n, i) => n.id === now[i]?.id);
  if (same) return undefined;
  return { kind: "swapped", was: was.length ? was.map((n) => namedText(n, kind)).join(", ") : "empty" };
}

function vocationMarks(was: MusterMember, now: MusterMember): Map<number, Mark> {
  const before = new Map(was.vocations.map((v) => [v.vocation.id, v.experience]));
  const marks = new Map<number, Mark>();
  for (const v of now.vocations) {
    const mark = numberMark(before.get(v.vocation.id) ?? 0, v.experience);
    if (mark) marks.set(v.vocation.id, mark);
  }
  return marks;
}

function gainMarks(was: MusterMember, now: MusterMember): MemberMarks["gains"] {
  const marks: MemberMarks["gains"] = {};
  for (const { key } of GAIN_ROWS) {
    const mark = numberMark(was.gains[key], now.gains[key]);
    if (mark) marks[key] = mark;
  }
  return marks;
}

function gearMarks(was: MusterMember, now: MusterMember): Map<number, Mark> {
  const marks = new Map<number, Mark>();
  for (const [i, g] of now.gear.entries()) {
    const mark = namedMark(was.gear.at(i)?.item ?? null, g.item, "item");
    if (mark) marks.set(i, mark);
  }
  return marks;
}

const trait = (m: MusterMember, which: "inclination" | "specialization"): Named | null => m.pawn?.[which] ?? null;

function memberMarks(was: MusterMember, now: MusterMember): MemberMarks {
  return {
    level: numberMark(was.level, now.level),
    experience: numberMark(was.experience, now.experience),
    vocation: namedMark(was.vocation, now.vocation, "vocation"),
    vocations: vocationMarks(was, now),
    gains: gainMarks(was, now),
    gear: gearMarks(was, now),
    augments: listMark(was.augments, now.augments, "augment"),
    skills: listMark(was.skills, now.skills, "skill"),
    inclination: namedMark(trait(was, "inclination"), trait(now, "inclination"), "inclination"),
    specialization: namedMark(trait(was, "specialization"), trait(now, "specialization"), "specialization"),
  };
}

export function marksIn(m: MemberMarks): number {
  const singles = [m.level, m.experience, m.vocation, m.augments, m.skills, m.inclination, m.specialization];
  return (
    singles.filter(Boolean).length + m.vocations.size + Object.values(m.gains).filter(Boolean).length + m.gear.size
  );
}

export const membersOf = (view: MusterView): MusterMember[] =>
  [view.arisen, view.mainPawn, ...view.hired].filter((m): m is MusterMember => m !== null);

/** What changed between two reads of the same slot. */
export function recarve(was: MusterView, now: MusterView): Recarving {
  const before = new Map(membersOf(was).map((m) => [m.memberKey, m]));
  const result: Recarving = { members: new Map(), joined: new Set(), count: 0 };
  for (const member of membersOf(now)) {
    const prior = before.get(member.memberKey);
    if (!prior) {
      result.joined.add(member.memberKey);
      continue;
    }
    const marks = memberMarks(prior, member);
    const n = marksIn(marks);
    if (n) result.members.set(member.memberKey, marks);
    result.count += n;
  }
  if (was.gold !== null && now.gold !== null) result.gold = numberMark(was.gold, now.gold);
  if (was.riftCrystals !== null && now.riftCrystals !== null)
    result.riftCrystals = numberMark(was.riftCrystals, now.riftCrystals);
  result.count += [result.gold, result.riftCrystals].filter(Boolean).length;
  return result;
}

/** R17.25 step 4: a number's rest mark (`▴22`, `▾3`), or null for a swap. */
export function deltaText(mark: Mark | undefined): string | null {
  if (!mark || mark.kind === "swapped") return null;
  return `${mark.kind === "rose" ? "▴" : "▾"}${mark.delta.toLocaleString("en-GB")}`;
}

/** R17.25 step 4: a swapped thing's accessible description (never a title). */
export function wasWords(mark: Mark | undefined): string | null {
  if (mark?.kind !== "swapped") return null;
  return mark.was === "empty" ? "changed at the last save, was empty" : `changed at the last save, was ${mark.was}`;
}
