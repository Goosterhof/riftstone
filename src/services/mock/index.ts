// The mock backend: the whole window contract over the invented library in
// ./library.ts and the invented far world in ./far.ts, in memory, on real
// timers, never fetching. Scenarios for rendering every state come from the
// page's query string, comma-separated to combine:
//   ?mock=empty | emptymods | nogame | scanning | refoff | patched | offline |
//         fresh | firstrun | key | premium | rejected | quotalow | running |
//         nxmother | nocreations | nofloor | gonemods
// - empty: an empty mods folder and an untouched game (nothing installed,
//   nothing parked).
// - emptymods: the mods folder exists and holds nothing, while the game keeps
//   every install the journal knows (each row then has no package).
// - nofloor: the mods folder was missing at launch and could not be made (§9
//   THE RIFT HAS NO FLOOR): no packages, mods.exists false, and the spine's
//   `mods` Error with no action.
// - gonemods: the mods folder was made at launch and went while the stone ran
//   (moved or deleted): no packages, mods.exists false, and the spine's `mods`
//   Warn whose action, Open mods folder, makes it again, empty, and scans it.
//   The game keeps every install, as under emptymods.
// - firstrun: nothing scanned yet, so the stone wakes on YOUR WORLD IS FOUND.
// - key: a free Nexus account, "Arisen of Vernworth"; premium: the same account,
//   premium; rejected: Nexus refused the stored key; quotalow implies key.
// - running: Dragon's Dogma 2 runs, and the engine's rules bite: pak
//   uninstalls, reorders and switches are refused in the engine's own words,
//   installs land with a gameRunning warning and need a restart.
// - nxmother: another program (Wagonmaster.exe, #00015 R16.24) answers nxm:// links.
// - nocreations keeps every package and strips every option: the library with
//   nothing to fold the trouble over (§8.1.1).
//
// v0.2: the Nexus labels (./library.ts MOCK_LABELS) are served only once an
// update check has run, so `?mock=fresh` is the first launch before any check:
// no labels, the NEXUS facet group collapsed to its one quiet line. The MD5
// identify recognises one package (MOCK_MD5_MATCH), and fails under `offline`
// like the spine's lookup. Five packages stay unlinked, three of them with a
// screenshot (Brine Water Clarity, Sacred Arbor Fog Removal, Old Road Armor
// Pack): the "adult" veil blurs them as unknown until they are linked and
// checked, beside Battahli Sun Bleach, the one mod Nexus flags adult. Every
// backend reads its own copy of the library. Two parked files wait in the
// backup store under the spine's own handle shape, `backups/<seq>/<gamePath>`:
// over the first an unowned file sits now (Put back swaps it out), over the
// second Pawn Chatter Filter's install sits (Put back is refused in the
// engine's words until it is uninstalled). An unknown handle is refused the
// way the spine refuses it. Uninstalling Sacred Arbor Fog Removal parks its
// changed pak and Ermine Cape - Variation 2 leaves one changed file in place,
// so THE RETURN's "kept aside" and "left in place" sentences render. One
// installed file is already gone from the game (the "already gone" plan row),
// and one hash conflict is unconfirmed.
//
// v0.3: one mods folder (MOCK_MODS_DIR), no roots. Two adopted installs: Ox Cart
// Express, which a package in the mods folder matches, and Pawn Banter Quieter,
// a package-less stand-in carrying one pak (first in the load order, so its
// uninstall door is the one that matters). Install records keep their own
// files, as the journal does, so an install outlives its package. Play plans
// and launches; the game starts 0.7 s after a launch and stops through the
// dev hook below. Downloads run queued → linking → downloading (progress
// events) → verifying → landed in the spine's order (the archive linked first,
// rift://updates-changed; the row landed; then a scan shelves the invented
// package, rift://library-changed); a free account's download waits for an
// nxm:// link, which is read and refused as the spine reads and refuses it.
// The spine's own sentences are used wherever the window shows them. Dev hooks, on the backend
// and, in `npm run dev`, on `window.riftMock` (from the browser console):
//   closeGame()        the game watcher sees Dragon's Dogma 2 stop;
//   deliverNxm(link)   Windows hands the stone an nxm:// link;
//   gameSaved()        v0.4: the game saves Slot 9 one level later (THE RE-CARVING).
//
// v0.4, the Muster (./muster.ts, #00015 R17.30–R17.31): every scene lists the
// invented party's four slots, so the room is never empty. ?mock= gains
// muster (the same, by name) | musternosteam | musternosaves | musternames
// (every Named nameless, names "missing", the spine's note) | musterslow
// (get_muster answers after 2 s) | musterwriting (Slot 9 rejects as being
// written once, then the save settles and rift://muster-changed fires).
// Under nogame the names are missing too: they live in the game's own text.

import type {
  BringBackResult,
  Check,
  ConflictView,
  DownloadPhase,
  DownloadView,
  EndorseStatus,
  FamilyView,
  GameSummary,
  InstallOrigin,
  InstallPlanView,
  InstallResultView,
  InstalledRow,
  InstalledView,
  KeepNewestPlanView,
  KeepNewestResultView,
  LabelCount,
  LabelFacets,
  LaunchPlanView,
  LaunchWarning,
  LibraryPackage,
  MatchedField,
  Md5Result,
  ModsFolderView,
  NexusBrowseSort,
  NexusCandidate,
  NexusFileCategory,
  NexusKeyStanding,
  NexusLabels,
  NexusReason,
  NexusStandings,
  NexusSummary,
  NxmHandlerView,
  OptionDetail,
  OlderShelfItem,
  OlderShelfView,
  OlderVersionView,
  OptionRef,
  Overview,
  PackageDetail,
  ParkedFileView,
  ParkedRestoreReport,
  PakView,
  PayloadFile,
  PayloadKinds,
  PlanWarning,
  PlannedRemoval,
  PlannedWrite,
  PutAwayView,
  RemovalAction,
  ReplacesView,
  RiftError,
  SearchQuery,
  SearchResult,
  Settings,
  ThumbnailSize,
  TreeNode,
  UninstallPlanView,
  UninstallResultView,
  UpdateRow,
  UpdatesView,
} from "@/types/rift";
import { EVENTS } from "@/types/rift";
import { RiftFailure, type RiftBackend, type RiftEvent } from "../rift";
import type { NexusQuota, PendingBackend } from "../pending";
import { TROUBLE_KINDS } from "../voice";
import { createMusterMock, type MusterMock } from "./muster";
import {
  FAMILY_NAME,
  familiesOf,
  freeName,
  newerInMods,
  sameOptionIn,
  versionScene,
  type ShelfEntry,
} from "./versions";
import {
  MOCK_DAY,
  MOCK_FINDS,
  MOCK_LABELS,
  MOCK_MD5_MATCH,
  MOCK_MODS_DIR,
  MOCK_NOW,
  MOCK_PACKAGES,
  downloadedPackage,
  type MockPackage,
} from "./library";
import {
  MOCK_ACCOUNT_NAME,
  MOCK_BROWSE,
  MOCK_ENDORSEMENTS,
  MOCK_OTHER_HANDLER,
  MOCK_OWN_MOD,
  MOCK_TRACKED,
  MOCK_USER_ID,
  mockFiles,
} from "./far";

type Listener = (payload: unknown) => void;
type MockOption = MockPackage["options"][number];

/** What only the mock can do: the two acts of the world outside the window. */
export interface MockHooks {
  /** The game watcher sees Dragon's Dogma 2 stop (it started from Play, or under `running`). */
  closeGame(): void;
  /** Windows hands the stone an nxm:// link, as the site's Mod Manager Download button does. */
  deliverNxm(link: string): void;
  /** v0.4: the game saves Slot 9 one level later (#00015 R17.31), and the save watcher fires. */
  gameSaved(): void;
  /** v0.5: the Muster's whole mock (./muster.ts), its own dev hooks included. */
  musterMock: MusterMock;
}

export type MockBackend = RiftBackend & PendingBackend & MockHooks;

function scenarios(): Set<string> {
  if (typeof window === "undefined") return new Set();
  const raw = new URLSearchParams(window.location.search).get("mock") ?? "";
  return new Set(
    raw
      .split(",")
      .map((s) => s.trim())
      .filter(Boolean),
  );
}

const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

const notFound = (entry: string) => new RiftFailure({ kind: "entryNotFound", detail: { entry } });
const refusedWith = (reason: string) => new RiftFailure({ kind: "refused", detail: { reason } });
/** rift-deploy engine.rs restore_parked / discard_parked, word for word. */
const nothingKept = (store: string) =>
  refusedWith(`Nothing is kept aside under ${store}. Reload the game state and try again.`);
/** The spine's RiftError::Nexus: its sentence, and (v0.3) the reason the window voices. */
const nexusError = (detail: string, reason?: NexusReason): RiftError => ({
  kind: "nexus",
  detail: reason ? { detail, reason } : { detail },
});
const nexusFailure = (detail: string, reason?: NexusReason) => new RiftFailure(nexusError(detail, reason));
const OFFLINE_SENTENCE = "Nexus Mods could not be reached (connection refused).";

/** rift-nexus error.rs, word for word: the sentences beside the reason codes. */
const NO_KEY_SENTENCE =
  "This needs a Nexus Mods personal API key and none is set. Update checks work without one; add a key in Settings only for the extras.";
const KEY_REJECTED_SENTENCE =
  "Nexus Mods rejected the API key. Paste a fresh personal API key from your Nexus Mods account (Site preferences, API Keys); update checks keep working without one.";
const LINK_EXPIRED_SENTENCE =
  "The download link from Nexus Mods has expired. Press Mod Manager Download on the file's Nexus Mods page again for a fresh one.";
/** rift-nexus nxm.rs NxmRefusal and the spine's nxm.rs NOT_YOURS, word for word. */
const NXM_OTHER_GAME = "That Nexus Mods link is for another game. The Riftstone handles Dragon's Dogma 2 mods only.";
const NXM_COLLECTION =
  "That Nexus Mods link opens a collection. The Riftstone downloads single mod files: download the collection's mods from their own pages.";
const NXM_UNREADABLE =
  "That nxm link could not be read. Press Mod Manager Download on the file's Nexus Mods page again.";
const NXM_NOT_YOURS =
  "That Mod Manager Download link was made for another Nexus Mods account than the one whose API key the Riftstone holds. Sign in on the site with that key's account, then press Mod Manager Download again.";
/** The spine's downloads.rs cancel refusals, word for word. */
const CANCEL_NOT_LISTED = "That download is not in this session's list any more. Look again at the downloads.";
const CANCEL_OVER = "That download is already over (landed, failed or cancelled), so there is nothing to cancel.";
const CANCEL_LANDING =
  "That download is being put in the mods folder right now and can no longer be stopped. It lands in a moment; remove the archive in Explorer if you do not want it.";
/** How long the scan that shelves a landed download takes. */
const LANDING_SCAN_MS = 150;
/** Why the nofloor scene's mods folder could not be made (the spine passes the OS's words). */
const MODS_UNMADE_WHY = "access is denied";
/** The largest id Nexus's GraphQL Int carries (nxm.rs LARGEST_ID). */
const LARGEST_NEXUS_ID = 2_147_483_647;

/** rift-deploy's running-game sentences, word for word. */
const RUNNING_REINSTALL = (name: string) =>
  `Dragon's Dogma 2 is running and holds "${name}"'s paks open, so it cannot be reinstalled now. Close the game, then try again.`;
const RUNNING_UNINSTALL = (name: string) =>
  `Dragon's Dogma 2 is running and holds "${name}"'s paks open. Close the game, then uninstall.`;
const RUNNING_REORDER =
  "Dragon's Dogma 2 is running and holds its paks open. Close the game, then change the load order.";
const RUNNING_SWITCH =
  "Dragon's Dogma 2 is running, and REFramework rewrites re2_fw_config.txt when the game closes, so the change would be lost. Close the game, then flip the switch.";

/** An install as the journal remembers it: its own name, kinds and files,
 *  which outlive the package it came from. */
interface InstallRecord {
  optionId: string;
  seq: number;
  installedAt: string;
  origin: InstallOrigin;
  name: string;
  packageName: string;
  version: string | null;
  kinds: PayloadKinds;
  files: PayloadFile[];
}

const ZERO_KINDS: PayloadKinds = { pak: 0, loose: 0, reframework: 0, root: 0 };
/** When REFramework's log says the game last started: installs before it loaded. */
const LOG_STARTED_MS = MOCK_NOW - 6 * 3_600_000;
const PAGE = 20;
const FILE_ORDER: Record<NexusFileCategory, number> = {
  main: 0,
  update: 1,
  optional: 2,
  miscellaneous: 3,
  oldVersion: 4,
  removed: 5,
  archived: 6,
  unknown: 7,
};
const OVER: ReadonlySet<DownloadPhase> = new Set(["landed", "failed", "cancelled"]);
/** A download's phase, read fresh (the worker's awaits let a cancel change it). */
const phaseOf = (row: DownloadView): DownloadPhase => row.phase;

function recordOf(option: MockOption, pkg: MockPackage, at: { seq: number; installedAt: string }): InstallRecord {
  return {
    optionId: option.id,
    seq: at.seq,
    installedAt: at.installedAt,
    origin: "riftstone",
    name: option.name,
    packageName: pkg.summary.displayName,
    version: option.info?.version ?? null,
    kinds: { ...option.kinds },
    files: option.payload.map((p) => ({ ...p, target: { ...p.target } })),
  };
}

/** The adopted stand-in: an install the journal took over that no package in
 *  the mods folder matches. Its option id is the spine's to spell; the window
 *  never parses it. */
function standInRecord(): InstallRecord {
  return {
    optionId: "adopted#Pawn Banter Quieter",
    seq: 1,
    installedAt: new Date(MOCK_NOW - 30 * MOCK_DAY).toISOString(),
    origin: "adopted",
    name: "Pawn Banter Quieter",
    packageName: "Pawn Banter Quieter",
    version: null,
    kinds: { pak: 1, loose: 0, reframework: 0, root: 0 },
    files: [
      {
        entry: "PawnBanterQuieter.pak",
        target: { kind: "pak", fileName: "PawnBanterQuieter.pak" },
        size: 2_310_144,
      },
    ],
  };
}

export function createMockBackend(): MockBackend {
  const scene = scenarios();
  const listeners = new Map<string, Set<Listener>>();
  const emit = (event: RiftEvent, payload: unknown) => {
    for (const l of listeners.get(event) ?? []) l(payload);
  };

  // Each backend reads its own copy: a link, an unlink, an MD5 identify or a
  // landed download writes into package summaries, and must never leak into
  // the next backend (the next spec, the next scenario).
  let packages: MockPackage[] = scene.has("empty") ? [] : structuredClone(MOCK_PACKAGES);
  if (scene.has("nocreations")) packages = packages.map((p) => ({ ...p, options: [] }));
  // v0.5 (./versions.ts): the family scenes bring Lantern Glow Recolor's versions
  // in place of the library's own Lantern Glow, so one name stands for one file.
  const versionsAt = versionScene(scene);
  if (versionsAt.packages.length && !scene.has("empty")) {
    packages = [...packages.filter((p) => p.summary.displayName !== FAMILY_NAME), ...versionsAt.packages];
  }
  /** The older-versions folder beside the mods folder (THE UNDERCROFT). */
  const shelf: { exists: boolean; items: ShelfEntry[] } = versionsAt.shelf;
  let putAwayFails = versionsAt.putAwayFails;
  // v0.5: the Muster's mock owns its state (./muster.ts); `gameRoot` is read when asked.
  const muster = createMusterMock(scene, () => gameRoot === null, emit, wait);
  let settings: Settings = { veil: "off", autoCheckUpdates: true, musterCharacter: muster.rememberedAtStart() };
  let gameRoot: string | null = scene.has("nogame")
    ? null
    : "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Dragons Dogma 2";
  let gameSource: GameSummary["source"] = "steam";
  let scanning = scene.has("scanning");
  let scannedOnce = !scene.has("firstrun");
  let lastCheckMs: number | null = scene.has("fresh") ? null : MOCK_NOW - 2 * 3_600_000;

  // --- Play: the game's own state, as the spine's watcher keeps it ----------
  let running = scene.has("running");
  let startedMs: number | null = running ? Date.now() - 20 * 60_000 : null;
  /** An install, uninstall, reorder, switch or restore holds the game folder. */
  let changing = false;

  // --- the key's standing (validate.json) ----------------------------------
  const keyed = ["key", "quotalow", "premium", "rejected"].some((s) => scene.has(s));
  let keyStanding: NexusKeyStanding = "none";
  if (keyed) keyStanding = scene.has("rejected") ? "rejected" : "connected";
  if (keyStanding === "connected" && scene.has("offline")) keyStanding = "unchecked";
  const premium = scene.has("premium");
  let accountCheckedMs = Date.now() - 5 * 60_000;
  let quotaSpent = 0;

  const optionByName = (name: string) => packages.flatMap((p) => p.options).find((o) => o.name === name);

  function findOption(optionId: string) {
    for (const pkg of packages) {
      const option = pkg.options.find((o) => o.id === optionId);
      if (option) return { pkg, option };
    }
    return null;
  }

  // --- what is in the game: the journal's installs --------------------------
  let installs: InstallRecord[] = [];
  /** An install of one library option, as the journal would hold it. */
  const preinstall = (name: string, at: { seq: number; installedAt: string }, origin: InstallOrigin) => {
    const pkg = packages.find((p) => p.options.some((o) => o.name === name));
    const o = pkg?.options.find((x) => x.name === name);
    if (pkg && o) installs.push({ ...recordOf(o, pkg, at), origin });
  };
  if (!scene.has("empty")) {
    // adopted first: the journal took them over before the stone installed anything
    installs.push(standInRecord());
    preinstall("Ox Cart Express", { seq: 2, installedAt: new Date(MOCK_NOW - 25 * MOCK_DAY).toISOString() }, "adopted");
    const riftstone = [
      "REFramework",
      "Pawn Chatter Filter",
      "Brine Water Clarity",
      "Sacred Arbor Fog Removal",
      "Ermine Cape - Variation 2",
    ];
    for (const [i, name] of riftstone.entries()) {
      const installedAt = new Date(MOCK_NOW - (10 - i) * MOCK_DAY).toISOString();
      preinstall(name, { seq: i + 3, installedAt }, "riftstone");
    }
    for (const [i, optionId] of versionsAt.installed.entries()) {
      const found = packages.flatMap((pkg) => pkg.options.map((o) => ({ pkg, o }))).find((x) => x.o.id === optionId);
      const installedAt = new Date(MOCK_NOW - (3 - i) * MOCK_DAY).toISOString();
      if (found) installs.push(recordOf(found.o, found.pkg, { seq: installs.length + 1, installedAt }));
    }
  }
  let seq = installs.length;
  const recordOfId = (optionId: string) => installs.find((r) => r.optionId === optionId);
  const isInstalled = (optionId: string) => installs.some((r) => r.optionId === optionId);
  const hasPaks = (record: InstallRecord) => record.files.some((f) => f.target.kind === "pak");

  // An installed file someone deleted by hand: the plan says "already gone".
  const gonePaths = new Set<string>();
  // Ermine's second loose file was edited by hand since install: kept on uninstall.
  const changedPaths = new Set<string>();
  const ermine = optionByName("Ermine Cape - Variation 2");
  const ermineLoose = (ermine?.payload ?? []).flatMap((p) => (p.target.kind === "pak" ? [] : [p.target.gamePath]));
  if (!scene.has("empty") && ermineLoose[0]) gonePaths.add(ermineLoose[0]);
  if (!scene.has("empty") && ermineLoose[1]) changedPaths.add(ermineLoose[1]);

  // emptymods: the folder is emptied AFTER the journal was written, so every
  // install stays in the game with no package in the mods folder (and so do
  // nofloor's and gonemods', whose folder is not there at all).
  if (scene.has("emptymods") || scene.has("nofloor") || scene.has("gonemods")) packages = [];

  /** The mods folder itself (the spine's mods_folder_state): on disk; missing
   *  at launch and never made (nofloor); or made at launch and gone since
   *  (gonemods), until Open mods folder makes it again. */
  let folder: "there" | "unmade" | "gone" = "there";
  if (scene.has("nofloor")) folder = "unmade";
  else if (scene.has("gonemods")) folder = "gone";

  type PayloadTarget = PayloadFile["target"];
  function removalAction(target: PayloadTarget): RemovalAction {
    if (target.kind === "pak") return { kind: "delete" };
    if (gonePaths.has(target.gamePath)) return { kind: "alreadyGone" };
    if (changedPaths.has(target.gamePath)) return { kind: "keepChanged" };
    return { kind: "delete" };
  }

  // The backup store: files kept aside rather than deleted or forced back.
  let parkSeq = 40;
  const parkStore = (gamePath: string) => `backups/${++parkSeq}/${gamePath}`;
  /** The install that sits over a parked file's path, by game path. */
  const heldBy = new Map<string, string>([["reframework/autorun/pawn_banter.lua", "Pawn Chatter Filter"]]);
  let parked: ParkedFileView[] = scene.has("empty")
    ? []
    : [
        {
          store: "backups/17/natives/stm/environment/sky/sky_noon.tex.251211553",
          gamePath: "natives/stm/environment/sky/sky_noon.tex.251211553",
          owner: "the file that was there before",
          ownerKind: "unowned",
          size: 2_621_440,
          parkedAt: new Date(MOCK_NOW - 4 * MOCK_DAY).toISOString(),
          reason: "Brine Water Clarity's install would have overwritten it, so it was kept aside first.",
          occupied: true,
        },
        {
          store: "backups/21/reframework/autorun/pawn_banter.lua",
          gamePath: "reframework/autorun/pawn_banter.lua",
          owner: "Pawn Banter Quieter",
          ownerKind: "adopted",
          size: 4_096,
          parkedAt: new Date(MOCK_NOW - 2 * MOCK_DAY).toISOString(),
          reason: "Pawn Chatter Filter's install would have overwritten it, so it was kept aside first.",
          occupied: true,
        },
      ];

  let pakOrder = installs.filter(hasPaks).map((r) => r.optionId);

  /** A pak's managed name: its slot in the load order before its own name. */
  const pakName = (optionId: string, fileName: string, slot = pakOrder.indexOf(optionId)) =>
    `zrift-${String(Math.max(0, slot)).padStart(4, "0")}-${fileName}`;
  const gamePathOf = (optionId: string, target: PayloadTarget, slot?: number) =>
    target.kind === "pak" ? `pak_mods/${pakName(optionId, target.fileName, slot)}` : target.gamePath;

  function conflicts(): ConflictView[] {
    const water = optionByName("Brine Water Clarity");
    const fog = optionByName("Sacred Arbor Fog Removal");
    if (!water || !fog || !isInstalled(water.id) || !isInstalled(fog.id)) return [];
    const [loser, winner] = pakOrder.indexOf(water.id) < pakOrder.indexOf(fog.id) ? [water, fog] : [fog, water];
    return [0x3a91c0de, 0x51be7702, 0x0c0ffee1].map((lower, i) => ({
      key: {
        kind: "hash" as const,
        hash: { lower, upper: lower ^ 0x5f5f5f5f },
        path: i === 0 ? "natives/stm/environment/water/ocean_shallow.mdf2.51" : null,
      },
      layers: [
        { optionId: loser.id, name: loser.name },
        { optionId: winner.id, name: winner.name },
      ],
      winner: { optionId: winner.id, name: winner.name },
      // the third pak entry carries table flags rift-pak does not know
      unconfirmed: i === 2,
    }));
  }
  const conflicted = (optionId: string) => conflicts().some((c) => c.layers.some((l) => l.optionId === optionId));

  // --- v0.5: versions of one file (./versions.ts holds the spine's rules) ---
  const membersOf = (pkg: MockPackage): MockPackage[] | null => familiesOf(packages).get(pkg.summary.id) ?? null;
  const installedIn = (pkg: MockPackage): number => pkg.options.filter((o) => isInstalled(o.id)).length;
  const isOlderMember = (pkg: MockPackage): boolean => {
    const members = membersOf(pkg);
    return members !== null && members[0] !== pkg;
  };

  /** On a family's lead: its older versions, newest first, with their same options. */
  function olderViews(pkg: MockPackage): OlderVersionView[] {
    const members = membersOf(pkg);
    if (!members || members[0] !== pkg) return [];
    return members.slice(1).map((older) => ({
      packageId: older.summary.id,
      fileName: older.summary.fileName,
      version: older.summary.nexus?.version ?? null,
      installed: installedIn(older),
      matches: older.options.flatMap((o) => {
        const leadOptionId = sameOptionIn(older, o.id, pkg);
        return leadOptionId ? [{ optionId: o.id, leadOptionId, installed: isInstalled(o.id) }] : [];
      }),
    }));
  }

  function familyView(pkg: MockPackage): FamilyView | null {
    const members = membersOf(pkg);
    if (!members) return null;
    return { lead: members[0].summary.id, version: pkg.summary.nexus?.version ?? null, older: olderViews(pkg) };
  }

  /** The installs of the same option in the other versions of the option's file. */
  function replacedBy(optionId: string): { record: InstallRecord; pkg: MockPackage }[] {
    const found = findOption(optionId);
    const members = found ? membersOf(found.pkg) : null;
    if (!found || !members) return [];
    return members.flatMap((m) => {
      if (m === found.pkg) return [];
      const same = sameOptionIn(found.pkg, optionId, m);
      const record = same ? recordOfId(same) : undefined;
      return record ? [{ record, pkg: m }] : [];
    });
  }

  const replacesView = (r: { record: InstallRecord; pkg: MockPackage }): ReplacesView => ({
    optionId: r.record.optionId,
    name: r.record.name,
    version: r.pkg.summary.nexus?.version ?? null,
    removals: r.record.files.length,
  });

  /** Moves one package, whole, to the older-versions folder: its name, or ` (2)` … */
  function moveToShelf(pkg: MockPackage): PutAwayView {
    const landedAs = freeName(
      pkg.summary.fileName,
      shelf.items.map((i) => i.fileName),
    );
    shelf.exists = true;
    shelf.items.push({ pkg, fileName: landedAs });
    packages = packages.filter((p) => p !== pkg);
    return { fileName: pkg.summary.fileName, landedAs };
  }

  /** After an install of a family member committed: every older member left
   *  without installs goes, never-installed ones too. */
  function putAwayAfter(pkg: MockPackage): Pick<InstallResultView, "putAway" | "putAwayFailed"> {
    const members = membersOf(pkg);
    if (!members) return { putAway: [], putAwayFailed: null };
    const older = members.slice(members.indexOf(pkg) + 1).filter((m) => installedIn(m) === 0);
    if (!older.length) return { putAway: [], putAwayFailed: null };
    if (putAwayFails) {
      putAwayFails = false;
      const names = older.map((m) => m.summary.fileName).join(", ");
      return {
        putAway: [],
        putAwayFailed: `The stone couldn't move ${names} to mods (older versions): access is denied.`,
      };
    }
    return { putAway: older.map(moveToShelf), putAwayFailed: null };
  }

  const ONLY_VERSION = (pkg: MockPackage) =>
    `"${pkg.summary.fileName}" is the only version of its file in the mods folder.`;

  /** What keeping only the newest would do, or why it cannot. */
  function keepNewestOf(
    pkg: MockPackage,
  ): KeepNewestPlanView & { olderMembers: MockPackage[]; records: InstallRecord[] } {
    const members = membersOf(pkg);
    const none = { olderMembers: [], records: [], uninstalls: [], putsAway: [] };
    if (!members) return { ...none, packageId: pkg.summary.id, keeps: null, refused: ONLY_VERSION(pkg) };
    const lead = members[0];
    const kept = members.find((m) => installedIn(m) > 0) ?? lead;
    const olderMembers = members.slice(members.indexOf(kept) + 1);
    const base = { packageId: lead.summary.id, keeps: kept.summary.fileName };
    if (!olderMembers.length) {
      return {
        ...none,
        ...base,
        refused: `No version of ${lead.summary.displayName} is older than the one kept, "${kept.summary.fileName}".`,
      };
    }
    const pairs = olderMembers.flatMap((m) =>
      m.options.flatMap((o) => {
        const record = recordOfId(o.id);
        return record ? [{ record, pkg: m }] : [];
      }),
    );
    const held = pairs.find((p) => running && hasPaks(p.record));
    return {
      ...base,
      olderMembers,
      records: pairs.map((p) => p.record),
      uninstalls: pairs.map(replacesView),
      putsAway: olderMembers.map((m) => m.summary.fileName),
      refused: held ? RUNNING_UNINSTALL(held.record.name) : null,
    };
  }

  /** Links made since the last SUCCESSFUL check: no check has judged them yet,
   *  so they read notChecked (a failed check judges none). */
  const unjudged = new Set<string>();

  function updateRow(pkg: MockPackage): UpdateRow | null {
    const link = pkg.summary.nexus;
    if (!link) return null;
    const base = {
      packageId: pkg.summary.id,
      packageName: pkg.summary.displayName,
      link,
      modName: pkg.summary.displayName,
      localVersion: link.version,
      localUploadedMs: link.uploadedMs,
      pageUrl: `https://www.nexusmods.com/dragonsdogma2/mods/${link.modId}`,
      checkedMs: lastCheckMs,
      older: olderViews(pkg),
    };
    // `offline` changes nothing here: a failed check leaves the cache alone
    // (get_updates is cached state), so the rows keep the last good verdict
    // the page's offline notice dates.
    if (lastCheckMs === null || unjudged.has(pkg.summary.id)) return { ...base, state: { kind: "notChecked" } };
    // the spine's pin_link: a link naming no file id, sqid, upload stamp or
    // version (a link by bare mod id) has nothing to pin by, so it stays unpinned
    if (link.fileId === null && link.sqid === null && link.uploadedMs === null && link.version === null) {
      return { ...base, state: { kind: "unpinned" } };
    }
    if (link.modId === 412 && link.fileId !== 2391) {
      return {
        ...base,
        state: {
          kind: "updateAvailable",
          newer: {
            fileId: 2391,
            name: "Lantern Glow Recolor",
            version: "2.2",
            uploadedMs: MOCK_NOW - 3 * MOCK_DAY,
          },
        },
      };
    }
    return { ...base, state: { kind: "current" } };
  }

  /** What Nexus said about the package's mod at the last check; null before one. */
  function labelsOf(pkg: MockPackage): NexusLabels | null {
    const link = pkg.summary.nexus;
    // a mod an earlier check heard of brings its labels at once (the spine's
    // relabel on link_package); MOCK_LABELS stands for what the checks heard
    if (!link || lastCheckMs === null) return null;
    return MOCK_LABELS[link.modId] ?? null;
  }

  function labelFacets(): LabelFacets {
    const categories = new Map<string, LabelCount>();
    const tagCounts = new Map<string, LabelCount>();
    let unlabeledPackages = 0;
    const bump = (map: Map<string, LabelCount>, name: string, n: number) => {
      const key = name.toLowerCase();
      const entry = map.get(key) ?? { name, options: 0 };
      entry.options += n;
      map.set(key, entry);
    };
    for (const pkg of packages) {
      const installable = pkg.options.filter((o) => !o.menuOnly).length;
      if (!installable) continue;
      const labels = labelsOf(pkg);
      if (!labels) {
        unlabeledPackages += 1;
        continue;
      }
      if (labels.category) bump(categories, labels.category, installable);
      for (const tag of labels.tags) bump(tagCounts, tag, installable);
    }
    const order = (a: LabelCount, b: LabelCount) => b.options - a.options || a.name.localeCompare(b.name);
    return {
      categories: [...categories.values()].sort(order),
      tags: [...tagCounts.values()].sort(order),
      unlabeledPackages,
    };
  }

  function updatesView(): UpdatesView {
    const rows: UpdateRow[] = [];
    for (const pkg of packages) {
      // v0.5: one row per family, the lead's; an older version has none of its own
      if (isOlderMember(pkg)) continue;
      const row = updateRow(pkg);
      if (row) rows.push(row);
      else if (pkg.options.length) {
        rows.push({
          packageId: pkg.summary.id,
          packageName: pkg.summary.displayName,
          link: null,
          modName: null,
          localVersion: pkg.options[0]?.info?.version ?? null,
          localUploadedMs: null,
          state: { kind: "unlinked" },
          pageUrl: null,
          checkedMs: null,
          older: [],
        });
      }
    }
    return { lastCheckMs, rows };
  }

  function nexusSummary(): NexusSummary {
    const view = updatesView();
    return {
      lastCheckMs,
      checking: false,
      linked: view.rows.filter((r) => r.link).length,
      unlinked: view.rows.filter((r) => !r.link).length,
      updates: view.rows.filter((r) => r.state.kind === "updateAvailable").length,
      hasApiKey: keyStanding !== "none",
      key: keyStanding,
      account:
        keyStanding === "connected"
          ? { name: MOCK_ACCOUNT_NAME, premium, supporter: false, checkedMs: accountCheckedMs }
          : null,
    };
  }

  function game(): GameSummary | null {
    if (!gameRoot) return null;
    return {
      root: gameRoot,
      buildId: scene.has("patched") ? "25391144" : "25290080",
      source: gameSource,
      running,
      startedMs,
      launch: gameSource === "steam" ? "steam" : "exe",
      buildChanged: scene.has("patched") ? { recorded: "25290080", now: "25391144" } : null,
    };
  }

  const modsFolder = (): ModsFolderView => ({ path: MOCK_MODS_DIR, exists: folder === "there" });

  /** The spine's overview.rs `mods` check, word for word: absent while the
   *  folder holds packages. */
  function modsCheck(): Check | null {
    if (folder === "unmade") {
      return {
        id: "mods",
        level: "error",
        title: "The mods folder is missing",
        detail: `The Riftstone keeps your mods in ${MOCK_MODS_DIR}, and that folder could not be made (${MODS_UNMADE_WHY}). Make it by hand, or install the Riftstone where it may write, then start it again.`,
        action: null,
      };
    }
    if (folder === "gone") {
      return {
        id: "mods",
        level: "warn",
        title: "The mods folder is gone",
        detail: `${MOCK_MODS_DIR} was moved or deleted while the Riftstone ran, and it reads mods from nowhere else. Open mods folder makes it again, empty, and watches it; then put your mods back in it.`,
        action: { kind: "openModsFolder" },
      };
    }
    if (packages.length) return null;
    return {
      id: "mods",
      level: "info",
      title: "The mods folder is empty",
      detail: `Put each mod's zip, 7z or rar, or its unpacked folder, directly in ${MOCK_MODS_DIR}. The Riftstone reads it, never changes what is there, and sees new mods by itself.`,
      action: { kind: "openModsFolder" },
    };
  }

  function gameChecks(): Check[] {
    const list: Check[] = [
      {
        id: "game",
        level: "ok",
        title: "Dragon's Dogma 2 found",
        detail: `Steam build ${game()?.buildId}.`,
        action: null,
      },
    ];
    if (scene.has("patched")) {
      list.push({
        id: "build",
        level: "warn",
        title: "The game updated since your last install",
        detail: "Build 25290080 → 25391144. REFramework and script mods may need updates before they work again.",
        action: { kind: "openUrl", url: "https://github.com/praydog/REFramework-nightly/releases" },
      });
    }
    list.push({
      id: "reframework",
      level: "ok",
      title: "REFramework v1.5.9.1+514",
      detail: "Built 11.10.2026.",
      action: null,
    });
    list.push(
      scene.has("refoff")
        ? {
            id: "pakdir",
            level: "warn",
            title: "REFramework's pak folder is switched off",
            detail: "Your pak mods will not load until 'PAK Directory Loading' is on.",
            action: { kind: "enableSwitch", switch: "pakDirectory" },
          }
        : {
            id: "pakdir",
            level: "ok",
            title: "pak_mods loading is on",
            detail: "REFramework injects every pak in pak_mods/ at launch.",
            action: null,
          },
    );
    list.push({
      id: "loose",
      level: "ok",
      title: "Loose files load",
      detail: "LooseFileLoader is enabled.",
      action: null,
    });
    return list;
  }

  function checks(): Check[] {
    if (!gameRoot) {
      return [
        {
          id: "game",
          level: "error",
          title: "Dragon's Dogma 2 was not found",
          detail: "Steam's library folders do not list app 2054970. Point the Riftstone at the game folder.",
          action: { kind: "pickGameFolder" },
        },
      ];
    }
    const list = gameChecks();
    const mods = modsCheck();
    if (mods) list.push(mods);
    if (parked.length) {
      list.push({
        id: "parkedBackups",
        level: "info",
        title: `${parked.length} ${parked.length === 1 ? "file is" : "files are"} kept aside in the backup store`,
        detail: "The stone parked them rather than delete them or force them back. Put each back or discard it.",
        action: { kind: "reviewParked" },
      });
    }
    if (running) {
      list.push({
        id: "gameRunning",
        level: "info",
        title: "Dragon's Dogma 2 is running",
        detail:
          "Installs land now and load at the next launch. Uninstalling paks and changing their order waits until the game closes.",
        action: null,
      });
    }
    return list;
  }

  function overview(): Overview {
    const allOptions = packages.flatMap((p) => p.options);
    return {
      game: game(),
      reframework: gameRoot
        ? {
            installed: true,
            version: "v1.5.9.1+514",
            buildDate: "11.10.2026",
            looseFileLoader: true,
            pakDirectory: !scene.has("refoff"),
            faultyFiles: [],
          }
        : null,
      mods: modsFolder(),
      library: {
        packages: packages.length,
        options: allOptions.length,
        entries: packages.reduce((n, p) => n + p.entries.length, 0),
        installedOptions: installs.length,
        updatesAvailable: nexusSummary().updates,
        conflicts: conflicts().length,
        // ?mock=firstrun: nothing scanned yet, so the stone wakes on YOUR WORLD IS FOUND
        lastScanMs: scannedOnce || scanning ? MOCK_NOW - 60_000 : null,
      },
      scan: scanning
        ? {
            state: "scanning",
            done: 3,
            total: packages.length || 11,
            current: "Vernworth Wardrobe Collection.zip",
          }
        : { state: "idle" },
      checks: checks(),
      nexus: nexusSummary(),
      recovered: null,
    };
  }

  // --- search: AND over tokens, quoted phrases, weighted fields -------------
  const WEIGHTS: Record<MatchedField, number> = {
    name: 10,
    trail: 6,
    bundle: 5,
    packageName: 4,
    category: 4,
    author: 3,
    version: 2,
    description: 2,
    file: 1,
    nexusLabel: 4,
  };

  /** Quoted phrases stay whole; everything else splits on whitespace. */
  function tokens(text: string): string[] {
    const out: string[] = [];
    for (const [i, part] of text.toLowerCase().split('"').entries()) {
      if (i % 2 === 1) {
        const phrase = part.trim();
        if (phrase) out.push(phrase);
      } else {
        out.push(...part.split(/\s+/).filter(Boolean));
      }
    }
    return out;
  }

  interface TokenScore {
    best: number;
    field: MatchedField | null;
    files: string[];
  }

  function scoreField(token: string, field: MatchedField, value: string): number {
    const v = value.toLowerCase();
    const at = v.indexOf(token);
    if (at < 0) return 0;
    const wordStart = at === 0 || /[\s\-_(/›]/.test(v.charAt(at - 1));
    const wholeWord = wordStart && !/[a-z0-9]/.test(v.charAt(at + token.length));
    if (wholeWord) return WEIGHTS[field] * 2;
    return wordStart ? WEIGHTS[field] * 1.5 : WEIGHTS[field];
  }

  function scoreToken(token: string, fields: [MatchedField, string][], paths: string[]): TokenScore {
    let best = 0;
    let field: MatchedField | null = null;
    for (const [name, value] of fields) {
      const w = scoreField(token, name, value);
      if (w > best) {
        best = w;
        field = name;
      }
    }
    const files = paths.filter((path) => path.toLowerCase().includes(token));
    if (files.length && best < WEIGHTS.file) {
      best = WEIGHTS.file;
      field = "file";
    }
    return { best, field, files };
  }

  function snippet(description: string | null | undefined): string | null {
    if (!description) return null;
    const flat = description.replace(/\s+/g, " ");
    return flat.length > 180 ? `${flat.slice(0, 177)}…` : flat;
  }

  function installBadge(optionId: string): SearchResult["installed"] {
    const record = recordOfId(optionId);
    return record ? { origin: record.origin, installedAt: record.installedAt } : null;
  }

  function updateBadge(pkg: MockPackage): SearchResult["update"] {
    const row = updateRow(pkg);
    if (!row) return null;
    if (row.state.kind === "updateAvailable") return "available";
    if (row.state.kind === "current") return "current";
    return "unknown";
  }

  function passesFilters(query: SearchQuery, pkg: MockPackage, o: MockOption): boolean {
    if (o.menuOnly && !query.filters.includeMenuOnly) return false;
    const wanted = (list: string[]) => list.map((x) => x.toLowerCase());
    const labels = labelsOf(pkg);
    const categories = wanted(query.filters.categories);
    if (categories.length && !categories.includes(labels?.category?.toLowerCase() ?? "")) return false;
    const tags = wanted(query.filters.tags);
    const carried = wanted(labels?.tags ?? []);
    if (tags.length && !tags.every((t) => carried.includes(t))) return false;
    if (query.filters.installed === true) return isInstalled(o.id);
    if (query.filters.installed === false) return !isInstalled(o.id);
    return true;
  }

  function matchOption(toks: string[], pkg: MockPackage, o: MockOption): SearchResult | null {
    const fields: [MatchedField, string][] = [
      ["name", o.name],
      ["trail", o.trail.join(" › ")],
      ["bundle", o.bundle ?? ""],
      ["packageName", `${pkg.summary.displayName} ${pkg.summary.fileName}`],
      ["category", o.info?.category ?? ""],
      ["author", o.info?.author ?? ""],
      ["version", o.info?.version ?? ""],
      ["description", o.info?.description ?? ""],
      ["nexusLabel", nexusLabelText(labelsOf(pkg))],
    ];
    const paths = [...o.payload.map((x) => x.entry), ...o.skipped.map((x) => x.entry)];
    let score = 0;
    const matched = new Set<MatchedField>();
    const fileMatches: string[] = [];
    for (const token of toks) {
      const result = scoreToken(token, fields, paths);
      if (!result.field) return null;
      matched.add(result.field);
      if (result.files.length) matched.add("file");
      for (const f of result.files) if (fileMatches.length < 5 && !fileMatches.includes(f)) fileMatches.push(f);
      score += result.best;
    }
    return {
      optionId: o.id,
      packageId: pkg.summary.id,
      score,
      name: o.name,
      trail: o.trail,
      bundle: o.bundle,
      packageName: pkg.summary.displayName,
      packageFormat: pkg.summary.format,
      author: o.info?.author ?? null,
      version: o.info?.version ?? null,
      category: o.info?.category ?? null,
      descriptionSnippet: snippet(o.info?.description),
      hasScreenshot: Boolean(o.screenshot),
      kinds: o.kinds,
      menuOnly: o.menuOnly,
      matched: [...matched].sort((a, b) => WEIGHTS[b] - WEIGHTS[a]),
      fileMatches,
      installed: installBadge(o.id),
      update: updateBadge(pkg),
      conflicted: conflicted(o.id),
      nexusCategory: labelsOf(pkg)?.category ?? null,
      nexusTags: labelsOf(pkg)?.tags ?? [],
      adult: labelsOf(pkg)?.adult ?? false,
    };
  }

  function nexusLabelText(labels: NexusLabels | null): string {
    if (!labels) return "";
    return [labels.category ?? "", ...labels.tags].filter(Boolean).join(" › ");
  }

  function search(query: SearchQuery): SearchResult[] {
    const toks = tokens(query.text);
    const hits: SearchResult[] = [];
    for (const pkg of packages) {
      for (const o of pkg.options) {
        if (!passesFilters(query, pkg, o)) continue;
        const hit = matchOption(toks, pkg, o);
        if (hit) hits.push(hit);
      }
    }
    // v0.5: an older version's answer goes when the lead's same option answers too
    const answered = new Set(hits.map((h) => h.optionId));
    const kept = hits.filter((hit) => {
      const found = findOption(hit.optionId);
      const members = found ? membersOf(found.pkg) : null;
      if (!found || !members || members[0] === found.pkg) return true;
      const same = sameOptionIn(found.pkg, hit.optionId, members[0]);
      return !same || !answered.has(same);
    });
    hits.splice(0, hits.length, ...kept);
    if (toks.length) hits.sort((a, b) => b.score - a.score || a.name.localeCompare(b.name));
    // v0.3: limit 0 is no cap — every match
    return query.limit > 0 ? hits.slice(0, query.limit) : hits;
  }

  // --- thumbnails: generated SVG plates, never real art --------------------
  function plate(name: string, size: ThumbnailSize): string {
    let h = 0x811c9dc5;
    for (const ch of name) h = Math.imul(h ^ ch.charCodeAt(0), 16777619) >>> 0;
    const hue = (h ^ (h >>> 13)) % 360;
    const w = size === "card" ? 360 : 1280;
    const ht = Math.round(w * 0.5625);
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${ht}" viewBox="0 0 16 9">
      <defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stop-color="hsl(${hue},45%,22%)"/><stop offset="1" stop-color="hsl(${(hue + 40) % 360},55%,9%)"/></linearGradient>
        <radialGradient id="l" cx=".5" cy=".38" r=".5"><stop offset="0" stop-color="hsl(${hue},70%,72%)" stop-opacity=".55"/><stop offset="1" stop-color="hsl(${hue},70%,60%)" stop-opacity="0"/></radialGradient></defs>
      <rect width="16" height="9" fill="url(#g)"/><rect width="16" height="9" fill="url(#l)"/>
      <path d="M8 1.6 C9.3 1.6 9.6 3 9.4 3.6 L11.2 8.4 H4.8 L6.6 3.6 C6.4 3 6.7 1.6 8 1.6Z" fill="hsl(${hue},25%,6%)" opacity=".78"/>
    </svg>`;
    return `data:image/svg+xml;base64,${btoa(svg)}`;
  }

  function planWarnings(option: MockOption): PlanWarning[] {
    const warnings: PlanWarning[] = [];
    if (running) warnings.push({ kind: "gameRunning" });
    if (scene.has("refoff") && option.kinds.pak) warnings.push({ kind: "pakFolderOff" });
    if (option.payload.some((p) => /\.(240423143|760230703|40)$/.test(p.entry))) {
      warnings.push({ kind: "outdatedFormats", count: 3 });
    }
    const rivals: Record<string, string> = {
      "Brine Water Clarity": "Sacred Arbor Fog Removal",
      "Sacred Arbor Fog Removal": "Brine Water Clarity",
    };
    const rival = rivals[option.name];
    if (rival) warnings.push({ kind: "conflicts", optionNames: [rival], paths: 3 });
    for (const req of option.info?.requires ?? []) {
      const r = optionByName(req);
      if (!r || !isInstalled(r.id)) warnings.push({ kind: "requiresMissing", name: req });
    }
    return warnings;
  }

  function planRefusal(option: MockOption): string | null {
    if (!gameRoot) return "Dragon's Dogma 2 has not been found yet — point the Riftstone at the game folder first.";
    if (option.menuOnly) return "This is a menu entry — it installs nothing. Pick one of its variants.";
    // the engine reinstalls nothing whose paks the running game holds open
    if (running && option.kinds.pak && isInstalled(option.id)) return RUNNING_REINSTALL(option.name);
    return null;
  }

  // --- the tree: rift-catalog tree::nest, decorated as the spine decorates it
  function optionNode(o: MockOption, children: Map<string, MockOption[]>, visited: Set<string>): TreeNode | null {
    if (visited.has(o.id)) return null; // a loop the resolver let through: shown once
    visited.add(o.id);
    return {
      optionId: o.id,
      label: o.name,
      kind: "option",
      menuOnly: o.menuOnly,
      hasScreenshot: Boolean(o.screenshot),
      installed: isInstalled(o.id),
      conflicted: conflicted(o.id),
      kinds: o.kinds,
      children: nestLevel(children.get(o.id) ?? [], children, visited),
    };
  }

  /** One level: options sharing a bundle (case-blind, at least two) gather
   *  under one bundle node at the first member's place; order is the package's. */
  function nestLevel(options: MockOption[], children: Map<string, MockOption[]>, visited: Set<string>): TreeNode[] {
    const shared = new Map<string, number>();
    for (const o of options) {
      if (o.bundle) shared.set(o.bundle.toLowerCase(), (shared.get(o.bundle.toLowerCase()) ?? 0) + 1);
    }
    const nodes: TreeNode[] = [];
    const bundles = new Map<string, TreeNode>();
    for (const o of options) {
      const node = optionNode(o, children, visited);
      if (!node) continue;
      const bundleName = o.bundle;
      const key = bundleName?.toLowerCase() ?? "";
      if (!bundleName || (shared.get(key) ?? 0) < 2) {
        nodes.push(node);
        continue;
      }
      let bundle = bundles.get(key);
      if (!bundle) {
        bundle = {
          optionId: null,
          label: bundleName,
          kind: "bundle",
          menuOnly: true,
          hasScreenshot: false,
          installed: false,
          conflicted: false,
          kinds: { ...ZERO_KINDS },
          children: [],
        };
        bundles.set(key, bundle);
        nodes.push(bundle);
      }
      bundle.children.push(node);
    }
    for (const b of bundles.values()) {
      b.menuOnly = b.children.every((k) => k.menuOnly);
      b.hasScreenshot = b.children.some((k) => k.hasScreenshot);
      b.installed = b.children.some((k) => k.installed);
      b.conflicted = b.children.some((k) => k.conflicted);
      for (const k of b.children) {
        b.kinds.pak += k.kinds.pak;
        b.kinds.loose += k.kinds.loose;
        b.kinds.reframework += k.kinds.reframework;
        b.kinds.root += k.kinds.root;
      }
    }
    return nodes;
  }

  function nest(pkg: MockPackage): TreeNode[] {
    const own = new Set(pkg.options.map((o) => o.id));
    const children = new Map<string, MockOption[]>();
    const top: MockOption[] = [];
    for (const o of pkg.options) {
      if (o.parent && own.has(o.parent)) children.set(o.parent, [...(children.get(o.parent) ?? []), o]);
      else top.push(o);
    }
    return nestLevel(top, children, new Set());
  }

  function libraryPackage(pkg: MockPackage): LibraryPackage {
    const labels = labelsOf(pkg);
    return {
      package: pkg.summary,
      installable: pkg.options.filter((o) => !o.menuOnly).length,
      installed: pkg.options.filter((o) => isInstalled(o.id)).length,
      update: updateBadge(pkg),
      labels,
      adult: labels?.adult ?? false,
      conflicted: pkg.options.some((o) => conflicted(o.id)),
      family: familyView(pkg),
      tree: nest(pkg),
    };
  }

  // --- what is in the game ------------------------------------------------
  function pakViews(record: InstallRecord): PakView[] {
    const position = Math.max(0, pakOrder.indexOf(record.optionId));
    const loaded = Date.parse(record.installedAt) < LOG_STARTED_MS;
    return record.files.flatMap((file): PakView[] =>
      file.target.kind === "pak"
        ? [
            {
              fileName: pakName(record.optionId, file.target.fileName, position),
              size: file.size,
              load: loaded
                ? {
                    kind: "loaded",
                    slot: 103 + position,
                    nativeName: `re_chunk_000.pak.sub_000.pak.patch_${String(position + 1).padStart(3, "0")}.pak`,
                  }
                : { kind: "pending" },
            },
          ]
        : [],
    );
  }

  function installedRows(): InstalledRow[] {
    return installs.map((record) => ({
      optionId: record.optionId,
      name: record.name,
      packageName: record.packageName,
      packageMissing: findOption(record.optionId) === null,
      version: record.version,
      installedAt: record.installedAt,
      gameBuild: "25290080",
      origin: record.origin,
      kinds: record.kinds,
      fileCount: record.files.length,
      paks: pakViews(record),
    }));
  }

  /** Hold the game folder for one change, as the spine's deploy lock does. */
  async function holding<T>(work: () => Promise<T>): Promise<T> {
    changing = true;
    try {
      return await work();
    } finally {
      changing = false;
    }
  }

  // --- Play ----------------------------------------------------------------
  /** rift-deploy launch.rs plan_launch's refusals, word for word. */
  function launchRefusal(): string | null {
    if (!gameRoot) return "Dragon's Dogma 2 was not found, so pick the game folder before you play.";
    if (running) {
      return "Dragon's Dogma 2 is already running; switch to its window, or close it before you start it again.";
    }
    if (changing) {
      return "The Riftstone is changing the game folder right now (an install, uninstall, reorder, switch or restore), so try Play again once that is done.";
    }
    return null;
  }

  function launchWarnings(): LaunchWarning[] {
    const paks = installs.reduce((n, r) => n + r.files.filter((f) => f.target.kind === "pak").length, 0);
    const warnings: LaunchWarning[] = [];
    if (scene.has("refoff") && paks) warnings.push({ kind: "pakFolderOff", paks });
    if (scene.has("patched")) warnings.push({ kind: "buildChanged", recorded: "25290080", now: "25391144" });
    return warnings;
  }

  function setRunning(next: boolean): void {
    if (running === next) return;
    running = next;
    startedMs = next ? Date.now() : null;
    emit(EVENTS.gameChanged, {});
  }

  // --- the far world with a key ----------------------------------------------
  /** Why a key call cannot go out now; null when it can. */
  function keyRefusal(): RiftFailure | null {
    if (keyStanding === "none") return nexusFailure(NO_KEY_SENTENCE, "noKey");
    if (keyStanding === "rejected") return nexusFailure(KEY_REJECTED_SENTENCE, "keyRejected");
    if (scene.has("offline")) return nexusFailure(OFFLINE_SENTENCE, "offline");
    return null;
  }

  const pageUrlOf = (modId: number) => `https://www.nexusmods.com/dragonsdogma2/mods/${modId}`;
  const linkedCount = (modId: number) => packages.filter((p) => p.summary.nexus?.modId === modId).length;

  /** A mod the account tracks, as the spine's dto.rs `NexusCandidate::of_tracked`
   *  makes its card: named through the batched v2 mod lookup (name, version,
   *  category, adult flag, last update) and nothing else, so the author, the
   *  counts and the summary stay unknown. A mod the lookup does not know keeps
   *  a card, so it can be untracked. */
  function trackedCard(modId: number): NexusCandidate {
    const known = candidateFor(modId);
    return {
      modId,
      name: known?.name ?? `Nexus mod ${modId}`,
      author: null,
      version: known?.version ?? null,
      category: known?.category ?? null,
      adult: known?.adult ?? false,
      downloads: null,
      endorsements: null,
      summary: null,
      updatedMs: known?.updatedMs ?? null,
      pageUrl: pageUrlOf(modId),
      linkedPackages: linkedCount(modId),
    };
  }

  /** A mod as a card: from the library's own link, else the finds, else browsing. */
  function candidateFor(modId: number): NexusCandidate | null {
    const pkg = packages.find((p) => p.summary.nexus?.modId === modId);
    const known = [...Object.values(MOCK_FINDS).flat(), ...MOCK_BROWSE].find((c) => c.modId === modId);
    if (known) return { ...known, pageUrl: pageUrlOf(modId), linkedPackages: linkedCount(modId) };
    if (!pkg) return null;
    const labels = MOCK_LABELS[modId] as NexusLabels | undefined;
    return {
      modId,
      name: pkg.summary.displayName,
      author: pkg.options[0]?.info?.author ?? null,
      version: pkg.summary.nexus?.version ?? null,
      category: labels?.category ?? null,
      adult: labels?.adult ?? false,
      downloads: null,
      endorsements: null,
      summary: null,
      updatedMs: pkg.summary.nexus?.uploadedMs ?? null,
      pageUrl: pageUrlOf(modId),
      linkedPackages: linkedCount(modId),
    };
  }

  /** The file a library package is pinned to for this mod, if one is. */
  function heldFile(modId: number): { fileId: number | null; version: string | null } | null {
    const link = packages.find((p) => p.summary.nexus?.modId === modId && p.summary.nexus.fileId !== null)?.summary
      .nexus;
    return link ? { fileId: link.fileId, version: link.version } : null;
  }

  function filesOf(modId: number) {
    const card = candidateFor(modId);
    if (!card) return null;
    const pinned = new Set(
      packages.flatMap((p) =>
        p.summary.nexus?.modId === modId && p.summary.nexus.fileId !== null ? [p.summary.nexus.fileId] : [],
      ),
    );
    const files = mockFiles(modId, card.name, heldFile(modId))
      .map((f) => ({ ...f, inLibrary: pinned.has(f.fileId) }))
      .sort((a, b) => FILE_ORDER[a.category] - FILE_ORDER[b.category] || b.uploadedMs - a.uploadedMs);
    return { card, files };
  }

  // tracked mods and endorsements (v1 user/tracked_mods.json, user/endorsements.json)
  const tracked = new Set<number>(MOCK_TRACKED);
  const endorsed = new Map<number, EndorseStatus>(MOCK_ENDORSEMENTS.map((e) => [e.modId, e.status]));
  let standingsCheckedMs = Date.now() - 10 * 60_000;
  /** When a download of each mod landed this session (endorsing waits 15 minutes). */
  const landedAt = new Map<number, number>();

  function standings(): NexusStandings {
    return {
      tracked: [...tracked].map(trackedCard),
      endorsements: [...endorsed].map(([modId, status]) => ({ modId, status })),
      checkedMs: standingsCheckedMs,
    };
  }

  function endorseRefusal(modId: number): RiftFailure | null {
    if (modId === MOCK_OWN_MOD) return nexusFailure("Nexus refused: you cannot endorse your own mod.", "endorseOwnMod");
    const landed = landedAt.get(modId);
    if (landed !== undefined && Date.now() - landed < 15 * 60_000) {
      return nexusFailure("Nexus refused: wait 15 minutes after a download to endorse.", "endorseTooSoon");
    }
    if (landed === undefined && !linkedCount(modId)) {
      return nexusFailure("Nexus refused: this account has not downloaded the mod.", "endorseNotDownloaded");
    }
    return null;
  }

  // --- downloads into the mods folder --------------------------------------
  let downloads: DownloadView[] = [];
  let downloadSeq = 0;
  const queue: string[] = [];
  /** Downloads an nxm link's key and expiry may fetch (a free account's). */
  const granted = new Set<string>();
  /** Downloads being put in place in the mods folder: a cancel is refused now. */
  const committed = new Set<string>();
  let working = false;

  function setPhase(row: DownloadView, phase: DownloadPhase): void {
    row.phase = phase;
    if (OVER.has(phase)) row.finishedMs = Date.now();
    emit(EVENTS.downloadsChanged, {});
  }

  function failDownload(row: DownloadView, failure: RiftError): void {
    row.failure = failure;
    setPhase(row, "failed");
  }

  /** Nexus's archive name for a file (the classic `<name>-<mod>-<version>-<stamp>` shape). */
  function archiveName(row: DownloadView, uploadedMs: number): string {
    const stem = (row.modName ?? `mod ${row.modId}`).replace(/[^A-Za-z0-9 ]+/g, "").trim();
    const version = (row.version ?? "1").replace(/\./g, "-");
    return `${stem}-${row.modId}-${version}-${Math.floor(uploadedMs / 1000)}.zip`;
  }

  /** The landing, in the spine's order (downloads.rs fetch → pre_link): the
   *  archive's package is linked to the exact file BEFORE any scan shelves it
   *  (rift://updates-changed, while get_updates cannot list it yet), the row
   *  lands (rift://downloads-changed), and then a scan shelves the package
   *  (rift://library-changed). A file that was there already is linked and told
   *  just the same, and nothing new is scanned. */
  function land(row: DownloadView): void {
    const file = filesOf(row.modId)?.files.find((f) => f.fileId === row.fileId);
    const uploadedMs = file?.uploadedMs ?? MOCK_NOW;
    const archive = archiveName(row, uploadedMs);
    const existing = packages.find((p) => p.summary.fileName.toLowerCase() === archive.toLowerCase());
    let arriving: MockPackage | null = null;
    if (existing) {
      // pre_link: the package the same bytes already are takes the exact link
      if (existing.summary.nexus?.modId !== row.modId) unjudged.add(existing.summary.id);
      existing.summary.nexus = {
        modId: row.modId,
        fileId: row.fileId,
        groupId: null,
        sqid: null,
        version: row.version,
        uploadedMs,
        source: "download",
      };
      row.alreadyHad = true;
      row.landedAs = existing.summary.fileName;
      row.packageId = existing.summary.id;
    } else {
      // pre_link: the link waits in nexus.json for the package the scan will find
      arriving = downloadedPackage({
        fileName: archive,
        displayName: row.modName ?? `Nexus mod ${row.modId}`,
        size: row.bytesTotal ?? 1_048_576,
        modId: row.modId,
        fileId: row.fileId,
        groupId: null,
        version: row.version,
        uploadedMs,
        author: candidateFor(row.modId)?.author ?? null,
      });
      row.landedAs = archive;
      row.packageId = arriving.summary.id;
    }
    landedAt.set(row.modId, Date.now());
    emit(EVENTS.updatesChanged, {});
    committed.delete(row.id);
    setPhase(row, "landed");
    if (!arriving) return;
    const shelved = arriving;
    // request_scan: the mods folder's new archive is shelved by the next scan
    setTimeout(() => {
      packages = [...packages, shelved];
      unjudged.add(shelved.summary.id);
      emit(EVENTS.libraryChanged, {});
    }, LANDING_SCAN_MS);
  }

  async function runDownload(row: DownloadView): Promise<void> {
    setPhase(row, "linking");
    await wait(250);
    if (phaseOf(row) !== "linking") return; // cancelled while asking
    const refusal = keyRefusal();
    if (refusal) {
      failDownload(row, { kind: "nexus", detail: refusal.detail });
      return;
    }
    if (!premium && !granted.has(row.id)) {
      failDownload(
        row,
        nexusError(
          "Nexus Mods lets only premium accounts download straight from the Riftstone. Press Mod Manager Download on the file's Nexus Mods page instead; the Riftstone takes the download over from there.",
          "premiumRequired",
        ),
      );
      return;
    }
    quotaSpent += 1;
    const total = filesOf(row.modId)?.files.find((f) => f.fileId === row.fileId)?.size ?? 4_194_304;
    row.bytesTotal = total;
    setPhase(row, "downloading");
    const chunks = 6;
    for (let i = 1; i <= chunks; i++) {
      await wait(210);
      if (phaseOf(row) !== "downloading") return; // cancelled: the partial file goes
      row.bytesDone = Math.round((total * i) / chunks);
      emit(EVENTS.downloadProgress, { id: row.id, phase: "downloading", bytesDone: row.bytesDone, bytesTotal: total });
    }
    setPhase(row, "verifying");
    await wait(250); // the size, then the MD5 round trip
    if (phaseOf(row) !== "verifying") return;
    // the landing commits: from here a cancel is refused (downloads.rs commit)
    committed.add(row.id);
    await wait(60);
    land(row);
  }

  /** One download at a time, in order. */
  async function work(): Promise<void> {
    if (working) return;
    working = true;
    try {
      for (let id = queue.shift(); id !== undefined; id = queue.shift()) {
        const row = downloads.find((d) => d.id === id);
        if (row && phaseOf(row) === "queued") await runDownload(row);
      }
    } finally {
      working = false;
    }
  }

  function enqueue(row: DownloadView): void {
    row.nexusPageUrl = null;
    setPhase(row, "queued");
    queue.push(row.id);
    // the worker picks it up a beat later, so the caller sees "queued" first
    setTimeout(() => void work(), 60);
  }

  function newDownload(modId: number, fileId: number, forPackageId: string | null): DownloadView {
    const found = filesOf(modId);
    const file = found?.files.find((f) => f.fileId === fileId);
    if (!found || !file) throw nexusFailure("Nexus knows no such file for that mod.", "fileGone");
    const row: DownloadView = {
      id: `download-${++downloadSeq}`,
      modId,
      fileId,
      modName: found.card.name,
      fileName: file.name,
      version: file.version,
      forPackageId,
      phase: "waiting",
      bytesDone: 0,
      bytesTotal: null,
      landedAs: null,
      packageId: null,
      alreadyHad: false,
      failure: null,
      nexusPageUrl: `${pageUrlOf(modId)}?tab=files&file_id=${fileId}&nmm=1`,
      startedMs: Date.now(),
      finishedMs: null,
    };
    downloads = [row, ...downloads];
    return row;
  }

  // --- the nxm:// handler and the links it brings --------------------------
  const otherAtStart = scene.has("nxmother") ? MOCK_OTHER_HANDLER : null;

  let handler: NxmHandlerView = otherAtStart
    ? { state: "other", other: otherAtStart, givesBackTo: null }
    : { state: "none", other: null, givesBackTo: null };

  /** What an nxm:// link says, read the way rift-nexus nxm.rs parse_nxm reads
   *  it: a refusal before any check (another game, a collection, a link that
   *  cannot be read) carries no ids. */
  type NxmRead =
    | { premium: true }
    | { refused: string }
    | { modId: number; fileId: number; grant: { expiresS: number } | null; userId: number | null };

  const nxmId = (text: string | undefined): number | null => {
    if (!text || !/^\d+$/.test(text)) return null;
    const id = Number(text);
    return id >= 1 && id <= LARGEST_NEXUS_ID ? id : null;
  };

  /** key, expires and user_id, the first of each, blank values ignored; null: unreadable. */
  function nxmParams(query: string): Map<string, string> | null {
    const params = new Map<string, string>();
    for (const pair of query.split("&").filter(Boolean)) {
      const at = pair.indexOf("=");
      let name: string;
      let value: string;
      try {
        name = decodeURIComponent(at < 0 ? pair : pair.slice(0, at)).toLowerCase();
        value = decodeURIComponent(at < 0 ? "" : pair.slice(at + 1)).trim();
      } catch {
        return null;
      }
      if (["key", "expires", "user_id"].includes(name) && value && !params.has(name)) params.set(name, value);
    }
    return params;
  }

  /** The grant and the account a file link names; null when they make it unreadable. */
  function nxmGrant(query: string): Pick<Extract<NxmRead, { modId: number }>, "grant" | "userId"> | null {
    const params = nxmParams(query);
    if (!params) return null;
    const key = params.get("key");
    const expires = params.get("expires");
    // a key without its expiry, or the other way round, is no grant at all
    if ((key === undefined) !== (expires === undefined)) return null;
    if (key !== undefined && !/^[\x21-\x7e]{1,512}$/.test(key)) return null;
    if (expires !== undefined && !/^\d+$/.test(expires)) return null;
    const user = params.get("user_id");
    const userId = user === undefined ? null : nxmId(user);
    if (user !== undefined && userId === null) return null;
    return { grant: expires === undefined ? null : { expiresS: Number(expires) }, userId };
  }

  function readNxm(link: string): NxmRead {
    const text = link
      .trim()
      .replace(/^[<>"']+|[<>"']+$/g, "")
      .trim();
    const m = /^nxm:\/\/([^/?#]*)([^?#]*)(?:\?([^#]*))?/i.exec(text);
    if (!m) return { refused: NXM_UNREADABLE };
    // host, path, query: the query group may not take part at all
    const groups: (string | undefined)[] = [...m];
    const host = (groups[1] ?? "").toLowerCase();
    if (!/^[a-z0-9_-]+$/.test(host)) return { refused: NXM_UNREADABLE };
    if (host === "premium") return { premium: true };
    if (host !== "dragonsdogma2") return { refused: NXM_OTHER_GAME };
    const segments = (groups[2] ?? "")
      .split("/")
      .filter(Boolean)
      .map((s) => s.toLowerCase());
    if (segments[0] === "collections") return { refused: NXM_COLLECTION };
    const [mods, mod, files, file, ...rest] = segments;
    const modId = nxmId(mod);
    const fileId = nxmId(file);
    const grant = nxmGrant(groups[3] ?? "");
    if (mods !== "mods" || files !== "files" || rest.length || modId === null || fileId === null || !grant) {
      return { refused: NXM_UNREADABLE };
    }
    return { modId, fileId, ...grant };
  }

  /** The spine's nxm.rs receive, its checks in its order: what refuses a file link. */
  function nxmRefusal(read: Extract<NxmRead, { modId: number }>): RiftError | null {
    if (keyStanding === "none") return nexusError(NO_KEY_SENTENCE, "noKey");
    if (keyStanding === "rejected") return nexusError(KEY_REJECTED_SENTENCE, "keyRejected");
    // the site's button always sends its grant: a link without one is not the button's
    if (!read.grant) return { kind: "refused", detail: { reason: NXM_UNREADABLE } };
    if (Math.floor(Date.now() / 1000) >= read.grant.expiresS) return nexusError(LINK_EXPIRED_SENTENCE, "linkExpired");
    if (read.userId !== null && read.userId !== MOCK_USER_ID) return nexusError(NXM_NOT_YOURS, "linkNotYours");
    return null;
  }

  function receiveNxm(link: string): void {
    const read = readNxm(link);
    if ("premium" in read) {
      if (keyStanding !== "none") accountCheckedMs = Date.now();
      emit(EVENTS.updatesChanged, {});
      emit(EVENTS.nxmReceived, { modId: null, fileId: null, accepted: false, downloadId: null, refusal: null });
      return;
    }
    if ("refused" in read) {
      const refusal: RiftError = { kind: "refused", detail: { reason: read.refused } };
      emit(EVENTS.nxmReceived, { modId: null, fileId: null, accepted: false, downloadId: null, refusal });
      return;
    }
    const ids = { modId: read.modId, fileId: read.fileId };
    const refusal = nxmRefusal(read);
    if (refusal) {
      emit(EVENTS.nxmReceived, { ...ids, accepted: false, downloadId: null, refusal });
      return;
    }
    // the waiting download of this file resumes; one already on its way takes the link; else a new one
    const same = (d: DownloadView) => d.modId === read.modId && d.fileId === read.fileId;
    const waiting = downloads.find((d) => same(d) && d.phase === "waiting");
    const onItsWay = downloads.find((d) => same(d) && !OVER.has(d.phase));
    let row: DownloadView;
    try {
      row = waiting ?? onItsWay ?? newDownload(read.modId, read.fileId, null);
    } catch (raw) {
      const failure = raw instanceof RiftFailure ? { kind: raw.kind, detail: raw.detail } : null;
      emit(EVENTS.nxmReceived, { ...ids, accepted: false, downloadId: null, refusal: failure });
      return;
    }
    granted.add(row.id);
    if (row.phase === "waiting") enqueue(row);
    emit(EVENTS.nxmReceived, { ...ids, accepted: true, downloadId: row.id, refusal: null });
  }

  const backend: MockBackend = {
    // Pending contract (services/pending.ts): rendered here so every §9 state
    // has a frame; the spine grows into the same duck-typed shapes.
    async troubledPackages() {
      await wait(30);
      // The spine's rule (WINDOW-CONTRACT get_troubled_packages): every
      // package carrying one of the four trouble kinds, ordered by the first
      // it carries, then library order.
      const rank = (p: MockPackage) => {
        const at = TROUBLE_KINDS.findIndex((kind) => p.summary.problems.some((x) => x.kind === kind));
        return at === -1 ? TROUBLE_KINDS.length : at;
      };
      const troubled = packages.filter((p) => rank(p) < TROUBLE_KINDS.length);
      return [...troubled].sort((a, b) => rank(a) - rank(b)).map((p) => p.summary);
    },
    async nexusQuota(): Promise<NexusQuota | null> {
      await wait(20);
      // WINDOW-CONTRACT get_nexus_quota: null without a key — anonymous v2
      // checks carry no quota headers (RS-6); v0.3: the launch validation is a
      // v1 call, so a stored key that Nexus took shows its quota from the start.
      if (keyStanding === "none" || keyStanding === "rejected") return null;
      const low = scene.has("quotalow");
      return {
        hourlyRemaining: Math.max(0, (low ? 12 : 486) - quotaSpent),
        hourlyLimit: 500,
        dailyRemaining: Math.max(0, (low ? 1_204 : 19_840) - quotaSpent),
        dailyLimit: 20_000,
        hourlyResetMs: MOCK_NOW + 25 * 60_000,
      };
    },
    async getOverview() {
      await wait(40);
      return overview();
    },
    async rescan() {
      if (scanning) return;
      scanning = true;
      const total = packages.length;
      for (let i = 0; i < total; i++) {
        await wait(120);
        emit(EVENTS.scanProgress, { done: i + 1, total, current: packages[i].summary.fileName });
      }
      scanning = false;
      scannedOnce = true;
      emit(EVENTS.libraryChanged, {});
    },
    async search(query) {
      await wait(8);
      return search(query);
    },
    async getOption(optionId) {
      await wait(30);
      const found = findOption(optionId);
      if (!found) throw notFound(optionId);
      const { pkg, option } = found;
      const children: OptionRef[] = pkg.options
        .filter((c) => c.parent === option.id)
        .map((c) => ({
          optionId: c.id,
          name: c.name,
          menuOnly: c.menuOnly,
          hasScreenshot: Boolean(c.screenshot),
          installed: isInstalled(c.id),
        }));
      const record = recordOfId(optionId);
      const detail: OptionDetail = {
        option,
        package: pkg.summary,
        parentName: option.parent ? (pkg.options.find((p) => p.id === option.parent)?.name ?? null) : null,
        children,
        installed: record
          ? {
              seq: record.seq,
              installedAt: record.installedAt,
              version: record.version,
              gameBuild: "25290080",
              origin: record.origin,
              files: record.files.map((p) => ({
                gamePath: gamePathOf(optionId, p.target),
                kind: p.target.kind,
                size: p.size,
              })),
            }
          : null,
        conflicts: conflicts().filter((c) => c.layers.some((l) => l.optionId === optionId)),
        update: updateRow(pkg),
        labels: labelsOf(pkg),
      };
      return detail;
    },
    async getPackage(packageId) {
      await wait(30);
      const pkg = packages.find((p) => p.summary.id === packageId);
      if (!pkg) throw notFound(packageId);
      const detail: PackageDetail = { package: pkg.summary, tree: nest(pkg), entries: pkg.entries };
      return detail;
    },
    async getLibrary() {
      await wait(30);
      const blind = (s: string) => s.toLowerCase();
      const shelf = [...packages].sort(
        (a, b) =>
          blind(a.summary.displayName).localeCompare(blind(b.summary.displayName)) ||
          blind(a.summary.fileName).localeCompare(blind(b.summary.fileName)),
      );
      return { mods: modsFolder(), packages: shelf.map(libraryPackage) };
    },
    async getThumbnail(optionId, size) {
      await wait(size === "card" ? 20 : 60);
      const found = findOption(optionId);
      return found?.option.screenshot ? plate(found.option.name, size) : null;
    },
    async planInstall(optionId) {
      await wait(40);
      const found = findOption(optionId);
      if (!found) throw notFound(optionId);
      const { option } = found;
      const reinstall = isInstalled(optionId);
      // v0.5: the replaced installs leave first, so a path one held is met after its removal
      const replaced = replacedBy(optionId);
      const held = new Set(replaced.flatMap((r) => r.record.files.map((f) => gamePathOf(r.record.optionId, f.target))));
      const takenSlot = replaced.map((r) => pakOrder.indexOf(r.record.optionId)).find((i) => i >= 0);
      const slot = reinstall ? pakOrder.indexOf(optionId) : (takenSlot ?? pakOrder.length);
      const writes: PlannedWrite[] = option.payload.map((p) => {
        const gamePath = gamePathOf(optionId, p.target, slot);
        return {
          entry: p.entry,
          gamePath,
          kind: p.target.kind,
          size: p.size,
          action: reinstall || held.has(gamePath) ? { kind: "reinstall" } : { kind: "create" },
        };
      });
      const plan: InstallPlanView = {
        optionId,
        name: option.name,
        writes,
        totalBytes: writes.reduce((n, w) => n + w.size, 0),
        warnings: planWarnings(option),
        replaces: replaced.map(replacesView),
        refused: planRefusal(option),
      };
      return plan;
    },
    async install(optionId) {
      const found = findOption(optionId);
      if (!found) throw notFound(optionId);
      const refusal = planRefusal(found.option);
      if (refusal) throw refusedWith(refusal);
      return holding(async () => {
        const total = found.option.payload.length;
        const bytesTotal = found.option.payload.reduce((n, p) => n + p.size, 0);
        let bytesDone = 0;
        for (const [i, file] of found.option.payload.entries()) {
          await wait(220);
          bytesDone += file.size;
          emit(EVENTS.installProgress, { optionId, filesDone: i + 1, filesTotal: total, bytesDone, bytesTotal });
        }
        // v0.5: the replaced installs go in the same step; the new one takes the first's pak slot
        let slot = -1;
        for (const { record } of replacedBy(optionId)) {
          const at = pakOrder.indexOf(record.optionId);
          if (slot < 0 && at >= 0) slot = at;
          installs = installs.filter((r) => r !== record);
          pakOrder = pakOrder.filter((id) => id !== record.optionId);
        }
        if (!isInstalled(optionId)) {
          installs.push(recordOf(found.option, found.pkg, { seq: ++seq, installedAt: new Date().toISOString() }));
          if (found.option.kinds.pak) {
            if (slot >= 0) pakOrder.splice(slot, 0, optionId);
            else pakOrder.push(optionId);
          }
        }
        const moved = putAwayAfter(found.pkg);
        emit(EVENTS.gameChanged, {});
        if (moved.putAway.length) emit(EVENTS.libraryChanged, {});
        const result: InstallResultView = {
          optionId,
          filesWritten: total,
          bytesWritten: bytesTotal,
          backups: 0,
          durationMs: 220 * total,
          // the game reads pak_mods/ and natives/ at start: this one loads next launch
          restartNeeded: running,
          ...moved,
        };
        return result;
      });
    },
    async planUninstall(optionId) {
      await wait(30);
      const record = recordOfId(optionId);
      const found = findOption(optionId);
      if (!record && !found) throw notFound(optionId);
      const removals: PlannedRemoval[] = (record?.files ?? []).map((p) => ({
        gamePath: gamePathOf(optionId, p.target),
        kind: p.target.kind,
        action: removalAction(p.target),
      }));
      const name = record?.name ?? found?.option.name ?? "";
      let refused: string | null = null;
      if (!record) refused = "This option is not installed.";
      else if (running && hasPaks(record)) refused = RUNNING_UNINSTALL(record.name);
      const plan: UninstallPlanView = { optionId, name, removals, refused };
      return plan;
    },
    async uninstall(optionId) {
      const record = recordOfId(optionId);
      if (record && running && hasPaks(record)) throw refusedWith(RUNNING_UNINSTALL(record.name));
      return holding(async () => {
        await wait(300);
        const name = record?.name ?? "";
        const pakSlot = pakOrder.indexOf(optionId);
        installs = installs.filter((r) => r.optionId !== optionId);
        pakOrder = pakOrder.filter((id) => id !== optionId);
        const files = record?.files ?? [];
        const kinds = files.map((p) => removalAction(p.target).kind);
        for (const p of files) {
          if (p.target.kind === "pak") continue;
          gonePaths.delete(p.target.gamePath);
          changedPaths.delete(p.target.gamePath);
        }
        for (const [path, holder] of heldBy) {
          if (holder !== name) continue;
          heldBy.delete(path);
          parked = parked.map((f) => (f.gamePath === path ? { ...f, occupied: false } : f));
        }
        // Sacred Arbor Fog Removal's pak changed since install: the engine keeps
        // a changed file aside instead of deleting it (it lands under PARKED).
        const parks = name === "Sacred Arbor Fog Removal" ? files.filter((p) => p.target.kind === "pak") : [];
        for (const p of parks) {
          const gamePath = gamePathOf(optionId, p.target, pakSlot);
          parked.push({
            store: parkStore(gamePath),
            gamePath,
            owner: name,
            ownerKind: "riftstone",
            size: p.size,
            parkedAt: new Date().toISOString(),
            reason: `${name}'s uninstall found it changed since install, so it was kept aside instead of deleted.`,
            occupied: false,
          });
        }
        emit(EVENTS.gameChanged, {});
        const count = (k: string) => kinds.filter((x) => x === k).length;
        const result: UninstallResultView = {
          optionId,
          filesRemoved: count("delete") - parks.length,
          filesRestored: count("restoreBackup"),
          filesKept: count("keepChanged") + count("keepVanilla"),
          filesHandedDown: count("handDown"),
          parked: parks.length,
          restartNeeded: running,
        };
        return result;
      });
    },
    async getInstalled() {
      await wait(40);
      const view: InstalledView = {
        installs: installedRows(),
        pakOrder,
        conflicts: conflicts(),
        strays: scene.has("empty") ? [] : [{ gamePath: "pak_mods/old_test_mod.pak", size: 1_048_576 }],
        missing: installs.flatMap((record) =>
          record.files.flatMap((p) =>
            p.target.kind !== "pak" && gonePaths.has(p.target.gamePath)
              ? [{ optionId: record.optionId, optionName: record.name, gamePath: p.target.gamePath }]
              : [],
          ),
        ),
        pakLoad: {
          logStarted: new Date(LOG_STARTED_MS).toISOString(),
          // Agrees with the rows' "loaded · slot N": the same paks, the same slots.
          injected: installs.flatMap((record) =>
            pakViews(record).flatMap((v) =>
              v.load.kind === "loaded"
                ? [{ fileName: v.fileName, nativeName: v.load.nativeName, slot: v.load.slot }]
                : [],
            ),
          ),
          error: null,
        },
        pakCountWarning: 100,
        parked: parked.map((f) => ({ ...f })),
      };
      return view;
    },
    async reorderPaks(order) {
      if (running) throw refusedWith(RUNNING_REORDER);
      await holding(async () => {
        await wait(80);
        pakOrder = [...order];
      });
      emit(EVENTS.gameChanged, {});
    },
    async setReframeworkSwitch() {
      if (running) throw refusedWith(RUNNING_SWITCH);
      await holding(() => wait(60));
      scene.delete("refoff");
      emit(EVENTS.gameChanged, {});
    },
    async setGameRoot(path) {
      await wait(60);
      gameRoot = path;
      gameSource = "manual";
      emit(EVENTS.gameChanged, {});
      return game() as GameSummary;
    },
    async openModsFolder() {
      await wait(30);
      // the spine's open_mods_folder_impl: a missing folder is made first (a
      // folder that could not be made at launch fails again the same way)...
      if (folder === "unmade") {
        throw new RiftFailure({
          kind: "io",
          detail: { detail: `could not make the mods folder ${MOCK_MODS_DIR}: ${MODS_UNMADE_WHY}` },
        });
      }
      if (folder === "gone") {
        // ...then watched afresh and scanned: the new folder is empty, so the
        // scan shelves nothing and says so (rift://library-changed)
        folder = "there";
        setTimeout(() => emit(EVENTS.libraryChanged, {}), LANDING_SCAN_MS);
      }
      // ...and Explorer opens it; the mock has nothing to open
    },
    async planLaunch() {
      await wait(40);
      const plan: LaunchPlanView = {
        // no game: the spine's commands/play.rs no_game() plans the exe route
        launch: game()?.launch ?? "exe",
        warnings: launchWarnings(),
        refused: launchRefusal(),
      };
      return plan;
    },
    async launchGame() {
      await wait(120);
      const refusal = launchRefusal();
      if (refusal) throw refusedWith(refusal);
      // the watcher sees DD2.exe a moment after Steam or Windows starts it
      setTimeout(() => setRunning(true), 700);
      return { launch: game()?.launch ?? "exe", launchedMs: Date.now() };
    },
    async getUpdates() {
      await wait(30);
      return updatesView();
    },
    async checkUpdates() {
      const rows = updatesView().rows.filter((r) => r.link);
      for (let i = 0; i < rows.length; i++) {
        await wait(260);
        emit(EVENTS.updatesProgress, { phase: "checking", done: i + 1, total: rows.length });
      }
      if (scene.has("offline")) throw nexusFailure(OFFLINE_SENTENCE, "offline");
      lastCheckMs = Date.now();
      unjudged.clear();
      emit(EVENTS.updatesChanged, {});
      return updatesView();
    },
    async identifyPackages(packageIds) {
      await wait(400);
      // the hashes are local, the MD5 lookup is a Nexus call: offline it fails
      if (scene.has("offline")) throw nexusFailure(OFFLINE_SENTENCE, "offline");
      return packageIds.map((packageId): Md5Result => {
        const pkg = packages.find((p) => p.summary.id === packageId);
        if (!pkg || pkg.summary.displayName !== MOCK_MD5_MATCH.displayName || pkg.summary.nexus) {
          return { packageId, matched: false, link: null };
        }
        pkg.summary.nexus = {
          modId: MOCK_MD5_MATCH.modId,
          fileId: MOCK_MD5_MATCH.fileId,
          groupId: null,
          sqid: null,
          version: pkg.options[0]?.info?.version ?? null,
          uploadedMs: null,
          source: "md5",
        };
        return { packageId, matched: true, link: pkg.summary.nexus };
      });
    },
    async findOnNexus(packageId) {
      // MOCK_FINDS by display name; REFramework (and any name absent there)
      // answers none. The terms are the name lower-cased, as the spine's
      // search_terms leaves a plain name. Offline it fails like the spine.
      await wait(200);
      const pkg = packages.find((p) => p.summary.id === packageId);
      if (!pkg) throw notFound(packageId);
      const terms = pkg.summary.displayName
        .replace(/[_.+-]+/g, " ")
        .replace(/\s+/g, " ")
        .trim()
        .toLowerCase();
      if (!terms) return { packageId, terms: "", candidates: [] };
      if (scene.has("offline")) throw nexusFailure(OFFLINE_SENTENCE, "offline");
      const candidates = (MOCK_FINDS[pkg.summary.displayName] ?? []).slice(0, 5).map((c) => ({
        ...c,
        pageUrl: pageUrlOf(c.modId),
        linkedPackages:
          c.linkedPackages +
          // the spine counts every linked package (for an unlinked row, the same as "others")
          linkedCount(c.modId),
      }));
      return { packageId, terms, candidates };
    },
    async linkPackage(packageId, urlOrId) {
      await wait(120);
      const pkg = packages.find((p) => p.summary.id === packageId);
      if (!pkg) throw notFound(packageId);
      // the spine's parse_mod_url: ".../mods/<id>" first, else a bare id
      const text = urlOrId.trim();
      const id = Number(/\/mods\/(\d+)/.exec(text)?.[1] ?? /^\d+$/.exec(text)?.[0] ?? Number.NaN);
      if (!Number.isFinite(id)) throw nexusFailure("That is not a Dragon's Dogma 2 mod link or id.");
      // a files-tab URL names the file (page_url.rs): that one link pins at once
      const fileId = Number(/[?&]file_id=(\d+)/i.exec(text)?.[1] ?? Number.NaN);
      pkg.summary.nexus = {
        modId: id,
        fileId: Number.isFinite(fileId) ? fileId : null,
        groupId: null,
        sqid: null,
        version: null,
        uploadedMs: null,
        source: "manual",
      };
      unjudged.add(packageId);
      emit(EVENTS.updatesChanged, {});
      return updateRow(pkg) as UpdateRow;
    },
    async unlinkPackage(packageId) {
      await wait(60);
      const pkg = packages.find((p) => p.summary.id === packageId);
      if (pkg) pkg.summary.nexus = null;
      unjudged.delete(packageId);
      emit(EVENTS.updatesChanged, {});
    },
    async setNexusKey(key) {
      await wait(200);
      if (!key) {
        keyStanding = "none";
        return nexusSummary();
      }
      // validate.json first: a key Nexus refuses, or one it could not be asked
      // about, is not stored (the spine's set_nexus_key; the old key stays)
      if (scene.has("offline")) throw nexusFailure(OFFLINE_SENTENCE, "offline");
      // dev: a key with "reject" in it plays the key Nexus refuses
      if (/reject/i.test(key)) throw nexusFailure(KEY_REJECTED_SENTENCE, "keyRejected");
      keyStanding = "connected";
      accountCheckedMs = Date.now();
      return nexusSummary();
    },
    async refreshNexusAccount() {
      await wait(200);
      if (keyStanding === "none" || keyStanding === "rejected") return nexusSummary();
      if (scene.has("offline")) {
        keyStanding = "unchecked";
        throw nexusFailure(OFFLINE_SENTENCE, "offline");
      }
      keyStanding = "connected";
      accountCheckedMs = Date.now();
      return nexusSummary();
    },
    async getModFiles(modId) {
      await wait(200);
      if (scene.has("offline")) throw nexusFailure(OFFLINE_SENTENCE, "offline");
      const found = filesOf(modId);
      if (!found) throw nexusFailure("Nexus knows no Dragon's Dogma 2 mod with that id.", "fileGone");
      return {
        modId,
        modName: found.card.name,
        pageUrl: found.card.pageUrl,
        adult: found.card.adult,
        files: found.files,
      };
    },
    async browseNexus(query) {
      await wait(250);
      if (scene.has("offline")) throw nexusFailure(OFFLINE_SENTENCE, "offline");
      const words = query.text.toLowerCase().split(/\s+/).filter(Boolean);
      const matches = MOCK_BROWSE.filter((m) => words.every((w) => m.name.toLowerCase().includes(w)));
      const ORDER: Record<
        NexusBrowseSort,
        ((a: (typeof matches)[number], b: (typeof matches)[number]) => number) | null
      > = {
        endorsements: (a, b) => (b.endorsements ?? 0) - (a.endorsements ?? 0),
        downloads: (a, b) => (b.downloads ?? 0) - (a.downloads ?? 0),
        updated: (a, b) => (b.updatedMs ?? 0) - (a.updatedMs ?? 0),
        relevance: null,
      };
      const order = ORDER[query.sort];
      const sorted = order ? [...matches].sort(order) : matches;
      const offset = Math.max(0, query.offset);
      return {
        total: sorted.length,
        offset,
        mods: sorted.slice(offset, offset + PAGE).map((m) => ({
          ...m,
          pageUrl: pageUrlOf(m.modId),
          linkedPackages: linkedCount(m.modId),
        })),
      };
    },
    async getNexusStandings(refresh) {
      await wait(refresh ? 300 : 40);
      const refusal = keyRefusal();
      if (refusal) throw refusal;
      if (refresh) {
        quotaSpent += 2;
        standingsCheckedMs = Date.now();
      }
      return standings();
    },
    async setTracked(modId, on) {
      await wait(150);
      const refusal = keyRefusal();
      if (refusal) throw refusal;
      quotaSpent += 1;
      if (on) tracked.add(modId);
      else tracked.delete(modId);
      standingsCheckedMs = Date.now();
      emit(EVENTS.updatesChanged, {});
      return standings();
    },
    async setEndorsement(modId, status) {
      await wait(150);
      const refusal = keyRefusal() ?? endorseRefusal(modId);
      if (refusal) throw refusal;
      quotaSpent += 1;
      endorsed.set(modId, status);
      standingsCheckedMs = Date.now();
      emit(EVENTS.updatesChanged, {});
      return standings();
    },
    async getDownloads() {
      await wait(20);
      return structuredClone(downloads);
    },
    async startDownload(modId, fileId, forPackageId) {
      await wait(60);
      // the spine's downloads.rs start, its checks in its order: no key, then a
      // download of this file already on its way answers as it is (no second
      // row, no event), then the key's standing, then the file on Nexus
      if (keyStanding === "none") throw nexusFailure(NO_KEY_SENTENCE, "noKey");
      const onItsWay = downloads.find((d) => d.modId === modId && d.fileId === fileId && !OVER.has(d.phase));
      if (onItsWay) return structuredClone(onItsWay);
      const refusal = keyRefusal();
      if (refusal) throw refusal;
      const row = newDownload(modId, fileId, forPackageId);
      // premium: queued at once; free: waits for the site's Mod Manager Download link
      if (premium) enqueue(row);
      else emit(EVENTS.downloadsChanged, {});
      return structuredClone(row);
    },
    async cancelDownload(id) {
      await wait(40);
      // the spine's downloads.rs cancel: its refusals word for word; the bytes stay as they were
      const row = downloads.find((d) => d.id === id);
      if (!row) throw refusedWith(CANCEL_NOT_LISTED);
      if (OVER.has(row.phase)) throw refusedWith(CANCEL_OVER);
      if (committed.has(id)) throw refusedWith(CANCEL_LANDING);
      const at = queue.indexOf(id);
      if (at >= 0) queue.splice(at, 1);
      granted.delete(id);
      row.nexusPageUrl = null;
      setPhase(row, "cancelled");
    },
    async clearDownloads() {
      await wait(30);
      const before = downloads.length;
      downloads = downloads.filter((d) => !OVER.has(d.phase));
      // the spine tells the window only when a row went
      if (downloads.length !== before) emit(EVENTS.downloadsChanged, {});
    },
    async getNxmHandler() {
      await wait(30);
      return { ...handler };
    },
    async setNxmHandler(on) {
      await wait(120);
      if (on && handler.state !== "ours") {
        handler = { state: "ours", other: null, givesBackTo: handler.state === "other" ? handler.other : null };
      } else if (!on && handler.state === "ours") {
        const back = handler.givesBackTo;
        handler = back
          ? { state: "other", other: back, givesBackTo: null }
          : { state: "none", other: null, givesBackTo: null };
      }
      return { ...handler };
    },
    async getLabels() {
      await wait(15);
      return labelFacets();
    },
    async restoreParked(store) {
      await wait(120);
      const file = parked.find((f) => f.store === store);
      if (!file) throw nothingKept(store);
      if (file.size === null)
        throw refusedWith(
          `The kept file for "${file.gamePath}" is no longer in the backup store. There is nothing left to put back; discard the entry.`,
        );
      const holder = heldBy.get(file.gamePath);
      if (holder)
        throw refusedWith(
          `"${holder}" is installed over ${file.gamePath}, so the kept file cannot go back there now. Uninstall it first, then put the kept file back.`,
        );
      parked = parked.filter((f) => f.store !== store);
      let swappedOut: string | null = null;
      if (file.occupied) {
        swappedOut = parkStore(file.gamePath);
        parked.push({
          store: swappedOut,
          gamePath: file.gamePath,
          owner: "the file that sat there",
          ownerKind: "unowned",
          size: 3_072,
          parkedAt: new Date().toISOString(),
          reason: "Kept aside so a parked file could go back in its place.",
          occupied: true,
        });
      }
      emit(EVENTS.gameChanged, {});
      // while the game runs the file still goes back, and loads at the next launch
      const report: ParkedRestoreReport = {
        gamePath: file.gamePath,
        restored: true,
        swappedOut,
        restartNeeded: running,
      };
      return report;
    },
    async discardParked(store) {
      await wait(80);
      if (!parked.some((f) => f.store === store)) throw nothingKept(store);
      parked = parked.filter((f) => f.store !== store);
      emit(EVENTS.gameChanged, {});
    },
    getSettings() {
      return Promise.resolve(settings);
    },
    setSettings(next) {
      settings = { ...next };
      return Promise.resolve(settings);
    },
    on(event, handler) {
      const set = listeners.get(event) ?? new Set<Listener>();
      set.add(handler as Listener);
      listeners.set(event, set);
      return Promise.resolve(() => {
        set.delete(handler as Listener);
      });
    },
    openUrl(url) {
      if (typeof window !== "undefined") window.open(url, "_blank", "noopener");
      return Promise.resolve();
    },
    revealPath() {
      return Promise.resolve();
    },
    pickFolder() {
      return Promise.resolve("D:\\Games\\Dragons Dogma 2");
    },
    // --- v0.4: the Muster (./muster.ts owns it whole since v0.5) -------------
    getMusterSlots: () => muster.getMusterSlots(),
    getMuster: (slotId) => muster.getMuster(slotId),
    openSavesFolder: (slotId) => muster.openSavesFolder(slotId),
    musterMock: muster,
    // --- v0.5: versions of one file (./versions.ts, WINDOW-CONTRACT § v0.5) ---
    async planKeepNewest(packageId) {
      await wait(30);
      const pkg = packages.find((p) => p.summary.id === packageId);
      if (!pkg) throw notFound(packageId);
      const { packageId: lead, keeps, uninstalls, putsAway, refused } = keepNewestOf(pkg);
      const plan: KeepNewestPlanView = { packageId: lead, keeps, uninstalls, putsAway, refused };
      return plan;
    },
    async keepNewest(packageId) {
      const pkg = packages.find((p) => p.summary.id === packageId);
      if (!pkg) throw notFound(packageId);
      const plan = keepNewestOf(pkg);
      if (plan.refused) throw refusedWith(plan.refused);
      return holding(async () => {
        await wait(300);
        for (const record of plan.records) {
          installs = installs.filter((r) => r !== record);
          pakOrder = pakOrder.filter((id) => id !== record.optionId);
        }
        if (plan.records.length) emit(EVENTS.gameChanged, {});
        const putAway = plan.olderMembers.filter((m) => installedIn(m) === 0).map(moveToShelf);
        if (putAway.length) emit(EVENTS.libraryChanged, {});
        const result: KeepNewestResultView = { uninstalled: plan.records.length, putAway, putAwayFailed: null };
        return result;
      });
    },
    async putAway(packageId) {
      await wait(60);
      const pkg = packages.find((p) => p.summary.id === packageId);
      if (!pkg) throw notFound(packageId);
      const members = membersOf(pkg);
      if (!members) throw refusedWith(ONLY_VERSION(pkg));
      if (members[0] === pkg) {
        throw refusedWith(
          `"${pkg.summary.fileName}" is the newest version of its file, so it stays in the mods folder.`,
        );
      }
      if (installedIn(pkg) > 0) {
        throw refusedWith(`"${pkg.summary.fileName}" still has options in your world; uninstall them first.`);
      }
      const moved = moveToShelf(pkg);
      emit(EVENTS.libraryChanged, {});
      return moved;
    },
    async getOlderVersions() {
      await wait(30);
      const blind = (s: string) => s.toLowerCase();
      const items: OlderShelfItem[] = [...shelf.items]
        .sort((a, b) => blind(a.fileName).localeCompare(blind(b.fileName)))
        .map((item) => ({
          fileName: item.fileName,
          size: item.pkg.summary.size,
          modName: item.pkg.summary.nexus ? item.pkg.summary.displayName : null,
          version: item.pkg.summary.nexus?.version ?? null,
          newerInMods: newerInMods(item.pkg, packages),
        }));
      const view: OlderShelfView = { exists: shelf.exists, items };
      return view;
    },
    async bringBack(fileName) {
      await wait(60);
      const at = shelf.items.findIndex((i) => i.fileName.toLowerCase() === fileName.toLowerCase());
      if (at < 0) throw notFound(fileName);
      const item = shelf.items[at];
      const landedAs = freeName(
        item.fileName,
        packages.map((p) => p.summary.fileName),
      );
      shelf.items.splice(at, 1);
      const pkg = item.pkg;
      pkg.summary = { ...pkg.summary, fileName: landedAs, path: `${MOCK_MODS_DIR}\\${landedAs}` };
      packages = [...packages, pkg];
      emit(EVENTS.libraryChanged, {});
      const result: BringBackResult = { landedAs };
      return result;
    },
    async openOlderVersionsFolder() {
      await wait(20);
      if (!shelf.exists) {
        throw refusedWith("The stone hasn't put anything away yet, so the older-versions folder is not there to show.");
      }
    },
    // --- the dev hooks: the world outside the window ------------------------
    gameSaved() {
      muster.gameSaved();
    },
    closeGame() {
      setRunning(false);
    },
    deliverNxm(link) {
      receiveNxm(link);
    },
  };
  // `npm run dev` in a browser: the hooks answer from the console (never in a spec)
  if (typeof window !== "undefined" && import.meta.env.MODE !== "test") {
    (window as unknown as { riftMock?: MockHooks }).riftMock = {
      closeGame: () => backend.closeGame(),
      deliverNxm: (link) => backend.deliverNxm(link),
      gameSaved: () => backend.gameSaved(),
      musterMock: backend.musterMock,
    };
  }
  return backend;
}
