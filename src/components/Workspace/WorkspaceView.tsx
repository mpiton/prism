import { lazy, Suspense, useCallback, useState, type ReactElement } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { FOCUS_RING } from "../../lib/a11y";
import type { Workspace, WorkspaceListEntry, WorkspaceStatusInfo } from "../../lib/types/workspace";
import { resumeWorkspace } from "../../lib/tauri";
import { useWorkspacesStore } from "../../stores/workspaces";
import { WorkspaceSwitcher } from "./WorkspaceSwitcher";
import { WorkspaceStatusBar } from "./WorkspaceStatusBar";
import { WorkspaceListPage } from "./WorkspaceListPage";

const Terminal = lazy(() => import("./Terminal").then((module) => ({ default: module.Terminal })));

function TerminalLoadingFallback(): ReactElement {
  return (
    <div
      data-testid="workspace-terminal-loading"
      role="status"
      aria-live="polite"
      className="flex h-full items-center justify-center bg-[#0a0a09] text-sm text-neutral-400"
    >
      Loading terminal…
    </div>
  );
}

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
      const entry = entries.find((e) => e.workspace.id === id);
      if (!entry || entry.workspace.state === "archived") return;

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
    [entries, queryClient, setActiveWorkspace],
  );

  return (
    <section data-testid="workspace-view" className="flex h-full flex-col">
      <WorkspaceSwitcher workspaces={workspaces} onBackToDashboard={onBackToDashboard} />

      {active && isSuspended ? (
        <div
          data-testid="workspace-suspended-placeholder"
          className="flex flex-1 flex-col items-center justify-center gap-4 bg-surface text-neutral-400"
        >
          <span>Workspace suspended — click Wake to resume</span>
          <button
            data-testid="btn-wake-workspace"
            type="button"
            className={`${FOCUS_RING} rounded bg-neutral-700 px-4 py-2 text-sm text-white hover:bg-neutral-600 disabled:cursor-not-allowed disabled:opacity-50`}
            disabled={waking}
            onClick={handleWakeWorkspace}
          >
            {waking ? "Waking…" : "Wake"}
          </button>
        </div>
      ) : active ? (
        <>
          <div className="min-h-0 flex-1">
            <Suspense fallback={<TerminalLoadingFallback />}>
              <Terminal ptyId={active.id} />
            </Suspense>
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
        <WorkspaceListPage entries={entries} onWorkspaceClick={handleWorkspaceClick} />
      )}
    </section>
  );
}
