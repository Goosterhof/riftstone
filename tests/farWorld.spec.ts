// THE FAR WORLD v0.3 (design system #00015 §16.3): the key does visible work.
// The account card, the downloads shelf, an update row's Download, a mod's
// files, browsing Nexus, the tracked mods, Endorse / Abstain and the nxm
// switch, all against the mock spine with its invented account.
import { afterEach, describe, expect, it, vi } from "vitest";
import type { DownloadView, NexusReason } from "@/types/rift";
import { MOCK_ACCOUNT_NAME, MOCK_BROWSE, MOCK_OTHER_HANDLER, MOCK_USER_ID } from "@/services/mock/far";
import { MOCK_PACKAGES } from "@/services/mock/library";
import { RiftFailure } from "@/services/rift";
import { useNavigation } from "@/composables/useNavigation";
import { useDownloads } from "@/composables/useDownloads";
import { useLibrary } from "@/composables/useLibrary";
import { useToasts } from "@/composables/useToasts";
import { button, hasButton, sleep, spine, text, textAt, textOf, until, wakeOn, type Scene } from "./harness";

async function openFarWorld(scene: Scene = ""): Promise<void> {
  await wakeOn(scene);
  useNavigation().go("nexus");
  await until(() => hasButton("Find all by name") || hasButton("Search"), "the far world");
}

const heads = (): string[] => [...document.querySelectorAll<HTMLElement>(".far__body h2")].map((h) => textOf(h).trim());

const sorts = (): string[] =>
  [...document.querySelectorAll<HTMLOptionElement>("[data-browse-sort] option")].map((o) => o.value);

const nxmLine = (): string => textOf(document.getElementById("nxm-why") ?? document.body).trim();

afterEach(() => {
  sleep();
  vi.restoreAllMocks();
});

describe("The far world v0.3", () => {
  it("should read its section heads in the ruled order, a download landing cyan and never gold", async () => {
    await openFarWorld("premium");
    await until(() => document.querySelector("[data-update-download]") !== null, "an update row's Download");
    const door = document.querySelector<HTMLButtonElement>("[data-update-download]");
    expect(textOf(door as Node)).toMatch(/^Download v|^Download$/);
    door?.click();
    await until(() => document.querySelector("#far-downloads") !== null, "the DOWNLOADS shelf");
    await until(() => text().includes("TRACKED ON NEXUS"), "the TRACKED section");
    expect(heads()).toStrictEqual(["ACCOUNT", "DOWNLOADS", "UPDATES", "UNLINKED", "TRACKED ON NEXUS", "FIND ON NEXUS"]);

    await until(() => document.querySelector("[data-phase='landed']") !== null, "the download landing", 10_000);
    const row = document.querySelector<HTMLElement>("[data-phase='landed']");
    expect(textOf(row as Node)).toMatch(/In your mods folder/);
    expect(row?.className).not.toMatch(/world|gold/);
    expect(text()).toMatch(/is in your mods folder\. Install it from the Rift\./);

    const landed = useDownloads().downloads.value.find((d) => d.phase === "landed");
    expect(typeof landed?.packageId).toBe("string");
    const show = [...(row?.querySelectorAll("button") ?? [])].find((b) => textOf(b).includes("Show in the Rift"));
    show?.click();
    await until(() => useNavigation().page.value === "rift", "THE RIFT");
    await until(() => useLibrary().focusedKey.value === landed?.packageId, "the landed package focused in the tree");
    expect(useLibrary().isExpanded(landed?.packageId ?? "")).toBe(true);
  }, 20_000);

  it("should name a premium account with one neutral badge and one sentence", async () => {
    await openFarWorld("premium");
    await until(() => text().includes(MOCK_ACCOUNT_NAME), "the account card");
    const card = document.querySelector(".acct");
    expect(textOf(card as Node)).toContain("PREMIUM");
    expect(card?.querySelectorAll("[data-unlock]")).toHaveLength(1);
    expect(textOf(card as Node)).toContain("lets the stone download mods straight into your mods folder");
    expect(hasButton("Check again") && hasButton("Replace key…") && hasButton("Remove key")).toBe(true);
  });

  it("should tell a free account that Nexus sends its downloads through its own button", async () => {
    await openFarWorld("key");
    await until(() => text().includes(MOCK_ACCOUNT_NAME), "the account card");
    expect(textOf(document.querySelector(".acct") as Node)).toContain("FREE");
    expect(text()).toContain("With a free account Nexus sends downloads through its own Mod Manager Download button");
  });

  it("should say who answers the Mod Manager Download button, and keep the switch's name", async () => {
    await openFarWorld("key");
    await until(() => nxmLine() === "No program answers it now.", "the switch at none");
    const sw = document.querySelector<HTMLButtonElement>("[role=switch]");
    const name = textOf(sw?.querySelector(".nxm__label") as Node);
    expect(sw?.getAttribute("aria-checked")).toBe("false");
    sw?.click();
    await until(() => sw?.getAttribute("aria-checked") === "true", "the switch on");
    expect(nxmLine()).toBe("The stone answers it now. Turning this off gives it back to no program.");
    expect(textOf(sw?.querySelector(".nxm__label") as Node)).toBe(name);
    sleep();

    await openFarWorld(["key", "nxmother"].join(",") as Scene);
    await until(() => nxmLine().startsWith(MOCK_OTHER_HANDLER), "the switch at other");
    expect(nxmLine()).toBe(
      `${MOCK_OTHER_HANDLER} answers it now. The stone remembers it and gives it back when you turn this off.`,
    );
  });

  it("should send the field's words only on Search, and append the next page on Show 20 more", async () => {
    await openFarWorld();
    const browse = vi.spyOn(spine(), "browseNexus");
    const field = document.querySelector<HTMLInputElement>("[data-browse-field]");
    if (!field) throw new Error("no browse field");
    expect(sorts()).not.toContain("relevance");
    field.value = "a";
    field.dispatchEvent(new Event("input"));
    await until(() => sorts().includes("relevance"), "Relevance with words");
    field.value = "";
    field.dispatchEvent(new Event("input"));
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(browse).not.toHaveBeenCalled();

    button("Search").click();
    await until(() => document.querySelectorAll("#far-find [data-mod]").length > 0, "the first page");
    expect(browse).toHaveBeenCalledTimes(1);
    expect(text()).toMatch(/\d+ mods on Nexus/);
    const first = document.querySelectorAll("#far-find [data-mod]").length;
    expect(first).toBe(20);
    button("Show 20 more").click();
    await until(() => document.querySelectorAll("#far-find [data-mod]").length > first, "the next page");
    expect(browse).toHaveBeenCalledTimes(2);
    expect(browse.mock.calls[1]?.[0].offset).toBe(20);
  });

  it("should open a mod's files in place of the scroll, fold the retired ones, and return on Esc", async () => {
    await openFarWorld("premium");
    button("Search").click();
    await until(() => document.querySelectorAll("#far-find [data-mod]").length > 0, "the first page");
    const files = [...document.querySelectorAll<HTMLButtonElement>("#far-find button")].find(
      (b) => textOf(b).trim() === "Files",
    );
    files?.click();
    await until(() => document.querySelectorAll("[data-mod-files] [data-file]").length > 0, "the mod's files");
    expect(hasButton("‹ The far world")).toBe(true);
    const heads = [...document.querySelectorAll("[data-mod-files] h3")].map((h) => textOf(h).trim());
    expect(heads.some((h) => /OLD VERSIONS|REMOVED|ARCHIVED/.test(h))).toBe(false);
    expect(
      [...document.querySelectorAll("[data-mod-files] button")].some((b) => textOf(b).trim().startsWith("Download")),
    ).toBe(true);
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    await until(() => document.querySelector("[data-mod-files]") === null, "the far world again");
  });

  it("should say an endorsement refusal under the pair, never as a toast", async () => {
    await openFarWorld("premium");
    await until(() => document.querySelector("#far-updates [data-standing]") !== null, "a linked row's standing");
    vi.spyOn(spine(), "setEndorsement").mockRejectedValueOnce(
      new RiftFailure({
        kind: "nexus",
        detail: { detail: "Nexus refused: you cannot endorse your own mod.", reason: "endorseOwnMod" },
      }),
    );
    const pair = document.querySelector<HTMLElement>("#far-updates [data-standing]");
    const endorse = [...(pair?.querySelectorAll("button") ?? [])].find((b) => textOf(b).trim() === "Endorse");
    expect(endorse?.getAttribute("aria-pressed")).not.toBeNull();
    endorse?.click();
    await until(() => textOf(pair as Node).includes("does not let an author endorse"), "the refusal under the pair");
    expect(
      useToasts()
        .toasts.value.map((t) => t.line)
        .join(" "),
    ).not.toContain("endorse their own mod");
  });
});

// ---------------------------------------------------------------------------
// The key's work, pinned (WINDOW-CONTRACT § v0.3 "The key's extras",
// "Downloads", "The nxm handler"): the account without its key, an update's
// Download for each tier, the shelf's cancel and its voiced failures, browsing
// by pages and orders, a mod's files and patch notes, tracking, endorsing and
// the handler switch.
// ---------------------------------------------------------------------------

/** An invented key: the window passes it to the spine once and never shows it. */
const KEY = "rift-bench-key-0000-invented";
const LANTERN_PAGE = "https://www.nexusmods.com/dragonsdogma2/mods/412?tab=files&file_id=2391&nmm=1";
const FREE_LINE =
  "With a free account Nexus sends downloads through its own Mod Manager Download button; the stone tracks and endorses mods for you here.";
const TAKEN_LINE = "A Mod Manager Download link reached the stone: the download is on its way to your mods folder.";

function must<T>(found: T | null | undefined, what: string): T {
  if (found === null || found === undefined) throw new Error(`The far world drew no ${what}`);
  return found;
}

const lanternId = must(
  MOCK_PACKAGES.find((p) => p.summary.displayName === "Lantern Glow Recolor"),
  "Lantern Glow Recolor in the mock library",
).summary.id;

const later = (): number => Math.floor(Date.now() / 1000) + 3600;
const toastLines = (): string[] => useToasts().toasts.value.map((t) => t.line);
const toneOf = (line: string): string | undefined => useToasts().toasts.value.find((t) => t.line === line)?.tone;
const labelsIn = (el: Element): string[] =>
  [...el.querySelectorAll(".rs-btn__label")].map((l) => textOf(l).trim()).filter(Boolean);
const shelf = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>("#far-downloads [data-download]")];
const phaseWord = (row: Element): string => textOf(must(row.querySelector(".shelf__phase"), "phase word")).trim();
const updateDoor = (): HTMLButtonElement | null =>
  document.querySelector<HTMLButtonElement>(`[data-update-download="${lanternId}"]`);
const keyChip = (): string => textOf(must(document.querySelector("#far-account .far__chip"), "key chip")).trim();
const indexWords = (): string[] => [...document.querySelectorAll(".far__index a")].map((a) => textOf(a).trim());
const findCards = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>("#far-find [data-mod]")];
const trackedCards = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>("#far-tracked [data-mod]")];
const cardName = (card: Element): string => textOf(must(card.querySelector(".fcard__name"), "card name")).trim();
const browseSorts = (): string[] =>
  [...document.querySelectorAll<HTMLOptionElement>("[data-browse-sort] option")].map((o) => o.value);
const fileRow = (fileId: number): HTMLElement =>
  must(document.querySelector<HTMLElement>(`[data-mod-files] [data-file="${fileId}"]`), `file ${fileId}`);
const fileHeads = (): string[] => [...document.querySelectorAll("[data-mod-files] h3")].map((h) => textOf(h).trim());

function buttonIn(el: Element, label: string): HTMLButtonElement {
  return must(
    [...el.querySelectorAll<HTMLButtonElement>("button")].find(
      (b) => textOf(b.querySelector(".rs-btn__label") ?? b).trim() === label,
    ),
    `"${label}" button`,
  );
}

function typeInto(field: HTMLInputElement, value: string): void {
  field.value = value;
  field.dispatchEvent(new Event("input"));
}

function chooseSort(value: string): void {
  const select = must(document.querySelector<HTMLSelectElement>("[data-browse-sort]"), "sort select");
  select.value = value;
  select.dispatchEvent(new Event("change"));
}

const lanternFiles = (): HTMLButtonElement | null =>
  document.querySelector<HTMLButtonElement>('[aria-label="Files of Lantern Glow Recolor on Nexus"]');
/** Pawn Chatter Filter's Endorse / Abstain pair on its UPDATES row (mod 3301). */
const chatter = (): HTMLElement | null => document.querySelector<HTMLElement>('#far-updates [data-standing="3301"]');

const browseField = (): HTMLInputElement =>
  must(document.querySelector<HTMLInputElement>("[data-browse-field]"), "browse field");

/** One of this session's downloads, as the spine lists it. */
function downloadRow(over: Partial<DownloadView>): DownloadView {
  return {
    id: "download-41",
    modId: 412,
    fileId: 2391,
    modName: "Lantern Glow Recolor",
    fileName: "Lantern Glow Recolor",
    version: "2.2",
    forPackageId: lanternId,
    phase: "downloading",
    bytesDone: 4_194_304,
    bytesTotal: 12_845_056,
    landedAs: null,
    packageId: null,
    alreadyHad: false,
    failure: null,
    nexusPageUrl: null,
    startedMs: Date.now() - 4_000,
    finishedMs: null,
    ...over,
  };
}

describe("The far world v0.3: the key's work", () => {
  it("should take a pasted key without ever drawing it, and name the free account it belongs to", async () => {
    await openFarWorld();
    await until(() => text().includes("THE FAR WORLD IS SILENT"), "the keyless account");
    expect(keyChip()).toBe("NO KEY");
    const sent = vi.spyOn(spine(), "setNexusKey");
    button("Paste key…").click();
    await until(() => document.querySelector("#far-account input[type=password]") !== null, "the key field");
    typeInto(must(document.querySelector<HTMLInputElement>("#far-account input[type=password]"), "key field"), KEY);
    await until(() => !button("Save & test").disabled, "Save & test ready");
    button("Save & test").click();
    await until(() => document.querySelector(".acct") !== null, "the account card");
    // the field closes once the key is saved and its quota read
    await until(() => document.querySelector("#far-account input[type=password]") === null, "the key field closed");

    expect(sent.mock.calls).toStrictEqual([[KEY]]);
    expect(keyChip()).toBe("CONNECTED");
    const card = must(document.querySelector(".acct"), "account card");
    expect(textOf(must(card.querySelector(".acct__name"), "name")).trim()).toBe(MOCK_ACCOUNT_NAME);
    expect([...card.querySelectorAll(".acct__tier")].map((t) => textOf(t).trim())).toStrictEqual(["FREE"]);
    expect([...card.querySelectorAll("[data-unlock]")].map((u) => textOf(u).trim())).toStrictEqual([FREE_LINE]);
    expect(textOf(card)).not.toMatch(/@/);
    // never the key: not in the page, not left in a field
    expect(document.body.innerHTML).not.toContain(KEY);
    expect([...document.querySelectorAll("input")].map((i) => i.value)).not.toContain(KEY);
    expect(document.querySelector("#far-account input[type=password]")).toBeNull();

    button("Remove key").click();
    await until(() => keyChip() === "NO KEY", "the key removed");
    expect(sent.mock.calls).toStrictEqual([[KEY], [null]]);
    expect(document.querySelector(".acct")).toBeNull();
  });

  it("should download a premium update into the mods folder: the row says so, the tree shows it, and Nexus wants 15 minutes before an endorsement", async () => {
    await openFarWorld("premium");
    await until(() => updateDoor() !== null, "the update row's Download");
    expect(textOf(must(updateDoor()?.querySelector(".rs-btn__label"), "label")).trim()).toBe("Download v2.2");
    const started = vi.spyOn(spine(), "startDownload");
    updateDoor()?.click();
    await until(() => shelf().at(0)?.dataset.phase === "landed", "the download landed", 10_000);
    expect(started.mock.calls).toStrictEqual([[412, 2391, lanternId]]);
    const landed = must(useDownloads().downloads.value.at(0), "landed row");
    expect(landed.landedAs).toMatch(/^Lantern Glow Recolor-412-2-2-\d+\.zip$/);
    const line = `Lantern Glow Recolor landed in your mods folder as ${landed.landedAs ?? ""}. Install it from the Rift.`;
    expect(toneOf(line)).toBe("world");
    await until(() => updateDoor() === null, "the update row's Download gone");
    expect(
      textOf(must(document.querySelector(".far__landed"), "landed line"))
        .replace(/\s+/g, " ")
        .trim(),
    ).toBe("v2.2 is in your mods folder. Install it from the Rift. Show in the Rift");

    // the account downloaded it a moment ago: Nexus wants 15 minutes, said under the pair
    const pair = must(document.querySelector<HTMLElement>('#far-updates [data-standing="412"]'), "Lantern's standing");
    buttonIn(pair, "Endorse").click();
    await until(() => pair.querySelector(".stand__refusal") !== null, "the refusal under the pair");
    expect(textOf(must(pair.querySelector(".stand__refusal"), "refusal")).trim()).toBe(
      "Nexus wants 15 minutes between a download and an endorsement. Nothing changed on Nexus. Endorse it again in a little while.",
    );
    expect(toastLines().some((l) => l.includes("15 minutes"))).toBe(false);

    // a landed download is a package in the Rift like any other
    useNavigation().go("rift");
    const packageId = must(landed.packageId, "landed package id");
    await until(
      () => document.querySelector(`#rift-tree [data-key="${CSS.escape(packageId)}"]`) !== null,
      "the new package in the tree",
    );
    const top = [...document.querySelectorAll('#rift-tree [role="treeitem"][aria-level="1"]')].map((r) =>
      r.getAttribute("aria-label"),
    );
    expect(top).toHaveLength(12);
    expect(top.filter((name) => name === "Lantern Glow Recolor")).toHaveLength(2);
  }, 20_000);

  it("should send a free account to the file's page, and finish the download when the site's link comes back", async () => {
    await openFarWorld("key");
    await until(() => updateDoor() !== null, "the update row's door");
    expect(textOf(must(updateDoor()?.querySelector(".rs-btn__label"), "label")).trim()).toBe("Get it on Nexus");
    const opened = vi.spyOn(window, "open");
    updateDoor()?.click();
    await until(() => shelf().at(0)?.dataset.phase === "waiting", "the waiting row");
    expect(opened.mock.calls).toStrictEqual([[LANTERN_PAGE, "_blank", "noopener"]]);
    const row = must(shelf().at(0), "waiting row");
    const id = row.dataset.download;
    expect(phaseWord(row)).toBe("Waiting for Nexus");
    expect(textOf(must(row.querySelector(".shelf__press"), "press line")).trim()).toBe(
      "Press Mod Manager Download on Nexus.",
    );
    expect(labelsIn(row)).toStrictEqual(["Open on Nexus", "Cancel"]);

    // Windows hands the stone the site's link while the player is elsewhere
    useNavigation().go("rift");
    spine().deliverNxm(
      `nxm://dragonsdogma2/mods/412/files/2391?key=invented&expires=${later()}&user_id=${MOCK_USER_ID}`,
    );
    await until(() => toastLines().includes(TAKEN_LINE), "the link taken");
    expect(useNavigation().page.value).toBe("nexus");
    await until(() => shelf().at(0)?.dataset.phase === "landed", "the resumed download landed", 10_000);
    expect(useDownloads().downloads.value.map((d) => [d.id, d.phase])).toStrictEqual([[id, "landed"]]);
  }, 20_000);

  it.each([
    [
      "an expired link",
      "nxm://dragonsdogma2/mods/412/files/2391?key=invented&expires=1000",
      "That Mod Manager Download link has expired. Nothing was downloaded. Press Mod Manager Download on the file's Nexus page again for a fresh one.",
    ],
    [
      "another account's link",
      "nxm://dragonsdogma2/mods/412/files/2391?key=invented&expires=EXPIRES&user_id=1",
      "That link was made for another Nexus account than the one whose key the stone holds. Nothing was downloaded. Sign in to Nexus with that key's account, or replace the key, then press Mod Manager Download again.",
    ],
  ])("should refuse %s in the stone's voice and keep the waiting download waiting", async (_what, link, line) => {
    await openFarWorld("key");
    await until(() => updateDoor() !== null, "the update row's door");
    updateDoor()?.click();
    await until(() => shelf().at(0)?.dataset.phase === "waiting", "the waiting row");
    spine().deliverNxm(link.replace("EXPIRES", String(later())));
    await until(() => toastLines().includes(line), "the refusal");
    expect(toneOf(line)).toBe("ember");
    expect(useDownloads().downloads.value.map((d) => d.phase)).toStrictEqual(["waiting"]);
    expect(toastLines()).not.toContain(TAKEN_LINE);
  });

  it("should refuse a link for another game or a collection, and download nothing", async () => {
    await openFarWorld("key");
    await until(() => keyChip() === "CONNECTED", "the connected account");
    spine().deliverNxm(`nxm://otherworld/mods/5/files/6?key=invented&expires=${later()}`);
    await until(() => toastLines().some((l) => l.includes("That link is for another game")), "the other game refused");
    spine().deliverNxm("nxm://dragonsdogma2/collections/vernworth/revisions/1");
    await until(() => toastLines().some((l) => l.includes("That link is for a collection")), "the collection refused");
    expect(
      useToasts()
        .toasts.value.filter((t) => t.line.startsWith("The stone held back: That link is for"))
        .map((t) => t.tone),
    ).toStrictEqual(["ember", "ember"]);
    expect(useDownloads().downloads.value).toStrictEqual([]);
    expect(document.getElementById("far-downloads")).toBeNull();
  });

  it("should cancel a download in flight, leave nothing in the mods folder, and clear the finished row", async () => {
    await openFarWorld("premium");
    await until(() => updateDoor() !== null, "the update row's Download");
    const before = (await spine().getLibrary()).packages.length;
    updateDoor()?.click();
    await until(() => ["queued", "linking"].includes(shelf().at(0)?.dataset.phase ?? ""), "the download under way");
    const row = must(shelf().at(0), "download row");
    must(
      row.querySelector<HTMLButtonElement>('[aria-label="Cancel the download of Lantern Glow Recolor"]'),
      "Cancel",
    ).click();
    await until(() => shelf().at(0)?.dataset.phase === "cancelled", "the download cancelled");
    expect(phaseWord(must(shelf().at(0), "row"))).toBe("Cancelled");
    // longer than the whole run would take: it never lands
    await new Promise((resolve) => setTimeout(resolve, 2_300));
    expect(useDownloads().downloads.value.map((d) => d.phase)).toStrictEqual(["cancelled"]);
    expect((await spine().getLibrary()).packages).toHaveLength(before);
    expect(updateDoor()).not.toBeNull();

    expect(indexWords()).toStrictEqual(["Account", "Downloads", "Updates", "Unlinked", "Tracked", "Find"]);
    button("Clear finished").click();
    await until(() => document.getElementById("far-downloads") === null, "the shelf cleared");
    expect(indexWords()).toStrictEqual(["Account", "Updates", "Unlinked", "Tracked", "Find"]);
  }, 20_000);

  it.each<[NexusReason, string]>([
    [
      "premiumRequired",
      "Nexus lets only premium accounts download from inside the stone. Nothing was written to your mods folder. Press Mod Manager Download on the file's Nexus page instead: while the stone answers nxm:// links, the file comes here.",
    ],
    [
      "linkExpired",
      "That Mod Manager Download link has expired. Nothing was written to your mods folder. Press Mod Manager Download on the file's Nexus page again for a fresh one.",
    ],
  ])("should voice a failed download by its reason (%s), never by the sentence beside it", async (reason, line) => {
    await openFarWorld("key");
    const listed = vi.spyOn(spine(), "getDownloads").mockResolvedValue([downloadRow({})]);
    await useDownloads().load();
    // the sentence says 403, which the words alone would read as a refused key
    const failure = { kind: "nexus" as const, detail: { detail: "Nexus answered HTTP 403.", reason } };
    listed.mockResolvedValue([downloadRow({ phase: "failed", failure, finishedMs: Date.now() })]);
    await useDownloads().load();
    await until(() => shelf().at(0)?.dataset.phase === "failed", "the failed row");
    const row = must(shelf().at(0), "failed row");
    expect(phaseWord(row)).toBe("Failed");
    expect(
      textOf(must(row.querySelector(".shelf__fail"), "failure line"))
        .replace(/\s+/g, " ")
        .trim(),
    ).toBe(line);
    expect(toneOf(line)).toBe("ember");
    expect(labelsIn(row)).toStrictEqual(["Try again"]);

    const started = vi.spyOn(spine(), "startDownload");
    buttonIn(row, "Try again").click();
    await until(() => started.mock.calls.length === 1, "the same file asked for again");
    expect(started.mock.calls).toStrictEqual([[412, 2391, lanternId]]);
  });

  it("should page through Nexus in the order asked, and send new words only on Search", async () => {
    await openFarWorld();
    const browse = vi.spyOn(spine(), "browseNexus");
    chooseSort("downloads");
    button("Search").click();
    await until(() => findCards().length === 20, "the first page");
    const byDownloads = [...MOCK_BROWSE].sort((a, b) => (b.downloads ?? 0) - (a.downloads ?? 0)).map((m) => m.name);
    expect(findCards().map(cardName)).toStrictEqual(byDownloads.slice(0, 20));
    expect(textAt("#far-find .browse__total")).toBe("52 mods on Nexus");

    // words typed after a page stay home: the next page is asked with the page's own words
    typeInto(browseField(), "harbour");
    button("Show 20 more").click();
    await until(() => findCards().length === 40, "the second page");
    button("Show 20 more").click();
    await until(() => findCards().length === 52, "the last page");
    expect(hasButton("Show 20 more")).toBe(false);
    expect(findCards().map(cardName)).toStrictEqual(byDownloads);
    expect(browse.mock.calls.map(([q]) => q)).toStrictEqual([
      { text: "", sort: "downloads", offset: 0 },
      { text: "", sort: "downloads", offset: 20 },
      { text: "", sort: "downloads", offset: 40 },
    ]);

    // Search sends them, and with words Relevance is an order
    expect(browseSorts()).toStrictEqual(["endorsements", "downloads", "updated", "relevance"]);
    chooseSort("relevance");
    button("Search").click();
    await until(() => browse.mock.calls.length === 4 && findCards().length === 6, "the harbour mods");
    expect(browse.mock.calls.at(-1)).toStrictEqual([{ text: "harbour", sort: "relevance", offset: 0 }]);
    expect(textAt("#far-find .browse__total")).toBe("6 mods on Nexus");
    expect(findCards().every((card) => cardName(card).includes("Harbour"))).toBe(true);

    // an empty field has nothing to be relevant to
    typeInto(browseField(), "");
    await until(() => !browseSorts().includes("relevance"), "Relevance withdrawn");
    expect(must(document.querySelector<HTMLSelectElement>("[data-browse-sort]"), "select").value).toBe("endorsements");
    expect(browse).toHaveBeenCalledTimes(4);
  });

  it("should open a mod's files with the retired ones folded, and each file's patch notes behind its own disclosure", async () => {
    await openFarWorld("premium");
    await until(() => lanternFiles() !== null, "the update row's Files");
    lanternFiles()?.click();
    await until(() => document.querySelectorAll("[data-mod-files] [data-file]").length > 0, "the mod's files");
    expect(fileHeads()).toStrictEqual(["MAIN FILES", "OPTIONAL FILES"]);
    expect(
      [...document.querySelectorAll<HTMLElement>("[data-mod-files] [data-file]")].map((f) => f.dataset.file),
    ).toStrictEqual(["2391", "2215"]);
    expect(textOf(fileRow(2391))).toContain("NEWEST");
    expect(textOf(fileRow(2391))).not.toContain("IN YOUR MODS FOLDER");

    const older = must(document.querySelector<HTMLButtonElement>(".files__older"), "Older files");
    expect(older.getAttribute("aria-expanded")).toBe("false");
    expect(textOf(older).replace(/\s+/g, " ").trim()).toBe("Older files ▸ 1");
    older.click();
    await until(() => fileHeads().includes("OLD VERSIONS"), "the older files");
    expect(fileHeads()).toStrictEqual(["MAIN FILES", "OPTIONAL FILES", "OLD VERSIONS"]);
    // the file the library holds carries the chip, and still offers Download: an update may be wanted
    expect(textOf(fileRow(2210))).toContain("IN YOUR MODS FOLDER");
    expect(buttonIn(fileRow(2210), "Download").disabled).toBe(false);

    const notes = must(fileRow(2391).querySelector<HTMLButtonElement>(".files__notes"), "Patch notes");
    expect(notes.getAttribute("aria-expanded")).toBe("false");
    expect(fileRow(2391).querySelector(".files__log")).toBeNull();
    notes.click();
    await until(() => notes.getAttribute("aria-expanded") === "true", "the patch notes open");
    expect([...fileRow(2391).querySelectorAll(".files__log li")].map((li) => textOf(li).trim())).toStrictEqual([
      "Amber for every hand lantern, not only the Arisen's.",
      "Caves keep a softer falloff.",
      "Fixed a flicker at dusk in Vernworth.",
    ]);
    // each file keeps its own notes shut
    expect(fileRow(2215).querySelector(".files__log")).toBeNull();
    notes.click();
    await until(() => fileRow(2391).querySelector(".files__log") === null, "the patch notes shut");

    button("‹ The far world").click();
    await until(() => document.querySelector("[data-mod-files]") === null, "the far world again");
  });

  it("should untrack a tracked mod, and track a mod found on Nexus", async () => {
    await openFarWorld("key");
    await until(() => trackedCards().length === 3, "the tracked mods");
    expect(trackedCards().map(cardName)).toStrictEqual([
      "Vernworth Wardrobe Collection",
      "Ox Cart Express",
      "Old Road Armor Pack",
    ]);
    expect(
      trackedCards()
        .filter((c) => c.querySelector(".fcard__have") !== null)
        .map(cardName),
    ).toStrictEqual(["Vernworth Wardrobe Collection"]);
    const set = vi.spyOn(spine(), "setTracked");
    const ox = must(
      trackedCards().find((c) => cardName(c) === "Ox Cart Express"),
      "Ox Cart Express card",
    );
    const untrack = buttonIn(ox, "Tracked");
    expect([untrack.getAttribute("aria-pressed"), untrack.title]).toStrictEqual(["true", "Stop tracking"]);
    untrack.click();
    await until(() => trackedCards().length === 2, "Ox Cart Express untracked");
    expect(trackedCards().map(cardName)).toStrictEqual(["Vernworth Wardrobe Collection", "Old Road Armor Pack"]);

    button("Search").click();
    await until(() => findCards().length > 0, "mods on Nexus");
    const found = must(findCards().at(0), "first found mod");
    const name = cardName(found);
    expect(buttonIn(found, "Track").getAttribute("aria-pressed")).toBe("false");
    buttonIn(found, "Track").click();
    await until(() => trackedCards().map(cardName).includes(name), "the found mod tracked");
    expect(buttonIn(found, "Tracked").getAttribute("aria-pressed")).toBe("true");
    expect(set.mock.calls).toStrictEqual([
      [6031, false],
      [Number(found.dataset.mod), true],
    ]);
  });

  it("should endorse a linked mod in place, and say under the pair why Nexus refuses a mod this account never downloaded", async () => {
    await openFarWorld("key");
    await until(() => chatter() !== null, "Pawn Chatter Filter's standing");
    const pair = must(chatter(), "standing");
    // the account's standings arrive a beat after the row (get_nexus_standings)
    await until(() => buttonIn(pair, "Abstain").getAttribute("aria-pressed") === "true", "the abstention read");
    expect(
      [buttonIn(pair, "Endorse"), buttonIn(pair, "Abstain")].map((b) => b.getAttribute("aria-pressed")),
    ).toStrictEqual(["false", "true"]);
    expect(textAt('#far-updates [data-standing="3301"] .stand__line')).toBe("You abstained from endorsing this mod.");
    buttonIn(pair, "Endorse").click();
    await until(() => buttonIn(pair, "Endorse").getAttribute("aria-pressed") === "true", "the endorsement");
    expect(buttonIn(pair, "Abstain").getAttribute("aria-pressed")).toBe("false");
    expect(textAt('#far-updates [data-standing="3301"] .stand__line')).toBe("You endorsed this mod.");

    typeInto(browseField(), "melve");
    button("Search").click();
    await until(() => findCards().length > 0, "the Melve mods");
    buttonIn(must(findCards().at(0), "first Melve mod"), "Files").click();
    await until(() => document.querySelector("[data-mod-files] [data-standing]") !== null, "the files' standing");
    const files = must(document.querySelector<HTMLElement>("[data-mod-files] [data-standing]"), "standing");
    buttonIn(files, "Endorse").click();
    await until(() => files.querySelector(".stand__refusal") !== null, "the refusal");
    expect(textOf(must(files.querySelector(".stand__refusal"), "refusal")).trim()).toBe(
      "Nexus takes an endorsement only from an account that downloaded the mod from Nexus. Nothing changed on Nexus. Download it with this account first, then endorse it.",
    );
    expect(buttonIn(files, "Endorse").getAttribute("aria-pressed")).toBe("false");
    expect(toastLines().some((l) => l.includes("takes an endorsement"))).toBe(false);
  });

  it("should take the handler from another program and give it back on off, naming it each way", async () => {
    await openFarWorld("key,nxmother");
    await until(() => nxmLine().startsWith(MOCK_OTHER_HANDLER), "the switch at other");
    const sw = must(document.querySelector<HTMLButtonElement>("[role=switch]"), "switch");
    const word = (): string => textOf(must(sw.querySelector(".nxm__state"), "state word")).trim();
    const set = vi.spyOn(spine(), "setNxmHandler");
    expect([sw.getAttribute("aria-checked"), word()]).toStrictEqual(["false", "OFF"]);

    sw.click();
    await until(() => sw.getAttribute("aria-checked") === "true", "the switch on");
    expect(word()).toBe("ON");
    expect(nxmLine()).toBe(`The stone answers it now. Turning this off gives it back to ${MOCK_OTHER_HANDLER}.`);
    expect(toastLines()).toContain(
      "The stone answers nxm:// links now: Mod Manager Download on a Nexus file page sends the file here.",
    );

    sw.click();
    await until(() => sw.getAttribute("aria-checked") === "false", "the switch off");
    expect(word()).toBe("OFF");
    expect(nxmLine()).toBe(
      `${MOCK_OTHER_HANDLER} answers it now. The stone remembers it and gives it back when you turn this off.`,
    );
    expect(toastLines()).toContain(`nxm:// links go back to ${MOCK_OTHER_HANDLER}.`);
    expect(set.mock.calls).toStrictEqual([[true], [false]]);
  });

  it("should keep the switch shut off Windows, and say why", async () => {
    await wakeOn("key");
    vi.spyOn(spine(), "getNxmHandler").mockResolvedValue({ state: "unsupported", other: null, givesBackTo: null });
    const set = vi.spyOn(spine(), "setNxmHandler");
    useNavigation().go("nexus");
    await until(() => nxmLine() === "Only on Windows.", "the unsupported switch");
    const sw = must(document.querySelector<HTMLButtonElement>("[role=switch]"), "switch");
    expect([sw.getAttribute("aria-disabled"), sw.getAttribute("aria-checked")]).toStrictEqual(["true", "false"]);
    sw.click();
    await new Promise((resolve) => setTimeout(resolve, 150));
    expect(set).not.toHaveBeenCalled();
  });
});
