// §9 — the voiced states, each from its own ?mock= scenario, with its exact
// copy and its recovery present (the Error Manifesto: voice, empathy, recovery).

import { afterEach, describe, expect, it } from "vitest";
import { useNavigation } from "@/composables/useNavigation";
import { button, hasButton, sleep, text, textOf, until, wakeOn, idle } from "./harness";

afterEach(() => sleep());

describe("the voiced states (§9)", () => {
  it("should speak THE RIFT IS QUIET over an empty library, with both ways out", async () => {
    await wakeOn("empty");
    await until(() => text().includes("THE RIFT IS QUIET"), "the quiet Rift");
    expect(text()).toContain(
      "No mods in your library roots yet. Add the folder where your downloads land, or adopt what Fluffy already has.",
    );
    expect(hasButton("Add library folder…")).toBe(true);
    expect(hasButton("Adopt Fluffy's library")).toBe(true);
  });

  it("should say NO WORLD ANSWERS when no game is found, and offer the folder and a retry", async () => {
    await wakeOn("nogame");
    await until(() => text().includes("NO WORLD ANSWERS"), "the no-game frame");
    expect(text()).toContain(
      "The stone searched every Steam library on this PC and found no Dragon's Dogma 2 install. If it lives somewhere unusual, point the stone at it.",
    );
    expect(hasButton("Choose game folder…")).toBe(true);
    expect(hasButton("Retry search")).toBe(true);
  });

  it("should read the Rift aloud while scanning, with the progress carrying the information", async () => {
    await wakeOn("scanning");
    await until(() => text().includes("READING THE RIFT"), "the scanning frame");
    expect(text()).toMatch(/Opened [\d,]+ of [\d,]+ packages/);
    // §9's Pause waits on a spine command: no dead button is drawn meanwhile.
    expect(hasButton("Pause")).toBe(false);
  });

  it("should warn that PAK mods won't load when the Alternate PAK Folder is off, and offer to turn it on", async () => {
    await wakeOn("refoff");
    useNavigation().go("preflight");
    await until(() => text().includes("Alternate PAK Folder is off"), "the switch-off row");
    expect(text()).toContain("Your PAK mods won't load: REFramework's Alternate PAK Folder is off.");
    expect(hasButton("Turn it on")).toBe(true);
  });

  it("should announce YOUR WORLD HAS CHANGED after a game patch, naming both builds", async () => {
    await wakeOn("patched");
    useNavigation().go("world");
    await until(() => text().includes("YOUR WORLD HAS CHANGED"), "the patched banner");
    expect(text()).toMatch(/updated from build 25290080 to 25391144/);
    expect(hasButton("I've tested it, dismiss")).toBe(true);
  });

  it("should say the far world is out of reach when Nexus is offline, and offer to try again", async () => {
    await wakeOn("offline");
    useNavigation().go("nexus");
    await until(() => hasButton("Check now"), "the Nexus room");
    button("Check now").click();
    await until(() => text().includes("THE FAR WORLD IS OUT OF REACH"), "the offline notice");
    expect(text()).toContain("The stone can't reach Nexus right now.");
    await until(() => idle(button("Try again")), "the retry to settle");
  });

  it("should say Never checked before the first Nexus check, and offer the check itself", async () => {
    await wakeOn("fresh");
    useNavigation().go("nexus");
    await until(() => hasButton("Check now"), "the Nexus room");
    expect(text()).toContain("Never checked");
    expect(text()).not.toMatch(/Last checked/);
    expect(button("Check now").disabled).toBe(false);
  });

  it("should keep the last good verdict in every Latest cell when a check fails, as its notice promises", async () => {
    await wakeOn("offline");
    useNavigation().go("nexus");
    await until(() => hasButton("Check now"), "the Nexus room");
    button("Check now").click();
    await until(() => text().includes("from the last successful check"), "the dated offline notice");
    await until(() => idle(button("Try again")), "the retry to settle");
    const latest = [...document.querySelectorAll(".far__table tbody tr td:nth-child(3)")].map((td) =>
      textOf(td).trim(),
    );
    // v0.2's mock links four packages (the wardrobe and the chatter filter
    // joined, so the NEXUS facets have labels to carve), in library order.
    expect(latest).toStrictEqual(["current", "v2.2", "current", "current"]);
    expect(text()).not.toContain("last check failed");
  });

  it("should draw no quota without a key, because anonymous checks carry none (WINDOW-CONTRACT get_nexus_quota)", async () => {
    await wakeOn("quotalow");
    useNavigation().go("nexus");
    await until(() => text().includes("12 requests left this hour"), "the keyed quota warning (control)");
    expect(text()).toContain("CONNECTED");
    sleep();
    await wakeOn();
    useNavigation().go("nexus");
    await until(() => text().includes("NO KEY") && hasButton("Check now"), "the keyless Key section");
    await new Promise((resolve) => setTimeout(resolve, 60)); // the quota probe's own 20 ms
    expect(text()).not.toMatch(/hourly \d+\/\d+/);
    expect(button("Check now").disabled).toBe(false);
  });

  it("should show none of those states on a healthy library (the control: each state needs its scenario)", async () => {
    await wakeOn();
    await until(() => document.querySelector("#rift-answers .card") !== null, "the library");
    for (const line of ["THE RIFT IS QUIET", "NO WORLD ANSWERS", "READING THE RIFT", "YOUR WORLD HAS CHANGED"]) {
      expect(text()).not.toContain(line);
    }
  });
});
