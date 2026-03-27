import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, type RenderHookResult } from "@testing-library/react";
import { useKeyboard, type KeyboardActions } from "./useKeyboard";

function fireKey(
  key: string,
  opts: Partial<KeyboardEventInit> = {},
): void {
  document.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, ...opts }));
}

function createActions(): KeyboardActions {
  return {
    onNavigate: vi.fn(),
    onOpen: vi.fn(),
    onOpenWorkspace: vi.fn(),
    onSwitchWorkspace: vi.fn(),
    onEscape: vi.fn(),
    onCommandPalette: vi.fn(),
  };
}

describe("useKeyboard", () => {
  let actions: KeyboardActions;
  let hook: RenderHookResult<void, KeyboardActions>;

  beforeEach(() => {
    actions = createActions();
    hook = renderHook(() => useKeyboard(actions));
  });

  afterEach(() => {
    hook.unmount();
  });

  it("should navigate down when j is pressed", () => {
    fireKey("j");
    expect(actions.onNavigate).toHaveBeenCalledWith("down");
  });

  it("should navigate up when k is pressed", () => {
    fireKey("k");
    expect(actions.onNavigate).toHaveBeenCalledWith("up");
  });

  it("should call onOpen when Enter is pressed", () => {
    fireKey("Enter");
    expect(actions.onOpen).toHaveBeenCalledOnce();
  });

  it("should call onOpenWorkspace when w is pressed", () => {
    fireKey("w");
    expect(actions.onOpenWorkspace).toHaveBeenCalledOnce();
  });

  it("should call onEscape when Escape is pressed", () => {
    fireKey("Escape");
    expect(actions.onEscape).toHaveBeenCalledOnce();
  });

  it("should open command palette when Cmd+K is pressed", () => {
    fireKey("k", { metaKey: true });
    expect(actions.onCommandPalette).toHaveBeenCalledOnce();
    expect(actions.onNavigate).not.toHaveBeenCalled();
  });

  it("should open command palette when Ctrl+K is pressed", () => {
    fireKey("k", { ctrlKey: true });
    expect(actions.onCommandPalette).toHaveBeenCalledOnce();
    expect(actions.onNavigate).not.toHaveBeenCalled();
  });

  it("should switch workspace when Ctrl+1/2/3 is pressed", () => {
    fireKey("1", { ctrlKey: true });
    fireKey("2", { ctrlKey: true });
    fireKey("3", { ctrlKey: true });
    expect(actions.onSwitchWorkspace).toHaveBeenCalledTimes(3);
    expect(actions.onSwitchWorkspace).toHaveBeenNthCalledWith(1, 0);
    expect(actions.onSwitchWorkspace).toHaveBeenNthCalledWith(2, 1);
    expect(actions.onSwitchWorkspace).toHaveBeenNthCalledWith(3, 2);
  });

  it("should switch workspace when Cmd+1/2/3 is pressed", () => {
    fireKey("1", { metaKey: true });
    fireKey("2", { metaKey: true });
    fireKey("3", { metaKey: true });
    expect(actions.onSwitchWorkspace).toHaveBeenCalledTimes(3);
    expect(actions.onSwitchWorkspace).toHaveBeenNthCalledWith(1, 0);
    expect(actions.onSwitchWorkspace).toHaveBeenNthCalledWith(2, 1);
    expect(actions.onSwitchWorkspace).toHaveBeenNthCalledWith(3, 2);
  });

  it("should not fire when typing in an input element", () => {
    const input = document.createElement("input");
    document.body.appendChild(input);
    input.dispatchEvent(new KeyboardEvent("keydown", { key: "j", bubbles: true }));
    expect(actions.onNavigate).not.toHaveBeenCalled();
    document.body.removeChild(input);
  });

  it("should not fire when typing in a textarea", () => {
    const textarea = document.createElement("textarea");
    document.body.appendChild(textarea);
    textarea.dispatchEvent(new KeyboardEvent("keydown", { key: "k", bubbles: true }));
    expect(actions.onNavigate).not.toHaveBeenCalled();
    document.body.removeChild(textarea);
  });

  it("should clean up listener on unmount", () => {
    hook.unmount();
    fireKey("j");
    expect(actions.onNavigate).not.toHaveBeenCalled();
    // Re-create hook so afterEach unmount doesn't error
    hook = renderHook(() => useKeyboard(actions));
  });

  it("should ignore plain keys when modifier is held", () => {
    fireKey("j", { ctrlKey: true });
    fireKey("w", { metaKey: true });
    expect(actions.onNavigate).not.toHaveBeenCalled();
    expect(actions.onOpenWorkspace).not.toHaveBeenCalled();
  });
});
