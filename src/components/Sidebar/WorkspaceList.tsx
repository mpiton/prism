import type { ReactElement } from "react";
import type { Workspace, WorkspaceState } from "../../lib/types";

interface WorkspaceListProps {
  readonly workspaces: readonly Workspace[];
  readonly onWorkspaceClick: (workspaceId: string) => void;
}

const DOT_COLOR: Record<WorkspaceState, string> = {
  active: "bg-green",
  suspended: "bg-orange",
  archived: "bg-dim",
};

export function WorkspaceList({
  workspaces,
  onWorkspaceClick,
}: WorkspaceListProps): ReactElement {
  const visible = workspaces.filter((ws) => ws.state !== "archived");

  return (
    <div className="flex flex-col gap-0.5">
      {visible.map((ws) => (
        <button
          key={ws.id}
          type="button"
          onClick={() => onWorkspaceClick(ws.id)}
          aria-label={`PR #${ws.pullRequestNumber} (${ws.state})`}
          className="flex items-center gap-2 rounded px-2 py-1 text-left text-xs text-dim hover:bg-surface-hover hover:text-foreground"
        >
          <span
            data-state={ws.state}
            className={`inline-block h-2 w-2 shrink-0 rounded-full ${DOT_COLOR[ws.state]}`}
          />
          <span>PR #{ws.pullRequestNumber}</span>
        </button>
      ))}
    </div>
  );
}
