import { type ReactElement, useEffect, useMemo } from "react";
import type { Priority, PullRequestWithReview } from "../../lib/types";
import { useDashboardStore } from "../../stores/dashboard";
import { EmptyState } from "../atoms/EmptyState";
import { SectionHead } from "../atoms/SectionHead";
import { ReviewCard } from "./ReviewCard";

interface ReviewQueueProps {
  readonly reviews: readonly PullRequestWithReview[];
  readonly onOpen: (url: string) => void;
  readonly onWorkspaceAction?: (workspaceId: string) => void;
}

const PRIORITY_ORDER: Record<Priority, number> = {
  critical: 4,
  high: 3,
  medium: 2,
  low: 1,
};

type PriorityFilter = "all" | Priority;

const PRIORITY_FILTERS: readonly PriorityFilter[] = [
  "all",
  "critical",
  "high",
  "medium",
  "low",
];

function sortByPriority(
  reviews: readonly PullRequestWithReview[],
): readonly PullRequestWithReview[] {
  return [...reviews].sort(
    (a, b) =>
      PRIORITY_ORDER[b.pullRequest.priority] -
        PRIORITY_ORDER[a.pullRequest.priority] ||
      b.pullRequest.updatedAt.localeCompare(a.pullRequest.updatedAt),
  );
}

function getUniqueRepos(
  reviews: readonly PullRequestWithReview[],
): readonly string[] {
  return [...new Set(reviews.map((r) => r.pullRequest.repoId))].sort();
}

export function ReviewQueue({
  reviews,
  onOpen,
  onWorkspaceAction,
}: ReviewQueueProps): ReactElement {
  const activeFilters = useDashboardStore((s) => s.activeFilters);
  const setFilter = useDashboardStore((s) => s.setFilter);

  useEffect(() => {
    return () => setFilter({ priority: undefined, repo: undefined });
  }, [setFilter]);

  const priorityFilter: PriorityFilter = activeFilters.priority ?? "all";
  const repoFilter = activeFilters.repo ?? "";

  const repos = useMemo(() => getUniqueRepos(reviews), [reviews]);

  useEffect(() => {
    if (repoFilter && !repos.includes(repoFilter)) {
      setFilter({ repo: undefined });
    }
  }, [repos, repoFilter, setFilter]);

  const filtered = reviews.filter((r) => {
    if (priorityFilter !== "all" && r.pullRequest.priority !== priorityFilter) {
      return false;
    }
    if (repoFilter && r.pullRequest.repoId !== repoFilter) {
      return false;
    }
    return true;
  });

  const sorted = sortByPriority(filtered);

  return (
    <section data-testid="review-queue" className="flex flex-col gap-2">
      <SectionHead title="Reviews" count={sorted.length} />

      <div className="flex items-center gap-3">
        <div className="flex gap-1" role="group" aria-label="Filter by priority">
          {PRIORITY_FILTERS.map((f) => (
            <button
              key={f}
              type="button"
              aria-pressed={priorityFilter === f}
              onClick={() =>
                setFilter({ priority: f === "all" ? undefined : f })
              }
              className={`rounded px-2 py-0.5 text-xs ${
                priorityFilter === f
                  ? "bg-accent text-white"
                  : "text-dim hover:text-foreground"
              }`}
            >
              {f}
            </button>
          ))}
        </div>

        {repos.length > 1 && (
          <select
            aria-label="Filter by repo"
            value={repoFilter}
            onChange={(e) =>
              setFilter({ repo: e.target.value || undefined })
            }
            className="rounded border border-border bg-surface px-2 py-0.5 text-xs text-foreground"
          >
            <option value="">All repos</option>
            {repos.map((id) => (
              <option key={id} value={id}>
                {id}
              </option>
            ))}
          </select>
        )}
      </div>

      {sorted.length === 0 ? (
        <EmptyState message="No reviews to display" />
      ) : (
        <div className="flex flex-col gap-1">
          {sorted.map((review) => (
            <ReviewCard
              key={review.pullRequest.id}
              data={review}
              onOpen={onOpen}
              onWorkspaceAction={onWorkspaceAction}
            />
          ))}
        </div>
      )}
    </section>
  );
}
