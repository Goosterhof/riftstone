// v0.3 (BUILD-SPEC § v0.3, ruling 1): the stone stands alone. The mod manager
// it once lived beside is named nowhere in the window or its specs: not in
// code, comments, identifiers, test names, fixtures or strings. This sweep
// reads every source file of the window (src/), every spec and the bench they
// stand on (tests/), the page around them (index.html) and its style config,
// and it builds the words it looks for from their letters' codes, so the
// guard never becomes the reference it forbids.

import { describe, expect, it } from "vitest";

const SOURCES = import.meta.glob<string>(
  ["../src/**/*.ts", "../src/**/*.vue", "../src/**/*.css", "../tests/**/*.ts", "../index.html", "../uno.config.ts"],
  { query: "?raw", import: "default", eager: true },
);

const word = (...codes: number[]): string => String.fromCharCode(...codes);

/** The old manager's name anywhere (inside identifiers too), and its initials
 *  as a word of their own; case-blind. */
const OLD_NAME = word(102, 108, 117, 102, 102, 121);
const OLD_INITIALS = word(102, 109, 109);
const FORBIDDEN = new RegExp(`${OLD_NAME}|\\b${OLD_INITIALS}\\b`, "i");

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
  it("should read the whole window, every spec and the page around them", () => {
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
    ]) {
      expect(files).toContain(file);
    }
    expect(files.filter((f) => f.endsWith(".vue")).length).toBeGreaterThan(40);
    expect(files.filter((f) => f.startsWith("tests/")).length).toBeGreaterThan(20);
  });

  it("should name the old manager nowhere: neither its name nor its initials, on any line", () => {
    expect(hitsIn(SOURCES)).toStrictEqual([]);
  });

  it("should catch the name in any case and inside an identifier, and the initials only as a word", () => {
    expect(
      hitsIn({
        "a.ts": `const shelf = "${title(OLD_NAME)} Mods";`,
        "b.ts": `function read${title(OLD_NAME)}Ledger(): void {}`,
        "c.vue": `<p>${upper(OLD_NAME)}</p>`,
        "d.ts": `// ${upper(OLD_INITIALS)} named it`,
        "e.ts": `const label = "${OLD_INITIALS}-adopted";`,
        "f.ts": "const offmmap = readOffsetMap();\nconst fine = true;",
      }),
    ).toStrictEqual(["a.ts:1", "b.ts:1", "c.vue:1", "d.ts:1", "e.ts:1"]);
  });
});
