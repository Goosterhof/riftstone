// v0.5 — the player's word (#00015 §18.18, Bold Choice #23; WINDOW-CONTRACT
// § v0.5, The player's word): the stone asks whether an unpacked folder linked
// by mod id is a version of the pinned v1.3, in THE VERSIONS card drawn the
// way the yes would make it. The yes joins, the no is remembered, `Take it
// back` parts, and R18.18.6's one manual door joins without a question. Every
// name is the mock's invented one (Lantern Glow Recolor, mod 2207).

import { afterEach, describe, expect, it, vi } from "vitest";
import { useLibrary, questionKey } from "@/composables/useLibrary";
import { useNavigation } from "@/composables/useNavigation";
import { useToasts } from "@/composables/useToasts";
import { RiftFailure } from "@/services/rift";
import { voice } from "@/services/voice";
import { CANDLES_FILE, FOLDER_NAME, familyFileName } from "@/services/mock/versions";
import {
  BY_YOUR_WORD,
  JOINED_LINE,
  MANUAL_LINE,
  askMark,
  evidenceLine,
  farAskLine,
  joinCandidates,
  keptApartToast,
  linkSourceText,
  partedToast,
} from "@/services/versions";
import type { LibraryPackage, VersionQuestion } from "@/types/rift";
import { button, hasButton, idle, press, sleep, spine, textOf, until, wakeOn, type Scene } from "./harness";

afterEach(() => {
  vi.restoreAllMocks();
  sleep();
});

function must<T>(found: T | null | undefined, what: string): T {
  if (found === null || found === undefined) throw new Error(`The stone drew no ${what}`);
  return found;
}

const said = (el: Element): string => textOf(el).replace(/\s+/g, " ").trim();
const pkgBy = (fileName: string): LibraryPackage =>
  must(
    useLibrary().packages.value.find((p) => p.package.fileName === fileName),
    `package ${fileName}`,
  );
const folder = (): LibraryPackage => pkgBy(FOLDER_NAME);
const lead = (): LibraryPackage => pkgBy(familyFileName("1.3"));
const item = (key: string): HTMLElement | null =>
  document.querySelector<HTMLElement>(`#rift-tree [data-key="${CSS.escape(key)}"]`);
const line = (el: HTMLElement): HTMLElement => must(el.querySelector<HTMLElement>(":scope > .ltr__row"), "row line");
const card = (): HTMLElement | null => document.querySelector<HTMLElement>(".vc");
const toastLines = (): string[] => useToasts().toasts.value.map((t) => t.line);

async function wakeAsked(scene: Scene): Promise<void> {
  await wakeOn(scene);
  await until(() => useLibrary().packages.value.some((p) => p.package.fileName === FOLDER_NAME), "the folder");
  await until(() => document.querySelector("#rift-tree [role='treeitem']") !== null, "the tree");
}

/** Opens the folder's entry and presses Enter on its question row. */
async function openQuestionCard(): Promise<void> {
  const id = folder().package.id;
  useLibrary().expand(id);
  await until(() => item(questionKey(id)) !== null, "the question row");
  const row = must(item(questionKey(id)), "the question row");
  useLibrary().focusedKey.value = questionKey(id);
  row.focus();
  press("Enter");
  await until(() => card() !== null && /VERSIONS OF ONE FILE\?/.test(said(must(card(), "card"))), "the question card");
}

/** A failure of the player's word, voiced in the olderVersions context. */
const io = (act: string) =>
  voice(
    new RiftFailure({ kind: "io", detail: { reason: "Access is denied.", act, file: FOLDER_NAME } }),
    "olderVersions",
  );

/** A link of mod 2207 (or another), with or without a file group. */
const link = (groupId: number | null, modId = 2207) => ({
  modId,
  fileId: null,
  groupId,
  sqid: null,
  version: null,
  uploadedMs: null,
  source: "manual" as const,
});

const QUESTION: VersionQuestion = {
  ofPackageId: "p13",
  ofFileName: "Lantern_Glow_Recolor-2207-1-3-1730540000.zip",
  ofVersion: "1.3",
  sharedFiles: 4,
  files: 4,
};

describe("Where the question shows (R18.18.1)", () => {
  it("should mark the folder's row, open its entry on the question row first, and leave v1.3 unmarked", async () => {
    await wakeAsked("question");
    const id = folder().package.id;
    const row = must(item(id), "the folder's row");
    expect(said(line(row))).toContain("a version of v1.3?");
    expect(row.querySelector(".ltr__ask")?.getAttribute("title")).toBe(
      "The stone asks whether this is a version of the same file as v1.3",
    );
    expect(said(line(must(item(lead().package.id), "v1.3's row")))).not.toContain("?");
    useLibrary().expand(id);
    await until(() => item(questionKey(id)) !== null, "the question row");
    const asked = must(item(questionKey(id)), "the question row");
    expect(asked.getAttribute("role")).toBe("treeitem");
    expect(asked.getAttribute("aria-level")).toBe("2");
    expect(asked.getAttribute("aria-posinset")).toBe("1");
    expect(asked.hasAttribute("aria-expanded")).toBe(false);
    expect(said(must(asked.querySelector(".ltr__label.is-ask"), "the question's label"))).toBe(
      "Is this a version of v1.3?",
    );
    // the first child of the entry, and no button on a tree row (Bold Choice 11)
    const group = must(row.querySelector(":scope > ul[role='group']"), "the entry's group");
    expect(group.firstElementChild).toBe(asked);
    expect(row.querySelector("button")).toBeNull();
  });

  it("should carry a FAR WORLD sub-row under the folder's own row, with no yes or no in it", async () => {
    await wakeAsked("question");
    useNavigation().go("nexus");
    await until(() => document.querySelector(".far__ask") !== null, "the far question");
    const ask = must(document.querySelector<HTMLElement>(".far__ask"), "the far question");
    expect(said(must(ask.querySelector(":scope > span:not([aria-hidden])"), "the far line"))).toBe(
      "The stone thinks this may be a version of the same file as v1.3: all 4 of its file names are in it too.",
    );
    expect(ask.closest("tr")?.previousElementSibling?.id).toBe(`far-linked-${folder().package.id}`);
    expect(hasButton("Yes, join them")).toBe(false);
    button("Show both versions in the Rift").click();
    await until(() => card() !== null, "the question card from THE FAR WORLD");
    expect(useNavigation().page.value).toBe("rift");
    expect(useLibrary().focusedKey.value).toBe(questionKey(folder().package.id));
  });

  it("should show the World pane's line and `Show both versions…` on the folder's option", async () => {
    await wakeAsked("question");
    const id = folder().package.id;
    useLibrary().expand(id);
    const option = must(folder().tree[0]?.optionId, "the folder's option");
    await until(() => item(option) !== null, "the option row");
    line(must(item(option), "option")).click();
    await until(() => document.querySelector(".odetail__word") !== null, "the word line");
    expect(said(must(document.querySelector(".odetail__word p"), "line"))).toBe(
      "The stone thinks this may be a version of the same file as v1.3.",
    );
    expect(hasButton("Yes, join them")).toBe(false);
    button("Show both versions…").click();
    await until(() => card() !== null, "the question card");
  });
});

describe("The question card (R18.18.2, Bold Choice #23)", () => {
  it("should draw v1.3's row, then the folder with the evidence and two doors", async () => {
    await wakeAsked("question");
    await openQuestionCard();
    const c = must(card(), "card");
    expect(said(must(c.querySelector(".vc__title"), "head"))).toBe("VERSIONS OF ONE FILE?");
    expect(said(must(c.querySelector(".vc__name"), "name"))).toBe("Lantern Glow Recolor");
    const rows = [...c.querySelectorAll<HTMLElement>(".vc__rows .vc__row")];
    expect(rows).toHaveLength(1);
    expect(said(must(rows[0]?.querySelector(".vc__ver"), "v1.3's version"))).toBe("v1.3");
    expect(said(must(rows[0]?.querySelector(".vc__badge"), "newest"))).toBe("newest");
    expect(said(must(rows[0]?.querySelector(".vc__file"), "v1.3's file"))).toBe(familyFileName("1.3"));
    expect(said(must(c.querySelector(".vc__ask"), "question"))).toBe(
      "Is this folder a version of the same file as v1.3?",
    );
    const asked = must(c.querySelector(".vc__row.is-asked"), "folder row");
    expect(said(must(asked.querySelector(".vc__asklabel"), "label"))).toBe("this folder");
    expect(said(must(asked.querySelector(".vc__file"), "name"))).toBe(FOLDER_NAME);
    expect(said(must(asked.querySelector(".vc__line2"), "world line"))).toBe("nothing installed");
    expect(said(must(c.querySelector(".vc__evidence"), "evidence"))).toBe(
      "All 4 of its file names are in v1.3 too. Nexus can't tell: its link names the mod, not the file. Your word can.",
    );
    expect(button("Yes, join them").className).toContain("rs-btn--primary");
    expect(button("No, keep them apart").className).toContain("rs-btn--secondary");
  });

  it("should read the evidence from the spine's two counts only, in each of its forms", () => {
    expect(evidenceLine({ sharedFiles: 3, files: 5 }, "v1.3")).toBe("3 of its 5 file names are in v1.3 too.");
    expect(evidenceLine({ sharedFiles: 4, files: 4 }, "v1.3")).toBe("All 4 of its file names are in v1.3 too.");
    expect(evidenceLine({ sharedFiles: 1, files: 1 }, "v1.3")).toBe("Its one file name is in v1.3 too.");
    expect(farAskLine({ ...QUESTION, sharedFiles: 1, files: 1 })).toBe(
      "The stone thinks this may be a version of the same file as v1.3: its one file name is in it too.",
    );
    expect(askMark({ ...QUESTION, ofVersion: null }).text).toBe(`a version of ${QUESTION.ofFileName}?`);
    expect(askMark(QUESTION).short).toBe("?");
  });

  it("should name the file in mono when Nexus named no version", async () => {
    await wakeAsked("question");
    const real = spine().getLibrary.bind(spine());
    vi.spyOn(spine(), "getLibrary").mockImplementation(async () => {
      const view = await real();
      return {
        ...view,
        packages: view.packages.map((p) =>
          p.versionQuestion ? { ...p, versionQuestion: { ...p.versionQuestion, ofVersion: null } } : p,
        ),
      };
    });
    await useLibrary().load();
    await openQuestionCard();
    const mono = must(card()?.querySelector(".vc__ask .vc__ask-file"), "the file in mono");
    expect(said(mono)).toBe(familyFileName("1.3"));
    expect(said(must(card()?.querySelector(".vc__ask"), "question"))).toBe(
      `Is this folder a version of the same file as ${familyFileName("1.3")}?`,
    );
  });
});

describe("After yes (R18.18.3)", () => {
  it("should send exactly versionQuestion's ids, hold the no, and draw the join at the library's next answer", async () => {
    await wakeAsked("question");
    const q = must(folder().versionQuestion, "the question");
    const id = folder().package.id;
    await openQuestionCard();
    const join = vi.spyOn(spine(), "joinVersions");
    button("Yes, join them").click();
    await until(() => button("No, keep them apart").getAttribute("aria-disabled") === "true", "the no held");
    expect(join).toHaveBeenCalledTimes(1);
    expect(join).toHaveBeenCalledWith(id, q.ofPackageId);
    await until(() => said(must(card(), "card")).includes(JOINED_LINE), "the joined card");
    const c = must(card(), "card");
    expect(said(must(c.querySelector(".vc__title"), "head"))).toBe("VERSIONS OF THIS FILE");
    expect(c.querySelector(".vc__ask")).toBeNull();
    expect(said(must(c.querySelector("[aria-live='polite']"), "live region"))).toBe(JOINED_LINE);
    const rows = [...c.querySelectorAll<HTMLElement>(".vc__rows .vc__row")];
    expect(rows).toHaveLength(2);
    expect(said(must(rows[1], "the joined row"))).toContain(BY_YOUR_WORD);
    expect(hasButton("Take it back")).toBe(true);
    expect(hasButton("Put it away")).toBe(true);
    expect(toastLines().some((t) => t.includes("Joined"))).toBe(false);
    // the Rift: one entry, the lead `+1 older`, the joined version row inside it
    await until(() => item(id)?.getAttribute("aria-level") === "2", "the folder inside the lead");
    expect(said(line(must(item(lead().package.id), "lead")))).toContain("+1 older");
    expect(said(line(must(item(id), "joined row")))).toContain("older version · by your word");
    expect(useLibrary().focusedKey.value).toBe(id);
  });

  it("should read `INSTALLED older` on v1.3's leaf when the joined folder holds the install", async () => {
    await wakeAsked("questioninstalled");
    await openQuestionCard();
    button("Yes, join them").click();
    await until(() => said(must(card(), "card")).includes(JOINED_LINE), "the joined card");
    const option = must(lead().tree[0]?.optionId, "v1.3's option");
    await until(() => item(option) !== null, "v1.3's leaf");
    expect(said(line(must(item(option), "leaf")))).toContain("INSTALLED older");
  });

  it("should print `by your word`, never the raw `joined`, as a link's source", () => {
    expect(linkSourceText("joined")).toBe("by your word");
    expect(linkSourceText("md5")).toBe("by MD5");
    expect(linkSourceText("manual")).toBe("manual");
  });
});

describe("After no (R18.18.4)", () => {
  it("should send exactly versionQuestion's ids, close the card, and leave no mark behind", async () => {
    await wakeAsked("question");
    const q = must(folder().versionQuestion, "the question");
    const id = folder().package.id;
    await openQuestionCard();
    const decline = vi.spyOn(spine(), "declineVersionQuestion");
    button("No, keep them apart").click();
    await until(() => card() === null, "the card closed");
    expect(decline).toHaveBeenCalledTimes(1);
    expect(decline).toHaveBeenCalledWith(id, q.ofPackageId);
    expect(toastLines()).toContain(keptApartToast("folder"));
    await until(() => item(questionKey(id)) === null, "the question row gone");
    expect(said(line(must(item(id), "folder")))).not.toContain("?");
    expect(said(line(must(item(id), "folder")))).not.toContain("declined");
  });
});

describe("Take it back (R18.18.3)", () => {
  it("should call partVersions once and bring the folder back to level 1 with no question", async () => {
    await wakeAsked("questionjoined");
    const id = folder().package.id;
    await until(() => item(lead().package.id) !== null, "the lead");
    useLibrary().expand(lead().package.id);
    await until(() => item(id) !== null, "the joined row");
    press(
      "Enter",
      {},
      (() => {
        useLibrary().focusedKey.value = id;
        const el = must(item(id), "joined row");
        el.focus();
        return el;
      })(),
    );
    await until(() => hasButton("Take it back"), "Take it back");
    const part = vi.spyOn(spine(), "partVersions");
    button("Take it back").click();
    await until(() => item(id)?.getAttribute("aria-level") === "1", "the folder its own entry again");
    expect(part).toHaveBeenCalledTimes(1);
    expect(part).toHaveBeenCalledWith(id);
    expect(toastLines()).toContain(partedToast(FOLDER_NAME));
    await until(() => card() === null, "the card closed: a family of one is none");
    expect(said(line(must(item(id), "folder")))).not.toContain("?");
  });

  it("should offer `Take it back` only on a row joined by your word", async () => {
    await wakeAsked("questionjoined");
    useNavigation().go("rift");
    const { useVersions } = await import("@/composables/useVersions");
    useVersions().open(lead().package.id);
    await until(() => card() !== null, "the card");
    const rows = [...must(card(), "card").querySelectorAll<HTMLElement>(".vc__rows .vc__row")];
    expect(rows).toHaveLength(2);
    expect(rows[0]?.textContent).not.toContain("Take it back");
    expect(rows[1]?.textContent).toContain("Take it back");
    sleep();
    await wakeOn("family");
    await until(() => useLibrary().packages.value.some((p) => p.family?.older.length), "the family");
    useVersions().open(pkgBy(familyFileName("1.3")).package.id);
    await until(() => card() !== null, "the family's card");
    expect(hasButton("Take it back")).toBe(false);
  });
});

describe("Failures (R18.18.5)", () => {
  it("should voice a refused join inside the card, verbatim, with `Scan again` first", async () => {
    await wakeAsked("questionrefused");
    await openQuestionCard();
    button("Yes, join them").click();
    await until(
      () =>
        card()?.querySelector(".rs-inscription, [class*='inscription']") !== null &&
        /couldn't join/.test(said(must(card(), "card"))),
      "the refusal",
    );
    const c = said(must(card(), "card"));
    expect(c).toContain(
      "The stone couldn't join this folder to v1.3: The folder's file names changed since the stone asked, so it no longer reads as that file. Nothing changed; both are still their own entries.",
    );
    expect(button("Scan again").className).toContain("rs-btn--primary");
    expect(hasButton("Copy details")).toBe(true);
    expect(useToasts().toasts.value.some((t) => t.tone === "ember")).toBe(false);
    expect(idle(button("Yes, join them"))).toBe(true);
  });

  it("should voice the no's and the take-back's failures from the voice module", () => {
    expect(`${io("decline").voice} ${io("decline").empathy}`).toBe(
      "The stone couldn't remember your no: Access is denied. The question stays; nothing changed.",
    );
    expect(`${io("part").voice} ${io("part").empathy}`).toBe(
      `The stone couldn't take your word back: Access is denied. ${FOLDER_NAME} is still a version of this file.`,
    );
    expect(io("part").recovery.map((r) => r.label)).toStrictEqual(["Try again", "Copy details"]);
  });
});

describe("Join without the question: one door (R18.18.6)", () => {
  it("should offer the door on the folder's option and list two files with neither picked", async () => {
    await wakeAsked("questionmanual");
    expect(folder().versionQuestion).toBeNull();
    const id = folder().package.id;
    useLibrary().expand(id);
    const option = must(folder().tree[0]?.optionId, "the folder's option");
    await until(() => item(option) !== null, "the option row");
    line(must(item(option), "option")).click();
    await until(() => document.querySelector(".odetail__word") !== null, "the manual line");
    expect(said(must(document.querySelector(".odetail__word p"), "line"))).toBe(MANUAL_LINE);
    button("This is a version of…").click();
    await until(() => document.querySelector(".vc__pick") !== null, "the radio list");
    const radios = [...document.querySelectorAll<HTMLInputElement>(".vc__pick input[type='radio']")];
    expect(radios).toHaveLength(2);
    expect(radios.every((r) => !r.checked)).toBe(true);
    expect(said(must(document.querySelector(".vc__pick"), "list"))).toContain(CANDLES_FILE);
    expect(said(must(card()?.querySelector(".vc__evidence"), "note"))).toBe(
      "The stone isn't asking: their file names don't say so. Join them only if you know they are.",
    );
    expect(hasButton("No, keep them apart")).toBe(false);
    expect(button("Yes, join them").getAttribute("aria-disabled")).toBe("true");
    const join = vi.spyOn(spine(), "joinVersions");
    button("Yes, join them").click();
    expect(join).not.toHaveBeenCalled();
    const candles = pkgBy(CANDLES_FILE).package.id;
    const pick = must(
      radios.find((r) => r.value === candles),
      "the candles radio",
    );
    pick.checked = true;
    pick.dispatchEvent(new Event("change"));
    await until(() => button("Yes, join them").getAttribute("aria-disabled") !== "true", "the yes free");
    button("Yes, join them").click();
    await until(() => join.mock.calls.length === 1, "the join");
    expect(join).toHaveBeenCalledWith(id, candles);
  });

  it("should check the predicate on all four of its inputs", () => {
    const pkg = (id: string, nexus: ReturnType<typeof link> | null, extra: Partial<LibraryPackage> = {}) =>
      ({
        package: { id, fileName: `${id}.zip`, nexus },
        family: null,
        versionQuestion: null,
        ...extra,
      }) as unknown as LibraryPackage;
    const pinned = pkg("pinned", link(220750));
    const asked = pkg("asked", link(null));
    expect(joinCandidates(asked, [asked, pinned]).map((c) => c.packageId)).toStrictEqual(["pinned"]);
    // 1: a link that names a file group, or no link
    expect(joinCandidates(pkg("asked", link(1)), [pinned])).toStrictEqual([]);
    expect(joinCandidates(pkg("asked", null), [pinned])).toStrictEqual([]);
    // 2: already in a family
    const inFamily = pkg("asked", link(null), { family: { lead: "pinned", version: null, older: [] } });
    expect(joinCandidates(inFamily, [inFamily, pinned])).toStrictEqual([]);
    // 3: a question is asked already
    const questioned = pkg("asked", link(null), { versionQuestion: QUESTION });
    expect(joinCandidates(questioned, [questioned, pinned])).toStrictEqual([]);
    // 4: no pinned package of the same mod
    expect(joinCandidates(asked, [asked, pkg("other", link(220750, 9999))])).toStrictEqual([]);
    expect(joinCandidates(asked, [asked, pkg("unpinned", link(null))])).toStrictEqual([]);
  });
});

describe("The window works nothing out (Pattern 013, R18.30)", () => {
  // the window's own code: every source but the mock (which plays the spine) and the types
  const WINDOW = import.meta.glob<string>(
    ["../src/**/*.ts", "../src/**/*.vue", "!../src/services/mock/**", "!../src/types/**"],
    { query: "?raw", import: "default", eager: true },
  );
  const sources = Object.entries(WINDOW).map(([f, s]) => ({ f: f.replace("../src/", ""), s }));

  it("should read the window's sources", () => {
    expect(sources.length).toBeGreaterThan(40);
    expect(sources.some((x) => x.f === "components/VersionsCard.vue")).toBe(true);
  });

  it("should never write sharedFiles or an ofPackageId, nor read file names to compare them", () => {
    for (const { f, s } of sources) {
      expect(/sharedFiles\s*(:(?!\s*number\b)|=(?!=)|[+-]=|\+\+|--)/.test(s), `${f} writes sharedFiles`).toBe(false);
      expect(/ofPackageId\s*(=(?!=)|:(?!\s*string\b))/.test(s), `${f} picks an ofPackageId`).toBe(false);
      expect(/\.entries\b.*\.toLowerCase\(\)/.test(s), `${f} compares file names`).toBe(false);
    }
  });

  it("should keep every sentence of the player's word out of the components and pages", () => {
    const sentences = [
      "Joined by your word",
      "Kept apart",
      "Nexus can't tell",
      "Yes, join them",
      "No, keep them apart",
      "is its own entry in the Rift again",
      "The stone thinks this may be",
      "couldn't join",
      "couldn't remember your no",
      "couldn't take your word back",
      "The stone isn't asking",
      "by your word",
    ];
    for (const { f, s } of sources.filter((x) => /^(components|pages)\//.test(x.f))) {
      const code = s.replace(/<!--[\s\S]*?-->|\/\/.*$|\/\*[\s\S]*?\*\//gm, "");
      for (const sentence of sentences) expect(code, `${f} composes "${sentence}"`).not.toContain(sentence);
    }
  });
});
