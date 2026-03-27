import { useEffect, useRef } from "react";

export interface KeyboardActions {
  readonly onNavigate: (direction: "up" | "down") => void;
  readonly onOpen: () => void;
  readonly onOpenWorkspace: () => void;
  readonly onSwitchWorkspace: (index: number) => void;
  readonly onEscape: () => void;
  readonly onCommandPalette: () => void;
}

const INPUT_TAGS = new Set(["INPUT", "TEXTAREA", "SELECT"]);

function isInputTarget(event: KeyboardEvent): boolean {
  const target = event.target;
  if (!(target instanceof Element)) return false;
  if (INPUT_TAGS.has(target.tagName)) return true;
  return target instanceof HTMLElement &&
    (target.isContentEditable || target.contentEditable === "true");
}

export function useKeyboard(actions: KeyboardActions): void {
  const actionsRef = useRef(actions);
  actionsRef.current = actions;

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent): void {
      if (isInputTarget(event)) return;

      const key = event.key.length === 1 ? event.key.toLowerCase() : event.key;
      const actionModifier = event.metaKey || event.ctrlKey;
      const anyModifier = actionModifier || event.altKey;

      if (actionModifier && key === "k") {
        event.preventDefault();
        actionsRef.current.onCommandPalette();
        return;
      }

      if (actionModifier && key >= "1" && key <= "3") {
        event.preventDefault();
        actionsRef.current.onSwitchWorkspace(Number(key) - 1);
        return;
      }

      if (anyModifier) return;

      switch (key) {
        case "j":
          actionsRef.current.onNavigate("down");
          break;
        case "k":
          actionsRef.current.onNavigate("up");
          break;
        case "Enter":
          actionsRef.current.onOpen();
          break;
        case "w":
          actionsRef.current.onOpenWorkspace();
          break;
        case "Escape":
          event.preventDefault();
          actionsRef.current.onEscape();
          break;
      }
    }

    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, []);
}
