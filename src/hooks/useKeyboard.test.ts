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

  it("should not trigger command palette when Alt+K is pressed", () => {
    fireKey("k", { altKey: true });
    expect(actions.onCommandPalette).not.toHaveBeenCalled();
    expect(actions.onNavigate).not.toHaveBeenCalled();
  });

  it("should not switch workspace when Alt+1/2/3 is pressed", () => {
    fireKey("1", { altKey: true });
    fireKey("2", { altKey: true });
    fireKey("3", { altKey: true });
    expect(actions.onSwitchWorkspace).not.toHaveBeenCalled();
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

  it("should not fire when typing in a select element", () => {
    const select = document.createElement("select");
    document.body.appendChild(select);
    select.dispatchEvent(new KeyboardEvent("keydown", { key: "j", bubbles: true }));
    expect(actions.onNavigate).not.toHaveBeenCalled();
    document.body.removeChild(select);
  });

  it("should not fire when typing in a contenteditable element", () => {
    const div = document.createElement("div");
    div.contentEditable = "true";
    document.body.appendChild(div);
    div.dispatchEvent(new KeyboardEvent("keydown", { key: "j", bubbles: true }));
    expect(actions.onNavigate).not.toHaveBeenCalled();
    document.body.removeChild(div);
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
    fireKey("j", { altKey: true });
    fireKey("Enter", { altKey: true });
    fireKey("Escape", { altKey: true });
    expect(actions.onNavigate).not.toHaveBeenCalled();
    expect(actions.onOpenWorkspace).not.toHaveBeenCalled();
    expect(actions.onOpen).not.toHaveBeenCalled();
    expect(actions.onEscape).not.toHaveBeenCalled();
  });
});
