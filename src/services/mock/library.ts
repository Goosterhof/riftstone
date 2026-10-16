// The mock library — an invented Dragon's Dogma 2 mod collection shaped like
// the investor's real one (collections behind nested addonfor menus, a
// Nexus-named download with an update waiting, a pak pair that conflicts, a
// pack built for the old engine, an archive that will not open). Every name
// here is invented. No real mod, author or file from anyone's library.

import type {
  ArchiveEntry,
  NexusCandidate,
  NexusLabels,
  LibraryRoot,
  ModInfo,
  ModOption,
  NexusLink,
  PackageFormat,
  PackageSummary,
  PayloadFile,
  PayloadKinds,
  Problem,
  SkippedFile,
} from "@/types/rift";

export interface MockPackage {
  summary: PackageSummary;
  options: ModOption[];
  entries: ArchiveEntry[];
}

export const MOCK_ROOTS: LibraryRoot[] = [
  {
    id: "fluffy",
    path: "C:\\Modding\\Fluffy\\Games\\DragonsDogma2\\Mods",
    label: "Fluffy's Mods folder",
    kind: "fluffy",
    dd2ShapedOnly: false,
  },
  {
    id: "downloads",
    path: "C:\\Users\\arisen\\Downloads",
    label: "Downloads",
    kind: "downloads",
    dd2ShapedOnly: true,
  },
];

function fnv(text: string): string {
  let hash = 0xcbf29ce484222325n;
  for (const ch of new TextEncoder().encode(text)) {
    hash ^= BigInt(ch);
    hash = (hash * 0x100000001b3n) & 0xffffffffffffffffn;
  }
  return hash.toString(16).padStart(16, "0");
}

function kindsOf(payload: PayloadFile[]): PayloadKinds {
  const k: PayloadKinds = { pak: 0, loose: 0, reframework: 0, root: 0 };
  for (const f of payload) k[f.target.kind] += 1;
  return k;
}

function info(partial: Partial<ModInfo>): ModInfo {
  return {
    name: null,
    description: null,
    author: null,
    version: null,
    category: null,
    screenshot: null,
    homepage: null,
    addonFor: null,
    nameAsBundle: null,
    requires: [],
    dummy: false,
    menuPriority: 0,
    extra: {},
    ...partial,
  };
}

interface OptionSeed {
  root: string;
  name: string;
  info?: Partial<ModInfo> | null;
  payload?: { rel: string; size: number }[];
  screenshot?: string | null;
  skipped?: { rel: string; reason: SkippedFile["reason"] }[];
}

function classify(rel: string): PayloadFile["target"] {
  const lower = rel.toLowerCase();
  if (lower.startsWith("natives/")) return { kind: "loose", gamePath: rel };
  if (lower.startsWith("reframework/")) return { kind: "reframework", gamePath: rel };
  if (lower.endsWith(".pak")) return { kind: "pak", fileName: rel.split("/").pop() ?? rel };
  return { kind: "root", gamePath: rel };
}

function join(root: string, rel: string): string {
  return root ? `${root}/${rel}` : rel;
}

function buildPackage(seed: {
  fileName: string;
  displayName: string;
  rootId: string;
  format: PackageFormat;
  size: number;
  modifiedMs: number;
  nexus?: NexusLink | null;
  problems?: Problem[];
  extraEntries?: { path: string; size: number }[];
  options: OptionSeed[];
}): MockPackage {
  const id = fnv(`archive|${seed.fileName.toLowerCase()}|${seed.size}`);
  const root = MOCK_ROOTS.find((r) => r.id === seed.rootId) ?? MOCK_ROOTS[0];
  const entries: ArchiveEntry[] = [...(seed.extraEntries ?? [])];
  const options: ModOption[] = seed.options.map((o) => {
    const payload: PayloadFile[] = (o.payload ?? []).map((p) => ({
      entry: join(o.root, p.rel),
      target: classify(p.rel),
      size: p.size,
    }));
    const skipped: SkippedFile[] = (o.skipped ?? []).map((s) => ({
      entry: join(o.root, s.rel),
      reason: s.reason,
    }));
    for (const p of payload) entries.push({ path: p.entry, size: p.size });
    for (const s of skipped) entries.push({ path: s.entry, size: 2048 });
    const shot = o.screenshot === undefined ? null : o.screenshot;
    if (shot) entries.push({ path: join(o.root, shot), size: 184_320 });
    const parsed = o.info === null ? null : info({ name: o.name, ...o.info });
    return {
      id: `${id}#${o.root}`,
      packageId: id,
      root: o.root,
      name: o.name,
      info: parsed,
      screenshot: shot ? join(o.root, shot) : null,
      payload,
      skipped,
      kinds: kindsOf(payload),
      parent: null,
      addonFor: parsed?.addonFor ?? null,
      bundle: parsed?.nameAsBundle ?? null,
      menuOnly: (parsed?.dummy ?? false) || payload.length === 0,
      trail: [],
    };
  });
  // Resolve addonfor parents inside the package and build trails.
  const byName = new Map(options.map((o) => [o.name.toLowerCase(), o]));
  for (const o of options) {
    if (o.addonFor) o.parent = byName.get(o.addonFor.toLowerCase())?.id ?? null;
  }
  for (const o of options) {
    const chain: string[] = [];
    let cursor = o.parent ? options.find((x) => x.id === o.parent) : undefined;
    let guard = 0;
    while (cursor && guard++ < 16) {
      chain.unshift(cursor.name);
      cursor = cursor.parent ? options.find((x) => x.id === cursor?.parent) : undefined;
    }
    o.trail = [seed.displayName, ...(o.bundle ? [o.bundle] : []), ...chain];
  }
  return {
    summary: {
      id,
      rootId: root.id,
      path: `${root.path}\\${seed.fileName}`,
      fileName: seed.fileName,
      displayName: seed.displayName,
      format: seed.format,
      size: seed.size,
      modifiedMs: seed.modifiedMs,
      entryCount: entries.length,
      optionCount: options.length,
      nexus: seed.nexus ?? null,
      problems: seed.problems ?? [],
    },
    options,
    entries,
  };
}

// --- The wardrobe collection: 3 menus, 18 variants -------------------------

const WARDROBE = "Vernworth Wardrobe Collection";
const wardrobeOptions: OptionSeed[] = [
  {
    root: `${WARDROBE}/CloaksMenu`,
    name: "Cloaks",
    info: {
      dummy: true,
      author: "Hennig of Melve",
      description: "Menu. Opens the cloak variants.",
    },
  },
  {
    root: `${WARDROBE}/HoodsMenu`,
    name: "Hoods",
    info: { dummy: true, author: "Hennig of Melve", description: "Menu. Opens the hood variants." },
  },
  {
    root: `${WARDROBE}/MantlesMenu`,
    name: "Mantles",
    info: {
      dummy: true,
      author: "Hennig of Melve",
      description: "Menu. Opens the mantle variants.",
    },
  },
  {
    root: `${WARDROBE}/ErmineCapeMenuOption`,
    name: "Ermine Cape (Menu Option)",
    info: {
      addonFor: "Cloaks",
      author: "Hennig of Melve",
      description: "Menu Option. Enabling/disabling does nothing.",
    },
  },
  {
    root: `${WARDROBE}/PilgrimHoodMenuOption`,
    name: "Pilgrim Hood (Menu Option)",
    info: {
      addonFor: "Hoods",
      author: "Hennig of Melve",
      description: "Menu Option. Enabling/disabling does nothing.",
    },
  },
  {
    root: `${WARDROBE}/SagesMantleMenuOption`,
    name: "Sage's Mantle (Menu Option)",
    info: {
      addonFor: "Mantles",
      author: "Hennig of Melve",
      description: "Menu Option. Enabling/disabling does nothing.",
    },
  },
];
const CAPE_COLOURS = [
  "Black with silver clasp",
  "Royal blue with fur trim",
  "Moss green, frayed hem",
  "Bone white with a red lining",
  "Ash grey, hood down",
  "Vermund crimson",
];
for (const [i, colour] of CAPE_COLOURS.entries()) {
  wardrobeOptions.push({
    root: `${WARDROBE}/ErmineCape-Variation${i + 1}`,
    name: `Ermine Cape - Variation ${i + 1}`,
    info: {
      addonFor: "Ermine Cape (Menu Option)",
      author: "Hennig of Melve",
      version: "1.2",
      category: "Non-Replacer Armors",
      description: `Purchase at the Vernworth tailor. ${colour}. Can be installed alongside any other Ermine Cape variation.`,
      screenshot: "preview.jpg",
    },
    screenshot: "preview.jpg",
    payload: [
      {
        rel: `natives/stm/appsystem/mantleWb/mesh/mantle_ermine_v${i + 1}_wb_f.pfb.18`,
        size: 4_096,
      },
      {
        rel: `natives/stm/character/_kit/_equipment/mantle/m${310 + i}/mantle_${310 + i}_f.mesh.260421070`,
        size: 1_843_200,
      },
      {
        rel: `natives/stm/character/_kit/_equipment/mantle/m${310 + i}/mantle_${310 + i}_f_alba.tex.251211553`,
        size: 2_796_544,
      },
      {
        rel: `reframework/data/WardrobeEnabler/database/mantle/Female/Mantle_${310 + i}.json`,
        size: 1_024,
      },
    ],
  });
}
const HOOD_LOOKS = ["Undyed wool", "Ink-black with a pilgrim's badge", "Sun-faded ochre", "Bakbattahli silk"];
for (const [i, look] of HOOD_LOOKS.entries()) {
  wardrobeOptions.push({
    root: `${WARDROBE}/PilgrimHood-Variation${i + 1}`,
    name: `Pilgrim Hood - Variation ${i + 1}`,
    info: {
      addonFor: "Pilgrim Hood (Menu Option)",
      author: "Hennig of Melve",
      version: "1.0",
      category: "Non-Replacer Armors",
      description: `Sold at Checkpoint Rest Town. ${look}. Works for both Arisen and pawns.`,
      screenshot: "preview.jpg",
    },
    screenshot: "preview.jpg",
    payload: [
      {
        rel: `natives/stm/character/_kit/_equipment/helm/h${120 + i}/helm_${120 + i}_f.mesh.260421070`,
        size: 921_600,
      },
      {
        rel: `natives/stm/character/_kit/_equipment/helm/h${120 + i}/helm_${120 + i}_f_alba.tex.251211553`,
        size: 1_398_272,
      },
    ],
  });
}
const MANTLE_LOOKS = [
  "Silver thread",
  "Plain linen",
  "Night-sky embroidery",
  "Volcanic island ash",
  "Harvest gold",
  "Twilight purple",
  "Sea-glass teal",
  "Charcoal with a black necklace",
];
for (const [i, look] of MANTLE_LOOKS.entries()) {
  wardrobeOptions.push({
    root: `${WARDROBE}/SagesMantle-Variation${i + 1}`,
    name: `Sage's Mantle - Variation ${i + 1}`,
    info: {
      addonFor: "Sage's Mantle (Menu Option)",
      author: "Hennig of Melve",
      version: "2.0",
      category: "Non-Replacer Armors",
      description: `Purchase at the Sacred Arbor. ${look}. Contains a different look for the male counterpart.`,
      screenshot: "preview.jpg",
    },
    screenshot: "preview.jpg",
    payload: [
      {
        rel: `natives/stm/appsystem/TopsWb/mesh/tops_SagesMantle${i + 1}_wb_f.pfb.18`,
        size: 6_144,
      },
      {
        rel: `natives/stm/character/_kit/_equipment/tops/mesh/top_${400 + i}_wb_f.mesh.260421070`,
        size: 2_457_600,
      },
      {
        rel: `natives/stm/character/_kit/_equipment/tops/mesh/top_${400 + i}_wb_f.mdf2.51`,
        size: 40_960,
      },
    ],
  });
}

// --- The rest of the library ------------------------------------------------

const DAY = 86_400_000;
const NOW = Date.UTC(2026, 9, 16, 9, 0, 0);

export const MOCK_PACKAGES: MockPackage[] = [
  buildPackage({
    fileName: `${WARDROBE}.zip`,
    displayName: WARDROBE,
    rootId: "fluffy",
    format: "zip",
    size: 241_172_480,
    modifiedMs: NOW - 3 * DAY,
    nexus: {
      modId: 2207,
      fileId: 5120,
      groupId: 220700,
      sqid: "Rk4vLmN2Q",
      version: "3",
      uploadedMs: NOW - 30 * DAY,
      source: "homepage",
    },
    extraEntries: [{ path: `${WARDROBE}/README-COLLECTION.txt`, size: 3_072 }],
    options: wardrobeOptions,
  }),
  buildPackage({
    fileName: "Lantern Glow Recolor 412 2.1 2026-09-30T08-12Z aB3dE9fQ1.zip",
    displayName: "Lantern Glow Recolor",
    rootId: "downloads",
    format: "zip",
    size: 12_582_912,
    modifiedMs: NOW - 16 * DAY,
    nexus: {
      modId: 412,
      fileId: 2210,
      groupId: 77100,
      sqid: "aB3dE9fQ1",
      version: "2.1",
      uploadedMs: Date.UTC(2026, 8, 30, 8, 12),
      source: "filename",
    },
    options: [
      {
        root: "Lantern Glow Recolor",
        name: "Lantern Glow Recolor",
        info: {
          author: "Wilhelmina's Lamplighter",
          version: "2.1",
          description:
            "Warmer lantern light — amber instead of white, with a softer falloff so caves stay frightening.",
          screenshot: "glow.png",
        },
        screenshot: "glow.png",
        payload: [
          { rel: "natives/stm/effect/lantern/lantern_light_00.tex.251211553", size: 1_048_576 },
          { rel: "natives/stm/effect/lantern/lantern_light_01.tex.251211553", size: 1_048_576 },
        ],
        skipped: [{ rel: "README.txt", reason: "document" }],
      },
    ],
  }),
  buildPackage({
    fileName: "Battahli Sun Bleach-889-1-0-1709876543.7z",
    displayName: "Battahli Sun Bleach",
    rootId: "fluffy",
    format: "sevenZip",
    size: 318_767_104,
    modifiedMs: NOW - 40 * DAY,
    nexus: {
      modId: 889,
      fileId: 3100,
      groupId: 88001,
      sqid: null,
      version: "1.0",
      uploadedMs: 1_709_876_543_000,
      source: "filename",
    },
    options: [
      {
        root: "BattahliSunBleach",
        name: "Battahli Sun Bleach",
        info: {
          author: "Ulrika the Bleacher",
          version: "1.0",
          description: "Desert light graded towards bleached bone — Bakbattahl at noon, without the orange cast.",
          screenshot: "bleach.jpg",
        },
        screenshot: "bleach.jpg",
        payload: [{ rel: "BattahliSunBleach.pak", size: 318_000_000 }],
      },
    ],
  }),
  buildPackage({
    fileName: "REFramework.zip",
    displayName: "REFramework",
    rootId: "fluffy",
    format: "zip",
    size: 13_272_178,
    modifiedMs: NOW - 5 * DAY,
    options: [
      {
        root: "",
        name: "REFramework",
        info: null,
        payload: [{ rel: "dinput8.dll", size: 23_084_544 }],
        skipped: [{ rel: "reframework_revision.txt", reason: "document" }],
      },
    ],
  }),
  buildPackage({
    fileName: "Pawn Chatter Filter.rar",
    displayName: "Pawn Chatter Filter",
    rootId: "fluffy",
    format: "rar",
    size: 137_284,
    modifiedMs: NOW - 9 * DAY,
    nexus: {
      modId: 3301,
      fileId: 7004,
      groupId: 330100,
      sqid: null,
      version: "1.2",
      uploadedMs: NOW - 60 * DAY,
      source: "manual",
    },
    options: [
      {
        root: "Pawn Chatter Filter",
        name: "Pawn Chatter Filter",
        info: {
          author: "Brant's Quiet Council",
          version: "3.4",
          description:
            "Pawns repeat 'the wolves hunt in packs' once per journey instead of once per wolf. Requires REFramework.",
          requires: ["REFramework"],
        },
        payload: [
          { rel: "reframework/autorun/PawnChatterFilter.lua", size: 21_759 },
          { rel: "reframework/data/PawnChatterFilter/lines.json", size: 45_904 },
        ],
      },
    ],
  }),
  buildPackage({
    fileName: "Ox Cart Express",
    displayName: "Ox Cart Express",
    rootId: "fluffy",
    format: "folder",
    size: 3_343,
    modifiedMs: NOW - 2 * DAY,
    options: [
      {
        root: "Ox Cart Express",
        name: "Ox Cart Express",
        info: {
          author: "Sven of the Checkpoint",
          version: "1.1",
          description: "Oxcarts stop being ambushed by goblins every single trip. Mostly.",
        },
        payload: [{ rel: "reframework/autorun/ox_cart_express.lua", size: 3_343 }],
      },
    ],
  }),
  buildPackage({
    fileName: "Brine Water Clarity.zip",
    displayName: "Brine Water Clarity",
    rootId: "fluffy",
    format: "zip",
    size: 52_428_800,
    modifiedMs: NOW - 12 * DAY,
    options: [
      {
        root: "BrineWaterClarity",
        name: "Brine Water Clarity",
        info: {
          author: "Seneschal's Cartographer",
          version: "1.3",
          description: "Clearer coastal water — you can see what the Brine is about to take from you.",
          screenshot: "water.jpg",
        },
        screenshot: "water.jpg",
        payload: [{ rel: "BrineWaterClarity.pak", size: 52_000_000 }],
      },
    ],
  }),
  buildPackage({
    fileName: "Sacred Arbor Fog Removal.zip",
    displayName: "Sacred Arbor Fog Removal",
    rootId: "fluffy",
    format: "zip",
    size: 31_457_280,
    modifiedMs: NOW - 11 * DAY,
    options: [
      {
        root: "SacredArborFogRemoval",
        name: "Sacred Arbor Fog Removal",
        info: {
          author: "Seneschal's Cartographer",
          version: "0.9",
          description: "Lifts the arbor fog. Also touches coastal water shaders, so it fights with water mods.",
          screenshot: "fog.jpg",
        },
        screenshot: "fog.jpg",
        payload: [{ rel: "SacredArborFogRemoval.pak", size: 31_000_000 }],
      },
    ],
  }),
  buildPackage({
    fileName: "Old Road Armor Pack.zip",
    displayName: "Old Road Armor Pack",
    rootId: "fluffy",
    format: "zip",
    size: 88_080_384,
    modifiedMs: NOW - 200 * DAY,
    problems: [
      {
        kind: "outdatedFormats",
        message:
          "2 loose files (mesh, tex) were made for Dragon's Dogma 2 before the September 2026 engine update (TU 3.2). DD2 Pak Bridge is the likely fix; otherwise look for an updated version of this mod.",
      },
    ],
    options: [
      {
        root: "Old Road Armor Pack",
        name: "Old Road Armor Pack",
        info: {
          author: "A Forgotten Arisen",
          version: "1.0",
          description: "Weathered armour sets for the Old Road. Built for the 2024 engine.",
          screenshot: "armor.jpg",
        },
        screenshot: "armor.jpg",
        payload: [
          {
            rel: "natives/stm/character/_kit/_equipment/body/b210/body_210_f.mesh.240423143",
            size: 3_145_728,
          },
          {
            rel: "natives/stm/character/_kit/_equipment/body/b210/body_210_f_alba.tex.760230703",
            size: 4_194_304,
          },
          {
            rel: "natives/stm/character/_kit/_equipment/body/b210/body_210_f.mdf2.40",
            size: 30_720,
          },
        ],
      },
    ],
  }),
  buildPackage({
    fileName: "Corrupted Download.7z",
    displayName: "Corrupted Download",
    rootId: "downloads",
    format: "sevenZip",
    size: 4_194_304,
    modifiedMs: NOW - 1 * DAY,
    problems: [
      {
        kind: "unreadable",
        message:
          "it is damaged (its table of contents ends early: the download probably stopped). Download it again from its mod page.",
      },
    ],
    options: [],
  }),
  buildPackage({
    fileName: "Readme Only.zip",
    displayName: "Readme Only",
    rootId: "downloads",
    format: "zip",
    size: 8_192,
    modifiedMs: NOW - 6 * DAY,
    extraEntries: [
      { path: "Readme Only/README.txt", size: 4_096 },
      { path: "Readme Only/preview.png", size: 4_096 },
    ],
    problems: [
      {
        kind: "nothingToInstall",
        message: "only a readme and a picture inside.",
      },
    ],
    options: [],
  }),
];

export const MOCK_NOW = NOW;
export const MOCK_DAY = DAY;

// --- v0.2: what Nexus says each linked mod is --------------------------------
// Invented labels in Nexus's own shape: one category per mod, tags often none,
// and one invented mod Nexus flags adult (it carries the "Sexualised" tag, so
// the "adult" veil has something to blur). Keyed by mod id; the mock serves
// them only after an update check has run, as the spine does (nexus.json).

export const MOCK_LABELS: Record<number, NexusLabels> = {
  2207: { category: "Armour", tags: ["Fashion", "Retexture"], adult: false },
  412: { category: "Visuals", tags: ["Lighting", "Retexture"], adult: false },
  889: { category: "Visuals", tags: ["Retexture", "Sexualised"], adult: true },
  3301: { category: "Utilities", tags: ["Utilities for Players", "Audio"], adult: false },
  // what an MD5 identify finds for Brine Water Clarity (the one Nexus knows)
  3302: { category: "Visuals", tags: ["Environment"], adult: false },
  // what Find on Nexus offers (MOCK_FINDS below); labels arrive once linked + checked
  4410: { category: "Armour", tags: ["Armour Sets", "Lore Friendly"], adult: false },
  4416: { category: "Armour", tags: ["Armour Sets", "Sexualised"], adult: true },
  5120: { category: "Visuals", tags: ["Environment", "Weather"], adult: false },
  6031: { category: "Gameplay", tags: ["Travel"], adult: false },
};

/** v0.2 Find on Nexus: what the mock's name search answers, by package display
 *  name. Invented mods only. `linkedPackages` here counts packages OUTSIDE the
 *  mock library's eight (an older download of the same mod); the mock adds the
 *  library's own links at answer time, as the spine counts them. A name absent
 *  here answers no candidates. */
type MockCandidate = Omit<NexusCandidate, "pageUrl">;
const mockCandidate = (c: MockCandidate): MockCandidate => c;
export const MOCK_FINDS: Record<string, MockCandidate[]> = {
  "Old Road Armor Pack": [
    mockCandidate({
      modId: 4410,
      name: "Old Road Armor Pack",
      author: "Hollow Smith",
      version: "1.3",
      category: "Armour",
      adult: false,
      downloads: 12840,
      endorsements: 611,
      summary: "Eleven weathered armour sets for the Old Road, each in three dyes, with matching helms.",
      updatedMs: NOW - 40 * DAY,
      linkedPackages: 1,
    }),
    mockCandidate({
      modId: 4416,
      name: "Old Road Armor Pack - Gilded Edition",
      author: "Hollow Smith",
      version: "0.9",
      category: "Armour",
      adult: true,
      downloads: 3021,
      endorsements: 140,
      summary:
        "The Old Road sets reworked with gilded trims and lighter cuts. Same eleven sets, rebuilt from scratch for the 2026 engine, with a few extras for the pawns.",
      updatedMs: NOW - 12 * DAY,
      linkedPackages: 0,
    }),
  ],
  "Sacred Arbor Fog Removal": [
    mockCandidate({
      modId: 5120,
      name: "Sacred Arbor Fog Removal",
      author: null,
      version: null,
      category: "Visuals",
      adult: false,
      downloads: 980,
      endorsements: null,
      summary: null,
      updatedMs: null,
      linkedPackages: 0,
    }),
  ],
  "Ox Cart Express": [
    mockCandidate({
      modId: 6031,
      name: "Ox Cart Express",
      author: "Wagonwright",
      version: "2.0",
      category: "Gameplay",
      adult: false,
      downloads: 45210,
      endorsements: 2213,
      summary: "Oxcarts leave on time, wait for you at the gate, and skip the goblin ambush once you have seen it.",
      updatedMs: NOW - 90 * DAY,
      linkedPackages: 0,
    }),
  ],
  "Brine Water Clarity": [
    mockCandidate({
      modId: 3302,
      name: "Brine Water Clarity",
      author: "Tidecaller",
      version: "1.1",
      category: "Visuals",
      adult: false,
      downloads: 7702,
      endorsements: 388,
      summary: "Clearer water along the coast and in the Brine, without touching the lighting.",
      updatedMs: NOW - 200 * DAY,
      linkedPackages: 0,
    }),
  ],
};

/** The package the mock's MD5 identify recognises, and the link it answers. */
export const MOCK_MD5_MATCH = { displayName: "Brine Water Clarity", modId: 3302, fileId: 7140 };
