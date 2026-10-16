// The bench the window specs stand on: every createMockBackend() is its own
// library, so a link, an unlink or an MD5 identify in one spec can never leak
// into the next; and the mock refuses what the real spine cannot do offline
// (an MD5 identify is a Nexus lookup), so an offline spec reads the truth.

import { afterEach, describe, expect, it } from "vitest";
import { MOCK_MD5_MATCH, MOCK_PACKAGES } from "@/services/mock/library";
import { createMockBackend } from "@/services/mock/index";
import { RiftFailure } from "@/services/rift";

afterEach(() => window.history.replaceState(null, "", "/"));

const idOf = (name: string): string => {
  const pkg = MOCK_PACKAGES.find((p) => p.summary.displayName === name);
  if (!pkg) throw new Error(`The mock library has no ${name}`);
  return pkg.summary.id;
};

describe("the mock spine", () => {
  it("should give every backend its own library: a link, an unlink or an identify on one leaks into no other", async () => {
    const brine = idOf(MOCK_MD5_MATCH.displayName);
    const lantern = idOf("Lantern Glow Recolor");
    const first = createMockBackend();
    await first.identifyPackages([brine]);
    await first.unlinkPackage(lantern);
    const firstRows = (await first.getUpdates()).rows;
    expect(firstRows.find((r) => r.packageId === brine)?.link?.modId).toBe(MOCK_MD5_MATCH.modId);
    expect(firstRows.find((r) => r.packageId === lantern)?.link).toBeNull();

    const second = createMockBackend();
    const rows = (await second.getUpdates()).rows;
    expect(rows.find((r) => r.packageId === brine)?.link).toBeNull();
    expect(rows.find((r) => r.packageId === lantern)?.link?.modId).toBe(412);
    expect(MOCK_PACKAGES.find((p) => p.summary.id === brine)?.summary.nexus).toBeNull();
  });

  it("should refuse an MD5 identify while Nexus is out of reach, as the spine's lookup would", async () => {
    window.history.replaceState(null, "", "/?mock=offline");
    const backend = createMockBackend();
    const brine = idOf(MOCK_MD5_MATCH.displayName);
    const refused = await backend.identifyPackages([brine]).then(
      () => null,
      (raw: unknown) => raw,
    );
    expect(refused).toBeInstanceOf(RiftFailure);
    expect((refused as RiftFailure).kind).toBe("nexus");
    expect((await backend.getUpdates()).rows.find((r) => r.packageId === brine)?.link).toBeNull();
  });
});
