// The mock's party (#00015 R17.30), value for value, and its scenes (R17.31).
// Every name here is invented for this mock. The vocations are the six
// plain-English ones; every item, augment, skill and trait name is the
// mock's own, never the game's text. This file ships as part of the mock's
// lazy chunk, so it may carry nothing else.

import type {
  MusterGains,
  MusterGear,
  MusterGearSlot,
  MusterMember,
  MusterSlot,
  MusterSlotsView,
  MusterView,
  Named,
} from "@/types/rift";

/** The six plain-English vocations, under the mock's own ids. */
export const MOCK_VOCATIONS = {
  Fighter: { id: 1, name: "Fighter" },
  Archer: { id: 2, name: "Archer" },
  Mage: { id: 3, name: "Mage" },
  Thief: { id: 4, name: "Thief" },
  Warrior: { id: 5, name: "Warrior" },
  Sorcerer: { id: 6, name: "Sorcerer" },
} as const satisfies Record<string, Named>;

type VocationName = keyof typeof MOCK_VOCATIONS;

/** The invented people of the mock party: the only person names any frame,
 *  example or spec may use (R17.30). */
export const MOCK_PARTY_NAMES = ["Odalric", "Wren", "Bramble", "Sable"] as const;

/** Every invented game-thing name of the mock (items, augments, skills, traits),
 *  so a spec can hold the party to them. */
export const MOCK_THING_NAMES: readonly string[] = [];

let nextThing = 40_000;
function thing(name: string): Named {
  (MOCK_THING_NAMES as string[]).push(name);
  nextThing += 7;
  return { id: nextThing, name };
}

const voc = (name: VocationName): Named => ({ ...MOCK_VOCATIONS[name] });

const GEAR_SLOTS: readonly MusterGearSlot[] = [
  "mainWeapon",
  "subWeapon",
  "head",
  "body",
  "legs",
  "cloak",
  "ring1",
  "ring2",
];

/** The one item the game's text has no name for (R17.21): Sable's first ring. */
export const MOCK_UNNAMED_RING: Named = { id: 40917, name: null };

const gear = (...items: (Named | null)[]): MusterGear[] =>
  GEAR_SLOTS.map((slot, i) => ({ slot, item: items[i] ?? null }));

const gains = (...n: number[]): MusterGains => ({
  health: n[0] ?? 0,
  stamina: n[1] ?? 0,
  strength: n[2] ?? 0,
  defence: n[3] ?? 0,
  magick: n[4] ?? 0,
  magickDefence: n[5] ?? 0,
  weight: n[6] ?? 0,
  knockdown: n[7] ?? 0,
});

/** The ring Odalric finds before the dev hook's save (R17.31). */
const RING_OF_EMBERS = thing("Ring of Embers");

function slotNine(): Omit<MusterView, "slotId" | "slot" | "savedAt" | "names" | "namesNote"> {
  const arisen: MusterMember = {
    memberKey: "a41c09e2d7b35f18",
    role: "arisen",
    name: "Odalric",
    level: 41,
    experience: 1_284_550,
    vocation: voc("Warrior"),
    vocations: [
      { vocation: voc("Warrior"), experience: 412_300 },
      { vocation: voc("Fighter"), experience: 198_440 },
      { vocation: voc("Archer"), experience: 36_010 },
    ],
    gains: gains(1_240, 610, 312, 268, 96, 118, 6, 140),
    gear: gear(
      thing("Ashen Greatsword"),
      null,
      thing("Warden's Barbute"),
      thing("Lanternwright's Hauberk"),
      thing("Pilgrim's Greaves"),
      thing("Mantle of the Ninth Bell"),
      thing("Ring of Low Tides"),
      thing("Ring of the Still Hour"),
    ),
    augments: [thing("Ironhide"), thing("Long Stride"), thing("Second Wind")],
    skills: [thing("Riven Arc"), thing("Cinder Rush"), thing("Bulwark Turn")],
    pawn: null,
    creator: null,
  };
  const wren: MusterMember = {
    memberKey: "5be0f7a19c24d613",
    role: "mainPawn",
    name: "Wren",
    level: 39,
    experience: 1_102_300,
    vocation: voc("Thief"),
    vocations: [
      { vocation: voc("Thief"), experience: 388_900 },
      { vocation: voc("Archer"), experience: 120_200 },
    ],
    gains: gains(880, 940, 276, 172, 88, 104, 2, 60),
    gear: gear(
      thing("Twin Fangs of Harrowmere"),
      null,
      thing("Fieldmouse Hood"),
      thing("Moth-Grey Jerkin"),
      thing("Brambleweave Breeches"),
      thing("Dusk Shawl"),
      thing("Thimble Ring"),
      null,
    ),
    augments: [thing("Light Step"), thing("Keen Ear")],
    skills: [thing("Shadow Feint"), thing("Quick Cut"), thing("Hook and Run")],
    pawn: { inclination: thing("Calm"), specialization: thing("Healer") },
    creator: null,
  };
  const bramble: MusterMember = {
    memberKey: "c3d81e5a40f97b26",
    role: "hiredPawn",
    name: "Bramble",
    level: 49,
    experience: 2_048_900,
    vocation: voc("Sorcerer"),
    vocations: [
      { vocation: voc("Sorcerer"), experience: 702_000 },
      { vocation: voc("Mage"), experience: 310_500 },
    ],
    gains: gains(760, 540, 98, 120, 486, 402, 1, 40),
    gear: gear(
      thing("Staff of the Ninth Ember"),
      null,
      thing("Cowl of the Grey Choir"),
      thing("Robe of Banked Coals"),
      thing("Ashwalker's Wraps"),
      null,
      thing("Ring of the Cold Hearth"),
      null,
    ),
    augments: [thing("Deep Well"), thing("Steady Hand"), thing("Slow Burn"), thing("Far Sight")],
    skills: [thing("Ember Lance"), thing("Ashfall"), thing("Kindle Ward")],
    pawn: { inclination: thing("Bold"), specialization: null },
    creator: { level: 52, vocation: voc("Warrior") },
  };
  const sable: MusterMember = {
    memberKey: "9f2a6c08e1b4d357",
    role: "hiredPawn",
    name: "Sable",
    level: 30,
    experience: 612_400,
    vocation: voc("Mage"),
    vocations: [{ vocation: voc("Mage"), experience: 244_100 }],
    gains: gains(520, 430, 74, 86, 262, 248, 1, 30),
    gear: gear(
      thing("Wand of Quiet Rain"),
      null,
      thing("Lamplighter's Cap"),
      thing("Wayfarer's Tunic"),
      thing("Wayfarer's Leggings"),
      thing("Mantle of Small Hours"),
      { ...MOCK_UNNAMED_RING },
      thing("Ring of Lantern Moss"),
    ),
    augments: [thing("Soft Tread")],
    skills: [thing("Mending Light"), thing("Frost Thread")],
    pawn: { inclination: thing("Kind"), specialization: thing("Scholar") },
    creator: { level: 30, vocation: voc("Archer") },
  };
  return { arisen, mainPawn: wren, hired: [bramble, sable], gold: 284_310, riftCrystals: 6_420 };
}

/** Built once, so every thing keeps one id; each read hands out a fresh copy. */
const SLOT_NINE = slotNine();

/** The window's local clock at a fixed time of day, `daysAgo` days back. */
function localAt(daysAgo: number, hours: number, minutes: number): string {
  const d = new Date();
  d.setDate(d.getDate() - daysAgo);
  d.setHours(hours, minutes, 0, 0);
  return d.toISOString();
}

const fixedLocal = (month: number, day: number, hours: number, minutes: number): string =>
  new Date(2026, month - 1, day, hours, minutes).toISOString();

/** The four slots of R17.30, newest first. */
function slotList(): MusterSlot[] {
  return [
    {
      slotId: "data009Slot",
      slot: 9,
      savedAt: localAt(0, 21, 4),
      account: null,
      arisen: { name: "Odalric", level: 41, vocation: voc("Warrior") },
      problem: null,
    },
    {
      slotId: "data008Slot",
      slot: 8,
      savedAt: localAt(1, 20, 12),
      account: null,
      arisen: { name: "Odalric", level: 40, vocation: voc("Warrior") },
      problem: null,
    },
    {
      slotId: "data003Slot",
      slot: 3,
      savedAt: fixedLocal(10, 12, 18, 40),
      account: null,
      arisen: null,
      problem: "damaged",
    },
    {
      slotId: "data001Slot",
      slot: 1,
      savedAt: fixedLocal(10, 9, 11, 2),
      account: null,
      arisen: null,
      problem: "otherAccount",
    },
  ];
}

/** R17.31 musternames: the sentence the spine gives when the names are missing. */
export const MOCK_NAMES_NOTE =
  "The stone found no Dragon's Dogma 2 install, and the names live in the game's own text.";

/** Every `Named` in a value with its name taken away (the names did not answer). */
export function stripNames<T>(value: T): T {
  if (Array.isArray(value)) return value.map((v: unknown) => stripNames(v)) as T;
  if (value && typeof value === "object") {
    const entries = Object.entries(value as Record<string, unknown>).map(([k, v]) => [k, stripNames(v)] as const);
    const out = Object.fromEntries(entries) as Record<string, unknown>;
    if (typeof out.id === "number" && "name" in out && Object.keys(out).length === 2) out.name = null;
    return out as T;
  }
  return value;
}

export interface MusterScene {
  folder: "noSteam" | "noSaves" | null;
  namesMissing: boolean;
}

/** Slot 9 after the dev hook's save: written half an hour later, its Arisen a level up. */
const LATER_SAVED_AT = (): string => localAt(0, 21, 34);

export function mockMusterSlots(scene: MusterScene, later = false): MusterSlotsView {
  if (scene.folder) return { problem: scene.folder, slots: [], accountCount: 0, names: "game" };
  const slots = slotList();
  const nine = slots.find((s) => s.slotId === "data009Slot");
  if (later && nine?.arisen) {
    nine.arisen.level = 42;
    nine.savedAt = LATER_SAVED_AT();
  }
  return {
    problem: null,
    slots: scene.namesMissing ? stripNames(slots) : slots,
    accountCount: 1,
    names: scene.namesMissing ? "missing" : "game",
  };
}

/** The spine's own sentences for the slots that will not open (invented, file names only). */
export const MOCK_SAVE_SENTENCES = {
  damaged: (file: string) => `${file} would not open: its stream ends before its table says it should.`,
  otherAccount: (file: string) => `${file} opens with no Steam account found on this machine.`,
  beingWritten: (file: string) => `${file} changed between two reads 500 ms apart.`,
  gone: (file: string) => `${file} is no longer in the save folder.`,
  noParty: (file: string) => `${file} holds no party record.`,
} as const;

/** One slot's party, or the reason it will not open. Slot 8 is the same party
 *  one level earlier (Odalric Lv 40). */
export function mockMuster(
  slotId: string,
  scene: MusterScene,
  later: boolean,
): MusterView | keyof typeof MOCK_SAVE_SENTENCES {
  const listed = slotList().find((s) => s.slotId === slotId);
  if (!listed) return "gone";
  if (listed.problem) return listed.problem;
  const party = structuredClone(SLOT_NINE);
  if (slotId === "data008Slot" && party.arisen) {
    party.arisen.level = 40;
    party.arisen.experience = 1_250_100;
  }
  if (slotId === "data009Slot" && later) applyLater(party);
  const view: MusterView = {
    slotId,
    slot: listed.slot,
    savedAt: slotId === "data009Slot" && later ? LATER_SAVED_AT() : listed.savedAt,
    names: scene.namesMissing ? "missing" : "game",
    namesNote: scene.namesMissing ? MOCK_NAMES_NOTE : null,
    ...party,
  };
  return scene.namesMissing ? stripNames(view) : view;
}

/** R17.31's dev hook: Slot 9 one level later. */
function applyLater(party: ReturnType<typeof slotNine>): void {
  const arisen = party.arisen;
  if (!arisen) return;
  arisen.level = 42;
  arisen.experience = 1_306_900;
  arisen.gains.health = 1_262;
  const ring = arisen.gear.find((g) => g.slot === "ring1");
  if (ring) ring.item = { ...RING_OF_EMBERS };
  party.gold = 291_040;
}
