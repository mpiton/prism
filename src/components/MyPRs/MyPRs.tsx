import { type ReactElement, useMemo, useState } from "react";
import type { PullRequestWithReview } from "../../lib/types";
import { useRegisterNavigableItems } from "../../hooks/useRegisterNavigableItems";
import { EmptyState } from "../atoms/EmptyState";
import { SectionHead } from "../atoms/SectionHead";
import { CardSkeleton, Skeleton } from "../atoms/Skeleton";
import { MyPrCard } from "./MyPrCard";

interface WorkspaceActionParams {
  readonly repoId: string;
  readonly pullRequestNumber: number;
  readonly headRefName: string;
  readonly workspaceId?: string;
  readonly workspaceState?: string;
}

interface MyPRsProps {
  readonly prs: readonly PullRequestWithReview[];
  readonly isLoading?: boolean;
  readonly onOpen: (url: string) => void;
  readonly onWorkspaceAction?: (params: WorkspaceActionParams) => void;
}

type Tab = "open" | "merged";

function isOpen(pr: PullRequestWithReview): boolean {
  const { state } = pr.pullRequest;
  return state === "open" || state === "draft";
}

function isMerged(pr: PullRequestWithReview): boolean {
  return pr.pullRequest.state === "merged";
}

export function MyPRs({
  prs,
  isLoading = false,
  onOpen,
  onWorkspaceAction,
}: MyPRsProps): ReactElement {
  const [tab, setTab] = useState<Tab>("open");

  const openPrs = prs.filter(isOpen);
  const mergedPrs = prs.filter(isMerged);
  const visible = tab === "open" ? openPrs : mergedPrs;

  const navItems = useMemo(
    () =>
      prs
        .filter(tab === "open" ? isOpen : isMerged)
        .map((pr) => ({ url: pr.pullRequest.url })),
    [prs, tab],
  );
  useRegisterNavigableItems(navItems);

  return (
    <section
      data-testid="my-prs"
      aria-busy={isLoading ? "true" : undefined}
      className="flex flex-col gap-2"
    >
      <SectionHead
        title="My PRs"
        count={isLoading ? undefined : openPrs.length + mergedPrs.length}
      />

      {isLoading ? (
        <>
          <div className="flex gap-1">
            <Skeleton className="h-8 w-16" />
            <Skeleton className="h-8 w-20" />
          </div>

          <div data-testid="my-prs-loading" className="flex flex-col gap-1">
            {Array.from({ length: 3 }, (_, index) => (
              <CardSkeleton
                key={`my-pr-skeleton-${index}`}
                testId="my-pr-card-skeleton"
                showTrailingBadge
              />
            ))}
          </div>
        </>
      ) : (
        <>
          <div className="flex gap-1" role="group" aria-label="Filter by state">
            <button
              type="button"
              aria-pressed={tab === "open"}
              onClick={() => setTab("open")}
              className={`rounded px-2 py-2 text-xs transition-colors ${
                tab === "open"
                  ? "bg-accent text-bg font-semibold hover:bg-accent/80"
                  : "text-dim hover:bg-surface-hover hover:text-foreground"
              }`}
            >
              Open {openPrs.length}
            </button>
            <button
              type="button"
              aria-pressed={tab === "merged"}
              onClick={() => setTab("merged")}
              className={`rounded px-2 py-2 text-xs transition-colors ${
                tab === "merged"
                  ? "bg-accent text-bg font-semibold hover:bg-accent/80"
                  : "text-dim hover:bg-surface-hover hover:text-foreground"
              }`}
            >
              Merged {mergedPrs.length}
            </button>
          </div>

          {visible.length === 0 ? (
            <EmptyState icon="↗" message="No pull requests to display" />
          ) : (
            <div className="flex flex-col gap-1">
              {visible.map((pr) => (
                <MyPrCard
                  key={pr.pullRequest.id}
                  data={pr}
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
