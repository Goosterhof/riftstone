// THE FRAY's one door to Tauri. The second window PULLS its frame
// (`fray_frame`) every 250 ms; it listens to no event (§19.15). Outside Tauri,
// or with `?mock=<scene>` in the URL, it reads the mock feed
// (src/services/mock/fray.ts) instead, loaded as its own chunk only then, the
// way src/services/rift.ts switches for the main window.

import type { FrayFrame } from "@/types/fray";
import type { FrayViewHint } from "./mock/fray";

export type { FrayViewHint } from "./mock/fray";

/** How often the window pulls (ms). */
export const FRAY_PULL_MS = 250;

export interface FraySource {
  pull(): Promise<FrayFrame>;
  /** Where the view starts (only the mock has a say). */
  view: FrayViewHint;
}

function insideTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** The `?mock=` scene, or null when the URL names none. */
export function mockScene(): string | null {
  if (typeof window === "undefined") return null;
  return new URLSearchParams(window.location.search).get("mock");
}

async function tauriSource(): Promise<FraySource> {
  const { invoke } = await import("@tauri-apps/api/core");
  return { pull: async () => invoke<FrayFrame>("fray_frame"), view: {} };
}

async function mockSource(scene: string): Promise<FraySource> {
  const { createFrayMock } = await import("./mock/fray");
  const mock = createFrayMock(scene);
  return { pull: () => Promise.resolve(mock.pull()), view: mock.view };
}

let source: Promise<FraySource> | null = null;

/** The source for this window: the spine inside Tauri, the mock outside or under ?mock=. */
export function fraySource(): Promise<FraySource> {
  const scene = mockScene();
  source ??= scene !== null || !insideTauri() ? mockSource(scene ?? "fray") : tauriSource();
  return source;
}

/** Tests swap in their own source; null puts the default back. */
export function setFraySource(next: FraySource | null): void {
  source = next ? Promise.resolve(next) : null;
}

/** One frame from the feed. */
export async function pullFrame(): Promise<FrayFrame> {
  const s = await fraySource();
  return s.pull();
}

/**
 * Pull every `everyMs` and hand each frame on. A pull still in flight is never
 * overlapped; a failed pull is reported and the next one is tried. Returns the
 * stop.
 */
export function startFrayPoll(
  onFrame: (frame: FrayFrame) => void,
  onError: (error: unknown) => void = () => undefined,
  everyMs: number = FRAY_PULL_MS,
): () => void {
  const run = { busy: false, stopped: false };
  // read through a call: a flag set by stop() while a pull is in flight
  const stopped = (): boolean => run.stopped;
  const tick = async (): Promise<void> => {
    if (run.busy || run.stopped) return;
    run.busy = true;
    try {
      const frame = await pullFrame();
      if (!stopped()) onFrame(frame);
    } catch (error) {
      onError(error);
    } finally {
      run.busy = false;
    }
  };
  void tick();
  const timer = setInterval(() => void tick(), everyMs);
  return () => {
    run.stopped = true;
    clearInterval(timer);
  };
}
