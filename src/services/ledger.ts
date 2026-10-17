// The honest sentences (v0.2): what a plan will do to each file, what an
// uninstall or a putting-back actually did, and how a cut-short act was
// undone. Every count the spine returns is said out loud — a number the
// window received and kept quiet about is a lie by omission. Pure: no Vue,
// no backend, so the specs read the exact copy.
// v0.3: what Play is about to do (its route and its warnings, which never
// block), and what a download into the mods folder came to.

import type {
  DownloadPhase,
  DownloadView,
  LaunchRoute,
  LaunchWarning,
  NexusLabels,
  ParkedFileView,
  ParkedRestoreReport,
  RecoveryNote,
  RemovalAction,
  SearchResult,
  UninstallResultView,
  VeilMode,
} from "@/types/rift";
import { formatBytes, formatDate, plural } from "./format";
import type { Voiced } from "./voice";

export type RemovalTone = "gone" | "back" | "skip";

export interface RemovalWords {
  glyph: string;
  verb: string;
  tone: RemovalTone;
}

/** One removal row of THE RETURN: glyph and word both carry the verb (§7.9). */
export function removalWords(action: RemovalAction): RemovalWords {
  switch (action.kind) {
    case "delete":
      return { glyph: "−", verb: "removed", tone: "gone" };
    case "alreadyGone":
      return { glyph: "∅", verb: "already gone from your game: nothing to remove", tone: "skip" };
    case "restoreBackup":
      return { glyph: "↩", verb: `restores ${action.owner}'s file`, tone: "back" };
    case "handDown":
      return { glyph: "↘", verb: `handed down to ${action.toOptionName}`, tone: "back" };
    case "keepChanged":
      return { glyph: "·", verb: "kept: changed since install", tone: "skip" };
    case "keepVanilla":
      return { glyph: "·", verb: "kept: the game's own file", tone: "skip" };
  }
}

const was = (n: number) => (n === 1 ? "was" : "were");
const them = (n: number) => (n === 1 ? "it" : "them");

/** A count and the clause it earns; a zero count says nothing. */
type Clause = readonly [count: number, say: (n: number) => string];

const spoken = (table: readonly Clause[]): string[] => table.filter(([n]) => n > 0).map(([n, say]) => say(n));

/** After THE RETURN: every count the spine reported, none left unsaid. The
 *  handed-down clause goes last, so its "which owns …" never runs into the
 *  next item, and the parts are joined with semicolons. */
export function returnedSummary(result: UninstallResultView): string {
  const parts = spoken([
    [result.filesRemoved, (n) => `${plural(n, "file")} removed`],
    [result.filesRestored, (n) => `${plural(n, "earlier file")} put back`],
    [result.filesKept, (n) => `${plural(n, "file")} left in place (changed since, or the game's own)`],
    [result.filesHandedDown, (n) => `${plural(n, "file")} handed down to the install below, which owns ${them(n)} now`],
  ]);
  const lines = [parts.length ? `${sentenceCase(parts.join("; "))}.` : "Nothing was left to remove."];
  lines.push(
    ...spoken([
      [
        result.parked,
        (n) =>
          `${plural(n, "file")} ${was(n)} kept aside instead of deleted: put ${them(n)} back or discard ${them(n)} under PARKED on Your World.`,
      ],
      [result.restartNeeded ? 1 : 0, () => "Restart the game to see the change."],
    ]),
  );
  return lines.join(" ");
}

/** THE RETURN's heading: only rows that take a file out of (or put one back
 *  into) the game count as leaving; "already gone" and kept rows are a clause. */
export function returnHeading(removals: readonly { action: RemovalAction }[]): string {
  const leaving = removals.filter((r) => ["delete", "restoreBackup", "handDown"].includes(r.action.kind)).length;
  const gone = removals.filter((r) => r.action.kind === "alreadyGone").length;
  const kept = removals.length - leaving - gone;
  const extra = spoken([
    [gone, (n) => `${n} already gone`],
    [kept, (n) => `${n} kept`],
  ]);
  return [`${plural(leaving, "file")} ${leaving === 1 ? "leaves" : "leave"} your game`, ...extra].join(" · ");
}

/** A parked file's second line: whose bytes, how big, since when. */
export function parkedMeta(file: ParkedFileView): string {
  const size = file.size === null ? "gone from the backup store" : formatBytes(file.size);
  return `${file.owner} · ${size} · kept aside ${formatDate(file.parkedAt)}`;
}

/** The ticket on a parked slip's stub: the store handle is
 *  `backups/<seq>/<gamePath>` (rift-deploy), so the seq is the ticket. A
 *  handle in any other shape falls back to the row's place in the list —
 *  never the handle's last segment, which is the whole file name. */
export function parkedTicket(store: string, index: number): string {
  const seq = store.split("/").at(1) ?? "";
  return /^\d{1,6}$/.test(seq) ? `#${seq}` : `#${index + 1}`;
}

/** The occupied line under a parked path, said before the press. */
export const PARKED_OCCUPIED_LINE = "something sits there now: putting back swaps it out, unless an install owns it";

/** What "Put back" will do, said before the press. */
export function parkedPutBackTitle(file: ParkedFileView): string {
  if (file.size === null) return "The kept file is gone from the backup store: there is nothing to put back.";
  return file.occupied
    ? `Something sits at ${file.gamePath} now: putting this back keeps that one aside in its turn, unless an install owns it — then the stone holds back and says which.`
    : `Puts the file back at ${file.gamePath}.`;
}

/** After restore_parked: where it went and what was swapped out. */
export function parkedRestoreSummary(report: ParkedRestoreReport): string {
  if (!report.restored) return `${report.gamePath} could not be put back. It is still kept aside, untouched.`;
  let line = `${report.gamePath} is back in your game.`;
  if (report.swappedOut) line += " The file that sat there is kept aside in its turn: it is in the PARKED list now.";
  if (report.restartNeeded) line += " Restart the game to load it.";
  return line;
}

const RECOVERY_ACT: Record<RecoveryNote["action"], string> = {
  install: "install",
  uninstall: "uninstall",
  reorder: "reorder",
  restoreParked: "putting-back",
};

/** The engine names a restoreParked intent "the file kept aside for <gamePath>". */
const KEPT_ASIDE_FOR = /^the file kept aside for /i;

/** The World pane's note after a crash was rolled back at launch (RS-4). */
export function recoverySentence(note: RecoveryNote): string {
  const subject = note.action === "restoreParked" ? note.optionName.replace(KEPT_ASIDE_FOR, "") : note.optionName;
  return `The last ${RECOVERY_ACT[note.action]} of ${subject} was cut short. The stone rolled back ${plural(note.filesRolledBack, "file")}; your game folder is as it was before.`;
}

/** The quiet line under the NEXUS facets. */
export function unlabeledLine(packages: number): string {
  return `${plural(packages, "package")} ${packages === 1 ? "carries" : "carry"} no Nexus labels yet.`;
}

/** What one Identify & check press learned, as one toast. */
export interface AskOutcome {
  /** Unlabeled packages before and after the press. */
  before: number;
  after: number;
  /** Any package carried labels before the press. */
  hadLabels: boolean;
  /** The check's failure, if it failed. */
  failure: Voiced | null;
}

export function askNexusToast(o: AskOutcome): { tone: "ember" | "rift"; line: string; learned: number } {
  const learned = Math.max(0, o.before - o.after);
  if (o.failure) {
    const line =
      o.failure.inscription === "THE FAR WORLD IS OUT OF REACH"
        ? "The stone can't reach Nexus right now, so no labels were learned. Try again when you're online."
        : `${o.failure.voice} No labels were learned this time.`;
    return { tone: "ember", line, learned: 0 };
  }
  if (!learned) {
    return {
      tone: "rift",
      line: `Nexus didn't recognise any of the ${plural(o.after, "package")} by MD5 — they may be repacks or come from elsewhere. Find them by name on the NEXUS page and their labels follow.`,
      learned,
    };
  }
  const named = `Nexus named ${plural(learned, o.hadLabels ? "more package" : "package", o.hadLabels ? "more packages" : "packages")}.`;
  const rest = o.after
    ? `${plural(o.after, "package")} Nexus doesn't know ${o.after === 1 ? "stays" : "stay"} unlabeled: find ${them(o.after)} by name on the NEXUS page.`
    : "Every package carries its labels now.";
  return { tone: "rift", line: `${named} ${rest}`, learned };
}

/** What Nexus has said about a mod's content (v0.2). `unknown`: Nexus has not
 *  labelled its package yet — unlinked, linked but never checked, or an
 *  answer that carried no labels. */
export type Adultness = "adult" | "notAdult" | "unknown";

/** A card's three label fields (SearchResult). */
type CardLabels = Pick<SearchResult, "nexusCategory" | "nexusTags" | "adult">;

function asLabels(source: CardLabels | NexusLabels | null): NexusLabels | null {
  if (source === null || !("nexusTags" in source)) return source;
  return { category: source.nexusCategory, tags: source.nexusTags, adult: source.adult };
}

/** The one reading of a mod's adultness, from a card's SearchResult or a
 *  detail's OptionDetail.labels (null: none). "No category, no tags and not
 *  adult" is unknown in either shape: it is what Nexus's silence looks like. */
export function adultnessOf(source: CardLabels | NexusLabels | null): Adultness {
  const labels = asLabels(source);
  if (labels?.adult) return "adult";
  const labelled = Boolean(labels?.category?.trim()) || (labels?.tags.length ?? 0) > 0;
  return labelled ? "notAdult" : "unknown";
}

/** Does this veil mode blur a screenshot? "adult" blurs what Nexus flags
 *  adult AND what Nexus has not labelled yet: a veil exists to protect, so the
 *  unknown is veiled, and it clears as packages are linked and checked
 *  (WINDOW-CONTRACT § v0.2 "Veil", ruled 2026-10-16). The card, the detail and
 *  the lookbook all ask this one question, through adultnessOf. */
export function veils(mode: VeilMode, adultness: Adultness): boolean {
  if (mode === "all") return true;
  return mode === "adult" && adultness !== "notAdult";
}

// ---------------------------------------------------------------------------
// v0.3 — Play
// ---------------------------------------------------------------------------

/** One warning Play carries (WINDOW-CONTRACT § v0.3 Play): never a blocker,
 *  always the reason the player may not see a mod in the game. */
export function launchWarningLine(warning: LaunchWarning): string {
  switch (warning.kind) {
    case "reframeworkMissing":
      return `REFramework is missing from the game folder, so ${plural(warning.paks, "installed pak")} won't load.`;
    case "pakFolderOff":
      return `REFramework's Alternate PAK Folder is off, so ${plural(warning.paks, "installed pak")} won't load.`;
    case "looseLoaderOff":
      return `The Loose File Loader is off, so ${plural(warning.files, "installed loose file")} won't load.`;
    case "buildChanged":
      return `Steam updated the game from build ${warning.recorded} to ${warning.now} since your newest install: script mods are the first to break.`;
  }
}

const LAUNCH_ROUTE: Record<LaunchRoute, string> = {
  steam: "through Steam",
  exe: "from its game folder",
};

/** After launch_game: how the game is starting, then every warning it carries. */
export function launchedLine(route: LaunchRoute, warnings: readonly LaunchWarning[]): string {
  return [`Dragon's Dogma 2 is starting ${LAUNCH_ROUTE[route]}.`, ...warnings.map(launchWarningLine)].join(" ");
}

// ---------------------------------------------------------------------------
// v0.3 — downloads into the mods folder
// ---------------------------------------------------------------------------

/** Each phase in a word or three, for a row or a screen reader. */
export const DOWNLOAD_PHASE_WORD: Record<DownloadPhase, string> = {
  waiting: "waiting for the site's link",
  queued: "queued",
  linking: "asking Nexus for a link",
  downloading: "downloading",
  verifying: "checking the file",
  landed: "in your mods folder",
  failed: "failed",
  cancelled: "cancelled",
};

/** A download's name as the player knows it: the file's Nexus name, else the mod's. */
export function downloadName(d: Pick<DownloadView, "fileName" | "modName" | "modId" | "fileId">): string {
  return d.fileName ?? d.modName ?? `Nexus mod ${d.modId}, file ${d.fileId}`;
}

/** What a download came to once it landed. */
export function landedLine(d: DownloadView): string {
  const name = downloadName(d);
  if (d.alreadyHad) return `${name} was in your mods folder already, so nothing new was written.`;
  const as = d.landedAs ? ` as ${d.landedAs}` : "";
  return `${name} landed in your mods folder${as}. Install it from the Rift.`;
}

function sentenceCase(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}
