<script setup lang="ts">
// R20.4 — the hollow: Tamsin's terminal, the Rift seen through a crack in the
// stone. xterm lives here alone so the room's specs can stub it. The bytes come
// from the store in order (attach); keystrokes go back unfiltered, because
// xterm answers ConPTY's cursor query (ESC[6n) itself and the real summon waits
// on that answer. The terminal stays mounted while other rooms show (§20.12):
// it refits when its box changes size and is on screen, never while hidden, so
// a room switch alone never repaints the conversation. The player's hands on it
// (copy, paste, Ctrl+click on a link) are in hollowHands.ts.
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import "@xterm/xterm/css/xterm.css";
import { useToasts } from "@/composables/useToasts";
import { rift } from "@/services/rift";
import { linkHint, makeHollowKeyHandler, makeHollowLinkActivate } from "./hollowHands";
import { useWorkshop } from "./useWorkshop";
import { TERMINAL_THEME } from "./words";

const { live = false } = defineProps<{
  /** A pawn is here: keystrokes cross. Otherwise the conversation is read-only. */
  live?: boolean;
}>();

const w = useWorkshop();
const screen = ref<HTMLElement | null>(null);

let term: Terminal | null = null;
let fit: FitAddon | null = null;
let observer: ResizeObserver | null = null;
let detach: (() => void) | null = null;
let gone = false;

function onScreen(): boolean {
  const el = screen.value;
  return !!el && el.clientWidth > 0 && el.clientHeight > 0;
}

function refit(): void {
  if (!term || !fit || !onScreen()) return;
  fit.fit();
}

/** The fitted size Summon passes to the spine. */
function size(): { cols: number; rows: number } {
  refit();
  return { cols: term?.cols ?? 80, rows: term?.rows ?? 24 };
}

function focus(): void {
  term?.focus();
}

const activateLink = makeHollowLinkActivate({
  open: async (url) => (await rift()).openUrl(url),
  opened: (host) => useToasts().push("rift", `Opened ${host} in your browser.`),
  // the sign-in URL is hard-wrapped on screen, so a selection of it copies broken lines
  failed: (url) =>
    void navigator.clipboard.writeText(url).then(
      () =>
        useToasts().push(
          "warn",
          "The stone could not open that link, so it is on your clipboard. Paste it into your browser.",
        ),
      () => useToasts().push("warn", "The stone could neither open that link nor copy it."),
    ),
});

function hover(_event: MouseEvent, text: string): void {
  if (screen.value) screen.value.title = linkHint(text);
}

function leave(): void {
  if (screen.value) screen.value.title = "";
}

async function open(): Promise<void> {
  // the cell is measured at open: measure it on JetBrains Mono, not the fallback
  await Promise.all([
    document.fonts?.load('400 14px "JetBrains Mono"'),
    document.fonts?.load('600 14px "JetBrains Mono"'),
  ]).catch(() => undefined);
  if (gone || !screen.value) return;
  term = new Terminal({
    fontFamily: '"JetBrains Mono", Consolas, monospace',
    fontSize: 14,
    fontWeight: 400,
    fontWeightBold: 600,
    lineHeight: 1.3,
    scrollback: 5000,
    cursorBlink: false,
    cursorStyle: "block",
    disableStdin: !live,
    theme: { ...TERMINAL_THEME },
    // OSC 8 links (a name over a URL) take the same Ctrl+click as plain ones
    linkHandler: { activate: activateLink, hover, leave, allowNonHttpProtocols: false },
  });
  fit = new FitAddon();
  term.loadAddon(fit);
  term.loadAddon(new WebLinksAddon(activateLink, { hover, leave }));
  const t = term;
  t.attachCustomKeyEventHandler(
    makeHollowKeyHandler(t, {
      copy: (text) => navigator.clipboard.writeText(text),
      // only while she is here: this ^C goes around disableStdin
      interrupt: () => {
        if (live) void w.write("\u0003");
      },
      copyFailed: () => useToasts().push("warn", "The clipboard refused the selection."),
    }),
  );
  term.open(screen.value);
  refit();
  term.onData((data) => void w.write(data));
  term.onResize(({ cols, rows }) => void w.resize(cols, rows));
  detach = w.attach((bytes) => term?.write(bytes));
  observer = new ResizeObserver(() => refit());
  observer.observe(screen.value);
}

watch(
  () => live,
  (on) => {
    if (term) term.options.disableStdin = !on;
  },
);
// A new summoning starts on a clean slate; synchronous, so the reset lands
// before the run's first byte does.
watch(
  () => w.runId.value,
  (next, prev) => {
    if (next && prev && next !== prev) term?.reset();
  },
  { flush: "sync" },
);

onMounted(() => void open());
onBeforeUnmount(() => {
  gone = true;
  observer?.disconnect();
  detach?.();
  term?.dispose();
  term = null;
});

defineExpose({ size, focus, refit });
</script>

<template>
  <div ref="screen" class="hollow-screen" />
</template>

<style scoped>
.hollow-screen {
  width: 100%;
  height: 100%;
}
.hollow-screen :deep(.xterm) {
  height: 100%;
}
.hollow-screen :deep(.xterm-viewport) {
  background: var(--rs-void) !important;
}
</style>
