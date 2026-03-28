import { type ReactElement, useState } from "react";
import type { Priority, PullRequestWithReview } from "../../lib/types";
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

const FILTERS: readonly PriorityFilter[] = [
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

export function ReviewQueue({
  reviews,
  onOpen,
  onWorkspaceAction,
}: ReviewQueueProps): ReactElement {
  const [filter, setFilter] = useState<PriorityFilter>("all");

  const filtered =
    filter === "all"
      ? reviews
      : reviews.filter((r) => r.pullRequest.priority === filter);

  const sorted = sortByPriority(filtered);

  return (
    <section data-testid="review-queue" className="flex flex-col gap-2">
      <SectionHead title="Reviews" count={sorted.length} />

      <div className="flex gap-1" role="group" aria-label="Filter by priority">
        {FILTERS.map((f) => (
          <button
            key={f}
            type="button"
            aria-pressed={filter === f}
            onClick={() => setFilter(f)}
            className={`rounded px-2 py-0.5 text-xs ${
              filter === f
                ? "bg-accent text-white"
                : "text-dim hover:text-foreground"
            }`}
          >
            {f}
          </button>
        ))}
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
