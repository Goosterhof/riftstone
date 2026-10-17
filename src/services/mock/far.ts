// The far world in the mock (v0.3): invented Nexus data for browsing, one
// mod's files and patch notes, the account and its standings. Every name,
// author, mod id and file id here is invented, and nothing is ever fetched:
// the mock answers from these tables on real timers.

import type { EndorseStatus, NexusCandidate, NexusFileEntry } from "@/types/rift";
import { MOCK_DAY, MOCK_NOW } from "./library";

/** The account a mock key validates as (never a real account). */
export const MOCK_ACCOUNT_NAME = "Arisen of Vernworth";
/** Its user id, as validate.json would give it; an nxm link carrying another is refused. */
export const MOCK_USER_ID = 7_700_123;
/** Another program's exe, for the handler scene `nxmother`. */
export const MOCK_OTHER_HANDLER = "OtherManager.exe";
/** A mod the mock account authored: endorsing it is refused (endorseOwnMod). */
export const MOCK_OWN_MOD = 7310;

/** The mods the mock account tracks on Nexus, and what it decided about others. */
export const MOCK_TRACKED: readonly number[] = [2207, 6031, 4410];
export const MOCK_ENDORSEMENTS: readonly { modId: number; status: EndorseStatus }[] = [
  { modId: 2207, status: "endorsed" },
  { modId: 3301, status: "abstained" },
];

export type BrowseSeed = Omit<NexusCandidate, "pageUrl" | "linkedPackages">;

const PLACES = [
  "Vernworth",
  "Bakbattahl",
  "Melve",
  "Harve",
  "Checkpoint",
  "Agamen",
  "Battahl",
  "Volcanic Island",
  "Seafloor Shrine",
  "Misty Marsh",
  "Sacred Arbor",
  "Pilgrim Road",
  "Moonlit Cove",
];
const THINGS = [
  "Tailor Dyes",
  "Market Rugs",
  "Camp Fires",
  "Harbour Ropes",
  "Ox Harness",
  "Bell Towers",
  "Road Signs",
  "Fishing Nets",
  "Night Sky",
  "Banners",
];
const KINDS = ["Recolor", "Overhaul", "Retexture", "Tweaks", "Pack"];
const AUTHORS = ["Marsh Lantern Guild", "Ferrywoman of Harve", "Brine Cartographer", "Tidecaller", "Hollow Smith"];
const CATEGORIES = ["Visuals", "Armour", "Gameplay", "Utilities", "Audio"];

function browseSeed(i: number): BrowseSeed {
  const place = PLACES[i % PLACES.length];
  const thing = THINGS[(i * 3) % THINGS.length];
  const kind = KINDS[(i * 7) % KINDS.length];
  const modId = 7300 + i;
  const downloads = ((i * 7919) % 48_000) + 120;
  return {
    modId,
    name: `${place} ${thing} ${kind}`,
    author: modId === MOCK_OWN_MOD ? MOCK_ACCOUNT_NAME : AUTHORS[i % AUTHORS.length],
    version: `1.${i % 7}`,
    category: CATEGORIES[(i * 2) % CATEGORIES.length],
    adult: i % 17 === 5,
    downloads,
    endorsements: Math.floor(downloads / 23),
    summary: `A ${kind.toLowerCase()} of ${place}'s ${thing.toLowerCase()}, invented for the stone's bench.`,
    updatedMs: MOCK_NOW - ((i * 37) % 400) * MOCK_DAY,
  };
}

/** Three pages and a bit of invented Dragon's Dogma 2 mods (20 a page). */
export const MOCK_BROWSE: readonly BrowseSeed[] = Array.from({ length: 52 }, (_, i) => browseSeed(i));

const CHANGELOGS: readonly string[][] = [
  ["Colours tuned for the September 2026 engine update.", "Fixed a seam on the left sleeve."],
  ["Softer edges at dusk.", "Smaller archive: the unused textures are gone."],
  ["First release."],
];

type FileSeed = Omit<NexusFileEntry, "inLibrary">;

/** Lantern Glow Recolor (412): the update the library is waiting for, the
 *  version the library holds (now an old version), and an optional file. */
const LANTERN_FILES: readonly FileSeed[] = [
  {
    fileId: 2391,
    name: "Lantern Glow Recolor",
    version: "2.2",
    category: "main",
    size: 12_845_056,
    uploadedMs: MOCK_NOW - 3 * MOCK_DAY,
    description: "Warmer amber for every hand lantern, with a softer falloff in caves.",
    changelog: [
      "Amber for every hand lantern, not only the Arisen's.",
      "Caves keep a softer falloff.",
      "Fixed a flicker at dusk in Vernworth.",
    ],
    newest: true,
  },
  {
    fileId: 2215,
    name: "Lantern Glow Recolor - Candles Only",
    version: "1.0",
    category: "optional",
    size: 2_097_152,
    uploadedMs: MOCK_NOW - 50 * MOCK_DAY,
    description: "Only the candles and the inn lamps; hand lanterns stay as the game made them.",
    changelog: ["First release."],
    newest: true,
  },
  {
    fileId: 2210,
    name: "Lantern Glow Recolor",
    version: "2.1",
    category: "oldVersion",
    size: 12_582_912,
    uploadedMs: Date.UTC(2026, 8, 30, 8, 12),
    description: "Warmer lantern light, amber instead of white.",
    changelog: ["Amber instead of white.", "Caves stay frightening."],
    newest: false,
  },
];

/** One mod's files on Nexus, invented. `held` is the file a library package
 *  is pinned to (its id and version), when one is. */
export function mockFiles(
  modId: number,
  modName: string,
  held: { fileId: number | null; version: string | null } | null,
): FileSeed[] {
  if (modId === 412) return LANTERN_FILES.map((f) => ({ ...f, changelog: [...f.changelog] }));
  const mainId = held?.fileId ?? modId * 10 + 1;
  const version = held?.version ?? "1.0";
  const size = ((modId % 40) + 4) * 1_048_576;
  return [
    {
      fileId: mainId,
      name: modName,
      version,
      category: "main",
      size,
      uploadedMs: MOCK_NOW - ((modId % 90) + 5) * MOCK_DAY,
      description: `${modName}, the whole mod in one archive.`,
      changelog: [...CHANGELOGS[modId % CHANGELOGS.length]],
      newest: true,
    },
    {
      fileId: mainId + 1,
      name: `${modName} - Lite`,
      version: "1.0",
      category: "optional",
      size: Math.floor(size / 3),
      uploadedMs: MOCK_NOW - ((modId % 60) + 20) * MOCK_DAY,
      description: "The same look with fewer files, for a lighter game folder.",
      changelog: ["First release."],
      newest: true,
    },
    {
      fileId: mainId + 2,
      name: modName,
      version: "0.9",
      category: "archived",
      size: Math.floor(size * 0.9),
      uploadedMs: MOCK_NOW - ((modId % 60) + 200) * MOCK_DAY,
      description: "Built for the engine before the September 2026 update.",
      changelog: ["Early access build."],
      newest: false,
    },
  ];
}
