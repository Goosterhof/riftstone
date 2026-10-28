// THE WORKSHOP's mock: the fifteen commands and four events over an invented
// workshop, in memory, on real timers. It never fetches and never names a real
// mod. The scenario comes from the page's query string, `?workshop=<name>`:
//   contained (default) | unprobed | stale | breached | nofence | first |
//   departed | torn | unpersisted | nametaken
// - unprobed: no probe record; stale: a record proved on another claude.
// - breached: the last probe reached a place the fence should close; summoning refuses.
// - nofence: the host survey finds no socat in the distro; summoning refuses.
// - first: the workshop folder is not made yet and nothing is built.
// - departed / torn: the last summoning ended cleanly (exit 0) or not (exit 3),
//   leaving two of its installs in the world (the Gift's Put back).
// - unpersisted: a Play answer stands but does not reach the run's ledger.
// - nametaken: the mods folder holds a different file under the build's name.
// A summon runs a short scripted session: the greeting, then the pawn's hands
// (a plan, an install, a refusal, the log, and a held request to play).

import { RiftFailure } from "@/services/rift";
import type { UninstallPlanView, UninstallResultView } from "@/types/rift";
import {
  WORKSHOP_EVENTS,
  type ArtifactView,
  type BringThroughPlan,
  type Chip,
  type FenceStep,
  type FenceView,
  type PawnHand,
  type PawnOutput,
  type PawnStateView,
  type PutBackPlan,
  type Voice,
  type WorkshopEvent,
  type WorkshopView,
} from "@/types/workshop";
import type { WorkshopBackend } from "./backend";

export type WorkshopScenario =
  | "contained"
  | "unprobed"
  | "stale"
  | "breached"
  | "nofence"
  | "first"
  | "departed"
  | "torn"
  | "unpersisted"
  | "nametaken";

const SCENARIOS: readonly WorkshopScenario[] = [
  "contained",
  "unprobed",
  "stale",
  "breached",
  "nofence",
  "first",
  "departed",
  "torn",
  "unpersisted",
  "nametaken",
];

export interface MockWorkshopOptions {
  scenario?: WorkshopScenario;
  /** Milliseconds between the session's scripted beats; 0 in tests that drive time themselves. */
  beat?: number;
  /** Run the scripted session after a summon (the render harness); tests drive hands themselves. */
  script?: boolean;
}

/** The controls a spec uses to drive what the spine would push. */
export interface MockWorkshopControls {
  emitOutput(text: string): void;
  /** One `rift://pawn-output` payload exactly as given, its seq included (an out-of-order delivery). */
  emitRawOutput(out: PawnOutput): void;
  /** The seq the next emitOutput will carry. */
  nextOutputSeq(): number;
  emitHand(hand: Omit<PawnHand, "runId" | "seq" | "at"> & { at?: string }): PawnHand;
  hold(): string;
  /** A rebuild: the artifact's bytes, and so its digest, change on disk. */
  rebuild(name: string): void;
  tear(exitCode: number): void;
  calls: { cmd: string; args: unknown[] }[];
  view(): WorkshopView;
}

export type MockWorkshop = WorkshopBackend & { controls: MockWorkshopControls };

const WORKSHOP_PATH = "C:\\Games\\Dragons Dogma 2\\mods (workshop)";
const HOST = { distro: "Ubuntu-24.04", claudeVersion: "2.1.312" };
const GROUND = {
  distro: "Ubuntu-24.04",
  claudeVersion: "2.1.312",
  kernel: "6.6.87.2-microsoft-standard-WSL2",
  digest: "4be1c07a9d2e51f3a8b6c0d4e7f29a1b3c5d7e9f0a2b4c6d8e0f1a3b5c7d9e1f",
};

const pawnRefused = (reason: string, detail: string) => new RiftFailure({ kind: "pawn", detail: { detail, reason } });
const bringRefused = (reason: string, detail: string) => new RiftFailure({ kind: "bring", detail: { detail, reason } });

function now(): string {
  return new Date().toISOString().replace(/\.\d{3}Z$/, "Z");
}

function scenarioFromPage(): WorkshopScenario {
  if (typeof window === "undefined") return "contained";
  const asked = new URLSearchParams(window.location.search).get("workshop");
  return SCENARIOS.find((s) => s === asked) ?? "contained";
}

function base64(text: string): string {
  let binary = "";
  for (const byte of new TextEncoder().encode(text)) binary += String.fromCharCode(byte);
  return btoa(binary);
}

function digestOf(seed: string): string {
  // An invented, stable 64-hex digest: not a hash of anything real.
  let h = 2166136261;
  let out = "";
  while (out.length < 64) {
    for (const c of seed + out.length) h = Math.imul(h ^ c.charCodeAt(0), 16777619) >>> 0;
    out += h.toString(16).padStart(8, "0");
  }
  return out.slice(0, 64);
}

const STEPS: { step: string; expected: FenceStep["expected"]; observed: string }[] = [
  { step: "control: write in the workshop", expected: "land", observed: "landed" },
  { step: "write into the mods folder", expected: "notLand", observed: "did not land: Directory nonexistent" },
  { step: "write into the game folder", expected: "notLand", observed: "did not land: Directory nonexistent" },
  { step: "list Steam's userdata", expected: "fail", observed: "refused: No such file or directory" },
  { step: "read the stone's canary", expected: "fail", observed: "refused: No such file or directory" },
  { step: "write into the investor's .claude", expected: "notLand", observed: "did not land: Directory nonexistent" },
  { step: "run a planted cmd.exe", expected: "notLand", observed: "did not land: not found" },
  { step: "run the real cmd.exe", expected: "notLand", observed: "did not land: not found" },
  { step: "claude --version", expected: "succeed", observed: "answered: 2.1.312 (Claude Code)" },
  { step: "read the canary through a junction", expected: "fail", observed: "refused: No such file or directory" },
  {
    step: "read the stone's secret from the environment",
    expected: "fail",
    observed: "the environment carried no secret",
  },
  { step: "link and write in answers/", expected: "fail", observed: "refused: Read-only file system" },
  { step: "inject keystrokes (TIOCSTI)", expected: "fail", observed: "refused: Operation not permitted" },
];

function fenceFor(chip: Chip, scenario: WorkshopScenario): FenceView {
  if (chip === "unprobed" && scenario !== "stale") {
    return { chip, ground: null, provedAt: null, steps: [], changed: null };
  }
  const steps = STEPS.map((s) => ({ ...s, ok: true }));
  if (chip === "breached") {
    steps[2] = { ...steps[2], observed: "landed: the file is on disk", ok: false };
  }
  return {
    chip,
    ground: scenario === "stale" ? { ...GROUND, claudeVersion: "2.1.298" } : GROUND,
    provedAt: "2026-10-28T21:02:00Z",
    steps,
    changed: scenario === "stale" ? "proved on claude 2.1.298; this is 2.1.312" : null,
  };
}

function initialChip(scenario: WorkshopScenario): Chip {
  if (scenario === "breached") return "breached";
  if (scenario === "unprobed" || scenario === "stale") return "unprobed";
  return "contained";
}

function initialArtifacts(scenario: WorkshopScenario): ArtifactView[] {
  if (scenario === "first") return [];
  const sha = digestOf("Stride Tuner 0.2.0");
  return [
    {
      name: "Stride-Tuner-0.2.0.zip",
      bytes: 42_188,
      sha256: sha,
      receipt: {
        artifact: "Stride-Tuner-0.2.0.zip",
        sha256: sha,
        builtAt: "2026-10-28T21:12:00Z",
        inputs: [{ path: "vanilla/re_chunk_000.pak.sub_000.pak", sha256: digestOf("vanilla pin") }],
        checks: [
          { name: "asset controls", passed: 12, total: 12 },
          { name: "Lua controls", passed: 8, total: 8 },
        ],
      },
      receiptProblem: null,
      inLibrary: false,
    },
    {
      name: "Stride-Tuner-0.1.0.zip",
      bytes: 40_921,
      sha256: digestOf("Stride Tuner 0.1.0"),
      receipt: null,
      receiptProblem: "missing",
      inLibrary: false,
    },
  ];
}

export function createMockWorkshop(options: MockWorkshopOptions = {}): MockWorkshop {
  const scenario = options.scenario ?? scenarioFromPage();
  const beat = options.beat ?? 700;
  const script = options.script ?? true;
  const calls: { cmd: string; args: unknown[] }[] = [];
  const listeners = new Map<string, Set<(payload: unknown) => void>>();
  const emit = (event: WorkshopEvent, payload: unknown) => {
    for (const l of listeners.get(event) ?? []) l(payload);
  };
  const call = (cmd: string, ...args: unknown[]) => calls.push({ cmd, args });

  let folderMade = scenario !== "first";
  let artifacts = initialArtifacts(scenario);
  let chip = initialChip(scenario);
  let fence = fenceFor(chip, scenario);
  let voice: Voice = "sparing";
  let inputs: string[] = [];
  let outSeq = 0;
  let handSeq = 0;
  let runCount = 0;
  // The journal keys an install by its option id, so the two are one id here.
  const pawnInstalls: { installId: string; name: string }[] = [];
  const timers: ReturnType<typeof setTimeout>[] = [];

  let pawn: PawnStateView = { runId: null, state: "absent", since: null, exitCode: null, host: null, held: null };
  if (scenario === "departed" || scenario === "torn") {
    pawn = {
      runId: "run-mock-0",
      state: scenario === "torn" ? "torn" : "departed",
      since: "2026-10-28T21:40:00Z",
      exitCode: scenario === "torn" ? 3 : 0,
      host: HOST,
      held: null,
    };
    pawnInstalls.push(
      { installId: "opt-stride-020", name: "Stride Tuner 0.2.0" },
      { installId: "opt-lantern-glow", name: "Lantern Glow Test" },
    );
  }

  const setPawn = (next: Partial<PawnStateView>) => {
    pawn = { ...pawn, ...next };
    emit(WORKSHOP_EVENTS.pawnState, pawn);
  };
  const later = (ms: number, fn: () => void) => {
    timers.push(setTimeout(fn, ms));
  };

  const controls: MockWorkshopControls = {
    calls,
    emitOutput(text) {
      outSeq += 1;
      emit(WORKSHOP_EVENTS.pawnOutput, { seq: outSeq, data: base64(text) } satisfies PawnOutput);
    },
    emitRawOutput(out) {
      outSeq = Math.max(outSeq, out.seq);
      emit(WORKSHOP_EVENTS.pawnOutput, out);
    },
    nextOutputSeq: () => outSeq + 1,
    emitHand(hand) {
      handSeq += 1;
      const row: PawnHand = { runId: pawn.runId ?? "run-mock", seq: handSeq, at: hand.at ?? now(), ...hand };
      if (row.tool === "install" && row.outcome === "done" && row.installId) {
        pawnInstalls.push({ installId: row.installId, name: row.summary });
      }
      emit(WORKSHOP_EVENTS.pawnHands, row);
      return row;
    },
    hold() {
      const id = `held-${handSeq + 1}`;
      controls.emitHand({
        tool: "play",
        outcome: "held",
        summary: "asks to start the game",
        installId: null,
        refusal: null,
      });
      setPawn({ held: { heldRequestId: id, since: now() } });
      return id;
    },
    rebuild(name) {
      artifacts = artifacts.map((a) => (a.name === name ? { ...a, sha256: digestOf(`${name} rebuilt ${now()}`) } : a));
      emit(WORKSHOP_EVENTS.workshopChanged, {});
    },
    tear(exitCode) {
      setPawn({ state: "torn", exitCode, held: null });
    },
    view,
  };

  function view(): WorkshopView {
    return {
      folder: { made: folderMade, path: WORKSHOP_PATH },
      artifacts: artifacts.map((a) => ({ ...a })),
      pawn: { ...pawn },
      fence: { ...fence, steps: fence.steps.map((s) => ({ ...s })) },
      voice,
      inputs: [...inputs],
    };
  }

  function playSession(): void {
    const greet =
      voice === "plain"
        ? "\x1b[1mClaude Code\x1b[0m 2.1.312 · in the workshop\r\n\r\n> "
        : "\x1b[36m✻\x1b[0m Arisen. I have the Stride Tuner sources open.\r\n" +
          "  The vanilla hashes still match the pins.\r\n\r\n> ";
    controls.emitOutput(greet);
    if (!script) return;
    let t = beat;
    const at = (fn: () => void) => {
      later(t, fn);
      t += beat;
    };
    at(() =>
      controls.emitHand({
        tool: "plan_install",
        outcome: "done",
        summary: "planned Stride Tuner 0.2.0 · 7 files · no conflicts",
        installId: null,
        refusal: null,
      }),
    );
    at(() =>
      controls.emitHand({
        tool: "install",
        outcome: "done",
        summary: "installed Stride Tuner 0.2.0 · 7 files",
        installId: "opt-stride-020",
        refusal: null,
      }),
    );
    at(() =>
      controls.emitHand({
        tool: "install",
        outcome: "refused",
        summary: "install Old Road Armor Pack",
        installId: null,
        refusal:
          "That install would write over a game file, natives\\stm\\gui\\ui01.pak. The stone never writes over vanilla.",
      }),
    );
    at(() =>
      controls.emitHand({
        tool: "read_reframework_log",
        outcome: "done",
        summary: "read the REFramework log · 214 lines",
        installId: null,
        refusal: null,
      }),
    );
    at(() => controls.hold());
  }

  const backend: WorkshopBackend = {
    getWorkshop: () => {
      call("get_workshop");
      return Promise.resolve(view());
    },
    summonPawn: (cols, rows) => {
      call("summon_pawn", cols, rows);
      if (pawn.state === "summoning" || pawn.state === "here") {
        return Promise.reject(pawnRefused("alreadyHere", "Tamsin is already here."));
      }
      if (chip === "breached") {
        return Promise.reject(
          pawnRefused("breached", "The last probe reached a place the fence should close. Run the probe again."),
        );
      }
      if (scenario === "nofence") {
        return Promise.reject(
          pawnRefused("noFence", "socat is not installed in Ubuntu-24.04. Install it with: sudo apt install socat"),
        );
      }
      folderMade = true;
      runCount += 1;
      pawnInstalls.length = 0;
      setPawn({
        runId: `run-mock-${runCount}`,
        state: "summoning",
        since: now(),
        exitCode: null,
        host: HOST,
        held: null,
      });
      later(beat, () => {
        setPawn({ state: "here" });
        playSession();
      });
      return Promise.resolve();
    },
    pawnWrite: (data) => {
      call("pawn_write", data);
      if (pawn.state !== "here") return Promise.reject(pawnRefused("notHere", "No pawn is here."));
      // A local echo, as the TUI would draw it; Enter gets one plain line back.
      if (data === "\r") controls.emitOutput("\r\n  Understood. Rebuilding.\r\n\r\n> ");
      else controls.emitOutput(data);
      return Promise.resolve();
    },
    pawnResize: (cols, rows) => {
      call("pawn_resize", cols, rows);
      return Promise.resolve();
    },
    departPawn: () => {
      call("depart_pawn");
      if (pawn.state === "summoning" || pawn.state === "here") {
        for (const t of timers.splice(0)) clearTimeout(t);
        setPawn({ state: "departed", exitCode: 0, held: null, since: now() });
      }
      return Promise.resolve();
    },
    answerPawnPlay: (runId, heldRequestId, accept) => {
      call("answer_pawn_play", runId, heldRequestId, accept);
      if (!pawn.held) return Promise.reject(pawnRefused("nothingHeld", "Tamsin is not waiting to play."));
      if (pawn.runId !== runId || pawn.held.heldRequestId !== heldRequestId) {
        return Promise.reject(pawnRefused("staleHeld", "That request is not the one waiting now."));
      }
      setPawn({ held: null });
      const decision = accept ? ("played" as const) : ("declined" as const);
      controls.emitHand({
        tool: "play",
        outcome: accept ? "done" : "declined",
        summary: accept ? "the stone started the game" : "asked to start the game · not now",
        installId: null,
        refusal: null,
      });
      return Promise.resolve({ decision, persisted: scenario !== "unpersisted" });
    },
    getFence: () => {
      call("get_fence");
      return Promise.resolve({ ...fence });
    },
    runFenceProbe: () => {
      call("run_fence_probe");
      if (pawn.state === "summoning" || pawn.state === "here") {
        return Promise.reject(pawnRefused("pawnHere", "Depart first: the probe needs the workshop to itself."));
      }
      return new Promise((resolve) => {
        later(beat * 2, () => {
          chip = "contained";
          fence = fenceFor("contained", "contained");
          emit(WORKSHOP_EVENTS.workshopChanged, {});
          resolve({ ...fence });
        });
      });
    },
    planBringThrough: (name) => {
      call("plan_bring_through", name);
      const a = artifacts.find((x) => x.name === name);
      if (!a) return Promise.reject(bringRefused("notInDist", `${name} is not in the workshop's dist folder.`));
      if (!a.receipt) return Promise.reject(bringRefused("noReceipt", `${name} has no receipt beside it.`));
      if (!a.sha256) return Promise.reject(bringRefused("bytesChanged", `${name} is still being read.`));
      let conflict: BringThroughPlan["conflict"] = "none";
      if (a.inLibrary) conflict = "alreadyHad";
      else if (scenario === "nametaken") conflict = "nameTaken";
      const plan: BringThroughPlan = {
        artifact: a.name,
        bytes: a.bytes,
        sha256: a.sha256,
        landsAs: `mods\\${a.name}`,
        receipt: a.receipt,
        conflict,
      };
      return Promise.resolve(plan);
    },
    bringThrough: (name, sha256) => {
      call("bring_through", name, sha256);
      const a = artifacts.find((x) => x.name === name);
      if (!a) return Promise.reject(bringRefused("notInDist", `${name} is not in the workshop's dist folder.`));
      if (a.sha256 !== sha256) {
        return Promise.reject(
          bringRefused(
            "bytesChanged",
            `${name} changed after the card opened: shown ${sha256.slice(0, 16)}, now ${(a.sha256 ?? "").slice(0, 16)}.`,
          ),
        );
      }
      if (scenario === "nametaken") {
        return Promise.reject(bringRefused("nameTaken", `The mods folder already holds a different ${name}.`));
      }
      if (a.inLibrary) return Promise.resolve({ outcome: "alreadyHad" });
      artifacts = artifacts.map((x) => (x.name === name ? { ...x, inLibrary: true } : x));
      emit(WORKSHOP_EVENTS.workshopChanged, {});
      return Promise.resolve({ outcome: "landed" });
    },
    planPutBack: (runId) => {
      call("plan_put_back", runId);
      const remove: UninstallPlanView[] = [...pawnInstalls].reverse().map((i) => ({
        optionId: i.installId,
        name: i.name,
        removals: [{ gamePath: "natives\\stm\\stride\\tuner.user.3", kind: "loose", action: { kind: "delete" } }],
        refused: null,
      }));
      const plan: PutBackPlan = {
        runId,
        remove,
        review: [
          {
            installId: "inst-3",
            name: "Brine Water Clarity",
            at: "2026-10-28T21:31:00Z",
            undo: "available",
            known: "yours",
          },
        ],
        sentence: `Removes the ${remove.length} installs Tamsin made in this summoning. She also uninstalled 1 of yours. It is listed below.`,
      };
      return Promise.resolve(plan);
    },
    putBack: (runId, installIds) => {
      call("put_back", runId, installIds);
      const done: UninstallResultView[] = [];
      for (const id of installIds) {
        const i = pawnInstalls.find((x) => x.installId === id);
        if (!i) {
          return Promise.reject(pawnRefused("notThisRun", "That install is not one Tamsin made in this summoning."));
        }
        done.push({
          optionId: i.installId,
          filesRemoved: 7,
          filesRestored: 0,
          filesKept: 0,
          filesHandedDown: 0,
          parked: 0,
          restartNeeded: false,
        });
      }
      pawnInstalls.splice(0, pawnInstalls.length, ...pawnInstalls.filter((x) => !installIds.includes(x.installId)));
      return Promise.resolve(done);
    },
    openWorkshopFolder: () => {
      call("open_workshop_folder");
      folderMade = true;
      return Promise.resolve();
    },
    setPawnVoice: (next) => {
      call("set_pawn_voice", next);
      voice = next;
      return Promise.resolve();
    },
    setPawnInputs: (next) => {
      call("set_pawn_inputs", next);
      const steam = next.find((p) => /steam/i.test(p));
      if (steam)
        return Promise.reject(pawnRefused("inputRefused", "That folder overlaps Steam, which the fence closes."));
      inputs = [...next];
      return Promise.resolve();
    },
    on: (event, handler) => {
      const set = listeners.get(event) ?? new Set();
      const l = handler as (payload: unknown) => void;
      set.add(l);
      listeners.set(event, set);
      return Promise.resolve(() => {
        set.delete(l);
      });
    },
  };
  return Object.assign(backend, { controls });
}
