import type { ReactElement } from "react";

interface SkeletonProps {
  readonly className?: string;
  readonly testId?: string;
}

interface CardSkeletonProps {
  readonly testId?: string;
  readonly showPriorityBar?: boolean;
  readonly showTrailingBadge?: boolean;
}

interface ListItemSkeletonProps {
  readonly testId?: string;
  readonly showBodyLine?: boolean;
  readonly showPill?: boolean;
}

interface StatsBarSkeletonProps {
  readonly focusMode?: boolean;
}

export function Skeleton({
  className = "",
  testId,
}: SkeletonProps): ReactElement {
  return (
    <div
      aria-hidden="true"
      data-testid={testId}
      className={`animate-pulse rounded bg-surface ${className}`.trim()}
    />
  );
}

export function CardSkeleton({
  testId,
  showPriorityBar = false,
  showTrailingBadge = false,
}: CardSkeletonProps): ReactElement {
  return (
    <div
      aria-hidden="true"
      data-testid={testId}
      className="flex items-center gap-3 rounded border border-border px-3 py-2"
    >
      {showPriorityBar ? (
        <Skeleton className="h-10 w-1 rounded-full" />
      ) : (
        <Skeleton className="h-2.5 w-2.5 rounded-full" />
      )}

      <div className="flex min-w-0 flex-1 flex-col gap-1.5">
        <div className="flex items-center gap-2">
          <Skeleton className="h-4 max-w-[220px] flex-1" />
          <Skeleton className="h-3 w-10" />
          {showTrailingBadge && <Skeleton className="h-5 w-20 rounded-full" />}
        </div>

        <div className="flex items-center gap-2">
          <Skeleton className="h-3 w-16" />
          <Skeleton className="h-3 w-12" />
          <Skeleton className="h-3 w-14" />
          <Skeleton className="ml-auto h-3 w-14" />
        </div>
      </div>

      {showTrailingBadge && <Skeleton className="h-8 w-16 rounded" />}
    </div>
  );
}

export function ListItemSkeleton({
  testId,
  showBodyLine = false,
  showPill = true,
}: ListItemSkeletonProps): ReactElement {
  return (
    <div
      aria-hidden="true"
      data-testid={testId}
      className="flex flex-col gap-2 rounded border border-border px-3 py-2"
    >
      <div className="flex min-w-0 items-center gap-2">
        <Skeleton className="h-2.5 w-2.5 rounded-full" />
        <Skeleton className="h-3 w-10" />
        <Skeleton className="h-4 max-w-[200px] flex-1" />
      </div>

      <div className="flex min-w-0 flex-col gap-2 pl-[18px]">
        {showBodyLine && <Skeleton className="h-3 w-full max-w-[240px]" />}
        <div className="flex min-w-0 items-center gap-2">
          <Skeleton className="h-5 w-20 rounded-full" />
          {showPill && <Skeleton className="h-5 w-16 rounded-full" />}
          <Skeleton className="ml-auto h-3 w-12" />
        </div>
      </div>
    </div>
  );
}

function StatSkeleton({ testId }: { readonly testId: string }): ReactElement {
  return (
    <div data-testid={testId} className="flex flex-col gap-1">
      <Skeleton className="h-6 w-10" />
      <Skeleton className="h-3 w-16" />
    </div>
  );
}

export function StatsBarSkeleton({
  focusMode = false,
}: StatsBarSkeletonProps): ReactElement {
  return (
    <div
      data-testid="stats-bar"
      role="region"
      aria-label="Statistics"
      aria-busy="true"
      className="flex items-center justify-between border-b border-border px-4 py-2"
    >
      <div className="flex items-center gap-6">
        <StatSkeleton testId="stat-pending-reviews-skeleton" />
        <StatSkeleton testId="stat-open-prs-skeleton" />
        <StatSkeleton testId="stat-issues-skeleton" />
        <StatSkeleton testId="stat-workspaces-skeleton" />
      </div>

      <div className="flex items-center gap-3">
        {focusMode && (
          <span className="rounded bg-accent px-2 py-0.5 text-xs font-bold text-bg">
            FOCUS MODE
          </span>
        )}
        <Skeleton testId="stats-bar-sync-skeleton" className="h-4 w-24" />
      </div>
    </div>
  );
}
