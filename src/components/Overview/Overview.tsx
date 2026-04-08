import { type ReactElement, useCallback } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useGitHubData } from "../../hooks/useGitHubData";
import { markAllActivityRead, openWorkspace, resumeWorkspace } from "../../lib/tauri";
import { useWorkspacesStore } from "../../stores/workspaces";
import { useDashboardStore } from "../../stores/dashboard";
import { ReviewQueue } from "../ReviewQueue";
import { MyPRs } from "../MyPRs";
import { Issues } from "../Issues";
import { ActivityFeed } from "../ActivityFeed";
import { openUrl } from "../../lib/open";

const MAX_REVIEWS = 5;
const MAX_PRS = 5;
const MAX_ACTIVITIES = 5;

export function Overview(): ReactElement {
  const { dashboard, error } = useGitHubData();
  const queryClient = useQueryClient();

  const markAllRead = useMutation({
    mutationFn: markAllActivityRead,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["github", "dashboard"] });
      queryClient.invalidateQueries({ queryKey: ["github", "stats"] });
    },
    onError: (err: unknown) => {
      console.error("[Overview] markAllActivityRead failed:", err);
    },
  });

  const handleWorkspaceAction = useCallback(
    async (params: {
      readonly repoId: string;
      readonly pullRequestNumber: number;
      readonly headRefName: string;
      readonly workspaceId?: string;
      readonly workspaceState?: string;
    }) => {
      try {
        const { setActiveWorkspace } = useWorkspacesStore.getState();
        const { setView } = useDashboardStore.getState();

        if (params.workspaceId && params.workspaceState !== "archived") {
          // Existing workspace — resume if suspended, then navigate
          if (params.workspaceState === "suspended") {
            await resumeWorkspace(params.workspaceId);
            await queryClient.invalidateQueries({ queryKey: ["workspaces"] });
            await queryClient.invalidateQueries({ queryKey: ["github", "dashboard"] });
          }
          setActiveWorkspace(params.workspaceId);
          setView("workspaces");
          return;
        }

        // No workspace or archived — create a new one
        if (!params.headRefName) {
          console.warn("[Overview] cannot open workspace: branch name unknown (force sync first)");
          return;
        }
        const response = await openWorkspace({
          repoId: params.repoId,
          pullRequestNumber: params.pullRequestNumber,
          branch: params.headRefName,
        });
        await queryClient.invalidateQueries({ queryKey: ["workspaces"] });
        await queryClient.invalidateQueries({
          queryKey: ["github", "dashboard"],
        });
        setActiveWorkspace(response.workspaceId);
        setView("workspaces");
      } catch (err: unknown) {
        console.error("[Overview] workspace action failed:", err);
      }
    },
    [queryClient],
  );

  if (!dashboard && error) {
    return (
      <div className="flex h-full items-center justify-center text-dim">
        Failed to load dashboard
      </div>
    );
  }

  if (!dashboard) {
    return (
      <div className="flex h-full items-center justify-center text-dim">
        Loading...
      </div>
    );
  }

  const reviews = dashboard.reviewRequests.slice(0, MAX_REVIEWS);
  const prs = dashboard.myPullRequests.slice(0, MAX_PRS);
  const issues = dashboard.assignedIssues;
  const activities = dashboard.recentActivity.slice(0, MAX_ACTIVITIES);

  return (
    <div data-testid="overview" className="flex h-full gap-6 overflow-y-auto p-4">
      <div className="flex min-w-0 flex-1 flex-col gap-6">
        <ReviewQueue reviews={reviews} onOpen={openUrl} onWorkspaceAction={handleWorkspaceAction} />
        <MyPRs prs={prs} onOpen={openUrl} onWorkspaceAction={handleWorkspaceAction} />
      </div>

      <div className="flex w-[300px] min-w-0 flex-col gap-6">
        <Issues issues={issues} onOpen={openUrl} />
        <ActivityFeed activities={activities} onMarkAllRead={() => markAllRead.mutate()} />
      </div>
    </div>
  );
}
