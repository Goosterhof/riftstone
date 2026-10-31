// THE WORKSHOP's door to the spine (WINDOW-CONTRACT § v0.7). Separate from
// services/rift.ts so the public build, which never imports src/workshop/,
// carries none of it (RS-10). Inside Tauri it invokes the fifteen commands;
// outside it serves the mock in ./mock.ts.

import type { UninstallResultView } from "@/types/rift";
import type {
  BringThroughPlan,
  BringThroughResult,
  FenceView,
  PlayAnswered,
  PutBackPlan,
  PawnSession,
  WorkshopEvent,
  WorkshopView,
} from "@/types/workshop";
import { asRiftFailure } from "@/services/rift";

export interface WorkshopBackend {
  getWorkshop(): Promise<WorkshopView>;
  summonPawn(cols: number, rows: number): Promise<void>;
  /** Keystrokes for the summoning `runId`; the stone refuses them for any other. */
  pawnWrite(runId: string, data: string): Promise<void>;
  pawnResize(cols: number, rows: number): Promise<void>;
  departPawn(): Promise<void>;
  answerPawnPlay(runId: string, heldRequestId: string, accept: boolean): Promise<PlayAnswered>;
  getFence(): Promise<FenceView>;
  runFenceProbe(): Promise<FenceView>;
  planBringThrough(artifact: string): Promise<BringThroughPlan>;
  bringThrough(artifact: string, sha256: string): Promise<BringThroughResult>;
  planPutBack(runId: string): Promise<PutBackPlan>;
  putBack(runId: string, installIds: string[]): Promise<UninstallResultView[]>;
  openWorkshopFolder(): Promise<void>;
  setPawnSession(session: PawnSession): Promise<void>;
  setPawnInputs(inputs: string[]): Promise<void>;
  on<T>(event: WorkshopEvent, handler: (payload: T) => void): Promise<() => void>;
}

function insideTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function tauriWorkshop(): Promise<WorkshopBackend> {
  const core = await import("@tauri-apps/api/core");
  const { listen } = await import("@tauri-apps/api/event");
  const invoke = <T>(cmd: string, args?: Record<string, unknown>): Promise<T> =>
    core.invoke<T>(cmd, args).catch((raw: unknown) => {
      throw asRiftFailure(raw);
    });
  return {
    getWorkshop: () => invoke("get_workshop"),
    summonPawn: (cols, rows) => invoke("summon_pawn", { cols, rows }),
    pawnWrite: (runId, data) => invoke("pawn_write", { runId, data }),
    pawnResize: (cols, rows) => invoke("pawn_resize", { cols, rows }),
    departPawn: () => invoke("depart_pawn"),
    answerPawnPlay: (runId, heldRequestId, accept) => invoke("answer_pawn_play", { runId, heldRequestId, accept }),
    getFence: () => invoke("get_fence"),
    runFenceProbe: () => invoke("run_fence_probe"),
    planBringThrough: (artifact) => invoke("plan_bring_through", { artifact }),
    bringThrough: (artifact, sha256) => invoke("bring_through", { artifact, sha256 }),
    planPutBack: (runId) => invoke("plan_put_back", { runId }),
    putBack: (runId, installIds) => invoke("put_back", { runId, installIds }),
    openWorkshopFolder: () => invoke("open_workshop_folder"),
    setPawnSession: (session) => invoke("set_pawn_session", { session }),
    setPawnInputs: (inputs) => invoke("set_pawn_inputs", { inputs }),
    on: async (event, handler) => {
      const unlisten = await listen(event, (e) => {
        handler(e.payload as never);
      });
      return unlisten;
    },
  };
}

let backend: Promise<WorkshopBackend> | null = null;

/** The workshop's backend for this page: the real spine inside Tauri, the mock outside. */
export function workshop(): Promise<WorkshopBackend> {
  backend ??= insideTauri() ? tauriWorkshop() : import("./mock").then((m) => m.createMockWorkshop());
  return backend;
}

/** Tests swap in their own backend. */
export function setWorkshopBackend(next: WorkshopBackend | null): void {
  backend = next ? Promise.resolve(next) : null;
}
