import type { ReactElement } from "react";
import type { WorkspaceListEntry, WorkspaceState } from "../../lib/types";
import { CI } from "../atoms/CI";
import { EmptyState } from "../atoms/EmptyState";

interface WorkspaceListPageProps {
  readonly entries: readonly WorkspaceListEntry[];
  readonly onWorkspaceClick: (workspaceId: string) => void;
}

const DOT_COLOR: Record<WorkspaceState, string> = {
  active: "bg-green",
  suspended: "bg-orange",
  archived: "bg-dim",
};

function formatDiskUsage(entries: readonly WorkspaceListEntry[]): string | null {
  const hasAnyDiskData = entries.some((e) => e.diskUsageMb != null);
  if (!hasAnyDiskData) return null;
  const total = entries.reduce((sum, e) => sum + (e.diskUsageMb ?? 0), 0);
  return `${total} MB`;
}

export function WorkspaceListPage({
  entries,
  onWorkspaceClick,
}: WorkspaceListPageProps): ReactElement {
  if (entries.length === 0) {
    return <EmptyState message="No workspaces yet" />;
  }

  const totalDisk = formatDiskUsage(entries);

  return (
    <section data-testid="workspace-list" className="flex h-full flex-col">
      <ul className="flex-1 divide-y divide-border overflow-y-auto" role="list">
        {entries.map(({ workspace, branch, ahead, behind, ciStatus, sessionCount, diskUsageMb, lastNote }) => (
          <li key={workspace.id}>
            <button
              type="button"
              onClick={() => onWorkspaceClick(workspace.id)}
              aria-label={`PR #${workspace.pullRequestNumber} (${workspace.state})`}
              className="flex w-full items-start gap-3 px-4 py-3 text-left hover:bg-surface-hover"
            >
              <span
                data-state={workspace.state}
                className={`mt-1.5 inline-block h-2 w-2 shrink-0 rounded-full ${DOT_COLOR[workspace.state]}`}
              />

              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="text-sm font-medium text-foreground">
                    PR #{workspace.pullRequestNumber}
                  </span>
                  {ciStatus && <CI status={ciStatus} />}
                </div>

                {branch && (
                  <div className="mt-0.5 flex items-center gap-2 text-xs text-dim">
                    <span className="truncate">{branch}</span>
                    {(ahead > 0 || behind > 0) && (
                      <span>
                        {ahead > 0 && <span className="text-green">+{ahead}</span>}
                        {ahead > 0 && behind > 0 && " "}
                        {behind > 0 && <span className="text-orange">-{behind}</span>}
                      </span>
                    )}
                  </div>
                )}

                <div className="mt-0.5 flex items-center gap-3 text-xs text-dim">
                  <span>{sessionCount} sessions</span>
                  {diskUsageMb != null && <span>{diskUsageMb} MB</span>}
                </div>

                {lastNote && (
                  <p className="mt-1 truncate text-xs text-dim/70">{lastNote}</p>
                )}
              </div>
            </button>
          </li>
        ))}
      </ul>

      {totalDisk && (
        <footer className="border-t border-border px-4 py-2 text-xs text-dim">
          Total: {totalDisk}
        </footer>
      )}
    </section>
  );
}
