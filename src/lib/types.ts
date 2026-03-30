// ── Enums (T-008) — snake_case string unions matching Rust serde ──

export type PrState = "open" | "closed" | "merged" | "draft";

export type CiStatus = "pending" | "running" | "success" | "failure" | "cancelled";

export type Priority = "low" | "medium" | "high" | "critical";

export type ReviewStatus = "pending" | "approved" | "changes_requested" | "commented" | "dismissed";

export type IssueState = "open" | "closed";

export type ActivityType =
  | "pr_opened"
  | "pr_merged"
  | "pr_closed"
  | "review_submitted"
  | "comment_added"
  | "ci_completed"
  | "issue_opened"
  | "issue_closed";

export type WorkspaceState = "active" | "suspended" | "archived";

// ── Core structs (T-009) — camelCase interfaces matching Rust serde ──

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
  readonly createdAt: string;
}

export interface Workspace {
  readonly id: string;
  readonly repoId: string;
  readonly pullRequestNumber: number;
  readonly state: WorkspaceState;
  readonly worktreePath: string | null;
  readonly sessionId: string | null;
  readonly createdAt: string;
  readonly updatedAt: string;
}

export interface WorkspaceNote {
  readonly id: string;
  readonly workspaceId: string;
  readonly content: string;
  readonly createdAt: string;
}

export interface WorkspaceStatusInfo {
  readonly branch: string;
  readonly ahead: number;
  readonly behind: number;
  readonly ciStatus: CiStatus;
  readonly sessionName: string | null;
  readonly sessionCount: number;
  readonly githubUrl: string;
}

// ── Workspace list entry (T-076) ─────────────────────────────────

export interface WorkspaceListEntry {
  readonly workspace: Workspace;
  readonly branch: string | null;
  readonly ahead: number;
  readonly behind: number;
  readonly ciStatus: CiStatus | null;
  readonly sessionCount: number;
  readonly diskUsageMb: number | null;
  readonly lastNote: string | null;
}

// ── Composite structs (T-010) ────────────────────────────────────

export interface WorkspaceSummary {
  readonly id: string;
  readonly state: WorkspaceState;
  readonly lastNoteContent: string | null;
}

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
  readonly activeWorkspaces: number;
  readonly unreadActivity: number;
}

// ── Auth (T-027) ─────────────────────────────────────────────────

export interface AuthStatus {
  readonly connected: boolean;
  readonly username: string | null;
  readonly error: string | null;
}

// ── IPC payloads (T-011) ─────────────────────────────────────────

export interface OpenWorkspaceRequest {
  readonly repoId: string;
  readonly pullRequestNumber: number;
}

export interface OpenWorkspaceResponse {
  readonly workspaceId: string;
  readonly worktreePath: string;
  readonly sessionId: string | null;
}

export interface PtyInput {
  readonly workspaceId: string;
  readonly data: string;
}

export interface PtyOutput {
  readonly workspaceId: string;
  readonly data: string;
}

export interface PtyResize {
  readonly workspaceId: string;
  readonly cols: number;
  readonly rows: number;
}

export interface AppConfig {
  /** Rust type is u64 — safe as `number` since practical values never exceed 2^53. */
  readonly pollIntervalSecs: number;
  readonly maxActiveWorkspaces: number;
  /** Minutes of inactivity before an active workspace is auto-suspended (min 5, default 30). */
  readonly autoSuspendMinutes: number;
  readonly githubToken: string | null;
  readonly dataDir: string | null;
  readonly workspacesDir: string | null;
}

/** Partial update payload for `config_set`. Mirrors Rust `PartialAppConfig`.
 *
 * - `undefined` (key absent) = leave unchanged
 * - `null` (for nullable fields) = explicitly clear to null
 * - value = set to that value
 */
export interface PartialAppConfig {
  readonly pollIntervalSecs?: number;
  readonly maxActiveWorkspaces?: number;
  readonly autoSuspendMinutes?: number;
  /** undefined = leave unchanged, null = clear token, string = set token.
   * Prefer `authSetToken` for validated token updates. */
  readonly githubToken?: string | null;
  readonly dataDir?: string | null;
  readonly workspacesDir?: string | null;
}

// ── Tauri IPC command & event registries ─────────────────────────

export type TauriCommandName =
  | "github_get_dashboard"
  | "github_get_stats"
  | "github_force_sync"
  | "repos_list"
  | "repos_set_enabled"
  | "repos_set_local_path"
  | "workspace_open"
  | "workspace_suspend"
  | "workspace_resume"
  | "workspace_archive"
  | "workspace_list"
  | "workspace_get_notes"
  | "workspace_cleanup"
  | "pty_write"
  | "pty_resize"
  | "pty_kill"
  | "config_get"
  | "config_set"
  | "activity_mark_read"
  | "activity_mark_all_read"
  | "auth_set_token"
  | "auth_get_status"
  | "auth_logout";

export const TAURI_COMMANDS = {
  github_get_dashboard: "github_get_dashboard",
  github_get_stats: "github_get_stats",
  github_force_sync: "github_force_sync",
  repos_list: "repos_list",
  repos_set_enabled: "repos_set_enabled",
  repos_set_local_path: "repos_set_local_path",
  workspace_open: "workspace_open",
  workspace_suspend: "workspace_suspend",
  workspace_resume: "workspace_resume",
  workspace_archive: "workspace_archive",
  workspace_list: "workspace_list",
  workspace_get_notes: "workspace_get_notes",
  workspace_cleanup: "workspace_cleanup",
  pty_write: "pty_write",
  pty_resize: "pty_resize",
  pty_kill: "pty_kill",
  config_get: "config_get",
  config_set: "config_set",
  activity_mark_read: "activity_mark_read",
  activity_mark_all_read: "activity_mark_all_read",
  auth_set_token: "auth_set_token",
  auth_get_status: "auth_get_status",
  auth_logout: "auth_logout",
} as const satisfies Record<TauriCommandName, TauriCommandName>;

export type TauriEventName =
  | "github:updated"
  | "github:sync_error"
  | "workspace:stdout"
  | "workspace:state_changed"
  | "workspace:claude_session"
  | "notification:review_request"
  | "notification:ci_failure"
  | "notification:pr_approved";

export const TAURI_EVENTS = {
  "github:updated": "github:updated",
  "github:sync_error": "github:sync_error",
  "workspace:stdout": "workspace:stdout",
  "workspace:state_changed": "workspace:state_changed",
  "workspace:claude_session": "workspace:claude_session",
  "notification:review_request": "notification:review_request",
  "notification:ci_failure": "notification:ci_failure",
  "notification:pr_approved": "notification:pr_approved",
} as const satisfies Record<TauriEventName, TauriEventName>;
