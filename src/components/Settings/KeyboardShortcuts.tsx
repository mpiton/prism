import type { ReactElement } from "react";

interface ShortcutRow {
  readonly action: string;
  readonly keys: readonly string[];
}

interface ShortcutGroup {
  readonly title: string;
  readonly shortcuts: readonly ShortcutRow[];
}

/**
 * Source of truth for the static shortcut catalog surfaced in Settings.
 * Keep this list aligned with the actual handlers in:
 * - src/hooks/useKeyboard.ts (global + list navigation)
 * - src/components/Sidebar/Sidebar.tsx (sidebar arrow navigation)
 * - src/components/CommandPalette/CommandPalette.tsx (palette-specific)
 *
 * The literal token "Mod" is rendered as ⌘ on macOS and Ctrl elsewhere.
 */
const SHORTCUT_GROUPS: readonly ShortcutGroup[] = [
  {
    title: "Global",
    shortcuts: [
      { action: "Open Command Palette", keys: ["Mod", "K"] },
      { action: "Back to overview", keys: ["Esc"] },
      { action: "Switch to Workspace 1", keys: ["Mod", "1"] },
      { action: "Switch to Workspace 2", keys: ["Mod", "2"] },
      { action: "Switch to Workspace 3", keys: ["Mod", "3"] },
    ],
  },
  {
    title: "List Navigation",
    shortcuts: [
      { action: "Navigate list down", keys: ["j"] },
      { action: "Navigate list up", keys: ["k"] },
      { action: "Navigate sidebar up/down", keys: ["↑", "↓"] },
      { action: "Jump to first / last sidebar item", keys: ["Home", "End"] },
      { action: "Open selected item", keys: ["Enter"] },
      { action: "Open workspace for item", keys: ["w"] },
    ],
  },
  {
    title: "Command Palette",
    shortcuts: [
      { action: "Navigate results", keys: ["↑", "↓"] },
      { action: "Open result in browser", keys: ["Mod", "Enter"] },
      { action: "Close palette", keys: ["Esc"] },
    ],
  },
];

// Evaluated lazily on each render so tests can mutate navigator.userAgent
// between renders without needing `vi.resetModules()`. PRism is a Tauri
// desktop app, so we match the macOS "Macintosh" UA token specifically and
// ignore mobile Apple platforms the app never ships on.
function isMacPlatform(): boolean {
  if (typeof navigator === "undefined") return false;
  return /Macintosh/i.test(navigator.userAgent);
}

function displayKey(key: string, mac: boolean): string {
  if (key === "Mod") return mac ? "⌘" : "Ctrl";
  return key;
}

const kbdClass =
  "bg-surface border-border rounded border px-1.5 py-0.5 font-mono text-xs text-white";

export function KeyboardShortcuts(): ReactElement {
  const mac = isMacPlatform();

  return (
    <div data-testid="settings-keyboard-shortcuts" className="flex flex-col gap-3">
      <h2 className="text-accent text-sm font-semibold uppercase tracking-wider">
        Keyboard Shortcuts
      </h2>
      {SHORTCUT_GROUPS.map((group) => (
        <div key={group.title} className="flex flex-col gap-2">
          <h3 className="text-dim text-xs font-semibold uppercase tracking-wider">{group.title}</h3>
          <dl className="flex flex-col gap-1">
            {group.shortcuts.map((shortcut) => (
              <div
                key={shortcut.action}
                className="flex items-center justify-between gap-4 text-sm"
              >
                <dt className="text-white">{shortcut.action}</dt>
                <dd className="flex items-center gap-1">
                  {shortcut.keys.map((key, index) => (
                    <span key={`${shortcut.action}-${key}`} className="flex items-center gap-1">
                      {index > 0 ? (
                        <span className="text-dim" aria-hidden="true">
                          +
                        </span>
                      ) : null}
                      <kbd className={kbdClass}>{displayKey(key, mac)}</kbd>
                    </span>
                  ))}
                </dd>
              </div>
            ))}
          </dl>
        </div>
      ))}
    </div>
  );
}
