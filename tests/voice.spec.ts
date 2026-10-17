// The Error Manifesto, as one module: every RiftError kind the spine can
// reject with leaves services/voice.ts with a voice, an empathy line and at
// least one recovery — in every context the window can be in.

import { describe, expect, it } from "vitest";
import type { NexusReason, ProblemKind, RiftError } from "@/types/rift";
import { RiftFailure, asRiftFailure } from "@/services/rift";
import { MOCK_PACKAGES } from "@/services/mock/library";
import {
  TROUBLE_KINDS,
  VOICE_CONTEXTS,
  failureDetails,
  leadProblem,
  missingPackageTitle,
  troubleLine,
  voice,
  voicedLine,
} from "@/services/voice";

const KINDS: RiftError["kind"][] = [
  "unreadable",
  "unsupportedFormat",
  "encrypted",
  "corruptArchive",
  "unsafeEntryPath",
  "entryNotFound",
  "gameNotFound",
  "refused",
  "journal",
  "nexus",
  "io",
];

/** Every context the window can be in: a new one is voiced in every kind before it ships. */
const CONTEXTS = VOICE_CONTEXTS;

const fail = (kind: RiftError["kind"], detail: Record<string, string> = {}) => new RiftFailure({ kind, detail });
/** A v0.3 far-world failure: the spine's sentence beside its reason code. */
const coded = (reason: NexusReason, detail = "Nexus said no.") => fail("nexus", { detail, reason });

describe("voice()", () => {
  describe("every kind in every context", () => {
    for (const kind of KINDS) {
      it(`should give ${kind} a voice, an empathy line and a recovery in all ${CONTEXTS.length} contexts`, () => {
        for (const context of CONTEXTS) {
          const voiced = voice(fail(kind), context);
          expect(voiced.kind).toBe(kind);
          expect(voiced.voice.trim().length).toBeGreaterThan(10);
          expect(voiced.empathy.trim().length).toBeGreaterThan(10);
          expect(voiced.recovery.length).toBeGreaterThan(0);
          for (const r of voiced.recovery) expect(r.label.trim()).not.toBe("");
          expect(voiced.voice).not.toMatch(/something went wrong|undefined|null|\[object/i);
        }
      });
    }
  });

  it("should say NO WORLD ANSWERS for a missing game and lead with the folder picker", () => {
    const voiced = voice(fail("gameNotFound"), "game");
    expect(voiced.inscription).toBe("NO WORLD ANSWERS");
    expect(voiced.voice).toBe(
      "The stone searched every Steam library on this PC and found no Dragon's Dogma 2 install.",
    );
    expect(voiced.recovery.map((r) => r.kind)).toStrictEqual(["pickGameFolder", "retry"]);
  });

  it("should voice a failed crossing with the rollback count when the spine reports one", () => {
    const voiced = voice(
      fail("io", { path: "pak_mods/x0004-SunwardCuirassGilded.pak", reason: "the file is in use", rolledBack: "4" }),
      "install",
    );
    expect(voiced.inscription).toBe("THE CROSSING FAILED");
    expect(voicedLine(voiced)).toBe(
      "Couldn't write `pak_mods/x0004-SunwardCuirassGilded.pak`: the file is in use. Your game folder is exactly as it was: the stone put back the 4 files already placed. Close the game if it's running.",
    );
    expect(voiced.recovery.map((r) => r.label)).toStrictEqual(["Try again", "Copy details"]);
  });

  it("should never claim a rollback the spine did not report", () => {
    const voiced = voice(fail("io", { path: "pak_mods/a.pak" }), "install");
    expect(voiced.empathy).not.toMatch(/put back/);
  });

  it("should name a broken archive by its file name and offer the folder and a rescan", () => {
    const voiced = voice(fail("corruptArchive", { path: "D:\\Mods\\Harbour Lights.rar" }), "scan");
    expect(voiced.voice).toBe(
      "The stone couldn't open `Harbour Lights.rar`: the archive ends early (truncated download?).",
    );
    expect(voiced.recovery.map((r) => r.kind)).toStrictEqual(["showInFolder", "rescan"]);
  });

  it("should tell a running game apart from other refusals", () => {
    expect(voice(fail("refused", { reason: "Dragon's Dogma 2 is running" }), "install").inscription).toBe(
      "THE WORLD IS AWAKE",
    );
    expect(voice(fail("refused", { reason: "that file is vanilla" }), "install").inscription).toBeNull();
  });

  describe("the far world", () => {
    it("should read a rejected key as THE FAR WORLD REFUSES with Replace key…", () => {
      const voiced = voice(fail("nexus", { detail: "HTTP 401 Unauthorized" }), "nexusKey");
      expect(voiced.inscription).toBe("THE FAR WORLD REFUSES");
      expect(voiced.recovery.map((r) => r.label)).toStrictEqual(["Replace key…"]);
    });

    it("should read a spent quota as rationing, not as an outage", () => {
      const voiced = voice(fail("nexus", { detail: "429 rate limit" }), "nexusCheck");
      expect(voiced.inscription).toBeNull();
      expect(voiced.voice).toMatch(/^Nexus has rationed the stone for now/);
    });

    it("should read anything else as out of reach, and keep the offline promise", () => {
      const voiced = voice(fail("nexus", { detail: "connection refused" }), "nexusCheck");
      expect(voiced.inscription).toBe("THE FAR WORLD IS OUT OF REACH");
      expect(voiced.empathy).toContain("Everything else works offline.");
    });

    it("should read a bad link as a link problem while linking", () => {
      expect(voice(fail("nexus", { reason: "no mod id in it" }), "nexusLink").voice).toMatch(/^That link didn't lead/);
    });
  });

  describe("the far world by its reason (v0.3)", () => {
    const REASONS: NexusReason[] = [
      "offline",
      "rateLimited",
      "noKey",
      "keyRejected",
      "premiumRequired",
      "linkExpired",
      "linkNotYours",
      "linkRefused",
      "fileGone",
      "sizeMismatch",
      "notTheFile",
      "endorseNotDownloaded",
      "endorseTooSoon",
      "endorseOwnMod",
    ];

    it("should give every reason a voice of its own in every context but a find (which sends no key)", () => {
      for (const context of CONTEXTS.filter((c) => c !== "nexusFind")) {
        const voices = new Set(REASONS.map((reason) => voice(coded(reason), context).voice));
        expect({ context, distinct: voices.size }).toStrictEqual({ context, distinct: REASONS.length });
      }
    });

    it("should read the reason first, whatever words the sentence carries", () => {
      // a sentence that would read as a rejected key says offline when the code says so
      expect(voice(coded("offline", "HTTP 401 Unauthorized"), "download").inscription).toBe(
        "THE FAR WORLD IS OUT OF REACH",
      );
      expect(voice(coded("keyRejected", "connection refused"), "standings").inscription).toBe("THE FAR WORLD REFUSES");
      // and the code is never read out as if it were the sentence
      expect(voice(fail("nexus", { reason: "rateLimited" }), "nexusCheck").voice).toBe(
        "Nexus has rationed the stone for now: the hourly request quota is spent.",
      );
    });

    it("should send a download's link troubles back to the file's page for a fresh link", () => {
      for (const reason of ["premiumRequired", "linkExpired", "linkRefused"] as const) {
        const voiced = voice(coded(reason), "download");
        expect(voiced.recovery.map((r) => r.kind)).toStrictEqual(["openNexusPage"]);
        expect(voiced.empathy).toMatch(/^Nothing was written to your mods folder\./);
      }
      expect(voice(coded("noKey"), "download").recovery.map((r) => r.label)).toStrictEqual(["Add key…"]);
    });

    it("should say an endorsement Nexus refused changed nothing on Nexus", () => {
      for (const reason of ["endorseNotDownloaded", "endorseTooSoon", "endorseOwnMod"] as const) {
        expect(voice(coded(reason), "standings").empathy).toMatch(/^Nothing changed on Nexus\./);
      }
    });

    it("should keep a refused find the search's refusal, never a key's", () => {
      const voiced = voice(coded("keyRejected"), "nexusFind");
      expect(voiced.voice).toBe("Nexus turned the search away, so it could not be asked about this package.");
    });
  });

  describe("Play (v0.3)", () => {
    it("should hold Play back in the engine's own sentence and say nothing was started", () => {
      const reason =
        "Dragon's Dogma 2 is already running; switch to its window, or close it before you start it again.";
      const voiced = voice(fail("refused", { reason }), "launch");
      expect(voiced.inscription).toBe("THE WORLD IS AWAKE");
      expect(voicedLine(voiced)).toBe(
        "The stone held back: Dragon's Dogma 2 is already running; switch to its window, or close it before you start it again. Nothing was started, and your game folder is as it was.",
      );
    });

    it("should voice a game that would not start as the world staying asleep", () => {
      const voiced = voice(fail("io", { detail: "could not start DD2.exe: access is denied" }), "launch");
      expect(voiced.inscription).toBe("THE WORLD STAYS ASLEEP");
      expect(voiced.voice).toBe(
        "The stone couldn't start Dragon's Dogma 2: could not start DD2.exe: access is denied.",
      );
    });
  });

  it("should put the kind, the context and every detail on the clipboard", () => {
    expect(failureDetails(fail("journal", { reason: "disk full" }), "install")).toBe(
      "Riftstone failure — journal during install\nreason: disk full",
    );
  });
});

describe("asRiftFailure()", () => {
  it("should keep a serialized RiftError's kind and stringify its detail", () => {
    const failure = asRiftFailure({ kind: "unsafeEntryPath", detail: { entry: "../../evil.dll", depth: 2 } });
    expect(failure.kind).toBe("unsafeEntryPath");
    expect(failure.detail).toStrictEqual({ entry: "../../evil.dll", depth: "2" });
  });

  it("should fold an unknown rejection into io with its message, so it still gets a voice", () => {
    const failure = asRiftFailure(new Error("bridge fell"));
    expect(failure.kind).toBe("io");
    expect(voice(failure, "scan").voice).toContain("bridge fell");
  });

  it("should not trust a kind the contract does not name", () => {
    expect(asRiftFailure({ kind: "meteor" }).kind).toBe("io");
  });
});

const problem = (kind: ProblemKind, message: string) => ({ kind, message });

/** The ways a trouble card can say what is wrong. A card says one of them, once
 *  (Chaos #00129 D3): the window owns the verdict, the spine sends only the why. */
const VERDICT =
  /won't load|will not load|stays? out|will not be installed|can(?:not|'t)? be installed|nothing the game loads|couldn't open|could not (?:be )?open(?:ed)?|cannot open|can(?:not|'t) look inside/gi;

/** rift-catalog's own trouble messages (classify.rs, scan.rs), verbatim. */
const SPINE_WHYS: [ProblemKind, string][] = [
  [
    "nothingToInstall",
    "no natives/ or reframework/ folder, no .pak file and no file meant for the game folder. If it should be a Dragon's Dogma 2 mod, check that the download is complete, or look for a repacked version on its mod page.",
  ],
  [
    "filesSkipped",
    "2 files sit in folders the game never reads, and 2 are programs (or named like game folders). The options list them under skipped files; if they were meant for the game, the mod needs repacking.",
  ],
  [
    "outdatedFormats",
    "2 loose files (mesh, tex) were made for Dragon's Dogma 2 before the September 2026 engine update (TU 3.2). DD2 Pak Bridge is the likely fix; otherwise look for an updated version of this mod.",
  ],
  [
    "unreadable",
    "it is password-protected. Unpack it with its password and put the unpacked folder in the mods folder instead.",
  ],
  // v0.3: a folder package holding archives of its own
  ["filesSkipped", "it holds 3 archives of its own; put each archive directly in the mods folder."],
  ["unreadable", "it is damaged (the central directory is cut off). Download it again from its mod page."],
];

describe("troubleLine() — the §9 trouble cards", () => {
  it("should open on the ember line for a package that can't be opened", () => {
    const trouble = troubleLine(
      {
        fileName: "Harbour Lights.7z",
        problems: [problem("unreadable", "The archive's table of contents is damaged.")],
      },
      8,
    );
    expect(trouble.ember).toBe(true);
    expect(trouble.line).toBe(
      "The stone couldn't open `Harbour Lights.7z`: the archive's table of contents is damaged. The other 8 packages are fine.",
    );
    expect(troubleLine({ fileName: "a.zip", problems: [problem("unreadable", "Gone.")] }, 1).line).toMatch(
      /The other package is fine\.$/,
    );
  });

  it("should say nothing loads only when nothing does", () => {
    const trouble = troubleLine(
      { fileName: "Readme Only.zip", problems: [problem("nothingToInstall", "only a readme inside.")] },
      3,
    );
    expect(trouble.ember).toBe(false);
    expect(trouble.line).toBe("`Readme Only.zip` opened, but holds nothing the game loads: only a readme inside.");
  });

  it("should word an outdated or partly skipped package as one that still opens", () => {
    const outdated = troubleLine(
      {
        fileName: "Old Road Armor Pack.zip",
        problems: [
          problem("filesSkipped", "1 file sits in a folder the game never reads."),
          problem("outdatedFormats", "3 loose files (mesh, tex) were made for Dragon's Dogma 2 before the update."),
        ],
      },
      3,
    );
    expect(outdated.kind).toBe("outdatedFormats");
    expect(outdated.line).toBe(
      "`Old Road Armor Pack.zip` opens, but part of it won't load: 3 loose files (mesh, tex) were made for Dragon's Dogma 2 before the update.",
    );
    expect(outdated.line).not.toMatch(/holds nothing/);
    const skipped = troubleLine(
      {
        fileName: "Lantern Glow.zip",
        problems: [problem("filesSkipped", "2 files sit in folders the game never reads.")],
      },
      3,
    );
    expect(skipped.line).toBe(
      "`Lantern Glow.zip` opens, but some of its files stay out: 2 files sit in folders the game never reads.",
    );
  });

  it("should state each card's verdict once, for every troubled package the mock holds", () => {
    const troubled = MOCK_PACKAGES.map((p) => p.summary).filter((s) => leadProblem(s) !== null);
    expect(troubled.length).toBeGreaterThanOrEqual(3);
    for (const pkg of troubled) {
      const line = troubleLine(pkg, troubled.length).line;
      expect({ line, verdicts: line.match(VERDICT)?.length }).toStrictEqual({ line, verdicts: 1 });
    }
  });

  it("should state each card's verdict once with the spine's own whys (rift-catalog)", () => {
    for (const [kind, why] of SPINE_WHYS) {
      const line = troubleLine({ fileName: "Harbour Lights.zip", problems: [problem(kind, why)] }, 4).line;
      expect({ line, verdicts: line.match(VERDICT)?.length }).toStrictEqual({ line, verdicts: 1 });
      // one colon between the verdict and the why
      expect(line).not.toMatch(/: [^:]*: /);
    }
  });

  it("should speak for the problem that put the package on the list, not merely the first", () => {
    const pkg = {
      problems: [problem("missingParent", "No Cloaks menu."), problem("nothingToInstall", "Nothing inside.")],
    };
    expect(leadProblem(pkg)?.kind).toBe("nothingToInstall");
    expect(TROUBLE_KINDS).toStrictEqual(["unreadable", "nothingToInstall", "outdatedFormats", "filesSkipped"]);
  });
});

describe("missingPackageTitle() — an installed row without a package", () => {
  it("should word an adopted install apart from one whose package left, by its origin and never its id", () => {
    expect(missingPackageTitle({ origin: "adopted" })).toBe(
      "Adopted: no package in your mods folder matches it. Uninstall still works: the journal knows every file.",
    );
    expect(missingPackageTitle({ origin: "riftstone" })).toBe(
      "The package is no longer in your mods folder. Uninstall still works: the journal knows every file.",
    );
  });
});
