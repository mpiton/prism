import { useQuery } from "@tanstack/react-query";
import { memo, type ReactElement, useCallback, useEffect, useMemo, useRef } from "react";
import { FOCUS_RING } from "../../lib/a11y";
import { listRepos } from "../../lib/tauri";
import { FILTER_BUTTON_CLASS } from "../../lib/uiClasses";
import type { PullRequestWithReview } from "../../lib/types/dashboard";
import { useFilterableList } from "../../hooks/useFilterableList";
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

const PR_TABS: Readonly<Record<Tab, (pr: PullRequestWithReview) => boolean>> = {
  open: (pr) => {
    const { state } = pr.pullRequest;
    return state === "open" || state === "draft";
  },
  merged: (pr) => pr.pullRequest.state === "merged",
};

// Exported for testing only — the memoized `MyPRs` below is the public API.
export function MyPRsImpl({
  prs,
  isLoading = false,
  onOpen,
  onWorkspaceAction,
}: MyPRsProps): ReactElement {
  const listRef = useRef<HTMLDivElement>(null);
  const { data: repos } = useQuery({ queryKey: ["repos"], queryFn: listRepos });

  const repoMap = useMemo<Map<string, string>>(() => {
    if (!repos) return new Map();
    return new Map(repos.map((repo) => [repo.id, repo.fullName]));
  }, [repos]);

  const searchPredicate = useCallback(
    (pr: PullRequestWithReview, query: string): boolean => {
      const repoName = repoMap.get(pr.pullRequest.repoId) ?? pr.pullRequest.repoId;
      return [pr.pullRequest.title, pr.pullRequest.author, repoName, ...pr.pullRequest.labels].some(
        (value) => value.toLowerCase().includes(query),
      );
    },
    [repoMap],
  );

  const {
    tab,
    setTab,
    searchQuery,
    setSearchQuery,
    normalizedQuery,
    visibleItems: visible,
    tabCounts,
  } = useFilterableList<PullRequestWithReview, Tab>({
    items: prs,
    tabs: PR_TABS,
    defaultTab: "open",
    searchPredicate,
  });

  useEffect(() => {
    listRef.current?.scrollTo({ top: 0, behavior: "instant" });
  }, [tab, normalizedQuery]);

  const navItems = useMemo(() => visible.map((pr) => ({ url: pr.pullRequest.url })), [visible]);
  useRegisterNavigableItems(navItems);

  return (
    <section
      data-testid="my-prs"
      aria-busy={isLoading ? "true" : undefined}
      className="flex flex-col gap-2"
    >
      {/*
        Count sums the two tab buckets rather than using `filteredItems.length`
        because `PrState` includes "closed" — a state that is intentionally not
        surfaced in any tab and should not appear in the header total.
      */}
      <SectionHead
        title="My PRs"
        count={isLoading ? undefined : tabCounts.open + tabCounts.merged}
      />

      {isLoading ? (
        <>
          <div className="flex gap-1">
            <Skeleton className="h-11 w-16" />
            <Skeleton className="h-11 w-20" />
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
          <input
            type="search"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Filter PRs..."
            aria-label="Filter PRs"
            className={`${FOCUS_RING} w-full rounded-md border border-border bg-bg px-3 py-2 text-sm text-fg placeholder:text-muted`}
          />

          <div className="flex gap-1" role="group" aria-label="Filter by state">
            <button
              type="button"
              aria-pressed={tab === "open"}
              onClick={() => setTab("open")}
              className={`${FILTER_BUTTON_CLASS} ${
                tab === "open"
                  ? "bg-accent text-bg font-semibold hover:bg-accent/80"
                  : "text-dim hover:bg-surface-hover hover:text-foreground"
              }`}
            >
              Open {tabCounts.open}
            </button>
            <button
              type="button"
              aria-pressed={tab === "merged"}
              onClick={() => setTab("merged")}
              className={`${FILTER_BUTTON_CLASS} ${
                tab === "merged"
                  ? "bg-accent text-bg font-semibold hover:bg-accent/80"
                  : "text-dim hover:bg-surface-hover hover:text-foreground"
              }`}
            >
              Merged {tabCounts.merged}
            </button>
          </div>

          {visible.length === 0 ? (
            <EmptyState icon="↗" message="No pull requests to display" />
          ) : (
            <div ref={listRef} className="max-h-[600px] overflow-y-auto">
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
            </div>
          )}
        </>
      )}
    </section>
  );
}

export const MyPRs = memo(MyPRsImpl);
