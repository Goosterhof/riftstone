// THE FRAY's readings: every figure the window draws, computed from the frame
// alone (src/types/fray.ts). One `share()` is the axis for every bar and every
// percentage (§19.13); the skill block is one reading taken on the 4 s beat
// (§19.9); the trust band compares hParty with the game's party ledger and
// nothing else (RF.14). Pure functions only: the components draw, this decides.

import type { FrayFight, FrayMember, FraySkill } from "@/types/fray";

/** The axis is ONE function (§19.13): a part of a whole, 0 while the whole is 0. */
export const share = (part: number, whole: number): number => (whole > 0 ? part / whole : 0);

export const nf = (n: number): string => Math.round(n).toLocaleString("en-GB");

export function pct(x: number): string {
  if (x <= 0) return "0%";
  if (x < 0.01) return "<1%";
  return `${Math.round(x * 100)}%`;
}

const pad2 = (n: number): string => String(n).padStart(2, "0");

export const clock = (s: number): string => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, "0")}`;

export function ageText(s: number): string {
  if (s < 60) return `${s} s`;
  if (s < 3600) return `${Math.floor(s / 60)} min`;
  return `${Math.floor(s / 3600)} h`;
}

/** The fight's per-second figure over its own duration: the stone's own rule for a member's
 *  `dps`, so the party's head and its cards divide alike. A fight of no length has none. */
export const perSecond = (damage: number, durationS: number): number | null =>
  durationS > 0 ? damage / durationS : null;

/** A per-second figure as the window prints it: `—` when there is none. */
export const rateText = (rate: number | null): string => (rate === null ? "—" : nf(rate));

export const isWarm = (m: FrayMember): boolean => m.role !== "hiredPawn";

const ROLE_WORD: Record<FrayMember["role"], string> = {
  arisen: "Arisen",
  mainPawn: "Main pawn",
  hiredPawn: "Hired pawn",
};
const NULL_NAME: Record<FrayMember["role"], string> = {
  arisen: "The Arisen",
  mainPawn: "Your pawn",
  hiredPawn: "A hired pawn",
};
export const roleWord = (m: Pick<FrayMember, "role">): string => ROLE_WORD[m.role];
/** A member the save has not named yet (FQ.9) is named by its place. */
export const nameOf = (m: Pick<FrayMember, "role" | "name">): string => m.name ?? NULL_NAME[m.role];
/** "Arisen · Warrior", or the role alone while the vocation is not read. */
export function roleLine(m: Pick<FrayMember, "role" | "vocation">): string {
  return m.vocation.name === null ? roleWord(m) : `${roleWord(m)} · ${m.vocation.name}`;
}
/** The same for a spoken label: "Arisen, Warrior". */
export const roleLabel = (m: Pick<FrayMember, "role" | "vocation">): string => roleLine(m).replace(" · ", ", ");

/** The foe line: the first kind ×count, then how many other kinds. */
export function foeLine(f: FrayFight): string {
  if (f.foes.length === 0) return "No foe named";
  const [first, ...rest] = f.foes;
  const name = first.name ?? "A foe the stone can't name";
  const count = first.count > 1 ? ` ×${first.count}` : "";
  return `${name}${count}${rest.length ? ` and ${rest.length} more` : ""}`;
}

/** Every hit the vein drew, summed by one function (P2.3). */
export function hitsOf(f: FrayFight): number {
  const members = f.members.reduce((a, m) => a + (m ? m.hits : 0), 0);
  return members + f.unattributed.hits + (f.world?.hits ?? 0);
}

export const approximate = (f: FrayFight): boolean => f.attribution !== "checked";

// ---------- the skill block ----------

/** By damage, a tie to the key that hit first (§19.9). Unknown never leads. */
export function leadingSkills(m: FrayMember, count: number): FraySkill[] {
  return m.skills
    .filter((k) => k.kind !== "unknown")
    .sort((a, b) => b.damage - a.damage || a.firstAt - b.firstAt)
    .slice(0, count);
}

export interface Remainder {
  other: number;
  unknown: number;
}

/** What the leading rows leave: the other keys and the Unknown, as shares of the member. */
export function remainder(m: FrayMember, shown: FraySkill[]): Remainder {
  let other = 0;
  let unknown = 0;
  for (const k of m.skills) {
    if (k.kind === "unknown") unknown += k.damage;
    else if (!shown.some((s) => s.id === k.id)) other += k.damage;
  }
  return { other: share(other, m.damage), unknown: share(unknown, m.damage) };
}

export interface LeadRow {
  kind: FraySkill["kind"];
  id: string;
  label: string;
  s: number;
  hits: number;
  biggest: number;
  lastBlow: boolean;
}

export interface SkillBlock {
  support: { name: string; count: number }[] | null;
  zero: boolean;
  lead: LeadRow[];
  rem: Remainder;
}

/** How a key reads: a named skill by its name, everything else by its id. */
export function keyLabel(k: Pick<FraySkill, "kind" | "id" | "name">): string {
  if (k.kind === "unknown") return "Unknown";
  if (k.kind === "action") return `action ${k.id}`;
  return k.name ?? `skill ${k.id}`;
}
/** Whether a key reads in mono (an id, not a name). */
export const keyIsId = (k: Pick<FraySkill, "kind" | "name">): boolean =>
  k.kind === "action" || (k.kind === "skill" && k.name === null);

/**
 * One member's skill block (P2.8, P3.6): live has 3 rows (support while fewer
 * than two named damaging keys, the leading keys, and the RESERVED remainder),
 * the record 4 (the support row always). No type shrinks.
 */
export function blockOf(m: FrayMember, rows: 3 | 4): SkillBlock {
  const named = m.skills.filter((k) => k.kind !== "unknown" && k.damage > 0).length;
  const evidence = m.noDamageSkills?.length ? m.noDamageSkills : null;
  const support = evidence && !(rows === 3 && named >= 2) ? evidence : null;
  const zero = m.damage === 0;
  const free = rows - 1 - (support ? 1 : 0) - (zero ? 1 : 0);
  const lead = zero ? [] : leadingSkills(m, Math.max(0, free));
  return {
    support,
    zero,
    lead: lead.map((k) => ({
      kind: k.kind,
      id: k.id,
      label: keyLabel(k),
      s: share(k.damage, m.damage),
      hits: k.hits,
      biggest: k.biggest,
      lastBlow: k.lastBlow,
    })),
    rem: remainder(m, lead),
  };
}

/** The remainder row's words: `Other n%` and `Unknown n%`, each only when above 0. */
export function remainderParts(r: Remainder): { word: string; v: string }[] {
  const out: { word: string; v: string }[] = [];
  if (r.other > 0) out.push({ word: "Other", v: pct(r.other) });
  if (r.unknown > 0) out.push({ word: "Unknown", v: pct(r.unknown) });
  return out;
}

/** Every key of a member for S3b: named and id keys by damage, Unknown last. */
export const everyKey = (m: FrayMember): FraySkill[] =>
  [...m.skills].sort((a, b) => Number(a.kind === "unknown") - Number(b.kind === "unknown") || b.damage - a.damage);

// ---------- the beat ----------

/** The 4 s beat's index for a fight second. */
export const beatOf = (durationS: number): number => Math.floor(durationS / 4);

export interface BeatState {
  fightId: string | null;
  lastBeat: number;
  blocks: Record<number, SkillBlock>;
}

export const freshBeat = (): BeatState => ({ fightId: null, lastBeat: -1, blocks: {} });

/** RF.12 × the beat: a member whose block reads zero and has now dealt damage forces a beat. */
function needsWake(f: FrayFight, st: BeatState): boolean {
  return f.members.some((m) => m !== null && m.damage > 0 && st.blocks[m.slot]?.zero === true);
}

/**
 * Take the beat into `st` when its 4 s mark has passed, on a fight's first
 * frame, at the settle (the final heartbeat), or on a member's first damaging
 * hit. The whole block is one reading: every member at once.
 */
export function takeBeat(st: BeatState, f: FrayFight): BeatState {
  const beat = beatOf(f.durationS);
  const fresh = st.fightId !== f.id;
  const force = fresh || f.state === "settled" || needsWake(f, st);
  if (!force && beat === st.lastBeat) return st;
  const blocks: Record<number, SkillBlock> = {};
  for (const m of f.members) if (m) blocks[m.slot] = blockOf(m, 3);
  return { fightId: f.id, lastBeat: beat, blocks };
}

// ---------- damage types (RF.20) ----------

export type Verdict = "additive" | "mix" | "unknown";

/** Fails closed: anything but the contract's two established words reads unknown (FR-D4). */
export function verdictOf(t: string | null | undefined): Verdict {
  if (t === "additive") return "additive";
  if (t === "mix" || t === "proportions") return "mix";
  return "unknown";
}

export interface Saying {
  bold: string | null;
  rest: string;
}

export const MIXSAY: Record<Verdict, Saying> = {
  additive: { bold: "ADDS UP", rest: " · the seven parts sum to each member's damage" },
  mix: { bold: "COMPONENT MIX", rest: " · % of these hits' parts, not of the damage" },
  unknown: { bold: "COMPONENT MIX", rest: " · not known to add up · % of these hits' parts" },
};
export const MIXSAY_SHORT: Record<Verdict, Saying> = {
  additive: { bold: "ADDS UP", rest: " to each member's damage" },
  mix: { bold: null, rest: "% of these hits' parts, not of the damage" },
  unknown: { bold: null, rest: "not known to add up · % of these hits' parts" },
};

export interface TypeCell {
  label: string;
  dash: boolean;
  s: number;
  pct: string;
  damage: string;
}

export const PART_LABELS = ["Slash", "Strike", "Shoot", "Magick", "Enchant", "Element", "Fixed"] as const;

/** The seven cells: additive ÷ the member's damage; a mix ÷ its own parts' sum. Null arrays draw dashes. */
export function typeCells(m: FrayMember, additive: boolean): TypeCell[] {
  const arr = (additive ? m.parts : m.mix) ?? [0, 0, 0, 0, 0, 0, 0];
  const sum = arr.reduce((a, b) => a + b, 0);
  return arr.map((v, i) => {
    const s = additive ? share(v, m.damage) : share(v, sum);
    return { label: PART_LABELS[i], dash: v === 0, s, pct: v === 0 ? "—" : pct(s), damage: nf(v) };
  });
}

// ---------- the trust band (RF.14) ----------

export interface Piece {
  text: string;
  num?: boolean;
  warn?: boolean;
}

function ledgerTail(p: number): string | null {
  if (p < 95) return "the stone missed some hits";
  if (p > 100.5) return "the stone may count some hits twice";
  return null;
}

function ledgerLine(f: FrayFight): { line: Piece[]; tail: string | null } {
  const credits: Piece[] = [{ text: "The stone credits the party " }, { text: nf(f.hParty), num: true }];
  if (f.ledger === null) return { line: [...credits, { text: " · the game's own count didn't answer" }], tail: null };
  if (!(f.ledger > 0)) {
    const tail = f.state === "settled" && f.hParty > 0 ? "the game's count never moved" : null;
    return { line: [...credits, { text: " · the game counts " }, { text: "0", num: true }], tail };
  }
  const p = (f.hParty / f.ledger) * 100;
  const out = ledgerTail(p);
  const line: Piece[] = [
    ...credits,
    { text: " · the game counts " },
    { text: nf(f.ledger), num: true },
    { text: " · " },
  ];
  line.push({ text: `${p.toFixed(1)}%`, num: true, warn: out !== null });
  return { line, tail: out };
}

const gapTail = (n: number): Piece[] => [
  { text: "the feed skipped ", warn: true },
  { text: String(n), num: true, warn: true },
  { text: " records", warn: true },
];
const NOT_COUNTED: Piece[] = [{ text: "Damage only: healing and help aren't counted" }];

/** Line 1 the honesty line; line 2 one slot by priority: the warnings, else what is not counted. */
export function trustLines(f: FrayFight): [Piece[], Piece[]] {
  const { line, tail } = f.partyFormed
    ? ledgerLine(f)
    : {
        line: [
          {
            text: "The stone can't tell the party's hits apart here, so it doesn't compare them with the game's count",
          },
        ],
        tail: null,
      };
  const tails: Piece[][] = tail ? [[{ text: tail, warn: true }]] : [];
  if (f.gaps > 0) tails.push(gapTail(f.gaps));
  if (!tails.length) return [line, NOT_COUNTED];
  const joined: Piece[] = [];
  for (const [i, t] of tails.entries()) {
    if (i) joined.push({ text: " · ", warn: true });
    joined.push(...t);
  }
  return [line, joined];
}

// ---------- the shelf (THE STRATA, RF.26) ----------

const tagDur = (s: number): string =>
  s >= 3600 ? `${Math.floor(s / 3600)}h${pad2(Math.floor((s % 3600) / 60))}` : clock(s);
export const tagText = (f: FrayFight): string => (f.n > 999 ? `FIGHT ${f.n}` : `FIGHT ${f.n} · ${tagDur(f.durationS)}`);

/** The shelf: the settled stele fight with the recent ones, newest first, five at most. */
export function shelfOf(fight: FrayFight | null, recent: FrayFight[]): FrayFight[] {
  const all =
    fight?.state === "settled"
      ? [fight, ...recent.filter((x) => x.id !== fight.id)]
      : recent.filter((x) => x.state === "settled");
  return [...all].sort((a, b) => b.n - a.n).slice(0, 5);
}

/** Minutes since a settled fight's local "HH:MM" (today, else yesterday); null when unreadable. */
export function minutesSince(endedAt: string | null, now: Date): number | null {
  const m = endedAt ? /^(\d{1,2}):(\d{2})$/.exec(endedAt) : null;
  if (!m) return null;
  const then = Number(m[1]) * 60 + Number(m[2]);
  const at = now.getHours() * 60 + now.getMinutes();
  return (at - then + 1440) % 1440;
}

/** Minutes since a fight settled: from its epoch stamp, else from its "HH:MM" (today, else yesterday). */
export function minutesAgoOf(f: Pick<FrayFight, "endedAt" | "endedAtMs"> | null, now: Date): number | null {
  if (!f) return null;
  if (f.endedAtMs !== null) return Math.max(0, Math.floor((now.getTime() - f.endedAtMs) / 60000));
  return minutesSince(f.endedAt, now);
}

/** Seconds since this run of the game started, or null when the feed doesn't know. */
export function startedAgoS(gameStartedMs: number | null, now: Date): number | null {
  return gameStartedMs === null ? null : Math.max(0, Math.floor((now.getTime() - gameStartedMs) / 1000));
}

export function agoWords(min: number): string {
  return min < 60 ? `${min} min ago` : `${Math.floor(min / 60)} h ago`;
}

/** The settle's spoken line (aria-live). */
export function settledWords(f: FrayFight): string {
  const mins = Math.floor(f.durationS / 60);
  const lead = mins ? `${mins} minute${mins > 1 ? "s" : ""} ` : "";
  const dur = `${lead}${Math.floor(f.durationS % 60)}`;
  const rate = perSecond(f.hParty, f.durationS);
  const per = rate === null ? "" : `, ${nf(rate)} per second`;
  return `Fight settled: ${dur}, the party dealt ${nf(f.hParty)}${per}.`;
}

/** The wall clock `ago` seconds before `now`, "21:03:52". */
export function clockAgo(agoS: number, now: Date): string {
  const d = new Date(now.getTime() - agoS * 1000);
  return `${pad2(d.getHours())}:${pad2(d.getMinutes())}:${pad2(d.getSeconds())}`;
}
