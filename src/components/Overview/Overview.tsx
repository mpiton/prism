import { type ReactElement, useCallback, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useGitHubData } from "../../hooks/useGitHubData";
import { FOCUS_RING } from "../../lib/a11y";
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
const MAX_ISSUES = 5;
const MAX_ACTIVITIES = 5;

export function Overview(): ReactElement {
  const { dashboard, error, isLoading } = useGitHubData();
  const queryClient = useQueryClient();
  const [isActivityExpanded, setIsActivityExpanded] = useState(true);

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

  const showLoadingState = !dashboard && !error;

  const reviewRequests = dashboard?.reviewRequests ?? [];
  const myPullRequests = dashboard?.myPullRequests ?? [];
  const assignedIssues = dashboard?.assignedIssues ?? [];
  const recentActivity = dashboard?.recentActivity ?? [];

  const reviews = reviewRequests.slice(0, MAX_REVIEWS);
  const prs = myPullRequests.slice(0, MAX_PRS);
  const openIssues = assignedIssues.filter((i) => i.state === "open");
  const issues = openIssues.slice(0, MAX_ISSUES);
  const openIssueCount = openIssues.length;
  const activities = recentActivity.slice(0, MAX_ACTIVITIES);
  const openPrCount = myPullRequests.filter(
    (pr) => pr.pullRequest.state === "open" || pr.pullRequest.state === "draft",
  ).length;
  const unreadActivityCount = recentActivity.filter((activity) => !activity.isRead).length;

  return (
    <div
      data-testid="overview"
      aria-busy={showLoadingState || isLoading ? "true" : undefined}
      className="grid h-full min-w-0 gap-6 overflow-y-auto p-4 xl:grid-cols-[minmax(0,1fr)_18rem]"
    >
      <div className="flex min-w-0 flex-col gap-6">
        <div
          data-testid="overview-reviews-panel"
          className="rounded-2xl border border-accent/30 bg-surface px-4 py-4 shadow-[0_0_0_1px_rgba(255,255,255,0.03)]"
        >
          <div className="mb-4 flex flex-wrap items-start justify-between gap-3">
            <div className="space-y-1">
              <p className="text-[11px] font-semibold uppercase tracking-[0.24em] text-accent">
                Priority lane
              </p>
              <h1 className="text-lg font-semibold text-white">Review requests come first</h1>
              <p className="max-w-2xl text-sm text-dim">
                Surface the PRs that need your attention now so approvals and requested changes do
                not get buried under passive updates.
              </p>
            </div>

            <div className="flex flex-wrap items-center gap-2">
              <span className="rounded-full border border-accent/40 bg-accent/10 px-3 py-1 text-xs font-medium text-accent">
                {showLoadingState ? "Loading" : `${reviewRequests.length} in queue`}
              </span>
              {!showLoadingState && reviewRequests.length > 0 ? (
                <span className="rounded-full border border-border px-3 py-1 text-xs text-dim">
                  Action required
                </span>
              ) : null}
            </div>
          </div>

          <ReviewQueue
            reviews={reviews}
            isLoading={showLoadingState}
            onOpen={openUrl}
            onWorkspaceAction={handleWorkspaceAction}
          />
        </div>

        <div data-testid="overview-secondary-grid" className="grid min-w-0 gap-6 lg:grid-cols-2">
          <div className="rounded-2xl border border-border bg-surface/70 px-4 py-4">
            <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
              <div className="space-y-1">
                <p className="text-[11px] font-semibold uppercase tracking-[0.24em] text-dim">
                  Work in flight
                </p>
                <p className="text-sm text-foreground">
                  Track the pull requests you still own after reviews move forward.
                </p>
              </div>
              <span className="rounded-full border border-border px-3 py-1 text-xs text-dim">
                {showLoadingState ? "Loading" : `${openPrCount} open`}
              </span>
            </div>

            <MyPRs
              prs={prs}
              isLoading={showLoadingState}
              onOpen={openUrl}
              onWorkspaceAction={handleWorkspaceAction}
            />
          </div>

          <div className="rounded-2xl border border-border bg-surface/70 px-4 py-4">
            <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
              <div className="space-y-1">
                <p className="text-[11px] font-semibold uppercase tracking-[0.24em] text-dim">
                  Assigned work
                </p>
                <p className="text-sm text-foreground">
                  Keep issue ownership visible without competing with active review work.
                </p>
              </div>
              <span className="rounded-full border border-border px-3 py-1 text-xs text-dim">
                {showLoadingState ? "Loading" : `${openIssueCount} open`}
              </span>
            </div>

            <Issues issues={issues} isLoading={showLoadingState} onOpen={openUrl} hideTabs />

            {!showLoadingState && openIssueCount > MAX_ISSUES ? (
              <button
                type="button"
                data-testid="overview-issues-view-all"
                onClick={() => useDashboardStore.getState().setView("issues")}
                className={`${FOCUS_RING} mt-3 w-full rounded-lg border border-border px-3 py-2 text-center text-xs text-dim transition-colors hover:border-foreground hover:text-foreground`}
              >
                View all {openIssueCount} issues
              </button>
            ) : null}
          </div>
        </div>
      </div>

      <aside
        data-testid="overview-activity-shell"
        className="flex min-w-0 flex-col self-start rounded-2xl border border-border bg-surface/60 px-4 py-4 xl:sticky xl:top-4"
      >
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div className="space-y-1">
            <p className="text-[11px] font-semibold uppercase tracking-[0.24em] text-dim">
              Signals
            </p>
            <h2 className="text-sm font-semibold text-white">Activity digest</h2>
            <p className="text-sm text-dim">
              Keep recent updates nearby without letting them dominate the page.
            </p>
          </div>

          <div className="flex items-center gap-2">
            <span className="rounded-full border border-border px-3 py-1 text-xs text-dim">
              {showLoadingState ? "Loading" : `${unreadActivityCount} unread`}
            </span>
            <button
              type="button"
              data-testid="overview-activity-toggle"
              aria-expanded={isActivityExpanded}
              aria-controls={isActivityExpanded ? "overview-activity-content" : undefined}
              onClick={() => setIsActivityExpanded((current) => !current)}
              className={`${FOCUS_RING} rounded-full border border-border px-3 py-1 text-xs text-dim transition-colors hover:border-foreground hover:text-foreground`}
            >
              {isActivityExpanded ? "Collapse" : "Expand"}
            </button>
          </div>
        </div>

        {isActivityExpanded ? (
          <div id="overview-activity-content" className="mt-4">
            <ActivityFeed
              activities={activities}
              isLoading={showLoadingState}
              onMarkAllRead={() => markAllRead.mutate()}
            />
          </div>
        ) : null}
      </aside>
    </div>
  );
}
