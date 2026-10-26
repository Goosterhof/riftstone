// THE FRAY's mock feed: the audition's invented party, FIGHT 7 and its shelf
// (#00015 §19.14), emitting the window's own contract, `FrayFrame`
// (src/types/fray.ts). Every name here is INVENTED: people, pawns, foes and
// skills. The window computes everything it draws from the frame alone; the
// only thing a scene adds beside the frame is where the view starts (a card
// opened, Damage types pressed, a shelf fight browsed), because that is the
// window's own state, never the feed's.
//
// ?mock=<scene> on fray.html (outside Tauri the window always runs on this):
//   the fight:   fraysettle (the last six seconds of FIGHT 7, then THE SETTLING)
//                | fray | fraypaused | fraydown | frayworld | frayworldledger
//                | frayroster | frayleft | frayunknown | fraylong
//                | fraysupporttwo | fraysupportunknown | frayhealer
//                | frayhealerquiet | fraythree | fraypartyunknown
//                | fraypartyfour | fraynext | frayleftnext
//   the record:  fraysettled | frayworldsettled | frayidleafter | fraymember
//                | fraytypes | frayproportions | fraytypesunknown
//                | frayverdictmix | frayverdictmixopen | fraytypesapprox
//                | fraysupporttwosettled | frayshelf | frayshelfsix
//                | frayshelfmix | fraywidetag
//   trust:       frayunqualified | frayinconclusive | frayshort | frayover
//                | fraynoledger | frayunmoved | fraygap | fraygapsettled
//   feed & eye:  fraystale | fraystaleapprox | frayidle | frayasleep
//                | frayasleepfresh | fraynorefw | fraynoeye | frayshut
//                | fraysilent
// An unknown or missing scene is `fray`. The audition's verdict scenes with a
// word the contract cannot carry (none, 'partial') live in tests/fray.spec.ts
// as calls to the window's verdict lookup instead: a typed frame cannot hold
// them, and 'proportions' arrives here as the contract's own word, `mix`.

import type {
  FrayFeed,
  FrayFight,
  FrayFrame,
  FrayKill,
  FrayMember,
  FrayParts,
  FrayRole,
  FraySkill,
  FraySkillKind,
  FrayTypes,
} from "@/types/fray";

export const FRAY_SCENES = [
  "fraysettle",
  "fray",
  "fraypaused",
  "fraydown",
  "frayworld",
  "frayworldledger",
  "frayroster",
  "frayleft",
  "frayunknown",
  "fraylong",
  "fraysupporttwo",
  "fraysupportunknown",
  "frayhealer",
  "frayhealerquiet",
  "fraythree",
  "fraypartyunknown",
  "fraypartyfour",
  "fraynext",
  "frayleftnext",
  "fraysettled",
  "frayworldsettled",
  "frayidleafter",
  "fraymember",
  "fraytypes",
  "frayproportions",
  "fraytypesunknown",
  "frayverdictmix",
  "frayverdictmixopen",
  "fraytypesapprox",
  "fraysupporttwosettled",
  "frayshelf",
  "frayshelfsix",
  "frayshelfmix",
  "fraywidetag",
  "frayunqualified",
  "frayinconclusive",
  "frayshort",
  "frayover",
  "fraynoledger",
  "frayunmoved",
  "fraygap",
  "fraygapsettled",
  "fraystale",
  "fraystaleapprox",
  "frayidle",
  "frayasleep",
  "frayasleepfresh",
  "fraynorefw",
  "fraynoeye",
  "frayshut",
  "fraysilent",
] as const;
export type FrayScene = (typeof FRAY_SCENES)[number];

export const isFrayScene = (name: string): name is FrayScene => (FRAY_SCENES as readonly string[]).includes(name);

/** Where the window's own view starts in a scene (never part of the frame). */
export interface FrayViewHint {
  sub?: "skills" | "types";
  open?: number;
  /** A shelf fight's number to browse. */
  browse?: number;
}

// ---------- the mock party and FIGHT 7 (§19.14) ----------

type SkillRow = [FraySkillKind, string, number, number, number];
type Casts = [string, number[]];

interface PartySeed {
  role: FrayRole;
  name: string;
  voc: string;
  vocId: number;
  skills: SkillRow[];
  casts?: Casts[];
  parts: FrayParts;
  mix: FrayParts;
}

const VOCATIONS: Record<string, number> = { Fighter: 1, Archer: 2, Mage: 3, Thief: 4, Warrior: 5, Sorcerer: 6 };

const PARTY7: PartySeed[] = [
  {
    role: "arisen",
    name: "Odalric",
    voc: "Warrior",
    vocId: 5,
    skills: [
      ["skill", "Riven Arc", 6390, 14, 812],
      ["skill", "Cinder Rush", 4670, 9, 1104],
      ["action", "3A9F", 2800, 31, 140],
      ["skill", "Bulwark Turn", 1090, 2, 600],
      ["unknown", "Unknown", 630, 5, 190],
    ],
    parts: [8100, 3430, 0, 0, 1400, 1710, 940],
    mix: [8100, 3430, 0, 0, 1400, 4510, 940],
  },
  {
    role: "mainPawn",
    name: "Wren",
    voc: "Thief",
    vocId: 4,
    skills: [
      ["skill", "Quick Cut", 4420, 22, 380],
      ["skill", "Shadow Feint", 2950, 12, 410],
      ["skill", "Hook and Run", 1980, 9, 330],
      ["unknown", "Unknown", 490, 4, 160],
    ],
    parts: [6400, 1900, 0, 0, 0, 980, 560],
    mix: [6400, 1900, 0, 0, 0, 2480, 560],
  },
  {
    role: "hiredPawn",
    name: "Bramble",
    voc: "Sorcerer",
    vocId: 6,
    skills: [
      ["skill", "Ember Lance", 7380, 12, 960],
      ["skill", "Ashfall", 4310, 8, 720],
      ["skill", "Kindle Ward", 610, 3, 260],
    ],
    parts: [0, 0, 0, 7900, 0, 4100, 300],
    mix: [0, 0, 0, 7900, 0, 7100, 300],
  },
  {
    role: "hiredPawn",
    name: "Sable",
    voc: "Mage",
    vocId: 3,
    skills: [["skill", "Frost Thread", 820, 6, 190]],
    casts: [["Mending Light", [9, 22, 37, 50, 66, 79, 93]]],
    parts: [0, 0, 0, 500, 0, 320, 0],
    mix: [0, 0, 0, 500, 0, 720, 0],
  },
];
const UNAT7: [number, number, number] = [2440, 11, 610];
export const DUR7 = 102;
const LEDGER7 = 38770;
const KILLS7: [number, string, number][] = [
  [0, "Riven Arc", 38.4],
  [1, "Quick Cut", 71.2],
  [2, "Ember Lance", 101.6],
];
const LIVE_T = 96;

/** A skill's stable key: its custom id (invented), an action hash, or "unknown". */
function keyId(kind: FraySkillKind, key: string): string {
  if (kind === "action") return key;
  if (kind === "unknown") return "unknown";
  return `sk.${key.toLowerCase().replace(/[^a-z]+/g, "-")}`;
}

function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

interface Hit {
  slot: number | null;
  kind: FraySkillKind;
  key: string;
  dmg: number;
  t: number;
  kill?: boolean;
}

/** The scripted timeline: every hit of FIGHT 7, seeded, summing to §19.14's table. */
function buildHits(): Hit[] {
  const r = rng(7);
  const hits: Hit[] = [];
  const split = (D: number, H: number, B: number): number[] => {
    if (H === 1) return [D];
    const w = Array.from({ length: H - 1 }, () => 0.6 + 0.8 * r());
    const ws = w.reduce((a, b) => a + b, 0);
    const v = w.map((x) => Math.min(B - 1, Math.round((x * (D - B)) / ws)));
    let res = D - B - v.reduce((a, b) => a + b, 0);
    let i = 0;
    while (res !== 0 && i < 100000) {
      const j = i % v.length;
      const step = res > 0 ? 1 : -1;
      if (v[j] + step < B && v[j] + step > 0) {
        v[j] += step;
        res -= step;
      }
      i++;
    }
    return [B, ...v];
  };
  for (const [slot, m] of PARTY7.entries()) {
    for (const [kind, key, D, H, B] of m.skills) {
      for (const dmg of split(D, H, B)) hits.push({ slot, kind, key, dmg, t: 0.6 + r() * 100.6 });
    }
  }
  for (const dmg of split(UNAT7[0], UNAT7[1], UNAT7[2])) {
    hits.push({ slot: null, kind: "unknown", key: "Unknown", dmg, t: 0.6 + r() * 100.6 });
  }
  for (const [slot, key, t] of KILLS7) {
    const h = hits.find((x) => x.slot === slot && x.key === key && !x.kill && x.dmg < 1104);
    if (h) {
      h.t = t;
      h.kill = true;
    }
  }
  return hits.sort((a, b) => a.t - b.t);
}
const HITS7 = buildHits();

// ---------- the internal fight (the audition's shape), then the contract ----------

interface Key {
  kind: FraySkillKind;
  key: string;
  damage: number;
  hits: number;
  biggest: number;
  lastBlow: boolean;
  first: number;
}

interface M {
  slot: number;
  role: FrayRole;
  name: string | null;
  voc: string;
  vocId: number;
  damage: number;
  hits: number;
  keys: Key[];
  hp: number | null;
  st: number | null;
  down: boolean;
  left: boolean;
  noDmg: [string, number][];
  baseParts: FrayParts;
  baseMix: FrayParts;
}

interface F {
  n: number;
  foes: { name: string; count: number }[];
  t: number;
  members: (M | null)[];
  unat: number;
  unatHits: number;
  unatBiggest: number;
  world: number;
  worldHits: number;
  kills: FrayKill[];
  settled: boolean;
  agoS: number;
  attribution: FrayFight["attribution"];
  types: FrayTypes;
  skills: FrayFight["skills"];
  gaps: number;
  paused: boolean;
  partyFormed: boolean;
  ledger: number | null;
}

function hpAt(slot: number, t: number): number {
  const base = [0.92, 0.8, 0.86, 0.95][slot];
  return Math.max(
    0.05,
    Math.min(1, base - 0.28 * Math.max(0, Math.sin((t + slot * 17) / 13)) + 0.05 * Math.cos(t / 3)),
  );
}
function stAt(slot: number, t: number): number {
  return Math.max(0.1, Math.min(1, 0.7 + 0.3 * Math.sin((t + slot * 9) / 5)));
}

/** RF.20 / P3c.2: parts re-scaled to the damage they belong to; the rounding remainder on the largest part. */
function partsFor(base: FrayParts, mix: FrayParts, dmg: number): { parts: FrayParts; mix: FrayParts } {
  const tot = base.reduce((a, b) => a + b, 0);
  const zero: FrayParts = [0, 0, 0, 0, 0, 0, 0];
  if (!(dmg > 0) || !(tot > 0)) return { parts: zero, mix: [...zero] };
  const k = dmg / tot;
  const parts = base.map((v) => Math.round(v * k)) as FrayParts;
  let big = 0;
  for (const [i, v] of parts.entries()) if (v > parts[big]) big = i;
  parts[big] += dmg - parts.reduce((a, b) => a + b, 0);
  return { parts, mix: mix.map((v) => Math.round(v * k)) as FrayParts };
}

const sumKeys = (m: M): number => m.keys.reduce((a, x) => a + x.damage, 0);
const ledgerSeed = (party: number): number => Math.round(party * (LEDGER7 / 38540));
const partyOf = (f: F): number => f.members.reduce((a, m) => a + (m ? m.damage : 0), 0);

interface Left {
  slot: number;
  t: number;
}

function seedMember(seed: PartySeed, slot: number, t: number): M {
  return {
    slot,
    role: seed.role,
    name: seed.name,
    voc: seed.voc,
    vocId: seed.vocId,
    damage: 0,
    hits: 0,
    keys: [],
    hp: hpAt(slot, t),
    st: stAt(slot, t),
    down: false,
    left: false,
    noDmg: [],
    baseParts: seed.parts,
    baseMix: seed.mix,
  };
}

/** FIGHT 7 at fight second t, as the vein would have drawn it. */
function frameAt(t: number, left: Left | null = null): F {
  const leftSlot = left ? left.slot : -1;
  const leftAt = left ? left.t : Number.POSITIVE_INFINITY;
  const ms = PARTY7.map((seed, slot) => {
    const m = seedMember(seed, slot, t);
    const until = slot === leftSlot ? leftAt : Number.POSITIVE_INFINITY;
    m.left = slot === leftSlot && t > leftAt;
    m.noDmg = (seed.casts ?? [])
      .map(([k, ts]): [string, number] => [k, ts.filter((x) => x <= t && x <= until).length])
      .filter(([, c]) => c > 0);
    return m;
  });
  const f: F = {
    n: 7,
    foes: [{ name: "Thornjaw", count: 3 }],
    t,
    members: ms,
    unat: 0,
    unatHits: 0,
    unatBiggest: 0,
    world: 0,
    worldHits: 0,
    kills: [],
    settled: false,
    agoS: 0,
    attribution: "checked",
    types: "additive",
    skills: "available",
    gaps: 0,
    paused: false,
    partyFormed: true,
    ledger: null,
  };
  for (const h of HITS7) {
    if (h.t > t) break;
    if (h.slot === null) {
      f.unat += h.dmg;
      f.unatHits++;
      f.unatBiggest = Math.max(f.unatBiggest, h.dmg);
      continue;
    }
    if (h.slot === leftSlot && h.t > leftAt) continue; // gone: what it dealt stays, nothing more lands
    const m = ms[h.slot];
    m.damage += h.dmg;
    m.hits++;
    let k = m.keys.find((x) => x.key === h.key);
    if (!k) {
      k = { kind: h.kind, key: h.key, damage: 0, hits: 0, biggest: 0, lastBlow: false, first: h.t };
      m.keys.push(k);
    }
    k.damage += h.dmg;
    k.hits++;
    k.biggest = Math.max(k.biggest, h.dmg);
    if (h.kill) {
      k.lastBlow = true;
      f.kills.push({ slot: h.slot, key: keyId(h.kind, h.key), at: h.t });
    }
  }
  f.ledger = ledgerSeed(partyOf(f));
  return f;
}

// Recent fights: [n, foe, count, dur s, ago min, ledger ratio, party, unat, unatHits]
type OldMember = [string, FrayRole, string, number, [string, number, number, number][], [string, number][]?];
type Old = [number, string, number, number, number, number, (OldMember | null)[], number, number];
const OLD: Old[] = [
  [
    6,
    "Ashgnawer",
    2,
    38,
    6,
    0.991,
    [
      [
        "Odalric",
        "arisen",
        "Warrior",
        5200,
        [
          ["Riven Arc", 0.48, 6, 790],
          ["Cinder Rush", 0.3, 3, 700],
          ["action 3A9F", 0.16, 11, 120],
          ["Unknown", 0.06, 2, 180],
        ],
      ],
      [
        "Wren",
        "mainPawn",
        "Thief",
        3100,
        [
          ["Quick Cut", 0.52, 9, 300],
          ["Shadow Feint", 0.38, 3, 450],
          ["Unknown", 0.1, 2, 200],
        ],
      ],
      [
        "Bramble",
        "hiredPawn",
        "Sorcerer",
        4400,
        [
          ["Ember Lance", 0.7, 4, 960],
          ["Ashfall", 0.3, 4, 400],
        ],
      ],
      ["Sable", "hiredPawn", "Mage", 300, [["Frost Thread", 1, 3, 130]], [["Mending Light", 3]]],
    ],
    400,
    3,
  ],
  [
    5,
    "the Marrowback",
    1,
    130,
    23,
    0.9926,
    [
      [
        "Odalric",
        "arisen",
        "Warrior",
        30100,
        [
          ["Cinder Rush", 0.4, 18, 1240],
          ["Riven Arc", 0.34, 20, 900],
          ["Bulwark Turn", 0.14, 5, 980],
          ["action 3A9F", 0.08, 40, 150],
          ["Unknown", 0.04, 7, 210],
        ],
      ],
      [
        "Wren",
        "mainPawn",
        "Thief",
        18200,
        [
          ["Shadow Feint", 0.44, 15, 610],
          ["Quick Cut", 0.36, 30, 340],
          ["Hook and Run", 0.14, 7, 520],
          ["Unknown", 0.06, 5, 230],
        ],
      ],
      [
        "Bramble",
        "hiredPawn",
        "Sorcerer",
        24500,
        [
          ["Ember Lance", 0.55, 14, 1180],
          ["Ashfall", 0.4, 20, 610],
          ["Kindle Ward", 0.05, 4, 330],
        ],
      ],
      [
        "Sable",
        "hiredPawn",
        "Mage",
        1900,
        [["Frost Thread", 1, 11, 240]],
        [
          ["Mending Light", 12],
          ["Hearthsong", 2],
        ],
      ],
    ],
    4720,
    21,
  ],
  [
    4,
    "Bell-Ram",
    1,
    51,
    34,
    0.97,
    [
      [
        "Odalric",
        "arisen",
        "Warrior",
        7000,
        [
          ["Riven Arc", 0.5, 9, 700],
          ["Cinder Rush", 0.35, 4, 820],
          ["Unknown", 0.15, 3, 400],
        ],
      ],
      [
        "Wren",
        "mainPawn",
        "Thief",
        5200,
        [
          ["Quick Cut", 0.6, 14, 330],
          ["Hook and Run", 0.4, 4, 560],
        ],
      ],
      [
        "Bramble",
        "hiredPawn",
        "Sorcerer",
        6100,
        [
          ["Ashfall", 0.55, 8, 520],
          ["Ember Lance", 0.45, 3, 980],
        ],
      ],
      ["Sable", "hiredPawn", "Mage", 600, [["Frost Thread", 1, 4, 190]], [["Mending Light", 4]]],
    ],
    900,
    5,
  ],
  [
    3,
    "Thornjaw",
    1,
    65,
    52,
    0.988,
    [
      [
        "Aldith",
        "arisen",
        "Fighter",
        9000,
        [
          ["Shieldfall", 0.46, 10, 720],
          ["Lantern Strike", 0.38, 12, 450],
          ["Unknown", 0.16, 4, 400],
        ],
      ],
      [
        "Corvin",
        "mainPawn",
        "Archer",
        7400,
        [
          ["Longdraw", 0.62, 9, 780],
          ["Split Fletch", 0.38, 14, 260],
        ],
      ],
      [
        "Bramble",
        "hiredPawn",
        "Sorcerer",
        6800,
        [
          ["Ember Lance", 0.6, 5, 1000],
          ["Ashfall", 0.4, 7, 480],
        ],
      ],
      null,
    ],
    1200,
    6,
  ],
];

function oldMember(p: OldMember, slot: number): M {
  const [name, role, voc, dmg, sk, noDmg] = p;
  const keys: Key[] = sk.map(([k, fr, h, b], i) => {
    const isAct = k.startsWith("action ");
    let kind: FraySkillKind = isAct ? "action" : "skill";
    if (k === "Unknown") kind = "unknown";
    return {
      kind,
      key: isAct ? k.slice(7) : k,
      damage: Math.round(dmg * fr),
      hits: h,
      biggest: b,
      lastBlow: false,
      first: i,
    };
  });
  // the damage is the sum of its keys (the frame's invariant), never the typed round figure
  const damage = keys.reduce((a, k) => a + k.damage, 0);
  const base = PARTY7.find((x) => x.voc === voc && x.name === name) ?? PARTY7[slot];
  return {
    ...seedMember(base, slot, 0),
    name,
    role,
    voc,
    vocId: VOCATIONS[voc] ?? 0,
    damage,
    hits: sk.reduce((a, s) => a + s[2], 0),
    keys,
    noDmg: noDmg ?? [],
    hp: 1,
    st: 1,
  };
}

function oldFight([n, foe, count, dur, ago, ratio, party, unat, unatHits]: Old): F {
  const f: F = {
    ...frameAt(0),
    n,
    foes: [{ name: foe, count }],
    t: dur,
    members: party.map((p, slot) => (p ? oldMember(p, slot) : null)),
    unat,
    unatHits,
    unatBiggest: Math.round(unat / Math.max(1, unatHits)) + 40,
    kills: [],
    settled: true,
    agoS: ago * 60,
  };
  f.ledger = Math.round(partyOf(f) / ratio);
  return f;
}

// ---------- the scene recipes (the audition's decorate) ----------

interface Recipe {
  t: number;
  settled?: boolean;
  agoS?: number;
  left?: Left;
  w?: number;
  scale?: number;
  long?: boolean;
  sup?: boolean;
  suptwo?: boolean;
  healer?: "casts" | "quiet";
  three?: boolean;
  partyUnknown?: boolean;
  roster?: boolean;
  next?: boolean;
  emptySlot?: number;
  ledgerRatio?: number;
  down?: number[];
  low?: number[];
  hpNull?: number[];
  f?: Partial<Pick<F, "attribution" | "types" | "skills" | "gaps" | "paused" | "n" | "t" | "ledger">>;
}

function member(f: F, slot: number): M {
  const m = f.members[slot];
  if (!m) throw new Error(`The mock fray has no member in slot ${slot}`);
  return m;
}

function applyLong(f: F): void {
  const nm = [
    "Odalric Ashfordine-Vellacourt the Unbending",
    "Wrenna Marchbank of the Long Hollow",
    "Bramblewood Fenwick-Oakhart",
    "Sable Ingleside Moorcroft",
  ];
  for (const [i, m] of f.members.entries()) if (m) m.name = nm[i];
  const m = member(f, 0);
  const k = (12840 * f.t) / m.damage; // seeded to the note: exactly 12,840 /s
  m.keys = m.keys.map((x) => ({
    ...x,
    damage: Math.round(x.damage * k),
    key: x.key === "Riven Arc" ? "Riven Arc of the Ninefold Stair" : x.key,
  }));
  m.damage = 12840 * f.t;
  m.keys[0].damage += m.damage - sumKeys(m);
  f.foes.push({ name: "Ashgnawer", count: 1 }, { name: "Bell-Ram", count: 1 });
}

function applyThree(f: F): void {
  const od = member(f, 0);
  const act = od.keys.find((k) => k.kind === "action");
  if (act) act.damage = 5200; // above Cinder Rush: a live top-two row
  od.damage = sumKeys(od);
  const wr = member(f, 1);
  const unk = wr.keys.find((k) => k.kind === "unknown");
  if (unk) {
    unk.damage = Math.round((wr.damage - unk.damage) * 0.3); // Unknown 23%
    unk.hits += 5;
    wr.hits += 5;
  }
  wr.damage = sumKeys(wr);
  f.unat = Math.round((0.13 * partyOf(f)) / 0.87); // unattributed 13%
  f.unatHits += 20;
}

function applyRoster(f: F): void {
  const merrow: M = { ...seedMember(PARTY7[3], 3, f.t), name: "Merrow", voc: "Archer", vocId: 2, hp: 0.9, st: 0.8 };
  const dmg = member(f, 3).damage * 6;
  merrow.keys = [
    { kind: "skill", key: "Longdraw", damage: Math.round(dmg * 0.7), hits: 8, biggest: 600, lastBlow: false, first: 1 },
    {
      kind: "action",
      key: "B21C",
      damage: dmg - Math.round(dmg * 0.7),
      hits: 15,
      biggest: 90,
      lastBlow: false,
      first: 2,
    },
  ];
  merrow.damage = dmg;
  merrow.hits = 23;
  f.members[2] = null;
  f.members[3] = merrow;
}

function applySupport(f: F, r: Recipe): void {
  const m = member(f, 3);
  if (r.sup) {
    m.noDmg = [["Mending Light", 7]];
    m.keys = [
      { kind: "skill", key: "Frost Thread", damage: 566, hits: 4, biggest: 190, lastBlow: false, first: 1 },
      { kind: "unknown", key: "Unknown", damage: 141, hits: 2, biggest: 90, lastBlow: false, first: 2 },
    ];
  }
  if (r.suptwo) {
    m.noDmg = [["Mending Light", 7]];
    m.keys = [
      { kind: "skill", key: "Frost Thread", damage: 540, hits: 4, biggest: 190, lastBlow: false, first: 1 },
      { kind: "skill", key: "Sleetquill", damage: 390, hits: 3, biggest: 160, lastBlow: false, first: 2 },
      { kind: "unknown", key: "Unknown", damage: 90, hits: 1, biggest: 90, lastBlow: false, first: 3 },
    ];
  }
  if (r.healer) {
    m.keys = [];
    m.noDmg =
      r.healer === "casts"
        ? [
            ["Mending Light", 7],
            ["Hearthsong", 2],
          ]
        : [];
  }
  m.damage = sumKeys(m);
  m.hits = m.keys.reduce((a, k) => a + k.hits, 0);
  // a kill credited to a key the member no longer holds is no kill of theirs
  f.kills = f.kills.filter((k) => k.slot !== 3 || m.keys.some((x) => keyId(x.kind, x.key) === k.key));
}

function applyHealth(f: F, r: Recipe): void {
  for (const m of f.members) {
    if (!m) continue;
    if (r.down?.includes(m.slot)) {
      m.down = true;
      m.hp = 0;
    } else if (r.low?.includes(m.slot)) m.hp = 0.18;
    if (r.hpNull?.includes(m.slot)) {
      m.hp = null;
      m.st = null;
    }
  }
}

function applyPartyUnknown(f: F): void {
  f.partyFormed = false;
  for (const m of f.members) {
    if (!m) continue;
    f.unat += m.damage;
    f.unatHits += m.hits;
    f.unatBiggest = Math.max(f.unatBiggest, ...m.keys.map((k) => k.biggest));
    m.damage = 0;
    m.hits = 0;
    m.keys = [];
  }
  f.kills = f.kills.map((k) => ({ ...k, slot: null, key: null }));
}

function applyScale(f: F, scale: number): void {
  for (const m of f.members) {
    if (!m) continue;
    m.keys = m.keys.map((x) => ({ ...x, damage: Math.round(x.damage * scale) }));
    m.damage = sumKeys(m);
  }
  f.unat = Math.round(f.unat * scale);
}

function cook(r: Recipe): F {
  const f = frameAt(r.f?.t ?? r.t, r.left ?? null);
  if (r.w) {
    f.world = r.w;
    f.worldHits = r.w > 2000 ? 9 : 4;
  }
  if (r.scale) applyScale(f, r.scale);
  if (r.long) applyLong(f);
  if (r.sup || r.suptwo || r.healer) applySupport(f, r);
  if (r.three) applyThree(f);
  if (r.partyUnknown) applyPartyUnknown(f);
  if (r.roster) applyRoster(f);
  if (r.next) {
    f.n = 8;
    f.foes = [{ name: "Ashgnawer", count: 1 }];
    if (r.emptySlot !== undefined) f.members[r.emptySlot] = null;
  }
  applyHealth(f, r);
  f.ledger = r.ledgerRatio ? Math.round(partyOf(f) / r.ledgerRatio) : ledgerSeed(partyOf(f)); // the world never enters it
  if (r.partyUnknown) f.ledger = ledgerSeed(partyOf(frameAt(f.t)));
  if (r.f) Object.assign(f, r.f);
  if (r.settled) {
    f.settled = true;
    f.agoS = r.agoS ?? 0;
    for (const m of f.members) if (m) m.hp = m.st = 1;
  }
  return f;
}

// ---------- the contract ----------

const pad = (n: number): string => String(n).padStart(2, "0");
const hhmm = (d: Date): string => `${pad(d.getHours())}:${pad(d.getMinutes())}`;

function toSkill(k: Key): FraySkill {
  return {
    kind: k.kind,
    id: keyId(k.kind, k.key),
    name: k.kind === "skill" ? k.key : null,
    damage: k.damage,
    hits: k.hits,
    biggest: k.biggest,
    lastBlow: k.lastBlow,
    firstAt: k.first,
  };
}

function toMember(m: M, f: F): FrayMember {
  const split = partsFor(m.baseParts, m.baseMix, m.damage);
  return {
    slot: m.slot,
    role: m.role,
    name: m.name,
    vocation: { id: m.vocId, name: m.voc },
    hp: m.hp,
    st: m.st,
    down: m.down,
    left: m.left,
    damage: m.damage,
    dps: f.t > 0 ? m.damage / f.t : null,
    hits: m.hits,
    skills: m.keys.map(toSkill),
    noDamageSkills: m.noDmg.length ? m.noDmg.map(([name, count]) => ({ name, count })) : null,
    parts: f.types === "additive" ? split.parts : null,
    mix: split.mix,
  };
}

function biggestOf(f: F): FrayFight["biggest"] {
  let best: FrayFight["biggest"] = null;
  for (const m of f.members) {
    for (const k of m?.keys ?? []) {
      if (m && (!best || k.biggest > best.damage))
        best = { slot: m.slot, key: keyId(k.kind, k.key), damage: k.biggest };
    }
  }
  return best;
}

function toFight(f: F, now: Date): FrayFight {
  const members = f.members.map((m) => (m ? toMember(m, f) : null));
  const hParty = members.reduce((a, m) => a + (m ? m.damage : 0), 0);
  const world =
    f.world > 0 ? { damage: f.world, hits: f.worldHits, biggest: Math.round(f.world / f.worldHits) + 60 } : null;
  return {
    id: `fight-${f.n}`,
    n: f.n,
    state: f.settled ? "settled" : "open",
    durationS: f.t,
    paused: f.paused,
    foes: f.foes.map((x, i) => ({ cid: 4100 + f.n * 10 + i, name: x.name, count: x.count })),
    endedAt: f.settled ? hhmm(new Date(now.getTime() - f.agoS * 1000)) : null,
    endedAtMs: f.settled ? now.getTime() - f.agoS * 1000 : null,
    members,
    hParty,
    hAll: hParty + f.unat + f.world,
    unattributed: { damage: f.unat, hits: f.unatHits, biggest: f.unatBiggest },
    world,
    ledger: f.ledger,
    partyFormed: f.partyFormed,
    attribution: f.attribution,
    types: f.types,
    skills: f.skills,
    kills: f.kills,
    gaps: f.gaps,
    biggest: f.partyFormed ? biggestOf(f) : null,
  };
}

const LIVE_FEED: FrayFeed = {
  state: "live",
  lastBeatAgoS: 0,
  gameRunning: true,
  gameStartedMs: null, // stamped per frame from the scene's startedAgoS
  reframework: true,
  eyeInstalled: true,
  eyeRecording: true,
  eyeReason: null,
};

// ---------- the scenes ----------

interface Scene {
  feed?: Partial<FrayFeed>;
  /** null: no fight on the stele. */
  fight: Recipe | null;
  /** The shelf under the stele, newest first (settled fights only). */
  shelf?: "old" | "seven" | "eight" | "mix" | "none";
  view?: FrayViewHint;
  /** How long ago this run of the game started (s); default 45 min. Unused while it isn't running. */
  startedAgoS?: number;
  /** The eye's roster outside a fight: the §19.14 party (default) or none read yet. */
  party?: "seven" | "none";
}

const REC = (extra: Omit<Recipe, "t"> = {}): Recipe => ({ t: DUR7, settled: true, ...extra });
const LIVE = (extra: Omit<Recipe, "t"> = {}, t = LIVE_T): Recipe => ({ t, ...extra });
const KEPT: Recipe = REC({ agoS: 2 * 3600 });

const SCENES: Record<Exclude<FrayScene, "fraysettle">, Scene> = {
  fray: { fight: LIVE() },
  fraypaused: { fight: LIVE({ f: { paused: true } }, 64) },
  fraydown: { fight: LIVE({ down: [1], low: [0], hpNull: [3] }, 80) },
  frayworld: { fight: LIVE({ w: 1250 }) },
  frayworldledger: { fight: LIVE({ w: 4000 }) },
  frayroster: { fight: LIVE({ roster: true }, 70) },
  frayleft: { fight: LIVE({ left: { slot: 2, t: 55 } }, 80) },
  frayunknown: { fight: LIVE({ f: { skills: "unavailable" } }) },
  fraylong: { fight: LIVE({ long: true }) },
  fraysupporttwo: { fight: LIVE({ suptwo: true }) },
  fraysupportunknown: { fight: LIVE({ sup: true }) },
  frayhealer: { fight: LIVE({ healer: "casts" }) },
  frayhealerquiet: { fight: LIVE({ healer: "quiet" }) },
  fraythree: { fight: LIVE({ three: true, f: { attribution: "unqualified" } }) },
  fraypartyunknown: { fight: LIVE({ partyUnknown: true }) },
  fraypartyfour: { fight: LIVE({ scale: 10 }) },
  fraynext: { fight: LIVE({ next: true }, 3), shelf: "seven" },
  frayleftnext: { fight: LIVE({ next: true, emptySlot: 2 }, 3), shelf: "seven" },
  fraysettled: { fight: REC() },
  frayworldsettled: { fight: REC({ w: 1250 }) },
  frayidleafter: { fight: REC({ agoS: 6 * 60 }) },
  fraymember: { fight: REC(), view: { open: 0 } },
  fraytypes: { fight: REC(), view: { sub: "types" } },
  frayproportions: { fight: REC({ f: { types: "mix" } }), view: { sub: "types" } },
  fraytypesunknown: { fight: REC({ f: { types: "unknown" } }), view: { sub: "types" } },
  frayverdictmix: { fight: REC({ f: { types: "mix" } }), view: { sub: "types" } },
  frayverdictmixopen: { fight: REC({ f: { types: "mix" } }), view: { open: 0 } },
  fraytypesapprox: { fight: REC({ f: { types: "mix", attribution: "unqualified" } }), view: { sub: "types" } },
  fraysupporttwosettled: { fight: REC({ suptwo: true }) },
  frayshelf: { fight: REC(), view: { browse: 5 } },
  frayshelfsix: { fight: REC({ next: true, f: { t: 60 } }), shelf: "seven" },
  frayshelfmix: { fight: REC(), shelf: "mix", view: { browse: 5, sub: "types" } },
  fraywidetag: { fight: REC({ f: { n: 999, t: 3599 } }) },
  frayunqualified: { fight: LIVE({ f: { attribution: "unqualified" } }) },
  frayinconclusive: { fight: REC({ f: { attribution: "inconclusive" } }) },
  frayshort: { fight: LIVE({ ledgerRatio: 0.88 }) },
  frayover: { fight: REC({ ledgerRatio: 1.021 }) },
  fraynoledger: { fight: LIVE({ f: { ledger: null } }) },
  frayunmoved: { fight: REC({ f: { ledger: 0 } }) },
  fraygap: { fight: LIVE({ ledgerRatio: 0.885, f: { gaps: 3, attribution: "unqualified" } }) },
  fraygapsettled: { fight: REC({ ledgerRatio: 0.885, f: { gaps: 3 } }) },
  fraystale: { fight: LIVE({}, 82), feed: { state: "stale", lastBeatAgoS: 14 } },
  fraystaleapprox: {
    fight: LIVE({ f: { attribution: "unqualified" } }, 82),
    feed: { state: "stale", lastBeatAgoS: 14 },
  },
  frayidle: { fight: null, shelf: "none" },
  frayasleep: { fight: KEPT, feed: { state: "none", lastBeatAgoS: null, gameRunning: false, eyeRecording: null } },
  frayasleepfresh: {
    fight: null,
    shelf: "none",
    feed: { state: "none", lastBeatAgoS: null, gameRunning: false, eyeRecording: null },
  },
  fraynorefw: {
    fight: KEPT,
    feed: { reframework: false, eyeInstalled: false, eyeRecording: null, state: "none", lastBeatAgoS: null },
  },
  fraynoeye: { fight: KEPT, feed: { eyeInstalled: false, eyeRecording: null, state: "none", lastBeatAgoS: null } },
  frayshut: {
    fight: KEPT,
    feed: {
      eyeRecording: false,
      eyeReason: "a hook found no method to hold on this game version",
      state: "none",
      lastBeatAgoS: null,
    },
  },
  fraysilent: {
    fight: null,
    shelf: "none",
    startedAgoS: 40,
    feed: { state: "none", lastBeatAgoS: null, eyeRecording: null },
  },
};

const SHELF_OLD = OLD.map(oldFight);

/** frayshelfmix: FIGHT 5's frame did not add up (mix), FIGHT 4's is unknown (a verdict is a frame's, RF.20). */
const MIX_SHELF: Record<number, FrayTypes | undefined> = { 5: "mix", 4: "unknown" };

function shelfOf(kind: Scene["shelf"], now: Date): FrayFight[] {
  if (kind === "none") return [];
  const seven = cook(REC({ agoS: 2 * 60 }));
  let fights: F[] = SHELF_OLD;
  if (kind === "seven" || kind === "eight") fights = [seven, ...SHELF_OLD];
  if (kind === "mix") fights = SHELF_OLD.map((x) => ({ ...x, types: MIX_SHELF[x.n] ?? x.types }));
  return fights.slice(0, 5).map((x) => toFight(x, now));
}

/** The shelf beside a settled stele fight: it leads, the rest follow, five at most (RF.26). */
function recentFor(fight: FrayFight | null, shelf: FrayFight[]): FrayFight[] {
  const all = fight?.state === "settled" ? [fight, ...shelf.filter((x) => x.n !== fight.n)] : shelf;
  return [...all].sort((a, b) => b.n - a.n).slice(0, 5);
}

/** One scene's frame at `now`. `elapsedS` drives the play scene and the stale age. */
export function frayMockFrame(scene: string, elapsedS = 0, now: Date = new Date()): FrayFrame {
  if (scene === "fraysettle") return settleFrame(elapsedS, now);
  const s = SCENES[isFrayScene(scene) && scene !== "fraysettle" ? scene : "fray"];
  const fight = s.fight ? toFight(cook(s.fight), now) : null;
  const feed: FrayFeed = { ...LIVE_FEED, ...s.feed };
  if (feed.state === "stale" && feed.lastBeatAgoS !== null) feed.lastBeatAgoS += Math.floor(elapsedS);
  feed.gameStartedMs = startedMs(feed, s.startedAgoS ?? DEFAULT_STARTED_AGO_S, elapsedS, now);
  const party = s.party === "none" ? [] : partyOf7(feed);
  return { feed, fight, recent: recentFor(fight, shelfOf(s.shelf ?? "old", now)), party };
}

const DEFAULT_STARTED_AGO_S = 45 * 60;

/** When this run of the game started, held still across pulls; null while it isn't running. */
function startedMs(feed: FrayFeed, agoS: number, elapsedS: number, now: Date): number | null {
  return feed.gameRunning ? now.getTime() - (agoS + elapsedS) * 1000 : null;
}

/** The eye's roster outside a fight: the §19.14 party, no damage, no skills, live hp/st only while the game runs. */
function partyOf7(feed: FrayFeed): (FrayMember | null)[] {
  const resting = frameAt(0);
  return PARTY7.map((seed, slot) => {
    const m = { ...seedMember(seed, slot, 3), noDmg: [] };
    if (!feed.gameRunning) m.hp = m.st = null;
    return toMember(m, resting);
  });
}

/** fraysettle: FIGHT 7 from 1:36 at 1×, its final heartbeat at 1:42, then settled. */
function settleFrame(elapsedS: number, now: Date): FrayFrame {
  const t = Math.min(DUR7, 96 + Math.floor(elapsedS));
  const settled = elapsedS >= DUR7 - 96 + 1;
  const fight = toFight(cook(settled ? REC() : LIVE({}, t)), now);
  const feed: FrayFeed = { ...LIVE_FEED };
  feed.gameStartedMs = startedMs(feed, DEFAULT_STARTED_AGO_S, elapsedS, now);
  return { feed, fight, recent: recentFor(fight, shelfOf("old", now)), party: partyOf7(feed) };
}

/** Where the window's view starts in this scene. */
export function frayMockView(scene: string): FrayViewHint {
  if (!isFrayScene(scene) || scene === "fraysettle") return {};
  return SCENES[scene].view ?? {};
}

/** The mock source: `pull()` answers the scene's frame on a real clock. */
export function createFrayMock(
  scene: string,
  clock: () => number = () => performance.now(),
): { pull(): FrayFrame; view: FrayViewHint } {
  const start = clock();
  return {
    pull: () => frayMockFrame(scene, (clock() - start) / 1000),
    view: frayMockView(scene),
  };
}
