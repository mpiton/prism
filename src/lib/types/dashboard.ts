// ── Dashboard composites and stats (T-010, T-085) ──

import type { PullRequest, ReviewSummary, Issue, Activity } from "./github";
import type { Workspace, WorkspaceSummary } from "./workspace";

export interface PullRequestWithReview {
  readonly pullRequest: PullRequest;
  readonly reviewSummary: ReviewSummary;
  readonly workspace: WorkspaceSummary | null;
}

export interface DashboardData {
  readonly reviewRequests: readonly PullRequestWithReview[];
  readonly myPullRequests: readonly PullRequestWithReview[];
  readonly assignedIssues: readonly Issue[];
  readonly recentActivity: readonly Activity[];
  readonly workspaces: readonly Workspace[];
  readonly syncedAt: string | null;
}

export interface DashboardStats {
  readonly pendingReviews: number;
  readonly openPrs: number;
  readonly openIssues: number;
  readonly totalWorkspaces: number;
  readonly unreadActivity: number;
}

export interface PersonalStats {
  readonly prsMergedThisWeek: number;
  readonly avgReviewResponseHours: number;
  readonly reviewsGivenThisWeek: number;
  readonly totalWorkspaceCount: number;
}
