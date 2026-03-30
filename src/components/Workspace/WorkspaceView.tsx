import type { ReactElement } from "react";
import type { Workspace, WorkspaceStatusInfo } from "../../lib/types";
import { useWorkspacesStore } from "../../stores/workspaces";
import { WorkspaceSwitcher } from "./WorkspaceSwitcher";
import { Terminal } from "./Terminal";
import { WorkspaceStatusBar } from "./WorkspaceStatusBar";

interface WorkspaceViewProps {
  readonly workspaces: readonly Workspace[];
  readonly statusInfo: Readonly<Record<string, WorkspaceStatusInfo>>;
  readonly onBackToDashboard: () => void;
}

export function WorkspaceView({
  workspaces,
  statusInfo,
  onBackToDashboard,
}: WorkspaceViewProps): ReactElement {
  const activeWorkspaceId = useWorkspacesStore((s) => s.activeWorkspaceId);
  const active = workspaces.find((w) => w.id === activeWorkspaceId);
  const info = active ? statusInfo[active.id] : undefined;

  return (
    <section
      data-testid="workspace-view"
      className="flex h-full flex-col"
    >
      <WorkspaceSwitcher
        workspaces={workspaces}
        onBackToDashboard={onBackToDashboard}
      />

      {active ? (
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
        <div className="flex flex-1 items-center justify-center text-dim">
          Select a workspace to begin
        </div>
      )}
    </section>
  );
}
