// THE WORKSHOP's store (WINDOW-CONTRACT § v0.7): the PTY's bytes reach the
// terminal in order and none is dropped, the hands belong to one summoning, a
// held Play is answered only past §16.2's plan, and a bring-through press
// carries the digest the card froze. The room's look is specced elsewhere;
// these are the contract's promises the look stands on.

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { asRiftFailure, setRiftBackend, type RiftBackend } from "@/services/rift";
import type { LaunchPlanView } from "@/types/rift";
import { setWorkshopBackend } from "@/workshop/backend";
import { createMockWorkshop, type MockWorkshop, type WorkshopScenario } from "@/workshop/mock";
import { resetWorkshop, useWorkshop } from "@/workshop/useWorkshop";

const text = (chunks: Uint8Array[]): string => chunks.map((c) => new TextDecoder().decode(c)).join("");
const b64 = (s: string): string => btoa(s);

let mock: MockWorkshop;

async function bench(scenario: WorkshopScenario = "contained"): Promise<ReturnType<typeof useWorkshop>> {
  mock = createMockWorkshop({ scenario, beat: 0, script: false });
  setWorkshopBackend(mock);
  const w = useWorkshop();
  await w.subscribe();
  await w.load();
  return w;
}

/** Summon, and let the mock's beat land: the pawn is here and has greeted. A
 *  timer left pending would fire into the next spec's store. */
async function summoned(w: ReturnType<typeof useWorkshop>): Promise<void> {
  await w.summon(80, 24);
  await new Promise((resolve) => setTimeout(resolve, 0));
  await flushPromises();
}

/** A spine whose Play plan is `plan`; nothing else is called. */
function launchSpine(plan: LaunchPlanView): void {
  setRiftBackend({ planLaunch: () => Promise.resolve(plan) } as unknown as RiftBackend);
}

beforeEach(() => resetWorkshop());
afterEach(() => {
  setWorkshopBackend(null);
  setRiftBackend(null);
});

describe("the pawn's bytes", () => {
  it("should write every chunk in seq order, holding an early one until the gap fills", async () => {
    const w = await bench();
    await summoned(w);
    const seen: Uint8Array[] = [];
    w.attach((b) => seen.push(b));
    expect(text(seen)).toContain("Arisen");
    const before = seen.length;
    const n = mock.controls.nextOutputSeq();
    mock.controls.emitRawOutput({ seq: n, data: b64("a") });
    mock.controls.emitRawOutput({ seq: n + 2, data: b64("c") });
    expect(text(seen.slice(before))).toBe("a");
    mock.controls.emitRawOutput({ seq: n + 1, data: b64("b") });
    // A replayed seq is written once.
    mock.controls.emitRawOutput({ seq: n + 1, data: b64("b") });
    expect(text(seen.slice(before))).toBe("abc");
  });

  it("should keep the bytes that came before the terminal listened, and hand them over first", async () => {
    const w = await bench();
    await summoned(w);
    mock.controls.emitOutput("second");
    const seen: Uint8Array[] = [];
    w.attach((b) => seen.push(b));
    mock.controls.emitOutput("third");
    expect(text(seen)).toMatch(/Arisen[\s\S]*secondthird$/);
    expect(w.firstBytes.value).toBe(true);
  });
});

describe("the pawn's hands", () => {
  it("should list one summoning's hands newest first, once each", async () => {
    const w = await bench();
    await summoned(w);
    const a = mock.controls.emitHand({
      tool: "plan_install",
      outcome: "done",
      summary: "planned",
      installId: null,
      refusal: null,
    });
    mock.controls.emitHand({ tool: "install", outcome: "done", summary: "installed", installId: "o1", refusal: null });
    await flushPromises();
    expect(w.hands.value.map((h) => h.summary)).toStrictEqual(["installed", "planned"]);
    expect(w.hands.value.at(-1)?.seq).toBe(a.seq);
  });

  it("should start a fresh ledger when a new summoning begins", async () => {
    const w = await bench();
    await summoned(w);
    mock.controls.emitHand({ tool: "install", outcome: "done", summary: "first run", installId: "o1", refusal: null });
    await w.depart();
    await summoned(w);
    expect(w.hands.value).toStrictEqual([]);
    expect(w.runId.value).toBe("run-mock-2");
  });
});

describe("the held Play", () => {
  it("should leave the request waiting, with the plan's refusal shown, when Play is refused", async () => {
    const w = await bench();
    await summoned(w);
    mock.controls.hold();
    launchSpine({ launch: "steam", warnings: [], refused: "Dragon's Dogma 2 is already running." });
    await w.pressPlay();
    expect(w.playRefusal.value).toBe("Dragon's Dogma 2 is already running.");
    expect(w.held.value).not.toBeNull();
    expect(mock.controls.calls.some((c) => c.cmd === "answer_pawn_play")).toBe(false);
  });

  it("should hold the answer behind the warnings until Play anyway, and record nothing on a cancel", async () => {
    const w = await bench();
    await summoned(w);
    mock.controls.hold();
    launchSpine({
      launch: "steam",
      warnings: [{ kind: "reframeworkMissing" }],
      refused: null,
    } as unknown as LaunchPlanView);
    await w.pressPlay();
    expect(w.playWarnings.value).toHaveLength(1);
    w.cancelWarnings();
    expect(mock.controls.calls.some((c) => c.cmd === "answer_pawn_play")).toBe(false);
    await w.pressPlay();
    await w.confirmPlay();
    expect(w.playAnswered.value).toStrictEqual({ decision: "played", persisted: true });
    expect(w.held.value).toBeNull();
  });

  it("should carry the run and the request it was shown, and report a decision that did not persist", async () => {
    const w = await bench("unpersisted");
    await summoned(w);
    const id = mock.controls.hold();
    await w.declinePlay();
    const answered = mock.controls.calls.find((c) => c.cmd === "answer_pawn_play");
    expect(answered?.args).toStrictEqual(["run-mock-1", id, false]);
    expect(w.playAnswered.value).toStrictEqual({ decision: "declined", persisted: false });
  });
});

describe("bringing a build through", () => {
  it("should press with the digest the card froze, and refuse when the bytes changed since", async () => {
    const w = await bench();
    const plan = await w.planBring("Stride-Tuner-0.2.0.zip");
    if (!plan) throw new Error("the card did not open");
    mock.controls.rebuild("Stride-Tuner-0.2.0.zip");
    const outcome = await w.bring(plan);
    expect(outcome).toBeNull();
    expect(w.failure.value?.kind).toBe("bring");
    expect(w.failure.value?.detail.reason).toBe("bytesChanged");
    const pressed = mock.controls.calls.find((c) => c.cmd === "bring_through");
    expect(pressed?.args).toStrictEqual(["Stride-Tuner-0.2.0.zip", plan.sha256]);
  });
});

describe("the workshop's refusals", () => {
  it("should keep the pawn and bring kinds, with their reasons, across the IPC boundary", () => {
    const pawn = asRiftFailure({ kind: "pawn", detail: { detail: "No pawn is here.", reason: "notHere" } });
    expect(pawn.kind).toBe("pawn");
    expect(pawn.detail.reason).toBe("notHere");
    expect(asRiftFailure({ kind: "bring", detail: { detail: "x", reason: "nameTaken" } }).kind).toBe("bring");
  });

  it("should refuse a summon while the last probe breached, and summon nothing", async () => {
    const w = await bench("breached");
    await w.summon(80, 24);
    expect(w.failure.value?.detail.reason).toBe("breached");
    expect(w.pawn.value?.state).toBe("absent");
  });
});
