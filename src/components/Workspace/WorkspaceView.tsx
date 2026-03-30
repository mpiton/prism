import type { ReactElement } from "react";
import type { CiStatus, Workspace } from "../../lib/types";
import { useWorkspacesStore } from "../../stores/workspaces";
import { WorkspaceSwitcher } from "./WorkspaceSwitcher";
import { Terminal } from "./Terminal";
import { WorkspaceStatusBar } from "./WorkspaceStatusBar";

export interface WorkspaceStatusInfo {
  readonly branch: string;
  readonly ahead: number;
  readonly behind: number;
  readonly ciStatus: CiStatus;
  readonly sessionName: string | null;
  readonly sessionCount: number;
  readonly githubUrl: string;
}

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

      {active && info ? (
        <>
          <div className="min-h-0 flex-1">
            <Terminal ptyId={active.id} />
          </div>
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
        </>
      ) : (
        <div className="flex flex-1 items-center justify-center text-dim">
          Select a workspace to begin
        </div>
      )}
    </section>
  );
}
