import { useVirtualizer } from "@tanstack/react-virtual";
import { useQuery } from "@tanstack/react-query";
import { memo, type ReactElement, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { FOCUS_RING } from "../../lib/a11y";
import { listRepos } from "../../lib/tauri";
import { FILTER_BUTTON_CLASS, INLINE_CONTROL_CLASS } from "../../lib/uiClasses";
import type { Issue } from "../../lib/types/github";
import { useFilterableList } from "../../hooks/useFilterableList";
import { useRegisterNavigableItems } from "../../hooks/useRegisterNavigableItems";
import { EmptyState } from "../atoms/EmptyState";
import { SectionHead } from "../atoms/SectionHead";
import { ListItemSkeleton, Skeleton } from "../atoms/Skeleton";
import { IssueCard } from "./IssueCard";

interface IssuesProps {
  readonly issues: readonly Issue[];
  readonly isLoading?: boolean;
  readonly onOpen: (url: string) => void;
}

type Tab = "open" | "closed";

const ISSUE_TABS: Readonly<Record<Tab, (issue: Issue) => boolean>> = {
  open: (issue) => issue.state === "open",
  closed: (issue) => issue.state === "closed",
};

function IssuesImpl({ issues, isLoading = false, onOpen }: IssuesProps): ReactElement {
  const parentRef = useRef<HTMLDivElement>(null);
  const { data: repos } = useQuery({ queryKey: ["repos"], queryFn: listRepos });
  const [repoFilter, setRepoFilter] = useState("");
  const [labelFilter, setLabelFilter] = useState<string | null>(null);

  const repoMap = useMemo<Map<string, string>>(() => {
    if (!repos) return new Map();
    return new Map(repos.map((repo) => [repo.id, repo.fullName]));
  }, [repos]);

  const uniqueRepos = useMemo<{ id: string; fullName: string }[]>(() => {
    const seen = new Set<string>();
    const result: { id: string; fullName: string }[] = [];
    for (const issue of issues) {
      if (!seen.has(issue.repoId)) {
        seen.add(issue.repoId);
        result.push({ id: issue.repoId, fullName: repoMap.get(issue.repoId) ?? issue.repoId });
      }
    }
    return result.sort((a, b) => a.fullName.localeCompare(b.fullName));
  }, [issues, repoMap]);

  const isRepoFilterValid =
    repoFilter === "" || uniqueRepos.some((r) => r.id === repoFilter);

  const repoFiltered = useMemo<readonly Issue[]>(
    () =>
      repoFilter === "" || !isRepoFilterValid
        ? issues
        : issues.filter((issue) => issue.repoId === repoFilter),
    [issues, repoFilter, isRepoFilterValid],
  );

  const uniqueLabels = useMemo<string[]>(() => {
    const seen = new Set<string>();
    for (const issue of repoFiltered) {
      for (const label of issue.labels) {
        seen.add(label);
      }
    }
    return [...seen].sort();
  }, [repoFiltered]);

  // Reset stale filters when available options shrink
  useEffect(() => {
    if (repoFilter !== "" && !uniqueRepos.some((r) => r.id === repoFilter)) {
      setRepoFilter("");
    }
  }, [uniqueRepos, repoFilter]);

  useEffect(() => {
    if (labelFilter !== null && !uniqueLabels.includes(labelFilter)) {
      setLabelFilter(null);
    }
  }, [uniqueLabels, labelFilter]);

  const preFiltered = useMemo<readonly Issue[]>(
    () =>
      labelFilter === null
        ? repoFiltered
        : repoFiltered.filter((issue) => issue.labels.includes(labelFilter)),
    [repoFiltered, labelFilter],
  );

  const searchPredicate = useCallback(
    (issue: Issue, query: string): boolean => {
      const repoName = repoMap.get(issue.repoId) ?? issue.repoId;
      return [issue.title, issue.author, repoName, ...issue.labels].some((value) =>
        value.toLowerCase().includes(query),
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
    filteredItems: matchingIssues,
    visibleItems: visible,
    tabCounts,
  } = useFilterableList<Issue, Tab>({
    items: preFiltered,
    tabs: ISSUE_TABS,
    defaultTab: "open",
    searchPredicate,
  });

  useEffect(() => {
    parentRef.current?.scrollTo({ top: 0, behavior: "instant" });
  }, [tab, normalizedQuery, repoFilter, labelFilter]);

  const virtualizer = useVirtualizer({
    count: visible.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 72,
    gap: 4,
    overscan: 3,
    // React 19 triggers "flushSync inside lifecycle" warnings with the default (true)
    useFlushSync: false,
  });

  const navItems = useMemo(() => visible.map((issue) => ({ url: issue.url })), [visible]);
  useRegisterNavigableItems(navItems);

  return (
    <section
      data-testid="issues"
      aria-busy={isLoading ? "true" : undefined}
      className="flex flex-col gap-2"
    >
      <SectionHead title="Issues" count={isLoading ? undefined : matchingIssues.length} />

      {isLoading ? (
        <>
          <div className="flex gap-1">
            <Skeleton className="h-11 w-16" />
            <Skeleton className="h-11 w-[4.5rem]" />
          </div>

          <div data-testid="issues-loading" className="flex flex-col gap-1">
            {Array.from({ length: 4 }, (_, index) => (
              <ListItemSkeleton
                key={`issue-skeleton-${index}`}
                testId="issue-card-skeleton"
                showPill
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
            placeholder="Filter issues..."
            aria-label="Filter issues"
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
              aria-pressed={tab === "closed"}
              onClick={() => setTab("closed")}
              className={`${FILTER_BUTTON_CLASS} ${
                tab === "closed"
                  ? "bg-accent text-bg font-semibold hover:bg-accent/80"
                  : "text-dim hover:bg-surface-hover hover:text-foreground"
              }`}
            >
              Closed {tabCounts.closed}
            </button>
          </div>

          {uniqueRepos.length > 1 && (
            <select
              aria-label="Filter by repo"
              value={repoFilter}
              onChange={(e) => setRepoFilter(e.target.value)}
              className={`cursor-pointer border border-border bg-surface text-foreground hover:border-foreground ${INLINE_CONTROL_CLASS}`}
            >
              <option value="">All repos</option>
              {uniqueRepos.map((r) => (
                <option key={r.id} value={r.id}>{r.fullName}</option>
              ))}
            </select>
          )}

          {uniqueLabels.length > 0 && (
            <div className="flex flex-wrap gap-1" role="group" aria-label="Filter by label">
              {uniqueLabels.map((label) => (
                <button
                  key={label}
                  type="button"
                  aria-pressed={labelFilter === label}
                  onClick={() => setLabelFilter(labelFilter === label ? null : label)}
                  className={`${FILTER_BUTTON_CLASS} ${
                    labelFilter === label
                      ? "bg-accent text-bg font-semibold hover:bg-accent/80"
                      : "text-dim hover:bg-surface-hover hover:text-foreground"
                  }`}
                >
                  {label}
                </button>
              ))}
            </div>
          )}

          {visible.length === 0 ? (
            <EmptyState icon="◎" message="No issues to display" />
          ) : (
            <div ref={parentRef} className="max-h-[600px] overflow-y-auto">
              <div
                className="relative w-full"
                style={{ height: `${virtualizer.getTotalSize()}px` }}
              >
                {virtualizer.getVirtualItems().map((virtualItem) => {
                  const issue = visible[virtualItem.index];
                  if (!issue)
                    return (
                      <div key={virtualItem.key} style={{ height: `${virtualItem.size}px` }} />
                    );
                  return (
                    <div
                      key={virtualItem.key}
                      className="absolute left-0 top-0 w-full"
                      style={{
                        height: `${virtualItem.size}px`,
                        transform: `translateY(${virtualItem.start}px)`,
                      }}
                    >
                      <IssueCard
                        issue={issue}
                        repoName={repoMap.get(issue.repoId) ?? issue.repoId}
                        onOpen={onOpen}
                      />
                    </div>
                  );
                })}
              </div>
            </div>
          )}
        </>
      )}
    </section>
  );
}

export const Issues = memo(IssuesImpl);
