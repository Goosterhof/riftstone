// v0.2 — Find on Nexus (useFarWorld.find / findAll / linkCandidate). An
// unlinked row asks Nexus about its NAME and the candidates unfold under it;
// the player picks one, Link this links it and runs the check at once, so its
// labels arrive (and says honestly that a link by mod id pins no file). Nothing
// found opens the Link… field as the way on; Find all walks every unlinked row
// one at a time and never links by itself. No focus move ever lands on a
// button that sends a name.

import { afterEach, describe, expect, it, vi } from "vitest";
import { MOCK_PACKAGES } from "@/services/mock/library";
import { RiftFailure } from "@/services/rift";
import { voice } from "@/services/voice";
import { useFarWorld } from "@/composables/useFarWorld";
import { useNavigation } from "@/composables/useNavigation";
import { useToasts } from "@/composables/useToasts";
import { button, hasButton, sleep, spine, text, textOf, until, wakeOn, type Scene } from "./harness";

const SLOW_TEST_MS = 20_000;
const SENDS = "(and, if that finds nothing, its first option's name)";

/** The five packages with options and no Nexus link, in the page's order. */
const UNLINKED = [
  "REFramework",
  "Ox Cart Express",
  "Brine Water Clarity",
  "Sacred Arbor Fog Removal",
  "Old Road Armor Pack",
];

const idOf = (name: string): string => {
  const pkg = MOCK_PACKAGES.find((p) => p.summary.displayName === name);
  if (!pkg) throw new Error(`The mock library has no ${name}`);
  return pkg.summary.id;
};

const findButton = (name: string): HTMLButtonElement => {
  const b = document.querySelector<HTMLButtonElement>(`button[data-find="${idOf(name)}"]`);
  if (!b) throw new Error(`No Find… on ${name}`);
  return b;
};

const candidatesOf = (name: string): HTMLOListElement | null =>
  document.querySelector<HTMLOListElement>(`ol[aria-label="Nexus candidates for ${name}"]`);

const toastLines = (): string[] => useToasts().toasts.value.map((t) => t.line);

async function openFarWorld(scene: Scene = ""): Promise<void> {
  await wakeOn(scene);
  useNavigation().go("nexus");
  await until(() => hasButton("Find all by name"), "the UNLINKED rows with Find all by name");
}

async function quiet(): Promise<void> {
  const far = useFarWorld();
  await until(
    () => !far.findingAll.value && far.linkingCandidate.value === null && !far.checking.value,
    "the far world at rest",
    8_000,
  ).catch(() => undefined);
  await new Promise((resolve) => setTimeout(resolve, 60));
}

afterEach(async () => {
  await quiet();
  vi.restoreAllMocks();
  sleep();
});

describe("Find on Nexus", () => {
  it("should offer Find… beside Link… on every unlinked row, saying it sends the name", async () => {
    await openFarWorld();
    for (const name of UNLINKED) {
      const b = findButton(name);
      expect(b.title).toBe(`Sends the name “${name}” to Nexus Mods ${SENDS} to search for it`);
    }
    expect(button("Find all by name").title).toContain(SENDS);
  });

  it("should unfold the candidates under the row, and link nothing by itself", async () => {
    await openFarWorld();
    const find = vi.spyOn(spine(), "findOnNexus");
    const link = vi.spyOn(spine(), "linkPackage");
    findButton("Old Road Armor Pack").click();
    await until(() => candidatesOf("Old Road Armor Pack") !== null, "the candidates");

    expect(find).toHaveBeenCalledWith(idOf("Old Road Armor Pack"));
    expect(link).not.toHaveBeenCalled();
    const list = candidatesOf("Old Road Armor Pack") as HTMLOListElement;
    const items = [...list.querySelectorAll(":scope > li")].map((li) => textOf(li).replace(/\s+/g, " "));
    expect(items).toHaveLength(2);
    expect(items[0]).toContain("Old Road Armor Pack");
    expect(items[0]).toContain("by Hollow Smith · Armour · 12,840 downloads · v1.3");
    expect(items[0]).toContain("already linked to 1 other package");
    expect(items[0]).not.toContain("ADULT");
    expect(items[1]).toContain("ADULT");
    expect(text()).toContain("ASKED “old road armor pack” · 2 ANSWERED");
    // the lit words carry colour AND underline: HitText's mark, never a coloured span
    const lit = [...list.querySelectorAll(".far__cname mark.rs-hit")].map((m) => textOf(m).toLowerCase());
    expect(lit).toContain("armor");
    expect(list.querySelector(".is-hit")).toBeNull();
    // drawn at once, not staggered in: no per-candidate delay is set
    expect(list.querySelector("li[style]")).toBeNull();
    // one status line speaks the answer; the row's list is not a live region
    expect(document.querySelector(".far__find[aria-live]")).toBeNull();
    expect(textOf(document.querySelector("[data-find-note]") as Node)).toBe(
      "Nexus offered 2 candidates for “Old Road Armor Pack”.",
    );
  });

  it("should open the mod's Nexus page from its name", async () => {
    await openFarWorld();
    // stood in: the mock opens a window, and happy-dom would fetch the page
    const open = vi.spyOn(spine(), "openUrl").mockResolvedValue(undefined);
    findButton("Sacred Arbor Fog Removal").click();
    await until(() => candidatesOf("Sacred Arbor Fog Removal") !== null, "the candidate");
    const name = candidatesOf("Sacred Arbor Fog Removal")?.querySelector<HTMLButtonElement>(".far__cname");
    // the null author and version are skipped, not printed
    expect(textOf(candidatesOf("Sacred Arbor Fog Removal") as Node)).toContain("Visuals · 980 downloads");
    name?.click();
    await until(() => open.mock.calls.length > 0, "the page opened");
    expect(open).toHaveBeenCalledWith("https://www.nexusmods.com/dragonsdogma2/mods/5120");
  });

  it(
    "should link the picked candidate, then check, so its labels arrive at once",
    async () => {
      await openFarWorld();
      const link = vi.spyOn(spine(), "linkPackage");
      const check = vi.spyOn(spine(), "checkUpdates");
      findButton("Old Road Armor Pack").click();
      await until(() => candidatesOf("Old Road Armor Pack") !== null, "the candidates");
      button("Link this").click();
      await until(() => toastLines().some((l) => l.includes("came through")), "the link toast", 8_000);

      expect(link).toHaveBeenCalledWith(idOf("Old Road Armor Pack"), "4410");
      expect(check).toHaveBeenCalledTimes(1);
      // a link by mod id names no file: the check cannot pin one, and the toast says so
      expect(toastLines().at(-1)).toBe(
        "Old Road Armor Pack came through: linked to Old Road Armor Pack, Nexus mod 4410. Its labels came home with it. No file is pinned yet, so the stone can't tell whether your copy is the newest; link its file's Nexus URL to pin one.",
      );
      const linkedRow = document.getElementById(`far-linked-${idOf("Old Road Armor Pack")}`);
      expect(textOf(linkedRow as Node)).toContain("no file pinned");
      expect(JSON.stringify(await spine().getLabels())).toContain("Armour Sets");
      expect(candidatesOf("Old Road Armor Pack")).toBeNull();
      // the last unlinked row left the list: the focus lands on its new line
      expect(document.activeElement?.id).toBe(`far-linked-${idOf("Old Road Armor Pack")}`);
      expect(text()).toContain("mod 4410 · manual");
    },
    SLOW_TEST_MS,
  );

  it(
    "should move the focus on to the next unlinked ROW after a link, never onto a button that sends",
    async () => {
      await openFarWorld();
      findButton("Ox Cart Express").click();
      await until(() => candidatesOf("Ox Cart Express") !== null, "the candidate");
      button("Link this").click();
      await until(() => toastLines().some((l) => l.includes("came through")), "the link toast", 8_000);
      expect(document.activeElement?.id).toBe(`far-row-${idOf("Brine Water Clarity")}`);
      expect(document.activeElement?.tagName).toBe("LI");
      // a Space or Enter where the focus landed sends no name
      const find = vi.spyOn(spine(), "findOnNexus");
      for (const key of [" ", "Enter"]) {
        document.activeElement?.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true }));
        document.activeElement?.dispatchEvent(new KeyboardEvent("keyup", { key, bubbles: true }));
      }
      await new Promise((resolve) => setTimeout(resolve, 250));
      expect(find).not.toHaveBeenCalled();
    },
    SLOW_TEST_MS,
  );

  it(
    "should say the labels follow when the check after a link cannot reach Nexus, and judge the link not checked",
    async () => {
      await openFarWorld();
      findButton("Old Road Armor Pack").click();
      await until(() => candidatesOf("Old Road Armor Pack") !== null, "the candidates");
      vi.spyOn(spine(), "checkUpdates").mockRejectedValueOnce(
        new RiftFailure({ kind: "nexus", detail: { detail: "Nexus Mods could not be reached (connection refused)." } }),
      );
      button("Link this").click();
      await until(() => toastLines().some((l) => l.includes("came through")), "the link toast", 8_000);
      expect(toastLines().at(-1)).toContain("Its labels follow at the next check that reaches Nexus.");
      const linkedRow = document.getElementById(`far-linked-${idOf("Old Road Armor Pack")}`);
      expect(textOf(linkedRow as Node)).toContain("not checked");
    },
    SLOW_TEST_MS,
  );

  it("should keep the focus on a busy Find… through the ask, not drop it to the page", async () => {
    await openFarWorld();
    const b = findButton("Old Road Armor Pack");
    b.focus();
    b.click();
    await until(() => b.getAttribute("aria-busy") === "true", "the busy Find…");
    expect(b.disabled).toBe(false);
    expect(b.getAttribute("aria-disabled")).toBe("true");
    expect(document.activeElement).toBe(b);
    // a second press while busy sends nothing more
    const find = vi.spyOn(spine(), "findOnNexus");
    b.click();
    await until(() => candidatesOf("Old Road Armor Pack") !== null, "the candidates");
    expect(find).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(b);
  });

  it(
    "should count 'already linked' as it stands now, and drop a row's old answer when it is linked or unlinked",
    async () => {
      await openFarWorld();
      const far = useFarWorld();
      findButton("Old Road Armor Pack").click();
      await until(() => candidatesOf("Old Road Armor Pack") !== null, "the candidates");
      expect(textOf(candidatesOf("Old Road Armor Pack") as Node)).toContain("already linked to 1 other package");
      // another row links to the same mod by URL: the open list follows
      expect(await far.link(idOf("Brine Water Clarity"), "4410")).toBe(true);
      await until(
        () => textOf(candidatesOf("Old Road Armor Pack") as Node).includes("already linked to 2 other packages"),
        "the count moved up",
      );
      await far.unlink(idOf("Brine Water Clarity"));
      await until(
        () => textOf(candidatesOf("Old Road Armor Pack") as Node).includes("already linked to 1 other package"),
        "the count moved back",
      );
      // a row's own answer goes when the row is linked, and does not come back on unlink
      findButton("Ox Cart Express").click();
      await until(() => candidatesOf("Ox Cart Express") !== null, "Ox Cart's candidate");
      expect(await far.link(idOf("Ox Cart Express"), "6031")).toBe(true);
      await far.unlink(idOf("Ox Cart Express"));
      await until(() => document.querySelector(`button[data-find="${idOf("Ox Cart Express")}"]`) !== null, "Ox back");
      expect(far.finds.value[idOf("Ox Cart Express")]).toBeUndefined();
      expect(candidatesOf("Ox Cart Express")).toBeNull();
    },
    SLOW_TEST_MS,
  );

  it("should say honestly when Nexus knows no mod by that name, and open the Link… field", async () => {
    await openFarWorld();
    findButton("REFramework").click();
    await until(() => text().includes("Nexus knows no"), "the honest line");
    expect(text()).toContain("Nexus knows no Dragon's Dogma 2 mod named like “reframework”.");
    const field = document.getElementById(`far-link-${idOf("REFramework")}`);
    expect(field).toBeInstanceOf(HTMLInputElement);
    expect(document.activeElement).toBe(field);
  });

  it("should say when the name holds nothing to search for, and send the player to the Link… field", async () => {
    await openFarWorld();
    const id = idOf("Ox Cart Express");
    vi.spyOn(spine(), "findOnNexus").mockResolvedValueOnce({ packageId: id, terms: "", candidates: [] });
    findButton("Ox Cart Express").click();
    await until(() => text().includes("holds nothing"), "the empty-terms line");
    expect(text()).toContain("This package's name holds nothing to search for. Paste its Nexus page below instead.");
    expect(document.getElementById(`far-link-${id}`)).toBeInstanceOf(HTMLInputElement);
  });

  it("should keep a URL already typed into the row's open field when Find… finds nothing", async () => {
    await openFarWorld();
    const id = idOf("REFramework");
    button("Link…").click(); // the first row's
    await until(() => document.getElementById(`far-link-${id}`) !== null, "the field");
    const field = document.getElementById(`far-link-${id}`) as HTMLInputElement;
    field.value = "https://www.nexusmods.com/dragonsdogma2/mods/2207";
    field.dispatchEvent(new Event("input"));
    findButton("REFramework").click();
    await until(() => text().includes("Nexus knows no"), "the honest line");
    expect((document.getElementById(`far-link-${id}`) as HTMLInputElement).value).toBe(
      "https://www.nexusmods.com/dragonsdogma2/mods/2207",
    );
  });

  it("should leave another row's open Link… field alone when a find comes back empty, and offer the field instead", async () => {
    await openFarWorld();
    const ox = idOf("Ox Cart Express");
    const ref = idOf("REFramework");
    useFarWorld().linkingId.value = ox;
    await until(() => document.getElementById(`far-link-${ox}`) !== null, "Ox Cart's field");
    const field = document.getElementById(`far-link-${ox}`) as HTMLInputElement;
    field.value = "2207";
    field.dispatchEvent(new Event("input"));
    field.focus();
    findButton("REFramework").click();
    await until(() => text().includes("Nexus knows no"), "the honest line");
    expect(useFarWorld().linkingId.value).toBe(ox);
    expect((document.getElementById(`far-link-${ox}`) as HTMLInputElement).value).toBe("2207");
    expect(document.getElementById(`far-link-${ref}`)).toBeNull();
    // the line itself opens the field
    button("Paste its Nexus page…").click();
    await until(() => document.getElementById(`far-link-${ref}`) !== null, "REFramework's field");
    expect(document.activeElement).toBe(document.getElementById(`far-link-${ref}`));
  });

  it(
    "should Find all by name one row at a time, unfold every answer, and link nothing",
    async () => {
      await openFarWorld();
      const find = vi.spyOn(spine(), "findOnNexus");
      const link = vi.spyOn(spine(), "linkPackage");
      const lines = new Set<string>();
      const far = useFarWorld();
      button("Find all by name").click();
      await until(
        () => {
          const line = document.querySelector(".far__sechead [role=status]");
          if (line) lines.add(textOf(line).trim());
          return !far.findingAll.value && find.mock.calls.length === UNLINKED.length;
        },
        "every row asked",
        8_000,
      );

      expect(find.mock.calls.map(([id]) => id)).toStrictEqual(UNLINKED.map(idOf));
      expect(lines).toContain("asking Nexus about 2 of 5…");
      expect(link).not.toHaveBeenCalled();
      for (const name of UNLINKED.filter((n) => n !== "REFramework")) expect(candidatesOf(name)).not.toBeNull();
      expect(text()).toContain("Nexus knows no Dragon's Dogma 2 mod named like “reframework”.");
      expect(far.unlinked.value).toHaveLength(UNLINKED.length);
      // no field is below after Find all: the line offers it, and one line sums the walk
      expect(hasButton("Paste its Nexus page…")).toBe(true);
      expect(document.querySelector("form.far__link")).toBeNull();
      expect(textOf(document.querySelector("[data-find-note]") as Node)).toBe(
        "Nexus offered candidates for 4 of 5; 1 found nothing.",
      );
    },
    SLOW_TEST_MS,
  );

  it("should voice an offline find as the far world's absence, not the player's mistake", async () => {
    await openFarWorld("offline");
    findButton("Ox Cart Express").click();
    await until(() => text().includes("OUT OF REACH"), "the offline notice");
    expect(text()).toContain("We can't reach Nexus right now, so it could not be asked about this package.");
    expect(text()).toContain("Nothing was linked; the package stays as it was.");
    expect(hasButton("Try again")).toBe(true);
    expect(candidatesOf("Ox Cart Express")).toBeNull();
  });

  it("should stop Find all at the first failure: offline, every later ask fails the same way", async () => {
    await openFarWorld("offline");
    const find = vi.spyOn(spine(), "findOnNexus");
    button("Find all by name").click();
    await until(() => !useFarWorld().findingAll.value && find.mock.calls.length > 0, "the walk stopped");
    expect(find).toHaveBeenCalledTimes(1);
    expect(textOf(document.querySelector("[data-find-note]") as Node)).toBe(
      "Find all stopped at 1 of 5: Nexus could not be asked (the reason is under that row). 4 packages not asked.",
    );
  });

  it(
    "should walk on past a package that left the library, and Rescan from its row rather than re-ask",
    async () => {
      await openFarWorld();
      const gone = idOf("Ox Cart Express");
      const real = spine().findOnNexus.bind(spine());
      const find = vi
        .spyOn(spine(), "findOnNexus")
        .mockImplementation((id) =>
          id === gone
            ? Promise.reject(new RiftFailure({ kind: "entryNotFound", detail: { packageId: id } }))
            : real(id),
        );
      button("Find all by name").click();
      await until(() => !useFarWorld().findingAll.value && find.mock.calls.length === UNLINKED.length, "every row");
      expect(textOf(document.querySelector("[data-find-note]") as Node)).toBe(
        "Nexus offered candidates for 3 of 5; 1 found nothing; 1 could not be asked.",
      );
      const rescan = vi.spyOn(spine(), "rescan");
      find.mockClear();
      button("Rescan").click();
      await until(() => rescan.mock.calls.length > 0, "the rescan");
      expect(find).not.toHaveBeenCalled();
    },
    SLOW_TEST_MS,
  );

  it("should voice a refused anonymous find without any key talk, and its button retries", async () => {
    await openFarWorld();
    const find = vi.spyOn(spine(), "findOnNexus").mockRejectedValueOnce(
      new RiftFailure({
        kind: "nexus",
        detail: {
          detail: "Nexus Mods answered with HTTP status 403. The site may be busy or down; try again in a few minutes.",
        },
      }),
    );
    findButton("Ox Cart Express").click();
    await until(() => text().includes("turned the search away"), "the refusal");
    const row = document.getElementById(`far-row-${idOf("Ox Cart Express")}`) as HTMLElement;
    expect(textOf(row)).not.toMatch(/accept that key|Replace key/);
    expect(textOf(row)).toContain("The search sends no key, so no key of yours was refused.");
    (row.querySelector(".far__findfail button") as HTMLButtonElement).click();
    await until(() => candidatesOf("Ox Cart Express") !== null, "the retry's candidates");
    expect(find).toHaveBeenCalledTimes(2);
    expect(document.querySelector("form.far__link input[type=password]")).toBeNull();
  });

  it("should give the find's quota and offline failures a recovery", () => {
    const offline = voice(new RiftFailure({ kind: "nexus", detail: { detail: "connection refused" } }), "nexusFind");
    expect(offline.inscription).toBe("THE FAR WORLD IS OUT OF REACH");
    expect(offline.recovery.map((r) => r.kind)).toStrictEqual(["retry"]);
    // the spine's own rate-limit detail (nexus.rs nexus_error): its retry hint stands as the voice
    const quota = voice(
      new RiftFailure({
        kind: "nexus",
        detail: { detail: "rate limit: Nexus Mods asked the Riftstone to slow down. Try again in 30 seconds." },
      }),
      "nexusFind",
    );
    expect(quota.voice).toBe("Nexus asked the stone to slow down: try again in 30 seconds.");
    expect(quota.empathy).toBe(
      "Nothing was linked; the package stays as it was. Find again after that, or paste its Nexus page.",
    );
    expect(quota.recovery.map((r) => r.kind)).toStrictEqual(["retry"]);
  });

  it("should say Nexus answered but could not be read when the answer was unusable, not that it is offline", () => {
    for (const detail of [
      "Nexus Mods sent an answer the Riftstone could not use (unexpected shape). Try again later; if it keeps happening, the Nexus API may have changed and the Riftstone needs an update.",
      "Nexus Mods answered with HTTP status 503. The site may be busy or down; try again in a few minutes.",
    ]) {
      const v = voice(new RiftFailure({ kind: "nexus", detail: { detail } }), "nexusFind");
      expect(v.inscription).toBeNull();
      expect(v.voice).toBe("Nexus answered, but the stone could not read its answer.");
      expect(v.recovery.map((r) => r.kind)).toStrictEqual(["retry"]);
    }
    // outside Find, a 403 still speaks of the key (the update check's own voice)
    const key = voice(new RiftFailure({ kind: "nexus", detail: { detail: "HTTP 403" } }), "nexusCheck");
    expect(key.inscription).toBe("THE FAR WORLD REFUSES");
  });

  it("should count only the candidates it shows", async () => {
    await openFarWorld();
    const id = idOf("Old Road Armor Pack");
    const real = await spine().findOnNexus(id);
    const [base] = real.candidates;
    const seven = Array.from({ length: 7 }, (_, i) => ({
      ...base,
      modId: 9000 + i,
      name: `Lantern Glow Recolor ${i}`,
    }));
    vi.spyOn(spine(), "findOnNexus").mockResolvedValueOnce({ ...real, candidates: seven });
    findButton("Old Road Armor Pack").click();
    await until(() => candidatesOf("Old Road Armor Pack") !== null, "the candidates");
    expect(text()).toContain("· 5 ANSWERED");
    expect(candidatesOf("Old Road Armor Pack")?.querySelectorAll(":scope > li")).toHaveLength(5);
    expect(useFarWorld().finds.value[id].status).toBe("found");
  });
});
