// One narrator (Chaos #00129 D2). The stone speaks of itself in the third
// person, as §2's pawn would: "The stone couldn't open…", "The stone searched
// every Steam library…". "We" is a support desk, and the pawn has no support
// team. Two guards: every line the voice can say, and a sweep of the window's
// own source for the words a desk uses, so the next line can't slip back.

import { describe, expect, it } from "vitest";
import type { ProblemKind, RiftError } from "@/types/rift";
import { RiftFailure } from "@/services/rift";
import { TROUBLE_KINDS, troubleLine, voice, type VoiceContext } from "@/services/voice";
import { askNexusToast, recoverySentence } from "@/services/ledger";

/** The words a support desk speaks in. "US" (capitals) is a country, not a desk. */
const DESK = /\b(?:[Ww]e|[Ww]e're|[Ww]e'll|[Ww]e've|[Uu]s|[Oo]ur|[Oo]urs)\b/;

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
  "nexusFind",
  "settings",
  "parked",
];

/** Details that walk each handler down its other branches. */
const DETAILS: Record<string, string>[] = [
  {},
  { path: "pak_mods/zrift-0004-SunwardCuirassGilded.pak", reason: "the file is in use", rolledBack: "4" },
  { reason: "Dragon's Dogma 2 is running and holds natives/stm/a.tex open" },
  { detail: "connection refused" },
  { detail: "429 rate limit: try again in 30 seconds" },
  { detail: "HTTP 401 Unauthorized" },
  { detail: "Nexus answered in a shape the stone could not use" },
  { gamePath: "natives/stm/cape_301.mesh.260421070", act: "discard", reason: "could not remove x: in use" },
  { gamePath: "natives/stm/cape_301.mesh.260421070", act: "putBack" },
];

function everyLine(): string[] {
  const lines: string[] = [];
  for (const kind of KINDS) {
    for (const context of CONTEXTS) {
      for (const detail of DETAILS) {
        const voiced = voice(new RiftFailure({ kind, detail }), context);
        lines.push(voiced.voice, voiced.empathy, voiced.inscription ?? "", ...voiced.recovery.map((r) => r.label));
      }
    }
  }
  const kinds: ProblemKind[] = [...TROUBLE_KINDS];
  for (const kind of kinds) {
    lines.push(troubleLine({ fileName: "Harbour Lights.7z", problems: [{ kind, message: "" }] }, 3).line);
    lines.push(troubleLine({ fileName: "Harbour Lights.7z", problems: [{ kind, message: "a why." }] }, 1).line);
  }
  lines.push(troubleLine({ fileName: "Readme Only.zip", problems: [] }, 2).line);
  for (const action of ["install", "uninstall", "reorder", "restoreParked"] as const) {
    lines.push(recoverySentence({ action, optionName: "Lantern Glow Recolor", filesRolledBack: 3 }));
  }
  const offline = voice(new RiftFailure({ kind: "nexus", detail: { detail: "connection refused" } }), "nexusCheck");
  lines.push(askNexusToast({ before: 4, after: 4, hadLabels: false, failure: offline }).line);
  return lines;
}

describe("one narrator: the stone", () => {
  it("should never speak as a support desk in any line the voice can say", () => {
    const desk = everyLine().filter((line) => DESK.test(line));
    expect(desk).toStrictEqual([]);
  });

  it("should open a failed archive and a missing game with the stone as the subject", () => {
    const archive = voice(new RiftFailure({ kind: "corruptArchive", detail: { path: "D:\\Mods\\a.rar" } }), "scan");
    expect(archive.voice).toMatch(/^The stone couldn't open `a\.rar`: /);
    expect(voice(new RiftFailure({ kind: "gameNotFound", detail: {} }), "game").voice).toMatch(
      /^The stone searched every Steam library/,
    );
  });

  it("should say the stone only once in a line about a locked or foreign archive", () => {
    for (const kind of ["encrypted", "unsupportedFormat", "unreadable"] as const) {
      const line = voice(new RiftFailure({ kind, detail: { path: "a.7z" } }), "scan").voice;
      expect({ line, stones: line.match(/the stone/gi)?.length }).toStrictEqual({ line, stones: 1 });
    }
  });
});

// ---------------------------------------------------------------------------
// The window's own words, read from source. Comments are the builders'
// voice, not the stone's, and the mock is dev tooling: both are left out.
// ---------------------------------------------------------------------------

const SOURCES = import.meta.glob<string>(["../src/**/*.ts", "../src/**/*.vue", "!../src/services/mock/**"], {
  query: "?raw",
  import: "default",
  eager: true,
});

const blank = (match: string) => match.replace(/[^\n]/g, " ");

/** The source with its comments blanked, so line numbers still point home. */
function spoken(source: string): string {
  return source
    .replace(/<!--[\s\S]*?-->/g, blank)
    .replace(/\/\*[\s\S]*?\*\//g, blank)
    .replace(/(^|[^:\\])\/\/.*$/gm, (match, lead: string) => lead + blank(match.slice(lead.length)));
}

describe("the window's source", () => {
  it("should be read at all (the glob found the window)", () => {
    const files = Object.keys(SOURCES);
    expect(files.some((f) => f.endsWith("/services/voice.ts"))).toBe(true);
    expect(files.some((f) => f.endsWith("/components/WakeScreen.vue"))).toBe(true);
    expect(files.some((f) => f.includes("/services/mock/"))).toBe(false);
  });

  it("should hold no line where the stone speaks as a support desk", () => {
    const desk: string[] = [];
    for (const [file, source] of Object.entries(SOURCES)) {
      for (const [index, line] of spoken(source).split("\n").entries()) {
        if (DESK.test(line)) desk.push(`${file.replace("../", "")}:${index + 1}`);
      }
    }
    expect(desk).toStrictEqual([]);
  });

  it("should catch a desk line it is shown, and pass over a comment", () => {
    expect(DESK.test(spoken(`const a = "We couldn't open it";`))).toBe(true);
    expect(DESK.test(spoken(`<p>we'll check</p>`))).toBe(true);
    expect(DESK.test(spoken(`// we keep this\nconst b = 1; /* our note */\n<!-- us -->`))).toBe(false);
    expect(DESK.test(spoken(`const url = "https://www.nexusmods.com/"; // we`))).toBe(false);
    expect(DESK.test(spoken(`const status = "your hour"; // wE`))).toBe(false);
  });
});
