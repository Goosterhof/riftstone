// 0.7.4 — THE BENCH folds (investor, 2026-10-31: "can we make the tabs on the
// side also a dropdown? cause i do not always need to see that summoned with
// and inputs and stuff"). Each section folds under its own head, and the
// window remembers which ones the investor folded. A window-only preference:
// it decides nothing the stone does, so it lives in the webview's storage,
// never in `pawn.json` or the settings (RS-10 keeps those free of the pawn).
import { ref } from "vue";

export type BenchSectionKey = "session" | "inputs" | "built" | "hands";

export const FOLDS_KEY = "riftstone.workshop.bench-folds";

/** Which sections the investor folded. Everything else is open. */
const folded = ref<Partial<Record<BenchSectionKey, boolean>>>(read());

function read(): Partial<Record<BenchSectionKey, boolean>> {
  try {
    const raw = globalThis.localStorage.getItem(FOLDS_KEY);
    if (!raw) return {};
    const parsed: unknown = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object") return {};
    const out: Partial<Record<BenchSectionKey, boolean>> = {};
    for (const key of ["session", "inputs", "built", "hands"] as const) {
      if ((parsed as Record<string, unknown>)[key] === true) out[key] = true;
    }
    return out;
  } catch {
    return {};
  }
}

function write(): void {
  try {
    globalThis.localStorage.setItem(FOLDS_KEY, JSON.stringify(folded.value));
  } catch {
    // A webview that keeps nothing still folds for this session.
  }
}

export function useBenchFolds() {
  return {
    isFolded: (key: BenchSectionKey): boolean => folded.value[key] === true,
    toggle(key: BenchSectionKey): void {
      folded.value = { ...folded.value, [key]: !folded.value[key] };
      write();
    },
  };
}

/** The specs start every mount from what storage holds now. */
export function resetBenchFolds(): void {
  folded.value = read();
}

/** A count in the folded head's words: `none`, `1 folder`, `3 folders`. */
export function counted(n: number, one: string, many: string, zero: string): string {
  if (n === 0) return zero;
  return `${n} ${n === 1 ? one : many}`;
}
