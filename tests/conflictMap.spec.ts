// v0.2 — WHO OVERRIDES WHOM: a clash whose pak carries table flags rift-pak
// does not know is marked "unconfirmed". The why is said once, as visible
// text under the map, and every marker points at it with aria-describedby:
// a keyboard or screen-reader user hears it, where a hover-only title would
// leave them guessing.

import { mount } from "@vue/test-utils";
import { afterEach, describe, expect, it } from "vitest";
import type { ConflictView } from "@/types/rift";
import ConflictMap from "@/components/ConflictMap.vue";
import { useNavigation } from "@/composables/useNavigation";
import { sleep, textOf, until, wakeOn } from "./harness";

afterEach(() => sleep());

const WATER = { optionId: "o-water", name: "Brine Water Clarity" };
const FOG = { optionId: "o-fog", name: "Sacred Arbor Fog Removal" };

const clash = (lower: number, unconfirmed: boolean): ConflictView => ({
  key: { kind: "hash", hash: { lower, upper: lower ^ 0x5f5f5f5f }, path: null },
  layers: [WATER, FOG],
  winner: FOG,
  unconfirmed,
});

const WHY = "can't be sure this hash is the one the game loads";

describe("the unconfirmed marker", () => {
  it("should describe every unconfirmed marker by the one visible note under the map", () => {
    const host = document.createElement("div");
    document.body.append(host);
    const map = mount(ConflictMap, {
      props: { conflicts: [clash(0x3a91c0de, false), clash(0x51be7702, true), clash(0x0c0ffee1, true)] },
      attachTo: host,
    });
    const markers = [...host.querySelectorAll(".cmap__row .cmap__unsure")];
    expect(markers).toHaveLength(2);
    const notes = [...host.querySelectorAll(".cmap__note")].map((n) => n.id);
    expect(notes).toHaveLength(1);
    const id = notes.join("");
    expect(id).not.toBe("");
    expect(markers.map((m) => m.getAttribute("aria-describedby"))).toStrictEqual([id, id]);
    expect(textOf(document.getElementById(id) ?? document.body)).toContain(WHY);
    // the note is text in the document, not a tooltip only a mouse can raise
    expect(markers.map((m) => m.getAttribute("title"))).toStrictEqual([null, null]);
    map.unmount();
  });

  it("should draw no note when every clash is confirmed", () => {
    const map = mount(ConflictMap, { props: { conflicts: [clash(0x3a91c0de, false)] } });
    expect(map.find(".cmap__unsure").exists()).toBe(false);
    expect(map.find(".cmap__note").exists()).toBe(false);
  });

  it("should point Your World's unconfirmed clash at its note", async () => {
    await wakeOn();
    useNavigation().go("world");
    await until(() => document.querySelector(".cmap__row .cmap__unsure") !== null, "the unconfirmed clash");
    const marker = document.querySelector(".cmap__row .cmap__unsure");
    const target = document.getElementById(marker?.getAttribute("aria-describedby") ?? "");
    expect(target?.classList.contains("cmap__note")).toBe(true);
    expect(textOf(target ?? document.body)).toContain(WHY);
  });
});
