// The Error Manifesto, as one module. Every RiftFailure the window shows
// passes through `voice()`: kind + detail in, three parts out —
//   voice    what happened, in the Rift's register (an inscription + a line),
//   empathy  that we understand what you tried, and what did NOT change,
//   recovery the action(s) that get you moving again (always at least one).
// Components never compose failure copy themselves.

import {
  FLUFFY_STAND_IN_PREFIX,
  type InstalledRow,
  type PackageSummary,
  type Problem,
  type ProblemKind,
  type RiftError,
} from "@/types/rift";
import type { RiftFailure } from "./rift";

/** What the user was doing when the failure struck. */
export type VoiceContext =
  | "install"
  | "uninstall"
  | "scan"
  | "package"
  | "search"
  | "reorder"
  | "adopt"
  | "switch"
  | "roots"
  | "game"
  | "nexusCheck"
  | "nexusKey"
  | "nexusLink"
  /** v0.2: Find on Nexus by name. Nothing is linked by a find, so a failure
   *  leaves the package exactly as it was. */
  | "nexusFind"
  | "settings"
  /** v0.2: Put back / Discard on a PARKED file. The caller adds `gamePath`
   *  and `act` ("putBack" | "discard") to the detail. */
  | "parked";

export type RecoveryKind =
  | "retry"
  | "copyDetails"
  | "pickGameFolder"
  | "rescan"
  | "showInFolder"
  | "replaceKey"
  | "openPreflight"
  | "dismiss";

export interface Recovery {
  kind: RecoveryKind;
  label: string;
}

export interface Voiced {
  kind: RiftError["kind"];
  /** Cinzel inscription, or null when the line stands alone (§9 rows with "—"). */
  inscription: string | null;
  /** What happened. */
  voice: string;
  /** We understand what you tried; what is still safe. */
  empathy: string;
  /** Always ≥ 1. */
  recovery: Recovery[];
}

type Detail = Record<string, string>;

const RETRY: Recovery = { kind: "retry", label: "Try again" };
const COPY: Recovery = { kind: "copyDetails", label: "Copy details" };
const RESCAN: Recovery = { kind: "rescan", label: "Rescan" };
const SHOW: Recovery = { kind: "showInFolder", label: "Show in folder" };
const DISMISS: Recovery = { kind: "dismiss", label: "Dismiss" };

/** The first non-empty detail value among `keys`. */
export function pick(detail: Detail, ...keys: string[]): string | null {
  const entries = new Map(Object.entries(detail));
  for (const key of keys) {
    const value = entries.get(key);
    if (value?.trim()) return value.trim();
  }
  return null;
}

const pathOf = (d: Detail) => pick(d, "path", "file", "gamePath", "package", "entry") ?? "that file";
const reasonOf = (d: Detail, fallback: string) => pick(d, "reason", "detail", "message", "error") ?? fallback;
const stripStop = (s: string) => s.replace(/[.\s]+$/, "");

const CROSSING_INSCRIPTION: Partial<Record<VoiceContext, string>> = {
  install: "THE CROSSING FAILED",
  uninstall: "THE RETURN FAILED",
  reorder: "THE ORDER HELD",
  adopt: "THE ADOPTION FAILED",
};

/** "we put back the 4 files already placed" — only when the spine says so. */
function rollbackSentence(d: Detail, context: VoiceContext): string {
  const undone = pick(d, "rolledBack", "restored", "placed", "filesRolledBack");
  const count = Number(undone);
  if (context === "install" && Number.isFinite(count) && count > 0) {
    return `Your game folder is exactly as it was: we put back the ${count} ${count === 1 ? "file" : "files"} already placed.`;
  }
  return "Your game folder is exactly as it was.";
}

function writeFailure(d: Detail, context: VoiceContext): Voiced {
  const verb = context === "uninstall" ? "remove" : "write";
  return {
    kind: "io",
    inscription: CROSSING_INSCRIPTION[context] ?? null,
    voice: `Couldn't ${verb} \`${pathOf(d)}\`: ${stripStop(reasonOf(d, "Windows refused the write"))}.`,
    empathy: `${rollbackSentence(d, context)} Close the game if it's running.`,
    recovery: [RETRY, COPY],
  };
}

function ioFailure(d: Detail, context: VoiceContext): Voiced {
  if (CROSSING_INSCRIPTION[context]) return writeFailure(d, context);
  return {
    kind: "io",
    inscription: null,
    voice: `The stone couldn't read or write a file: ${stripStop(reasonOf(d, "the disk refused"))}.`,
    empathy: "Nothing in your game folder or your library was changed.",
    recovery: [RETRY, COPY],
  };
}

function archiveFailure(kind: RiftError["kind"], d: Detail): Voiced {
  const name = pathOf(d).split(/[\\/]/).at(-1) ?? pathOf(d);
  const why: Partial<Record<RiftError["kind"], string>> = {
    corruptArchive: "the archive ends early (truncated download?)",
    unreadable: stripStop(reasonOf(d, "Windows would not let us read it")),
    unsupportedFormat: "it isn't a zip, 7z, rar or folder the stone can read",
    encrypted: "it is password-protected, so the stone can't look inside",
  };
  return {
    kind,
    inscription: null,
    voice: `We couldn't open \`${name}\`: ${why[kind] ?? "it would not open"}.`,
    empathy: "The rest of your library is fine, and nothing was written.",
    recovery: [SHOW, RESCAN],
  };
}

function unsafePath(d: Detail): Voiced {
  return {
    kind: "unsafeEntryPath",
    inscription: null,
    voice: `\`${pathOf(d)}\` tries to place a file outside the game folder, so the stone refused it.`,
    empathy: "Nothing was written. The archive itself is untouched.",
    recovery: [SHOW, COPY],
  };
}

function goneFromLibrary(): Voiced {
  return {
    kind: "entryNotFound",
    inscription: null,
    voice: "That mod has left the library since the last scan.",
    empathy: "Nothing was written. A rescan brings the list up to date.",
    recovery: [RESCAN],
  };
}

function noGame(): Voiced {
  return {
    kind: "gameNotFound",
    inscription: "NO WORLD ANSWERS",
    voice: "We searched every Steam library on this PC and found no Dragon's Dogma 2 install.",
    empathy: "If it lives somewhere unusual, point the stone at it.",
    recovery: [{ kind: "pickGameFolder", label: "Choose game folder…" }, RETRY],
  };
}

/** The engine's running-game wording ("Dragon's Dogma 2 is running and holds
 *  … open", Windows' "being used by another process") — not the bare word
 *  "open", which a path like opening_title.tex carries too. */
const RUNNING = /\b(?:is|are) running\b|\bin use\b|being used by another process|\bholds\b.{0,120}\bopen\b/i;

function refused(d: Detail, context: VoiceContext): Voiced {
  if (context === "parked") return parkedRefused(d);
  const reason = reasonOf(d, "the game is running");
  const running = RUNNING.test(reason);
  return {
    kind: "refused",
    inscription: running ? "THE WORLD IS AWAKE" : null,
    voice: `The stone held back: ${stripStop(reason)}.`,
    empathy:
      context === "reorder" || context === "switch"
        ? "Nothing was changed. Close Dragon's Dogma 2, then try again."
        : "Nothing was written. Close Dragon's Dogma 2 if it's running, then try again.",
    recovery: [RETRY],
  };
}

/** A parked refusal is the spine's own sentence; game-running advice only
 *  when that is what the sentence says, never a retry that cannot succeed. */
function parkedRefused(d: Detail): Voiced {
  const reason = reasonOf(d, "the stone could not act on the kept file");
  const running = RUNNING.test(reason);
  return {
    kind: "refused",
    inscription: running ? "THE WORLD IS AWAKE" : null,
    voice: `The stone held back: ${stripStop(reason)}.`,
    empathy: running
      ? "Nothing was moved: the file is still kept aside. Close Dragon's Dogma 2, then try again."
      : "Nothing was moved, and the PARKED list has been read again.",
    recovery: running ? [RETRY] : [DISMISS],
  };
}

const OS_ERROR_TAIL = /\s*\(os error \d+\)\s*\.?\s*$/i;

/** The engine's io detail is "<context>: <os text>": keep only the os text. */
function parkedReason(d: Detail): string {
  const raw = reasonOf(d, "Windows refused the move");
  const os = /^could not [^:]*:\s*([\s\S]+)$/i.exec(raw)?.[1] ?? raw;
  return stripStop(os.replace(OS_ERROR_TAIL, ""));
}

function parkedIo(d: Detail): Voiced {
  const path = pick(d, "gamePath") ?? "the kept file";
  const reason = parkedReason(d);
  if (pick(d, "act") === "discard") {
    return {
      kind: "io",
      inscription: null,
      voice: `Couldn't let go of the kept copy of ${path}: ${reason}.`,
      empathy: "It is still kept aside, and your game folder was not touched.",
      recovery: [RETRY, COPY],
    };
  }
  return {
    kind: "io",
    inscription: null,
    voice: `Couldn't put ${path} back: ${reason}.`,
    empathy:
      "It is still kept aside; your game folder is as it was. Close the game if it holds the file, then Put back again.",
    recovery: [RETRY, COPY],
  };
}

function journalFailure(d: Detail, context: VoiceContext): Voiced {
  return {
    kind: "journal",
    inscription: CROSSING_INSCRIPTION[context] ?? "THE LEDGER WON'T OPEN",
    voice: `The stone couldn't write its install journal: ${stripStop(reasonOf(d, "the app data folder refused"))}.`,
    empathy: "No game file was touched: the journal is written before the first byte moves.",
    recovery: [RETRY, COPY],
  };
}

const NEXUS_REFUSED = /\b(401|403)\b|reject|invalid key|unauthori[sz]ed|forbidden/i;
const NEXUS_QUOTA = /\b429\b|quota|rate limit/i;

function nexusRefused(): Voiced {
  return {
    kind: "nexus",
    inscription: "THE FAR WORLD REFUSES",
    voice: "Nexus didn't accept that key: it may have been regenerated.",
    empathy: "Nothing else changed.",
    recovery: [{ kind: "replaceKey", label: "Replace key…" }],
  };
}

function nexusQuota(d: Detail): Voiced {
  return {
    kind: "nexus",
    inscription: null,
    voice: `Nexus has rationed the stone for now: ${stripStop(reasonOf(d, "the hourly request quota is spent"))}.`,
    empathy: "Update status below is from the last check. Nothing was lost.",
    recovery: [RETRY],
  };
}

function nexusOffline(): Voiced {
  return {
    kind: "nexus",
    inscription: "THE FAR WORLD IS OUT OF REACH",
    voice: "We can't reach Nexus right now.",
    empathy: "Update status below is from the last check. Everything else works offline.",
    recovery: [RETRY],
  };
}

function nexusLink(d: Detail): Voiced {
  return {
    kind: "nexus",
    inscription: null,
    voice: `That link didn't lead to a Dragon's Dogma 2 mod: ${stripStop(reasonOf(d, "no mod id in it"))}.`,
    empathy: "The package stays as it was. Paste a nexusmods.com/dragonsdogma2/mods/… link or a bare mod id.",
    recovery: [RETRY],
  };
}

/** Find on Nexus could not ask. Offline is the far world's absence, not the
 *  player's mistake: the package stays as it was, and a URL still links it.
 *  The find is anonymous (it sends no key), so a 401/403 is the search turned
 *  away, never a key; and an answer the spine could not use means Nexus WAS
 *  reached, so it is not voiced as offline. */
const FIND_UNREADABLE = /could not use|not graphql|unexpected shape|HTTP status \d{3}/i;
const UNCHANGED = "Nothing was linked; the package stays as it was.";

function findQuota(d: Detail): Voiced {
  // the spine's detail is "rate limit: <its own sentence>"; its retry hint stands as the voice
  const reason = stripStop(reasonOf(d, "")).replace(/^rate limit:\s*/i, "");
  const wait = /try again in ([^.;]+)/i.exec(reason)?.[1]?.trim();
  return {
    kind: "nexus",
    inscription: null,
    voice: wait
      ? `Nexus asked the stone to slow down: try again in ${wait}.`
      : "Nexus asked the stone to slow down for now.",
    empathy: `${UNCHANGED} Find again after that, or paste its Nexus page.`,
    recovery: [RETRY],
  };
}

function nexusFind(d: Detail, raw: string): Voiced {
  if (NEXUS_QUOTA.test(raw)) return findQuota(d);
  if (NEXUS_REFUSED.test(raw)) {
    return {
      kind: "nexus",
      inscription: "THE FAR WORLD REFUSES",
      voice: "Nexus turned the search away, so it could not be asked about this package.",
      empathy: `The search sends no key, so no key of yours was refused. ${UNCHANGED} Try again later, or paste its Nexus page.`,
      recovery: [RETRY],
    };
  }
  if (FIND_UNREADABLE.test(raw)) {
    return {
      kind: "nexus",
      inscription: null,
      voice: "Nexus answered, but the stone could not read its answer.",
      empathy: `${UNCHANGED} Try again later; its Nexus page's URL still links it.`,
      recovery: [RETRY],
    };
  }
  return {
    kind: "nexus",
    inscription: "THE FAR WORLD IS OUT OF REACH",
    voice: "We can't reach Nexus right now, so it could not be asked about this package.",
    empathy: `${UNCHANGED} Find again when you're online, or paste its Nexus page.`,
    recovery: [RETRY],
  };
}

function nexusFailure(d: Detail, context: VoiceContext): Voiced {
  const raw = Object.values(d).join(" ");
  if (context === "nexusFind") return nexusFind(d, raw);
  if (NEXUS_REFUSED.test(raw)) return nexusRefused();
  if (NEXUS_QUOTA.test(raw)) return nexusQuota(d);
  if (context === "nexusLink") return nexusLink(d);
  return nexusOffline();
}

type Handler = (detail: Detail, context: VoiceContext) => Voiced;

const HANDLERS: Record<RiftError["kind"], Handler> = {
  unreadable: (d) => archiveFailure("unreadable", d),
  unsupportedFormat: (d) => archiveFailure("unsupportedFormat", d),
  encrypted: (d) => archiveFailure("encrypted", d),
  corruptArchive: (d) => archiveFailure("corruptArchive", d),
  unsafeEntryPath: (d) => unsafePath(d),
  entryNotFound: () => goneFromLibrary(),
  gameNotFound: () => noGame(),
  refused,
  journal: journalFailure,
  nexus: nexusFailure,
  io: (d, context) => (context === "parked" ? parkedIo(d) : ioFailure(d, context)),
};

/** Give a failure its voice. */
export function voice(failure: Pick<RiftFailure, "kind" | "detail">, context: VoiceContext): Voiced {
  const voiced = HANDLERS[failure.kind](failure.detail, context);
  return voiced.recovery.length ? voiced : { ...voiced, recovery: [DISMISS] };
}

/** One line for a toast or an inline notice: "voice empathy". */
export function voicedLine(voiced: Voiced): string {
  return `${voiced.voice} ${voiced.empathy}`;
}

/** The text "Copy details" puts on the clipboard. */
export function failureDetails(failure: Pick<RiftFailure, "kind" | "detail">, context: VoiceContext): string {
  const lines = [`Riftstone failure — ${failure.kind} during ${context}`];
  for (const [key, value] of Object.entries(failure.detail)) lines.push(`${key}: ${value}`);
  return lines.join("\n");
}

// ---------------------------------------------------------------------------
// §9 trouble cards — the packages get_troubled_packages returns
// ---------------------------------------------------------------------------

/** The problems that put a package on the trouble list, in the spine's order
 * (WINDOW-CONTRACT get_troubled_packages). A card speaks for the first. */
export const TROUBLE_KINDS: readonly ProblemKind[] = [
  "unreadable",
  "nothingToInstall",
  "outdatedFormats",
  "filesSkipped",
];

export interface TroubleLine {
  kind: ProblemKind | null;
  /** Unreadable packages carry the ember rail; the rest are warnings. */
  ember: boolean;
  line: string;
}

/** The problem a trouble card speaks for: the first of TROUBLE_KINDS the
 * package carries, else its first problem at all. */
export function leadProblem(pkg: Pick<PackageSummary, "problems">): Problem | null {
  for (const kind of TROUBLE_KINDS) {
    const found = pkg.problems.find((problem) => problem.kind === kind);
    if (found) return found;
  }
  return pkg.problems.at(0) ?? null;
}

const lowerFirst = (s: string) => s.charAt(0).toLowerCase() + s.slice(1);

function unreadableLine(name: string, message: string, otherPackages: number): string {
  const why = stripStop(message) || "the archive ends early (truncated download?)";
  const others = otherPackages === 1 ? "The other package is fine." : `The other ${otherPackages} packages are fine.`;
  return `We couldn't open ${name}: ${lowerFirst(why)}. ${others}`;
}

/** Packages that open, worded by the trouble that put them on the list. */
const OPENS_WITH: Partial<Record<ProblemKind, string>> = {
  outdatedFormats: "opens, but part of it won't load",
  filesSkipped: "opens, but some of its files stay out",
};

/** One card's line, worded for the problem that put the package on the list. */
export function troubleLine(pkg: Pick<PackageSummary, "fileName" | "problems">, otherPackages: number): TroubleLine {
  const problem = leadProblem(pkg);
  const name = `\`${pkg.fileName}\``;
  if (!problem) return { kind: null, ember: false, line: `${name} opened, but holds nothing the game loads.` };
  const message = problem.message.trim();
  if (problem.kind === "unreadable") {
    return { kind: problem.kind, ember: true, line: unreadableLine(name, message, otherPackages) };
  }
  const opens = OPENS_WITH[problem.kind];
  const line = opens
    ? `${name} ${opens}: ${message}`
    : `${name} opened, but holds nothing the game loads: ${message || "no mod files inside."}`;
  return { kind: problem.kind, ember: false, line };
}

/** Why an installed row has no package, worded for the two ways that happens. */
export function missingPackageTitle(row: Pick<InstalledRow, "optionId">): string {
  return row.optionId.startsWith(FLUFFY_STAND_IN_PREFIX)
    ? "Adopted from Fluffy: no package in your library matches it"
    : "The package is no longer in your library";
}
