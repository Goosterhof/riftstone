// Small, pure formatting helpers the window shares. No Vue, no backend.

import type { DeployKind, PayloadKinds } from "@/types/rift";

const UNITS = ["B", "KB", "MB", "GB", "TB"] as const;

/** 4_404_019 → "4.2 MB". One decimal below 10, none above. */
export function formatBytes(bytes: number): string {
  let value = Math.max(0, bytes);
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = value < 10 && unit > 0 ? 1 : 0;
  return `${value.toFixed(digits)} ${UNITS[unit] ?? "B"}`;
}

export function plural(count: number, one: string, many = `${one}s`): string {
  return `${count.toLocaleString("en-GB")} ${count === 1 ? one : many}`;
}

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** "just now", "12 minutes ago", "2 hours ago", "3 days ago". */
export function formatAgo(thenMs: number, nowMs: number = Date.now()): string {
  const delta = Math.max(0, nowMs - thenMs);
  if (delta < MINUTE) return "just now";
  if (delta < HOUR) return `${plural(Math.floor(delta / MINUTE), "minute")} ago`;
  if (delta < DAY) return `${plural(Math.floor(delta / HOUR), "hour")} ago`;
  return `${plural(Math.floor(delta / DAY), "day")} ago`;
}

/** "15:00" in the user's clock. */
export function formatClock(ms: number): string {
  const d = new Date(ms);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

/** "16 Oct 2026". */
export function formatDate(iso: string | number): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return String(iso);
  return d.toLocaleDateString("en-GB", { day: "numeric", month: "short", year: "numeric" });
}

/** The kind badge vocabulary (§7.5). Kind badges are neutral; colour is state. */
export const KIND_LABEL: Record<DeployKind, string> = {
  pak: "PAK",
  loose: "LOOSE",
  reframework: "SCRIPT",
  root: "ROOT",
};

export const KIND_DESTINATION: Record<DeployKind, string> = {
  pak: "Lands in pak_mods/",
  loose: "Lands in natives/",
  reframework: "Lands in reframework/",
  root: "Lands in the game root (.dll)",
};

const KIND_ORDER: DeployKind[] = ["pak", "loose", "reframework", "root"];

/** The kinds an option carries, in badge order. */
export function kindsPresent(kinds: PayloadKinds): DeployKind[] {
  return KIND_ORDER.filter((k) => kinds[k] > 0);
}

/** "3 PAK · 9 loose · 2 script". */
export function kindsSentence(kinds: PayloadKinds): string {
  const words: Record<DeployKind, string> = { pak: "PAK", loose: "loose", reframework: "script", root: "root" };
  return kindsPresent(kinds)
    .map((k) => `${kinds[k]} ${words[k]}`)
    .join(" · ");
}

/** The real priority prefix of a managed pak file, e.g. "x0004" or "zrift-0004". */
export function pakPrefix(fileName: string): string {
  const match = /^([a-z]*-?\d{3,5})-/i.exec(fileName);
  return match?.[1] ?? "—";
}

export function lastSegment(path: string): string {
  return path.split(/[\\/]/).at(-1) ?? path;
}
