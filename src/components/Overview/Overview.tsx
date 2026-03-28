import type { ReactElement } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useGitHubData } from "../../hooks/useGitHubData";
import { markAllActivityRead } from "../../lib/tauri";
import { ReviewQueue } from "../ReviewQueue";
import { MyPRs } from "../MyPRs";
import { Issues } from "../Issues";
import { ActivityFeed } from "../ActivityFeed";

const MAX_REVIEWS = 5;
const MAX_PRS = 5;
const MAX_ISSUES = 3;
const MAX_ACTIVITIES = 5;

function openUrl(url: string): void {
  window.open(url, "_blank", "noopener,noreferrer");
}

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
  const issues = dashboard.assignedIssues.slice(0, MAX_ISSUES);
  const activities = dashboard.recentActivity.slice(0, MAX_ACTIVITIES);

  return (
    <div data-testid="overview" className="flex h-full gap-6 overflow-y-auto p-4">
      <div className="flex min-w-0 flex-1 flex-col gap-6">
        <ReviewQueue reviews={reviews} onOpen={openUrl} />
        <MyPRs prs={prs} onOpen={openUrl} />
      </div>

      <div className="flex w-[340px] shrink-0 flex-col gap-6">
        <Issues issues={issues} onOpen={openUrl} />
        <ActivityFeed activities={activities} onMarkAllRead={() => markAllRead.mutate()} />
      </div>
    </div>
  );
}
