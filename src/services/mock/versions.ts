// v0.5 — the mock's versions of one file (#00015 §18.15): the invented family
// Lantern Glow Recolor, v1.3 (the lead), v1.2 and v1.1, all linked to mod 2207
// in one file group, and the rules the spine reads them by (WINDOW-CONTRACT
// § v0.5): the family by mod and group id, the newest by upload time, the same
// option by root, else name, else the one installable option each holds, and
// the ` (2)` a move takes when its name is taken. Every name here is invented.
//
// Scenes, comma-combinable with the rest (./index.ts header):
//   family       v1.3 and v1.2, the same option installed in both (stacked)
//   familyfresh  v1.3 downloaded, not installed; v1.2 installed
//   familythree  family, and v1.1 with nothing installed
//   putaway      familyfresh; installing v1.3's option puts v1.2 away
//   putawaytaken putaway, with a v1.2 already in the folder: it lands as ` (2)`
//   putawayfail  putaway, whose move fails after the install committed
//   putawayhalf  putawayfail with v1.1 too; Try again moves v1.2, and v1.1 still stays
//   undercroft   v1.3 alone in the mods folder, v1.2 in mods (older versions)
//   undercroftempty / undercroftnone   the folder empty / never made
//
// The player's word (#00015 R18.18.8): an unpacked folder, Lantern Glow
// Recolor, linked by mod id as Find links (mod 2207, no file, no group, no
// version, no upload time), beside the pinned v1.3:
//   question          v1.3 alone (installed) and the folder, nothing installed;
//                     the stone asks (all 4 of its file names are in v1.3)
//   questioninstalled the same, the folder installed and v1.3 not
//   questionjoined    after the yes: the folder joined (`source: "joined"`)
//   questionmanual    a folder that shares no name, and a second pinned file of
//                     mod 2207 (Lantern_Glow_Candles-2207-1-0-1730000000.zip)
//   questionrefused   question, whose join answers `refused` once

import type { NexusLink, PackageSummary, VersionQuestion } from "@/types/rift";
import { buildPackage, type MockPackage, type PackageSeed } from "./library";

export const FAMILY_NAME = "Lantern Glow Recolor";
const MOD_ID = 2207;
/** One file group for every version (Nexus gives the versions of one file one group id). */
const GROUP_ID = 220750;
const MAIN_ROOT = "Lantern Glow/Main";
export const MAIN_NAME = "Lantern Glow — Main";
/** An option only v1.2 holds (R18.7). */
export const CANDLE_NAME = "Lantern Glow — Candle Warm";

/** The versions, newest first: `Lantern_Glow_Recolor-2207-1-3-1730540000.zip` and its elders. */
const VERSIONS = [
  { version: "1.3", fileId: 2404, uploaded: 1_730_540_000, size: 1_572_864 },
  { version: "1.2", fileId: 2403, uploaded: 1_729_940_000, size: 1_468_006 },
  { version: "1.1", fileId: 2402, uploaded: 1_729_100_000, size: 1_363_148 },
] as const;

export const familyFileName = (version: string): string => {
  const v = VERSIONS.find((x) => x.version === version);
  const [major, minor] = version.split(".");
  return `Lantern_Glow_Recolor-${MOD_ID}-${major}-${minor}-${v?.uploaded ?? 0}.zip`;
};

const GLOW = (n: number) => ({ rel: `natives/stm/effect/lantern_glow/glow_0${n}.tex.251211553`, size: 349_525 });

function versionSeed(at: (typeof VERSIONS)[number]): PackageSeed {
  const main = {
    root: MAIN_ROOT,
    name: MAIN_NAME,
    info: {
      author: "Wilhelmina's Lamplighter",
      version: at.version,
      description: "Amber lantern light for the road at night, in one main recolor.",
    },
    payload:
      at.version === "1.3"
        ? [{ rel: "LanternGlowRecolor.pak", size: 524_288 }, GLOW(0), GLOW(1), GLOW(2)]
        : [GLOW(0), GLOW(1), GLOW(2)],
  };
  const candle = {
    root: "Lantern Glow/Candle Warm",
    name: CANDLE_NAME,
    info: { version: at.version, description: "A warmer candle flame, dropped from the newer versions." },
    payload: [{ rel: "natives/stm/effect/lantern_glow/candle_warm.tex.251211553", size: 262_144 }],
  };
  return {
    fileName: familyFileName(at.version),
    displayName: FAMILY_NAME,
    format: "zip",
    size: at.size,
    modifiedMs: at.uploaded * 1000,
    nexus: {
      modId: MOD_ID,
      fileId: at.fileId,
      groupId: GROUP_ID,
      sqid: null,
      version: at.version,
      uploadedMs: at.uploaded * 1000,
      source: "filename",
    },
    options: at.version === "1.2" ? [main, candle] : [main],
  };
}

export const familyPackage = (version: "1.3" | "1.2" | "1.1"): MockPackage => {
  const at = VERSIONS.find((v) => v.version === version) ?? VERSIONS[0];
  return buildPackage(versionSeed(at));
};

// --- R18.18: the player's word -------------------------------------------------

/** The folder's own name in the mods folder, and the version it holds. */
export const FOLDER_NAME = "Lantern Glow Recolor";
/** A second file of the same mod, in its own file group (questionmanual). */
export const CANDLES_FILE = "Lantern_Glow_Candles-2207-1-0-1730000000.zip";
export const CANDLES_NAME = "Lantern Glow Candles";

/** The link Find makes: the mod, and nothing that pins a file. */
const modOnlyLink = (): NexusLink => ({
  modId: MOD_ID,
  fileId: null,
  groupId: null,
  sqid: null,
  version: null,
  uploadedMs: null,
  source: "manual",
});

/** The unpacked folder: v1.3's own four file names (question), or names it shares with no file (questionmanual). */
export function folderPackage(sharesNames: boolean): MockPackage {
  return buildPackage({
    fileName: FOLDER_NAME,
    displayName: FAMILY_NAME,
    format: "folder",
    size: 1_400_000,
    modifiedMs: 1_729_500_000_000,
    nexus: modOnlyLink(),
    options: [
      {
        root: MAIN_ROOT,
        name: MAIN_NAME,
        info: { author: "Wilhelmina's Lamplighter", description: "Amber lantern light, unpacked by hand." },
        payload: sharesNames
          ? [{ rel: "LanternGlowRecolor.pak", size: 520_000 }, GLOW(0), GLOW(1), GLOW(2)]
          : [
              { rel: "natives/stm/effect/lantern_amber/amber_00.tex.251211553", size: 300_000 },
              { rel: "natives/stm/effect/lantern_amber/amber_01.tex.251211553", size: 300_000 },
            ],
      },
    ],
  });
}

function candlesPackage(): MockPackage {
  return buildPackage({
    fileName: CANDLES_FILE,
    displayName: CANDLES_NAME,
    format: "zip",
    size: 655_360,
    modifiedMs: 1_730_000_000_000,
    nexus: {
      modId: MOD_ID,
      fileId: 2380,
      groupId: 220760,
      sqid: null,
      version: "1.0",
      uploadedMs: 1_730_000_000_000,
      source: "filename",
    },
    options: [
      {
        root: "Lantern Glow Candles/Main",
        name: "Lantern Glow — Candles",
        info: { version: "1.0", description: "The candles alone, as their own file." },
        payload: [{ rel: "natives/stm/effect/lantern_glow/candles_00.tex.251211553", size: 262_144 }],
      },
    ],
  });
}

const QUESTION_SCENES = ["question", "questioninstalled", "questionjoined", "questionmanual", "questionrefused"];

export interface QuestionScene {
  packages: MockPackage[];
  installed: string[];
  /** Links as they were before a join, by package id (questionjoined). */
  priorLinks: Map<string, NexusLink>;
  /** The next join answers `refused` (questionrefused). */
  joinRefuses: boolean;
}

export function questionScene(scene: ReadonlySet<string>): QuestionScene | null {
  if (!QUESTION_SCENES.some((s) => scene.has(s))) return null;
  const lead = familyPackage("1.3");
  const manual = scene.has("questionmanual");
  const folder = folderPackage(!manual);
  const out: QuestionScene = {
    packages: [lead, folder],
    installed: [],
    priorLinks: new Map(),
    joinRefuses: scene.has("questionrefused"),
  };
  if (scene.has("questioninstalled")) out.installed.push(mainOf(folder));
  else out.installed.push(mainOf(lead));
  if (manual) out.packages.push(candlesPackage());
  if (scene.has("questionjoined")) {
    out.priorLinks.set(folder.summary.id, folder.summary.nexus ?? modOnlyLink());
    folder.summary.nexus = joinedLink(folder.summary.nexus, lead.summary.nexus);
  }
  return out;
}

/** The yes: the other's mod and file group, `source: "joined"`; the package's
 *  own file id, version and upload time stay as they were. */
export function joinedLink(own: NexusLink | null, of: NexusLink | null): NexusLink {
  return {
    modId: of?.modId ?? MOD_ID,
    fileId: own?.fileId ?? null,
    groupId: of?.groupId ?? null,
    sqid: own?.sqid ?? null,
    version: own?.version ?? null,
    uploadedMs: own?.uploadedMs ?? null,
    source: "joined",
  };
}

/** The file names a package holds, folders and case ignored, the catalog's own
 *  `modinfo.ini` and screenshots left out (WINDOW-CONTRACT § v0.5, The player's word). */
export function fileNamesOf(pkg: MockPackage): Set<string> {
  const names = new Set<string>();
  for (const entry of pkg.entries) {
    const name = (entry.path.split(/[\\/]/).at(-1) ?? "").toLowerCase();
    if (!name || name === "modinfo.ini" || name.startsWith("screenshot.")) continue;
    names.add(name);
  }
  return names;
}

/** The spine's question for one package, or null (the mock plays the spine). */
export interface QuestionRules {
  inFamily: (p: MockPackage) => boolean;
  leadOf: (p: MockPackage) => MockPackage;
  /** The player's noes, `packageId|groupId`. */
  declined: ReadonlySet<string>;
}

export function questionOf(
  pkg: MockPackage,
  packages: readonly MockPackage[],
  { inFamily, leadOf, declined }: QuestionRules,
): VersionQuestion | null {
  const link = pkg.summary.nexus;
  if (link?.groupId !== null || inFamily(pkg)) return null;
  const mine = fileNamesOf(pkg);
  let best: { of: MockPackage; shared: number } | null = null;
  for (const other of packages) {
    const theirs = other.summary.nexus;
    if (other === pkg || !theirs || theirs.modId !== link.modId || theirs.groupId === null) continue;
    if (declined.has(`${pkg.summary.id}|${theirs.groupId}`)) continue;
    const names = fileNamesOf(other);
    const shared = [...mine].filter((n) => names.has(n)).length;
    const contained = shared === mine.size || shared === names.size;
    // two names in common, or the one name each holds when it is the mod and no settings
    // file (the spine's `likely_shared`)
    const lone = [...mine].find((n) => names.has(n)) ?? "";
    const enough = shared >= 2 || (shared === 1 && mine.size === 1 && names.size === 1 && !isSettings(lone));
    if (!enough || !contained) continue;
    const lead = leadOf(other);
    const newer = (lead.summary.nexus?.uploadedMs ?? 0) > (best?.of.summary.nexus?.uploadedMs ?? 0);
    if (!best || shared > best.shared || (shared === best.shared && newer)) best = { of: lead, shared };
  }
  if (!best) return null;
  return {
    ofPackageId: best.of.summary.id,
    ofFileName: best.of.summary.fileName,
    ofVersion: best.of.summary.nexus?.version ?? null,
    sharedFiles: best.shared,
    files: mine.size,
  };
}

const FAMILY_SCENES = ["family", "familyfresh", "familythree", "putaway", "putawaytaken", "putawayfail", "putawayhalf"];
const SHELF_SCENES = ["undercroft", "undercroftempty", "undercroftnone"];

/** The scene asks for the family, or for the folder beside the mods folder. */
export const wantsVersions = (scene: ReadonlySet<string>): boolean =>
  [...FAMILY_SCENES, ...SHELF_SCENES].some((s) => scene.has(s));

export interface ShelfEntry {
  pkg: MockPackage;
  /** Its name in the older-versions folder. */
  fileName: string;
}

export interface VersionScene {
  /** Packages the mods folder gains. */
  packages: MockPackage[];
  /** Option ids already installed (the journal's), oldest first. */
  installed: string[];
  shelf: { exists: boolean; items: ShelfEntry[] };
  /** The put-away after an install fails once (putawayfail, putawayhalf). */
  putAwayFails: boolean;
  /** The put-away after that moves the first older version and fails the rest, once (putawayhalf). */
  putAwayHalf: boolean;
}

const mainOf = (pkg: MockPackage): string => pkg.options.find((o) => o.root === MAIN_ROOT)?.id ?? "";

export function versionScene(scene: ReadonlySet<string>): VersionScene {
  const out: VersionScene = {
    packages: [],
    installed: [],
    shelf: { exists: !scene.has("undercroftnone"), items: [] },
    putAwayFails: scene.has("putawayfail") || scene.has("putawayhalf"),
    putAwayHalf: scene.has("putawayhalf"),
  };
  if (scene.has("undercroft")) {
    const lead = familyPackage("1.3");
    const older = familyPackage("1.2");
    out.packages.push(lead);
    out.installed.push(mainOf(lead));
    out.shelf.items.push({ pkg: older, fileName: older.summary.fileName });
  }
  if (!FAMILY_SCENES.some((s) => scene.has(s))) {
    // a scene with no shelf named keeps the folder unmade, as on a first launch
    if (!SHELF_SCENES.some((s) => scene.has(s))) out.shelf.exists = false;
    return out;
  }
  const lead = familyPackage("1.3");
  const older = familyPackage("1.2");
  out.packages.push(lead, older);
  const fresh = ["familyfresh", "putaway", "putawaytaken", "putawayfail", "putawayhalf"].some((s) => scene.has(s));
  out.installed.push(mainOf(older));
  if (!fresh) out.installed.push(mainOf(lead));
  if (scene.has("familythree") || scene.has("putawayhalf")) out.packages.push(familyPackage("1.1"));
  if (scene.has("putawaytaken")) {
    // an earlier v1.2 the player set in the folder by hand, under the same name
    const earlier = familyPackage("1.2");
    out.shelf.items.push({ pkg: earlier, fileName: earlier.summary.fileName });
  }
  if (!SHELF_SCENES.some((s) => scene.has(s)) && !out.shelf.items.length) out.shelf.exists = false;
  return out;
}

// --- the spine's rules, over the mock's packages -------------------------------

/** Settings and text files: never a version's proof on their own. The spine's
 *  `SETTINGS_EXTENSIONS` (src-tauri/src/family.rs) and WINDOW-CONTRACT § v0.5 name the same list;
 *  tests/settingsExtensions.spec.ts holds the three together. */
export const SETTINGS_EXTENSIONS = ["json", "ini", "cfg", "toml", "yaml", "yml", "xml", "txt", "md", "log"];
const isSettings = (name: string): boolean => SETTINGS_EXTENSIONS.includes(name.split(".").pop()?.toLowerCase() ?? "");

const familyKey = (summary: PackageSummary): string | null => {
  const link = summary.nexus;
  if (!link || link.groupId === null) return null;
  return `${link.modId}|${link.groupId}`;
};

/** Higher first; a missing value last. */
function descending(a: number | null, b: number | null): number {
  if (a === b) return 0;
  if (a === null) return 1;
  if (b === null) return -1;
  return b - a;
}

/** Newest first: the latest upload, then the higher file id; a member stating neither last, by name. */
function newestFirst(a: MockPackage, b: MockPackage): number {
  const la = a.summary.nexus;
  const lb = b.summary.nexus;
  const byUpload = descending(la?.uploadedMs ?? null, lb?.uploadedMs ?? null);
  if (byUpload !== 0) return byUpload;
  const byFile = descending(la?.fileId ?? null, lb?.fileId ?? null);
  if (byFile !== 0) return byFile;
  return a.summary.fileName.toLowerCase().localeCompare(b.summary.fileName.toLowerCase());
}

/** Every family of two or more, by package id: its members newest first. */
export function familiesOf(packages: readonly MockPackage[]): Map<string, MockPackage[]> {
  const groups = new Map<string, MockPackage[]>();
  for (const pkg of packages) {
    const key = familyKey(pkg.summary);
    if (!key) continue;
    groups.set(key, [...(groups.get(key) ?? []), pkg]);
  }
  const out = new Map<string, MockPackage[]>();
  for (const members of groups.values()) {
    if (members.length < 2) continue;
    const sorted = [...members].sort(newestFirst);
    for (const m of sorted) out.set(m.summary.id, sorted);
  }
  return out;
}

/** A newer version of the shelved package's file is in the mods folder. */
export function newerInMods(shelved: MockPackage, packages: readonly MockPackage[]): boolean {
  const key = familyKey(shelved.summary);
  if (!key) return false;
  return packages.some((p) => familyKey(p.summary) === key && newestFirst(p, shelved) < 0);
}

const installable = (pkg: MockPackage) => pkg.options.filter((o) => !o.menuOnly);

/** The option of `to` that is the same option as `optionId` of `from`, or null
 *  (WINDOW-CONTRACT § v0.5, The same option). */
export function sameOptionIn(from: MockPackage, optionId: string, to: MockPackage): string | null {
  const option = from.options.find((o) => o.id === optionId);
  if (!option || option.menuOnly) return null;
  const theirs = installable(to);
  const byRoot = theirs.find((o) => o.root === option.root);
  if (byRoot) return byRoot.id;
  const byName = theirs.find((o) => o.name.toLowerCase() === option.name.toLowerCase());
  if (byName) return byName.id;
  if (installable(from).length === 1 && theirs.length === 1) return theirs[0]?.id ?? null;
  return null;
}

/** A name not taken among `taken` (compared case-blind, as Windows does): `name`, else `stem (2).ext` … */
export function freeName(name: string, taken: readonly string[]): string {
  const blind = new Set(taken.map((t) => t.toLowerCase()));
  if (!blind.has(name.toLowerCase())) return name;
  const dot = name.lastIndexOf(".");
  const stem = dot > 0 ? name.slice(0, dot) : name;
  const ext = dot > 0 ? name.slice(dot) : "";
  for (let n = 2; ; n++) {
    const next = `${stem} (${n})${ext}`;
    if (!blind.has(next.toLowerCase())) return next;
  }
}
