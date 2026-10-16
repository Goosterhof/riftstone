// §11.1 step 3 — the seam as a gauge (Chaos #00129 D6). The answered rest
// carries a quantity as a length: how much of the library answered, floored
// and on a log so one answer and thirty both read. Nothing in JS animates it;
// a facet snaps it, and the flare grows to it, not to the pillar's foot.

import { afterEach, describe, expect, it } from "vitest";
import { SEAM_FLOOR, seamReach } from "@/services/seam";
import { useSearch } from "@/composables/useSearch";
import { useStone } from "@/composables/useStone";
import { sleep, text, textOf, typeQuery, until, wakeOn } from "./harness";

afterEach(() => sleep());

function pillar(): HTMLElement {
  const el = document.querySelector<HTMLElement>(".pillar");
  if (!el) throw new Error("The stone is not standing");
  return el;
}
const reachOnStone = (): string => pillar().style.getPropertyValue("--seam-reach");

/** The State facet's radio by its label (a radio, not a button). */
function stateRadio(label: string): HTMLInputElement {
  const found = [...document.querySelectorAll<HTMLInputElement>('input[name="rift-state"]')].find(
    (r) => textOf(r.closest("label") ?? r).trim() === label,
  );
  if (!found) throw new Error(`No State facet "${label}"`);
  return found;
}

describe("the seam as a gauge (§11.1 step 3)", () => {
  it("should map answers to a floored log length", () => {
    expect(SEAM_FLOOR).toBe(0.1);
    expect([1, 2, 3, 7, 30, 120, 300].map((n) => seamReach(n, 300))).toStrictEqual([
      0.209, 0.273, 0.319, 0.428, 0.642, 0.856, 1,
    ]);
    expect([seamReach(1, 32), seamReach(6, 32)]).toStrictEqual([0.278, 0.601]);
    expect(seamReach(1, 5000)).toBe(0.173);
    expect(seamReach(0, 300)).toBe(0);
    expect(seamReach(5, 0)).toBe(1);
    expect(seamReach(40, 32)).toBe(1);
  });

  it("should light the seam to the gauge length for a standing query", async () => {
    await wakeOn();
    await until(() => !!useStone().overview.value, "the library");
    await typeQuery("cloak");
    await until(() => pillar().dataset.seam === "answered", "the answered seam");
    const n = useSearch().results.value.length;
    const N = useStone().overview.value?.library.options ?? 0;
    expect({ n: n > 0, reach: reachOnStone() }).toStrictEqual({ n: true, reach: String(seamReach(n, N)) });
    expect(pillar().style.getPropertyValue("--seam-answer")).toBe("1");
  });

  it("should carry no reach when silent, at rest, or scanning", async () => {
    await wakeOn();
    await typeQuery("zzqx-nothing-answers");
    await until(() => pillar().dataset.seam === "silent", "the silent seam");
    expect(reachOnStone()).toBe("0");
    await typeQuery("");
    await until(() => pillar().dataset.seam === "rest", "the seam at rest");
    expect(reachOnStone()).toBe("0");
    sleep();
    await wakeOn("scanning");
    await until(() => text().includes("READING THE RIFT"), "the scan");
    expect(reachOnStone()).toBe("0");
  });

  it("should snap the length on a facet change without a new flare", async () => {
    await wakeOn();
    await typeQuery("cloak");
    await until(() => pillar().dataset.seam === "answered", "the answered seam");
    const before = reachOnStone();
    const seq = useSearch().answerSeq.value;
    const installed = stateRadio("Installed");
    installed.checked = true;
    installed.dispatchEvent(new Event("change"));
    await until(() => reachOnStone() !== before, "the shorter length");
    // a facet re-run calls run() without `answer`, so the flare key must not move
    expect({ seq: useSearch().answerSeq.value, shorter: Number(reachOnStone()) < Number(before) }).toStrictEqual({
      seq,
      shorter: true,
    });
  });

  it("should size the answered layer and the flare by the reach, and transition only opacity", () => {
    const src = Object.values(
      import.meta.glob<string>("../src/components/RiftstonePillar.vue", {
        query: "?raw",
        import: "default",
        eager: true,
      }),
    ).join("");
    const block = (sel: string): string => {
      const at = src.indexOf(`\n${sel} {`);
      if (at < 0) throw new Error(`No ${sel} rule`);
      return src.slice(at, src.indexOf("}", at));
    };
    for (const sel of [".seam-answered", ".seam-flare"]) {
      const rule = block(sel);
      expect({
        sel,
        height: rule.includes("height: calc((100% - var(--seam-top)) * var(--seam-reach))"),
      }).toStrictEqual({ sel, height: true });
      expect({ sel, bottom: rule.includes("bottom: 0") }).toStrictEqual({ sel, bottom: false });
      expect({ sel, animatesHeight: /transition[^;]*height/.test(rule) }).toStrictEqual({
        sel,
        animatesHeight: false,
      });
    }
  });
});
