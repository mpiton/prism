import { useCallback, useMemo, useState, type ReactElement } from "react";
import { useQueryClient } from "@tanstack/react-query";
import type { Workspace, WorkspaceListEntry, WorkspaceStatusInfo } from "../../lib/types";
import { resumeWorkspace } from "../../lib/tauri";
import { useWorkspacesStore } from "../../stores/workspaces";
import { WorkspaceSwitcher } from "./WorkspaceSwitcher";
import { Terminal } from "./Terminal";
import { WorkspaceStatusBar } from "./WorkspaceStatusBar";
import { WorkspaceListPage } from "./WorkspaceListPage";

interface WorkspaceViewProps {
  readonly workspaces: readonly Workspace[];
  readonly statusInfo: Readonly<Record<string, WorkspaceStatusInfo>>;
  readonly entries: readonly WorkspaceListEntry[];
  readonly onBackToDashboard: () => void;
}

export function WorkspaceView({
  workspaces,
  statusInfo,
  entries,
  onBackToDashboard,
}: WorkspaceViewProps): ReactElement {
  const queryClient = useQueryClient();
  const activeWorkspaceId = useWorkspacesStore((s) => s.activeWorkspaceId);
  const setActiveWorkspace = useWorkspacesStore((s) => s.setActiveWorkspace);
  const active = workspaces.find((w) => w.id === activeWorkspaceId);
  const info = active ? statusInfo[active.id] : undefined;
  const isSuspended = active !== undefined && active.state === "suspended";

  const visibleEntries = useMemo(
    () => entries.filter((e) => e.workspace.state !== "archived"),
    [entries],
  );

  const [waking, setWaking] = useState(false);

  const handleWakeWorkspace = useCallback(async () => {
    if (!active || waking) return;
    setWaking(true);
    try {
      await resumeWorkspace(active.id);
      queryClient.invalidateQueries({ queryKey: ["workspaces"] });
      queryClient.invalidateQueries({ queryKey: ["github", "dashboard"] });
    } catch (err: unknown) {
      console.error("[WorkspaceView] failed to wake workspace:", err);
    } finally {
      setWaking(false);
    }
  }, [active, waking, queryClient]);

  const handleWorkspaceClick = useCallback(
    async (id: string) => {
      const entry = visibleEntries.find((e) => e.workspace.id === id);
      if (!entry) return;

      try {
        if (entry.workspace.state === "suspended") {
          await resumeWorkspace(id);
        }
        setActiveWorkspace(id);
        queryClient.invalidateQueries({ queryKey: ["workspaces"] });
        queryClient.invalidateQueries({ queryKey: ["github", "dashboard"] });
      } catch (err: unknown) {
        console.error("[WorkspaceView] failed to resume workspace:", err);
      }
    },
    [visibleEntries, queryClient, setActiveWorkspace],
  );

  return (
    <section
      data-testid="workspace-view"
      className="flex h-full flex-col"
    >
      <WorkspaceSwitcher
        workspaces={workspaces}
        onBackToDashboard={onBackToDashboard}
      />

      {active && isSuspended ? (
        <div
          data-testid="workspace-suspended-placeholder"
          className="flex flex-1 flex-col items-center justify-center gap-4 bg-surface text-neutral-400"
        >
          <span>Workspace suspended — click Wake to resume</span>
          <button
            data-testid="btn-wake-workspace"
            type="button"
            className="rounded bg-neutral-700 px-4 py-2 text-sm text-white hover:bg-neutral-600 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-blue-500 disabled:cursor-not-allowed disabled:opacity-50"
            disabled={waking}
            onClick={handleWakeWorkspace}
          >
            {waking ? "Waking…" : "Wake"}
          </button>
        </div>
      ) : active ? (
        <>
          <div className="min-h-0 flex-1">
            <Terminal ptyId={active.id} />
          </div>
          {info && (
            <WorkspaceStatusBar
              workspaceId={active.id}
              branch={info.branch}
              ahead={info.ahead}
              behind={info.behind}
              ciStatus={info.ciStatus}
              sessionName={info.sessionName}
              sessionCount={info.sessionCount}
              githubUrl={info.githubUrl}
            />
          )}
        </>
      ) : (
        <WorkspaceListPage entries={visibleEntries} onWorkspaceClick={handleWorkspaceClick} />
      )}
    </section>
  );
}
