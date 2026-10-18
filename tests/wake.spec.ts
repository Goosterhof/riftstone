// The wake (src/composables/boot.ts): the window subscribes to every event of
// the contract BEFORE its first get_overview. The spine takes that first read
// as the sign that the window listens, and only then lets the nxm:// links
// that waited at its door through (WINDOW-CONTRACT startup step 9). A read
// before the subscription would let a cold start's rift://nxm-received fire
// into a window that hears nothing: the link taken or refused, nobody told.

import { afterEach, describe, expect, it, vi } from "vitest";
import { EVENTS } from "@/types/rift";
import { resetRiftstone, wakeRiftstone } from "@/composables/boot";
import { setRiftBackend } from "@/services/rift";
import { createMockBackend } from "@/services/mock/index";

afterEach(() => {
  vi.restoreAllMocks();
  resetRiftstone();
  setRiftBackend(null);
});

describe("the wake", () => {
  it("should listen to every event of the contract before its first get_overview, the spine's sign that it listens", async () => {
    resetRiftstone();
    const backend = createMockBackend();
    const listens = vi.spyOn(backend, "on");
    const reads = vi.spyOn(backend, "getOverview");
    setRiftBackend(backend);

    await wakeRiftstone();

    expect(new Set(listens.mock.calls.map(([event]) => event))).toStrictEqual(new Set(Object.values(EVENTS)));
    expect(reads).toHaveBeenCalled();
    const lastListen = Math.max(...listens.mock.invocationCallOrder);
    const firstRead = Math.min(...reads.mock.invocationCallOrder);
    expect(lastListen).toBeLessThan(firstRead);
  });
});
