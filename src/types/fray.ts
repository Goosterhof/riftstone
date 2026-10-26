// THE FRAY's window contract — the one frame the `fray` window pulls from the spine.
//
// The Rust side (crates/rift-fray/src/frame.rs) mirrors these types field for
// field with `#[serde(rename_all = "camelCase")]`. Change one, change both, in
// the same commit. The shape is design system #00015 §19.15's "Contract gaps",
// and every rule the window draws by is §19 (THE FRAY).
//
// Two scopes, never one total (§19.15 item 3, FR-D1): `hParty` is the four
// members' damage, `hAll` every source the vein draws (the members, the
// unattributed and the world). The honesty line divides hParty by the game's
// own party ledger; a share bar divides by hAll (FQ.23, ruled "every source").

/** The seven additive parts of one hit's damage, in this order (§19.4). */
export const FRAY_PARTS = ["Slash", "Strike", "Shoot", "Magick", "Enchant", "Element", "Fixed"] as const;
export type FrayParts = [number, number, number, number, number, number, number];

/** How a member's damage key was named: a custom skill, a bare action hash, or nothing. */
export type FraySkillKind = "skill" | "action" | "unknown";

export interface FraySkill {
  kind: FraySkillKind;
  /** The key: the custom skill id, the action hash in hex, a shell param id,
   *  or "unknown". Stable within a session, so the 4 s beat can re-rank by it. */
  id: string;
  /** The game's own text for it, or null when the stone cannot name it (RF.7). */
  name: string | null;
  damage: number;
  hits: number;
  biggest: number;
  /** True when one of its hits was a foe's killing blow. */
  lastBlow: boolean;
  /** The fight second of its first hit: the tie-break when two keys deal the same. */
  firstAt: number;
}

export type FrayRole = "arisen" | "mainPawn" | "hiredPawn";

export interface FrayVocation {
  id: number | null;
  name: string | null;
}

export interface FrayMember {
  /** 0 the Arisen, 1 the main pawn, 2 and 3 the hired pawns (fixed order, never re-sorted). */
  slot: number;
  role: FrayRole;
  /** From THE MUSTER's save read; null until the game saves (FQ.9, R17.21). */
  name: string | null;
  vocation: FrayVocation;
  /** Fractions 0..1, or null when the feed could not read them. */
  hp: number | null;
  st: number | null;
  down: boolean;
  /** The member left mid-fight: the card keeps its figures, frozen, with `· left` (RF.3). */
  left: boolean;
  damage: number;
  /** damage ÷ the fight's duration, per second; null while the duration is 0. */
  dps: number | null;
  hits: number;
  /** Every damage key of this fight, unsorted (the window sorts on its beat). */
  skills: FraySkill[];
  /** Skills seen used without damage (a support pawn's evidence), or null when none were observable (FQ.15). */
  noDamageSkills: { name: string; count: number }[] | null;
  /** The additive split (sums to `damage`), or null when the types are not additive. */
  parts: FrayParts | null;
  /** The component mix (does not sum to damage; RF.20), or null. */
  mix: FrayParts | null;
}

export interface FrayFoe {
  /** The game's CharacterID; foes of one kind group under it. */
  cid: number | null;
  name: string | null;
  count: number;
}

export interface FrayKill {
  /** The member who dealt the killing blow, or null when it was no member's. */
  slot: number | null;
  /** That blow's skill key id. */
  key: string | null;
  /** Fight seconds. */
  at: number;
}

export type FrayAttribution = "checked" | "unqualified" | "inconclusive";
export type FrayTypes = "additive" | "mix" | "unknown";
export type FraySkillsState = "available" | "unavailable";

export interface FraySource {
  damage: number;
  hits: number;
  biggest: number;
}

export interface FrayFight {
  /** Unique within the stone's run. */
  id: string;
  /** Its number this session: FIGHT 7. */
  n: number;
  state: "open" | "settled";
  /** Game seconds, pauses excluded. */
  durationS: number;
  paused: boolean;
  foes: FrayFoe[];
  /** Local wall-clock time it settled, "21:04", or null while open. */
  endedAt: string | null;
  /** Four entries, slot order; null where the slot is empty. */
  members: (FrayMember | null)[];
  hParty: number;
  hAll: number;
  unattributed: FraySource;
  /** The world row (throws, terrain, gimmicks): only on evidence, else null. */
  world: FraySource | null;
  /** The game's own party ledger (TotalDamageFromPlayerParty) over this fight, or null. */
  ledger: number | null;
  /** False when no roster was read, so no member could be credited. */
  partyFormed: boolean;
  attribution: FrayAttribution;
  types: FrayTypes;
  skills: FraySkillsState;
  kills: FrayKill[];
  /** Records the feed lost inside this fight (sequence gaps). */
  gaps: number;
  biggest: { slot: number; key: string; damage: number } | null;
}

export type FrayFeedState = "live" | "stale" | "none";

export interface FrayFeed {
  state: FrayFeedState;
  /** Seconds since the last heartbeat, or null when there has been none. */
  lastBeatAgoS: number | null;
  gameRunning: boolean;
  reframework: boolean;
  eyeInstalled: boolean;
  /** Whether the eye says it is recording, and why not when it is not. */
  eyeRecording: boolean | null;
  eyeReason: string | null;
}

export interface FrayFrame {
  feed: FrayFeed;
  /** The fight on the stele now: open while it runs, the last settled one after. */
  fight: FrayFight | null;
  /** Up to five settled fights, newest first (THE STRATA, RF.26). */
  recent: FrayFight[];
}
