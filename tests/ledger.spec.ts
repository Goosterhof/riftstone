// v0.2 — the honest sentences (services/ledger.ts): what THE RETURN will do
// to each file, and what an uninstall, an adoption or a putting-back actually
// did. Every count the spine returns is said out loud, a zero says nothing,
// and THE RETURN's heading counts only the rows that move a file: "already
// gone" is its own clause, never a file that "leaves your game".

import { afterEach, describe, expect, it } from "vitest";
import type { AdoptResultView, RemovalAction, UninstallResultView } from "@/types/rift";
import {
  adoptSummary,
  parkedRestoreSummary,
  recoverySentence,
  removalWords,
  returnHeading,
  returnedSummary,
  unlabeledLine,
} from "@/services/ledger";
import { MOCK_PACKAGES } from "@/services/mock/library";
import { useNavigation } from "@/composables/useNavigation";
import { useToasts } from "@/composables/useToasts";
import { useWorld } from "@/composables/useWorld";
import { button, hasButton, sleep, text, textAt, textOf, until, wakeOn } from "./harness";

afterEach(() => sleep());

const SKY = "natives/stm/environment/sky/sky_noon.tex.251211553";

const uninstalled = (counts: Partial<UninstallResultView>): UninstallResultView => ({
  optionId: "o1",
  filesRemoved: 0,
  filesRestored: 0,
  filesKept: 0,
  filesHandedDown: 0,
  parked: 0,
  restartNeeded: false,
  ...counts,
});

const adopted = (counts: Partial<AdoptResultView>): AdoptResultView => ({
  adopted: 0,
  alreadyAdopted: 0,
  missingFiles: 0,
  refusedFiles: 0,
  sharedFiles: 0,
  unmatched: [],
  ...counts,
});

const removals = (...actions: RemovalAction[]) => actions.map((action) => ({ action }));
const capeMinus = () => document.querySelector<HTMLButtonElement>('[aria-label="Uninstall Ermine Cape - Variation 2"]');
const DELETE: RemovalAction = { kind: "delete" };
const GONE: RemovalAction = { kind: "alreadyGone" };

describe("returnedSummary()", () => {
  it("should say every count the uninstall reported, in order, the handed-down clause last", () => {
    expect(
      returnedSummary(
        uninstalled({
          filesRemoved: 3,
          filesRestored: 1,
          filesKept: 2,
          filesHandedDown: 1,
          parked: 1,
          restartNeeded: true,
        }),
      ),
    ).toBe(
      "3 files removed; 1 earlier file put back; 2 files left in place (changed since, or the game's own); 1 file handed down to the install below, which owns it now. 1 file was kept aside instead of deleted: put it back or discard it under PARKED on Your World. Restart the game to see the change.",
    );
  });

  it("should leave a zero unsaid and agree its words with the count", () => {
    expect(returnedSummary(uninstalled({ filesRemoved: 1 }))).toBe("1 file removed.");
    expect(returnedSummary(uninstalled({ filesHandedDown: 2, parked: 2 }))).toBe(
      "2 files handed down to the install below, which owns them now. 2 files were kept aside instead of deleted: put them back or discard them under PARKED on Your World.",
    );
    expect(returnedSummary(uninstalled({}))).toBe("Nothing was left to remove.");
    expect(returnedSummary(uninstalled({ parked: 1 }))).toBe(
      "Nothing was left to remove. 1 file was kept aside instead of deleted: put it back or discard it under PARKED on Your World.",
    );
  });
});

describe("adoptSummary()", () => {
  it("should say what came into the journal and what stayed Fluffy's", () => {
    expect(adoptSummary(adopted({ adopted: 1, alreadyAdopted: 5, refusedFiles: 2, sharedFiles: 1 }))).toBe(
      "Adopted 1 install. 5 installs were already the stone's. 2 files sit outside the places the stone manages, so they stay Fluffy's to remove. 1 file is also another mod's file, so the stone records it without taking ownership. Fluffy's own files were not changed.",
    );
    expect(
      adoptSummary(
        adopted({
          adopted: 2,
          alreadyAdopted: 1,
          missingFiles: 1,
          refusedFiles: 1,
          sharedFiles: 2,
          unmatched: ["Ox Cart Express", "Pawn Banter Quieter"],
        }),
      ),
    ).toBe(
      "Adopted 2 installs. 1 install was already the stone's. 1 file Fluffy listed was already gone. 1 file sits outside the places the stone manages, so it stays Fluffy's to remove. 2 files are also other mods' files, so the stone records them without taking ownership. Adopted under their Fluffy names: Ox Cart Express, Pawn Banter Quieter. Fluffy's own files were not changed.",
    );
    expect(adoptSummary(adopted({}))).toBe("Adopted 0 installs. Fluffy's own files were not changed.");
  });
});

describe("parkedRestoreSummary()", () => {
  const report = { gamePath: SKY, restored: true, swappedOut: null, restartNeeded: false };

  it("should say where the file went, what was swapped out, and when the game must restart", () => {
    expect(parkedRestoreSummary(report)).toBe(`${SKY} is back in your game.`);
    expect(parkedRestoreSummary({ ...report, swappedOut: `backups/41/${SKY}`, restartNeeded: true })).toBe(
      `${SKY} is back in your game. The file that sat there is kept aside in its turn: it is in the PARKED list now. Restart the game to load it.`,
    );
    expect(parkedRestoreSummary({ ...report, restored: false })).toBe(
      `${SKY} could not be put back. It is still kept aside, untouched.`,
    );
  });
});

describe("THE RETURN's rows and heading", () => {
  it("should word a file that is already gone as nothing to remove", () => {
    expect(removalWords(GONE)).toStrictEqual({
      glyph: "∅",
      verb: "already gone from your game: nothing to remove",
      tone: "skip",
    });
    expect(removalWords(DELETE)).toStrictEqual({ glyph: "−", verb: "removed", tone: "gone" });
  });

  it("should count only the rows that move a file as leaving, and give already gone its own clause", () => {
    expect(
      returnHeading(
        removals(
          DELETE,
          { kind: "restoreBackup", owner: "Sacred Arbor Fog Removal" },
          { kind: "handDown", toOptionName: "Brine Water Clarity" },
          GONE,
          { kind: "keepChanged" },
          { kind: "keepVanilla" },
        ),
      ),
    ).toBe("3 files leave your game · 1 already gone · 2 kept");
    expect(returnHeading(removals(DELETE))).toBe("1 file leaves your game");
    expect(returnHeading(removals(GONE, GONE))).toBe("0 files leave your game · 2 already gone");
    expect(returnHeading(removals({ kind: "keepVanilla" }))).toBe("0 files leave your game · 1 kept");
  });

  it("should plan Ermine Cape's return in the detail: one file already gone, one kept, two that leave", async () => {
    await wakeOn();
    const ermine = MOCK_PACKAGES.flatMap((p) => p.options).find((o) => o.name === "Ermine Cape - Variation 2");
    if (!ermine) throw new Error("The mock library has no Ermine Cape - Variation 2");
    void useWorld().open(ermine.id);
    await until(() => hasButton("Uninstall…"), "the installed cape's detail");
    button("Uninstall…").click();
    await until(() => textAt(".plan__title")?.includes("THE RETURN") === true, "THE RETURN");

    expect(textOf(document.querySelector(".plan__title") ?? document.body).trim()).toBe(
      "THE RETURN — 2 files leave your game · 1 already gone · 1 kept",
    );
    const rows = [...document.querySelectorAll(".plan__row")].map((r) => [
      textOf(r.querySelector(".plan__glyph") ?? r).trim(),
      textOf(r.querySelector(".plan__verb") ?? r).trim(),
    ]);
    expect(rows).toStrictEqual([
      ["∅", "already gone from your game: nothing to remove"],
      ["·", "kept: changed since install"],
      ["−", "removed"],
      ["−", "removed"],
    ]);

    button("Uninstall").click();
    await until(() => textAt(".plan__returned .plan__carved") === "RETURNED TO THE RIFT", "the carved return", 4000);
    // THE RETURN mirrors the crossing (Chaos #00129 D4): the plan carves the state
    // and keeps the result's facts; the toast alone says the sentence.
    expect(textAt(".plan__returned .plan__note")?.replace(/\s+/g, " ")).toBe(
      "2 files removed; 1 file left in place (changed since, or the game's own).",
    );
    // the sentence, said once
    expect(text().split("has returned to the Rift.").length - 1).toBe(1);
    expect(
      useToasts().toasts.value.some((t) => t.line.startsWith("Ermine Cape - Variation 2 has returned to the Rift.")),
    ).toBe(true);
  });

  it("should give the same heading on Your World, and toast what the return did", async () => {
    await wakeOn();
    useNavigation().go("world");
    await until(() => capeMinus() !== null, "the cape among the loose and script installs");
    capeMinus()?.click();
    await until(() => document.querySelector(".home__return") !== null, "THE RETURN on Your World");
    expect(textOf(document.querySelector(".home__return .home__h") ?? document.body).trim()).toBe(
      "THE RETURN — 2 files leave your game · 1 already gone · 1 kept",
    );

    button("Uninstall Ermine Cape - Variation 2").click();
    const line =
      "Ermine Cape - Variation 2 has returned to the Rift. 2 files removed; 1 file left in place (changed since, or the game's own).";
    await until(() => useToasts().toasts.value.some((t) => t.line === line), "the returned toast", 4000);
  });

  it("should put the adoption's refused and shared counts on Your World", async () => {
    await wakeOn();
    useNavigation().go("world");
    await until(() => hasButton("Adopt Fluffy's installs"), "the adoption door");
    button("Adopt Fluffy's installs").click();
    await until(() => hasButton("Adopt 1 install"), "the adoption plan");
    button("Adopt 1 install").click();
    await until(() => document.querySelector(".home__adopt .home__line.is-world") !== null, "the adoption done");
    expect(textOf(document.querySelector(".home__adopt .home__line.is-world") ?? document.body).trim()).toBe(
      "Adopted 1 install. 5 installs were already the stone's. 2 files sit outside the places the stone manages, so they stay Fluffy's to remove. 1 file is also another mod's file, so the stone records it without taking ownership. Fluffy's own files were not changed.",
    );
  });
});

describe("the other v0.2 sentences", () => {
  it("should name a cut-short putting-back by its file, not by the engine's intent wording", () => {
    expect(
      recoverySentence({
        optionName: `the file kept aside for ${SKY}`,
        action: "restoreParked",
        filesRolledBack: 1,
      }),
    ).toBe(
      `The last putting-back of ${SKY} was cut short. The stone rolled back 1 file; your game folder is as it was before.`,
    );
  });

  it("should count the unlabeled packages in agreement with the number", () => {
    expect(unlabeledLine(1)).toBe("1 package carries no Nexus labels yet.");
    expect(unlabeledLine(5)).toBe("5 packages carry no Nexus labels yet.");
  });
});
