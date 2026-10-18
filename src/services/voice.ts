// The Error Manifesto, as one module. Every RiftFailure the window shows
// passes through `voice()`: kind + detail in, three parts out —
//   voice    what happened, in the Rift's register (an inscription + a line),
//   empathy  that the stone understands what you tried, and what did NOT change,
//   recovery the action(s) that get you moving again (always at least one).
// Components never compose failure copy themselves.
//
// v0.3: a "nexus" failure carries a NexusReason code beside its sentence
// (`detail: { detail, reason? }`). The code is read FIRST and decides the
// voice; the sentence's words are read only when no code came (an odd answer,
// or an older spine), through the regexes the v0.2 voice used.

import type {
  InstalledRow,
  NexusReason,
  PackageSummary,
  Problem,
  ProblemKind,
  RiftError,
  SaveReason,
} from "@/types/rift";
import type { RiftFailure } from "./rift";

/** What the user was doing when the failure struck. The specs walk every one
 *  of them, so a context added here is voiced in every kind before it ships. */
export const VOICE_CONTEXTS = [
  "install",
  "uninstall",
  "scan",
  "package",
  "search",
  "reorder",
  "switch",
  "game",
  "nexusCheck",
  "nexusKey",
  "nexusLink",
  // v0.2: Find on Nexus by name. Nothing is linked by a find, so a failure
  // leaves the package exactly as it was.
  "nexusFind",
  "settings",
  // v0.2: Put back / Discard on a PARKED file. The caller adds `gamePath` and
  // `act` ("putBack" | "discard") to the detail.
  "parked",
  // v0.3: Play (plan_launch / launch_game).
  "launch",
  // v0.3: a download into the mods folder (start, cancel, clear, or a row that failed).
  "download",
  // v0.3: validating the stored key again (refresh_nexus_account).
  "nexusAccount",
  // v0.3: browsing Nexus, or reading one mod's files and patch notes.
  "browse",
  // v0.3: tracked mods and endorsements (get_nexus_standings, set_tracked, set_endorsement).
  "standings",
  // v0.3: an nxm:// link that reached the stone, or the handler switch.
  "nxm",
  // v0.3: opening the mods folder in Explorer.
  "modsFolder",
  // v0.4: THE MUSTER, reading a save slot's party (get_muster_slots, get_muster,
  // open_saves_folder). The caller adds `file` (the slot id) to the detail.
  "muster",
  // v0.5: a move to or from mods (older versions) beside the mods folder, or a
  // read of it (put_away, keep_newest's moves, bring_back, get_older_versions,
  // open_older_versions_folder). The caller adds `act` ("putAway" | "bringBack"
  // | "read" | "open") and `file` (the file's own name, never a path).
  "olderVersions",
] as const;

export type VoiceContext = (typeof VOICE_CONTEXTS)[number];

export type RecoveryKind =
  | "retry"
  | "copyDetails"
  | "pickGameFolder"
  | "rescan"
  | "showInFolder"
  | "replaceKey"
  | "openPreflight"
  /** v0.3: the Nexus page the failure belongs to (a file's page for a
   *  download: its Mod Manager Download button makes a fresh link). The
   *  caller knows which page; the voice only offers the door. */
  | "openNexusPage"
  /** v0.4: focus the Muster's slot list, on the newest slot that opens. */
  | "pickSlot"
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
  /** The stone understands what you tried; what is still safe. */
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
const REPLACE_KEY: Recovery = { kind: "replaceKey", label: "Replace key…" };
const ADD_KEY: Recovery = { kind: "replaceKey", label: "Add key…" };
const OPEN_PAGE: Recovery = { kind: "openNexusPage", label: "Open its Nexus page" };

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
};

/** "the stone put back the 4 files already placed" — only when the spine says so. */
function rollbackSentence(d: Detail, context: VoiceContext): string {
  const undone = pick(d, "rolledBack", "restored", "placed", "filesRolledBack");
  const count = Number(undone);
  if (context === "install" && Number.isFinite(count) && count > 0) {
    return `Your game folder is exactly as it was: the stone put back the ${count} ${count === 1 ? "file" : "files"} already placed.`;
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

/** v0.3: the io failures of the new acts, each in its own words. */
const IO_VOICES: Partial<Record<VoiceContext, (why: string) => Voiced>> = {
  launch: (why) => ({
    kind: "io",
    inscription: "THE WORLD STAYS ASLEEP",
    voice: `The stone couldn't start Dragon's Dogma 2: ${why}.`,
    empathy: "Nothing in your game folder changed. Steam can start the game meanwhile.",
    recovery: [RETRY, COPY],
  }),
  download: (why) => ({
    kind: "io",
    inscription: null,
    voice: `The stone couldn't write the download into your mods folder: ${why}.`,
    empathy: "The stone removed the partial file; your mods folder is as it was.",
    recovery: [RETRY, COPY],
  }),
  modsFolder: (why) => ({
    kind: "io",
    inscription: null,
    voice: `The stone couldn't open your mods folder: ${why}.`,
    empathy: "Nothing in it changed. Its path is on PREFLIGHT, so Explorer can open it too.",
    recovery: [RETRY, COPY],
  }),
  nxm: (why) => ({
    kind: "io",
    inscription: null,
    voice: `The stone couldn't change which program answers nxm:// links: ${why}.`,
    empathy: "The Windows setting is as it was.",
    recovery: [RETRY, COPY],
  }),
};

/** v0.5 (#00015 §18.11): a move the stone could not make names the file by its
 *  own name and says where it still is. */
function olderVersionsIo(d: Detail): Voiced {
  const why = stripStop(reasonOf(d, "Windows refused the move"));
  const file = pick(d, "file") ?? "the archive";
  const act = pick(d, "act");
  const io = (voice: string, empathy: string, recovery: Recovery[] = [RETRY, COPY]): Voiced => ({
    kind: "io",
    inscription: null,
    voice,
    empathy,
    recovery,
  });
  if (act === "bringBack") {
    return io(
      `The stone couldn't move ${file} back into your mods folder: ${why}.`,
      "It is still in mods (older versions), exactly as it was.",
    );
  }
  if (act === "putAway") {
    return io(
      `The stone couldn't move ${file} to mods (older versions): ${why}.`,
      "It is still in your mods folder, exactly as it was.",
    );
  }
  if (act === "open") {
    return io(`The stone couldn't open mods (older versions): ${why}.`, "Nothing in it was changed.");
  }
  return io(`The stone couldn't look inside mods (older versions): ${why}.`, "Nothing in it was changed.", [RETRY]);
}

function ioFailure(d: Detail, context: VoiceContext): Voiced {
  if (context === "olderVersions") return olderVersionsIo(d);
  if (CROSSING_INSCRIPTION[context]) return writeFailure(d, context);
  const own = IO_VOICES[context];
  if (own) return own(stripStop(reasonOf(d, "Windows refused")));
  return {
    kind: "io",
    inscription: null,
    voice: `The stone couldn't read or write a file: ${stripStop(reasonOf(d, "the disk refused"))}.`,
    empathy: "Nothing in your game folder or your mods folder was changed.",
    recovery: [RETRY, COPY],
  };
}

function archiveFailure(kind: RiftError["kind"], d: Detail): Voiced {
  const name = pathOf(d).split(/[\\/]/).at(-1) ?? pathOf(d);
  const why: Partial<Record<RiftError["kind"], string>> = {
    corruptArchive: "the archive ends early (truncated download?)",
    unreadable: stripStop(reasonOf(d, "Windows would not let it be read")),
    unsupportedFormat: "it isn't a zip, 7z, rar or folder",
    encrypted: "it is password-protected",
  };
  return {
    kind,
    inscription: null,
    voice: `The stone couldn't open \`${name}\`: ${why[kind] ?? "it would not open"}.`,
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
    voice: "That mod has left your mods folder since the last scan.",
    empathy: "Nothing was written. A rescan brings the list up to date.",
    recovery: [RESCAN],
  };
}

function noGame(): Voiced {
  return {
    kind: "gameNotFound",
    inscription: "NO WORLD ANSWERS",
    voice: "The stone searched every Steam library on this PC and found no Dragon's Dogma 2 install.",
    empathy: "If it lives somewhere unusual, point the stone at it.",
    recovery: [{ kind: "pickGameFolder", label: "Choose game folder…" }, RETRY],
  };
}

/** The engine's running-game wording ("Dragon's Dogma 2 is running and holds
 *  … open", Windows' "being used by another process") — not the bare word
 *  "open", which a path like opening_title.tex carries too. */
const RUNNING =
  /\b(?:is|are)(?: already)? running\b|\bin use\b|being used by another process|\bholds\b.{0,120}\bopen\b/i;

const CLOSE_AND_RETRY = "Nothing was changed. Close Dragon's Dogma 2, then try again.";

/** What a refusal left as it was, by what was refused. */
const REFUSED_EMPATHY: Partial<Record<VoiceContext, string>> = {
  reorder: CLOSE_AND_RETRY,
  switch: CLOSE_AND_RETRY,
  launch: "Nothing was started, and your game folder is as it was.",
  download: "Nothing in your mods folder changed.",
  nxm: "Nothing was downloaded, and no setting changed.",
  modsFolder: "Nothing in your mods folder changed.",
  standings: "Nothing changed on Nexus.",
};

/** A refusal these acts meet is a fact about the moment, not a press that
 *  failed: trying the same press again cannot help. */
const REFUSED_FOR_GOOD: ReadonlySet<VoiceContext> = new Set(["download", "nxm", "standings"]);

function refused(d: Detail, context: VoiceContext): Voiced {
  if (context === "parked") return parkedRefused(d);
  // v0.5 (#00015 §18.11): the spine's sentence, verbatim, and what it left alone
  if (context === "olderVersions") {
    return {
      kind: "refused",
      inscription: null,
      voice: reasonOf(d, "The stone held the move back."),
      empathy: "Nothing was moved.",
      recovery: [DISMISS],
    };
  }
  const reason = reasonOf(d, "the game is running");
  const running = RUNNING.test(reason);
  return {
    kind: "refused",
    inscription: running ? "THE WORLD IS AWAKE" : null,
    voice: `The stone held back: ${stripStop(reason)}.`,
    empathy: REFUSED_EMPATHY[context] ?? "Nothing was written. Close Dragon's Dogma 2 if it's running, then try again.",
    recovery: REFUSED_FOR_GOOD.has(context) ? [DISMISS] : [RETRY],
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

// ---------------------------------------------------------------------------
// The far world — Nexus. The reason code first; the words only as a fallback.
// ---------------------------------------------------------------------------

const NEXUS_REASONS: readonly NexusReason[] = [
  "offline",
  "rateLimited",
  "noKey",
  "keyRejected",
  "premiumRequired",
  "linkExpired",
  "linkNotYours",
  "linkRefused",
  "fileGone",
  "sizeMismatch",
  "notTheFile",
  "endorseNotDownloaded",
  "endorseTooSoon",
  "endorseOwnMod",
];

/** The spine's reason code, when the detail carries a known one. */
export function nexusReasonOf(d: Detail): NexusReason | null {
  const raw = pick(d, "reason");
  return NEXUS_REASONS.find((r) => r === raw) ?? null;
}

/** The spine's sentence for a nexus failure: its `detail`, never the code. A
 *  `reason` that is not a code (an older spine's free text) is a sentence too. */
function nexusSentence(d: Detail, fallback: string): string {
  const reason = pick(d, "reason");
  const sentence = pick(d, "detail", "message", "error");
  if (sentence) return sentence;
  return reason && !nexusReasonOf(d) ? reason : fallback;
}

const NEXUS_REFUSED = /\b(401|403)\b|reject|invalid key|unauthori[sz]ed|forbidden/i;
const NEXUS_QUOTA = /\b429\b|quota|rate limit/i;
/** An answer the spine could not use means Nexus WAS reached: not offline. */
const NEXUS_UNREADABLE = /could not use|not graphql|unexpected shape|HTTP status \d{3}/i;
/** Words that say Nexus was never reached (v0.2's offline sentences carried no code). */
const NEXUS_UNREACHED =
  /could not be reached|can(?:no|')t reach|connection|timed? ?out|unreachable|offline|\bdns\b|network/i;

/** Every way the far world can fail: a reason code, an answer the stone could
 *  not read, or (linking only) a link that leads nowhere. */
export type FarCase = NexusReason | "odd" | "badLink";

/** The code when one came; else what the sentence's words say (the v0.2
 *  voice). Without a code, "out of reach" needs words that say so: since v0.3
 *  the spine codes every outage `offline`, so a failure with no code and no
 *  such words is one where Nexus answered (no mirror offered, a 401/403 on a
 *  working key) — never THE FAR WORLD IS OUT OF REACH. */
function farCase(d: Detail, context: VoiceContext): FarCase {
  const coded = nexusReasonOf(d);
  if (coded) return coded;
  const raw = Object.values(d).join(" ");
  if (NEXUS_QUOTA.test(raw)) return "rateLimited";
  if (NEXUS_REFUSED.test(raw)) return "keyRejected";
  if (NEXUS_UNREADABLE.test(raw)) return "odd";
  if (NEXUS_UNREACHED.test(raw)) return "offline";
  return context === "nexusLink" ? "badLink" : "odd";
}

/** The far-world case a nexus failure is voiced by: its reason code, else
 *  what its words say. Null for a failure of another kind. A page reads it to
 *  tell a key Nexus refused from a Nexus it could not reach. */
export function farCaseOf(failure: Pick<RiftFailure, "kind" | "detail">, context: VoiceContext): FarCase | null {
  return failure.kind === "nexus" ? farCase(failure.detail, context) : null;
}

/** What a far-world failure left as it was, by what was being done. */
const FAR_UNCHANGED: Partial<Record<VoiceContext, string>> = {
  nexusCheck: "Update status below is from the last check.",
  nexusKey: "Nothing else changed.",
  nexusLink: "The package stays as it was.",
  download: "Nothing was written to your mods folder.",
  nexusAccount: "Your key is still stored, and nothing else changed.",
  browse: "Your mods folder is as it was.",
  standings: "Nothing changed on Nexus.",
  nxm: "Nothing was downloaded.",
};

const unchanged = (context: VoiceContext): string =>
  FAR_UNCHANGED[context] ?? "Nothing in your game folder or your mods folder changed.";

type FarVoice = (d: Detail, context: VoiceContext) => Voiced;

const far = (inscription: string | null, voiceLine: string, empathy: string, recovery: Recovery[]): Voiced => ({
  kind: "nexus",
  inscription,
  voice: voiceLine,
  empathy,
  recovery,
});

/** The press the site's Mod Manager Download button gives again. */
const FRESH_LINK = "Press Mod Manager Download on the file's Nexus page again for a fresh one.";

const FAR_VOICES: Record<Exclude<FarCase, "badLink">, FarVoice> = {
  offline: (_d, c) =>
    far(
      "THE FAR WORLD IS OUT OF REACH",
      "The stone can't reach Nexus right now.",
      `${unchanged(c)} Everything else works offline.`,
      [RETRY],
    ),
  rateLimited: (d, c) =>
    far(
      null,
      `Nexus has rationed the stone for now: ${stripStop(nexusSentence(d, "the hourly request quota is spent")).replace(/^rate limit:\s*/i, "")}.`,
      `${unchanged(c)} Nothing was lost.`,
      [RETRY],
    ),
  noKey: (_d, c) =>
    far(
      null,
      // §9: a download or an arriving link says what the key is for here
      c === "download" || c === "nxm"
        ? "The stone needs your Nexus key before it can take a download."
        : "That needs your Nexus personal API key, and the stone holds none.",
      `${unchanged(c)} Update checks, browsing and patch notes work without one.`,
      [ADD_KEY],
    ),
  keyRejected: (_d, c) =>
    far("THE FAR WORLD REFUSES", "Nexus didn't accept that key: it may have been regenerated.", unchanged(c), [
      REPLACE_KEY,
    ]),
  premiumRequired: (_d, c) =>
    far(
      null,
      "Nexus lets only premium accounts download straight into a mod manager.",
      `${unchanged(c)} Press Mod Manager Download on the file's Nexus page and the stone takes it from there, while it answers Nexus's button.`,
      [OPEN_PAGE],
    ),
  linkExpired: (_d, c) =>
    far(null, "That Mod Manager Download link has expired.", `${unchanged(c)} ${FRESH_LINK}`, [OPEN_PAGE]),
  linkNotYours: (_d, c) =>
    far(
      null,
      "That link was made for a different Nexus account than the key in the stone.",
      `${unchanged(c)} Sign in to Nexus with that key's account, or replace the key, then press Mod Manager Download again.`,
      [OPEN_PAGE, REPLACE_KEY],
    ),
  linkRefused: (_d, c) =>
    far(null, "Nexus refused that link for this file.", `${unchanged(c)} ${FRESH_LINK}`, [OPEN_PAGE]),
  fileGone: (_d, c) =>
    far(
      null,
      "That file is gone from Nexus: its author removed or archived it.",
      `${unchanged(c)} The mod's other files may still be there.`,
      [OPEN_PAGE],
    ),
  sizeMismatch: () =>
    far(
      null,
      "The download arrived at the wrong size, so the stone let it go.",
      "Nothing was left in your mods folder. A second try usually arrives whole.",
      [RETRY],
    ),
  notTheFile: () =>
    far(
      null,
      "What arrived is not the file that was asked for: its fingerprint names another file on Nexus.",
      "The stone let it go; nothing was left in your mods folder.",
      [RETRY, COPY],
    ),
  endorseNotDownloaded: () =>
    far(
      null,
      "Nexus takes an endorsement only from an account that downloaded the mod from Nexus.",
      "Nothing changed on Nexus. Download it with this account first, then endorse it.",
      [DISMISS],
    ),
  endorseTooSoon: () =>
    far(
      null,
      "Nexus wants 15 minutes between a download and an endorsement.",
      "Nothing changed on Nexus. Endorse it again in a little while.",
      [RETRY],
    ),
  endorseOwnMod: () =>
    far(null, "Nexus does not let an author endorse their own mod.", "Nothing changed on Nexus.", [DISMISS]),
  odd: (_d, c) =>
    far(null, "Nexus answered, but the stone could not read its answer.", `${unchanged(c)} Try again later.`, [RETRY]),
};

function nexusLink(d: Detail): Voiced {
  return far(
    null,
    `That link didn't lead to a Dragon's Dogma 2 mod: ${stripStop(nexusSentence(d, "no mod id in it"))}.`,
    "The package stays as it was. Paste a nexusmods.com/dragonsdogma2/mods/… link or a bare mod id.",
    [RETRY],
  );
}

/** Find on Nexus could not ask. Offline is the far world's absence, not the
 *  player's mistake: the package stays as it was, and a URL still links it.
 *  The find is anonymous (it sends no key), so a refusal is the search turned
 *  away, never a key; and an answer the spine could not use means Nexus WAS
 *  reached, so it is not voiced as offline. */
const UNCHANGED = "Nothing was linked; the package stays as it was.";

function findQuota(d: Detail): Voiced {
  // the spine's detail is "rate limit: <its own sentence>"; its retry hint stands as the voice
  const reason = stripStop(nexusSentence(d, "")).replace(/^rate limit:\s*/i, "");
  const wait = /try again in ([^.;]+)/i.exec(reason)?.[1]?.trim();
  return far(
    null,
    wait ? `Nexus asked the stone to slow down: try again in ${wait}.` : "Nexus asked the stone to slow down for now.",
    `${UNCHANGED} Find again after that, or paste its Nexus page.`,
    [RETRY],
  );
}

const FIND_VOICES: Partial<Record<FarCase, (d: Detail) => Voiced>> = {
  rateLimited: findQuota,
  keyRejected: () =>
    far(
      "THE FAR WORLD REFUSES",
      "Nexus turned the search away, so it could not be asked about this package.",
      `The search sends no key, so no key of yours was refused. ${UNCHANGED} Try again later, or paste its Nexus page.`,
      [RETRY],
    ),
  odd: () =>
    far(
      null,
      "Nexus answered, but the stone could not read its answer.",
      `${UNCHANGED} Try again later; its Nexus page's URL still links it.`,
      [RETRY],
    ),
};

function nexusFind(d: Detail, which: FarCase): Voiced {
  const own = FIND_VOICES[which];
  if (own) return own(d);
  return far(
    "THE FAR WORLD IS OUT OF REACH",
    "The stone can't reach Nexus right now, so it could not be asked about this package.",
    `${UNCHANGED} Find again when you're online, or paste its Nexus page.`,
    [RETRY],
  );
}

function nexusFailure(d: Detail, context: VoiceContext): Voiced {
  const which = farCase(d, context);
  if (context === "nexusFind") return nexusFind(d, which);
  if (which === "badLink") return nexusLink(d);
  return FAR_VOICES[which](d, context);
}

// ---------------------------------------------------------------------------
// v0.4 — THE MUSTER (#00015 R17.22, R17.24). A save failure is voiced by its
// reason first; the spine's sentence is read only when no known reason came.
// `file` is the slot's file name (its id), never a path: the path holds the
// Steam account number.
// ---------------------------------------------------------------------------

const READ_AGAIN: Recovery = { kind: "retry", label: "Read again" };
const PICK_SLOT: Recovery = { kind: "pickSlot", label: "Pick another slot" };

const SAVE_REASONS: readonly SaveReason[] = ["otherAccount", "beingWritten", "damaged", "noParty", "gone"];

/** The spine's reason code for a save failure, when the detail carries a known one. */
export function saveReasonOf(d: Detail): SaveReason | null {
  const raw = pick(d, "reason");
  return SAVE_REASONS.find((r) => r === raw) ?? null;
}

const fileOf = (d: Detail) => `\`${pick(d, "file", "slotId") ?? "that save"}\``;

const save = (inscription: string, voiceLine: string, empathy: string, recovery: Recovery[]): Voiced => ({
  kind: "save",
  inscription,
  voice: voiceLine,
  empathy,
  recovery,
});

const READ_ONLY = "The stone only reads saves; nothing was changed.";
export const MUSTER_UNREAD = "THE STONE COULDN'T READ THE PARTY";

const SAVE_VOICES: Record<SaveReason, (file: string) => Voiced> = {
  otherAccount: (file) =>
    save(
      "ANOTHER ARISEN'S SAVE",
      `${file} belongs to a Steam account the stone didn't find on this PC, so it can't open it: it was probably copied in from another account.`,
      "Nothing was changed.",
      [SHOW, PICK_SLOT],
    ),
  beingWritten: (file) =>
    save(
      "THE GAME IS SAVING",
      `The game is writing ${file} right now, or another program is holding it for a moment.`,
      "The stone reads it again as soon as the save settles; nothing was changed.",
      [READ_AGAIN],
    ),
  damaged: (file) =>
    save(
      "THIS SAVE WON'T OPEN",
      `The stone couldn't read ${file}: the file is damaged inside.`,
      "The stone only reads saves, so it is exactly as it was. If the game can't load it either, load another slot in the game.",
      [SHOW, READ_AGAIN, COPY],
    ),
  noParty: (file) =>
    save("NO PARTY IN THIS SAVE", `${file} opens, but holds no Arisen and no pawns to read.`, "", [PICK_SLOT]),
  gone: (file) =>
    save(
      "THIS SAVE IS GONE",
      `${file} left the save folder after the stone listed it.`,
      "The stone is reading the list again.",
      [PICK_SLOT],
    ),
};

function saveFailure(d: Detail): Voiced {
  const reason = saveReasonOf(d);
  if (reason) return SAVE_VOICES[reason](fileOf(d));
  const sentence = stripStop(pick(d, "detail", "message", "error") ?? "the save gave an answer it could not use");
  return save(MUSTER_UNREAD, `${sentence}.`, READ_ONLY, [READ_AGAIN, COPY]);
}

/** Any other failure while reading the party: the kind's own sentence, then
 *  the Muster's promise, and its two doors (R17.22's last row). */
function musterOther(base: Voiced): Voiced {
  return { ...base, inscription: MUSTER_UNREAD, empathy: READ_ONLY, recovery: [READ_AGAIN, COPY] };
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
  save: (d) => saveFailure(d),
  io: (d, context) => (context === "parked" ? parkedIo(d) : ioFailure(d, context)),
};

/** Give a failure its voice. */
export function voice(failure: Pick<RiftFailure, "kind" | "detail">, context: VoiceContext): Voiced {
  const base = HANDLERS[failure.kind](failure.detail, context);
  const voiced = context === "muster" && failure.kind !== "save" ? musterOther(base) : base;
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

/** The trouble that puts a package on the trouble list: the first of
 * TROUBLE_KINDS it carries, or null when it carries none of them (a missing
 * menu parent, say, is the option's business, not a verdict on the package).
 * The library tree's package rows read their verdict here, so a row and the
 * fold never disagree (R16.2). */
export function troubleProblem(pkg: Pick<PackageSummary, "problems">): Problem | null {
  for (const kind of TROUBLE_KINDS) {
    const found = pkg.problems.find((problem) => problem.kind === kind);
    if (found) return found;
  }
  return null;
}

/** The problem a trouble card speaks for: the first of TROUBLE_KINDS the
 * package carries, else its first problem at all. */
export function leadProblem(pkg: Pick<PackageSummary, "problems">): Problem | null {
  return troubleProblem(pkg) ?? pkg.problems.at(0) ?? null;
}

const lowerFirst = (s: string) => s.charAt(0).toLowerCase() + s.slice(1);

function unreadableLine(name: string, message: string, otherPackages: number): string {
  const why = stripStop(message) || "the archive ends early (truncated download?)";
  const others = otherPackages === 1 ? "The other package is fine." : `The other ${otherPackages} packages are fine.`;
  return `The stone couldn't open ${name}: ${lowerFirst(why)}. ${others}`;
}

const NOTHING_LOADS = "opened, but holds nothing the game loads";

/** The verdict for each package that opens, worded by the trouble that put it
 *  on the list. The window owns the verdict and the spine's message is only the
 *  why after the colon, so a card states its verdict once (Chaos #00129 D3). */
const OPENS_WITH: Partial<Record<ProblemKind, string>> = {
  nothingToInstall: NOTHING_LOADS,
  outdatedFormats: "opens, but part of it won't load",
  filesSkipped: "opens, but some of its files stay out",
};

/** One card's line, worded for the problem that put the package on the list. */
export function troubleLine(pkg: Pick<PackageSummary, "fileName" | "problems">, otherPackages: number): TroubleLine {
  const problem = leadProblem(pkg);
  const name = `\`${pkg.fileName}\``;
  if (!problem) return { kind: null, ember: false, line: `${name} ${NOTHING_LOADS}.` };
  const message = problem.message.trim();
  if (problem.kind === "unreadable") {
    return { kind: problem.kind, ember: true, line: unreadableLine(name, message, otherPackages) };
  }
  const verdict = OPENS_WITH[problem.kind];
  // a kind the trouble list never leads with carries its own full sentence
  if (!verdict)
    return { kind: problem.kind, ember: false, line: message ? `${name}: ${message}` : `${name} ${NOTHING_LOADS}.` };
  const why = message || (problem.kind === "nothingToInstall" ? "no mod files inside." : "");
  return { kind: problem.kind, ember: false, line: why ? `${name} ${verdict}: ${why}` : `${name} ${verdict}.` };
}

/** R16.26: provenance, said in words and a title, never a chip or a colour. */
export const ADOPTED_TITLE = "The stone took this install over from files that were already in your game.";
/** R16.26: an install whose package left the mods folder. */
export const PACKAGE_GONE_LINE = "its package isn't in your mods folder";

/** Why an installed row has no package, worded for the two ways that happens:
 *  an adopted install that no package in the mods folder matches, or an
 *  install whose package has left the folder. The option id is opaque: the
 *  row's origin says which, never the id's spelling. */
export function missingPackageTitle(row: Pick<InstalledRow, "origin">): string {
  return row.origin === "adopted"
    ? "Adopted: no package in your mods folder matches it. Uninstall still works: the journal knows every file."
    : "The package is no longer in your mods folder. Uninstall still works: the journal knows every file.";
}
