import { type ReactElement, useState } from "react";
import type { Activity, ActivityType } from "../../lib/types";
import { EmptyState } from "../atoms/EmptyState";
import { SectionHead } from "../atoms/SectionHead";
import { ListItemSkeleton, Skeleton } from "../atoms/Skeleton";
import { ActivityItem } from "./ActivityItem";

interface ActivityFeedProps {
  readonly activities: readonly Activity[];
  readonly isLoading?: boolean;
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

const FILTER_LABEL: Record<FilterType, string> = {
  all: "all",
  comment: "comment",
  review: "review",
  ci: "CI",
  mention: "mention",
  other: "other",
};

const FILTER_BUTTON_CLASS =
  "inline-flex min-h-11 min-w-11 items-center justify-center rounded px-3 text-xs leading-none transition-colors";

const ACTION_BUTTON_CLASS =
  "inline-flex min-h-11 items-center rounded px-3 text-xs transition-colors";

function matchesFilter(activity: Activity, filter: FilterType): boolean {
  if (filter === "all") return true;
  if (filter === "mention") {
    return activity.activityType === "comment_added" && MENTION_PATTERN.test(activity.message);
  }
  return FILTER_MATCH[filter].includes(activity.activityType);
}

export function ActivityFeed({
  activities,
  isLoading = false,
  onMarkAllRead,
}: ActivityFeedProps): ReactElement {
  const [filter, setFilter] = useState<FilterType>("all");
  const [searchQuery, setSearchQuery] = useState("");
  const normalizedQuery = searchQuery.trim().toLowerCase();

  const visible = activities.filter((activity) => {
    if (!matchesFilter(activity, filter)) return false;
    if (normalizedQuery.length === 0) return true;

    return [activity.actor, activity.repoId, activity.message].some((value) =>
      value.toLowerCase().includes(normalizedQuery),
    );
  });

  return (
    <section
      data-testid="activity-feed"
      aria-busy={isLoading ? "true" : undefined}
      className="flex flex-col gap-2"
    >
      <SectionHead title="Activity" count={isLoading ? undefined : visible.length} />

      {isLoading ? (
        <>
          <div className="flex min-w-0 flex-wrap items-center gap-1">
            <Skeleton className="h-11 w-11" />
            <Skeleton className="h-11 w-16" />
            <Skeleton className="h-11 w-12" />
            <Skeleton className="ml-auto h-11 w-24" />
          </div>

          <div data-testid="activity-feed-loading" className="flex flex-col gap-1">
            {Array.from({ length: 4 }, (_, index) => (
              <ListItemSkeleton
                key={`activity-skeleton-${index}`}
                testId="activity-item-skeleton"
                showBodyLine
                showPill={false}
              />
            ))}
          </div>
        </>
      ) : (
        <>
          <input
            type="search"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Filter activity..."
            aria-label="Filter activity"
            className="w-full rounded-md border border-border bg-bg px-3 py-2 text-sm text-fg placeholder:text-muted"
          />

          <div className="flex min-w-0 flex-wrap items-center gap-1">
            <div className="flex flex-wrap gap-1" role="group" aria-label="Filter by type">
              {FILTER_LABELS.map((f) => (
                <button
                  key={f}
                  type="button"
                  aria-pressed={filter === f}
                  onClick={() => setFilter(f)}
                  className={`${FILTER_BUTTON_CLASS} ${
                    filter === f
                      ? "bg-accent text-bg font-semibold hover:bg-accent/80"
                      : "text-dim hover:bg-surface-hover hover:text-foreground"
                  }`}
                >
                  {FILTER_LABEL[f]}
                </button>
              ))}
            </div>

            <button
              type="button"
              onClick={onMarkAllRead}
              className={`ml-auto text-dim hover:text-foreground ${ACTION_BUTTON_CLASS}`}
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
        </>
      )}
    </section>
  );
}
