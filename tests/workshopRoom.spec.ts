// THE WORKSHOP's room (#00015 §20 under WINDOW-CONTRACT § v0.7): the chip says
// only the stone's three words, a breach holds Summon back, the survey's
// sentence reaches the investor verbatim, the hands say the stone's own
// sentences, the held Play is answered by run and id, the card carries the
// digest it froze, and the Gift and the receipts say what the stone said.
// The terminal is stubbed: xterm is the hollow's alone.

import { defineComponent, h } from "vue";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import * as built from "../uno.config";
import { asRiftFailure, setRiftBackend, type RiftBackend } from "@/services/rift";
import type { LaunchPlanView } from "@/types/rift";
import { resetToasts } from "@/composables/useToasts";
import { setWorkshopBackend } from "@/workshop/backend";
import { createMockWorkshop, type MockWorkshop, type WorkshopScenario } from "@/workshop/mock";
import { resetWorkshop, useWorkshop } from "@/workshop/useWorkshop";
import { FOLDS_KEY, resetBenchFolds } from "@/workshop/benchFolds";
import { MODE_CHOICES, NO_RECEIPT_TITLE, SESSION_LATER, TERMINAL_THEME, swapRefusedLine } from "@/workshop/words";
import WorkshopPage from "@/workshop/WorkshopPage.vue";

/** The hollow without xterm: a fitted 80×24, a focus that does nothing. */
const HollowStub = defineComponent({
  name: "WorkshopHollow",
  setup(_, { expose }) {
    expose({ size: () => ({ cols: 80, rows: 24 }), focus: () => undefined, refit: () => undefined });
    return () => h("div", { class: "hollow-stub" });
  },
});

let mock: MockWorkshop;
let wrapper: VueWrapper | null = null;

/** Let the mock's timers and the store's promises land inside this spec. */
async function settle(): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, 0));
  await flushPromises();
}

async function room(scenario: WorkshopScenario = "contained"): Promise<VueWrapper> {
  mock = createMockWorkshop({ scenario, beat: 0, script: false });
  setWorkshopBackend(mock);
  wrapper = mount(WorkshopPage, { attachTo: document.body, global: { stubs: { WorkshopHollow: HollowStub } } });
  await settle();
  return wrapper;
}

/** A refused RiftButton carries its reason in a hidden span: read the label alone. */
function labelOf(b: ReturnType<VueWrapper["find"]>): string {
  const own = b.find(".rs-btn__label");
  return own.exists() ? own.text() : b.text();
}

function button(w: VueWrapper, label: string) {
  const found = w.findAll("button").find((b) => labelOf(b) === label);
  if (!found) throw new Error(`no button reads "${label}"`);
  return found;
}

async function summoned(w: VueWrapper): Promise<void> {
  await button(w, "Summon").trigger("click");
  await settle();
}

beforeEach(() => {
  localStorage.removeItem(FOLDS_KEY);
  resetBenchFolds();
  resetWorkshop();
  resetToasts();
});
afterEach(async () => {
  wrapper?.unmount();
  wrapper = null;
  await settle();
  setWorkshopBackend(null);
  setRiftBackend(null);
});

describe("the fence's chip", () => {
  it.each([
    ["contained", "CONTAINED"],
    ["unprobed", "UNPROBED"],
    ["breached", "BREACHED"],
  ] as const)("should read %s as the word %s", async (scenario, word) => {
    const w = await room(scenario);
    expect(w.get("[data-chip]").text()).toBe(word);
  });

  it("should open the probe record with its ground, the control first, and the two closing lines", async () => {
    const w = await room();
    await w.get("[aria-controls='ws-fence-record']").trigger("click");
    const record = w.get("#ws-fence-record");
    expect(record.text()).toContain("Ubuntu-24.04 · claude 2.1.312 · 6.6.87.2-microsoft-standard-WSL2 · 4be1c07a");
    expect(record.findAll("li")[0].text()).toContain("control: write in the workshop");
    expect(record.findAll("li")[0].text()).toContain("must land");
    expect(record.text()).toContain("Each step is read outside the fence, never from the pawn's answer.");
    expect(record.text()).toContain("What Tamsin reads, the REFramework log included, leaves this PC for Anthropic.");
  });

  it("should say the stone claims nothing when no probe record matches", async () => {
    const w = await room("unprobed");
    await w.get("[aria-controls='ws-fence-record']").trigger("click");
    expect(w.get("#ws-fence-record").text()).toContain("No probe record matches this host. The stone claims nothing.");
  });
});

describe("a breached fence", () => {
  it("should hold Summon back, summon nothing, and leave the probe as the door", async () => {
    const w = await room("breached");
    expect(w.text()).toContain("THE FENCE DID NOT HOLD");
    const summon = button(w, "Summon");
    expect(summon.attributes("aria-disabled")).toBe("true");
    await summon.trigger("click");
    await settle();
    expect(mock.controls.calls.some((c) => c.cmd === "summon_pawn")).toBe(false);
    expect(button(w, "Run the probe").exists()).toBe(true);
  });
});

describe("no fence to build", () => {
  it("should carve NO PAWN CAN CROSS and say the survey's sentence verbatim", async () => {
    const w = await room("nofence");
    await summoned(w);
    expect(w.text()).toContain("NO PAWN CAN CROSS");
    expect(w.text()).toContain("socat is not installed in Ubuntu-24.04. Install it with: sudo apt install socat");
    expect(button(w, "Look again").exists()).toBe(true);
  });
});

describe("the pawn's hands", () => {
  it("should write a refused call as ember, with the stone's sentence verbatim", async () => {
    const w = await room();
    await summoned(w);
    const sentence = "That install would write over a game file, natives\\x.pak. The stone never writes over vanilla.";
    const hand = mock.controls.emitHand({
      tool: "install",
      outcome: "refused",
      summary: sentence,
      installId: null,
      refusal: sentence,
    });
    await settle();
    const row = w.get(`[data-hand="${hand.seq}"]`);
    expect(row.classes()).toContain("is-ember");
    expect(row.find(".hand__line").text()).toBe("refused: install");
    expect(row.find(".hand__second").text()).toBe(sentence);
  });

  it("should answer the held request Not now with the run and the request it showed", async () => {
    const w = await room();
    await summoned(w);
    const id = mock.controls.hold();
    await settle();
    expect(w.text()).toContain("Tamsin asks to start the game.");
    expect(button(w, "Play").exists()).toBe(true);
    await button(w, "Not now").trigger("click");
    await settle();
    const answered = mock.controls.calls.find((c) => c.cmd === "answer_pawn_play");
    expect(answered?.args).toStrictEqual(["run-mock-1", id, false]);
    expect(w.text()).toContain("Not now: Tamsin was told.");
  });

  it("should keep the decision apart from the game watcher's own state after Play", async () => {
    setRiftBackend({
      planLaunch: () => Promise.resolve({ launch: "steam", warnings: [], refused: null } as unknown as LaunchPlanView),
    } as unknown as RiftBackend);
    const w = await room();
    await summoned(w);
    mock.controls.hold();
    await settle();
    await button(w, "Play").trigger("click");
    await settle();
    expect(w.get(".hand__decision").text()).toBe("The stone started the launch.");
    expect(w.get(".hand__watcher").text()).toBe("The stone has not seen the game start yet.");
  });

  it("should say so when the decision did not reach Tamsin's ledger", async () => {
    const w = await room("unpersisted");
    await summoned(w);
    mock.controls.hold();
    await settle();
    await button(w, "Not now").trigger("click");
    await settle();
    expect(w.get(".hand__unpersisted").text()).toBe("Tamsin's ledger could not be written, so she may not hear of it.");
  });
});

describe("bringing a build through", () => {
  it("should carve the digest frozen at opening, and turn to THE BYTES CHANGED when the file was rebuilt", async () => {
    const w = await room();
    const frozen = useWorkshop().view.value?.artifacts[0].sha256 ?? "";
    expect(frozen).toHaveLength(64);
    await button(w, "Add to library…").trigger("click");
    await settle();
    const tablet = w.get("[data-digest]");
    expect(tablet.findAll("span").map((s) => s.text())).toHaveLength(16);
    expect(
      tablet
        .findAll("span")
        .map((s) => s.text())
        .join(""),
    ).toBe(frozen);
    mock.controls.rebuild("Stride-Tuner-0.2.0.zip");
    await settle();
    expect(useWorkshop().view.value?.artifacts[0].sha256).not.toBe(frozen);
    expect(
      w
        .get("[data-digest]")
        .findAll("span")
        .map((s) => s.text())
        .join(""),
    ).toBe(frozen);
    await button(w, "Add to library ▶").trigger("click");
    await settle();
    expect(w.text()).toContain("THE BYTES CHANGED");
    expect(w.text()).toContain("Nothing was added.");
    expect(button(w, "Review it again").exists()).toBe(true);
    const pressed = mock.controls.calls.find((c) => c.cmd === "bring_through");
    expect(pressed?.args).toStrictEqual(["Stride-Tuner-0.2.0.zip", frozen]);
  });

  it("should list a build with no receipt, faint, with no button", async () => {
    const w = await room();
    const row = w.get('[data-artifact="Stride-Tuner-0.1.0.zip"]');
    expect(row.text()).toContain("no receipt");
    expect(row.attributes("title")).toBe(NO_RECEIPT_TITLE);
    expect(row.findAll("button").filter((b) => b.text() === "Add to library…")).toHaveLength(0);
  });
});

describe("after the pawn has gone", () => {
  it("should offer Put back with the stone's own sentence", async () => {
    const w = await room("departed");
    expect(w.text()).toContain("THE PAWN HAS GONE BACK");
    expect(w.get(".putback__sentence").text()).toBe(
      "Removes the 2 installs Tamsin made in this summoning. She also uninstalled 1 of yours. It is listed below.",
    );
    expect(w.get(".putback__review").text()).toContain("yours");
    await button(w, "Put them back").trigger("click");
    await settle();
    const put = mock.controls.calls.find((c) => c.cmd === "put_back");
    expect(put?.args).toStrictEqual(["run-mock-0", ["opt-lantern-glow", "opt-stride-020"]]);
  });
});

describe("the terminal's colours", () => {
  it("should map the ANSI colours onto the stone's tokens, with the selection's ink forced to bone", () => {
    const rs = built.riftColors.rs;
    expect(TERMINAL_THEME.background).toBe(rs.void);
    expect(TERMINAL_THEME.foreground).toBe(rs.text);
    expect(TERMINAL_THEME.brightBlack).toBe(rs.faint);
    expect([TERMINAL_THEME.red, TERMINAL_THEME.brightRed]).toStrictEqual([rs.ember, rs.ember]);
    expect([TERMINAL_THEME.green, TERMINAL_THEME.brightGreen]).toStrictEqual([rs.verdant, rs.verdant]);
    expect([TERMINAL_THEME.yellow, TERMINAL_THEME.brightYellow]).toStrictEqual([rs.warn, rs.warn]);
    for (const c of [TERMINAL_THEME.blue, TERMINAL_THEME.cyan, TERMINAL_THEME.brightBlue, TERMINAL_THEME.brightCyan]) {
      expect(c).toBe(rs.rift);
    }
    expect([TERMINAL_THEME.magenta, TERMINAL_THEME.brightMagenta]).toStrictEqual([rs.world, rs.world]);
    expect(TERMINAL_THEME.black).toBe(rs.well);
    expect(TERMINAL_THEME.cursor).toBe(rs.rift);
    expect(TERMINAL_THEME.selectionBackground).toBe(rs.riftdeep);
    expect(TERMINAL_THEME.selectionForeground).toBe(rs.text);
  });
});

describe("the fence's record", () => {
  it("should show each row's verdict, a row that could not show its refusal apart from a breach, and the control first", async () => {
    const w = await room("inconclusive");
    expect(w.find(".ws-chip").text()).toContain("UNPROBED");
    await w.find("[aria-controls='ws-fence-record']").trigger("click");
    await settle();
    const rows = w.findAll(".fence__step");
    expect(rows).toHaveLength(15);
    const control = rows[0];
    const three = rows[3];
    const last = rows[14];
    expect(control.text()).toContain("control: write in the workshop");
    expect(control.text()).toContain("✓");
    expect(three.text()).toContain("list Steam's userdata");
    expect(three.find(".fence__mark").text()).toBe("?");
    expect(three.find(".fence__mark").attributes("aria-label")).toBe("could not show");
    expect(three.classes()).toContain("is-open");
    expect(three.classes()).not.toContain("is-broken");
    expect(last.text()).toContain("broker: a bad name, 64 KiB + 1");
    expect(w.text()).toContain("the last probe could not show step 3");
  });
});

const group = (w: VueWrapper, name: string) => w.find(`[role=radiogroup][aria-labelledby="ws-${name}"]`);
const chosen = (w: VueWrapper, name: string) => group(w, name).find('[aria-checked="true"]').text();
const says = (w: VueWrapper, name: string) => w.find(`#ws-${name}-says`).text();

describe("THE BENCH's SUMMONED WITH (0.7.2)", () => {
  it("should carry mode, effort and model, and no voice dial", async () => {
    const w = await room();
    expect(w.find("#ws-session").text()).toBe("SUMMONED WITH");
    expect(w.text()).not.toContain("VOICE");
    expect(w.text()).not.toContain("Sparing");
    expect(chosen(w, "mode")).toBe("Ask");
    expect(chosen(w, "effort")).toBe("default");
    expect(chosen(w, "model")).toBe("Default");
    expect(says(w, "mode")).toBe(
      "She asks before each file edit. Most shell commands run unasked, inside her sandbox.",
    );
  });

  it("should say what the chosen setting does, and send it to the stone", async () => {
    const w = await room();
    await group(w, "mode")
      .findAll("button")
      .find((b) => b.text() === "Edits")
      ?.trigger("click");
    await settle();
    expect(chosen(w, "mode")).toBe("Edits");
    expect(says(w, "mode")).toBe(
      "Her file edits in the workshop go through unasked, like most sandboxed shell commands.",
    );
    await group(w, "effort")
      .findAll("button")
      .find((b) => b.text() === "xhigh")
      ?.trigger("click");
    await settle();
    const sent = mock.controls.calls.filter((c) => c.cmd === "set_pawn_session").map((c) => c.args[0]);
    expect(sent).toStrictEqual([
      { mode: "acceptEdits", effort: "default", model: "default" },
      { mode: "acceptEdits", effort: "xhigh", model: "default" },
    ]);
  });

  it("should rove with the arrows and wrap at the ends", async () => {
    const w = await room();
    const ask = group(w, "mode").find('[aria-checked="true"]');
    await ask.trigger("keydown", { key: "ArrowLeft" });
    await settle();
    expect(chosen(w, "mode")).toBe(MODE_CHOICES.at(-1)?.label);
    expect(document.activeElement).toBe(group(w, "mode").findAll("button").at(-1)?.element);
  });

  it("should say a change waits for the next summon while she is here", async () => {
    const w = await room();
    expect(w.text()).not.toContain(SESSION_LATER);
    await summoned(w);
    expect(w.text()).toContain(SESSION_LATER);
    expect(group(w, "model").attributes("title")).toBe(SESSION_LATER);
  });
});

const toggle = (w: VueWrapper, section: string) => w.get(`[aria-controls="ws-${section}-body"]`);
const body = (w: VueWrapper, section: string) => w.get(`#ws-${section}-body`);
const isShown = (w: VueWrapper, section: string) => (body(w, section).element as HTMLElement).style.display !== "none";

describe("THE BENCH folds (0.7.4)", () => {
  it("should open every section until the investor folds one", async () => {
    const w = await room();
    for (const section of ["session", "inputs", "built", "hands"]) {
      expect(toggle(w, section).attributes("aria-expanded")).toBe("true");
      expect(isShown(w, section)).toBe(true);
    }
  });

  it("should fold SUMMONED WITH to a line that still says how she is summoned", async () => {
    const w = await room();
    await toggle(w, "session").trigger("click");
    expect(toggle(w, "session").attributes("aria-expanded")).toBe("false");
    expect(isShown(w, "session")).toBe(false);
    expect(toggle(w, "session").find(".bench__summary").text()).toBe("Ask · default · Default");
    expect(isShown(w, "built")).toBe(true);
    await toggle(w, "session").trigger("click");
    expect(isShown(w, "session")).toBe(true);
    expect(toggle(w, "session").find(".bench__summary").exists()).toBe(false);
  });

  it("should remember the folds the investor made in the next window", async () => {
    const first = await room();
    await toggle(first, "session").trigger("click");
    await toggle(first, "inputs").trigger("click");
    first.unmount();
    wrapper = null;
    expect(JSON.parse(localStorage.getItem(FOLDS_KEY) ?? "{}")).toStrictEqual({ session: true, inputs: true });

    resetBenchFolds();
    resetWorkshop();
    const w = await room();
    expect(isShown(w, "session")).toBe(false);
    expect(isShown(w, "inputs")).toBe(false);
    expect(toggle(w, "inputs").find(".bench__summary").text()).toBe("none");
    expect(isShown(w, "built")).toBe(true);
  });

  it("should keep THE PAWN'S HANDS open while Tamsin waits on a press, and say why", async () => {
    const w = await room();
    await summoned(w);
    await toggle(w, "hands").trigger("click");
    expect(isShown(w, "hands")).toBe(false);
    mock.controls.hold();
    await settle();
    expect(isShown(w, "hands")).toBe(true);
    expect(w.text()).toContain("Tamsin asks to start the game.");
    expect(toggle(w, "hands").attributes("aria-disabled")).toBe("true");
    expect(toggle(w, "hands").attributes("title")).toBe("Open while Tamsin waits for your answer.");
    // A press while held changes nothing, and the fold returns once she is answered.
    await toggle(w, "hands").trigger("click");
    await button(w, "Not now").trigger("click");
    await settle();
    expect(isShown(w, "hands")).toBe(false);
  });

  it("should keep BUILT open while a build's refusal shows, until it is dismissed (the Heretic's BF1)", async () => {
    const w = await room();
    const sentence = "The build changed before its plan could be opened.";
    setWorkshopBackend({ ...mock, planBringThrough: () => Promise.reject(new Error(sentence)) });
    await button(w, "Add to library…").trigger("click");
    await settle();
    expect(w.get(".built__refused").text()).toBe(sentence);
    await toggle(w, "built").trigger("click");
    expect(isShown(w, "built")).toBe(true);
    expect(toggle(w, "built").attributes("aria-disabled")).toBe("true");
    expect(toggle(w, "built").attributes("title")).toBe("Open while a build's card or the stone's refusal is showing.");
    await button(w, "Dismiss").trigger("click");
    expect(w.find(".built__refused").exists()).toBe(false);
    await toggle(w, "built").trigger("click");
    expect(isShown(w, "built")).toBe(false);
  });

  it("should reopen BUILT for a refusal that lands after it was folded (the Heretic's BF1)", async () => {
    const w = await room();
    const sentence = "The build was removed while its plan was being read.";
    let refuse: ((reason: Error) => void) | null = null;
    setWorkshopBackend({
      ...mock,
      planBringThrough: () =>
        new Promise((_resolve, reject: (reason: Error) => void) => {
          refuse = reject;
        }),
    });
    await button(w, "Add to library…").trigger("click");
    await settle();
    await toggle(w, "built").trigger("click");
    expect(isShown(w, "built")).toBe(false);
    (refuse as unknown as (reason: Error) => void)(new Error(sentence));
    await settle();
    expect(isShown(w, "built")).toBe(true);
    expect(w.get(".built__refused").text()).toBe(sentence);
  });

  it("should keep BUILT open while a build's card is open", async () => {
    const w = await room();
    await button(w, "Add to library…").trigger("click");
    await settle();
    expect(w.find(".bring").exists()).toBe(true);
    await toggle(w, "built").trigger("click");
    expect(isShown(w, "built")).toBe(true);
    await button(w, "Cancel").trigger("click");
    await toggle(w, "built").trigger("click");
    expect(isShown(w, "built")).toBe(false);
  });

  it("should ignore storage it cannot read", async () => {
    localStorage.setItem(FOLDS_KEY, "not json");
    resetBenchFolds();
    const w = await room();
    expect(isShown(w, "session")).toBe(true);
  });
});

describe("Update in its place (0.8, #00015 §20.14)", () => {
  const BUILD = "Stride-Tuner-0.2.0.zip";
  const row = (w: VueWrapper) => w.get(`[data-artifact="${BUILD}"]`);

  it("should name the earlier build on the row and offer Update first", async () => {
    const w = await room("update");
    expect(row(w).find(".built__replaces").text()).toBe("replaces Stride-Tuner-0.1.0.zip");
    expect(button(w, "Update…").classes()).toContain("rs-btn--primary");
    expect(button(w, "Add to library…").classes()).toContain("rs-btn--secondary");
  });

  it("should offer no Update for a build with no earlier build", async () => {
    const w = await room();
    expect(w.find(".built__replaces").exists()).toBe(false);
    expect(w.findAll("button").some((b) => labelOf(b) === "Update…")).toBe(false);
  });

  it("should show the swap and the put-away, and press with the digest and the plan it showed", async () => {
    const w = await room("update");
    await button(w, "Update…").trigger("click");
    await settle();
    const card = w.get(".update");
    expect(card.text()).toContain("IN ITS PLACE");
    expect(card.get("[data-swap]").text()).toBe("↻Stride Tuner  0.1.0 → 0.2.0");
    expect(card.get("[data-put-away]").text()).toBe("↧Stride-Tuner-0.1.0.zip to mods (older versions)");
    expect(button(w, "Update ▶").classes()).toContain("rs-btn--world");
    const sha = mock.controls.view().artifacts.find((a) => a.name === BUILD)?.sha256;
    await button(w, "Update ▶").trigger("click");
    await settle();
    const pressed = mock.controls.calls.find((c) => c.cmd === "update_build");
    expect(pressed?.args).toStrictEqual([BUILD, sha, "mock-token-stride-010-to-020"]);
    expect(w.get(".update__state").text()).toBe("IN YOUR WORLD");
    expect(w.text()).toContain("Stride-Tuner-0.1.0.zip is put away in mods (older versions)");
  });

  it("should say the swap did not happen and the game folder is as it was", async () => {
    const w = await room("updaterefused");
    await button(w, "Update…").trigger("click");
    await settle();
    await button(w, "Update ▶").trigger("click");
    await settle();
    expect(w.get(".update__state").text()).toBe("IN THE RIFT");
    expect(w.text()).toContain("but the swap didn't happen: That install would write over a game file");
    expect(w.text()).toContain("Stride-Tuner-0.1.0.zip is still installed");
  });

  it("should refuse bytes that changed after the card opened, and review them again", async () => {
    const w = await room("update");
    await button(w, "Update…").trigger("click");
    await settle();
    mock.controls.rebuild(BUILD);
    await settle();
    await button(w, "Update ▶").trigger("click");
    await settle();
    expect(w.get(".update__state").text()).toBe("THE BYTES CHANGED");
    expect(w.text()).toContain("Nothing was added or swapped.");
    await button(w, "Review it again").trigger("click");
    await settle();
    expect(w.get(".update").text()).toContain("IN ITS PLACE");
    expect(w.find(".update__state").exists()).toBe(false);
  });
});

const asks = (w: VueWrapper) => w.findAll("[data-ask]").map((a) => a.attributes("data-ask"));

describe("SHE ASKS (0.8, R20.36, Q20.33 ruled A)", () => {
  const BUILD = "Stride-Tuner-0.2.0.zip";

  it("should pin a lone held Play under SHE ASKS", async () => {
    const w = await room();
    await summoned(w);
    mock.controls.hold();
    await settle();
    expect(w.get("#ws-she-asks").text()).toBe("SHE ASKS");
    expect(asks(w)).toStrictEqual(["play"]);
    expect(w.get('[data-ask="play"]').text()).toContain("Tamsin asks to start the game.");
  });

  it("should pin both asks oldest first, and answer the update by its run, id and the plan it showed", async () => {
    const w = await room("update");
    await summoned(w);
    const id = mock.controls.holdUpdate(BUILD);
    await new Promise((resolve) => setTimeout(resolve, 5));
    mock.controls.hold();
    await settle();
    expect(asks(w)).toStrictEqual(["update", "play"]);
    const update = w.get('[data-ask="update"]');
    expect(update.text()).toContain(`Tamsin asks to update to ${BUILD}.`);
    expect(update.text()).toContain("↻ Stride Tuner  0.1.0 → 0.2.0");
    expect(update.text()).toContain("frozen at her ask");
    expect(w.get('[data-ask="play"]').text()).toContain("Her update is still waiting above.");
    const updateButton = update.findAll("button").find((b) => labelOf(b) === "Update");
    await updateButton?.trigger("click");
    await settle();
    const answered = mock.controls.calls.find((c) => c.cmd === "answer_pawn_update");
    expect(answered?.args).toStrictEqual(["run-mock-1", id, true, "mock-token-stride-010-to-020"]);
    expect(asks(w)).toStrictEqual(["play"]);
  });

  it("should not update bytes that changed after she asked", async () => {
    const w = await room("update");
    await summoned(w);
    mock.controls.holdUpdate(BUILD);
    await settle();
    mock.controls.rebuild(BUILD);
    await settle();
    // her ask's card is read again when the ask appears; a new look reads the new bytes
    resetWorkshop();
    wrapper?.unmount();
    wrapper = mount(WorkshopPage, { attachTo: document.body, global: { stubs: { WorkshopHollow: HollowStub } } });
    await settle();
    const update = wrapper.get('[data-ask="update"]');
    expect(update.text()).toContain("changed after Tamsin asked");
    const updateButton = update.findAll("button").find((b) => labelOf(b) === "Update");
    expect(updateButton?.attributes("aria-disabled")).toBe("true");
    await updateButton?.trigger("click");
    await settle();
    expect(mock.controls.calls.some((c) => c.cmd === "answer_pawn_update")).toBe(false);
  });

  it("should keep the order she asked in when she asks again (the Heretic's U5)", async () => {
    const w = await room("update");
    await summoned(w);
    mock.controls.holdUpdate(BUILD);
    mock.controls.hold();
    await settle();
    // She asks for the update again: the stone answers held, naming the one waiting.
    mock.controls.emitHand({
      tool: "ask_update",
      outcome: "held",
      summary: `asks to update to ${BUILD}`,
      installId: null,
      refusal: null,
    });
    await settle();
    expect(asks(w)).toStrictEqual(["update", "play"]);
  });

  it("should never show an earlier ask's refusal under her next one (the Heretic's U6)", async () => {
    const w = await room("update");
    await summoned(w);
    mock.controls.holdUpdate(BUILD);
    await settle();
    let refuse: ((reason: unknown) => void) | null = null;
    mock.answerPawnUpdate = () =>
      new Promise((_resolve, reject: (reason: unknown) => void) => {
        refuse = reject;
      });
    const updateButton = w
      .get('[data-ask="update"]')
      .findAll("button")
      .find((b) => labelOf(b) === "Update");
    await updateButton?.trigger("click");
    await settle();
    // Her first ask goes; she asks again before the stone's refusal of the first arrives.
    mock.controls.emitRawState({ ...mock.controls.view().pawn, heldUpdate: null });
    mock.controls.holdUpdate(BUILD);
    await settle();
    (refuse as unknown as (reason: unknown) => void)(
      asRiftFailure({ kind: "refused", detail: { reason: "The old ask's own refusal." } }),
    );
    await settle();
    expect(w.get('[data-ask="update"]').text()).not.toContain("The old ask's own refusal.");
  });

  it("should answer Not now with no earlier build", async () => {
    const w = await room("update");
    await summoned(w);
    const id = mock.controls.holdUpdate(BUILD);
    await settle();
    const notNow = w
      .get('[data-ask="update"]')
      .findAll("button")
      .find((b) => labelOf(b) === "Not now");
    await notNow?.trigger("click");
    await settle();
    const answered = mock.controls.calls.find((c) => c.cmd === "answer_pawn_update");
    expect(answered?.args).toStrictEqual(["run-mock-1", id, false, null]);
    expect(w.find("#ws-she-asks").exists()).toBe(false);
  });
});

describe("a refused swap says what was swapped (the Heretic's U3)", () => {
  it("should say the game is as it was only when nothing was swapped", () => {
    const none = swapRefusedLine("Stride-Tuner-0.2.0.zip", {
      earlier: "Stride-Tuner-0.1.0.zip",
      swapped: [],
      failed: "Refused.",
    });
    expect(none).toContain("Your game folder is exactly as it was");
    const one = swapRefusedLine("Stride-Tuner-0.2.0.zip", {
      earlier: "Stride-Tuner-0.1.0.zip",
      swapped: [{}],
      failed: "Refused.",
    });
    expect(one).not.toContain("exactly as it was");
    expect(one).toContain(
      "1 option was swapped in place of Stride-Tuner-0.1.0.zip's; then a swap was refused: Refused.",
    );
    expect(one).toContain("What was not swapped is as it was, and Stride-Tuner-0.1.0.zip stays in your mods folder.");
  });
});
