import { useRef, useCallback, useMemo } from "react";
import { FOCUS_RING } from "../../lib/a11y";
import { useWorkspacesStore } from "../../stores/workspaces";
import { useSettingsStore } from "../../stores/settings";
import type { Workspace, WorkspaceState } from "../../lib/types";

interface WorkspaceSwitcherProps {
  readonly workspaces: readonly Workspace[];
  readonly onBackToDashboard: () => void;
}

const DEFAULT_MAX_ACTIVE = 3;

const STATE_DOT_CLASS = {
  active: "bg-green",
  suspended: "bg-orange",
  archived: "bg-dim",
} satisfies Record<WorkspaceState, string>;

function sanitizeMax(value: number | undefined | null): number {
  return Number.isFinite(value) && value! > 0 ? value! : DEFAULT_MAX_ACTIVE;
}

export function WorkspaceSwitcher({ workspaces, onBackToDashboard }: WorkspaceSwitcherProps) {
  const activeWorkspaceId = useWorkspacesStore((s) => s.activeWorkspaceId);
  const setActiveWorkspace = useWorkspacesStore((s) => s.setActiveWorkspace);
  const maxActiveWorkspaces = sanitizeMax(useSettingsStore((s) => s.config?.maxActiveWorkspaces));

  const visibleWorkspaces = useMemo(
    () => workspaces.filter((w) => w.state !== "archived"),
    [workspaces],
  );
  const activeCount = workspaces.filter((w) => w.state === "active").length;
  const tabRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const focusedId = visibleWorkspaces.some((w) => w.id === activeWorkspaceId)
    ? activeWorkspaceId
    : (visibleWorkspaces[0]?.id ?? null);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent, index: number) => {
      const count = visibleWorkspaces.length;
      if (count === 0) return;

      let next: number | null = null;
      switch (e.key) {
        case "ArrowRight":
          next = (index + 1) % count;
          break;
        case "ArrowLeft":
          next = (index - 1 + count) % count;
          break;
        case "Home":
          next = 0;
          break;
        case "End":
          next = count - 1;
          break;
        default:
          return;
      }

      e.preventDefault();
      const ws = visibleWorkspaces[next];
      if (ws) {
        setActiveWorkspace(ws.id);
        tabRefs.current[next]?.focus();
      }
    },
    [visibleWorkspaces, setActiveWorkspace],
  );

  return (
    <nav
      className="flex items-center gap-1 border-b border-border bg-surface px-2 py-1"
      data-testid="workspace-switcher"
    >
      <button
        type="button"
        onClick={onBackToDashboard}
        className={`${FOCUS_RING} mr-2 rounded px-2 py-2 text-xs text-dim hover:bg-surface-hover hover:text-text`}
      >
        Dashboard
      </button>

      <div className="flex items-center gap-1" role="tablist" aria-label="Workspaces">
        {visibleWorkspaces.map((ws, i) => {
          const isActive = ws.id === focusedId;
          return (
            <button
              key={ws.id}
              ref={(el) => {
                tabRefs.current[i] = el;
              }}
              type="button"
              role="tab"
              aria-selected={isActive}
              tabIndex={isActive ? 0 : -1}
              data-testid={`tab-${ws.id}`}
              data-active={isActive ? "true" : "false"}
              onClick={() => setActiveWorkspace(ws.id)}
              onKeyDown={(e) => handleKeyDown(e, i)}
              className={`${FOCUS_RING} flex items-center gap-1.5 rounded px-2.5 py-2 text-xs transition-colors ${
                isActive
                  ? "bg-surface-hover text-accent hover:bg-accent/10"
                  : "text-dim hover:bg-surface-hover hover:text-text"
              }`}
            >
              <span
                data-testid={`dot-${ws.id}`}
                className={`inline-block h-1.5 w-1.5 rounded-full ${STATE_DOT_CLASS[ws.state]}`}
              />
              #{ws.pullRequestNumber}
            </button>
          );
        })}
      </div>

      <span className="ml-auto text-xs text-muted">
        {activeCount}/{maxActiveWorkspaces}
      </span>
    </nav>
  );
}
