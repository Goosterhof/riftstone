// THE FAR WORLD v0.3 (design system #00015 §16.3): the key does visible work.
// The account card, the downloads shelf, an update row's Download, a mod's
// files, browsing Nexus, the tracked mods, Endorse / Abstain and the nxm
// switch, all against the mock spine with its invented account.
import { afterEach, describe, expect, it, vi } from "vitest";
import type { DownloadPhase, DownloadView, NexusReason } from "@/types/rift";
import { MOCK_ACCOUNT_NAME, MOCK_BROWSE, MOCK_OTHER_HANDLER, MOCK_USER_ID } from "@/services/mock/far";
import { MOCK_MODS_DIR, MOCK_PACKAGES } from "@/services/mock/library";
import { RiftFailure } from "@/services/rift";
import { useNavigation } from "@/composables/useNavigation";
import { useDownloads } from "@/composables/useDownloads";
import { useLibrary } from "@/composables/useLibrary";
import { useNexusAccount } from "@/composables/useNexusAccount";
import { useNxmHandler } from "@/composables/useNxmHandler";
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

  it("should keep Endorse / Abstain off the UPDATES rows: a linked mod's pair lives in its FILES (R16.22)", async () => {
    await openFarWorld("key");
    await until(() => linkedRowOf(3301) !== null, "Pawn Chatter Filter's update row");
    // the account's standings arrive a beat after the rows: give them time to land
    await until(() => (useNexusAccount().standings.value?.endorsements.length ?? 0) > 0, "the standings read");
    await new Promise((resolve) => setTimeout(resolve, 30));
    expect(document.querySelector("#far-updates [data-standing]")).toBeNull();
    expect(textOf(must(linkedRowOf(3301), "row"))).not.toMatch(/Endorse|Abstain/);
    const pair = await standingOf(3301);
    expect(pair.closest("[data-mod-files]")).not.toBeNull();
  });

  it("should say an endorsement refusal under the pair, never as a toast", async () => {
    await openFarWorld("premium");
    vi.spyOn(spine(), "setEndorsement").mockRejectedValueOnce(
      new RiftFailure({
        kind: "nexus",
        detail: { detail: "Nexus refused: you cannot endorse your own mod.", reason: "endorseOwnMod" },
      }),
    );
    const pair = await standingOf(null);
    const endorse = [...pair.querySelectorAll("button")].find((b) => textOf(b).trim() === "Endorse");
    expect(endorse?.getAttribute("aria-pressed")).not.toBeNull();
    endorse?.click();
    await until(() => textOf(pair).includes("does not let an author endorse"), "the refusal under the pair");
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
const TAKEN_LINE = "Nexus sent the file for Lantern Glow Recolor. Its download is on the shelf.";

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
/** A linked package's UPDATES row, found by the mod id it carries. */
const linkedRowOf = (modId: number | null): HTMLElement | null =>
  [...document.querySelectorAll<HTMLElement>("#far-updates .far__linkedrow")].find(
    (r) => modId === null || textOf(r).includes(`mod ${modId} `),
  ) ?? null;

/** A linked mod's Endorse / Abstain pair. It lives in the mod's FILES view
 *  (R16.22), never on its UPDATES row: open the row's Files and wait for it.
 *  `null` takes the first linked row. */
async function standingOf(modId: number | null): Promise<HTMLElement> {
  await until(() => linkedRowOf(modId) !== null, `mod ${modId ?? "first"}'s update row`);
  buttonIn(must(linkedRowOf(modId), "the update row"), "Files").click();
  const at = modId === null ? "[data-mod-files] [data-standing]" : `[data-mod-files] [data-standing="${modId}"]`;
  await until(() => document.querySelector(at) !== null, `mod ${modId ?? "first"}'s standing in its files`);
  return must(document.querySelector<HTMLElement>(at), "standing");
}

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
    // R16.18 / Bold Choice 8: the landing toast is cyan, names the mod (never the archive) and leads to it
    const line = "Lantern Glow Recolor is in your mods folder. Install it from the Rift.";
    expect(toneOf(line)).toBe("rift");
    expect(useToasts().toasts.value.some((t) => t.tone === "world")).toBe(false);
    expect(
      useToasts()
        .toasts.value.find((t) => t.line === line)
        ?.actions.map((a) => a.label),
    ).toStrictEqual(["Show in the Rift"]);
    await until(() => updateDoor() === null, "the update row's Download gone");
    expect(
      textOf(must(document.querySelector(".far__landed"), "landed line"))
        .replace(/\s+/g, " ")
        .trim(),
    ).toBe("v2.2 is in your mods folder. Install it from the Rift. Show in the Rift");

    // the account downloaded it a moment ago: Nexus wants 15 minutes, said under the pair in its files
    const pair = await standingOf(412);
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
    // each row is named by its own label (aria-labelledby), never by the rows nested inside it
    const top = [...document.querySelectorAll('#rift-tree [role="treeitem"][aria-level="1"]')].map((r) =>
      textOf(must(document.getElementById(r.getAttribute("aria-labelledby") ?? ""), "row label")).trim(),
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
    // Chaos D1: nothing answers the site's button yet, so the row says so and offers the switch's act
    expect(textOf(must(row.querySelector(".shelf__press--off"), "press line")).replace(/\s+/g, " ")).toContain(
      "Nothing hands Nexus's button to the stone now. Let the stone answer it first",
    );
    expect(row.querySelector(".shelf__press:not(.shelf__press--off)")).toBeNull();
    expect(labelsIn(row)).toStrictEqual(["Open on Nexus", "Cancel", "Answer Nexus's button"]);
    // §9 Download waiting: once the stone answers the button, the row says how the file comes
    buttonIn(row, "Answer Nexus's button").click();
    await until(() => row.querySelector(".shelf__press:not(.shelf__press--off)") !== null, "the press line");
    expect(
      textOf(must(row.querySelector(".shelf__press"), "press line"))
        .replace(/\s+/g, " ")
        .trim(),
    ).toBe(
      "Press Mod Manager Download on Nexus. Nexus sends a free account's downloads through its own button, and the stone picks them up from there.",
    );

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
      "That link was made for a different Nexus account than the key in the stone. Nothing was downloaded. Sign in to Nexus with that key's account, or replace the key, then press Mod Manager Download again.",
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

  it("should refuse a link for another game or a collection in the spine's words, and download nothing", async () => {
    await openFarWorld("key");
    await until(() => keyChip() === "CONNECTED", "the connected account");
    const OTHER_GAME =
      "That Nexus Mods link is for another game. The Riftstone handles Dragon's Dogma 2 mods only. Nothing was downloaded, and no setting changed.";
    const COLLECTION =
      "That Nexus Mods link opens a collection. The Riftstone downloads single mod files: download the collection's mods from their own pages. Nothing was downloaded, and no setting changed.";
    spine().deliverNxm(`nxm://otherworld/mods/5/files/6?key=invented&expires=${later()}`);
    await until(() => toastLines().includes(OTHER_GAME), "the other game refused");
    spine().deliverNxm("nxm://dragonsdogma2/collections/vernworth/revisions/1");
    await until(() => toastLines().includes(COLLECTION), "the collection refused");
    expect([toneOf(OTHER_GAME), toneOf(COLLECTION)]).toStrictEqual(["ember", "ember"]);
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
      "Nexus lets only premium accounts download straight into a mod manager. Nothing was written to your mods folder. Press Mod Manager Download on the file's Nexus page and the stone takes it from there, while it answers Nexus's button.",
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
    ).toBe(`The download of Lantern Glow Recolor stopped. ${line}`);
    // the far world is on screen and its row says it: no toast repeats the sentence
    expect(toneOf(line)).toBeUndefined();
    // a second try would fail the same way: the way out is the file's page, where a fresh link is made
    expect(labelsIn(row)).toStrictEqual(["Open on Nexus"]);
    const started = vi.spyOn(spine(), "startDownload");
    const opened = vi.spyOn(window, "open");
    buttonIn(row, "Open on Nexus").click();
    await until(() => opened.mock.calls.length === 1, "the file's page");
    expect(opened.mock.calls[0]?.[0]).toBe(LANTERN_PAGE);
    expect(started).not.toHaveBeenCalled();
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
    const pair = await standingOf(3301);
    // the account's standings arrive a beat after the row (get_nexus_standings)
    await until(() => buttonIn(pair, "Abstain").getAttribute("aria-pressed") === "true", "the abstention read");
    expect(
      [buttonIn(pair, "Endorse"), buttonIn(pair, "Abstain")].map((b) => b.getAttribute("aria-pressed")),
    ).toStrictEqual(["false", "true"]);
    expect(textAt('[data-mod-files] [data-standing="3301"] .stand__line')).toBe(
      "You abstained from endorsing this mod.",
    );
    buttonIn(pair, "Endorse").click();
    await until(() => buttonIn(pair, "Endorse").getAttribute("aria-pressed") === "true", "the endorsement");
    expect(buttonIn(pair, "Abstain").getAttribute("aria-pressed")).toBe("false");
    expect(textAt('[data-mod-files] [data-standing="3301"] .stand__line')).toBe("You endorsed this mod.");

    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    await until(() => document.querySelector("[data-mod-files]") === null, "the far world again");
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

// ---------------------------------------------------------------------------
// The keyboard on the far world's own controls (R16.22, §6.3) and the index
// line (R16.16): a press never drops the focus to <body>, and the underline
// follows the reader down to the last section.
// ---------------------------------------------------------------------------

/** A tracked mod's Track toggle (pressed: "Tracked") on its TRACKED card. */
const trackedToggle = (name: string): HTMLButtonElement =>
  buttonIn(
    must(
      trackedCards().find((c) => cardName(c) === name),
      name,
    ),
    "Tracked",
  );

/** A layout box, where happy-dom lays nothing out. */
const rect = (top: number, height = 40): DOMRect => ({
  top,
  bottom: top + height,
  left: 0,
  right: 0,
  width: 0,
  height,
  x: 0,
  y: top,
  toJSON: () => ({}),
});

/** The index line's words that carry aria-current. */
const marked = (): string[] =>
  [...document.querySelectorAll(".far__index a[aria-current]")].map((a) => textOf(a).trim());

describe("The far world v0.3: the keyboard and the index line", () => {
  it("should keep the keyboard on Endorse while Nexus answers: busy, never disabled", async () => {
    await openFarWorld("key");
    const pair = await standingOf(3301);
    await until(() => buttonIn(pair, "Abstain").getAttribute("aria-pressed") === "true", "the abstention read");
    let answer: (() => void) | null = null;
    const real = spine().setEndorsement.bind(spine());
    vi.spyOn(spine(), "setEndorsement").mockImplementation(
      (modId, status) =>
        new Promise((resolve) => {
          answer = () => resolve(real(modId, status));
        }),
    );
    const endorse = buttonIn(pair, "Endorse");
    endorse.focus();
    endorse.click();
    await until(() => endorse.getAttribute("aria-busy") === "true", "Nexus answering");
    // aria-disabled, never disabled: a disabled button drops the keyboard to <body>
    expect([endorse.disabled, buttonIn(pair, "Abstain").disabled]).toStrictEqual([false, false]);
    expect(buttonIn(pair, "Abstain").getAttribute("aria-disabled")).toBe("true");
    expect(document.activeElement).toBe(endorse);
    must(answer as (() => void) | null, "the pending answer")();
    await until(() => endorse.getAttribute("aria-pressed") === "true", "the endorsement");
    expect(document.activeElement).toBe(endorse);
  });

  it("should hand the keyboard to the next tracked mod when an untrack takes the card away, and to the one before at the end", async () => {
    await openFarWorld("key");
    await until(() => trackedCards().length === 3, "the tracked mods");
    const ox = trackedToggle("Ox Cart Express");
    ox.focus();
    ox.click();
    await until(() => trackedCards().length === 2, "Ox Cart Express untracked");
    await until(() => document.activeElement === trackedToggle("Old Road Armor Pack"), "the next card's toggle");
    expect(document.activeElement?.getAttribute("aria-pressed")).toBe("true");

    trackedToggle("Old Road Armor Pack").click();
    await until(() => trackedCards().length === 1, "Old Road Armor Pack untracked");
    await until(() => document.activeElement === trackedToggle("Vernworth Wardrobe Collection"), "the card before");

    trackedToggle("Vernworth Wardrobe Collection").click();
    await until(() => trackedCards().length === 0, "nothing tracked");
    await until(() => document.activeElement === document.getElementById("far-h-tracked"), "the section's head");
    expect(document.activeElement?.getAttribute("tabindex")).toBe("-1");
  });

  it("should underline the last section once the scroll reaches its end, and a section a jump went to", async () => {
    await openFarWorld("key");
    await until(() => indexWords().includes("Tracked"), "the index line with Tracked");
    const scroller = must(document.querySelector<HTMLElement>("main.far"), "the far world's scroll");
    // the scroll stands at its end: the last two sections share the window
    Object.defineProperty(scroller, "scrollHeight", { configurable: true, value: 3000 });
    Object.defineProperty(scroller, "clientHeight", { configurable: true, value: 800 });
    scroller.scrollTop = 2200;
    vi.spyOn(scroller, "getBoundingClientRect").mockReturnValue(rect(0, 800));
    const heads: Record<string, number> = {
      "far-account": -2000,
      "far-updates": -1500,
      "far-unlinked": -900,
      "far-tracked": 300,
      "far-find": 560,
    };
    for (const [id, top] of Object.entries(heads)) {
      vi.spyOn(must(document.getElementById(id), id), "getBoundingClientRect").mockReturnValue(rect(top));
    }
    scroller.dispatchEvent(new Event("scroll"));
    await until(() => marked().join() === "Find", "Find marked at the end");

    // a jump to Tracked: its head cannot reach the top, yet it is the one asked for
    must(
      [...document.querySelectorAll<HTMLAnchorElement>(".far__index a")].find((a) => textOf(a).trim() === "Tracked"),
      "the Tracked link",
    ).click();
    scroller.dispatchEvent(new Event("scroll"));
    await until(() => marked().join() === "Tracked", "Tracked marked after the jump");
    await new Promise((resolve) => setTimeout(resolve, 30));
    expect(marked()).toStrictEqual(["Tracked"]);
  });
});

// ---------------------------------------------------------------------------
// An update row's door while its own download is on its way (Chaos D2). The
// spine answers a second start of a file already on its way with that same
// row, so a press would do nothing: the door is held, never hidden.
// aria-disabled, still in the tab order, its reason its description.
// ---------------------------------------------------------------------------

const DOWNLOADING_REASON = "Downloading: see DOWNLOADS";

/** The text an element's aria-describedby points at. */
const describedBy = (el: Element): string =>
  (el.getAttribute("aria-describedby") ?? "")
    .split(/\s+/)
    .filter(Boolean)
    .map((id) => textOf(document.getElementById(id) ?? document.body).trim())
    .join(" ");

const doorLabel = (): string => textOf(must(updateDoor()?.querySelector(".rs-btn__label"), "door label")).trim();

/** The real timers' turn: whatever a press set going has had its chance to land. */
const aMoment = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 40));

/** Show this session's downloads as `rows`, the way a rift://downloads-changed read would. */
async function listDownloads(rows: DownloadView[]): Promise<void> {
  vi.spyOn(spine(), "getDownloads").mockResolvedValue(rows);
  await useDownloads().load();
}

describe("The far world v0.3: an update row's door while its download is on its way", () => {
  it.each<DownloadPhase>(["queued", "linking", "downloading", "verifying"])(
    "should hold Download while the row's own download is %s: reachable, its reason said, a press asking nothing",
    async (phase) => {
      await openFarWorld("premium");
      await until(() => updateDoor() !== null, "the update row's Download");
      await listDownloads([downloadRow({ phase })]);
      await until(() => updateDoor()?.getAttribute("aria-disabled") === "true", "the door held");
      const door = must(updateDoor(), "the held door");
      expect(doorLabel()).toBe("Download v2.2");
      // held, never disabled: the keyboard reaches it and hears why
      expect([door.disabled, door.title, describedBy(door)]).toStrictEqual([
        false,
        DOWNLOADING_REASON,
        DOWNLOADING_REASON,
      ]);
      const started = vi.spyOn(spine(), "startDownload");
      door.focus();
      door.click();
      await aMoment();
      expect(started).not.toHaveBeenCalled();
      expect(document.activeElement).toBe(door);
    },
  );

  it("should hold Get it on Nexus while a free account's download waits for the site's link, and free it once that row is cancelled", async () => {
    await openFarWorld("key");
    await until(() => updateDoor() !== null, "the update row's door");
    updateDoor()?.click();
    await until(() => shelf().at(0)?.dataset.phase === "waiting", "the waiting row");
    await until(() => updateDoor()?.getAttribute("aria-disabled") === "true", "the door held");
    expect(doorLabel()).toBe("Get it on Nexus");
    expect(must(updateDoor(), "door").title).toBe(DOWNLOADING_REASON);
    const started = vi.spyOn(spine(), "startDownload");
    updateDoor()?.click();
    await aMoment();
    expect(started).not.toHaveBeenCalled();

    must(
      shelf().at(0)?.querySelector<HTMLButtonElement>('[aria-label="Cancel the download of Lantern Glow Recolor"]'),
      "Cancel",
    ).click();
    await until(() => shelf().at(0)?.dataset.phase === "cancelled", "the waiting row cancelled");
    // a cancelled download is over: the door is free again, and a press asks once more
    await until(() => updateDoor()?.getAttribute("aria-disabled") === null, "the door free again");
    expect(must(updateDoor(), "door").title).toBe("");
    updateDoor()?.click();
    await until(() => started.mock.calls.length === 1, "the file asked for again");
    expect(started.mock.calls).toStrictEqual([[412, 2391, lanternId]]);
  });

  it("should hold the door for a download of the same mod and file that started elsewhere, a mod's files say", async () => {
    await openFarWorld("premium");
    await until(() => updateDoor() !== null, "the update row's Download");
    await listDownloads([downloadRow({ forPackageId: null, phase: "downloading" })]);
    await until(() => updateDoor()?.getAttribute("aria-disabled") === "true", "the door held");
    expect(must(updateDoor(), "door").title).toBe(DOWNLOADING_REASON);
  });

  it.each<DownloadPhase>(["failed", "cancelled"])(
    "should leave the door free over a %s download of its file: a second try may land",
    async (phase) => {
      await openFarWorld("premium");
      await until(() => updateDoor() !== null, "the update row's Download");
      const failure =
        phase === "failed" ? { kind: "nexus" as const, detail: { detail: "Nexus answered oddly." } } : null;
      await listDownloads([downloadRow({ phase, failure, finishedMs: Date.now() })]);
      await until(() => shelf().at(0)?.dataset.phase === phase, `the ${phase} row`);
      const door = must(updateDoor(), "the free door");
      expect(door.getAttribute("aria-disabled")).toBeNull();
      const started = vi.spyOn(spine(), "startDownload");
      door.click();
      await until(() => started.mock.calls.length === 1, "the file asked for again");
    },
  );
});

// ---------------------------------------------------------------------------
// A free account's start (Chaos D1, the toast's half): the file's page opens,
// and the toast says what the site's button will do. It reaches the stone only
// while the stone answers nxm:// links; until then the toast says so and
// carries the switch's own act.
// ---------------------------------------------------------------------------

const PAGE_OPEN_LINE =
  "The file's Nexus page is open: press Mod Manager Download there, and the stone takes the file from that link.";
const PAGE_OPEN_SILENT_LINE =
  "The file's Nexus page is open, but nothing hands Nexus's button to the stone now. Let the stone answer it first, then press Mod Manager Download there.";

const toastOf = (line: string) => useToasts().toasts.value.find((t) => t.line === line);

describe("The far world v0.3: a free account's start, by who answers Nexus's button", () => {
  it("should say the stone must answer Nexus's button first while nothing hands it the link, and turn the switch on from the toast", async () => {
    await openFarWorld("key");
    await until(() => updateDoor() !== null, "the update row's door");
    await useNxmHandler().load();
    expect(useNxmHandler().handler.value?.state).toBe("none");
    updateDoor()?.click();
    await until(() => toastOf(PAGE_OPEN_SILENT_LINE) !== undefined, "the start's toast");
    expect(toastLines()).not.toContain(PAGE_OPEN_LINE);
    const toast = must(toastOf(PAGE_OPEN_SILENT_LINE), "toast");
    expect([toast.tone, toast.actions.map((a) => a.label)]).toStrictEqual(["rift", ["Answer Nexus's button"]]);

    const switched = vi.spyOn(spine(), "setNxmHandler");
    await useToasts().act(toast, must(toast.actions.at(0), "the switch's act"));
    expect(switched.mock.calls).toStrictEqual([[true]]);
    expect(useNxmHandler().answering.value).toBe(true);
  });

  it("should say the same while another program answers Nexus's button", async () => {
    await openFarWorld("key,nxmother");
    await until(() => updateDoor() !== null, "the update row's door");
    await useNxmHandler().load();
    expect(useNxmHandler().handler.value?.state).toBe("other");
    updateDoor()?.click();
    await until(() => toastOf(PAGE_OPEN_SILENT_LINE) !== undefined, "the start's toast");
    expect(must(toastOf(PAGE_OPEN_SILENT_LINE), "toast").actions.map((a) => a.label)).toStrictEqual([
      "Answer Nexus's button",
    ]);
  });

  it("should keep the plain line while the stone answers Nexus's button", async () => {
    await openFarWorld("key");
    await until(() => updateDoor() !== null, "the update row's door");
    await useNxmHandler().set(true);
    expect(useNxmHandler().answering.value).toBe(true);
    updateDoor()?.click();
    await until(() => toastOf(PAGE_OPEN_LINE) !== undefined, "the start's toast");
    expect(toastLines()).not.toContain(PAGE_OPEN_SILENT_LINE);
    expect(must(toastOf(PAGE_OPEN_LINE), "toast").actions).toStrictEqual([]);
  });

  it("should read who answers before it speaks, when nothing read it yet", async () => {
    await openFarWorld("key");
    await until(() => updateDoor() !== null, "the update row's door");
    // the switch's own first read lands first; then the handler is as an unread
    // one: the start reads it before choosing its line
    await until(() => useNxmHandler().handler.value !== null, "the switch's first read");
    useNxmHandler().handler.value = null;
    const read = vi.spyOn(spine(), "getNxmHandler");
    updateDoor()?.click();
    await until(() => toastOf(PAGE_OPEN_SILENT_LINE) !== undefined, "the start's toast");
    expect(read).toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// A download that is no archive (a raw .pak, an .exe) lands in the mods folder
// under its own name with no package: the scan never lists it. Its toast, its
// shelf row and its update row say so, and lead to the file in Explorer.
// ---------------------------------------------------------------------------

const RAW_NAME = "Lantern Glow Recolor 2.2.pak";
const RAW_PATH = `${MOCK_MODS_DIR}\\${RAW_NAME}`;
const RAW_LINE = "Lantern Glow Recolor is in your mods folder, but it is no archive, so the Rift does not list it.";

/** The update row's download, seen downloading, then landed as a raw .pak: no package. */
async function landRaw(): Promise<void> {
  await openFarWorld("premium");
  await until(() => updateDoor() !== null, "the update row's Download");
  await listDownloads([downloadRow({ phase: "downloading" })]);
  await listDownloads([
    downloadRow({
      phase: "landed",
      bytesDone: 12_845_056,
      landedAs: RAW_NAME,
      packageId: null,
      finishedMs: Date.now(),
    }),
  ]);
  await until(() => shelf().at(0)?.dataset.phase === "landed", "the landed row");
}

describe("The far world v0.3: a download that is no archive", () => {
  it("should toast that it is in the mods folder but no archive, and show it in its folder instead of the Rift", async () => {
    await landRaw();
    const toast = must(toastOf(RAW_LINE), "the landing toast");
    expect([toast.tone, toast.actions.map((a) => a.label)]).toStrictEqual(["rift", ["Show in folder"]]);
    expect(toastLines().some((l) => l.includes("Install it from the Rift"))).toBe(false);
    const revealed = vi.spyOn(spine(), "revealPath");
    await useToasts().act(toast, must(toast.actions.at(0), "Show in folder"));
    expect(revealed.mock.calls).toStrictEqual([[RAW_PATH]]);
  });

  it("should say so on the update row and on the shelf, each with Show in folder, never Show in the Rift", async () => {
    await landRaw();
    await until(() => document.querySelector(".far__landed") !== null, "the update row's landed line");
    const landedLine = must(document.querySelector<HTMLElement>(".far__landed"), "landed line");
    expect(textOf(landedLine).replace(/\s+/g, " ").trim()).toBe(
      "v2.2 is in your mods folder, but it is no archive, so the Rift does not list it. Show in folder",
    );
    const row = must(shelf().at(0), "the landed row");
    expect(phaseWord(row)).toBe("In your mods folder");
    expect(textOf(must(row.querySelector(".shelf__gone"), "the row's line")).trim()).toBe(
      "It is no archive, so the Rift does not list it.",
    );
    expect(labelsIn(row)).toStrictEqual(["Show in folder"]);
    expect(hasButton("Show in the Rift")).toBe(false);

    const revealed = vi.spyOn(spine(), "revealPath");
    buttonIn(landedLine, "Show in folder").click();
    await until(() => revealed.mock.calls.length === 1, "the file shown in its folder");
    buttonIn(row, "Show in folder").click();
    await until(() => revealed.mock.calls.length === 2, "the file shown again from the shelf");
    expect(revealed.mock.calls).toStrictEqual([[RAW_PATH], [RAW_PATH]]);
  });
});

// ---------------------------------------------------------------------------
// §9 Nexus link received / refused: the toast that says what became of an
// arriving link. A link taken carries its carved line, THE FAR WORLD ANSWERS,
// and leads to the DOWNLOADS shelf; a refusal leads to the file's own page
// (where the site's button makes a fresh link), and to PREFLIGHT when the key
// is the way out.
// ---------------------------------------------------------------------------

const EXPIRED_LINE =
  "That Mod Manager Download link has expired. Nothing was downloaded. Press Mod Manager Download on the file's Nexus page again for a fresh one.";
const NO_KEY_LINE =
  "The stone needs your Nexus key before it can take a download. Nothing was downloaded. Update checks, browsing and patch notes work without one.";

describe("The far world v0.3: what the stone says of an arriving link", () => {
  it("should carve THE FAR WORLD ANSWERS over a link taken, and lead to the DOWNLOADS shelf", async () => {
    await openFarWorld("key");
    await until(() => keyChip() === "CONNECTED", "the connected account");
    useNavigation().go("rift");
    spine().deliverNxm(
      `nxm://dragonsdogma2/mods/412/files/2391?key=invented&expires=${later()}&user_id=${MOCK_USER_ID}`,
    );
    await until(() => toastOf(TAKEN_LINE) !== undefined, "the link taken");
    const toast = must(toastOf(TAKEN_LINE), "toast");
    expect([toast.tone, toast.inscription, toast.actions.map((a) => a.label)]).toStrictEqual([
      "rift",
      "THE FAR WORLD ANSWERS",
      ["Show downloads"],
    ]);
    // the shelf of toasts carves that line above the sentence
    await until(() => document.querySelector(".toast__ins") !== null, "the carved line drawn");
    const drawn = must(
      [...document.querySelectorAll(".toast")].find((t) => textOf(t).includes(TAKEN_LINE)),
      "the drawn toast",
    );
    expect(textOf(must(drawn.querySelector(".toast__ins"), "carved line")).trim()).toBe("THE FAR WORLD ANSWERS");

    // Show downloads: THE FAR WORLD, scrolled to its DOWNLOADS shelf
    useNavigation().go("rift");
    const scrolled = vi.spyOn(Element.prototype, "scrollIntoView");
    await useToasts().act(toast, must(toast.actions.at(0), "Show downloads"));
    expect(useNavigation().page.value).toBe("nexus");
    expect(scrolled.mock.contexts.map((el) => (el as Element).id)).toContain("far-downloads");
  });

  it("should lead a refused link to the file's own page, where the site's button makes a fresh one", async () => {
    await openFarWorld("key");
    await until(() => keyChip() === "CONNECTED", "the connected account");
    spine().deliverNxm("nxm://dragonsdogma2/mods/412/files/2391?key=invented&expires=1000");
    await until(() => toastOf(EXPIRED_LINE) !== undefined, "the refusal");
    const toast = must(toastOf(EXPIRED_LINE), "toast");
    expect([toast.tone, toast.inscription, toast.actions.map((a) => a.label)]).toStrictEqual([
      "ember",
      null,
      ["Open on Nexus"],
    ]);
    const opened = vi.spyOn(window, "open");
    await useToasts().act(toast, must(toast.actions.at(0), "Open on Nexus"));
    expect(opened.mock.calls).toStrictEqual([[LANTERN_PAGE, "_blank", "noopener"]]);
  });

  it("should offer Open Preflight beside the file's page when the stone holds no key", async () => {
    await openFarWorld();
    await until(() => text().includes("THE FAR WORLD IS SILENT"), "the keyless account");
    spine().deliverNxm(`nxm://dragonsdogma2/mods/412/files/2391?key=invented&expires=${later()}`);
    await until(() => toastOf(NO_KEY_LINE) !== undefined, "the refusal");
    const toast = must(toastOf(NO_KEY_LINE), "toast");
    expect(toast.actions.map((a) => a.label)).toStrictEqual(["Open on Nexus", "Open Preflight"]);
    await useToasts().act(toast, must(toast.actions.at(1), "Open Preflight"));
    expect(useNavigation().page.value).toBe("preflight");
  });

  it("should give a link that names no file of this game nothing to open, and no key to fix", async () => {
    await openFarWorld("key");
    await until(() => keyChip() === "CONNECTED", "the connected account");
    spine().deliverNxm(`nxm://otherworld/mods/5/files/6?key=invented&expires=${later()}`);
    const line =
      "That Nexus Mods link is for another game. The Riftstone handles Dragon's Dogma 2 mods only. Nothing was downloaded, and no setting changed.";
    await until(() => toastOf(line) !== undefined, "the refusal");
    expect(must(toastOf(line), "toast").actions).toStrictEqual([]);
  });
});
