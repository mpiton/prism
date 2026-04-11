import { useVirtualizer } from "@tanstack/react-virtual";
import { useQuery } from "@tanstack/react-query";
import { type ReactElement, useEffect, useMemo, useRef, useState } from "react";
import { FOCUS_RING } from "../../lib/a11y";
import { listRepos } from "../../lib/tauri";
import type { Issue } from "../../lib/types";
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

const FILTER_BUTTON_CLASS = `${FOCUS_RING} inline-flex min-h-11 min-w-11 items-center justify-center rounded px-3 text-xs leading-none transition-colors`;

function isOpen(issue: Issue): boolean {
  return issue.state === "open";
}

function isClosed(issue: Issue): boolean {
  return issue.state === "closed";
}

export function Issues({ issues, isLoading = false, onOpen }: IssuesProps): ReactElement {
  const [tab, setTab] = useState<Tab>("open");
  const [searchQuery, setSearchQuery] = useState("");
  const parentRef = useRef<HTMLDivElement>(null);
  const normalizedQuery = searchQuery.trim().toLowerCase();

  useEffect(() => {
    parentRef.current?.scrollTo({ top: 0, behavior: "instant" });
  }, [tab, normalizedQuery]);

  const { data: repos } = useQuery({ queryKey: ["repos"], queryFn: listRepos });

  const repoMap = useMemo<Map<string, string>>(() => {
    if (!repos) return new Map();
    return new Map(repos.map((repo) => [repo.id, repo.fullName]));
  }, [repos]);

  const matchesSearch = (issue: Issue): boolean => {
    if (normalizedQuery.length === 0) return true;

    const repoName = repoMap.get(issue.repoId) ?? issue.repoId;
    return [issue.title, issue.author, repoName, ...issue.labels].some((value) =>
      value.toLowerCase().includes(normalizedQuery),
    );
  };

  const matchingIssues = issues.filter(matchesSearch);
  const openIssues = matchingIssues.filter(isOpen);
  const closedIssues = matchingIssues.filter(isClosed);
  const visible = tab === "open" ? openIssues : closedIssues;

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
              Open {openIssues.length}
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
              Closed {closedIssues.length}
            </button>
          </div>

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
