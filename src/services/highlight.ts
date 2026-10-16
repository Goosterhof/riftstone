// The proof of a match (Bold Choice 3): split a query into the tokens the
// spine searched for, then cut any field into lit and unlit runs.

export interface Run {
  text: string;
  hit: boolean;
}

/** Lower-cased tokens; "quoted phrases" stay whole — the spine's own rule. */
export function searchTokens(text: string): string[] {
  const out: string[] = [];
  for (const [i, part] of text.toLowerCase().split('"').entries()) {
    if (i % 2 === 1) {
      const phrase = part.trim();
      if (phrase) out.push(phrase);
    } else {
      out.push(...part.split(/\s+/).filter(Boolean));
    }
  }
  return out;
}

/** A token this short ("1", "v2") lights only where it stands on its own:
 *  both of its edges sit on a boundary — the string's end, punctuation, or a
 *  letter↔digit change ("Variation1" yes; ".pfb.18" and "260421070" no).
 *  Without this, a one-character token lights noise inside every hash. */
export const SHORT_TOKEN = 2;

function charClass(ch: string | undefined): "a" | "d" | "-" {
  if (ch === undefined) return "-";
  if (/[0-9]/.test(ch)) return "d";
  return /[\p{L}]/u.test(ch) ? "a" : "-";
}

function edge(lower: string, at: number): boolean {
  const before = charClass(lower[at - 1]);
  const after = charClass(lower[at]);
  return before === "-" || after === "-" || before !== after;
}

function standsAlone(lower: string, at: number, token: string): boolean {
  return token.length > SHORT_TOKEN || (edge(lower, at) && edge(lower, at + token.length));
}

function markHits(lower: string, tokens: string[]): boolean[] {
  const lit: boolean[] = Array.from({ length: lower.length }, () => false);
  for (const token of tokens) {
    let at = lower.indexOf(token);
    while (at >= 0 && token.length > 0) {
      if (standsAlone(lower, at, token)) lit.fill(true, at, at + token.length);
      at = lower.indexOf(token, at + 1);
    }
  }
  return lit;
}

/** True when `text` carries `token` the way markHits would light it. */
export function carriesToken(text: string, token: string): boolean {
  return markHits(text.toLowerCase(), [token]).includes(true);
}

/** Cut `text` into runs; every occurrence of every token is lit. */
export function highlightRuns(text: string, tokens: string[]): Run[] {
  if (!tokens.length || !text) return [{ text, hit: false }];
  const lit = markHits(text.toLowerCase(), tokens);
  const runs: Run[] = [];
  let start = 0;
  for (let i = 1; i <= text.length; i++) {
    if (i === text.length || lit[i] !== lit[start]) {
      runs.push({ text: text.slice(start, i), hit: lit[start] ?? false });
      start = i;
    }
  }
  return runs;
}

export function containsAnyToken(text: string | null, tokens: string[]): boolean {
  if (!text) return false;
  return tokens.some((t) => carriesToken(text, t));
}

/** A matched archive path cut into the four runs the card's IN FILE line
 *  lays out (Bold Choice 3: the proof of why a card answered).
 *    lead      the directories above the matched segment, middle elided:
 *              "natives/stm/…/" (the first thing to give way on a narrow card)
 *    before    the matched segment up to its first lit character
 *    fragment  first lit character → last lit character: NEVER cut
 *    after     the rest of the segment and anything below it
 *  natives/stm/…/ · mantle_310_f_ · alba · .tex.251211553 */
export interface PathProof {
  lead: string;
  before: string;
  fragment: string;
  after: string;
}

/** The deepest segment that carries a hit: the most specific proof. */
function anchorSegment(parts: string[], tokens: string[]): number {
  for (let i = parts.length - 1; i >= 0; i--) {
    if (containsAnyToken(parts.at(i) ?? "", tokens)) return i;
  }
  return parts.length - 1;
}

/** Where the game-relative path starts inside an archive's wrapper folders
 *  ("Vernworth Wardrobe Collection/ErmineCape-Variation1/natives/…"). */
const GAME_ROOTS = new Set(["natives", "reframework", "pak_mods"]);

function elidedLead(all: string[]): string {
  const from = all.findIndex((d) => GAME_ROOTS.has(d.toLowerCase()));
  const dirs = from > 0 ? all.slice(from) : all;
  if (!dirs.length) return "";
  if (dirs.length <= 2) return `${dirs.join("/")}/`;
  return `${dirs.slice(0, 2).join("/")}/…/`;
}

export function pathProof(path: string, tokens: string[]): PathProof {
  const parts = path.split("/");
  const at = anchorSegment(parts, tokens);
  const segment = parts.at(at) ?? path;
  const below = parts
    .slice(at + 1)
    .map((p) => `/${p}`)
    .join("");
  const lit = markHits(segment.toLowerCase(), tokens);
  const first = lit.indexOf(true);
  const lead = elidedLead(parts.slice(0, at));
  if (first < 0) return { lead, before: segment, fragment: "", after: below };
  const end = lit.lastIndexOf(true) + 1;
  return {
    lead,
    before: segment.slice(0, first),
    fragment: segment.slice(first, end),
    after: segment.slice(end) + below,
  };
}
