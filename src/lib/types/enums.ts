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
