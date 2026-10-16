// Token parity: the window's uno.config.ts carries the ruled tokens of
// docs/design/tokens.uno.ts (Design System #00015) VERBATIM. Every rs.<key>
// in riftColors must appear as `--rs-<key>: <same hex>` in riftRootCss, and
// both must equal the ruled file, key for key and value for value.

import { describe, expect, it } from "vitest";
import * as ruled from "../docs/design/tokens.uno";
import * as built from "../uno.config";

const rootVars = (css: string): Record<string, string> =>
  Object.fromEntries([...css.matchAll(/--(rs-[\w-]+):\s*([^;]+);/g)].map((m) => [m[1], m[2].trim()]));

describe("the ruled tokens", () => {
  it("should carry every ruled colour into the window's theme map, unchanged", () => {
    expect(built.riftColors).toStrictEqual(ruled.riftColors);
    expect(built.riftFonts).toStrictEqual(ruled.riftFonts);
  });

  it("should agree, colour for colour, between the theme map and the :root custom properties", () => {
    const vars = rootVars(built.riftRootCss);
    const keys = Object.keys(built.riftColors.rs);
    expect(keys.length).toBeGreaterThan(0);
    for (const [key, hex] of Object.entries(built.riftColors.rs)) {
      expect(vars[`rs-${key}`]?.toLowerCase(), `--rs-${key}`).toBe(hex.toLowerCase());
    }
  });

  it("should paste the ruled :root block verbatim", () => {
    expect(rootVars(built.riftRootCss)).toStrictEqual(rootVars(ruled.riftRootCss));
  });
});
