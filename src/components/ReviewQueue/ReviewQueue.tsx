import { memo, type ReactElement, useEffect, useMemo } from "react";
import { FOCUS_RING } from "../../lib/a11y";
import { FILTER_BUTTON_CLASS } from "../../lib/uiClasses";
import type { Priority } from "../../lib/types/enums";
import type { PullRequestWithReview } from "../../lib/types/dashboard";
import { useRegisterNavigableItems } from "../../hooks/useRegisterNavigableItems";
import { useDashboardStore } from "../../stores/dashboard";
import { CardSkeleton, Skeleton } from "../atoms/Skeleton";
import { EmptyState } from "../atoms/EmptyState";
import { SectionHead } from "../atoms/SectionHead";
import { ReviewCard } from "./ReviewCard";

interface WorkspaceActionParams {
  readonly repoId: string;
  readonly pullRequestNumber: number;
  readonly headRefName: string;
  readonly workspaceId?: string;
  readonly workspaceState?: string;
}

interface ReviewQueueProps {
  readonly reviews: readonly PullRequestWithReview[];
  readonly isLoading?: boolean;
  readonly onOpen: (url: string) => void;
  readonly onWorkspaceAction?: (params: WorkspaceActionParams) => void;
}

const PRIORITY_ORDER: Record<Priority, number> = {
  critical: 4,
  high: 3,
  medium: 2,
  low: 1,
};

type PriorityFilter = "all" | Priority;

const FOCUS_PRIORITIES: readonly Priority[] = ["critical", "high"];

const PRIORITY_FILTERS: readonly PriorityFilter[] = ["all", "critical", "high", "medium", "low"];

const INLINE_CONTROL_CLASS = `${FOCUS_RING} min-h-11 rounded px-3 text-xs transition-colors`;

function sortByPriority(
  reviews: readonly PullRequestWithReview[],
): readonly PullRequestWithReview[] {
  return [...reviews].sort(
    (a, b) =>
      PRIORITY_ORDER[b.pullRequest.priority] - PRIORITY_ORDER[a.pullRequest.priority] ||
      b.pullRequest.updatedAt.localeCompare(a.pullRequest.updatedAt),
  );
}

function getUniqueRepos(reviews: readonly PullRequestWithReview[]): readonly string[] {
  return [...new Set(reviews.map((r) => r.pullRequest.repoId))].sort();
}

// Exported for testing only — the memoized `ReviewQueue` below is the public API.
export function ReviewQueueImpl({
  reviews,
  isLoading = false,
  onOpen,
  onWorkspaceAction,
}: ReviewQueueProps): ReactElement {
  const storePriority = useDashboardStore((s) => s.activeFilters.priority);
  const storeRepo = useDashboardStore((s) => s.activeFilters.repo);
  const setFilter = useDashboardStore((s) => s.setFilter);
  const focusMode = useDashboardStore((s) => s.focusMode);

  useEffect(() => {
    return () => setFilter({ priority: undefined, repo: undefined });
  }, [setFilter]);

  const priorityFilter: PriorityFilter = storePriority ?? "all";
  const repos = useMemo(() => getUniqueRepos(reviews), [reviews]);

  // Derive effective repo filter synchronously — avoids flash when stale repo disappears
  const repoFilter = storeRepo && repos.includes(storeRepo) ? storeRepo : "";

  // Sync store when the derived value diverges (stale repo removed from reviews)
  useEffect(() => {
    if (storeRepo && !repos.includes(storeRepo)) {
      setFilter({ repo: undefined });
    }
  }, [repos, storeRepo, setFilter]);

  const sorted = useMemo(() => {
    const filtered = reviews.filter((r) => {
      if (focusMode && !FOCUS_PRIORITIES.includes(r.pullRequest.priority)) return false;
      if (priorityFilter !== "all" && r.pullRequest.priority !== priorityFilter) return false;
      if (repoFilter && r.pullRequest.repoId !== repoFilter) return false;
      return true;
    });
    return sortByPriority(filtered);
  }, [reviews, focusMode, priorityFilter, repoFilter]);

  const navItems = useMemo(
    () => (isLoading ? [] : sorted.map((r) => ({ url: r.pullRequest.url }))),
    [isLoading, sorted],
  );
  useRegisterNavigableItems(navItems);

  return (
    <section
      data-testid="review-queue"
      aria-busy={isLoading ? "true" : undefined}
      className="flex flex-col gap-2"
    >
      <SectionHead title="Reviews" count={isLoading ? undefined : sorted.length} />

      {isLoading ? (
        <>
          <div className="flex items-center gap-3">
            {PRIORITY_FILTERS.map((filter) => (
              <Skeleton
                key={`priority-filter-skeleton-${filter}`}
                className={`h-11 ${
                  filter === "all"
                    ? "w-11"
                    : filter === "critical"
                      ? "w-16"
                      : filter === "high"
                        ? "w-12"
                        : filter === "medium"
                          ? "w-14"
                          : "w-10"
                }`}
              />
            ))}
          </div>

          <div data-testid="review-queue-loading" className="flex flex-col gap-1">
            {Array.from({ length: 3 }, (_, index) => (
              <CardSkeleton
                key={`review-skeleton-${index}`}
                testId="review-card-skeleton"
                showPriorityBar
                showTrailingBadge
              />
            ))}
          </div>
        </>
      ) : (
        <>
          <div className="flex items-center gap-3">
            <div className="flex gap-1" role="group" aria-label="Filter by priority">
              {PRIORITY_FILTERS.map((f) => (
                <button
                  key={f}
                  type="button"
                  aria-pressed={priorityFilter === f}
                  onClick={() => setFilter({ priority: f === "all" ? undefined : f })}
                  className={`${FILTER_BUTTON_CLASS} ${
                    priorityFilter === f
                      ? "bg-accent text-bg font-semibold hover:bg-accent/80"
                      : "text-dim hover:bg-surface-hover hover:text-foreground"
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
                onChange={(e) => setFilter({ repo: e.target.value || undefined })}
                className={`cursor-pointer border border-border bg-surface text-foreground hover:border-foreground ${INLINE_CONTROL_CLASS}`}
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
            <EmptyState icon="✓" message="No pending reviews — you're all caught up!" />
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
        </>
      )}
    </section>
  );
}

export const ReviewQueue = memo(ReviewQueueImpl);
