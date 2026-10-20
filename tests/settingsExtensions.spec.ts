// The version question's settings files (#00015 §18.18, the General's round 2 on PR #2): a lone
// shared name that is a settings or text file never asks. The spine, the mock and the window
// contract each name the list; this spec holds the three to one list, so none drifts unseen.

import { describe, expect, it } from "vitest";
import { SETTINGS_EXTENSIONS } from "@/services/mock/versions";

const FILES = import.meta.glob<string>(["../src-tauri/src/family.rs", "../docs/WINDOW-CONTRACT.md"], {
  query: "?raw",
  import: "default",
  eager: true,
});

function read(path: string): string {
  const found = Object.entries(FILES).find(([at]) => at === `../${path}`);
  if (!found) throw new Error(`The spec could not read ${path}`);
  return found[1];
}

/** The spine's list: the string literals of `SETTINGS_EXTENSIONS` in src-tauri/src/family.rs. */
function spineList(): string[] {
  const source = read("src-tauri/src/family.rs");
  const body = /const SETTINGS_EXTENSIONS: &\[&str\] = &\[([^\]]*)\]/.exec(source)?.[1];
  if (body === undefined) throw new Error("src-tauri/src/family.rs names no SETTINGS_EXTENSIONS");
  return [...body.matchAll(/"([^"]+)"/g)].map((m) => String(m[1]));
}

/** The contract's list: the backticked extensions in § v0.5's question rule. */
function contractList(): string[] {
  const contract = read("docs/WINDOW-CONTRACT.md");
  const rule = /no settings or text file \(([^)]*)\)/.exec(contract.replace(/\s+/g, " "))?.[1];
  if (rule === undefined) throw new Error("WINDOW-CONTRACT names no settings files for the question");
  return [...rule.matchAll(/`\.([a-z]+)`/g)].map((m) => String(m[1]));
}

describe("The version question's settings files (General, PR #2)", () => {
  it("should name one list in the spine, the mock and the window contract", () => {
    const mock = [...SETTINGS_EXTENSIONS].sort();
    expect(spineList().sort()).toStrictEqual(mock);
    expect(contractList().sort()).toStrictEqual(mock);
    expect(mock.length).toBeGreaterThan(0);
  });
});
