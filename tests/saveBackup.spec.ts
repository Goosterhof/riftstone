import { afterEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { RiftFailure } from "@/services/rift";
import { button, hasButton, press, sleep, spine, text, until, wakeOn } from "./harness";

afterEach(() => {
  vi.restoreAllMocks();
  sleep();
});
async function open(): Promise<void> {
  await wakeOn("muster");
  press("5", {}, window);
  await until(() => hasButton("Back up all saves…") && !button("Back up all saves…").disabled, "save folders found");
}

describe("manual save backup", () => {
  it("chooses a destination and reveals only a verified backup", async () => {
    await open();
    const backend = spine();
    const pick = vi.spyOn(backend, "pickFolder").mockResolvedValue("D:/Personal Backups");
    const make = vi.spyOn(backend, "backupSaves");
    const reveal = vi.spyOn(backend, "revealPath");
    button("Back up all saves…").click();
    await until(() => text().includes("Verified backup:"), "verified receipt");
    expect(pick).toHaveBeenCalledOnce();
    expect(make).toHaveBeenCalledWith("D:/Personal Backups");
    expect(text()).toContain("including system data");
    expect(text()).toContain("8 files");
    button("Show backup").click();
    await flushPromises();
    expect(reveal).toHaveBeenCalledWith(expect.stringContaining("D:/Personal Backups/DD2 saves"));
  });
  it("cancelling the folder picker makes no backup", async () => {
    await open();
    const backend = spine();
    vi.spyOn(backend, "pickFolder").mockResolvedValue(null);
    const make = vi.spyOn(backend, "backupSaves");
    button("Back up all saves…").click();
    await flushPromises();
    expect(make).not.toHaveBeenCalled();
    expect(text()).not.toContain("Verified backup:");
  });
  it("can request a backup even when no playable slot is readable", async () => {
    await wakeOn("musternosaves");
    press("5", {}, window);
    await until(() => hasButton("Back up all saves…"), "backup door with no playable slots");
    vi.spyOn(spine(), "pickFolder").mockResolvedValue("D:/Backups");
    const make = vi.spyOn(spine(), "backupSaves").mockResolvedValue({
      path: "D:/Backups/DD2 saves system-only",
      folderName: "DD2 saves system-only",
      createdAt: "2026-10-27T20:00:00.000Z",
      accounts: 1,
      files: 1,
      bytes: 20,
    });
    expect(button("Back up all saves…").disabled).toBe(false);
    button("Back up all saves…").click();
    await until(() => text().includes("Verified backup:"), "system-only snapshot receipt");
    expect(make).toHaveBeenCalledOnce();
    expect(text()).toContain("1 file across 1 account");
  });
  it("holds repeated presses and explains a failed attempt without claiming success", async () => {
    await open();
    const backend = spine();
    vi.spyOn(backend, "pickFolder").mockResolvedValue("D:/Backups");
    let refuse!: (reason: unknown) => void;
    const make = vi.spyOn(backend, "backupSaves").mockImplementation(
      () =>
        new Promise((_resolve, reject) => {
          refuse = reject;
        }),
    );
    const trigger = button("Back up all saves…");
    trigger.click();
    await flushPromises();
    trigger.click();
    await flushPromises();
    expect(make).toHaveBeenCalledOnce();
    expect(trigger.getAttribute("aria-busy")).toBe("true");
    refuse(
      new RiftFailure({
        kind: "refused",
        detail: { reason: "Close Dragon's Dogma 2 before backing up its saves, then try again." },
      }),
    );
    await flushPromises();
    expect(text()).toContain("Close Dragon's Dogma 2");
    expect(text()).not.toContain("Verified backup:");
    expect(trigger.getAttribute("aria-busy")).not.toBe("true");
  });
});
