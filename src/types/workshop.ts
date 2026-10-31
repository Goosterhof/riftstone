// v0.7 — THE WORKSHOP (WINDOW-CONTRACT § v0.7): Tamsin, a Claude Code session the
// stone summons in WSL inside its fence. Only the investor's build carries any of
// this (RS-10): the module is imported only under VITE_RIFT_PAWN=1.
// Shapes follow the spine's serialised views (src-tauri/src/pawn/).

import type { UninstallPlanView } from "@/types/rift";

/** Who approves her actions (0.7.2): `ask` is Claude Code's `default`. */
export type PawnMode = "ask" | "acceptEdits" | "plan" | "auto";
/** `--effort`; `default` passes no flag. */
export type PawnEffort = "default" | "low" | "medium" | "high" | "xhigh" | "max";
/** `--model` by Claude Code's alias; `default` passes no flag. */
export type PawnModel = "default" | "fable" | "opus" | "sonnet" | "haiku";

/** What THE BENCH sets for her Claude Code, kept in `pawn.json` and passed at
 *  each summon (the stone launches her `--restricted`, which reads no user
 *  settings, so nothing she is told in the terminal outlives her summoning). */
export interface PawnSession {
  mode: PawnMode;
  effort: PawnEffort;
  model: PawnModel;
}

export type PawnState = "absent" | "summoning" | "here" | "departed" | "torn";

export interface HostView {
  distro: string;
  claudeVersion: string;
}

export interface HeldView {
  heldRequestId: string;
  since: string;
}

/** 0.8: Tamsin's update ask waiting, with the digest frozen at her ask. */
export interface HeldUpdateView {
  heldRequestId: string;
  since: string;
  artifact: string;
  sha256: string;
}

export interface PawnStateView {
  runId: string | null;
  state: PawnState;
  since: string | null;
  exitCode: number | null;
  host: HostView | null;
  /** The waiting Play request, so the press can carry it with `runId`. */
  held: HeldView | null;
  /** 0.8: the waiting update ask. */
  heldUpdate: HeldUpdateView | null;
}

export type HandOutcome = "done" | "refused" | "held" | "declined" | "repeated" | "unknown";

/** One call the stone answered for the pawn, refused calls included. */
export interface PawnHand {
  runId: string;
  seq: number;
  at: string;
  /** `list_installed`, `plan_install`, `install`, `uninstall`, `play`, `read_reframework_log`;
   *  null for a request the stone could not read as any tool. */
  tool: string | null;
  outcome: HandOutcome;
  /** The row's text, e.g. `installed Stature 0.2.0 · 7 files`. */
  summary: string;
  installId: string | null;
  /** The stone's own sentence, verbatim. */
  refusal: string | null;
}

export type Chip = "contained" | "unprobed" | "breached";
export type StepExpect = "land" | "notLand" | "fail" | "succeed";

export interface Ground {
  distro: string;
  claudeVersion: string;
  kernel: string;
  digest: string;
}

export type StepVerdict = "held" | "breached" | "inconclusive";

/** One row of the record: steps "1"–"12", then the broker checks "B1" and "B2". */
export interface FenceStep {
  step: string;
  expected: StepExpect;
  observed: string;
  /** `verdict === "held"`. */
  ok: boolean;
  /** held: the stone saw the refusal; breached: something crossed; inconclusive: it showed neither. */
  verdict: StepVerdict;
}

export interface FenceView {
  chip: Chip;
  /** Whether the record's control landed; null without a record. */
  control: boolean | null;
  ground: Ground | null;
  provedAt: string | null;
  steps: FenceStep[];
  /** The sentence for a record proved on another ground. */
  changed: string | null;
}

export interface ReceiptInput {
  path: string;
  sha256: string;
}

export interface ReceiptCheck {
  name: string;
  passed: number;
  total: number;
}

/** What the pawn wrote beside a build. Never trusted: shown, attributed, under THE RECEIPT SAYS. */
export interface ReceiptView {
  artifact: string;
  sha256: string;
  builtAt: string;
  inputs: ReceiptInput[];
  checks: ReceiptCheck[];
}

export interface ArtifactView {
  name: string;
  bytes: number;
  /** The stone's own digest of the bytes on disk now; null while hashing. */
  sha256: string | null;
  receipt: ReceiptView | null;
  receiptProblem: "missing" | "unreadable" | "mismatch" | null;
  /** A file with these exact bytes is in the mods folder. */
  inLibrary: boolean;
  /** 0.8: the earlier build an Update would replace, by file name. */
  earlier: string | null;
}

export interface WorkshopView {
  folder: { made: boolean; path: string };
  artifacts: ArtifactView[];
  pawn: PawnStateView;
  fence: FenceView;
  session: PawnSession;
  inputs: string[];
}

export interface PlayAnswered {
  decision: "played" | "declined";
  /** False: the decision stands but did not reach the run's ledger on disk. */
  persisted: boolean;
}

export interface BringThroughPlan {
  artifact: string;
  bytes: number;
  /** Frozen when the plan was made: the digest the press refers to. */
  sha256: string;
  landsAs: string;
  receipt: ReceiptView;
  conflict: "none" | "alreadyHad" | "nameTaken";
  /** 0.8: the earlier build an Update replaces, and what it does to its installs. */
  earlier: EarlierView | null;
}

/** 0.8: one install the Update swaps, the earlier build's option for the new one's. */
export interface SwapView {
  fromOptionId: string;
  toOptionId: string;
  name: string;
  fromVersion: string | null;
  toVersion: string | null;
}

/** 0.8: an install of the earlier build that stays: the new build has no option that is the same. */
export interface StayView {
  optionId: string;
  name: string;
}

export interface EarlierView {
  fileName: string;
  packageId: string;
  swaps: SwapView[];
  stays: StayView[];
  /** The earlier zip goes to the older-versions folder. */
  putsAway: boolean;
}

/** 0.8: what an Update press did. */
export interface UpdateResult {
  outcome: "landed" | "alreadyHad";
  earlier: string | null;
  swapped: SwapView[];
  stays: StayView[];
  /** The stone's sentence when a swap was refused: the build is in the mods folder, nothing more changed. */
  failed: string | null;
  putAway: { fileName: string; landedAs: string }[];
  putAwayFailed: string | null;
}

/** 0.8: what the press on Tamsin's update ask did. */
export interface UpdateAnswered {
  decision: "updated" | "declined";
  result: UpdateResult | null;
  /** What Tamsin is told. */
  told: string;
  persisted: boolean;
}

export interface BringThroughResult {
  outcome: "landed" | "alreadyHad";
}

export interface PutBackReview {
  installId: string;
  name: string;
  at: string;
  undo: "available" | "reinstallFromRift";
  /** What the ledger can say: done and the player's, done and another summoning's, or not recorded. */
  known: "yours" | "anotherSummoning" | "uncertain";
}

export interface PutBackPlan {
  runId: string;
  /** That run's own installs still installed and unchanged, newest first, as THE RETURN's plans. */
  remove: UninstallPlanView[];
  review: PutBackReview[];
  /** The press's own promise. */
  sentence: string;
}

export const WORKSHOP_EVENTS = {
  pawnOutput: "rift://pawn-output",
  pawnState: "rift://pawn-state",
  pawnHands: "rift://pawn-hands",
  workshopChanged: "rift://workshop-changed",
} as const;

export type WorkshopEvent = (typeof WORKSHOP_EVENTS)[keyof typeof WORKSHOP_EVENTS];

/** `rift://pawn-output`: base64 bytes of one run; `seq` starts at 0 and rises by one. */
export interface PawnOutput {
  runId: string;
  seq: number;
  data: string;
}
