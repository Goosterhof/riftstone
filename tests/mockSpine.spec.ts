// The bench the window specs stand on: every createMockBackend() is its own
// library, so a link, an unlink or an MD5 identify in one spec can never leak
// into the next; and the mock refuses what the real spine cannot do offline
// (an MD5 identify is a Nexus lookup), so an offline spec reads the truth.

import { afterEach, describe, expect, it } from "vitest";
import {
  EVENTS,
  type DownloadProgressEvent,
  type DownloadView,
  type NxmReceivedEvent,
  type TreeNode,
} from "@/types/rift";
import { MOCK_MD5_MATCH, MOCK_MODS_DIR, MOCK_PACKAGES } from "@/services/mock/library";
import { createMockBackend, type MockBackend } from "@/services/mock/index";
import { RiftFailure } from "@/services/rift";

afterEach(() => window.history.replaceState(null, "", "/"));

const idOf = (name: string): string => {
  const pkg = MOCK_PACKAGES.find((p) => p.summary.displayName === name);
  if (!pkg) throw new Error(`The mock library has no ${name}`);
  return pkg.summary.id;
};

describe("the mock spine", () => {
  it("should give every backend its own library: a link, an unlink or an identify on one leaks into no other", async () => {
    const brine = idOf(MOCK_MD5_MATCH.displayName);
    const lantern = idOf("Lantern Glow Recolor");
    const first = createMockBackend();
    await first.identifyPackages([brine]);
    await first.unlinkPackage(lantern);
    const firstRows = (await first.getUpdates()).rows;
    expect(firstRows.find((r) => r.packageId === brine)?.link?.modId).toBe(MOCK_MD5_MATCH.modId);
    expect(firstRows.find((r) => r.packageId === lantern)?.link).toBeNull();

    const second = createMockBackend();
    const rows = (await second.getUpdates()).rows;
    expect(rows.find((r) => r.packageId === brine)?.link).toBeNull();
    expect(rows.find((r) => r.packageId === lantern)?.link?.modId).toBe(412);
    expect(MOCK_PACKAGES.find((p) => p.summary.id === brine)?.summary.nexus).toBeNull();
  });

  it("should refuse an MD5 identify while Nexus is out of reach, as the spine's lookup would", async () => {
    window.history.replaceState(null, "", "/?mock=offline");
    const backend = createMockBackend();
    const brine = idOf(MOCK_MD5_MATCH.displayName);
    const refused = await backend.identifyPackages([brine]).then(
      () => null,
      (raw: unknown) => raw,
    );
    expect(refused).toBeInstanceOf(RiftFailure);
    expect((refused as RiftFailure).kind).toBe("nexus");
    expect((await backend.getUpdates()).rows.find((r) => r.packageId === brine)?.link).toBeNull();
  });
});

// ---------------------------------------------------------------------------
// v0.3 — the bench the tree, Play and the far world will stand on
// ---------------------------------------------------------------------------

const onScene = (scene: string): MockBackend => {
  window.history.replaceState(null, "", `/?mock=${scene}`);
  return createMockBackend();
};

async function settled<T>(read: () => Promise<T>, done: (value: T) => boolean, what: string, ms = 5000): Promise<T> {
  const start = Date.now();
  for (;;) {
    const value = await read();
    if (done(value)) return value;
    if (Date.now() - start > ms) throw new Error(`The mock never showed: ${what}`);
    await new Promise((resolve) => setTimeout(resolve, 40));
  }
}

const labels = (nodes: TreeNode[]): string[] => nodes.map((n) => `${n.kind}:${n.label}`);

describe("the v0.3 mock spine", () => {
  it("should shelve one mods folder by display name, and gather NameAsBundle siblings under one bundle node", async () => {
    const backend = createMockBackend();
    const library = await backend.getLibrary();
    expect(library.mods).toStrictEqual({ path: MOCK_MODS_DIR, exists: true });
    const names = library.packages.map((p) => p.package.displayName);
    expect(names).toStrictEqual([...names].sort((a, b) => a.toLowerCase().localeCompare(b.toLowerCase())));
    const wardrobe = library.packages.find((p) => p.package.displayName === "Vernworth Wardrobe Collection");
    const hoods = wardrobe?.tree.find((n) => n.label === "Hoods")?.children[0];
    expect(hoods?.label).toBe("Pilgrim Hood (Menu Option)");
    expect(labels(hoods?.children ?? [])).toStrictEqual([
      "option:Pilgrim Hood - Variation 1",
      "option:Pilgrim Hood - Variation 2",
      "bundle:Pilgrim Hood - Southern Dyes",
    ]);
    const bundle = hoods?.children[2];
    expect(bundle?.optionId).toBeNull();
    expect(labels(bundle?.children ?? [])).toStrictEqual([
      "option:Pilgrim Hood - Variation 3",
      "option:Pilgrim Hood - Variation 4",
    ]);
    expect(bundle?.kinds).toStrictEqual({ pak: 0, loose: 4, reframework: 0, root: 0 });
  });

  it("should keep an adopted stand-in with no package uninstallable from its own records", async () => {
    const backend = createMockBackend();
    const view = await backend.getInstalled();
    const stand = view.installs.find((r) => r.name === "Pawn Banter Quieter");
    expect(stand).toMatchObject({ origin: "adopted", packageMissing: true, kinds: { pak: 1 } });
    expect(view.pakOrder[0]).toBe(stand?.optionId);
    const plan = await backend.planUninstall(stand?.optionId ?? "");
    expect(plan.refused).toBeNull();
    expect(plan.removals.map((r) => r.gamePath)).toStrictEqual(["pak_mods/zrift-0000-PawnBanterQuieter.pak"]);
    const opened = await backend.getOption(stand?.optionId ?? "").then(
      () => null,
      (raw: unknown) => raw,
    );
    expect((opened as RiftFailure).kind).toBe("entryNotFound");
  });

  it("should start the game from Play, refuse a second start while it runs, and stop through its hook", async () => {
    const backend = createMockBackend();
    let changes = 0;
    await backend.on(EVENTS.gameChanged, () => (changes += 1));
    expect(await backend.planLaunch()).toStrictEqual({ launch: "steam", warnings: [], refused: null });
    await backend.launchGame();
    const running = await settled(
      () => backend.getOverview(),
      (o) => o.game?.running === true,
      "the game running",
    );
    expect(running.game?.startedMs).not.toBeNull();
    expect((await backend.planLaunch()).refused).toBe(
      "Dragon's Dogma 2 is already running; switch to its window, or close it before you start it again.",
    );
    backend.closeGame();
    expect((await backend.getOverview()).game).toMatchObject({ running: false, startedMs: null });
    expect(changes).toBe(2);
  });

  it("should land a premium download in the mods folder: queued, linking, downloading with progress, verifying, landed", async () => {
    const backend = onScene("premium");
    const phases: string[] = [];
    const progress: DownloadProgressEvent[] = [];
    await backend.on<DownloadProgressEvent>(EVENTS.downloadProgress, (e) => progress.push(e));
    await backend.on(EVENTS.downloadsChanged, () => {
      void backend.getDownloads().then((rows) => {
        const phase = rows.at(0)?.phase;
        if (phase && phases.at(-1) !== phase) phases.push(phase);
      });
    });
    const before = (await backend.getLibrary()).packages.length;
    const started = await backend.startDownload(412, 2391, null);
    expect(started.phase).toBe("queued");
    const landed = await settled(
      () => backend.getDownloads(),
      (rows) => rows.at(0)?.phase === "landed",
      "the download landed",
    );
    const row: DownloadView | undefined = landed.at(0);
    if (!row) throw new Error("No download row");
    expect(row).toMatchObject({ alreadyHad: false, bytesDone: 12_845_056, bytesTotal: 12_845_056 });
    expect(row.landedAs).toMatch(/^Lantern Glow Recolor-412-2-2-\d+\.zip$/);
    expect(progress.at(-1)?.bytesDone).toBe(12_845_056);
    await settled(
      () => Promise.resolve(phases),
      (p) => p.includes("landed"),
      "the phases told",
    );
    expect(phases).toStrictEqual(["queued", "linking", "downloading", "verifying", "landed"]);
    const library = await backend.getLibrary();
    expect(library.packages).toHaveLength(before + 1);
    const pkg = library.packages.find((p) => p.package.id === row.packageId);
    expect(pkg?.package.nexus).toMatchObject({ modId: 412, fileId: 2391, source: "download" });
  });

  it("should park a free account's download until the site's link arrives, and refuse an expired link", async () => {
    const backend = onScene("key");
    const received: NxmReceivedEvent[] = [];
    await backend.on<NxmReceivedEvent>(EVENTS.nxmReceived, (e) => received.push(e));
    const waiting = await backend.startDownload(412, 2391, null);
    expect(waiting.phase).toBe("waiting");
    expect(waiting.nexusPageUrl).toBe("https://www.nexusmods.com/dragonsdogma2/mods/412?tab=files&file_id=2391&nmm=1");

    backend.deliverNxm("nxm://dragonsdogma2/mods/412/files/2391?key=invented&expires=1000");
    expect(received.at(-1)).toMatchObject({ modId: 412, fileId: 2391, accepted: false });
    expect(received.at(-1)?.refusal?.detail?.reason).toBe("linkExpired");

    const later = Math.floor(Date.now() / 1000) + 3600;
    backend.deliverNxm(`nxm://dragonsdogma2/mods/412/files/2391?key=invented&expires=${later}`);
    expect(received.at(-1)).toMatchObject({ accepted: true, downloadId: waiting.id, refusal: null });
    await settled(
      () => backend.getDownloads(),
      (rows) => rows.at(0)?.phase === "landed",
      "the resumed download landed",
    );
  });
});
