import type { ReactElement } from "react";
import { useQuery } from "@tanstack/react-query";
import { getPersonalStats } from "../../lib/tauri";

interface StatItemProps {
  readonly label: string;
  readonly value: string;
}

function StatItem({ label, value }: StatItemProps): ReactElement {
  return (
    <div className="flex items-center justify-between text-sm">
      <span className="text-dim">{label}</span>
      <span className="font-mono text-white">{value}</span>
    </div>
  );
}

export function Stats(): ReactElement {
  const statsQuery = useQuery({
    queryKey: ["stats", "personal"],
    queryFn: getPersonalStats,
  });

  if (statsQuery.isLoading) {
    return (
      <div data-testid="settings-stats" className="flex flex-col gap-3">
        <h2 className="text-accent text-sm font-semibold uppercase tracking-wider">Statistics</h2>
        <span className="text-dim text-sm">Loading stats...</span>
      </div>
    );
  }

  if (statsQuery.error || !statsQuery.data) {
    return (
      <div data-testid="settings-stats" className="flex flex-col gap-3">
        <h2 className="text-accent text-sm font-semibold uppercase tracking-wider">Statistics</h2>
        <span className="text-dim text-sm">Stats unavailable</span>
      </div>
    );
  }

  const stats = statsQuery.data;
  const avgHours = Number.isFinite(stats.avgReviewResponseHours)
    ? `${Math.round(stats.avgReviewResponseHours * 10) / 10}h`
    : "N/A";

  return (
    <div data-testid="settings-stats" className="flex flex-col gap-3">
      <h2 className="text-accent text-sm font-semibold uppercase tracking-wider">Statistics</h2>
      <StatItem label="PRs merged this week" value={String(stats.prsMergedThisWeek)} />
      <StatItem label="Avg review response" value={avgHours} />
      <StatItem label="Reviews given this week" value={String(stats.reviewsGivenThisWeek)} />
      <StatItem label="Active workspaces" value={String(stats.activeWorkspaceCount)} />
    </div>
  );
}
