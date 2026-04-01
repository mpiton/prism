import { type ReactElement, useState } from "react";
import type { Activity, ActivityType } from "../../lib/types";
import { EmptyState } from "../atoms/EmptyState";
import { SectionHead } from "../atoms/SectionHead";
import { ActivityItem } from "./ActivityItem";

interface ActivityFeedProps {
  readonly activities: readonly Activity[];
  readonly onMarkAllRead: () => void;
}

type FilterType = "all" | "comment" | "review" | "ci" | "mention" | "other";

const FILTER_MATCH: Record<Exclude<FilterType, "all" | "mention">, readonly ActivityType[]> = {
  comment: ["comment_added"],
  review: ["review_submitted"],
  ci: ["ci_completed"],
  other: ["pr_opened", "pr_merged", "pr_closed", "issue_opened", "issue_closed"],
};

const MENTION_PATTERN = /(^|\s)@\w+/;

const FILTER_LABELS = ["all", "comment", "review", "ci", "mention", "other"] as const satisfies readonly FilterType[];

function matchesFilter(activity: Activity, filter: FilterType): boolean {
  if (filter === "all") return true;
  if (filter === "mention") {
    return activity.activityType === "comment_added" && MENTION_PATTERN.test(activity.message);
  }
  return FILTER_MATCH[filter].includes(activity.activityType);
}

export function ActivityFeed({ activities, onMarkAllRead }: ActivityFeedProps): ReactElement {
  const [filter, setFilter] = useState<FilterType>("all");

  const visible = activities.filter((a) => matchesFilter(a, filter));

  return (
    <section data-testid="activity-feed" className="flex flex-col gap-2">
      <SectionHead title="Activity" count={visible.length} />

      <div className="flex min-w-0 flex-wrap items-center gap-1">
        <div className="flex flex-wrap gap-1" role="group" aria-label="Filter by type">
          {FILTER_LABELS.map((f) => (
            <button
              key={f}
              type="button"
              aria-pressed={filter === f}
              onClick={() => setFilter(f)}
              className={`rounded px-2 py-0.5 text-xs capitalize transition-colors ${
                filter === f
                  ? "bg-accent text-white"
                  : "text-dim hover:text-foreground"
              }`}
            >
              {f}
            </button>
          ))}
        </div>

        <button
          type="button"
          onClick={onMarkAllRead}
          className="ml-auto rounded px-2 py-0.5 text-xs text-dim hover:text-foreground"
        >
          Mark all read
        </button>
      </div>

      {visible.length === 0 ? (
        <EmptyState icon="◌" message="No activity to display" />
      ) : (
        <div className="flex flex-col gap-1">
          {visible.map((activity) => (
            <ActivityItem key={activity.id} activity={activity} />
          ))}
        </div>
      )}
    </section>
  );
}
