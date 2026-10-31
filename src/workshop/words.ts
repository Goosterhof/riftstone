// THE WORKSHOP's words (#00015 §20, WINDOW-CONTRACT § v0.7): how the room says
// what the stone sent. Pure, so the specs read the same sentences the room
// renders. Nothing here adds a claim the stone did not make.

import type {
  ArtifactView,
  Chip,
  PawnEffort,
  PawnHand,
  PawnMode,
  PawnModel,
  PutBackReview,
  StepExpect,
  StepVerdict,
} from "@/types/workshop";

export const PAWN_NAME = "Tamsin";

/** R20.12, three values since the contract (the fence is whole-process). */
export const CHIP_WORD: Record<Chip, string> = {
  contained: "CONTAINED",
  unprobed: "UNPROBED",
  breached: "BREACHED",
};

export const EXPECT_WORD: Record<StepExpect, string> = {
  land: "must land",
  notLand: "must not land",
  fail: "must fail",
  succeed: "must answer",
};

/** What each of the record's rows tried (BUILD-SPEC § v0.7, the probe's table). */
export const STEP_WORD: Record<string, string> = {
  "1": "write into the mods folder",
  "2": "write into the game folder",
  "3": "list Steam's userdata",
  "4": "read the stone's canary",
  "5": "write into the investor's .claude",
  "6": "run a planted cmd.exe",
  "7": "run the real cmd.exe",
  "8": "claude --version",
  "9": "read the canary through a junction",
  "10": "read the stone's secret from the environment",
  "11": "link and write in answers/",
  "12": "inject keystrokes (TIOCSTI)",
  B1: "broker: a link as a request",
  B2: "broker: a bad name, 64 KiB + 1",
};

/** The row's mark and its spoken word, never colour alone. */
export const VERDICT_MARK: Record<StepVerdict, { mark: string; word: string }> = {
  held: { mark: "✓", word: "held" },
  breached: { mark: "✕", word: "breached" },
  inconclusive: { mark: "?", word: "could not show" },
};

export const KNOWN_WORD: Record<PutBackReview["known"], string> = {
  yours: "yours",
  anotherSummoning: "from another summoning",
  uncertain: "unknown whose",
};

export const RECEIPT_PROBLEM: Record<NonNullable<ArtifactView["receiptProblem"]>, string> = {
  missing: "no receipt",
  unreadable: "receipt unreadable",
  mismatch: "the receipt names other bytes",
};

export const NO_RECEIPT_TITLE = "The stone adds a build to the library only with its receipt beside it.";
export const PROBE_HELD_BACK = "Depart first: the probe needs the workshop to itself.";
export const SESSION_LATER = "Takes effect at the next summon.";

/** THE BENCH's SUMMONED WITH (0.7.2): each choice, and the one line that says
 *  what the chosen one does. The stone passes them as Claude Code's own flags.
 *  The mode lines describe her settings as they are (`fence::pawn_settings`):
 *  outside Plan, an eligible shell command can run in Claude Code's sandbox
 *  without asking (`autoAllowBashIfSandboxed`), and none may run outside it;
 *  permission-flow exceptions and new network domains can still ask. No line
 *  promises a prompt the sandbox or Auto's judge does not give, nor the
 *  converse absolute (the Heretic's BM2 on PR #14, against
 *  code.claude.com/docs/en/permission-modes and /sandboxing). */
export interface DialChoice<T extends string> {
  value: T;
  label: string;
  says: string;
}

export const MODE_CHOICES: DialChoice<PawnMode>[] = [
  {
    value: "ask",
    label: "Ask",
    says: "She asks before each file edit. Most shell commands run unasked, inside her sandbox.",
  },
  {
    value: "acceptEdits",
    label: "Edits",
    says: "Her file edits in the workshop go through unasked, like most sandboxed shell commands.",
  },
  { value: "plan", label: "Plan", says: "She researches and plans, and edits nothing until you approve the plan." },
  {
    value: "auto",
    label: "Auto",
    says: "Claude Code's judge approves or blocks each action; repeated blocks bring your prompts back.",
  },
];

/** Effort's labels are Claude Code's own words, as typed after `/effort`. */
export const EFFORT_CHOICES: DialChoice<PawnEffort>[] = [
  { value: "default", label: "default", says: "Claude Code picks the effort for her model." },
  { value: "low", label: "low", says: "She answers quickly and thinks little." },
  { value: "medium", label: "med", says: "She thinks a while before she answers." },
  { value: "high", label: "high", says: "She thinks hard: slower, and more of your usage." },
  { value: "xhigh", label: "xhigh", says: "She thinks harder still: slower, and more of your usage." },
  { value: "max", label: "max", says: "Her deepest thinking: the slowest, and the most of your usage." },
];

export const MODEL_CHOICES: DialChoice<PawnModel>[] = [
  { value: "default", label: "Default", says: "Claude Code's default model for your account." },
  { value: "fable", label: "Fable", says: "The newest Fable: the most capable, and the costliest." },
  { value: "opus", label: "Opus", says: "The newest Opus: deep, careful work." },
  { value: "sonnet", label: "Sonnet", says: "The newest Sonnet: quick and capable." },
  { value: "haiku", label: "Haiku", says: "The newest Haiku: the quickest and lightest." },
];

export const SESSION_NOTE =
  "She starts every summon with these. Changed in her terminal (Shift+Tab, /effort, /model), a setting lasts until she departs.";

/** HH:MM, 24-hour, local: the second line of a hands row. */
export function clock(at: string | null | undefined): string {
  if (!at) return "";
  const d = new Date(at);
  if (Number.isNaN(d.getTime())) return at;
  return d.toLocaleTimeString("en-GB", { hour: "2-digit", minute: "2-digit", hour12: false });
}

const pad = (n: number) => String(n).padStart(2, "0");

/** YYYY-MM-DD HH:MM, local: when a probe record was proved. */
export function stamp(at: string | null | undefined): string {
  if (!at) return "";
  const d = new Date(at);
  if (Number.isNaN(d.getTime())) return at;
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${clock(at)}`;
}

/** A digest in fours, as the stone carves it (R20.6). */
export function quads(sha256: string): string[] {
  return sha256.match(/.{1,4}/g) ?? [];
}

/** `41.2 KB`: one decimal, in KiB as the size line reads elsewhere. */
export function size(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const kb = bytes / 1024;
  if (kb < 1024) return `${kb.toFixed(1)} KB`;
  return `${(kb / 1024).toFixed(1)} MB`;
}

export type HandTone = "muted" | "world" | "warn" | "ember" | "faint";

export interface HandFace {
  glyph: string;
  tone: HandTone;
  line: string;
  second: string;
}

const GLYPH: Record<string, string> = {
  plan_install: "▸",
  install: "+",
  uninstall: "↺",
  read_reframework_log: "≡",
  list_installed: "≡",
  play: "▶",
};

const TOOL_WORD: Record<string, string> = {
  plan_install: "plan an install",
  install: "install",
  uninstall: "uninstall",
  read_reframework_log: "read the REFramework log",
  list_installed: "list what is installed",
  play: "start the game",
};

function quiet(hand: PawnHand): HandTone {
  if (hand.tool === "install" && hand.outcome === "done") return "world";
  if (hand.tool === "play" && hand.outcome === "held") return "warn";
  return "muted";
}

/** One hands row from the stone's own summary (contract amendment 3). The
 *  spine's refused summary IS the refusal, so the first line then names the
 *  tool and the sentence stands once, on the second. */
function refusedFace(hand: PawnHand): HandFace {
  const what = hand.refusal && hand.summary !== hand.refusal ? hand.summary : (TOOL_WORD[hand.tool ?? ""] ?? "a call");
  return { glyph: "✕", tone: "ember", line: `refused: ${what}`, second: hand.refusal ?? hand.summary };
}

export function handFace(hand: PawnHand): HandFace {
  if (hand.outcome === "refused") return refusedFace(hand);
  const glyph = GLYPH[hand.tool ?? ""] ?? "·";
  if (hand.outcome === "repeated") {
    return { glyph, tone: "faint", line: hand.summary, second: "answered again from the ledger" };
  }
  if (hand.outcome === "unknown") {
    return { glyph, tone: "warn", line: hand.summary, second: "made, but the stone could not record it" };
  }
  return { glyph, tone: quiet(hand), line: hand.summary, second: clock(hand.at) };
}

/** R20.4: the sixteen ANSI colours on the stone's tokens (hex from uno.config.ts's
 *  riftColors; tests/workshopRoom.spec.ts holds them equal). Cyan and blue are
 *  the Rift, magenta is your world's gold, black is the well, used only as an
 *  inverse ground. The selection's foreground is forced to bone: muted on
 *  riftdeep measures 3.80 and fails. */
export const TERMINAL_THEME = {
  background: "#0A0E13", //    --rs-void
  foreground: "#ECE6D8", //    --rs-text
  cursor: "#8FE3F2", //        --rs-rift
  cursorAccent: "#0A0E13", //  --rs-void
  selectionBackground: "#1E5560", // --rs-riftdeep
  selectionForeground: "#ECE6D8", // --rs-text, forced
  black: "#121417", //         --rs-well
  brightBlack: "#8A929C", //   --rs-faint
  red: "#FF7B66", //           --rs-ember
  brightRed: "#FF7B66",
  green: "#B5E07C", //         --rs-verdant
  brightGreen: "#B5E07C",
  yellow: "#FFAE57", //        --rs-warn
  brightYellow: "#FFAE57",
  blue: "#8FE3F2", //          --rs-rift
  brightBlue: "#8FE3F2",
  cyan: "#8FE3F2",
  brightCyan: "#8FE3F2",
  magenta: "#EBC46F", //       --rs-world
  brightMagenta: "#EBC46F",
  white: "#ECE6D8", //         --rs-text
  brightWhite: "#ECE6D8",
  // the scrollbar is a control boundary: --rs-edgehi, faint at rest
  scrollbarSliderBackground: "rgba(110, 120, 131, 0.3)",
  scrollbarSliderHoverBackground: "rgba(110, 120, 131, 0.5)",
  scrollbarSliderActiveBackground: "rgba(110, 120, 131, 0.65)",
} as const;
