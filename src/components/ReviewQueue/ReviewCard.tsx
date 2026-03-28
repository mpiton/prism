import type { ReactElement } from "react";
import { timeAgo } from "../../lib/timeAgo";
import type { PullRequestWithReview } from "../../lib/types";
import { CI } from "../atoms/CI";
import { Diff } from "../atoms/Diff";
import { PriorityBar } from "../atoms/PriorityBar";
import { WsBadge } from "../atoms/WsBadge";

interface ReviewCardProps {
  readonly data: PullRequestWithReview;
  readonly onOpen: (url: string) => void;
  readonly onWorkspaceAction?: (workspaceId: string) => void;
}

export function ReviewCard({
  data,
  onOpen,
  onWorkspaceAction,
}: ReviewCardProps): ReactElement {
  const { pullRequest: pr, workspace } = data;

  function handleClick(e: React.MouseEvent) {
    e.preventDefault();
    onOpen(pr.url);
  }

  return (
    <div className="flex items-center gap-3 rounded border border-border px-3 py-2 hover:bg-surface-hover">
      <a
        href={pr.url}
        onClick={handleClick}
        className="flex min-w-0 flex-1 cursor-pointer items-center gap-3 no-underline"
      >
        <PriorityBar priority={pr.priority} />

        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <div className="flex items-center gap-2">
            <span className="truncate text-sm font-medium text-foreground">
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

      {workspace && (
        <WsBadge
          state={workspace.state}
          onClick={
            onWorkspaceAction
              ? () => onWorkspaceAction(workspace.id)
              : undefined
          }
          ariaLabel={`${workspace.state === "active" ? "Resume" : workspace.state === "suspended" ? "Wake" : "Open"} workspace for PR #${pr.number}`}
        />
      )}
    </div>
  );
}
