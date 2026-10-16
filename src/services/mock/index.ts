// The mock backend: the whole window contract over the invented library in
// ./library.ts, in memory. Scenarios for rendering every state come from the
// page's query string: ?mock=empty | nogame | scanning | refoff | patched |
// offline | fresh | key | quotalow (comma-separated to combine). `quotalow`
// implies `key`: the quota is the v1 budget, and only a keyed window has one.
//
// v0.2: the Nexus labels (./library.ts MOCK_LABELS) are served only once an
// update check has run, so `?mock=fresh` is the first launch before any check:
// no labels, the NEXUS facet group collapsed to its one quiet line. The MD5
// identify recognises one package (MOCK_MD5_MATCH), and fails under `offline`
// like the spine's lookup. Five packages stay unlinked, three of them with a
// screenshot (Brine Water Clarity, Sacred Arbor Fog Removal, Old Road Armor
// Pack): the "adult" veil blurs them as unknown until they are linked and
// checked, beside Battahli Sun Bleach, the one mod Nexus flags adult. Every
// backend reads its own copy of the library. Two parked files wait in
// the backup store under the spine's own handle shape, `backups/<seq>/<gamePath>`:
// over the first an unowned file sits now (Put back swaps it out), over the
// second Pawn Chatter Filter's install sits (Put back is refused in the
// engine's words until it is uninstalled). An unknown handle is refused the
// way the spine refuses it. Uninstalling Sacred Arbor Fog Removal parks its
// changed pak and Ermine Cape - Variation 2 leaves one changed file in place,
// so THE RETURN's "kept aside" and "left in place" sentences render. One
// installed file is already gone from the game (the "already gone" plan row),
// and one hash conflict is unconfirmed. No new scenario was needed.

import type {
  AdoptResultView,
  LabelCount,
  LabelFacets,
  NexusLabels,
  ParkedFileView,
  ParkedRestoreReport,
  Check,
  ConflictView,
  GameSummary,
  InstallPlanView,
  InstallResultView,
  InstalledRow,
  InstalledView,
  LibraryRoot,
  MatchedField,
  Md5Result,
  NexusSummary,
  OptionDetail,
  OptionRef,
  Overview,
  PackageDetail,
  PakView,
  PlannedRemoval,
  RemovalAction,
  PlannedWrite,
  PlanWarning,
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
import {
  MOCK_DAY,
  MOCK_LABELS,
  MOCK_MD5_MATCH,
  MOCK_NOW,
  MOCK_PACKAGES,
  MOCK_ROOTS,
  type MockPackage,
} from "./library";

type Listener = (payload: unknown) => void;

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
const nexusFailure = (detail: string) => new RiftFailure({ kind: "nexus", detail: { detail } });

interface InstallRecord {
  optionId: string;
  seq: number;
  installedAt: string;
  origin: "riftstone" | "fluffy";
}

export function createMockBackend(): RiftBackend {
  const scene = scenarios();
  const listeners = new Map<string, Set<Listener>>();
  const emit = (event: RiftEvent, payload: unknown) => {
    for (const l of listeners.get(event) ?? []) l(payload);
  };

  // Each backend reads its own copy: a link, an unlink or an MD5 identify
  // writes into package summaries, and must never leak into the next backend
  // (the next spec, the next scenario).
  let packages: MockPackage[] = scene.has("empty") ? [] : structuredClone(MOCK_PACKAGES);
  let roots: LibraryRoot[] = [...MOCK_ROOTS];
  let settings: Settings = { veil: "off", autoCheckUpdates: true };
  let gameRoot: string | null = scene.has("nogame")
    ? null
    : "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Dragons Dogma 2";
  let scanning = scene.has("scanning");
  let scannedOnce = !scene.has("firstrun");
  let seq = 3;
  let lastCheckMs: number | null = scene.has("fresh") ? null : MOCK_NOW - 2 * 3_600_000;
  let hasApiKey = scene.has("key") || scene.has("quotalow");

  const optionByName = (name: string) => packages.flatMap((p) => p.options).find((o) => o.name === name);

  const installs: InstallRecord[] = [];
  const preinstalled: [string, "riftstone" | "fluffy"][] = [
    ["REFramework", "riftstone"],
    ["Pawn Chatter Filter", "riftstone"],
    ["Brine Water Clarity", "riftstone"],
    ["Sacred Arbor Fog Removal", "riftstone"],
    ["Ermine Cape - Variation 2", "riftstone"],
  ];
  if (!scene.has("empty")) {
    for (const [i, [name, origin]] of preinstalled.entries()) {
      const o = optionByName(name);
      if (o)
        installs.push({
          optionId: o.id,
          seq: i + 1,
          installedAt: new Date(MOCK_NOW - (10 - i) * MOCK_DAY).toISOString(),
          origin,
        });
    }
  }
  // Fluffy installed "Ox Cart Express" and nobody adopted it yet.
  let fluffyUnadopted = scene.has("empty") ? [] : ["Ox Cart Express"];
  // An installed file someone deleted by hand: the plan says "already gone".
  const gonePaths = new Set<string>();
  const ermine = optionByName("Ermine Cape - Variation 2");
  const ermineFirst = ermine?.payload.find((p) => p.target.kind !== "pak");
  if (!scene.has("empty") && ermineFirst && ermineFirst.target.kind !== "pak")
    gonePaths.add(ermineFirst.target.gamePath);

  // Ermine's second loose file was edited by hand since install: kept on uninstall.
  const changedPaths = new Set<string>();
  const ermineSecond = ermine?.payload.filter((p) => p.target.kind !== "pak").at(1);
  if (!scene.has("empty") && ermineSecond && ermineSecond.target.kind !== "pak")
    changedPaths.add(ermineSecond.target.gamePath);

  type PayloadTarget = NonNullable<typeof ermineFirst>["target"];
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
          owner: "Pawn Banter Quieter (Fluffy)",
          ownerKind: "fluffy",
          size: 4_096,
          parkedAt: new Date(MOCK_NOW - 2 * MOCK_DAY).toISOString(),
          reason: "Pawn Chatter Filter's install would have overwritten it, so it was kept aside first.",
          occupied: true,
        },
      ];

  let pakOrder = installs.filter((r) => findOption(r.optionId)?.option.kinds.pak).map((r) => r.optionId);

  function findOption(optionId: string) {
    for (const pkg of packages) {
      const option = pkg.options.find((o) => o.id === optionId);
      if (option) return { pkg, option };
    }
    return null;
  }
  const isInstalled = (optionId: string) => installs.some((r) => r.optionId === optionId);

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
    };
    // `offline` changes nothing here: a failed check leaves the cache alone
    // (get_updates is cached state), so the rows keep the last good verdict
    // the page's offline notice dates.
    if (lastCheckMs === null) return { ...base, state: { kind: "notChecked" } };
    if (link.modId === 412) {
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
      hasApiKey,
    };
  }

  function game(): GameSummary | null {
    if (!gameRoot) return null;
    return {
      root: gameRoot,
      buildId: scene.has("patched") ? "25391144" : "25290080",
      source: "steam",
      running: false,
      buildChanged: scene.has("patched") ? { recorded: "25290080", now: "25391144" } : null,
    };
  }

  function checks(): Check[] {
    const list: Check[] = [];
    if (!gameRoot) {
      list.push({
        id: "game",
        level: "error",
        title: "Dragon's Dogma 2 was not found",
        detail: "Steam's library folders do not list app 2054970. Point the Riftstone at the game folder.",
        action: { kind: "pickGameFolder" },
      });
      return list;
    }
    list.push({
      id: "game",
      level: "ok",
      title: "Dragon's Dogma 2 found",
      detail: `Steam build ${game()?.buildId}.`,
      action: null,
    });
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
    if (scene.has("refoff")) {
      list.push({
        id: "pakdir",
        level: "warn",
        title: "REFramework's pak folder is switched off",
        detail: "Your pak mods will not load until 'PAK Directory Loading' is on.",
        action: { kind: "enableSwitch", switch: "pakDirectory" },
      });
    } else {
      list.push({
        id: "pakdir",
        level: "ok",
        title: "pak_mods loading is on",
        detail: "REFramework injects every pak in pak_mods/ at launch.",
        action: null,
      });
    }
    list.push({
      id: "loose",
      level: "ok",
      title: "Loose files load",
      detail: "LooseFileLoader is enabled.",
      action: null,
    });
    if (fluffyUnadopted.length) {
      list.push({
        id: "fluffy",
        level: "info",
        title: `Fluffy installed ${fluffyUnadopted.length} mod the Riftstone does not manage yet`,
        detail: "Adopt it to uninstall or reorder it from here. Fluffy's own files are not changed.",
        action: { kind: "adoptFluffy" },
      });
    }
    if (parked.length) {
      list.push({
        id: "parkedBackups",
        level: "info",
        title: `${parked.length} ${parked.length === 1 ? "file is" : "files are"} kept aside in the backup store`,
        detail: "The stone parked them rather than delete them or force them back. Put each back or discard it.",
        action: { kind: "reviewParked" },
      });
    }
    list.push({
      id: "crashreporter",
      level: "info",
      title: "Fluffy renamed the crash reporter",
      detail: "CrashReport.exe sits as CrashReport_FMMBACKUP.exe. Steam's 'Verify integrity' restores it.",
      action: null,
    });
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
      fluffy: {
        root: "C:\\Modding\\Fluffy",
        modsDir: MOCK_ROOTS[0].path,
        installedCount: 6,
        unadoptedCount: fluffyUnadopted.length,
      },
      roots,
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

  function installBadge(optionId: string, name: string): SearchResult["installed"] {
    const record = installs.find((r) => r.optionId === optionId);
    if (record) return { origin: record.origin, installedAt: record.installedAt };
    if (fluffyUnadopted.includes(name)) return { origin: "fluffyUnadopted", installedAt: null };
    return null;
  }

  function updateBadge(pkg: MockPackage): SearchResult["update"] {
    const row = updateRow(pkg);
    if (!row) return null;
    if (row.state.kind === "updateAvailable") return "available";
    if (row.state.kind === "current") return "current";
    return "unknown";
  }

  function passesFilters(query: SearchQuery, pkg: MockPackage, o: MockPackage["options"][number]): boolean {
    if (query.filters.rootIds.length && !query.filters.rootIds.includes(pkg.summary.rootId)) return false;
    if (o.menuOnly && !query.filters.includeMenuOnly) return false;
    const installedAnywhere = isInstalled(o.id) || fluffyUnadopted.includes(o.name);
    const wanted = (list: string[]) => list.map((x) => x.toLowerCase());
    const labels = labelsOf(pkg);
    const categories = wanted(query.filters.categories);
    if (categories.length && !categories.includes(labels?.category?.toLowerCase() ?? "")) return false;
    const tags = wanted(query.filters.tags);
    const carried = wanted(labels?.tags ?? []);
    if (tags.length && !tags.every((t) => carried.includes(t))) return false;
    if (query.filters.installed === true) return installedAnywhere;
    if (query.filters.installed === false) return !installedAnywhere;
    return true;
  }

  function matchOption(toks: string[], pkg: MockPackage, o: MockPackage["options"][number]): SearchResult | null {
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
      rootId: pkg.summary.rootId,
      author: o.info?.author ?? null,
      version: o.info?.version ?? null,
      category: o.info?.category ?? null,
      descriptionSnippet: snippet(o.info?.description),
      hasScreenshot: Boolean(o.screenshot),
      kinds: o.kinds,
      menuOnly: o.menuOnly,
      matched: [...matched].sort((a, b) => WEIGHTS[b] - WEIGHTS[a]),
      fileMatches,
      installed: installBadge(o.id, o.name),
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
    if (toks.length) hits.sort((a, b) => b.score - a.score || a.name.localeCompare(b.name));
    return hits.slice(0, query.limit || 200);
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

  function planWarnings(option: MockPackage["options"][number]): PlanWarning[] {
    const warnings: PlanWarning[] = [];
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

  function planRefusal(option: MockPackage["options"][number]): string | null {
    if (!gameRoot) return "Dragon's Dogma 2 has not been found yet — point the Riftstone at the game folder first.";
    if (option.menuOnly) return "This is a menu entry — it installs nothing. Pick one of its variants.";
    return null;
  }

  function tree(pkg: MockPackage): TreeNode[] {
    const node = (o: (typeof pkg.options)[number]): TreeNode => ({
      optionId: o.id,
      label: o.name,
      kind: "option",
      menuOnly: o.menuOnly,
      hasScreenshot: Boolean(o.screenshot),
      installed: isInstalled(o.id),
      kinds: o.kinds,
      children: pkg.options.filter((c) => c.parent === o.id).map(node),
    });
    return pkg.options.filter((o) => !o.parent).map(node);
  }

  function pakViews(record: InstallRecord, payload: MockPackage["options"][number]["payload"]): PakView[] {
    const position = Math.max(0, pakOrder.indexOf(record.optionId));
    const views: PakView[] = [];
    for (const file of payload) {
      if (file.target.kind !== "pak") continue;
      views.push({
        fileName: `zrift-${String(position).padStart(4, "0")}-${file.target.fileName}`,
        size: file.size,
        load:
          record.seq <= 4
            ? {
                kind: "loaded",
                slot: 103 + position,
                nativeName: `re_chunk_000.pak.sub_000.pak.patch_${String(position + 1).padStart(3, "0")}.pak`,
              }
            : { kind: "pending" },
      });
    }
    return views;
  }

  function installedRows(): InstalledRow[] {
    return installs.map((record) => {
      const found = findOption(record.optionId);
      const option = found?.option;
      return {
        optionId: record.optionId,
        name: option?.name ?? "(missing)",
        packageName: found?.pkg.summary.displayName ?? "(gone from the library)",
        packageMissing: !found,
        version: option?.info?.version ?? null,
        installedAt: record.installedAt,
        gameBuild: "25290080",
        origin: record.origin,
        kinds: option?.kinds ?? { pak: 0, loose: 0, reframework: 0, root: 0 },
        fileCount: option?.payload.length ?? 0,
        paks: pakViews(record, option?.payload ?? []),
      };
    });
  }

  const backend: RiftBackend & PendingBackend = {
    // Pending contract (services/pending.ts): rendered here so every §9 state
    // has a frame; the spine grows into the same duck-typed shapes.
    async troubledPackages() {
      await wait(30);
      // The spine's rule (WINDOW-CONTRACT get_troubled_packages): every
      // package carrying one of the four trouble kinds, ordered by the first
      // it carries, then library order.
      const rank = (p: (typeof packages)[number]) => {
        const at = TROUBLE_KINDS.findIndex((kind) => p.summary.problems.some((x) => x.kind === kind));
        return at === -1 ? TROUBLE_KINDS.length : at;
      };
      const troubled = packages.filter((p) => rank(p) < TROUBLE_KINDS.length);
      return [...troubled].sort((a, b) => rank(a) - rank(b)).map((p) => p.summary);
    },
    async nexusQuota(): Promise<NexusQuota | null> {
      await wait(20);
      // WINDOW-CONTRACT get_nexus_quota: null without a key — anonymous v2
      // checks carry no quota headers (RS-6).
      if (!hasApiKey) return null;
      const low = scene.has("quotalow");
      return {
        hourlyRemaining: low ? 12 : 486,
        hourlyLimit: 500,
        dailyRemaining: low ? 1_204 : 19_840,
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
      const record = installs.find((r) => r.optionId === optionId);
      const detail: OptionDetail = {
        option,
        package: pkg.summary,
        parentName: option.parent ? (pkg.options.find((p) => p.id === option.parent)?.name ?? null) : null,
        children,
        installed: record
          ? {
              seq: record.seq,
              installedAt: record.installedAt,
              version: option.info?.version ?? null,
              gameBuild: "25290080",
              origin: record.origin,
              files: option.payload.map((p) => ({
                gamePath:
                  p.target.kind === "pak"
                    ? `pak_mods/zrift-${String(pakOrder.indexOf(optionId)).padStart(4, "0")}-${p.target.fileName}`
                    : p.target.gamePath,
                kind: p.target.kind,
                size: p.size,
              })),
            }
          : null,
        fluffyInstalled: fluffyUnadopted.includes(option.name),
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
      const detail: PackageDetail = { package: pkg.summary, tree: tree(pkg), entries: pkg.entries };
      return detail;
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
      const writes: PlannedWrite[] = option.payload.map((p) => ({
        entry: p.entry,
        gamePath:
          p.target.kind === "pak"
            ? `pak_mods/zrift-${String(pakOrder.length).padStart(4, "0")}-${p.target.fileName}`
            : p.target.gamePath,
        kind: p.target.kind,
        size: p.size,
        action: reinstall ? { kind: "reinstall" } : { kind: "create" },
      }));
      const plan: InstallPlanView = {
        optionId,
        name: option.name,
        writes,
        totalBytes: writes.reduce((n, w) => n + w.size, 0),
        warnings: planWarnings(option),
        refused: planRefusal(option),
      };
      return plan;
    },
    async install(optionId) {
      const found = findOption(optionId);
      if (!found) throw notFound(optionId);
      const total = found.option.payload.length;
      const bytesTotal = found.option.payload.reduce((n, p) => n + p.size, 0);
      let bytesDone = 0;
      for (let i = 0; i < total; i++) {
        await wait(220);
        bytesDone += found.option.payload[i].size;
        emit(EVENTS.installProgress, {
          optionId,
          filesDone: i + 1,
          filesTotal: total,
          bytesDone,
          bytesTotal,
        });
      }
      if (!isInstalled(optionId)) {
        installs.push({
          optionId,
          seq: ++seq,
          installedAt: new Date().toISOString(),
          origin: "riftstone",
        });
        if (found.option.kinds.pak) pakOrder.push(optionId);
      }
      emit(EVENTS.gameChanged, {});
      const result: InstallResultView = {
        optionId,
        filesWritten: total,
        bytesWritten: bytesTotal,
        backups: 0,
        durationMs: 220 * total,
        restartNeeded: false,
      };
      return result;
    },
    async planUninstall(optionId) {
      await wait(30);
      const found = findOption(optionId);
      if (!found) throw notFound(optionId);
      const removals: PlannedRemoval[] = found.option.payload.map((p) => ({
        gamePath:
          p.target.kind === "pak"
            ? `pak_mods/zrift-${String(pakOrder.indexOf(optionId)).padStart(4, "0")}-${p.target.fileName}`
            : p.target.gamePath,
        kind: p.target.kind,
        action: removalAction(p.target),
      }));
      const plan: UninstallPlanView = {
        optionId,
        name: found.option.name,
        removals,
        refused: isInstalled(optionId) ? null : "This option is not installed.",
      };
      return plan;
    },
    async uninstall(optionId) {
      await wait(300);
      const found = findOption(optionId);
      const name = found?.option.name ?? "";
      const pakSlot = String(pakOrder.indexOf(optionId)).padStart(4, "0");
      const at = installs.findIndex((r) => r.optionId === optionId);
      if (at >= 0) installs.splice(at, 1);
      pakOrder = pakOrder.filter((id) => id !== optionId);
      const payload = found?.option.payload ?? [];
      const kinds = payload.map((p) => removalAction(p.target).kind);
      for (const p of payload) {
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
      const parks = name === "Sacred Arbor Fog Removal" ? payload.filter((p) => p.target.kind === "pak") : [];
      for (const p of parks) {
        const gamePath = `pak_mods/zrift-${pakSlot}-${p.target.kind === "pak" ? p.target.fileName : ""}`;
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
        restartNeeded: false,
      };
      return result;
    },
    async getInstalled() {
      await wait(40);
      const view: InstalledView = {
        installs: installedRows(),
        pakOrder,
        conflicts: conflicts(),
        strays: scene.has("empty") ? [] : [{ gamePath: "pak_mods/old_test_mod.pak", size: 1_048_576 }],
        missing: installs.flatMap((record) => {
          const option = findOption(record.optionId)?.option;
          return (option?.payload ?? []).flatMap((p) =>
            p.target.kind !== "pak" && gonePaths.has(p.target.gamePath)
              ? [{ optionId: record.optionId, optionName: option?.name ?? "", gamePath: p.target.gamePath }]
              : [],
          );
        }),
        fluffyUnadopted: fluffyUnadopted.map((name) => ({
          section: name.replace(/\s+/g, ""),
          name,
          fileCount: 1,
          matchedOptionId: optionByName(name)?.id ?? null,
        })),
        pakLoad: {
          logStarted: new Date(MOCK_NOW - 6 * 3_600_000).toISOString(),
          // Agrees with the rows' "loaded · slot N": the same paks, the same slots.
          injected: installs.flatMap((record) =>
            pakViews(record, findOption(record.optionId)?.option.payload ?? []).flatMap((v) =>
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
      await wait(80);
      pakOrder = [...order];
      emit(EVENTS.gameChanged, {});
    },
    async adoptFluffy() {
      await wait(200);
      const adopted = fluffyUnadopted.length;
      for (const name of fluffyUnadopted) {
        const o = optionByName(name);
        if (o)
          installs.push({
            optionId: o.id,
            seq: ++seq,
            installedAt: new Date().toISOString(),
            origin: "fluffy",
          });
      }
      fluffyUnadopted = [];
      emit(EVENTS.gameChanged, {});
      const result: AdoptResultView = {
        adopted,
        alreadyAdopted: 5,
        missingFiles: 0,
        refusedFiles: adopted ? 2 : 0,
        sharedFiles: adopted ? 1 : 0,
        unmatched: [],
      };
      return result;
    },
    async setReframeworkSwitch() {
      await wait(60);
      scene.delete("refoff");
      emit(EVENTS.gameChanged, {});
    },
    async addRoot(path) {
      await wait(40);
      const root: LibraryRoot = {
        id: `custom-${roots.length}`,
        path,
        label: path.split(/[\\/]/).pop() ?? path,
        kind: "custom",
        dd2ShapedOnly: false,
      };
      roots = [...roots, root];
      return root;
    },
    async removeRoot(rootId) {
      await wait(40);
      roots = roots.filter((r) => r.id !== rootId);
      packages = packages.filter((p) => p.summary.rootId !== rootId);
      emit(EVENTS.libraryChanged, {});
    },
    async setGameRoot(path) {
      await wait(60);
      gameRoot = path;
      emit(EVENTS.gameChanged, {});
      return game() as GameSummary;
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
      if (scene.has("offline")) throw nexusFailure("Nexus Mods could not be reached (connection refused).");
      lastCheckMs = Date.now();
      emit(EVENTS.updatesChanged, {});
      return updatesView();
    },
    async identifyPackages(packageIds) {
      await wait(400);
      // the hashes are local, the MD5 lookup is a Nexus call: offline it fails
      if (scene.has("offline")) throw nexusFailure("Nexus Mods could not be reached (connection refused).");
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
    async linkPackage(packageId, urlOrId) {
      await wait(120);
      const pkg = packages.find((p) => p.summary.id === packageId);
      if (!pkg) throw notFound(packageId);
      // the spine's parse_mod_url: ".../mods/<id>" first, else a bare id
      const text = urlOrId.trim();
      const id = Number(/\/mods\/(\d+)/.exec(text)?.[1] ?? /^\d+$/.exec(text)?.[0] ?? Number.NaN);
      if (!Number.isFinite(id)) throw nexusFailure("That is not a Dragon's Dogma 2 mod link or id.");
      pkg.summary.nexus = {
        modId: id,
        fileId: null,
        groupId: null,
        sqid: null,
        version: null,
        uploadedMs: null,
        source: "manual",
      };
      emit(EVENTS.updatesChanged, {});
      return updateRow(pkg) as UpdateRow;
    },
    async unlinkPackage(packageId) {
      await wait(60);
      const pkg = packages.find((p) => p.summary.id === packageId);
      if (pkg) pkg.summary.nexus = null;
      emit(EVENTS.updatesChanged, {});
    },
    async setNexusKey(key) {
      await wait(200);
      hasApiKey = Boolean(key);
      return nexusSummary();
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
      const report: ParkedRestoreReport = { gamePath: file.gamePath, restored: true, swappedOut, restartNeeded: false };
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
      return Promise.resolve("C:\\Users\\arisen\\Documents\\DD2 Mods");
    },
  };
  return backend;
}
