// THE FRAY (#00015 §19): the mock feed's every scene satisfies the frame's
// invariants (src/types/fray.ts), the window's readings sort and remainder by
// §19.9's rules, the 4 s beat holds its reading, and the window mounts every
// scene from the frame alone. Every name is invented (§19.14).

import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import type { FrayFight, FrayFrame, FrayMember } from "@/types/fray";
import { DUR7, FRAY_SCENES, frayMockFrame, frayMockView } from "@/services/mock/fray";
import { setFraySource } from "@/services/fray";
import {
  blockOf,
  freshBeat,
  leadingSkills,
  minutesAgoOf,
  perSecond,
  remainder,
  share,
  startedAgoS,
  takeBeat,
  trustLines,
  verdictOf,
} from "@/fray/reading";
import { PULL_STALE_S, freshened, modeOf, placesOf, sleepOf } from "@/fray/stage";
import FrayApp from "@/fray/FrayApp.vue";

const sum = (xs: number[]): number => xs.reduce((a, b) => a + b, 0);
const membersOf = (f: FrayFight): FrayMember[] => f.members.filter((m): m is FrayMember => m !== null);

/** Every fight a frame carries: the stele's and the shelf's. */
const fightsOf = (frame: FrayFrame): FrayFight[] => [...(frame.fight ? [frame.fight] : []), ...frame.recent];

/** Every frame the mock can hand the window: each scene, and fraysettle across its play. */
function everyFrame(): [string, FrayFrame][] {
  const out: [string, FrayFrame][] = FRAY_SCENES.map((s) => [s, frayMockFrame(s)]);
  for (let e = 0; e <= 8; e++) out.push([`fraysettle +${e}s`, frayMockFrame("fraysettle", e)]);
  return out;
}

function fightBroken(f: FrayFight): string[] {
  const bad: string[] = [];
  const ms = membersOf(f);
  if (f.members.length !== 4) bad.push(`${f.id}: ${f.members.length} places`);
  if (f.hParty !== sum(ms.map((m) => m.damage))) bad.push(`${f.id}: hParty ${f.hParty}`);
  if (f.hAll !== f.hParty + f.unattributed.damage + (f.world?.damage ?? 0)) bad.push(`${f.id}: hAll ${f.hAll}`);
  if (!f.partyFormed && f.hParty !== 0) bad.push(`${f.id}: credited with no party formed`);
  for (const m of ms) {
    if (m.parts !== null && sum(m.parts) !== m.damage)
      bad.push(`${f.id} ${m.name}: parts ${sum(m.parts)} ≠ ${m.damage}`);
    if (sum(m.skills.map((k) => k.damage)) !== m.damage) bad.push(`${f.id} ${m.name}: skills ≠ damage`);
    if (sum(m.skills.map((k) => k.hits)) !== m.hits) bad.push(`${f.id} ${m.name}: skill hits ≠ hits`);
    if (m.dps !== null && Math.abs(m.dps * f.durationS - m.damage) > 1e-6) bad.push(`${f.id} ${m.name}: dps`);
    if (new Set(m.skills.map((k) => k.id)).size !== m.skills.length) bad.push(`${f.id} ${m.name}: a key twice`);
  }
  if (f.types !== "additive" && ms.some((m) => m.parts !== null)) bad.push(`${f.id}: parts while not additive`);
  if (f.state === "settled" && f.endedAt === null) bad.push(`${f.id}: settled with no end`);
  return bad;
}

const sorted = (s: number[]): boolean => s.every((v, i) => i === 0 || s[i - 1] >= v);
const at = (t: number, base: FrayFight): FrayFight => ({ ...base, durationS: t });
function line(scene: string, i: 0 | 1): string {
  const f = frayMockFrame(scene).fight;
  if (!f) throw new Error(`no fight in ${scene}`);
  const pieces = trustLines(f)[i];
  return pieces.map((p) => p.text).join("");
}

const kind = (s: string): string => {
  const m = modeOf(frayMockFrame(s));
  return m.kind === "sleep" ? `${m.sleep}${m.kept ? "+kept" : ""}` : m.kind;
};
const odalric = (): FrayMember => {
  const m = frayMockFrame("fraysettled").fight?.members[0];
  if (!m) throw new Error("no Odalric");
  return m;
};

describe("the mock fray's frames", () => {
  it("should hold every frame invariant in every scene", () => {
    const bad = everyFrame().flatMap(([scene, frame]) =>
      fightsOf(frame).flatMap((f) => fightBroken(f).map((b) => `${scene}: ${b}`)),
    );
    expect(bad).toStrictEqual([]);
  });

  it("should keep the shelf to five settled fights, newest first", () => {
    for (const [, frame] of everyFrame()) {
      expect(frame.recent.length).toBeLessThanOrEqual(5);
      expect(frame.recent.every((f) => f.state === "settled")).toBe(true);
      expect(frame.recent.map((f) => f.n)).toStrictEqual(frame.recent.map((f) => f.n).sort((a, b) => b - a));
    }
  });

  it("should seed FIGHT 7 to §19.14's table: 38,540 for the party at 1:42", () => {
    const f = frayMockFrame("fraysettled").fight;
    expect(f?.durationS).toBe(DUR7);
    expect(f?.hParty).toBe(38540);
    expect(f?.unattributed.damage).toBe(2440);
    expect(f?.ledger).toBe(38770);
    expect(f?.kills.map((k) => k.slot)).toStrictEqual([0, 1, 2]);
  });

  it("should leave the party and its ledger alone when the world deals damage (FR-D1)", () => {
    const plain = frayMockFrame("fray").fight;
    const world = frayMockFrame("frayworldledger").fight;
    expect(world?.hParty).toBe(plain?.hParty);
    expect(world?.ledger).toBe(plain?.ledger);
    expect(world?.hAll).toBe((plain?.hAll ?? 0) + 4000);
  });

  it("should stop crediting a member who left, and keep what it dealt", () => {
    const at = frayMockFrame("frayleft").fight;
    const bramble = at?.members[2];
    expect(bramble?.left).toBe(true);
    expect(bramble?.damage).toBeGreaterThan(0);
    expect(bramble?.damage).toBeLessThan(frayMockFrame("fraydown").fight?.members[2]?.damage ?? 0);
  });

  it("should play fraysettle from 1:36 to its final heartbeat at 1:42, then settle", () => {
    const states = [0, 3, 6, 7].map((e) => frayMockFrame("fraysettle", e).fight);
    expect(states.map((f) => f?.durationS)).toStrictEqual([96, 99, 102, 102]);
    expect(states.map((f) => f?.state)).toStrictEqual(["open", "open", "open", "settled"]);
  });

  it("should stand in the screen each scene names", () => {
    expect(kind("fray")).toBe("live");
    expect(kind("fraysettled")).toBe("record");
    expect(kind("frayidle")).toBe("idle");
    expect(kind("frayasleep")).toBe("asleep+kept");
    expect(kind("frayasleepfresh")).toBe("asleep");
    expect(kind("fraynorefw")).toBe("norefw+kept");
    expect(kind("fraynoeye")).toBe("noeye+kept");
    expect(kind("frayshut")).toBe("shut+kept");
    expect(kind("fraysilent")).toBe("silent");
    expect(modeOf(frayMockFrame("fraystale"))).toMatchObject({ kind: "live", stale: true });
  });
});

describe("the skill block's readings (§19.9)", () => {
  it("should lead by damage, a tie going to the key that hit first, and never lead with Unknown", () => {
    const m = odalric();
    const tied = {
      ...m,
      skills: [
        { ...m.skills[0], id: "late", damage: 500, firstAt: 9 },
        { ...m.skills[0], id: "early", damage: 500, firstAt: 2 },
        {
          kind: "unknown" as const,
          id: "unknown",
          name: null,
          damage: 900,
          hits: 3,
          biggest: 400,
          lastBlow: false,
          firstAt: 1,
        },
      ],
      damage: 1900,
    };
    expect(leadingSkills(tied, 3).map((k) => k.id)).toStrictEqual(["early", "late"]);
    expect(leadingSkills(m, 2).map((k) => k.name)).toStrictEqual(["Riven Arc", "Cinder Rush"]);
  });

  it("should put everything the rows leave into Other and Unknown, as shares of the member", () => {
    const m = odalric();
    const lead = leadingSkills(m, 2);
    const r = remainder(m, lead);
    expect(r.unknown).toBe(share(630, m.damage));
    expect(r.other).toBe(share(2800 + 1090, m.damage));
    expect(share(lead[0].damage + lead[1].damage, m.damage) + r.other + r.unknown).toBeCloseTo(1, 10);
  });

  it("should let the support row give way live once two damaging keys stand, and keep it in the record (P3.6)", () => {
    const sable = frayMockFrame("fraysupporttwo").fight?.members[3];
    if (!sable) throw new Error("no Sable");
    expect(blockOf(sable, 3).support).toBeNull();
    expect(blockOf(sable, 3).lead.map((k) => k.label)).toStrictEqual(["Frost Thread", "Sleetquill"]);
    expect(blockOf(sable, 4).support).toStrictEqual([{ name: "Mending Light", count: 7 }]);
    const lone = frayMockFrame("fraysupportunknown").fight?.members[3];
    if (!lone) throw new Error("no Sable");
    expect(blockOf(lone, 3).support).not.toBeNull();
    expect(blockOf(lone, 3).rem.unknown).toBeCloseTo(141 / 707, 10);
  });

  it("should sort every member's block on every beat of FIGHT 7, and catch a planted inversion", () => {
    expect(sorted([0.49, 0.51])).toBe(false);
    for (let e = 0; e <= 6; e++) {
      const f = frayMockFrame("fraysettle", e).fight;
      for (const m of f ? membersOf(f) : []) expect(sorted(blockOf(m, 3).lead.map((k) => k.s))).toBe(true);
    }
  });

  it("should read 'No damaging hit' only at zero damage", () => {
    const healer = frayMockFrame("frayhealer").fight?.members[3];
    if (!healer) throw new Error("no healer");
    expect(blockOf(healer, 3)).toMatchObject({ zero: true, lead: [] });
    expect(blockOf(healer, 3).support?.map((s) => s.name)).toStrictEqual(["Mending Light", "Hearthsong"]);
  });
});

describe("the 4 s beat", () => {
  it("should hold one reading inside a beat and take a new one when the beat turns", () => {
    const f = frayMockFrame("fray").fight;
    if (!f) throw new Error("no fight");
    const first = takeBeat(freshBeat(), at(96, f));
    expect(takeBeat(first, at(99, f))).toBe(first);
    expect(takeBeat(first, at(100, f))).not.toBe(first);
  });

  it("should force a beat on a member's first damaging hit and at the settle", () => {
    const f = frayMockFrame("frayhealer").fight;
    if (!f) throw new Error("no fight");
    const st = takeBeat(freshBeat(), f);
    const sable = f.members[3];
    if (!sable) throw new Error("no Sable");
    const woke: FrayFight = {
      ...f,
      durationS: f.durationS + 1,
      members: f.members.map((m, i) => (i === 3 ? { ...sable, damage: 10 } : m)),
    };
    expect(takeBeat(st, woke)).not.toBe(st);
    expect(takeBeat(st, { ...f, durationS: f.durationS + 1, state: "settled" })).not.toBe(st);
  });
});

describe("the verdict and the trust band", () => {
  it("should fail closed: no verdict, or a word the contract lacks, reads unknown; proportions is mix", () => {
    expect([undefined, null, "partial", "proportions", "mix", "additive"].map((w) => verdictOf(w))).toStrictEqual([
      "unknown",
      "unknown",
      "unknown",
      "mix",
      "mix",
      "additive",
    ]);
  });

  it("should compare the party with the game's own count, and say when the stone falls short", () => {
    expect(line("fray", 0)).toMatch(/^The stone credits the party [\d,]+ · the game counts [\d,]+ · \d+\.\d%$/);
    expect(line("fray", 1)).toBe("Damage only: healing and help aren't counted");
    expect(line("frayshort", 0)).toContain(" · 88.0%");
    expect(line("frayshort", 1)).toBe("the stone missed some hits");
    expect(line("frayover", 1)).toBe("the stone may count some hits twice");
  });

  it("should print no percentage of a count that did not answer or never moved", () => {
    expect(line("fraynoledger", 0)).toMatch(/ · the game's own count didn't answer$/);
    expect(line("frayunmoved", 0)).toMatch(/ · the game counts 0$/);
    expect(line("frayunmoved", 1)).toBe("the game's count never moved");
    expect(line("fraygap", 1)).toBe("the stone missed some hits · the feed skipped 3 records");
  });
});

// ---------- the window ----------

const mounted: VueWrapper[] = [];
afterEach(() => {
  for (const w of mounted.splice(0)) w.unmount();
  setFraySource(null);
  vi.useRealTimers();
});

async function mountOn(pull: () => FrayFrame, scene = ""): Promise<VueWrapper> {
  setFraySource({ pull: () => Promise.resolve(pull()), view: frayMockView(scene) });
  const w = mount(FrayApp, { attachTo: document.body });
  mounted.push(w);
  await flushPromises();
  return w;
}

describe("the window", () => {
  it.each(everyFrame())("should draw %s from the frame alone", async (scene, frame) => {
    const w = await mountOn(() => frame, scene.split(" ")[0]);
    expect(w.find("main.stage").exists()).toBe(true);
    expect(w.text().length).toBeGreaterThan(40);
    expect(w.text()).not.toMatch(/undefined|NaN|null/);
  });

  it("should draw the live fight with its words apart", async () => {
    const w = await mountOn(() => frayMockFrame("fray"), "fray");
    const text = w.text();
    expect(text).toContain("THE FRAY");
    expect(text).toContain("Thornjaw ×3");
    expect(text).toContain("Arisen · Warrior");
    expect(text).toContain("Unattributed · no one's credit");
    expect(text).toContain("Hired pawn · Mage");
    expect(text).toContain("Used without damage: Mending Light ×7");
    expect(w.find("main.stage").attributes("inert")).toBeDefined();
    expect(w.findAll(".cards > .card")).toHaveLength(4);
  });

  it("should open a member's whole table in the record, and close it", async () => {
    const w = await mountOn(() => frayMockFrame("fraysettled"), "fraysettled");
    expect(w.text()).toContain("SET IN STONE");
    await w.find('.card[data-slot="0"]').trigger("click");
    expect(w.find(".card.open").text()).toContain("Odalric");
    expect(w.findAll(".card.strip")).toHaveLength(3);
    await w.find("button.close").trigger("click");
    expect(w.find(".card.open").exists()).toBe(false);
  });

  it("should browse the shelf with Older and Newer", async () => {
    const w = await mountOn(() => frayMockFrame("frayshelf"), "frayshelf");
    expect(w.text()).toContain("FIGHT 5 · SET IN STONE");
    await w.find('button[data-act="older"]').trigger("click");
    expect(w.text()).toContain("FIGHT 4 · SET IN STONE");
    await w.find('button[data-act="newer"]').trigger("click");
    await w.find('button[data-act="newer"]').trigger("click");
    expect(w.text()).toContain("FIGHT 6 · SET IN STONE");
  });

  it("should settle FIGHT 7 on the same vein, carved, and say so once", async () => {
    vi.useFakeTimers();
    const frames = [frayMockFrame("fraysettle", 6), frayMockFrame("fraysettle", 7)];
    let i = 0;
    const w = await mountOn(() => frames[Math.min(i++, frames.length - 1)], "fraysettle");
    const vein = w.find(".vein").element;
    expect(vein.classList.contains("carved")).toBe(false);
    await vi.advanceTimersByTimeAsync(260);
    await flushPromises();
    expect(w.find(".vein").element).toBe(vein);
    expect(vein.classList.contains("carved")).toBe(true);
    expect(w.find(".sr").text()).toBe("Fight settled: 1 minute 42, the party dealt 38,540, 378 per second.");
  });
});

// ---------- the roster, the settle stamp and the game's start (contract v0.6 additions) ----------

const hhmm = (ms: number): string => {
  const d = new Date(ms);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
};

describe("the frame's roster and stamps", () => {
  it("should stamp endedAtMs exactly when a fight is settled, at the same minute as endedAt", () => {
    const bad = everyFrame().flatMap(([scene, frame]) =>
      fightsOf(frame)
        .filter(
          (f) =>
            (f.state === "settled") !== (f.endedAtMs !== null) ||
            (f.endedAtMs !== null && hhmm(f.endedAtMs) !== f.endedAt),
        )
        .map((f) => `${scene}: ${f.id} ${f.endedAt} / ${f.endedAtMs}`),
    );
    expect(bad).toStrictEqual([]);
  });

  it("should know when the game started exactly while it runs", () => {
    for (const [, frame] of everyFrame()) expect(frame.feed.gameStartedMs === null).toBe(!frame.feed.gameRunning);
    const now = new Date(2026, 9, 26, 21, 0, 0);
    expect(frayMockFrame("fraysilent", 0, now).feed.gameStartedMs).toBe(now.getTime() - 40_000);
    expect(startedAgoS(now.getTime() - 40_000, now)).toBe(40);
    expect(startedAgoS(null, now)).toBeNull();
  });

  it("should carry the §19.14 party outside a fight: four places, no damage, no skills", () => {
    for (const [, frame] of everyFrame()) {
      expect(frame.party.map((m) => m?.name ?? null)).toStrictEqual(["Odalric", "Wren", "Bramble", "Sable"]);
      expect(frame.party.map((m) => m?.slot)).toStrictEqual([0, 1, 2, 3]);
      expect(frame.party.filter((m) => m?.damage === 0 && m.skills.length === 0 && m.hits === 0)).toHaveLength(4);
    }
    expect(frayMockFrame("frayidle").party[0]?.hp).not.toBeNull();
    expect(frayMockFrame("frayasleepfresh").party[0]?.hp).toBeNull();
  });

  it("should stand the eye's roster in the places, and fall back to the shelf, then the roles, when it is empty", () => {
    const frame = frayMockFrame("frayidle");
    expect(placesOf(frame)).toBe(frame.party);
    const kept = frayMockFrame("fraysettled");
    expect(placesOf({ ...kept, party: [] })).toBe(kept.recent[0].members);
    const bare = placesOf({ ...frame, party: [], recent: [] });
    expect(bare.map((m) => m?.role)).toStrictEqual(["arisen", "mainPawn", "hiredPawn", "hiredPawn"]);
    expect(bare.every((m) => m?.name === null)).toBe(true);
  });

  it("should count minutes since the settle from endedAtMs, across midnight and past a day, and from HH:MM only without it", () => {
    const now = new Date(2026, 9, 26, 0, 30, 0);
    const dayAndTwoHours = now.getTime() - 26 * 3600_000;
    expect(minutesAgoOf({ endedAt: hhmm(dayAndTwoHours), endedAtMs: dayAndTwoHours }, now)).toBe(26 * 60);
    expect(minutesAgoOf({ endedAt: "22:30", endedAtMs: null }, now)).toBe(120);
    expect(minutesAgoOf(null, now)).toBeNull();
  });
});

describe("the window's sleeping and resting screens", () => {
  it("should name the resting party from the eye's roster (S5)", async () => {
    const w = await mountOn(() => frayMockFrame("frayidle"), "frayidle");
    expect(w.text()).toContain("THE PARTY RESTS");
    expect(w.text()).toContain("Odalric");
    expect(w.text()).toContain("Hired pawn · Sorcerer");
  });

  it("should fall back to the places by role before the eye's first roster", async () => {
    const w = await mountOn(() => ({ ...frayMockFrame("frayidle"), party: [] }), "frayidle");
    expect(w.text()).toContain("The Arisen");
    expect(w.text()).not.toContain("Odalric");
  });

  it("should tell the kept fight's age from its settle stamp (S6)", async () => {
    const w = await mountOn(() => frayMockFrame("frayasleep"), "frayasleep");
    expect(w.find(".shead .kept").text()).toBe("FIGHT 7 · 2 h ago");
  });

  it("should say how long ago the game started when the eye stays silent", async () => {
    const w = await mountOn(() => frayMockFrame("fraysilent"), "fraysilent");
    expect(w.find(".shead .body").text()).toBe(
      "The game started 40 s ago and the eye hasn't spoken. If the game just updated, the eye may not fit this version yet.",
    );
    expect(w.text()).toContain("Odalric");
  });

  it("should keep the plain silent copy when the start is unknown", async () => {
    const frame = frayMockFrame("fraysilent");
    const w = await mountOn(() => ({ ...frame, feed: { ...frame.feed, gameStartedMs: null } }), "fraysilent");
    expect(w.find(".shead .body").text()).toBe(
      "The game is running and the eye hasn't spoken. If the game just updated, the eye may not fit this version yet.",
    );
  });
});

// ---------- PR #3's review (the Heretic, at f0769ac): what a frame may not claim ----------

describe("what the window may not claim", () => {
  it("should call a live frame stale once pulls stop answering for the spine's own 3 s", async () => {
    vi.useFakeTimers();
    let pulls = 0;
    const frame = frayMockFrame("fray");
    setFraySource({
      pull: () => (pulls++ === 0 ? Promise.resolve(frame) : Promise.reject(new Error("the spine is gone"))),
      view: frayMockView("fray"),
    });
    const w = mount(FrayApp, { attachTo: document.body });
    mounted.push(w);
    await flushPromises();
    expect(w.find(".legend .say.stl").exists()).toBe(false);
    await vi.advanceTimersByTimeAsync(PULL_STALE_S * 1000 + 300);
    await flushPromises();
    expect(pulls).toBeGreaterThan(10);
    expect(w.find(".legend .say.stl").text()).toContain("They are not live.");
  });

  it("should call a live frame stale when a pull never returns at all", async () => {
    vi.useFakeTimers();
    let pulls = 0;
    const frame = frayMockFrame("fray");
    setFraySource({
      pull: () => (pulls++ === 0 ? Promise.resolve(frame) : new Promise<FrayFrame>(() => undefined)),
      view: frayMockView("fray"),
    });
    const w = mount(FrayApp, { attachTo: document.body });
    mounted.push(w);
    await flushPromises();
    await vi.advanceTimersByTimeAsync(PULL_STALE_S * 1000 + 600);
    await flushPromises();
    expect(pulls).toBe(2);
    expect(w.find(".legend .say.stl").text()).toContain("They are not live.");
  });

  it("should leave a frame alone while pulls answer, and never freshen a frame that is not live", () => {
    const live = frayMockFrame("fray");
    expect(freshened(live, 1_000, 1_000 + 2_900)).toBe(live);
    expect(freshened(live, 1_000, 1_000 + 3_000)?.feed.state).toBe("stale");
    const asleep = frayMockFrame("frayasleep");
    expect(asleep.feed.state).not.toBe("live");
    expect(freshened(asleep, 0, 60_000)).toBe(asleep);
  });

  it("should divide the party's head and its cards by the same duration, and print none for a fight of no length", async () => {
    expect(perSecond(100, 0.5)).toBe(200);
    expect(perSecond(100, 0)).toBeNull();
    const base = frayMockFrame("fraysettled");
    const fight = base.fight;
    if (!fight) throw new Error("no fight");
    const flat = {
      ...fight,
      durationS: 0,
      members: fight.members.map((m) => (m ? { ...m, dps: null } : m)),
    };
    const w = await mountOn(() => ({ ...base, fight: flat, recent: [flat, ...base.recent.slice(1)] }), "fraysettled");
    expect(w.find(".pdps").text()).toBe("—");
    expect(w.findAll(".card .dps").map((d) => d.text())).toStrictEqual(["—", "—", "—", "—"]);
    expect(w.text()).not.toMatch(/\b0\/s/);
  });

  it("should tell a finished recording from an eye that never started", async () => {
    const shut = frayMockFrame("frayshut");
    const done = { ...shut, feed: { ...shut.feed, eyeRecording: false, eyeReason: "stopped" } };
    expect(sleepOf(done.feed)).toBe("done");
    expect(sleepOf({ ...shut.feed, eyeRecording: false, eyeReason: "a hook found nothing" })).toBe("shut");
    const w = await mountOn(() => done, "frayshut");
    expect(w.text()).toContain("THE EYE HAS CLOSED");
    expect(w.text()).not.toMatch(/didn't start|hooked nothing/);
  });

  it("should keep the keyboard on the door pressed while it steps along the shelf", async () => {
    const w = await mountOn(() => frayMockFrame("frayshelf"), "frayshelf");
    for (const door of ["older", "newer"] as const) {
      const el = w.find<HTMLElement>(`button[data-act="${door}"]`).element;
      el.focus();
      el.click();
      await flushPromises();
      expect(document.activeElement?.getAttribute("data-act")).toBe(door);
    }
  });

  it("should give a browsed unchecked fight's legend the doors' words", async () => {
    const w = await mountOn(() => frayMockFrame("frayshelfapprox"), "frayshelfapprox");
    expect(w.find('button[data-act="older"]').text()).toBe("◀");
    expect(w.find('button[data-act="older"]').attributes("aria-label")).toBe("Older fight");
    expect(w.find(".legend .say").text()).toBe("≈ unchecked · % of these hits' parts, not of the damage");
    const plain = await mountOn(() => frayMockFrame("frayshelfmix"), "frayshelfmix");
    expect(plain.find('button[data-act="older"]').text()).toBe("◀ Older");
  });
});
