// A picture pasted on the hollow (0.7.5): its path is typed to the summoning it
// was pasted for, and never to one that took the hollow while the picture
// travelled (the Heretic's PP1 on PR #17). The real WorkshopHollow and store,
// over the mock spine; xterm is a stub whose paste emits onData synchronously,
// as the real one does (xterm src/browser/Clipboard.ts).

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { resetToasts, useToasts } from "@/composables/useToasts";
import { setWorkshopBackend } from "@/workshop/backend";
import { createMockWorkshop, type MockWorkshop } from "@/workshop/mock";
import { resetWorkshop, useWorkshop } from "@/workshop/useWorkshop";
import WorkshopHollow from "@/workshop/WorkshopHollow.vue";

const xterm = vi.hoisted(() => ({ bracketed: false }));

vi.mock("@xterm/xterm", () => ({
  Terminal: class {
    cols = 80;
    rows = 24;
    options: Record<string, unknown>;
    modes = {
      get bracketedPasteMode() {
        return xterm.bracketed;
      },
    };
    send: (data: string) => void = () => undefined;
    constructor(options: Record<string, unknown>) {
      this.options = options;
    }
    loadAddon() {}
    attachCustomKeyEventHandler() {}
    open() {}
    focus() {}
    onData(fn: (data: string) => void) {
      this.send = fn;
    }
    onResize() {}
    write() {}
    reset() {}
    dispose() {}
    paste(text: string) {
      this.send(text);
    }
  },
}));
vi.mock("@xterm/addon-fit", () => ({
  FitAddon: class {
    fit() {}
  },
}));
vi.mock("@xterm/addon-web-links", () => ({ WebLinksAddon: class {} }));

let mock: MockWorkshop;
let wrapper: VueWrapper | null = null;

async function hollow(): Promise<VueWrapper> {
  mock = createMockWorkshop({ scenario: "contained", beat: 0, script: false });
  setWorkshopBackend(mock);
  const w = useWorkshop();
  await w.subscribe();
  await w.load();
  await w.summon(80, 24);
  await new Promise((resolve) => setTimeout(resolve, 0));
  await flushPromises();
  wrapper = mount(WorkshopHollow, { props: { live: true } });
  await flushPromises();
  return wrapper;
}

function pasteScreenshot(on: VueWrapper): void {
  const event = new Event("paste", { bubbles: true, cancelable: true });
  Object.defineProperty(event, "clipboardData", {
    value: {
      getData: () => "",
      items: [{ kind: "file", type: "image/png", getAsFile: () => new Blob(["picture"]) }],
    },
  });
  (on.element as HTMLElement).dispatchEvent(event);
}

const writes = () => mock.controls.calls.filter((c) => c.cmd === "pawn_write").map((c) => c.args);
const said = () => useToasts().toasts.value.map((t) => t.line);

beforeEach(() => {
  xterm.bracketed = false;
  resetWorkshop();
  resetToasts();
});
afterEach(async () => {
  wrapper?.unmount();
  wrapper = null;
  await flushPromises();
  setWorkshopBackend(null);
});

describe("a picture pasted on the hollow (0.7.5)", () => {
  it("should type the path to the summoning it was pasted for", async () => {
    const on = await hollow();
    const before = writes().length;
    pasteScreenshot(on);
    await flushPromises();
    expect(writes().slice(before)).toStrictEqual([["run-mock-1", "pasted/paste-20261031-231401.png "]]);
    expect(said().at(-1)).toBe(
      "The picture is in the workshop as pasted/paste-20261031-231401.png, and its path is in her prompt.",
    );
  });

  it("should bracket the path when her terminal asked for bracketed paste", async () => {
    xterm.bracketed = true;
    const on = await hollow();
    const before = writes().length;
    pasteScreenshot(on);
    await flushPromises();
    expect(writes().slice(before)).toStrictEqual([
      ["run-mock-1", "\u001b[200~pasted/paste-20261031-231401.png \u001b[201~"],
    ]);
  });

  it("should type nothing to a summoning that took the hollow while the picture travelled (PP1)", async () => {
    const on = await hollow();
    let finish: ((path: string) => void) | null = null;
    mock.pasteToPawn = () =>
      new Promise<string>((resolve) => {
        finish = resolve;
      });
    const before = writes().length;
    pasteScreenshot(on);
    await flushPromises();
    (finish as unknown as (path: string) => void)("pasted/old-run-picture.png");
    queueMicrotask(() =>
      mock.controls.emitRawState({
        runId: "run-successor",
        state: "here",
        since: "2026-10-31T23:00:00Z",
        exitCode: null,
        host: null,
        held: null,
      }),
    );
    await flushPromises();
    expect(writes().slice(before), "OLD_PICTURE_MUST_NOT_REACH_SUCCESSOR").toStrictEqual([]);
    expect(said().at(-1)).toBe(
      "The picture is in the workshop as pasted/old-run-picture.png, but that summoning ended, so its path was not typed.",
    );
  });
});
