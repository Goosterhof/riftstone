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

function ignore(): void {
  // Replaced before it is called.
}

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
    mock.controls.emitRawOutput({ runId: "run-mock-1", seq: n, data: b64("a") });
    mock.controls.emitRawOutput({ runId: "run-mock-1", seq: n + 2, data: b64("c") });
    expect(text(seen.slice(before))).toBe("a");
    mock.controls.emitRawOutput({ runId: "run-mock-1", seq: n + 1, data: b64("b") });
    // A replayed seq is written once.
    mock.controls.emitRawOutput({ runId: "run-mock-1", seq: n + 1, data: b64("b") });
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

describe("the Heretic's review of the room (PR #11)", () => {
  it("should write the run's first chunk, which the spine numbers 0", async () => {
    const w = await bench();
    // Summoned, before the mock's greeting lands: the spine's chunk 0 first.
    await w.summon(80, 24);
    const seen: Uint8Array[] = [];
    w.attach((b) => seen.push(b));
    mock.controls.emitRawOutput({ runId: "run-mock-1", seq: 0, data: b64("\x1b[6n") });
    expect(text(seen)).toBe("\x1b[6n");
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(text(seen)).toContain("Arisen");
  });

  it("should answer only the request the press was made on, when another took its place while the plan was read", async () => {
    const w = await bench();
    await summoned(w);
    const first = mock.controls.hold();
    let release: (plan: LaunchPlanView) => void = ignore;
    setRiftBackend({
      planLaunch: () =>
        new Promise<LaunchPlanView>((resolve) => {
          release = resolve;
        }),
    } as unknown as RiftBackend);
    const pressed = w.pressPlay();
    await flushPromises();
    // The first request is answered elsewhere, and the pawn asks again.
    await mock.answerPawnPlay("run-mock-1", first, false);
    const second = mock.controls.hold();
    expect(w.held.value?.heldRequestId).toBe(second);
    release({ launch: "steam", warnings: [], refused: null } as unknown as LaunchPlanView);
    await pressed;
    const answers = mock.controls.calls.filter((c) => c.cmd === "answer_pawn_play");
    expect(answers.map((c) => c.args)).toStrictEqual([["run-mock-1", first, false]]);
    expect(w.playRefusal.value).toMatch(/no longer the one waiting/);
    expect(w.held.value?.heldRequestId).toBe(second);
  });

  it("should keep the run on show when a late row from an earlier run arrives", async () => {
    const w = await bench();
    await summoned(w);
    await w.depart();
    await summoned(w);
    mock.controls.emitHand({ tool: "install", outcome: "done", summary: "this run", installId: "o2", refusal: null });
    mock.controls.emitRawHand({
      runId: "run-mock-1",
      seq: 99,
      at: "2026-10-28T21:00:00Z",
      tool: "install",
      outcome: "done",
      summary: "late",
      installId: "o1",
      refusal: null,
    });
    expect(w.runId.value).toBe("run-mock-2");
    expect(w.hands.value.map((h) => h.summary)).toStrictEqual(["this run"]);
  });

  /** The Heretic's WR6: the spine may publish a run's output before its state. */
  it("should show a run's output whether it arrives before or after the run's state, once each and in order", async () => {
    const w = await bench();
    const seen: Uint8Array[] = [];
    w.attach((b) => seen.push(b));
    const state = (run: string, s: "summoning" | "here") =>
      mock.controls.emitRawState({ runId: run, state: s, since: null, exitCode: null, host: null, held: null });
    // Output before state.
    mock.controls.emitRawOutput({ runId: "run-a", seq: 0, data: b64("FIRST") });
    expect(text(seen)).toBe("");
    state("run-a", "here");
    mock.controls.emitRawOutput({ runId: "run-a", seq: 1, data: b64("SECOND") });
    expect(text(seen)).toBe("FIRSTSECOND");
    expect(w.firstBytes.value).toBe(true);
    // State before output, for the next run; the last run's late bytes are dropped.
    state("run-b", "summoning");
    mock.controls.emitRawOutput({ runId: "run-a", seq: 2, data: b64("LATE") });
    mock.controls.emitRawOutput({ runId: "run-b", seq: 0, data: b64("THIRD") });
    expect(text(seen)).toBe("FIRSTSECONDTHIRD");
    expect(w.runId.value).toBe("run-b");
  });

  it("should send what the terminal answered while the run was summoned, once the run is here", async () => {
    const w = await bench();
    await w.summon(80, 24);
    expect(w.pawn.value?.state).toBe("summoning");
    // ConPTY's cursor query, and xterm's answer, before the stone can take keystrokes.
    mock.controls.emitRawOutput({ runId: "run-mock-1", seq: 0, data: b64("\x1b[6n") });
    await w.write("\x1b[1;1R");
    expect(mock.controls.calls.filter((c) => c.cmd === "pawn_write")).toStrictEqual([]);
    await new Promise((resolve) => setTimeout(resolve, 0));
    await flushPromises();
    expect(w.pawn.value?.state).toBe("here");
    const writes = mock.controls.calls.filter((c) => c.cmd === "pawn_write").map((c) => c.args);
    expect(writes).toStrictEqual([["run-mock-1", "\x1b[1;1R"]]);
  });

  it("should drop what a summoned run's terminal said when that run never arrives", async () => {
    const w = await bench();
    await w.summon(80, 24);
    await w.write("x");
    mock.controls.tear(3);
    await new Promise((resolve) => setTimeout(resolve, 0));
    await flushPromises();
    expect(mock.controls.calls.filter((c) => c.cmd === "pawn_write")).toStrictEqual([]);
  });

  /** The Heretic's WR8: a drain already sending survives no change of run. */
  it("should stop sending a summoned run's words the moment another run is on show, and name the run on each", async () => {
    const w = await bench();
    await w.summon(80, 24);
    await w.write("\x1b[1;1R");
    await w.write("old run input");
    const sent: unknown[][] = [];
    let release: () => void = ignore;
    setWorkshopBackend({
      ...mock,
      pawnWrite: (run: string, data: string) => {
        sent.push([run, data]);
        return new Promise<void>((resolve) => {
          release = resolve;
        });
      },
    });
    const state = (run: string, s: "summoning" | "here") =>
      mock.controls.emitRawState({ runId: run, state: s, since: null, exitCode: null, host: null, held: null });
    state("run-mock-1", "here");
    await flushPromises();
    expect(sent).toStrictEqual([["run-mock-1", "\x1b[1;1R"]]);
    state("run-b", "here");
    release();
    await flushPromises();
    expect(sent).toStrictEqual([["run-mock-1", "\x1b[1;1R"]]);
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
});
