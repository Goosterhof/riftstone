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
import { EVENTS } from "@/types/rift";
import { RiftFailure, type RiftEvent } from "../rift";

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

/** v0.5: every invented thing name of §18.15's second party and of Thistle, kept apart from
 *  R17.30's list so each spec can hold its own party to its own names. */
export const MOCK_SECOND_THING_NAMES: readonly string[] = [];

function secondThing(name: string): Named {
  (MOCK_SECOND_THING_NAMES as string[]).push(name);
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

/** v0.5: the mock's characters (`MusterSlot.character`), invented keys in the spine's shape (16
 *  lowercase hex digits). A real key is private; these are no one's. */
export const MOCK_CHARACTERS = {
  odalric: "5e0d1c2b3a49f871",
  /** #00015 §18.8: the second character, Aldith & Corvin. */
  aldith: "b7a3e90c41d5f268",
  /** `mustergone` (R18.27): the key a character carried whose saves are gone. No slot carries it. */
  gone: "0f1e2d3c4b5a6978",
} as const;

/** v0.5 (#00015 §18.15): the second character's people and the hired pawn who joins Odalric in
 *  Slot 8 under `musterfollow`. Invented, like the party of R17.30. */
export const MOCK_SECOND_NAMES = ["Aldith", "Corvin", "Thistle"] as const;

interface SlotSeed {
  slotId: string;
  slot: number;
  savedAt: () => string;
  /** When the game saved it again (the dev hooks), and the Arisen's level then. */
  savedAgainAt?: () => string;
  arisen: { name: string; level: number; vocation: VocationName; levelAgain?: number } | null;
  problem: MusterSlot["problem"];
  character: string | null;
  mainPawnName: string | null;
  /** v0.5: listed only in the two-character scenes (§18.15 musterchars). */
  second?: true;
}

const odalric = (level: number, levelAgain?: number): SlotSeed["arisen"] => ({
  name: "Odalric",
  level,
  vocation: "Warrior",
  levelAgain,
});
const aldith = (level: number, levelAgain?: number): SlotSeed["arisen"] => ({
  name: "Aldith",
  level,
  vocation: "Fighter",
  levelAgain,
});

/** Every slot of the mock, R17.30's four and §18.15's three more. */
const SEEDS: readonly SlotSeed[] = [
  {
    slotId: "data009Slot",
    slot: 9,
    savedAt: () => localAt(0, 21, 4),
    savedAgainAt: () => localAt(0, 21, 34),
    arisen: odalric(41, 42),
    problem: null,
    character: MOCK_CHARACTERS.odalric,
    mainPawnName: "Wren",
  },
  {
    slotId: "data008Slot",
    slot: 8,
    savedAt: () => localAt(1, 20, 12),
    savedAgainAt: () => localAt(0, 21, 44),
    arisen: odalric(40, 42),
    problem: null,
    character: MOCK_CHARACTERS.odalric,
    mainPawnName: "Wren",
  },
  {
    slotId: "data007Slot",
    slot: 7,
    savedAt: () => fixedLocal(10, 14, 22, 10),
    savedAgainAt: () => localAt(0, 21, 54),
    arisen: aldith(19, 20),
    problem: null,
    character: MOCK_CHARACTERS.aldith,
    mainPawnName: "Corvin",
    second: true,
  },
  {
    slotId: "data002Slot",
    slot: 2,
    savedAt: () => fixedLocal(10, 13, 18, 5),
    arisen: aldith(18),
    problem: null,
    character: MOCK_CHARACTERS.aldith,
    mainPawnName: "Corvin",
    second: true,
  },
  {
    slotId: "data003Slot",
    slot: 3,
    savedAt: () => fixedLocal(10, 12, 18, 40),
    arisen: null,
    problem: "damaged",
    character: null,
    mainPawnName: null,
  },
  {
    slotId: "data004Slot",
    slot: 4,
    savedAt: () => fixedLocal(10, 11, 19, 30),
    arisen: odalric(33),
    problem: null,
    character: MOCK_CHARACTERS.odalric,
    mainPawnName: "Wren",
    second: true,
  },
  {
    slotId: "data001Slot",
    slot: 1,
    savedAt: () => fixedLocal(10, 9, 11, 2),
    arisen: null,
    problem: "otherAccount",
    character: null,
    mainPawnName: null,
  },
];

/** What the game did since the mock woke (the dev hooks): the slots it saved again, and the
 *  slots it is writing right now (they answer `beingWritten`, character null, R18.19). */
export interface MusterSaves {
  saved: ReadonlySet<string>;
  writing: ReadonlySet<string>;
}

const NO_SAVES: MusterSaves = { saved: new Set(), writing: new Set() };

/** v0.4's `later` (Slot 9 saved once) or v0.5's whole record. */
function savesOf(later: boolean | MusterSaves): MusterSaves {
  if (typeof later !== "boolean") return later;
  return later ? { saved: new Set(["data009Slot"]), writing: new Set() } : NO_SAVES;
}

function slotOf(seed: SlotSeed, saves: MusterSaves): MusterSlot {
  // a slot the game is writing carries the new write's time already: it is the newest file
  const again = (saves.saved.has(seed.slotId) || saves.writing.has(seed.slotId)) && seed.savedAgainAt;
  const savedAt = again && seed.savedAgainAt ? seed.savedAgainAt() : seed.savedAt();
  const base = { slotId: seed.slotId, slot: seed.slot, savedAt, account: null };
  if (saves.writing.has(seed.slotId))
    return { ...base, arisen: null, problem: "beingWritten", character: null, mainPawnName: null };
  const a = seed.arisen;
  return {
    ...base,
    arisen: a
      ? { name: a.name, level: again && a.levelAgain ? a.levelAgain : a.level, vocation: voc(a.vocation) }
      : null,
    problem: seed.problem,
    character: seed.character,
    mainPawnName: seed.mainPawnName,
  };
}

/** The listed slots, newest first: R17.30's four, or with `characters` §18.15's seven. */
function slotList(characters = false, saves: MusterSaves = NO_SAVES): MusterSlot[] {
  return SEEDS.filter((seed) => characters || !seed.second)
    .map((seed) => slotOf(seed, saves))
    .sort((a, b) => Date.parse(b.savedAt) - Date.parse(a.savedAt));
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
  /** v0.5 (#00015 §18.15 musterchars): Aldith & Corvin in Slots 7 and 2, and Odalric's Slot 4. */
  characters?: boolean;
}

export function mockMusterSlots(scene: MusterScene, later: boolean | MusterSaves = false): MusterSlotsView {
  if (scene.folder) return { problem: scene.folder, slots: [], accountCount: 0, names: "game" };
  const slots = slotList(scene.characters, savesOf(later));
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
  beingWritten: (file: string) =>
    `The game is writing ${file} right now, or another program is holding it for a moment.`,
  gone: (file: string) => `${file} is no longer in the save folder.`,
  noParty: (file: string) => `${file} holds no party record.`,
} as const;

/** Aldith & Corvin (#00015 §18.15): an Arisen and a main pawn, no hired pawns. Every thing invented. */
function aldithParty(): Omit<MusterView, "slotId" | "slot" | "savedAt" | "names" | "namesNote"> {
  const arisen: MusterMember = {
    memberKey: "e14b7c2a09d3f586",
    role: "arisen",
    name: "Aldith",
    level: 19,
    experience: 214_600,
    vocation: voc("Fighter"),
    vocations: [
      { vocation: voc("Fighter"), experience: 168_200 },
      { vocation: voc("Mage"), experience: 22_400 },
    ],
    gains: gains(540, 380, 132, 128, 30, 44, 3, 70),
    gear: gear(
      secondThing("Hearthguard Blade"),
      secondThing("Tarnished Kite Shield"),
      null,
      secondThing("Drover's Brigandine"),
      secondThing("Drover's Boots"),
      null,
      secondThing("Ring of Small Mercies"),
      null,
    ),
    augments: [secondThing("Iron Grip"), secondThing("Stout Heart")],
    skills: [secondThing("Shield Bash Turn"), secondThing("Rising Edge")],
    pawn: null,
    creator: null,
  };
  const corvin: MusterMember = {
    memberKey: "73c0a5e9d1b82f44",
    role: "mainPawn",
    name: "Corvin",
    level: 18,
    experience: 198_300,
    vocation: voc("Archer"),
    vocations: [{ vocation: voc("Archer"), experience: 151_900 }],
    gains: gains(430, 520, 104, 86, 52, 60, 1, 30),
    gear: gear(
      secondThing("Rookwood Longbow"),
      null,
      secondThing("Feltcap of the Fens"),
      secondThing("Reedcloth Vest"),
      secondThing("Reedcloth Trews"),
      secondThing("Heron-Grey Cloak"),
      null,
      null,
    ),
    augments: [secondThing("Far Eye")],
    skills: [secondThing("Split Shaft"), secondThing("Low Volley")],
    pawn: { inclination: secondThing("Steady"), specialization: null },
    creator: null,
  };
  return { arisen, mainPawn: corvin, hired: [], gold: 18_220, riftCrystals: 940 };
}

const ALDITH = aldithParty();

/** `musterfollow`'s newcomer (R18.25): a hired pawn Slot 8 did not hold before. */
const THISTLE: MusterMember = {
  memberKey: "2d9e4f60a1c7b538",
  role: "hiredPawn",
  name: "Thistle",
  level: 44,
  experience: 1_712_000,
  vocation: voc("Sorcerer"),
  vocations: [{ vocation: voc("Sorcerer"), experience: 590_400 }],
  gains: gains(700, 500, 90, 110, 452, 380, 1, 36),
  gear: gear(
    secondThing("Staff of the Thornward"),
    null,
    secondThing("Hood of Burnt Briar"),
    secondThing("Robe of the Pale Thicket"),
    secondThing("Thornwalker's Wraps"),
    null,
    null,
    null,
  ),
  augments: [secondThing("Quiet Mind")],
  skills: [secondThing("Briar Bolt"), secondThing("Ash Bloom")],
  pawn: { inclination: secondThing("Wary"), specialization: secondThing("Guide") },
  creator: { level: 47, vocation: voc("Mage") },
};

type Party = ReturnType<typeof slotNine>;

/** Each slot's party as the game last wrote it (`again`: since the mock woke). */
function partyOf(slotId: string, again: boolean): Party {
  if (slotId === "data007Slot" || slotId === "data002Slot") {
    const party = structuredClone(ALDITH);
    if (party.arisen && slotId === "data002Slot") {
      party.arisen.level = 18;
      party.arisen.experience = 181_000;
    }
    if (party.arisen && slotId === "data007Slot" && again) {
      party.arisen.level = 20;
      party.arisen.experience = 238_900;
    }
    return party;
  }
  const party = structuredClone(SLOT_NINE);
  if (slotId === "data008Slot" && party.arisen) {
    party.arisen.level = 40;
    party.arisen.experience = 1_250_100;
  }
  if (slotId === "data004Slot" && party.arisen) {
    party.arisen.level = 33;
    party.arisen.experience = 884_300;
  }
  if ((slotId === "data009Slot" || slotId === "data008Slot") && again) applyLater(party);
  // §18.15 musterfollow: Bramble left Slot 8's party, Thistle joined it
  if (slotId === "data008Slot" && again)
    party.hired = party.hired.map((m) => (m.memberKey === "c3d81e5a40f97b26" ? structuredClone(THISTLE) : m));
  return party;
}

/** One slot's party, or the reason it will not open. Slot 8 is the same party
 *  one level earlier (Odalric Lv 40). */
export function mockMuster(
  slotId: string,
  scene: MusterScene,
  later: boolean | MusterSaves,
): MusterView | keyof typeof MOCK_SAVE_SENTENCES {
  const saves = savesOf(later);
  const listed = slotList(scene.characters, saves).find((s) => s.slotId === slotId);
  if (!listed) return "gone";
  if (listed.problem) return listed.problem;
  const view: MusterView = {
    slotId,
    slot: listed.slot,
    savedAt: listed.savedAt,
    names: scene.namesMissing ? "missing" : "game",
    namesNote: scene.namesMissing ? MOCK_NAMES_NOTE : null,
    ...partyOf(slotId, saves.saved.has(slotId)),
  };
  return scene.namesMissing ? stripNames(view) : view;
}

/** R17.31's dev hook: Slot 9 one level later. */
function applyLater(party: Party): void {
  const arisen = party.arisen;
  if (!arisen) return;
  arisen.level = 42;
  arisen.experience = 1_306_900;
  arisen.gains.health = 1_262;
  const ring = arisen.gear.find((g) => g.slot === "ring1");
  if (ring) ring.item = { ...RING_OF_EMBERS };
  party.gold = 291_040;
}

// ---------------------------------------------------------------------------
// v0.5: the Muster's side of the mock backend, whole (its state, its three
// commands, its dev hooks, the remembered character it starts with), so the
// mock's index only wires it in.
// ---------------------------------------------------------------------------

export interface MusterMock {
  getMusterSlots(): Promise<MusterSlotsView>;
  getMuster(slotId: string): Promise<MusterView>;
  openSavesFolder(slotId: string): Promise<void>;
  /** v0.4: the game saves Slot 9 one level later (#00015 R17.31), and the save watcher fires. */
  gameSaved(): void;
  /** v0.5 (#00015 §18.15): the game saves `slotId` again, and the save watcher fires. Slot 9 and
   *  Slot 8 come back with Odalric at Lv 42 (Slot 8 with Thistle in Bramble's place, the
   *  `musterfollow` save), Slot 7 with Aldith at Lv 20, each newer than every slot before it.
   *  `writingFirst`: the slot list answers the slot `beingWritten` once, and the save settles
   *  900 ms later (the `musterwriting` autosave, R18.19 and Bold Choice 22). */
  saveSlot(slotId: string, writingFirst?: boolean): void;
  /** v0.5: `Settings.musterCharacter` as the mock's settings start (null: none picked). */
  rememberedAtStart(): string | null;
}

/** v0.5 (#00015 §18.15): the scenes that list two characters. */
const CHARACTER_SCENES = ["musterchars", "musterpinned", "musterfollow", "musterwriting", "mustergone"] as const;

/** The Muster's mock. `scene` is the `?mock=` set; `namesGone` says the game's text is out of
 *  reach (no game); `emit` fires a backend event; `wait` is the mock's latency.
 *
 *  v0.5 scenes (§18.15): `musterchars` two characters across five slots, Slot 3 damaged and
 *  Slot 1 another account's; `musterpinned` and `musterfollow` the same folder, their saves fired
 *  by `saveSlot("data009Slot")` (pin Slot 8 in the menu first) and `saveSlot("data008Slot")`;
 *  `musterwriting` v0.4's refusal grown into the same folder, with `saveSlot(id, true)` for a
 *  listing that answers `beingWritten` once; `mustergone` a remembered character no slot carries. */
export function createMusterMock(
  scene: ReadonlySet<string>,
  namesGone: () => boolean,
  emit: (event: RiftEvent, payload: unknown) => void,
  wait: (ms: number) => Promise<unknown>,
): MusterMock {
  // v0.4: one being-written refusal under musterwriting. v0.5: what the game saved since.
  let writingOnce = scene.has("musterwriting");
  const saved = new Set<string>();
  const writing = new Set<string>();
  const saves: MusterSaves = { saved, writing };
  function musterScene(): MusterScene {
    let folder: MusterScene["folder"] = null;
    if (scene.has("musternosteam")) folder = "noSteam";
    else if (scene.has("musternosaves")) folder = "noSaves";
    const characters = CHARACTER_SCENES.some((name) => scene.has(name));
    return { folder, namesMissing: scene.has("musternames") || namesGone(), characters };
  }
  function saveFailure(slotId: string, reason: keyof typeof MOCK_SAVE_SENTENCES): RiftFailure {
    return new RiftFailure({ kind: "save", detail: { detail: MOCK_SAVE_SENTENCES[reason](slotId), reason } });
  }
  function saveSlot(slotId: string, writingFirst = false): void {
    if (!writingFirst) {
      saved.add(slotId);
      emit(EVENTS.musterChanged, {});
      return;
    }
    writing.add(slotId);
    emit(EVENTS.musterChanged, {});
    setTimeout(() => {
      writing.delete(slotId);
      saveSlot(slotId);
    }, 900);
  }
  return {
    async getMusterSlots() {
      await wait(60);
      return mockMusterSlots(musterScene(), saves);
    },
    async getMuster(slotId) {
      await wait(scene.has("musterslow") ? 2000 : 120);
      if (writingOnce && slotId === "data009Slot") {
        writingOnce = false;
        // the save settles a moment later, and the watcher says so
        setTimeout(() => emit(EVENTS.musterChanged, {}), 900);
        throw saveFailure(slotId, "beingWritten");
      }
      const read = mockMuster(slotId, musterScene(), saves);
      if (typeof read === "string") throw saveFailure(slotId, read);
      return read;
    },
    async openSavesFolder(slotId) {
      await wait(40);
      if (!mockMusterSlots(musterScene(), saves).slots.some((s) => s.slotId === slotId))
        throw saveFailure(slotId, "gone");
    },
    gameSaved() {
      saveSlot("data009Slot");
    },
    saveSlot,
    rememberedAtStart() {
      return scene.has("mustergone") ? MOCK_CHARACTERS.gone : null;
    },
  };
}
