// The player's hands on the hollow: what reaches Tamsin, and what stays with
// the player. A bare xterm owned every key, so Ctrl+C became a ^C on her PTY,
// Ctrl+V a ^V, and a link on the screen was only text. Her first sign-in shows
// a URL to open and asks for a code back, so none of it could be done.
//
// - Copy: Ctrl+C with a selection copies it and sends nothing; with none it
//   still interrupts her. Ctrl+Shift+C always copies (lifted from the
//   Mezzanine's copy chord, the war-tent's WR-0912).
// - Paste: Ctrl+V, Ctrl+Shift+V and Shift+Insert are handed back to the
//   webview, whose own paste event xterm reads and sends as one paste
//   (bracketed when her terminal asked for it). No clipboard is read here.
// - A pasted picture (0.7.5): her Claude Code cannot reach the player's
//   clipboard through the fence, and xterm pastes text only. A paste whose
//   clipboard holds a picture and no text goes to the stone instead, which
//   writes it into the workshop's pasted\ and answers its path; the hollow
//   pastes that path into her prompt (pictureIn picks the picture).
// - Links: Ctrl+click opens an https link in the player's browser, one the
//   player chose. Her screen is the fence's output, so nothing else opens:
//   no http, no file, no other scheme, and never a plain click. Claude Code
//   2.1.312 prints its sign-in URL as one OSC 8 link (465 characters) over
//   text it hard-wraps at the width, so the link handler, not a selection, is
//   the way to it.
// - Not OSC 52: her "c to copy" writes nothing. Honouring it would let her
//   set the player's clipboard without a press of theirs.
//
// This is the pure layer; WorkshopHollow.vue wires it to xterm.

export interface HollowSelection {
  hasSelection(): boolean;
  getSelection(): string;
  clearSelection(): void;
}

export interface HollowKeyHooks {
  /** Write the selection to the clipboard; reject on failure. */
  copy: (text: string) => Promise<void>;
  /** A plain Ctrl+C whose copy fails still interrupts her. Called at the keydown, it
   *  returns the ^C for the run on show then: the refusal can come after she has
   *  departed and another was summoned, and that one must never get it (HH1). */
  armInterrupt: () => () => void;
  /** The copy failed; tell the player. */
  copyFailed: () => void;
}

function chord(ev: KeyboardEvent): boolean {
  return ev.ctrlKey && !ev.metaKey && !ev.altKey;
}

function isPaste(ev: KeyboardEvent): boolean {
  const key = ev.key.toLowerCase();
  if (chord(ev) && key === "v") return true;
  return ev.shiftKey && !ev.ctrlKey && !ev.metaKey && !ev.altKey && ev.key === "Insert";
}

/** For xterm's `attachCustomKeyEventHandler`: false keeps the key from her PTY. */
export function makeHollowKeyHandler(term: HollowSelection, hooks: HollowKeyHooks): (ev: KeyboardEvent) => boolean {
  return (ev) => {
    // xterm calls this for keydown and keyup alike; act on keydown only
    if (ev.type !== "keydown") return true;
    // false here returns before xterm cancels the event, so the webview pastes
    if (isPaste(ev)) return false;
    if (!chord(ev) || ev.key.toLowerCase() !== "c") return true;
    const alwaysCopy = ev.shiftKey;
    if (!term.hasSelection()) return !alwaysCopy;
    const interrupt = alwaysCopy ? null : hooks.armInterrupt();
    void hooks
      .copy(term.getSelection())
      .then(() => term.clearSelection())
      .catch(() => {
        hooks.copyFailed();
        interrupt?.();
      });
    return false;
  };
}

/** The picture a paste carries, when it carries a picture and no text: a
 *  screenshot on the clipboard. Text wins, so a copied page's text pastes as
 *  text even when the clipboard holds its images too. */
export function pictureIn(data: DataTransfer | null): File | null {
  if (!data) return null;
  if (data.getData("text/plain").trim()) return null;
  for (const item of data.items) {
    if (item.kind === "file" && item.type.startsWith("image/")) return item.getAsFile();
  }
  return null;
}

/** Bytes as base64, in chunks a call stack can take (a screenshot is megabytes). */
export function base64Of(bytes: Uint8Array): string {
  let binary = "";
  const CHUNK = 0x8000;
  for (let i = 0; i < bytes.length; i += CHUNK) {
    binary += String.fromCharCode(...bytes.subarray(i, i + CHUNK));
  }
  return btoa(binary);
}

/** The link the player may open from her screen, or null. https only. */
export function openableLink(text: string): URL | null {
  try {
    const url = new URL(text);
    return url.protocol === "https:" ? url : null;
  } catch {
    return null;
  }
}

export interface HollowLinkHooks {
  open: (url: string) => Promise<void>;
  opened: (host: string) => void;
  failed: (url: string) => void;
}

/** For the web-links addon and xterm's OSC 8 `linkHandler`: Ctrl+click only. */
export function makeHollowLinkActivate(hooks: HollowLinkHooks): (event: MouseEvent, text: string) => void {
  return (event, text) => {
    if (event.button !== 0 || !event.ctrlKey) return;
    const url = openableLink(text);
    if (!url) return;
    event.preventDefault();
    void hooks.open(url.href).then(
      () => hooks.opened(url.host),
      () => hooks.failed(url.href),
    );
  };
}

/** What the hollow's tooltip says while the pointer rests on a link. */
export function linkHint(text: string): string {
  const url = openableLink(text);
  return url ? `Ctrl+click to open ${url.host} in your browser` : "";
}
