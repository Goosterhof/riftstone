// v0.5 — THE CROSSING over versions of one file (#00015 §18.4, §18.6, R18.5,
// R18.6, R18.9, R18.10, with §18.17's rulings): an install that takes another
// version's place says so above its files, lands with no Undo, and tells the
// put-away once, in the plan, with `Bring it back`; `Put it away` on THE
// VERSIONS card is one cyan toast; `Keep only the newest…` is THE RETURN —
// OLDER VERSIONS, Danger, said once. Every name is the mock's invented one.

import { afterEach, describe, expect, it, vi } from "vitest";
import type { LibraryPackage } from "@/types/rift";
import { useLibrary } from "@/composables/useLibrary";
import { useToasts } from "@/composables/useToasts";
import { useUndercroft } from "@/composables/useUndercroft";
import { useVersions } from "@/composables/useVersions";
import { useWorld } from "@/composables/useWorld";
import { CANDLE_NAME, FAMILY_NAME, MAIN_NAME, familyFileName } from "@/services/mock/versions";
import { button, hasButton, idle, sleep, spine, text, textAt, textOf, until, wakeOn, type Scene } from "./harness";

afterEach(() => {
  vi.restoreAllMocks();
  sleep();
});

function must<T>(found: T | null | undefined, what: string): T {
  if (found === null || found === undefined) throw new Error(`The stone drew no ${what}`);
  return found;
}

const member = (version: "1.3" | "1.2" | "1.1"): LibraryPackage =>
  must(
    useLibrary().packages.value.find((p) => p.package.fileName === familyFileName(version)),
    `package v${version}`,
  );

const optionOf = (pkg: LibraryPackage, name: string): string =>
  must(
    pkg.tree.find((n) => n.label === name),
    `${name} in ${pkg.package.fileName}`,
  ).optionId ?? "";

const replaces = (): HTMLElement | null => document.querySelector<HTMLElement>(".plan__replaces");
const steps = (): string[] =>
  [...(replaces()?.querySelectorAll<HTMLElement>(".plan__step") ?? [])].map((p) => textOf(p).trim());
const awayLines = (): string[] =>
  [...document.querySelectorAll<HTMLElement>(".plan__away-line")].map((p) => textOf(p).replace(/\s+/g, " ").trim());
const toastLines = (): string[] => useToasts().toasts.value.map((t) => t.line);
const card = (): HTMLElement | null => document.querySelector<HTMLElement>(".vc");

const planRows = (): string[] =>
  [...(card()?.querySelectorAll<HTMLElement>(".vc__planrow") ?? [])].map((r) =>
    [...r.children].map((c) => textOf(c).trim()).join(" "),
  );

async function wakeFamily(scene: Scene): Promise<void> {
  await wakeOn(scene);
  await until(() => useLibrary().packages.value.some((p) => p.family?.older.length), "the family in the library");
}

/** Opens an option in Your World and its plan, by the door its detail carries. */
async function planOf(optionId: string, door: string | RegExp = /^Install( v[\d.]+)?…$/): Promise<void> {
  await useWorld().open(optionId);
  await until(() => document.querySelector(".odetail") !== null, "the option's detail");
  button(door).click();
  await until(() => hasButton("Install ▶") && idle(button("Install ▶")), "THE CROSSING's plan");
}

async function install(optionId: string): Promise<void> {
  await planOf(optionId);
  button("Install ▶").click();
  await until(() => textAt(".plan__landed .plan__carved") === "IN YOUR WORLD", "the plan's landing", 6000);
}

describe("THE CROSSING, TAKES THE PLACE OF (R18.9)", () => {
  it("should name what v1.3's install replaces, the step in words, and the put-away to come", async () => {
    await wakeFamily("familyfresh");
    await planOf(optionOf(member("1.3"), MAIN_NAME));
    const block = must(replaces(), "the replace block");
    expect(textOf(must(block.querySelector(".plan__subhead"), "head")).trim()).toBe("TAKES THE PLACE OF");
    const rows = [...block.querySelectorAll<HTMLElement>(".plan__replace")];
    expect(rows).toHaveLength(1);
    const row = must(rows[0], "the replace row");
    expect(textOf(must(row.querySelector(".plan__replace-glyph"), "glyph")).trim()).toBe("↺");
    expect(row.querySelector(".plan__replace-glyph")?.getAttribute("aria-hidden")).toBe("true");
    expect(textOf(must(row.querySelector(".plan__replace-name"), "name")).trim()).toBe(MAIN_NAME);
    expect(textOf(must(row.querySelector(".plan__replace-ver"), "version")).trim()).toBe("v1.2");
    expect(textOf(must(row.querySelector(".plan__replace-count"), "count")).trim()).toBe(
      "3 files removed or handed back",
    );
    expect(steps()).toStrictEqual([
      "In one step: v1.2's files leave first, then v1.3's are placed. If anything fails, both are put back.",
      "After the install, an older version with nothing left installed is put away in mods (older versions).",
    ]);
    // above the files, muted, never ember: a replace is not a conflict
    const plan = must(document.querySelector(".plan"), "the plan");
    const order = [...plan.querySelectorAll<HTMLElement>(".plan__replaces, .plan__rows, .plan__files, ul, table")];
    expect(order[0]).toBe(block);
    expect(block.className).not.toMatch(/ember/);
    // Q18.4 (§18.17): a write onto a path v1.2 held reads reinstall, never an overwrite
    expect(text()).not.toMatch(/OVERWRITES/);
    // the plan never names which versions will go (Pattern 013)
    expect(textOf(block)).not.toContain(familyFileName("1.2"));
  });

  it("should carry the rollback line on an older version's install, and no put-away sentence when nothing older remains", async () => {
    await wakeFamily("family");
    await spine().uninstall(optionOf(member("1.2"), MAIN_NAME));
    await until(() => member("1.3").family?.older[0]?.installed === 0, "v1.2 uninstalled");
    await planOf(optionOf(member("1.2"), MAIN_NAME));
    const row = must(replaces()?.querySelector(".plan__replace"), "the replace row");
    expect(textOf(must(row.querySelector(".plan__replace-ver"), "version")).trim()).toBe("v1.3");
    expect(steps()).toStrictEqual([
      "You are installing an older version: v1.3 leaves your game, and stays in your mods folder.",
      "In one step: v1.3's files leave first, then v1.2's are placed. If anything fails, both are put back.",
    ]);
    expect(text()).not.toContain("is put away in mods (older versions)");
  });

  it("should keep the put-away sentence on a rollback that leaves an older member without installs (§18.17)", async () => {
    await wakeFamily("familythree");
    // v1.3 in your world, v1.2 not: installing v1.2 again is a rollback, and v1.1 goes
    await spine().install(optionOf(member("1.3"), MAIN_NAME));
    await until(
      () => !useLibrary().packages.value.some((p) => p.package.fileName === familyFileName("1.2")),
      "v1.2 away",
    );
    // bring both back: the mods folder holds the three versions again, only v1.3 installed
    for (const item of (await spine().getOlderVersions()).items) await spine().bringBack(item.fileName);
    await until(() => (member("1.3").family?.older.length ?? 0) === 2, "the three versions again");
    await planOf(optionOf(member("1.2"), MAIN_NAME));
    expect(steps()).toStrictEqual([
      "You are installing an older version: v1.3 leaves your game, and stays in your mods folder.",
      "In one step: v1.3's files leave first, then v1.2's are placed. If anything fails, both are put back.",
      "After the install, an older version with nothing left installed is put away in mods (older versions).",
    ]);
  });

  it("should show no replace block for an install that replaces nothing", async () => {
    await wakeFamily("family");
    await planOf(optionOf(member("1.2"), CANDLE_NAME));
    expect(replaces()).toBeNull();
  });
});

describe("THE CROSSING, landed (R18.5, R18.6)", () => {
  it("should tell v1.2's put-away once, in the plan, with `Bring it back`, and the replace's toast with no Undo", async () => {
    await wakeFamily("putaway");
    await install(optionOf(member("1.3"), MAIN_NAME));
    await until(() => awayLines().length === 1, "the put-away line");
    expect(awayLines()).toStrictEqual(["↧ v1.2 is put away in mods (older versions), beside your mods folder."]);
    expect(textOf(must(document.querySelector(".plan__away-line [aria-hidden='true']"), "glyph")).trim()).toBe("↧");
    const back = button("Bring it back");
    expect(back.className).toContain("rs-btn--secondary");
    expect(back.className).toContain("rs-btn--sm");
    // R18.6: the replace lands with Show in load order and no Undo; the put-away is not in the toast
    const landed = useToasts().toasts.value.find((t) => t.line.includes("in v1.2's place"));
    expect(landed?.line).toBe(`${FAMILY_NAME} v1.3 is in your world, in v1.2's place.`);
    expect(landed?.tone).toBe("world");
    expect(landed?.actions.map((a) => a.label)).toStrictEqual(["Show in load order"]);
    expect(toastLines().some((l) => l.includes("put away"))).toBe(false);
    expect(text().split("is put away in").length - 1).toBe(1);
    // the version row is simply not there at the next render; the undercroft carves it
    // a version alone in the mods folder is in no family (WINDOW-CONTRACT § v0.5, One entry)
    await until(() => member("1.3").family === null, "v1.2 gone from the family");
    await until(() => useUndercroft().view.value?.items.length === 1, "the undercroft read again");

    back.click();
    await until(() => awayLines()[0] === "v1.2 is back in your mods folder.", "the line brought back");
    expect(hasButton("Bring it back")).toBe(false);
    await until(() => (member("1.3").family?.older.length ?? 0) === 1, "v1.2 folded under the lead again");
    await until(() => useUndercroft().view.value?.items.length === 0, "the undercroft emptied");
  });

  it("should name the taken-name landing", async () => {
    await wakeFamily("putawaytaken");
    await install(optionOf(member("1.3"), MAIN_NAME));
    await until(() => awayLines().length === 1, "the put-away line");
    const taken = familyFileName("1.2").replace(/\.zip$/, " (2).zip");
    expect(awayLines()).toStrictEqual([
      `↧ v1.2 is put away in mods (older versions), beside your mods folder, as ${taken} (a file of that name was already there).`,
    ]);
  });

  it("should draw a failed put-away as the warn line with `Try again`, the install standing", async () => {
    await wakeFamily("putawayfail");
    await install(optionOf(member("1.3"), MAIN_NAME));
    await until(() => document.querySelector(".plan__away-warn") !== null, "the warn line");
    const warn = must(document.querySelector<HTMLElement>(".plan__away-warn"), "warn line");
    expect(warn.classList.contains("plan__warn")).toBe(true);
    const said = (): string =>
      textOf(must(warn.querySelector("span"), "the warn sentence"))
        .replace(/\s+/g, " ")
        .trim();
    await until(() => said().includes("v1.2 stays"), "the warn line naming v1.2");
    expect(said()).toBe(
      `The stone couldn't move ${familyFileName("1.2")} to mods (older versions): access is denied. The install stands; v1.2 stays in your mods folder.`,
    );
    const again = button("Try again");
    expect(again.className).toContain("rs-btn--secondary");
    expect(again.className).toContain("rs-btn--sm");
    // the install stands; the entry keeps showing the older version
    expect(member("1.3").installed).toBe(1);
    expect(member("1.3").family?.older).toHaveLength(1);
    // Try again asks the spine to make the install's put-away again; the window works out no set
    const retry = vi.spyOn(spine(), "retryPutAway");
    const one = vi.spyOn(spine(), "putAway");
    again.click();
    await until(() => awayLines().length === 1, "the put-away line after Try again");
    expect(retry).toHaveBeenCalledExactlyOnceWith(member("1.3").package.id);
    expect(one).not.toHaveBeenCalled();
    expect(document.querySelector(".plan__away-warn")).toBeNull();
    expect(awayLines()[0]).toBe("↧ v1.2 is put away in mods (older versions), beside your mods folder.");
  });

  it("should keep a move that worked when Try again moves v1.2 and v1.1 still stays (General, PR #2)", async () => {
    await wakeFamily("putawayhalf");
    await install(optionOf(member("1.3"), MAIN_NAME));
    await until(() => document.querySelector(".plan__away-warn") !== null, "the warn line");
    const shelf = vi.spyOn(spine(), "getOlderVersions");
    button("Try again").click();
    await until(() => awayLines().length === 1, "v1.2's put-away line");
    expect(awayLines()[0]).toBe("↧ v1.2 is put away in mods (older versions), beside your mods folder.");
    expect(hasButton("Bring it back")).toBe(true);
    // what would not move is still said, beside the move that worked
    const warn = must(document.querySelector<HTMLElement>(".plan__away-warn span"), "the warn sentence");
    expect(textOf(warn).replace(/\s+/g, " ")).toContain(
      `The stone couldn't move ${familyFileName("1.1")} to mods (older versions): access is denied.`,
    );
    // THE UNDERCROFT reads the folder again for the move that worked
    await until(() => shelf.mock.calls.length > 0, "the undercroft's read");
  });

  it("should keep §9's toast and its Undo for an install that replaced nothing", async () => {
    await wakeFamily("family");
    await install(optionOf(member("1.2"), CANDLE_NAME));
    const landed = useToasts().toasts.value.find((t) => t.line.startsWith(`${CANDLE_NAME} is in your world.`));
    expect(landed?.actions.map((a) => a.label)).toStrictEqual(["Undo", "Show in load order"]);
    expect(awayLines()).toStrictEqual([]);
  });
});

describe("THE VERSIONS card, `Put it away` (R18.5)", () => {
  it("should move v1.1 in one press, said once in a cyan toast with `Bring it back`", async () => {
    await wakeFamily("familythree");
    useVersions().open(member("1.3").package.id);
    await until(() => card()?.querySelectorAll(".vc__row").length === 3, "three rows");
    const away = button("Put it away");
    expect(away.className).toContain("rs-btn--secondary");
    expect(away.className).toContain("rs-btn--sm");
    away.click();
    await until(() => toastLines().some((l) => l.includes("is put away")), "the put-away toast");
    const toast = useToasts().toasts.value.find((t) => t.line.includes("is put away"));
    expect(toast?.line).toBe(
      `v1.1 of ${FAMILY_NAME} is put away in mods (older versions). Nothing in your game changed.`,
    );
    expect(toast?.tone).toBe("rift");
    expect(toast?.actions.map((a) => a.label)).toStrictEqual(["Bring it back"]);
    await until(() => card()?.querySelectorAll(".vc__row").length === 2, "the row simply not there");
    expect(hasButton("Put it away")).toBe(false);
    await until(() => useUndercroft().view.value?.items.length === 1, "the undercroft read again");

    await toast?.actions[0]?.run();
    await until(() => card()?.querySelectorAll(".vc__row").length === 3, "v1.1 back in the family");
    const back = useToasts().toasts.value.find((t) => t.line.includes("is back"));
    expect(back?.line).toBe(`v1.1 of ${FAMILY_NAME} is back in your mods folder. It sits under v1.3 in the Rift.`);
    expect(back?.tone).toBe("rift");
  });

  it("should voice a refused put-away in ember, the spine's sentence verbatim, `Nothing was moved.`", async () => {
    await wakeFamily("familythree");
    useVersions().open(member("1.3").package.id);
    await until(() => hasButton("Put it away"), "Put it away");
    const { RiftFailure } = await import("@/services/rift");
    vi.spyOn(spine(), "putAway").mockRejectedValue(
      new RiftFailure({ kind: "refused", detail: { reason: "The game is holding that archive open." } }),
    );
    button("Put it away").click();
    await until(() => useToasts().toasts.value.some((t) => t.tone === "ember"), "the ember toast");
    const toast = useToasts().toasts.value.find((t) => t.tone === "ember");
    expect(toast?.line).toBe("The game is holding that archive open. Nothing was moved.");
    expect(toast?.actions).toStrictEqual([]);
  });
});

describe("THE VERSIONS card, a put-away that fails (§18.11)", () => {
  it("should voice an io failure in ember, naming the file only, with `Try again` and `Copy details`", async () => {
    await wakeFamily("familythree");
    useVersions().open(member("1.3").package.id);
    await until(() => hasButton("Put it away"), "Put it away");
    const { RiftFailure } = await import("@/services/rift");
    vi.spyOn(spine(), "putAway").mockRejectedValueOnce(
      new RiftFailure({ kind: "io", detail: { reason: "access is denied" } }),
    );
    button("Put it away").click();
    await until(() => useToasts().toasts.value.some((t) => t.tone === "ember"), "the ember toast");
    const toast = useToasts().toasts.value.find((t) => t.tone === "ember");
    expect(toast?.line).toBe(
      `The stone couldn't move ${familyFileName("1.1")} to mods (older versions): access is denied. It is still in your mods folder, exactly as it was.`,
    );
    expect(toast?.actions.map((a) => a.label)).toStrictEqual(["Try again", "Copy details"]);
    await toast?.actions[0]?.run();
    await until(() => toastLines().some((l) => l.startsWith("v1.1 of")), "Try again puts it away");
  });
});

describe("THE RETURN — OLDER VERSIONS (R18.10)", () => {
  it("should plan one ↺ and one ↧ on the stacked family, confirm in Danger, and leave the rail gold", async () => {
    await wakeFamily("family");
    const leadId = member("1.3").package.id;
    useVersions().open(leadId);
    await until(() => hasButton("Keep only the newest…"), "the card's door");
    button("Keep only the newest…").click();
    await until(() => hasButton("Keep only v1.3"), "the keep-newest plan");
    expect(textOf(must(card()?.querySelector(".vc__title"), "head")).trim()).toBe("THE RETURN — OLDER VERSIONS");
    expect(text()).toContain("Keeps v1.3, the newest version with an install.");
    expect(planRows()).toStrictEqual([
      `↺ ${MAIN_NAME} v1.2 3 files removed or handed back`,
      `↧ ${familyFileName("1.2")} put away in mods (older versions)`,
    ]);
    const confirm = button("Keep only v1.3");
    expect(confirm.className).toContain("rs-btn--danger");
    expect(confirm.className).not.toMatch(/world/);
    expect(hasButton("Cancel")).toBe(true);

    confirm.click();
    await until(() => text().includes("Removing 3 files…"), "in progress");
    expect(button("Keep only v1.3").getAttribute("aria-busy")).toBe("true");
    await until(() => text().includes("RETURNED TO THE RIFT"), "done", 6000);
    expect(text()).toContain("1 install of older versions removed.");
    const lines = [...(card()?.querySelectorAll<HTMLElement>(".vc__away-line p") ?? [])].map((p) =>
      textOf(p).replace(/\s+/g, " ").trim(),
    );
    expect(lines).toStrictEqual(["↧ v1.2 is put away in mods (older versions), beside your mods folder."]);
    expect(hasButton("Bring it back")).toBe(true);
    expect(hasButton("Done")).toBe(true);
    const toasts = useToasts().toasts.value.filter((t) => t.line.startsWith("Only "));
    expect(toasts.map((t) => t.line)).toStrictEqual([`Only v1.3 of ${FAMILY_NAME} is in your world now.`]);
    expect(toasts[0]?.actions).toStrictEqual([]);

    // the investor's moment: the warn rail goes out, the mark leaves, the undercroft carves 1
    await until(() => member("1.3").family === null, "the older version gone");
    const row = must(
      document.querySelector<HTMLElement>(`#rift-tree [data-key="${CSS.escape(leadId)}"] > .ltr__row`),
      "lead row",
    );
    await until(() => row.classList.contains("rail--world"), "the gold rail");
    expect(row.classList.contains("rail--warn")).toBe(false);
    expect(row.querySelector(".ltr__older")).toBeNull();
    await until(() => textAt("#rift-undercroft-count") === "1 PUT AWAY", "the undercroft's count");
  });

  it("should hold the confirm back on a refused plan, the spine's sentence in ember under the heading", async () => {
    await wakeFamily("familyfresh");
    useVersions().open(member("1.3").package.id);
    await useVersions().beginKeepNewest();
    await until(() => text().includes("THE RETURN — OLDER VERSIONS"), "the keep-newest plan");
    const refusal = must(card()?.querySelector<HTMLElement>(".vc__note.is-ember"), "the refusal");
    expect(refusal.getAttribute("role")).toBe("alert");
    expect(textOf(refusal).trim()).toBe(
      `No version of ${FAMILY_NAME} is older than the one kept, "${familyFileName("1.2")}".`,
    );
    const confirm = button(/^Keep only/);
    expect(confirm.getAttribute("aria-disabled")).toBe("true");
    const keep = vi.spyOn(spine(), "keepNewest");
    confirm.click();
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(keep).not.toHaveBeenCalled();
    button("Cancel").click();
    await until(
      () => textOf(card()?.querySelector(".vc__title") ?? document.body).trim() === "VERSIONS OF THIS FILE",
      "back to the card",
    );
  });
});

const unset = (): void => undefined;

/** A spine call held at the gate until the spec lets it answer. */
function held(): { gate: Promise<void>; open: () => void } {
  let open = unset;
  const gate = new Promise<void>((resolve) => {
    open = resolve;
  });
  return { gate, open };
}

/** Any package in no family: the card the player opens next. */
const stranger = (): LibraryPackage =>
  must(
    useLibrary().packages.value.find((p) => !p.family),
    "a package in no family",
  );

describe("THE RETURN — OLDER VERSIONS, a card the player left (General, PR #2)", () => {
  it("should drop a keep-newest plan that lands after its card closed and another opened", async () => {
    await wakeFamily("family");
    const versions = useVersions();
    const plan = spine().planKeepNewest.bind(spine());
    const hold = held();
    vi.spyOn(spine(), "planKeepNewest").mockImplementationOnce(async (id) => {
      await hold.gate;
      return plan(id);
    });
    versions.open(member("1.3").package.id);
    const pressed = versions.beginKeepNewest();
    versions.close();
    const other = stranger().package.id;
    versions.open(other);
    hold.open();
    await pressed;
    expect(versions.leadId.value).toBe(other);
    expect(versions.stage.value).toBe("card");
    expect(versions.keepPlan.value).toBeNull();
  });

  it("should tell a keep-newest that lands after its card closed in a toast, and draw nothing on the card open now", async () => {
    await wakeFamily("family");
    const versions = useVersions();
    versions.open(member("1.3").package.id);
    await versions.beginKeepNewest();
    expect(versions.stage.value).toBe("keepPlan");
    const keep = spine().keepNewest.bind(spine());
    const hold = held();
    vi.spyOn(spine(), "keepNewest").mockImplementationOnce(async (id) => {
      await hold.gate;
      return keep(id);
    });
    const confirmed = versions.confirmKeepNewest();
    expect(versions.stage.value).toBe("keeping");
    versions.close();
    const other = stranger().package.id;
    versions.open(other);
    hold.open();
    await confirmed;
    expect(versions.leadId.value).toBe(other);
    expect(versions.stage.value).toBe("card");
    expect(versions.kept.value).toBeNull();
    expect(toastLines().some((t) => t.includes(FAMILY_NAME))).toBe(true);
  });

  it("should tell a keep-newest that fails after its card closed in ember, never on the card open now", async () => {
    await wakeFamily("family");
    const versions = useVersions();
    versions.open(member("1.3").package.id);
    await versions.beginKeepNewest();
    const hold = held();
    vi.spyOn(spine(), "keepNewest").mockImplementationOnce(async () => {
      await hold.gate;
      throw new Error("Access is denied.");
    });
    const confirmed = versions.confirmKeepNewest();
    versions.close();
    hold.open();
    await confirmed;
    expect(versions.stage.value).toBe("card");
    expect(versions.failure.value).toBeNull();
    expect(useToasts().toasts.value.some((t) => t.tone === "ember")).toBe(true);
  });
});
