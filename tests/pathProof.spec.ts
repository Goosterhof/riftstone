// The IN FILE line's proof: the matched fragment of an archive path is its
// own run that never shrinks, the directories above it are elided in the
// middle, and the game-relative path starts at natives/ (not the archive's
// wrapper folders). Chromium's layout of the shrink order is measured by the
// render harness; here the cut itself is pinned.

import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import type { SearchResult } from "@/types/rift";
import { highlightRuns, pathProof } from "@/services/highlight";
import OptionCard from "@/components/OptionCard.vue";

const ALBA =
  "Vernworth Wardrobe Collection/ErmineCape-Variation1/natives/stm/character/_kit/_equipment/mantle/m310/mantle_310_f_alba.tex.251211553";

describe("pathProof()", () => {
  it("should cut the path around the lit fragment, eliding the middle and keeping the tail", () => {
    expect(pathProof(ALBA, ["alba"])).toStrictEqual({
      lead: "natives/stm/…/",
      before: "mantle_310_f_",
      fragment: "alba",
      after: ".tex.251211553",
    });
  });

  it("should anchor on the deepest segment that matched, and carry everything below it in the tail", () => {
    const proof = pathProof("natives/stm/appsystem/TopsWb/mesh/tops_0477_wb_f.pfb.18", ["topswb"]);
    expect(proof).toStrictEqual({
      lead: "natives/stm/…/",
      before: "",
      fragment: "TopsWb",
      after: "/mesh/tops_0477_wb_f.pfb.18",
    });
  });

  it("should span every lit token in the anchor segment, unlit runs between them included", () => {
    expect(pathProof("natives/stm/a/mantle_310_f_alba.tex", ["mantle", "alba"]).fragment).toBe("mantle_310_f_alba");
  });

  it("should keep short leads whole", () => {
    expect(pathProof("reframework/autorun/lantern.lua", ["lantern"]).lead).toBe("reframework/autorun/");
    expect(pathProof("lantern.lua", ["lantern"]).lead).toBe("");
  });

  it("should still show the path, unlit, when no token is in it", () => {
    expect(pathProof("natives/stm/a/b/c.tex", ["zzz"])).toStrictEqual({
      lead: "natives/stm/…/",
      before: "c.tex",
      fragment: "",
      after: "",
    });
  });
});

const HIT: SearchResult = {
  optionId: "o1",
  packageId: "p1",
  score: 1,
  name: "Ermine Cape - Variation 1",
  trail: [],
  bundle: null,
  packageName: "Vernworth Wardrobe Collection",
  packageFormat: "zip",
  rootId: "r1",
  author: null,
  version: null,
  category: null,
  descriptionSnippet: null,
  hasScreenshot: false,
  kinds: { pak: 0, loose: 2, reframework: 0, root: 0 },
  menuOnly: false,
  matched: ["file"],
  fileMatches: [ALBA, ALBA.replace("310", "311")],
  installed: null,
  update: null,
  conflicted: false,
  nexusCategory: null,
  nexusTags: [],
  adult: false,
};

describe("the IN FILE line", () => {
  it("should lead with the path proof when only a file path matched, the fragment lit in its own run", () => {
    const card = mount(OptionCard, { props: { hit: HIT, tokens: ["alba"], index: 0 } });
    const line = card.get(".card__match.is-first");
    expect(line.get(".card__why").text()).toBe("in file");
    expect(line.get(".proof__hit mark.rs-hit").text()).toBe("alba");
    expect(line.get(".proof__lead").text()).toBe("natives/stm/…/");
    expect(line.get(".proof__after").text()).toBe(".tex.251211553");
    expect(line.get(".proof").attributes("aria-label")).toBe(ALBA);
    expect(line.attributes("title")).toBe(HIT.fileMatches.join("\n"));
    expect(line.get(".card__more").text()).toBe("+1");
  });

  it("should not raise a path proof for a token the title already lights, nor for a lone digit", () => {
    // "Variation 1": the title carries both tokens; "1" also sits inside
    // ".pfb.18" and "260421070", which is noise, not proof.
    const hit = { ...HIT, matched: ["name", "file"] as SearchResult["matched"], fileMatches: [NOISE] };
    const card = mount(OptionCard, { props: { hit, tokens: ["variation", "1"], index: 0 } });
    expect(card.find(".card__match--path").exists()).toBe(false);
  });

  it("should prove a path hit the visible fields do not carry", () => {
    const hit = { ...HIT, matched: ["name", "file"] as SearchResult["matched"] };
    const card = mount(OptionCard, { props: { hit, tokens: ["ermine", "alba"], index: 0 } });
    expect(card.get(".card__match--path .proof__hit").text()).toBe("alba");
  });
});

const NOISE = "natives/stm/appsystem/TopsWb/mesh/tops_0477_wb_f.pfb.18/tops.mesh.260421070";

describe("short tokens", () => {
  it("should light a one-character token only where it stands alone", () => {
    expect(highlightRuns("Variation 1", ["1"]).filter((r) => r.hit)).toStrictEqual([{ text: "1", hit: true }]);
    expect(highlightRuns(".pfb.18 260421070", ["1"]).some((r) => r.hit)).toBe(false);
    expect(pathProof("ErmineCape-Variation1/x.tex", ["1"]).fragment).toBe("1");
  });
});
