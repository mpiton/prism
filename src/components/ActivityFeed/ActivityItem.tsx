import type { ReactElement } from "react";
import { timeAgo } from "../../lib/timeAgo";
import type { Activity, ActivityType } from "../../lib/types";

interface ActivityItemProps {
  readonly activity: Activity;
}

const ACTIVITY_ICON = {
  pr_opened: "↗",
  pr_merged: "⇌",
  pr_closed: "✕",
  review_submitted: "✓",
  comment_added: "💬",
  ci_completed: "⚙",
  issue_opened: "◉",
  issue_closed: "◎",
} satisfies Record<ActivityType, string>;

const ACTIVITY_ACTION = {
  pr_opened: "opened a PR",
  pr_merged: "merged a PR",
  pr_closed: "closed a PR",
  review_submitted: "submitted a review",
  comment_added: "commented",
  ci_completed: "CI completed",
  issue_opened: "opened an issue",
  issue_closed: "closed an issue",
} satisfies Record<ActivityType, string>;

const MAX_BODY_LENGTH = 80;

function truncate(text: string, max: number): string {
  if (text.length <= max) return text;
  if (max <= 1) return "…";
  return `${text.slice(0, max - 1)}…`;
}

export function ActivityItem({ activity }: ActivityItemProps): ReactElement {
  return (
    <div data-testid="activity-item" className="flex items-start gap-2 rounded border border-border px-3 py-2">
      <span data-testid="activity-icon" aria-hidden="true" className="shrink-0 text-sm text-dim">
        {ACTIVITY_ICON[activity.activityType]}
      </span>

      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-1 text-sm">
          <span data-testid="activity-actor" className="font-bold text-foreground">
            {activity.actor}
          </span>
          <span data-testid="activity-action" className="text-dim">
            {ACTIVITY_ACTION[activity.activityType]}
          </span>
        </div>

        {activity.message.length > 0 && (
          <p data-testid="activity-body" className="mt-0.5 text-xs text-dim">
            {truncate(activity.message, MAX_BODY_LENGTH)}
          </p>
        )}

        <div className="mt-0.5 flex items-center gap-2 text-xs text-dim">
          <span>{activity.repoId}</span>
          <span data-testid="activity-time">{timeAgo(activity.createdAt)}</span>
        </div>
      </div>
    </div>
  );
}
