// The player's hands on the hollow (src/workshop/hollowHands.ts): what a key
// or a click on Tamsin's terminal sends to her, and what stays with the player.

import { describe, expect, it, vi } from "vitest";
import { linkHint, makeHollowKeyHandler, makeHollowLinkActivate, openableLink } from "@/workshop/hollowHands";

function key(k: string, mods: Partial<KeyboardEventInit> = {}, type = "keydown"): KeyboardEvent {
  return new KeyboardEvent(type, { key: k, ...mods });
}

function selection(text: string | null) {
  return {
    hasSelection: () => text !== null,
    getSelection: () => text ?? "",
    clearSelection: vi.fn<() => void>(),
  };
}

function hooks(copy: (text: string) => Promise<void> = () => Promise.resolve()) {
  return {
    copy: vi.fn<(text: string) => Promise<void>>(copy),
    interrupt: vi.fn<() => void>(),
    copyFailed: vi.fn<() => void>(),
  };
}

function click(mods: Partial<MouseEventInit> = {}): MouseEvent {
  return new MouseEvent("click", { button: 0, cancelable: true, ...mods });
}

const SIGN_IN = "https://claude.com/cai/oauth/authorize?code=true&client_id=invented&state=invented";

describe("the copy chord", () => {
  it("should copy a selection on Ctrl+C and send nothing to her", async () => {
    const sel = selection("the code");
    const h = hooks();
    expect(makeHollowKeyHandler(sel, h)(key("c", { ctrlKey: true }))).toBe(false);
    await Promise.resolve();
    await Promise.resolve();
    expect(h.copy).toHaveBeenCalledWith("the code");
    expect(sel.clearSelection).toHaveBeenCalled();
    expect(h.interrupt).not.toHaveBeenCalled();
  });

  it("should let a plain Ctrl+C through as her interrupt when nothing is selected", () => {
    const h = hooks();
    expect(makeHollowKeyHandler(selection(null), h)(key("c", { ctrlKey: true }))).toBe(true);
    expect(h.copy).not.toHaveBeenCalled();
  });

  it("should swallow Ctrl+Shift+C with nothing selected, never sending a stray ^C", () => {
    const h = hooks();
    expect(makeHollowKeyHandler(selection(null), h)(key("C", { ctrlKey: true, shiftKey: true }))).toBe(false);
    expect(h.copy).not.toHaveBeenCalled();
  });

  it("should interrupt her after all when the clipboard refuses a plain Ctrl+C, and say so", async () => {
    const sel = selection("text");
    const h = hooks(() => Promise.reject(new Error("refused")));
    makeHollowKeyHandler(sel, h)(key("c", { ctrlKey: true }));
    await vi.waitFor(() => expect(h.interrupt).toHaveBeenCalledOnce());
    expect(h.copyFailed).toHaveBeenCalledOnce();
    expect(sel.clearSelection).not.toHaveBeenCalled();
  });

  it("should never interrupt her for a refused Ctrl+Shift+C", async () => {
    const h = hooks(() => Promise.reject(new Error("refused")));
    makeHollowKeyHandler(selection("text"), h)(key("C", { ctrlKey: true, shiftKey: true }));
    await vi.waitFor(() => expect(h.copyFailed).toHaveBeenCalledOnce());
    expect(h.interrupt).not.toHaveBeenCalled();
  });

  it("should act on keydown only, so one chord copies once", () => {
    const h = hooks();
    expect(makeHollowKeyHandler(selection("text"), h)(key("c", { ctrlKey: true }, "keyup"))).toBe(true);
    expect(h.copy).not.toHaveBeenCalled();
  });

  it("should leave Ctrl+Alt+C and other keys to xterm", () => {
    const handle = makeHollowKeyHandler(selection("text"), hooks());
    expect(handle(key("c", { ctrlKey: true, altKey: true }))).toBe(true);
    expect(handle(key("c"))).toBe(true);
    expect(handle(key("x", { ctrlKey: true }))).toBe(true);
  });
});

describe("the paste chord", () => {
  it("should hand Ctrl+V, Ctrl+Shift+V and Shift+Insert to the webview's own paste", () => {
    const h = hooks();
    const handle = makeHollowKeyHandler(selection(null), h);
    expect(handle(key("v", { ctrlKey: true }))).toBe(false);
    expect(handle(key("V", { ctrlKey: true, shiftKey: true }))).toBe(false);
    expect(handle(key("Insert", { shiftKey: true }))).toBe(false);
    expect(h.copy).not.toHaveBeenCalled();
    expect(h.interrupt).not.toHaveBeenCalled();
  });

  it("should leave a plain v, Insert and Ctrl+Alt+V to xterm", () => {
    const handle = makeHollowKeyHandler(selection(null), hooks());
    expect(handle(key("v"))).toBe(true);
    expect(handle(key("Insert"))).toBe(true);
    expect(handle(key("v", { ctrlKey: true, altKey: true }))).toBe(true);
  });
});

describe("a link on her screen", () => {
  it("should open an https link on Ctrl+click and name its host", async () => {
    const open = vi.fn<(url: string) => Promise<void>>(() => Promise.resolve());
    const opened = vi.fn<(host: string) => void>();
    const ev = click({ ctrlKey: true });
    makeHollowLinkActivate({ open, opened, failed: vi.fn<(url: string) => void>() })(ev, SIGN_IN);
    expect(open).toHaveBeenCalledWith(SIGN_IN);
    expect(ev.defaultPrevented).toBe(true);
    await vi.waitFor(() => expect(opened).toHaveBeenCalledWith("claude.com"));
  });

  it("should never open on a plain click, which belongs to selecting", () => {
    const open = vi.fn<(url: string) => Promise<void>>(() => Promise.resolve());
    makeHollowLinkActivate({ open, opened: vi.fn<(host: string) => void>(), failed: vi.fn<(url: string) => void>() })(
      click(),
      SIGN_IN,
    );
    expect(open).not.toHaveBeenCalled();
  });

  it("should open nothing but https, whatever she prints", () => {
    const open = vi.fn<(url: string) => Promise<void>>(() => Promise.resolve());
    const activate = makeHollowLinkActivate({
      open,
      opened: vi.fn<(host: string) => void>(),
      failed: vi.fn<(url: string) => void>(),
    });
    for (const text of [
      "http://claude.com/x",
      "file:///etc/passwd",
      "javascript:alert(1)",
      "ms-settings:",
      "not a url",
    ]) {
      activate(click({ ctrlKey: true }), text);
    }
    expect(open).not.toHaveBeenCalled();
  });

  it("should hand a link the stone could not open to the failure path, whole", async () => {
    const failed = vi.fn<(url: string) => void>();
    makeHollowLinkActivate({
      open: () => Promise.reject(new Error("no")),
      opened: vi.fn<(host: string) => void>(),
      failed,
    })(click({ ctrlKey: true }), SIGN_IN);
    await vi.waitFor(() => expect(failed).toHaveBeenCalledWith(SIGN_IN));
  });

  it("should hint at the Ctrl+click on an https link only", () => {
    expect(linkHint(SIGN_IN)).toBe("Ctrl+click to open claude.com in your browser");
    expect(linkHint("http://claude.com")).toBe("");
    expect(openableLink("https://claude.com")?.host).toBe("claude.com");
  });
});
