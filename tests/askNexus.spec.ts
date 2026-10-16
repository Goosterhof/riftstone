// v0.2 — Identify & check (useLabels.askNexus): one press under the NEXUS
// LABELS facets identifies the unlinked packages by MD5, THEN asks Nexus
// about everything (the check is what brings the labels home), and says in
// one toast what it learned (ledger askNexusToast). An archive Nexus did not
// recognise once is never hashed again, and after a press that learned
// nothing the button gives way to the NEXUS page's Link… rows.

import { afterEach, describe, expect, it, vi } from "vitest";
import type { RiftError } from "@/types/rift";
import { askNexusToast } from "@/services/ledger";
import { RiftFailure } from "@/services/rift";
import { voice } from "@/services/voice";
import { MOCK_PACKAGES } from "@/services/mock/library";
import { useFarWorld } from "@/composables/useFarWorld";
import { useLabels } from "@/composables/useLabels";
import { useNavigation } from "@/composables/useNavigation";
import { useToasts } from "@/composables/useToasts";
import { button, hasButton, sleep, spine, text, textOf, until, wakeOn, idle } from "./harness";

/** A press runs ~2 s on the mock's timers (hashing 400 ms, 260 ms per linked mod). */
const ASKING_MS = 8_000;
const SLOW_TEST_MS = 20_000;

/** Let a press and the events it raised land before the next spec wakes. */
async function quiet(): Promise<void> {
  const far = useFarWorld();
  await until(
    () => !useLabels().asking.value && !far.checking.value && !far.identifying.value,
    "the stone at rest",
    ASKING_MS,
  ).catch(() => undefined);
  await new Promise((resolve) => setTimeout(resolve, 80));
}

afterEach(async () => {
  await quiet();
  vi.restoreAllMocks();
  sleep();
});

/** The five packages with options and no Nexus link (./mock/library.ts). */
const UNLINKED = [
  "REFramework",
  "Ox Cart Express",
  "Brine Water Clarity",
  "Sacred Arbor Fog Removal",
  "Old Road Armor Pack",
];

const idOf = (name: string): string => {
  const pkg = MOCK_PACKAGES.find((p) => p.summary.displayName === name);
  if (!pkg) throw new Error(`The mock library has no ${name}`);
  return pkg.summary.id;
};

const toastLines = (): string[] => useToasts().toasts.value.map((t) => t.line);
const lastToast = (): { tone: string; line: string } | null => {
  const last = useToasts().toasts.value.at(-1);
  return last ? { tone: last.tone, line: last.line } : null;
};
const quietLine = (): string =>
  textOf(document.querySelector("#nexus-labels .flab__quiet") ?? document.body)
    .replace(/\s+/g, " ")
    .trim();
const chipNames = (): string[] =>
  [...document.querySelectorAll("#nexus-labels .lchip__name")].map((n) => textOf(n).trim());

async function askOnce(toastStart: string): Promise<void> {
  await until(() => hasButton("Identify & check") && idle(button("Identify & check")), "Identify & check");
  button("Identify & check").click();
  await until(() => toastLines().some((l) => l.startsWith(toastStart)), `the toast "${toastStart}…"`, ASKING_MS);
  await until(() => !useLabels().asking.value, "the press done");
}

describe("Identify & check", () => {
  it(
    "should identify the unlinked packages by MD5 first, then check Nexus, and toast what it learned",
    async () => {
      await wakeOn();
      const identify = vi.spyOn(spine(), "identifyPackages");
      const check = vi.spyOn(spine(), "checkUpdates");
      await until(() => quietLine().startsWith("5 packages"), "five unlabeled packages");

      button("Identify & check").click();
      await until(() => text().includes("asking Nexus · "), "the press asking Nexus", ASKING_MS);
      await until(() => toastLines().some((l) => l.startsWith("Nexus named")), "the learned toast", ASKING_MS);

      expect(identify).toHaveBeenCalledTimes(1);
      expect([...identify.mock.calls[0][0]].sort()).toStrictEqual(UNLINKED.map(idOf).sort());
      expect(check).toHaveBeenCalledTimes(1);
      expect(identify.mock.invocationCallOrder[0]).toBeLessThan(check.mock.invocationCallOrder[0]);
      expect(lastToast()).toStrictEqual({
        tone: "rift",
        line: "Nexus named 1 more package. 4 packages Nexus doesn't know stay unlabeled: find them by name on the NEXUS page.",
      });
      expect(useLabels().facets.value?.unlabeledPackages).toBe(4);
      await until(() => quietLine().startsWith("4 packages"), "the quiet line counting four");
      expect(quietLine()).toBe("4 packages carry no Nexus labels yet. A label filter hides them.");
      // the one Nexus knew came home with its labels: its tag is a chip now
      expect(chipNames()).toContain("Environment");
    },
    SLOW_TEST_MS,
  );

  it(
    "should name the packages it learned on a first launch, when no package carried labels before",
    async () => {
      await wakeOn("fresh");
      await until(() => quietLine().startsWith("9 packages"), "nine unlabeled packages");
      expect(quietLine()).toBe(
        "9 packages carry no Nexus labels yet. The stone learns each mod's category and tags when it asks Nexus about updates.",
      );
      expect(chipNames()).toStrictEqual([]);

      await askOnce("Nexus named");
      expect(lastToast()).toStrictEqual({
        tone: "rift",
        line: "Nexus named 5 packages. 4 packages Nexus doesn't know stay unlabeled: find them by name on the NEXUS page.",
      });
      expect(chipNames()).toContain("Armour");
    },
    SLOW_TEST_MS,
  );

  it(
    "should never hash the same bytes twice: a second press only checks, learns nothing, and gives way to the NEXUS page",
    async () => {
      await wakeOn();
      const identify = vi.spyOn(spine(), "identifyPackages");
      const check = vi.spyOn(spine(), "checkUpdates");
      await askOnce("Nexus named");
      expect(useLabels().exhausted.value).toBe(false);

      await askOnce("Nexus didn't recognise");
      expect(identify).toHaveBeenCalledTimes(1);
      expect(check).toHaveBeenCalledTimes(2);
      expect(lastToast()).toStrictEqual({
        tone: "rift",
        line: "Nexus didn't recognise any of the 4 packages by MD5 — they may be repacks or come from elsewhere. Find them by name on the NEXUS page and their labels follow.",
      });
      expect(useLabels().exhausted.value).toBe(true);
      expect(hasButton("Identify & check")).toBe(false);
      expect(text()).toContain("Nexus knows none of the rest by their bytes.");

      button("Find them by name on the NEXUS page ▸").click();
      expect(useNavigation().page.value).toBe("nexus");
    },
    SLOW_TEST_MS,
  );

  it(
    "should say the far world is out of reach and that no labels were learned, and count no package as a miss",
    async () => {
      await wakeOn("offline");
      const identify = vi.spyOn(spine(), "identifyPackages");
      await askOnce("The stone can't reach Nexus");
      expect(lastToast()).toStrictEqual({
        tone: "ember",
        line: "The stone can't reach Nexus right now, so no labels were learned. Try again when you're online.",
      });
      expect(useLabels().facets.value?.unlabeledPackages).toBe(5);
      expect(useLabels().exhausted.value).toBe(false);

      // offline is not a miss: the next press hashes all five again
      await until(() => idle(button("Identify & check")), "the button back");
      button("Identify & check").click();
      await until(() => identify.mock.calls.length === 2, "the second identify");
      expect([...identify.mock.calls[1][0]].sort()).toStrictEqual(UNLINKED.map(idOf).sort());
    },
    SLOW_TEST_MS,
  );
});

const failure = (kind: RiftError["kind"], detail: Record<string, string>) =>
  voice(new RiftFailure({ kind, detail }), "nexusCheck");

describe("askNexusToast()", () => {
  it("should count what a press learned, said for a library that had labels and for one that had none", () => {
    expect(askNexusToast({ before: 5, after: 4, hadLabels: true, failure: null })).toStrictEqual({
      tone: "rift",
      line: "Nexus named 1 more package. 4 packages Nexus doesn't know stay unlabeled: find them by name on the NEXUS page.",
      learned: 1,
    });
    expect(askNexusToast({ before: 9, after: 1, hadLabels: false, failure: null }).line).toBe(
      "Nexus named 8 packages. 1 package Nexus doesn't know stays unlabeled: find it by name on the NEXUS page.",
    );
    expect(askNexusToast({ before: 2, after: 0, hadLabels: true, failure: null }).line).toBe(
      "Nexus named 2 more packages. Every package carries its labels now.",
    );
  });

  it("should say plainly when Nexus recognised nothing, and point at finding them by name", () => {
    expect(askNexusToast({ before: 4, after: 4, hadLabels: true, failure: null })).toStrictEqual({
      tone: "rift",
      line: "Nexus didn't recognise any of the 4 packages by MD5 — they may be repacks or come from elsewhere. Find them by name on the NEXUS page and their labels follow.",
      learned: 0,
    });
  });

  it("should never claim a lesson from a failed press: offline in its own words, anything else in the voice's", () => {
    const offline = failure("nexus", { detail: "connection refused" });
    expect(askNexusToast({ before: 5, after: 3, hadLabels: true, failure: offline })).toStrictEqual({
      tone: "ember",
      line: "The stone can't reach Nexus right now, so no labels were learned. Try again when you're online.",
      learned: 0,
    });
    const rationed = failure("nexus", { detail: "429 rate limit" });
    expect(askNexusToast({ before: 5, after: 5, hadLabels: true, failure: rationed }).line).toBe(
      "Nexus has rationed the stone for now: 429 rate limit. No labels were learned this time.",
    );
  });
});
