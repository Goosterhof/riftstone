// The setup's network guard (tests/setup.ts): no spec reaches the network.
// The hosts here are `.invalid`, which never resolve, so even with the guard
// removed nothing could leave the machine — the assertions would just fail.

describe("the specs' network guard", () => {
  it("should refuse a fetch to any host but this machine", async () => {
    await expect(fetch("https://riftstone-guard.invalid/mods/2207")).rejects.toThrow(
      "A spec tried to reach the network: https://riftstone-guard.invalid/mods/2207",
    );
  });

  it("should let window.open record a page without loading it", () => {
    const opened = window.open("https://riftstone-guard.invalid/mods/2207", "_blank", "noopener");
    expect(opened).toBeNull();
    expect(window.open).toHaveBeenCalledWith("https://riftstone-guard.invalid/mods/2207", "_blank", "noopener");
  });
});
