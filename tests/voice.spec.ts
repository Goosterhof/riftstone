// The Error Manifesto, as one module: every RiftError kind the spine can
// reject with leaves services/voice.ts with a voice, an empathy line and at
// least one recovery — in every context the window can be in.

import { describe, expect, it } from "vitest";
import type { ProblemKind, RiftError } from "@/types/rift";
import { RiftFailure, asRiftFailure } from "@/services/rift";
import {
  TROUBLE_KINDS,
  failureDetails,
  leadProblem,
  missingPackageTitle,
  troubleLine,
  voice,
  voicedLine,
  type VoiceContext,
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

const CONTEXTS: VoiceContext[] = [
  "install",
  "uninstall",
  "scan",
  "package",
  "search",
  "reorder",
  "adopt",
  "switch",
  "roots",
  "game",
  "nexusCheck",
  "nexusKey",
  "nexusLink",
  "settings",
];

const fail = (kind: RiftError["kind"], detail: Record<string, string> = {}) => new RiftFailure({ kind, detail });

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
    expect(voiced.voice).toBe("We searched every Steam library on this PC and found no Dragon's Dogma 2 install.");
    expect(voiced.recovery.map((r) => r.kind)).toStrictEqual(["pickGameFolder", "retry"]);
  });

  it("should voice a failed crossing with the rollback count when the spine reports one", () => {
    const voiced = voice(
      fail("io", { path: "pak_mods/x0004-SunwardCuirassGilded.pak", reason: "the file is in use", rolledBack: "4" }),
      "install",
    );
    expect(voiced.inscription).toBe("THE CROSSING FAILED");
    expect(voicedLine(voiced)).toBe(
      "Couldn't write `pak_mods/x0004-SunwardCuirassGilded.pak`: the file is in use. Your game folder is exactly as it was: we put back the 4 files already placed. Close the game if it's running.",
    );
    expect(voiced.recovery.map((r) => r.label)).toStrictEqual(["Try again", "Copy details"]);
  });

  it("should never claim a rollback the spine did not report", () => {
    const voiced = voice(fail("io", { path: "pak_mods/a.pak" }), "install");
    expect(voiced.empathy).not.toMatch(/put back/);
  });

  it("should name a broken archive by its file name and offer the folder and a rescan", () => {
    const voiced = voice(fail("corruptArchive", { path: "D:\\Mods\\Harbour Lights.rar" }), "scan");
    expect(voiced.voice).toBe("We couldn't open `Harbour Lights.rar`: the archive ends early (truncated download?).");
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
      "We couldn't open `Harbour Lights.7z`: the archive's table of contents is damaged. The other 8 packages are fine.",
    );
    expect(troubleLine({ fileName: "a.zip", problems: [problem("unreadable", "Gone.")] }, 1).line).toMatch(
      /The other package is fine\.$/,
    );
  });

  it("should say nothing loads only when nothing does", () => {
    const trouble = troubleLine(
      { fileName: "Readme Only.zip", problems: [problem("nothingToInstall", "Only a readme inside.")] },
      3,
    );
    expect(trouble.ember).toBe(false);
    expect(trouble.line).toBe("`Readme Only.zip` opened, but holds nothing the game loads: Only a readme inside.");
  });

  it("should word an outdated or partly skipped package as one that still opens", () => {
    const outdated = troubleLine(
      {
        fileName: "Old Road Armor Pack.zip",
        problems: [
          problem("filesSkipped", "1 file in this package will not be installed."),
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
        problems: [problem("filesSkipped", "2 files in this package will not be installed.")],
      },
      3,
    );
    expect(skipped.line).toBe(
      "`Lantern Glow.zip` opens, but some of its files stay out: 2 files in this package will not be installed.",
    );
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
  it("should not call a Fluffy install that never matched a package one that left the library", () => {
    expect(missingPackageTitle({ optionId: "fluffy#Ox Cart Express" })).toBe(
      "Adopted from Fluffy: no package in your library matches it",
    );
    expect(missingPackageTitle({ optionId: "pkg-3f2a/Lantern Glow" })).toBe("The package is no longer in your library");
  });
});
