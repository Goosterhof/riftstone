// Shapes that began as the window's contract requests and have since
// graduated into the contract (src/types/rift.ts, RiftBackend): the troubled
// packages behind the §9 "can't be opened" cards, and the Nexus v1 quota.
// The probes stay as the window's one seam for them, so a backend that cannot
// answer (an old spine, a narrowed test double) degrades to "surface hidden".

import type { NexusQuota, NexusSummary, PackageSummary } from "@/types/rift";
import type { RiftBackend } from "./rift";

export type { NexusQuota } from "@/types/rift";

/** The key's last known standing. "none" is not an error: checks run without one. */
export type NexusKeyState = "connected" | "none" | "rejected" | "offline";

export interface PendingBackend {
  /** Packages with problems, including those that yielded zero options. */
  troubledPackages?: () => Promise<PackageSummary[]>;
  nexusQuota?: () => Promise<NexusQuota | null>;
}

export function troubledProbe(backend: RiftBackend | PendingBackend): (() => Promise<PackageSummary[]>) | null {
  const candidate: PendingBackend = backend;
  return typeof candidate.troubledPackages === "function" ? candidate.troubledPackages : null;
}

export function quotaProbe(backend: RiftBackend | PendingBackend): (() => Promise<NexusQuota | null>) | null {
  const candidate: PendingBackend = backend;
  return typeof candidate.nexusQuota === "function" ? candidate.nexusQuota : null;
}

export function keyStateOf(summary: NexusSummary, rejected: boolean, offline: boolean): NexusKeyState {
  if (rejected) return "rejected";
  if (offline) return "offline";
  return summary.hasApiKey ? "connected" : "none";
}
