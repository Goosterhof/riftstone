// v0.3 (R16.3-R16.5) — the package view's tree speaks the library tree's
// grammar: option rows lie on the void with a 2 px rail in their state colour
// (a menu or a bundle installs nothing, so it carries none), each level hangs
// under a 1 px guide at its parent's chevron, and the crumb's landing is the
// row that holds the focus, drawn by the ring, not by a second rail.

import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import type { TreeNode } from "@/types/rift";
import PackageTree from "@/components/PackageTree.vue";

const node = (over: Partial<TreeNode>): TreeNode => ({
  optionId: "o-lantern",
  label: "Lantern Glow Recolor",
  kind: "option",
  menuOnly: false,
  hasScreenshot: false,
  installed: false,
  conflicted: false,
  kinds: { pak: 1, loose: 0, reframework: 0, root: 0 },
  children: [],
  ...over,
});

const TREE: TreeNode[] = [
  node({
    optionId: null,
    label: "Vernworth Cloaks Collection",
    kind: "bundle",
    installed: true,
    children: [
      node({ optionId: "o-amber", label: "Amber Cloak", installed: true }),
      node({ optionId: "o-ash", label: "Ash Cloak" }),
      node({ optionId: "o-ember", label: "Ember Cloak", conflicted: true, installed: true }),
      node({ optionId: "o-menu", label: "Cloak colours (Menu Option)", menuOnly: true }),
    ],
  }),
];

const railOf = (row: Element): string | null =>
  [...row.classList].find((c) => c.startsWith("rail--"))?.slice("rail--".length) ?? null;

describe("The package view's tree (R16.3-R16.5)", () => {
  it("should rail each leaf in its state colour and leave menus and bundles bare", () => {
    const tree = mount(PackageTree, { props: { nodes: TREE } });
    const rows = tree.findAll(".ptr__row").map((r) => [r.find(".ptr__label").text(), railOf(r.element)]);
    expect(rows).toStrictEqual([
      ["Vernworth Cloaks Collection", null],
      ["Amber Cloak", "world"],
      ["Ash Cloak", "rift"],
      ["Ember Cloak", "ember"],
      ["Cloak colours (Menu Option)", null],
    ]);
    tree.unmount();
  });

  it("should hang a nested level under a guide at its parent's chevron, 16 px a level", () => {
    const tree = mount(PackageTree, { props: { nodes: TREE } });
    const group = tree.find("ul.ptr:not(.ptr--root)");
    // the parent row's chevron starts at its 12 px inset and is 10 px wide: the guide runs down its middle
    expect(group.attributes("style")).toContain("--guide-x: 17px");
    expect(tree.findAll(".ptr__row").map((r) => r.attributes("style"))).toStrictEqual([
      "padding-left: 12px;",
      "padding-left: 28px;",
      "padding-left: 28px;",
      "padding-left: 28px;",
      "padding-left: 28px;",
    ]);
    tree.unmount();
  });

  it("should frame only the open option as selected, never a bundle while nothing is open (R16.5)", async () => {
    const tree = mount(PackageTree, { props: { nodes: TREE } });
    expect(tree.findAll(".ptr__row.is-sel")).toHaveLength(0);
    await tree.setProps({ selectedId: "o-ash" });
    expect(tree.findAll(".ptr__row.is-sel").map((r) => r.find(".ptr__label").text())).toStrictEqual(["Ash Cloak"]);
    tree.unmount();
  });

  it("should show a CONFLICT chip on a conflicted leaf, as the library tree does", () => {
    const tree = mount(PackageTree, { props: { nodes: TREE } });
    const ember = tree.findAll(".ptr__row").find((r) => r.find(".ptr__label").text() === "Ember Cloak");
    expect(ember?.text()).toContain("CONFLICT");
    // a bundle installs nothing: no state chip and no kind badges on its row (R16.3)
    const bundle = tree.findAll(".ptr__row").at(0);
    expect(bundle?.find(".ptr__meta").exists()).toBe(false);
    tree.unmount();
  });
});
