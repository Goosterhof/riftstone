// v0.4 — THE MUSTER's pure parts: the mock party holds only the invented
// names of #00015 R17.30 (the mock ships in the bundle's lazy chunk, so no
// game text may ride in it), THE RE-CARVING's diff (R17.25 step 2), the party
// mark's one formula (R17.12), the when-words (R17.7), and the Muster's copy
// never saying "we".

import { describe, expect, it } from "vitest";
import type { MusterView, Named, SaveReason } from "@/types/rift";
import {
  MOCK_PARTY_NAMES,
  MOCK_THING_NAMES,
  MOCK_VOCATIONS,
  mockMuster,
  mockMusterSlots,
  stripNames,
} from "@/services/mock/muster";
import { recarve, deltaText, wasWords, membersOf } from "@/services/recarving";
import {
  MUSTER_FOLDER_STATES,
  NAMES_NOTICE,
  READING_PARTY,
  GAINS_SCOPE,
  LISTING_SLOTS,
  partyMark,
  recarvedSentence,
  whenWords,
  creatorLead,
} from "@/services/muster";
import { voice, VOICE_CONTEXTS } from "@/services/voice";

/** R17.30, value for value: every game-thing name the mock may carry. */
const R17_30_THINGS = [
  "Ring of Embers",
  "Ashen Greatsword",
  "Warden's Barbute",
  "Lanternwright's Hauberk",
  "Pilgrim's Greaves",
  "Mantle of the Ninth Bell",
  "Ring of Low Tides",
  "Ring of the Still Hour",
  "Ironhide",
  "Long Stride",
  "Second Wind",
  "Riven Arc",
  "Cinder Rush",
  "Bulwark Turn",
  "Twin Fangs of Harrowmere",
  "Fieldmouse Hood",
  "Moth-Grey Jerkin",
  "Brambleweave Breeches",
  "Dusk Shawl",
  "Thimble Ring",
  "Light Step",
  "Keen Ear",
  "Shadow Feint",
  "Quick Cut",
  "Hook and Run",
  "Calm",
  "Healer",
  "Staff of the Ninth Ember",
  "Cowl of the Grey Choir",
  "Robe of Banked Coals",
  "Ashwalker's Wraps",
  "Ring of the Cold Hearth",
  "Deep Well",
  "Steady Hand",
  "Slow Burn",
  "Far Sight",
  "Ember Lance",
  "Ashfall",
  "Kindle Ward",
  "Bold",
  "Wand of Quiet Rain",
  "Lamplighter's Cap",
  "Wayfarer's Tunic",
  "Wayfarer's Leggings",
  "Mantle of Small Hours",
  "Ring of Lantern Moss",
  "Soft Tread",
  "Kind",
  "Scholar",
  "Mending Light",
  "Frost Thread",
];
const PLAIN_VOCATIONS = ["Fighter", "Archer", "Mage", "Thief", "Warrior", "Sorcerer"];

const SCENE = { folder: null, namesMissing: false } as const;

function party(later = false): MusterView {
  const view = mockMuster("data009Slot", SCENE, later);
  if (typeof view === "string") throw new Error(`Slot 9 would not open: ${view}`);
  return view;
}

/** Every Named in a value, wherever it sits. */
function namedIn(value: unknown, out: Named[] = []): Named[] {
  if (Array.isArray(value)) for (const v of value) namedIn(v, out);
  else if (value && typeof value === "object") {
    const o = value as Record<string, unknown>;
    if (typeof o.id === "number" && "name" in o && Object.keys(o).length === 2) out.push(o as unknown as Named);
    else for (const v of Object.values(o)) namedIn(v, out);
  }
  return out;
}

describe("the mock party (R17.30)", () => {
  it("should hold only the invented people of R17.30", () => {
    const people = [
      ...membersOf(party()).map((m) => m.name),
      ...mockMusterSlots(SCENE).slots.map((s) => s.arisen?.name),
    ];
    const strangers = people.filter((name) => name && !(MOCK_PARTY_NAMES as readonly string[]).includes(name));
    expect(strangers).toStrictEqual([]);
    expect(new Set(membersOf(party()).map((m) => m.name))).toStrictEqual(new Set(MOCK_PARTY_NAMES));
  });

  it("should name things only with R17.30's invented names and the six plain vocations", () => {
    expect(MOCK_THING_NAMES).toHaveLength(R17_30_THINGS.length);
    expect(new Set(MOCK_THING_NAMES)).toStrictEqual(new Set(R17_30_THINGS));
    expect(new Set(Object.keys(MOCK_VOCATIONS))).toStrictEqual(new Set(PLAIN_VOCATIONS));
    const allowed = new Set([...R17_30_THINGS, ...PLAIN_VOCATIONS]);
    const named = [party(), party(true)].flatMap((view) => namedIn(view).map((n) => n.name));
    expect(named.filter((name) => name !== null && !allowed.has(name))).toStrictEqual([]);
  });

  it("should keep Wren a Healer, Bramble without a specialization, and Sable's first ring nameless", () => {
    const view = party();
    expect(view.mainPawn?.pawn?.specialization?.name).toBe("Healer");
    expect(view.hired[0]?.pawn?.specialization).toBeNull();
    expect(view.hired[1]?.gear.find((g) => g.slot === "ring1")?.item).toStrictEqual({ id: 40917, name: null });
  });

  it("should take every name away when the names did not answer", () => {
    expect(namedIn(stripNames(party())).every((n) => n.name === null)).toBe(true);
  });
});

describe("THE RE-CARVING's diff (R17.25 step 2)", () => {
  it("should find the five values the dev hook's save changed", () => {
    const marks = recarve(party(), party(true));
    expect(marks.count).toBe(5);
    const arisen = marks.members.get("a41c09e2d7b35f18");
    expect(deltaText(arisen?.level)).toBe("▴1");
    expect(deltaText(arisen?.experience)).toBe("▴22,350");
    expect(deltaText(arisen?.gains.health)).toBe("▴22");
    expect(wasWords(arisen?.gear.get(6))).toBe("changed at the last save, was Ring of Low Tides");
    expect(deltaText(marks.gold)).toBe("▴6,730");
    expect(marks.members.size).toBe(1);
  });

  it("should find nothing when nothing changed", () => {
    expect(recarve(party(), party()).count).toBe(0);
  });

  it("should mark a pawn that joined as new, with no marks of its own", () => {
    const before = party();
    const after = party();
    after.hired[1] = { ...after.hired[1], memberKey: "0000000000000001", level: 60 };
    const marks = recarve(before, after);
    expect(marks.joined).toStrictEqual(new Set(["0000000000000001"]));
    expect(marks.count).toBe(0);
  });

  it("should read a fall as ▾ and an emptied slot as was empty", () => {
    const before = party();
    const after = party();
    if (!after.arisen || !before.arisen) throw new Error("No Arisen");
    after.arisen.gains.weight = 4;
    before.arisen.gear[1].item = { id: 1, name: "Bench-Mock Shield" };
    const marks = recarve(before, after).members.get(after.arisen.memberKey);
    expect(deltaText(marks?.gains.weight)).toBe("▾2");
    expect(wasWords(marks?.gear.get(1))).toBe("changed at the last save, was Bench-Mock Shield");
    const back = recarve(after, before).members.get(after.arisen.memberKey);
    expect(wasWords(back?.gear.get(1))).toBe("changed at the last save, was empty");
  });
});

describe("THE MUSTER's small rules", () => {
  it("should scale a party mark to the party's highest, 0 when that is 0", () => {
    expect(partyMark(1_240, 1_240)).toBe(1);
    expect(partyMark(620, 1_240)).toBe(0.5);
    expect(partyMark(0, 0)).toBe(0);
  });

  it("should say when in the window's own words", () => {
    const now = new Date(2026, 9, 18, 22, 0).getTime();
    expect(whenWords(new Date(2026, 9, 18, 21, 4).toISOString(), now)).toBe("today 21:04");
    expect(whenWords(new Date(2026, 9, 17, 20, 12).toISOString(), now)).toBe("yesterday 20:12");
    expect(whenWords(new Date(2026, 9, 16, 20, 12).toISOString(), now)).toBe("16 Oct 2026, 20:12");
  });

  it("should never say we in any of its copy", () => {
    const reasons: SaveReason[] = ["otherAccount", "beingWritten", "damaged", "noParty", "gone"];
    const lines: string[] = [
      ...Object.values(MUSTER_FOLDER_STATES).flatMap((s) => [s.inscription, s.body]),
      NAMES_NOTICE.inscription,
      NAMES_NOTICE.line,
      READING_PARTY.body,
      GAINS_SCOPE,
      LISTING_SLOTS,
      creatorLead(),
      recarvedSentence(0),
      recarvedSentence(5),
    ];
    for (const reason of [...reasons, "odd"]) {
      const v = voice({ kind: "save", detail: { detail: "data009Slot: odd.", reason, file: "data009Slot" } }, "muster");
      lines.push(v.inscription ?? "", v.voice, v.empathy, ...v.recovery.map((r) => r.label));
    }
    for (const line of lines) expect(line).not.toMatch(/\bwe\b|\bwe'|\bour\b|\bus\b/i);
  });

  it("should voice a save failure by its reason, naming the file and never a path", () => {
    const v = voice(
      {
        kind: "save",
        detail: {
          detail: "C:\\Steam\\userdata\\x\\data003Slot.bin is damaged",
          reason: "damaged",
          file: "data003Slot",
        },
      },
      "muster",
    );
    expect(v.inscription).toBe("THIS SAVE WON'T OPEN");
    expect(v.voice).toBe("The stone couldn't read `data003Slot`: the file is damaged inside.");
    expect(v.recovery.map((r) => r.label)).toStrictEqual(["Show in folder", "Read again", "Copy details"]);
    expect(VOICE_CONTEXTS).toContain("muster");
  });
});
