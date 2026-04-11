// ── GitHub domain entities — Repo, PR, Review, Issue, Activity ──

import type { PrState, CiStatus, Priority, ReviewStatus, IssueState, ActivityType } from "./enums";

export interface Repo {
  readonly id: string;
  readonly org: string;
  readonly name: string;
  readonly fullName: string;
  readonly url: string;
  readonly defaultBranch: string;
  readonly isArchived: boolean;
  readonly enabled: boolean;
  readonly localPath: string | null;
  readonly lastSyncAt: string | null;
}

export interface PullRequest {
  readonly id: string;
  readonly number: number;
  readonly title: string;
  readonly author: string;
  readonly state: PrState;
  readonly ciStatus: CiStatus;
  readonly priority: Priority;
  readonly repoId: string;
  readonly url: string;
  readonly labels: readonly string[];
  readonly additions?: number;
  readonly deletions?: number;
  readonly changedFiles?: number;
  readonly commentsCount?: number;
  readonly headRefName: string;
  readonly createdAt: string;
  readonly updatedAt: string;
}

export interface ReviewRequest {
  readonly id: string;
  readonly pullRequestId: string;
  readonly reviewer: string;
  readonly status: ReviewStatus;
  readonly requestedAt: string;
}

export interface ReviewSummary {
  readonly totalReviews: number;
  readonly approved: number;
  readonly changesRequested: number;
  readonly pending: number;
  readonly reviewers: readonly string[];
}

export interface Issue {
  readonly id: string;
  readonly number: number;
  readonly title: string;
  readonly author: string;
  readonly state: IssueState;
  readonly priority: Priority;
  readonly repoId: string;
  readonly url: string;
  readonly labels: readonly string[];
  readonly createdAt: string;
  readonly updatedAt: string;
}

export interface Activity {
  readonly id: string;
  readonly activityType: ActivityType;
  readonly actor: string;
  readonly repoId: string;
  readonly pullRequestId: string | null;
  readonly issueId: string | null;
  readonly message: string;
  readonly isRead: boolean;
  readonly createdAt: string;
}
