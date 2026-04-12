import { type ReactElement, useState } from "react";
import { FOCUS_RING } from "../../lib/a11y";
import { timeAgo } from "../../lib/timeAgo";
import { SELECTED_ITEM_CLASS } from "../../lib/uiClasses";
import type { PullRequestWithReview } from "../../lib/types/dashboard";
import { CI } from "../atoms/CI";
import { Diff } from "../atoms/Diff";
import { PriorityBar } from "../atoms/PriorityBar";
import { WsBadge } from "../atoms/WsBadge";

interface ReviewCardProps {
  readonly data: PullRequestWithReview;
  readonly onOpen: (url: string) => void;
  readonly isSelected?: boolean;
  readonly onWorkspaceAction?: (params: {
    readonly repoId: string;
    readonly pullRequestNumber: number;
    readonly headRefName: string;
    readonly workspaceId?: string;
    readonly workspaceState?: string;
  }) => void;
}

export function ReviewCard({
  data,
  onOpen,
  isSelected = false,
  onWorkspaceAction,
}: ReviewCardProps): ReactElement {
  const { pullRequest: pr, workspace } = data;
  const [loading, setLoading] = useState(false);

  function handleClick(e: React.MouseEvent) {
    e.preventDefault();
    onOpen(pr.url);
  }

  function handleWorkspace() {
    if (!onWorkspaceAction || loading) return;
    setLoading(true);
    Promise.resolve(
      onWorkspaceAction({
        repoId: pr.repoId,
        pullRequestNumber: pr.number,
        headRefName: pr.headRefName,
        workspaceId: workspace?.id,
        workspaceState: workspace?.state,
      }),
    ).finally(() => setLoading(false));
  }

  return (
    <div
      data-selected={isSelected ? "true" : undefined}
      aria-current={isSelected ? "true" : undefined}
      className={`flex items-center gap-3 rounded border border-border px-3 py-2 hover:bg-surface-hover${
        isSelected ? ` ${SELECTED_ITEM_CLASS}` : ""
      }`}
    >
      <a
        href={pr.url}
        onClick={handleClick}
        aria-label={`PR #${pr.number}: ${pr.title} by ${pr.author}`}
        className={`${FOCUS_RING} flex min-w-0 flex-1 cursor-pointer items-center gap-3 rounded no-underline`}
      >
        <PriorityBar priority={pr.priority} />

        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <div className="flex min-w-0 items-center gap-2">
            <span className="min-w-0 truncate text-sm font-medium text-foreground" title={pr.title}>
              {pr.title}
            </span>
            <span className="shrink-0 text-xs text-dim">#{pr.number}</span>
          </div>

          <div className="flex items-center gap-3 text-xs text-dim">
            <span>{pr.author}</span>
            {pr.additions !== undefined && pr.deletions !== undefined && (
              <Diff additions={pr.additions} deletions={pr.deletions} />
            )}
            <CI status={pr.ciStatus} />
            <span>{timeAgo(pr.updatedAt)}</span>
          </div>
        </div>
      </a>

      {onWorkspaceAction &&
        pr.state === "open" &&
        ((workspace && workspace.state !== "archived") || pr.headRefName) && (
          <WsBadge
            state={workspace?.state === "archived" ? undefined : workspace?.state}
            loading={loading}
            onClick={handleWorkspace}
            ariaLabel={`${workspace?.state === "active" ? "Resume" : workspace?.state === "suspended" ? "Wake" : "Open"} workspace for PR #${pr.number}`}
          />
        )}
    </div>
  );
}
