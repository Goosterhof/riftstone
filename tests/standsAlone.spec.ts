// v0.3 (BUILD-SPEC § v0.3, ruling 1): the stone stands alone, with no
// reference to other mod managers. The forbidden words stand nowhere in the
// repo's text: not in code, comments, identifiers, test names, fixtures,
// strings or docs. This sweep reads every source file of the window (src/),
// every spec and the bench they stand on (tests/), the page around them
// (index.html) and its style config,
// the docs (docs/, README.md, CLAUDE.md), the privacy sweep (scripts/) and
// every Rust file of the spine and the crates, so CI and a fresh clone check
// what the machine-local sweep checks. It builds the words it looks for from
// their letters' codes, so the guard never becomes the reference it forbids.
// Pictures are text-blind here, as in every gate (CLAUDE.md, Privacy).

import { describe, expect, it } from "vitest";

const SOURCES = import.meta.glob<string>(
  [
    "../src/**/*.ts",
    "../src/**/*.vue",
    "../src/**/*.css",
    "../tests/**/*.ts",
    "../index.html",
    "../uno.config.ts",
    "../docs/**/*.{md,html,ts}",
    "../README.md",
    "../CLAUDE.md",
    "../scripts/**/*.py",
    "../src-tauri/**/*.rs",
    "../crates/**/*.rs",
  ],
  { query: "?raw", import: "default", eager: true },
);

const word = (...codes: number[]): string => String.fromCharCode(...codes);

/** The forbidden words: the first anywhere (inside identifiers too), the
 *  second only as a word of its own; case-blind. */
const FIRST_WORD = word(102, 108, 117, 102, 102, 121);
const SECOND_WORD = word(102, 109, 109);
const FORBIDDEN = new RegExp(`${FIRST_WORD}|\\b${SECOND_WORD}\\b`, "i");

const upper = (s: string): string => s.toUpperCase();
const title = (s: string): string => s.charAt(0).toUpperCase() + s.slice(1);

/** A glob key as a path from the gadget's root. Vite keys the specs' own
 *  folder relative to this file (`./harness.ts`), everything else from `../`.
 *  (A module never globs itself: this file is read by the git grep gate, and
 *  spells neither word, as its letters' codes show.) */
const pathOf = (key: string): string => key.replace(/^\.\.\//, "").replace(/^\.\//, "tests/");

/** Every `file:line` of the window's tree where a forbidden word stands. */
function hitsIn(sources: Record<string, string>): string[] {
  const hits: string[] = [];
  for (const [file, source] of Object.entries(sources)) {
    for (const [index, line] of source.split("\n").entries()) {
      if (FORBIDDEN.test(line)) hits.push(`${pathOf(file)}:${index + 1}`);
    }
  }
  return hits;
}

describe("the stone stands alone", () => {
  it("should read the whole window, every spec, the page around them, the docs, the sweep and the Rust", () => {
    const files = Object.keys(SOURCES).map(pathOf);
    for (const file of [
      "src/App.vue",
      "src/types/rift.ts",
      "src/services/rift.ts",
      "src/services/voice.ts",
      "src/services/mock/index.ts",
      "src/services/mock/library.ts",
      "src/services/mock/far.ts",
      "src/pages/PreflightPage.vue",
      "src/components/LibraryTree.vue",
      "tests/harness.ts",
      "tests/tree.spec.ts",
      "index.html",
      "uno.config.ts",
      "docs/BUILD-SPEC.md",
      "docs/WINDOW-CONTRACT.md",
      "docs/design/audition.html",
      "docs/design/tokens.uno.ts",
      "README.md",
      "CLAUDE.md",
      "scripts/privacy-sweep.py",
      "src-tauri/src/lib.rs",
      "src-tauri/src/commands/overview.rs",
      "crates/rift-core/src/lib.rs",
      "crates/rift-deploy/src/journal.rs",
      "crates/rift-nexus/src/lib.rs",
    ]) {
      expect(files).toContain(file);
    }
    expect(files.filter((f) => f.endsWith(".vue")).length).toBeGreaterThan(40);
    expect(files.filter((f) => f.startsWith("tests/")).length).toBeGreaterThan(20);
    expect(files.filter((f) => f.startsWith("src-tauri/") && f.endsWith(".rs")).length).toBeGreaterThan(20);
    expect(files.filter((f) => f.startsWith("crates/") && f.endsWith(".rs")).length).toBeGreaterThan(40);
    // the args file holds this machine's private paths: the sweep never reads it, nor anything outside these globs
    expect(files.some((f) => f.startsWith("scripts/") && !f.endsWith(".py"))).toBe(false);
  });

  it("should carry none of the forbidden words", () => {
    expect(hitsIn(SOURCES)).toStrictEqual([]);
  });

  it("should catch the first word in any case and inside an identifier, and the second only as a word of its own", () => {
    expect(
      hitsIn({
        "a.ts": `const shelf = "${title(FIRST_WORD)} Mods";`,
        "b.ts": `function read${title(FIRST_WORD)}Ledger(): void {}`,
        "c.vue": `<p>${upper(FIRST_WORD)}</p>`,
        "d.ts": `// ${upper(SECOND_WORD)} named it`,
        "e.ts": `const label = "${SECOND_WORD}-adopted";`,
        "f.ts": `const of${SECOND_WORD}ap = readOffsetMap();\nconst fine = true;`,
      }),
    ).toStrictEqual(["a.ts:1", "b.ts:1", "c.vue:1", "d.ts:1", "e.ts:1"]);
  });
});
