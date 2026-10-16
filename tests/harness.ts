// The bench every window spec stands on: a scenario in the query string, the
// mock spine swapped in, every composable back to its first frame, the whole
// App mounted into the document (focus needs a real attachment).

import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import App from "@/App.vue";
import { resetRiftstone } from "@/composables/boot";
import { setRiftBackend, type RiftBackend } from "@/services/rift";
import { createMockBackend } from "@/services/mock/index";

/** A node's text. Typed on Node, where the DOM lib (every TS version the
 *  gates run) still says `string | null`, so neither vue-tsc nor tsgolint
 *  argues about the fallback. */
export const textOf = (n: Node): string => n.textContent ?? "";

/** The trimmed text of the first element matching `selector`, or null. */
export function textAt(selector: string): string | null {
  const el = document.querySelector(selector);
  return el ? textOf(el).trim() : null;
}

export type Scene =
  | ""
  | "empty"
  | "nogame"
  | "scanning"
  | "refoff"
  | "patched"
  | "offline"
  | "fresh"
  | "key"
  | "quotalow";

let mounted: VueWrapper | null = null;
let backend: RiftBackend | null = null;

/** The mock spine this spec's window talks to: spy on it to read what the
 *  window sent, or stand in for one of its answers. */
export function spine(): RiftBackend {
  if (!backend) throw new Error("No spine is awake: call wakeOn() first");
  return backend;
}

/** Poll until `probe` holds; the mock answers on real timers (8–400 ms). */
export async function until(probe: () => boolean, what: string, timeoutMs = 3000): Promise<void> {
  const start = Date.now();
  while (!probe()) {
    if (Date.now() - start > timeoutMs) throw new Error(`The stone never showed: ${what}`);
    await flushPromises();
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
}

export async function wakeOn(scene: Scene = ""): Promise<VueWrapper> {
  window.history.replaceState(null, "", scene ? `/?mock=${scene}` : "/");
  resetRiftstone();
  backend = createMockBackend();
  setRiftBackend(backend);
  const host = document.createElement("div");
  document.body.append(host);
  mounted = mount(App, { attachTo: host });
  await until(() => !document.querySelector(".wake") || /ANSWERS|FOUND/.test(textOf(document.body)), "the first frame");
  await flushPromises();
  return mounted;
}

export function sleep(): void {
  mounted?.unmount();
  mounted = null;
  resetRiftstone();
  setRiftBackend(null);
  backend = null;
  document.body.innerHTML = "";
}

export const text = (): string => textOf(document.body).replace(/\s+/g, " ");

/** A button's label: RiftButton's label span (its busy "···" is not the label). */
const labelOf = (b: HTMLButtonElement): string => textOf(b.querySelector(".rs-btn__label") ?? b).trim();

/** The button whose label matches; throws a voiced miss. */
export function button(label: string | RegExp): HTMLButtonElement {
  const all = [...document.querySelectorAll<HTMLButtonElement>("button")];
  const found = all.find((b) => (typeof label === "string" ? labelOf(b) === label : label.test(labelOf(b))));
  if (!found) {
    const seen = all.map(labelOf).filter(Boolean);
    throw new Error(`No button "${String(label)}" — the window shows: ${seen.join(" | ")}`);
  }
  return found;
}

/** A button that can take a press now: not disabled, and not busy (a busy
 *  RiftButton stays focusable, aria-disabled, so `.disabled` alone says nothing). */
export function idle(b: HTMLButtonElement): boolean {
  return !b.disabled && b.getAttribute("aria-busy") !== "true";
}

export function hasButton(label: string): boolean {
  return [...document.querySelectorAll<HTMLButtonElement>("button")].some((b) => labelOf(b) === label);
}

export async function typeQuery(query: string): Promise<void> {
  const input = document.getElementById("rift-search");
  if (!(input instanceof HTMLInputElement)) throw new Error("The search well is missing");
  input.value = query;
  input.dispatchEvent(new Event("input"));
  await new Promise((resolve) => setTimeout(resolve, 160));
  await flushPromises();
}

export function press(
  key: string,
  init: KeyboardEventInit = {},
  target: EventTarget = document.activeElement ?? window,
): KeyboardEvent {
  const event = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true, ...init });
  target.dispatchEvent(event);
  return event;
}
