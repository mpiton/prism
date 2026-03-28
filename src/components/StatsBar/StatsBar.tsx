import type { ReactElement } from "react";
import { useGitHubData } from "../../hooks/useGitHubData";

function formatSyncedTime(syncedAt: string | null): string {
  if (!syncedAt) return "never";

  const diffMs = Date.now() - new Date(syncedAt).getTime();
  const diffSec = Math.floor(diffMs / 1000);

  if (diffSec < 0) return "synced just now";

  if (diffSec < 60) return `synced ${diffSec}s ago`;
  const diffMin = Math.floor(diffSec / 60);
  if (diffMin < 60) return `synced ${diffMin}m ago`;
  const diffHours = Math.floor(diffMin / 60);
  return `synced ${diffHours}h ago`;
}

interface StatItemProps {
  readonly label: string;
  readonly value: number | null;
  readonly testId: string;
  readonly highlight?: boolean;
}

function StatItem({ label, value, testId, highlight }: StatItemProps): ReactElement {
  return (
    <div className="flex flex-col gap-0.5">
      <span
        data-testid={`${testId}-value`}
        className={`text-lg font-bold leading-none ${highlight ? "text-accent" : "text-white"}`}
      >
        {value ?? "—"}
      </span>
      <span className="text-[10px] text-dim">{label}</span>
    </div>
  );
}

export function StatsBar(): ReactElement {
  const { stats, dashboard } = useGitHubData();

  const syncedAt = dashboard?.syncedAt ?? null;

  return (
    <div
      data-testid="stats-bar"
      className="flex items-center justify-between border-b border-border px-4 py-2"
    >
      <div className="flex items-center gap-6">
        <StatItem
          label="Pending Reviews"
          value={stats?.pendingReviews ?? null}
          testId="stat-pending-reviews"
          highlight
        />
        <StatItem
          label="Open PRs"
          value={stats?.openPrs ?? null}
          testId="stat-open-prs"
        />
        <StatItem
          label="Issues"
          value={stats?.openIssues ?? null}
          testId="stat-issues"
        />
        <StatItem
          label="Workspaces"
          value={stats?.activeWorkspaces ?? null}
          testId="stat-workspaces"
        />
      </div>
      <span className="text-xs text-dim">{formatSyncedTime(syncedAt)}</span>
    </div>
  );
}
