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

export function WorkspaceSwitcher({
  workspaces,
  onBackToDashboard,
}: WorkspaceSwitcherProps) {
  const activeWorkspaceId = useWorkspacesStore((s) => s.activeWorkspaceId);
  const setActiveWorkspace = useWorkspacesStore((s) => s.setActiveWorkspace);
  const maxActiveWorkspaces =
    useSettingsStore((s) => s.config?.maxActiveWorkspaces) ?? DEFAULT_MAX_ACTIVE;

  const activeCount = workspaces.filter((w) => w.state === "active").length;

  return (
    <nav
      className="flex items-center gap-1 border-b border-border bg-surface px-2 py-1"
      data-testid="workspace-switcher"
    >
      <button
        type="button"
        onClick={onBackToDashboard}
        className="mr-2 rounded px-2 py-1 text-xs text-dim hover:bg-surface-hover hover:text-text"
      >
        Dashboard
      </button>

      <div className="flex items-center gap-1" role="tablist" aria-label="Workspaces">
        {workspaces.map((ws) => {
          const isActive = ws.id === activeWorkspaceId;
          return (
            <button
              key={ws.id}
              type="button"
              role="tab"
              aria-selected={isActive}
              data-testid={`tab-${ws.id}`}
              data-active={isActive ? "true" : "false"}
              onClick={() => setActiveWorkspace(ws.id)}
              className={`flex items-center gap-1.5 rounded px-2.5 py-1 text-xs transition-colors ${
                isActive
                  ? "bg-surface-hover text-accent"
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
