// Which screen THE FRAY stands in, read from the frame alone (§19.6): the
// fight live (S1, or S8 when the feed went stale), its record (S3), the party
// resting (S5), or a sleeping screen (S6/S7) with or without a fight kept.

import type { FrayFeed, FrayFight, FrayFrame, FrayMember, FrayRole } from "@/types/fray";

export type FraySleep = "asleep" | "norefw" | "noeye" | "done" | "shut" | "silent";
export type FrayDoor = "Open the Riftstone" | "Open Preflight" | "Copy details";

export type FrayMode =
  | { kind: "live"; fight: FrayFight; stale: boolean }
  | { kind: "record"; fight: FrayFight }
  | { kind: "idle" }
  | { kind: "sleep"; sleep: FraySleep; kept: FrayFight | null };

/** The feed's own reason to sleep, in the order a player can fix them. */
export function sleepOf(feed: FrayFeed): FraySleep | null {
  if (!feed.gameRunning) return "asleep";
  if (!feed.reframework) return "norefw";
  if (!feed.eyeInstalled) return "noeye";
  // The eye's stop line: a recording that finished, never a hook that failed (WINDOW-CONTRACT § v0.6).
  if (feed.eyeRecording === false) return feed.eyeReason === "stopped" ? "done" : "shut";
  if (feed.state === "none") return "silent";
  return null;
}

/** How long the window waits on pulls that fail before it calls a live feed stale: the
 *  spine's own heartbeat deadline (`STALE_AFTER`, 3 s). */
export const PULL_STALE_S = 3;

/**
 * The frame as the window may still believe it. A pull that fails leaves the last frame
 * standing, and a live frame must not stand for long: once no pull has answered for
 * `PULL_STALE_S`, a live feed reads stale, its heartbeat aged by the wait.
 */
export function freshened(frame: FrayFrame | null, lastPullMs: number | null, nowMs: number): FrayFrame | null {
  if (!frame || lastPullMs === null || frame.feed.state !== "live") return frame;
  const waited = (nowMs - lastPullMs) / 1000;
  if (waited < PULL_STALE_S) return frame;
  return { ...frame, feed: { ...frame.feed, state: "stale", lastBeatAgoS: (frame.feed.lastBeatAgoS ?? 0) + waited } };
}

const keptOf = (frame: FrayFrame): FrayFight | null => frame.fight ?? frame.recent.at(0) ?? null;

export function modeOf(frame: FrayFrame): FrayMode {
  const sleep = sleepOf(frame.feed);
  if (sleep) return { kind: "sleep", sleep, kept: keptOf(frame) };
  const fight = frame.fight;
  if (!fight)
    return frame.feed.state === "stale" ? { kind: "sleep", sleep: "silent", kept: keptOf(frame) } : { kind: "idle" };
  if (fight.state === "open") return { kind: "live", fight, stale: frame.feed.state === "stale" };
  return { kind: "record", fight };
}

const ROLES: FrayRole[] = ["arisen", "mainPawn", "hiredPawn", "hiredPawn"];

/** A place nobody has read yet: named by its role, no vocation, no reading. */
export function placeholder(slot: number): FrayMember {
  return {
    slot,
    role: ROLES[slot] ?? "hiredPawn",
    name: null,
    vocation: { id: null, name: null },
    hp: null,
    st: null,
    down: false,
    left: false,
    damage: 0,
    dps: null,
    hits: 0,
    skills: [],
    noDamageSkills: null,
    parts: null,
    mix: null,
  };
}

/**
 * The party with no fight on the stele (S5, S6 fresh, the silent eye): the
 * eye's own roster (`frame.party`). Before the eye's first roster it is empty,
 * so the newest shelf fight's party stands in, and failing that the four
 * places by role.
 */
export function placesOf(frame: FrayFrame): (FrayMember | null)[] {
  if (frame.party.length) return frame.party;
  return frame.recent.at(0)?.members ?? ROLES.map((_, slot) => placeholder(slot));
}

/**
 * `· new` (a pawn hired this session): a named hired pawn no fight on the
 * shelf has. The contract has no such flag; this reads it from the shelf.
 */
export function isNewcomer(m: FrayMember, shelf: FrayFight[]): boolean {
  if (m.role !== "hiredPawn" || m.name === null || !shelf.length) return false;
  return !shelf.some((f) => f.members.some((x) => x?.name === m.name));
}
