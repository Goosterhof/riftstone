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
import { setRiftBackend, type RiftBackend } from "@/services/rift";
import type { LaunchPlanView } from "@/types/rift";
import { resetToasts } from "@/composables/useToasts";
import { setWorkshopBackend } from "@/workshop/backend";
import { createMockWorkshop, type MockWorkshop, type WorkshopScenario } from "@/workshop/mock";
import { resetWorkshop, useWorkshop } from "@/workshop/useWorkshop";
import { NO_RECEIPT_TITLE, TERMINAL_THEME } from "@/workshop/words";
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
    expect(w.get(".hand__decision").text()).toBe("The stone started the game.");
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
