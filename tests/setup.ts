// Vitest setup — the Tauri bridge does not exist under happy-dom; specs mock
// `@/services/rift` instead of the bridge (see docs/BUILD-SPEC.md § The Window).
//
// No spec may reach the network. The mock's openUrl calls window.open, and
// happy-dom FETCHES whatever a window opens: on 2026-10-16 a Find spec sent a
// real GET to nexusmods.com that way. So window.open only records here, and
// fetch refuses every host but this machine. A spec that wants either spies
// on it (`vi.spyOn(window, "open")`).
import { vi } from "vitest";

const LOCAL_HOSTS = new Set(["localhost", "127.0.0.1", "[::1]"]);

function isLocal(target: string): boolean {
  try {
    return LOCAL_HOSTS.has(new URL(target, "http://localhost/").hostname);
  } catch {
    return false;
  }
}

function targetOf(input: RequestInfo | URL): string {
  if (typeof input === "string") return input;
  if (input instanceof URL) return input.href;
  return input.url;
}

window.open = vi.fn<typeof window.open>(() => null);

const realFetch = globalThis.fetch.bind(globalThis);
globalThis.fetch = async (input: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
  const target = targetOf(input);
  if (!isLocal(target)) throw new Error(`A spec tried to reach the network: ${target}`);
  return realFetch(input, init);
};
