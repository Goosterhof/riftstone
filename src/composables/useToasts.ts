// The stone's short sentences: one shelf, bottom of the Rift. A toast with an
// Undo lives 8 s (§9 "Install landed"); a plain one 5 s; a failure stays until
// dismissed or acted on.

import { ref } from "vue";

export type ToastTone = "world" | "rift" | "ember" | "warn";

export interface ToastAction {
  label: string;
  run: () => void | Promise<void>;
}

export interface Toast {
  id: number;
  tone: ToastTone;
  line: string;
  actions: ToastAction[];
  ttlMs: number | null;
  /** A carved line above the sentence (§9's inscription column), or null. */
  inscription: string | null;
}

const toasts = ref<Toast[]>([]);
let nextId = 1;
const timers = new Map<number, ReturnType<typeof setTimeout>>();

function dismiss(id: number): void {
  toasts.value = toasts.value.filter((t) => t.id !== id);
  const timer = timers.get(id);
  if (timer) clearTimeout(timer);
  timers.delete(id);
}

function defaultTtl(tone: ToastTone, actions: ToastAction[]): number | null {
  if (tone === "ember") return null;
  return actions.length ? 8_000 : 5_000;
}

/** What a toast may carry beyond its tone, sentence and doors. */
export interface ToastOptions {
  /** How long it stays: null keeps it until dismissed; left out, the tone's own (above). */
  ttlMs?: number | null;
  /** A carved line above the sentence (§9's inscription column). */
  inscription?: string | null;
}

function push(tone: ToastTone, line: string, actions: ToastAction[] = [], options: ToastOptions = {}): number {
  const id = nextId++;
  const ttl = options.ttlMs === undefined ? defaultTtl(tone, actions) : options.ttlMs;
  const inscription = options.inscription ?? null;
  toasts.value = [...toasts.value.slice(-2), { id, tone, line, actions, ttlMs: ttl, inscription }];
  if (ttl !== null)
    timers.set(
      id,
      setTimeout(() => dismiss(id), ttl),
    );
  return id;
}

async function act(toast: Toast, action: ToastAction): Promise<void> {
  dismiss(toast.id);
  await action.run();
}

export function useToasts() {
  return { toasts, push, dismiss, act };
}

export function resetToasts(): void {
  for (const timer of timers.values()) clearTimeout(timer);
  timers.clear();
  toasts.value = [];
  nextId = 1;
}
