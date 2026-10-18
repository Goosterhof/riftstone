// The bench the window specs stand on: every createMockBackend() is its own
// library, so a link, an unlink or an MD5 identify in one spec can never leak
// into the next; and the mock refuses what the real spine cannot do offline
// (an MD5 identify is a Nexus lookup), so an offline spec reads the truth.

import { afterEach, describe, expect, it, vi } from "vitest";
import {
  EVENTS,
  type DownloadProgressEvent,
  type DownloadView,
  type NxmReceivedEvent,
  type TreeNode,
  type UpdatesView,
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

  it("should land a premium download in the spine's order: linked before any scan, landed, then shelved by the scan", async () => {
    const backend = onScene("premium");
    const phases: string[] = [];
    const progress: DownloadProgressEvent[] = [];
    /** The two events a landing rings, in order, and what get_updates says at the link. */
    const rang: string[] = [];
    const updatesAtLink: Promise<UpdatesView>[] = [];
    await backend.on<DownloadProgressEvent>(EVENTS.downloadProgress, (e) => progress.push(e));
    await backend.on(EVENTS.downloadsChanged, () => {
      void backend.getDownloads().then((rows) => {
        const phase = rows.at(0)?.phase;
        if (phase && phases.at(-1) !== phase) phases.push(phase);
      });
    });
    await backend.on(EVENTS.updatesChanged, () => {
      rang.push("updates-changed");
      updatesAtLink.push(backend.getUpdates());
    });
    await backend.on(EVENTS.libraryChanged, () => rang.push("library-changed"));
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
    // every progress event says "downloading": the spine's Meter never says anything else
    expect(new Set(progress.map((e) => e.phase))).toStrictEqual(new Set(["downloading"]));
    await settled(
      () => Promise.resolve(phases),
      (p) => p.includes("landed"),
      "the phases told",
    );
    expect(phases).toStrictEqual(["queued", "linking", "downloading", "verifying", "landed"]);

    // the scan shelves the package after the landing, and only then can get_updates list it
    const library = await settled(
      () => backend.getLibrary(),
      (l) => l.packages.length === before + 1,
      "the scan shelving the landed archive",
    );
    expect(rang).toStrictEqual(["updates-changed", "library-changed"]);
    const firstRead = updatesAtLink.at(0);
    if (!firstRead) throw new Error("No get_updates was asked at the link");
    expect((await firstRead).rows.some((r) => r.packageId === row.packageId)).toBe(false);
    const pkg = library.packages.find((p) => p.package.id === row.packageId);
    expect(pkg?.package.nexus).toMatchObject({ modId: 412, fileId: 2391, source: "download" });
    const now = (await backend.getUpdates()).rows.find((r) => r.packageId === row.packageId);
    expect(now?.link).toMatchObject({ modId: 412, fileId: 2391, source: "download" });

    // the same file again: already had, linked and told all the same, and nothing new to scan
    await backend.startDownload(412, 2391, null);
    const again = await settled(
      () => backend.getDownloads(),
      (rows) => rows.at(0)?.phase === "landed",
      "the second landing",
    );
    expect(again.at(0)).toMatchObject({ alreadyHad: true, packageId: row.packageId });
    await new Promise((resolve) => setTimeout(resolve, 250));
    expect(rang).toStrictEqual(["updates-changed", "library-changed", "updates-changed"]);
    expect((await backend.getLibrary()).packages).toHaveLength(before + 1);
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

// ---------------------------------------------------------------------------
// The mock answers as the spine answers (src-tauri nxm.rs, downloads.rs,
// commands/nexus.rs, commands/play.rs, commands/overview.rs; dto.rs
// of_tracked): the same checks in the same order, the same sentences, so a
// spec on the mock pins what the real stone does.
// ---------------------------------------------------------------------------

const refusalOf = (raw: unknown): RiftFailure => {
  if (!(raw instanceof RiftFailure)) throw new Error(`Not a RiftFailure: ${String(raw)}`);
  return raw;
};

/** The download list under fake timers: the mock answers after its own 20 ms. */
async function peek(backend: MockBackend): Promise<DownloadView[]> {
  const list = backend.getDownloads();
  await vi.advanceTimersByTimeAsync(20);
  return list;
}

async function received(backend: MockBackend, link: string): Promise<NxmReceivedEvent> {
  const heard: NxmReceivedEvent[] = [];
  const off = await backend.on<NxmReceivedEvent>(EVENTS.nxmReceived, (e) => heard.push(e));
  backend.deliverNxm(link);
  off();
  const event = heard.at(0);
  if (!event) throw new Error(`No nxm-received for ${link}`);
  return event;
}

/** An nxm grant that runs out an hour from now. */
const later = (): number => Math.floor(Date.now() / 1000) + 3600;

describe("the mock spine answers as the spine does", () => {
  it("should read an nxm link as parse_nxm does: another game, a collection or an unreadable link carries no ids", async () => {
    const backend = onScene("key");
    const other = await received(backend, `nxm://otherworld/mods/5/files/6?key=invented&expires=${later()}`);
    expect(other).toStrictEqual({
      modId: null,
      fileId: null,
      accepted: false,
      downloadId: null,
      refusal: {
        kind: "refused",
        detail: {
          reason: "That Nexus Mods link is for another game. The Riftstone handles Dragon's Dogma 2 mods only.",
        },
      },
    });
    const collection = await received(backend, "nxm://dragonsdogma2/collections/vernworth/revisions/1");
    expect(collection.refusal?.detail?.reason).toBe(
      "That Nexus Mods link opens a collection. The Riftstone downloads single mod files: download the collection's mods from their own pages.",
    );
    expect([collection.modId, collection.fileId]).toStrictEqual([null, null]);
    for (const unreadable of [
      "nxm://dragonsdogma2/mods/2207",
      "nxm://dragonsdogma2/mods/2207/files/0?key=invented&expires=1",
      `nxm://dragonsdogma2/mods/2207/files/9001?key=invented`,
      `nxm://dragonsdogma2/mods/2207/files/9001?expires=${later()}`,
      `nxm://dragonsdogma2/mods/2207/files/9001?key=invented&expires=${later()}&user_id=someone`,
    ]) {
      const event = await received(backend, unreadable);
      expect({ unreadable, ids: [event.modId, event.fileId], refusal: event.refusal }).toStrictEqual({
        unreadable,
        ids: [null, null],
        refusal: {
          kind: "refused",
          detail: {
            reason: "That nxm link could not be read. Press Mod Manager Download on the file's Nexus Mods page again.",
          },
        },
      });
    }
  });

  it("should refuse a file link in the spine's order: no key, a refused key, no grant, expired, another account's", async () => {
    const keyless = await received(onScene(""), `nxm://dragonsdogma2/mods/412/files/2391?key=invented&expires=1`);
    expect([keyless.modId, keyless.fileId, keyless.refusal?.detail?.reason]).toStrictEqual([412, 2391, "noKey"]);
    const rejected = await received(
      onScene("rejected"),
      `nxm://dragonsdogma2/mods/412/files/2391?key=invented&expires=1`,
    );
    expect(rejected.refusal?.detail?.reason).toBe("keyRejected");

    const backend = onScene("key");
    // the site's button always sends its grant: a link without one is not the button's
    const bare = await received(backend, "nxm://dragonsdogma2/mods/412/files/2391");
    expect([bare.modId, bare.fileId, bare.refusal?.kind]).toStrictEqual([412, 2391, "refused"]);
    expect(bare.refusal?.detail?.reason).toMatch(/^That nxm link could not be read\./);
    const expired = await received(backend, "nxm://dragonsdogma2/mods/412/files/2391?key=invented&expires=1000");
    expect(expired.refusal?.detail?.reason).toBe("linkExpired");
    const notYours = await received(
      backend,
      `nxm://dragonsdogma2/mods/412/files/2391?key=invented&expires=${later()}&user_id=1`,
    );
    expect(notYours.refusal?.detail?.reason).toBe("linkNotYours");
    expect(await backend.getDownloads()).toStrictEqual([]);
  });

  it("should store no key Nexus could not be asked about, as set_nexus_key refuses it offline", async () => {
    const backend = onScene("offline");
    const refused = refusalOf(await backend.setNexusKey("rift-bench-key-0000-invented").catch((raw: unknown) => raw));
    expect([refused.kind, refused.detail.reason]).toStrictEqual(["nexus", "offline"]);
    expect((await backend.getOverview()).nexus).toMatchObject({ key: "none", hasApiKey: false, account: null });
  });

  it("should answer a second start of a file already on its way with that download: no second row, no event", async () => {
    const backend = onScene("key");
    const first = await backend.startDownload(412, 2391, null);
    let rang = 0;
    await backend.on(EVENTS.downloadsChanged, () => (rang += 1));
    const second = await backend.startDownload(412, 2391, null);
    expect(second.id).toBe(first.id);
    expect((await backend.getDownloads()).map((d) => d.id)).toStrictEqual([first.id]);
    expect(rang).toBe(0);
  });

  it("should name tracked mods as of_tracked does: the author, the counts and the summary unknown", async () => {
    const backend = onScene("key");
    const { tracked } = await backend.getNexusStandings(false);
    expect(tracked.length).toBeGreaterThan(0);
    for (const card of tracked) {
      expect([card.author, card.downloads, card.endorsements, card.summary]).toStrictEqual([null, null, null, null]);
      expect(card.pageUrl).toBe(`https://www.nexusmods.com/dragonsdogma2/mods/${card.modId}`);
    }
  });

  it("should refuse a cancel in the spine's words, and keep the bytes a cancel left", async () => {
    const backend = onScene("premium");
    const missing = refusalOf(await backend.cancelDownload("download-404").catch((raw: unknown) => raw));
    expect(missing.detail.reason).toBe(
      "That download is not in this session's list any more. Look again at the downloads.",
    );
    const row = await backend.startDownload(412, 2391, null);
    await settled(
      () => backend.getDownloads(),
      (rows) => (rows.at(0)?.bytesDone ?? 0) > 0,
      "bytes arriving",
    );
    await backend.cancelDownload(row.id);
    const cancelled = (await backend.getDownloads()).at(0);
    expect(cancelled?.phase).toBe("cancelled");
    expect(cancelled?.bytesDone).toBeGreaterThan(0);
    const over = refusalOf(await backend.cancelDownload(row.id).catch((raw: unknown) => raw));
    expect(over.detail.reason).toBe(
      "That download is already over (landed, failed or cancelled), so there is nothing to cancel.",
    );
  });

  it("should refuse a cancel once the landing has committed, as downloads.rs does: the archive lands", async () => {
    vi.useFakeTimers();
    try {
      const backend = onScene("premium");
      const asked = backend.startDownload(412, 2391, null);
      await vi.advanceTimersByTimeAsync(60);
      const row = await asked;
      // the worker's beat, linking, six chunks of bytes, then the size and MD5 checks: the landing commits
      await vi.advanceTimersByTimeAsync(60 + 250 + 6 * 210 + 250 + 1);
      const tried = backend.cancelDownload(row.id).then(
        () => null,
        (raw: unknown) => raw,
      );
      const during = backend.getDownloads();
      await vi.advanceTimersByTimeAsync(40);
      // still verifying to the window, yet past the point of no return
      expect((await during).at(0)?.phase).toBe("verifying");
      expect(refusalOf(await tried).detail.reason).toBe(
        "That download is being put in the mods folder right now and can no longer be stopped. It lands in a moment; remove the archive in Explorer if you do not want it.",
      );
      await vi.advanceTimersByTimeAsync(100);
      expect((await peek(backend)).at(0)?.phase).toBe("landed");
    } finally {
      vi.useRealTimers();
    }
  });

  it("should plan the exe route without a game, as no_game() does", async () => {
    const backend = onScene("nogame");
    expect(await backend.planLaunch()).toStrictEqual({
      launch: "exe",
      warnings: [],
      refused: "Dragon's Dogma 2 was not found, so pick the game folder before you play.",
    });
  });

  it("should word the empty mods folder's check as overview.rs does", async () => {
    const backend = onScene("emptymods");
    const mods = (await backend.getOverview()).checks.find((c) => c.id === "mods");
    expect(mods).toStrictEqual({
      id: "mods",
      level: "info",
      title: "The mods folder is empty",
      detail: `Put each mod's zip, 7z or rar, or its unpacked folder, directly in ${MOCK_MODS_DIR}. The Riftstone reads it, never changes what is there, and sees new mods by itself.`,
      action: { kind: "openModsFolder" },
    });
  });
});
