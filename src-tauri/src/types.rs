use std::fmt;

use serde::{Deserialize, Serialize};

/// Pull request state.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrState {
    Open,
    Closed,
    Merged,
    Draft,
}

/// CI pipeline status.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CiStatus {
    Pending,
    Running,
    Success,
    Failure,
    Cancelled,
}

impl CiStatus {
    /// Parse a CI status string. Returns `None` for unrecognised values.
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "running" => Some(Self::Running),
            "success" => Some(Self::Success),
            "failure" => Some(Self::Failure),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

/// Priority level. `Critical` is the highest priority (`Critical > High > Medium > Low`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Low,
    Medium,
    High,
    Critical,
}

/// Code review status.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    Pending,
    Approved,
    ChangesRequested,
    Commented,
    Dismissed,
}

/// Issue state.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueState {
    Open,
    Closed,
}

/// Activity feed event type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityType {
    PrOpened,
    PrMerged,
    PrClosed,
    ReviewSubmitted,
    CommentAdded,
    CiCompleted,
    IssueOpened,
    IssueClosed,
}

/// Workspace lifecycle state.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceState {
    Active,
    Suspended,
    Archived,
}

// ── Core structs (T-009) ────────────────────────────────────────

/// GitHub repository with local tracking state.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Repo {
    pub id: String,
    pub org: String,
    pub name: String,
    pub full_name: String,
    pub url: String,
    pub default_branch: String,
    pub is_archived: bool,
    pub enabled: bool,
    pub local_path: Option<String>,
    pub last_sync_at: Option<String>,
}

/// Pull request.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub id: String,
    pub number: u32,
    pub title: String,
    pub author: String,
    pub state: PrState,
    pub ci_status: CiStatus,
    pub priority: Priority,
    pub repo_id: String,
    pub url: String,
    pub labels: Vec<String>,
    pub additions: u32,
    pub deletions: u32,
    pub head_ref_name: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Review request assigned to a reviewer.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewRequest {
    pub id: String,
    pub pull_request_id: String,
    pub reviewer: String,
    pub status: ReviewStatus,
    pub requested_at: String,
}

/// Aggregated review summary for a pull request.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewSummary {
    pub total_reviews: u32,
    pub approved: u32,
    pub changes_requested: u32,
    pub pending: u32,
    pub reviewers: Vec<String>,
}

/// Code review submitted on a pull request.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    pub id: String,
    pub pull_request_id: String,
    pub reviewer: String,
    pub status: ReviewStatus,
    pub body: Option<String>,
    pub submitted_at: String,
}

/// GitHub issue.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub id: String,
    pub number: u32,
    pub title: String,
    pub author: String,
    pub state: IssueState,
    pub priority: Priority,
    pub repo_id: String,
    pub url: String,
    pub labels: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Activity feed event.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub id: String,
    pub activity_type: ActivityType,
    pub actor: String,
    pub repo_id: String,
    pub pull_request_id: Option<String>,
    pub issue_id: Option<String>,
    pub message: String,
    pub created_at: String,
}

/// PR workspace with git worktree and Claude Code session.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub repo_id: String,
    pub pull_request_number: u32,
    pub state: WorkspaceState,
    pub worktree_path: Option<String>,
    pub session_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Note attached to a workspace.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceNote {
    pub id: String,
    pub workspace_id: String,
    pub content: String,
    pub created_at: String,
}

// ── Workspace list entry (T-097) ─────────────────────────────────

/// Enriched workspace entry with git status, CI, and notes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceListEntry {
    pub workspace: Workspace,
    pub branch: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub ci_status: Option<CiStatus>,
    pub github_url: Option<String>,
    /// Currently 0 or 1 — derived from the single `session_id` column.
    /// Will become a real count when multi-session support is added.
    pub session_count: u32,
    pub disk_usage_mb: Option<u64>,
    pub last_note: Option<String>,
}

// ── Composite structs (T-010) ─────────────────────────────────────

/// Projection of [`Workspace`] for embedding in dashboard views.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSummary {
    pub id: String,
    pub state: WorkspaceState,
    /// Content of the most recent [`WorkspaceNote`], if any.
    pub last_note_content: Option<String>,
}

/// Pull request enriched with review summary and optional workspace.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullRequestWithReview {
    pub pull_request: PullRequest,
    pub review_summary: ReviewSummary,
    pub workspace: Option<WorkspaceSummary>,
}

/// Full dashboard aggregate returned by a single IPC call.
// Hash cannot be derived: Vec<T> does not implement Hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardData {
    pub review_requests: Vec<PullRequestWithReview>,
    pub my_pull_requests: Vec<PullRequestWithReview>,
    pub assigned_issues: Vec<Issue>,
    pub recent_activity: Vec<Activity>,
    pub workspaces: Vec<Workspace>,
    /// `None` before the first GitHub sync completes.
    pub synced_at: Option<String>,
}

/// Dashboard counter stats for the header/sidebar.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardStats {
    pub pending_reviews: u32,
    pub open_prs: u32,
    pub open_issues: u32,
    pub active_workspaces: u32,
    pub unread_activity: u32,
}

/// Personal statistics for the authenticated user (T-085).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalStats {
    pub prs_merged_this_week: u32,
    pub avg_review_response_hours: f64,
    pub reviews_given_this_week: u32,
    pub active_workspace_count: u32,
}

// ── IPC payloads (T-011) ──────────────────────────────────────

/// Request payload for the `workspace_open` IPC command.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenWorkspaceRequest {
    pub repo_id: String,
    pub pull_request_number: u32,
    /// Git branch name for the PR (e.g. `fix/bug-42`).
    pub branch: String,
}

/// Response payload from the `workspace_open` IPC command.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenWorkspaceResponse {
    pub workspace_id: String,
    /// Absolute path to the git worktree directory.
    pub worktree_path: String,
    /// PTY identifier (UUID) for terminal I/O commands.
    pub pty_id: String,
    /// `None` until a Claude Code session is started.
    pub session_id: Option<String>,
}

/// Terminal stdin data sent to a workspace PTY via `workspace:stdin` event.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyInput {
    pub workspace_id: String,
    /// UTF-8 text from xterm.js (escape sequences encoded inline).
    pub data: String,
}

/// Terminal stdout data received from a workspace PTY via `workspace:stdout` event.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyOutput {
    pub workspace_id: String,
    /// UTF-8 text for xterm.js (escape sequences encoded inline).
    pub data: String,
}

/// Terminal resize event for a workspace PTY.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PtyResize {
    pub workspace_id: String,
    pub cols: u16,
    pub rows: u16,
}

/// Memory usage statistics returned by `debug_memory_usage` (T-087).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryStats {
    /// Resident Set Size of the main process in bytes.
    pub rss_bytes: u64,
    /// Size of the `SQLite` database file in bytes.
    pub db_size_bytes: u64,
}

/// Event payload emitted when a workspace changes state (T-070).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStateChanged {
    pub workspace_id: String,
    pub new_state: WorkspaceState,
}

/// Application-level configuration persisted in the `config` table.
// Hash omitted: config structs are likely to gain Vec fields (e.g. watched_repos).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    /// GitHub sync polling interval in seconds (minimum 30, default 300).
    pub poll_interval_secs: u64,
    /// Maximum number of simultaneously active workspaces (LRU eviction).
    pub max_active_workspaces: u32,
    /// Hours after a PR is merged before its workspace is auto-archived (default 24).
    pub archive_delay_hours: u64,
    /// Hours after a PR is closed before its workspace is auto-archived (default 48).
    pub archive_delay_closed_hours: u64,
    /// Minutes of inactivity before an active workspace is auto-suspended (default 30).
    pub auto_suspend_minutes: u64,
    /// GitHub personal access token or OAuth token. `None` means not yet configured.
    pub github_token: Option<String>,
    /// Override for the `SQLite` data directory (`~/.local/share/prism/` by default).
    pub data_dir: Option<String>,
    /// Override for the workspaces root directory (`~/.prism/workspaces/` by default).
    pub workspaces_dir: Option<String>,
}

impl fmt::Debug for AppConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppConfig")
            .field("poll_interval_secs", &self.poll_interval_secs)
            .field("max_active_workspaces", &self.max_active_workspaces)
            .field("archive_delay_hours", &self.archive_delay_hours)
            .field(
                "archive_delay_closed_hours",
                &self.archive_delay_closed_hours,
            )
            .field("auto_suspend_minutes", &self.auto_suspend_minutes)
            .field(
                "github_token",
                &self.github_token.as_ref().map(|_| "<redacted>"),
            )
            .field("data_dir", &self.data_dir)
            .field("workspaces_dir", &self.workspaces_dir)
            .finish()
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            poll_interval_secs: 300,
            max_active_workspaces: 3,
            archive_delay_hours: 24,
            archive_delay_closed_hours: 48,
            auto_suspend_minutes: 30,
            github_token: None,
            data_dir: None,
            workspaces_dir: None,
        }
    }
}

/// Deserializer for `Option<Option<T>>` that distinguishes three JSON states:
/// - key absent → `None` (don't touch)
/// - key present with `null` → `Some(None)` (clear)
/// - key present with value → `Some(Some(v))` (set)
///
/// Standard serde collapses absent and `null` into `None` for the outer
/// `Option`, making `Some(None)` unreachable. This custom deserializer
/// wraps the inner `Option<T>` result in `Some(...)` whenever the key
/// is present in the JSON payload (the `#[serde(default)]` on the struct
/// ensures absent keys produce `None` at the outer level).
#[allow(clippy::option_option)]
fn deserialize_double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    // If serde calls this function, the key IS present in the JSON.
    // Deserialize the inner Option<T>: null → None, value → Some(v).
    Ok(Some(Option::<T>::deserialize(deserializer)?))
}

/// Partial update payload for [`AppConfig`], used by the `config_set` IPC command.
///
/// Each optional field uses `None` = "don't touch this field".
/// For nullable fields (`github_token`, `data_dir`, `workspaces_dir`),
/// `Some(None)` means "explicitly clear to null" and `Some(Some(v))` means "set to v".
///
/// **Note:** To update the GitHub token with validation, use `auth_set_token` instead.
/// Setting `github_token` here bypasses API validation — use only for clearing the token
/// or for advanced scenarios where the caller has already validated the token.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[allow(clippy::option_option)] // Deliberate: None = absent, Some(None) = clear, Some(Some(v)) = set
pub struct PartialAppConfig {
    pub poll_interval_secs: Option<u64>,
    pub max_active_workspaces: Option<u32>,
    pub archive_delay_hours: Option<u64>,
    pub archive_delay_closed_hours: Option<u64>,
    pub auto_suspend_minutes: Option<u64>,
    #[serde(deserialize_with = "deserialize_double_option", default)]
    pub github_token: Option<Option<String>>,
    #[serde(deserialize_with = "deserialize_double_option", default)]
    pub data_dir: Option<Option<String>>,
    #[serde(deserialize_with = "deserialize_double_option", default)]
    pub workspaces_dir: Option<Option<String>>,
}

/// Merge a partial update into a base config, returning a new config.
///
/// Only fields present in `partial` override the base.
pub fn merge_partial_config(base: &AppConfig, partial: &PartialAppConfig) -> AppConfig {
    AppConfig {
        poll_interval_secs: partial
            .poll_interval_secs
            .unwrap_or(base.poll_interval_secs),
        max_active_workspaces: partial
            .max_active_workspaces
            .unwrap_or(base.max_active_workspaces),
        archive_delay_hours: partial
            .archive_delay_hours
            .unwrap_or(base.archive_delay_hours),
        archive_delay_closed_hours: partial
            .archive_delay_closed_hours
            .unwrap_or(base.archive_delay_closed_hours),
        auto_suspend_minutes: partial
            .auto_suspend_minutes
            .unwrap_or(base.auto_suspend_minutes),
        github_token: match &partial.github_token {
            Some(v) => v.clone(),
            None => base.github_token.clone(),
        },
        data_dir: match &partial.data_dir {
            Some(v) => v.clone(),
            None => base.data_dir.clone(),
        },
        workspaces_dir: match &partial.workspaces_dir {
            Some(v) => v.clone(),
            None => base.workspaces_dir.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pr_state_serialization() {
        assert_eq!(serde_json::to_string(&PrState::Open).unwrap(), "\"open\"");
        assert_eq!(
            serde_json::to_string(&PrState::Closed).unwrap(),
            "\"closed\""
        );
        assert_eq!(
            serde_json::to_string(&PrState::Merged).unwrap(),
            "\"merged\""
        );
        assert_eq!(serde_json::to_string(&PrState::Draft).unwrap(), "\"draft\"");

        let deserialized: PrState = serde_json::from_str("\"merged\"").unwrap();
        assert_eq!(deserialized, PrState::Merged);
    }

    #[test]
    fn test_ci_status_serialization() {
        assert_eq!(
            serde_json::to_string(&CiStatus::Pending).unwrap(),
            "\"pending\""
        );
        assert_eq!(
            serde_json::to_string(&CiStatus::Running).unwrap(),
            "\"running\""
        );
        assert_eq!(
            serde_json::to_string(&CiStatus::Success).unwrap(),
            "\"success\""
        );
        assert_eq!(
            serde_json::to_string(&CiStatus::Failure).unwrap(),
            "\"failure\""
        );
        assert_eq!(
            serde_json::to_string(&CiStatus::Cancelled).unwrap(),
            "\"cancelled\""
        );

        let deserialized: CiStatus = serde_json::from_str("\"failure\"").unwrap();
        assert_eq!(deserialized, CiStatus::Failure);
    }

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::Critical > Priority::High);
        assert!(Priority::High > Priority::Medium);
        assert!(Priority::Medium > Priority::Low);

        let mut priorities = vec![
            Priority::Low,
            Priority::Critical,
            Priority::Medium,
            Priority::High,
        ];
        priorities.sort();
        assert_eq!(
            priorities,
            vec![
                Priority::Low,
                Priority::Medium,
                Priority::High,
                Priority::Critical
            ]
        );
    }

    #[test]
    fn test_priority_serialization() {
        assert_eq!(
            serde_json::to_string(&Priority::Critical).unwrap(),
            "\"critical\""
        );
        assert_eq!(serde_json::to_string(&Priority::High).unwrap(), "\"high\"");
        assert_eq!(
            serde_json::to_string(&Priority::Medium).unwrap(),
            "\"medium\""
        );
        assert_eq!(serde_json::to_string(&Priority::Low).unwrap(), "\"low\"");

        let deserialized: Priority = serde_json::from_str("\"high\"").unwrap();
        assert_eq!(deserialized, Priority::High);
    }

    #[test]
    fn test_review_status_serialization() {
        assert_eq!(
            serde_json::to_string(&ReviewStatus::Pending).unwrap(),
            "\"pending\""
        );
        assert_eq!(
            serde_json::to_string(&ReviewStatus::Approved).unwrap(),
            "\"approved\""
        );
        assert_eq!(
            serde_json::to_string(&ReviewStatus::ChangesRequested).unwrap(),
            "\"changes_requested\""
        );
        assert_eq!(
            serde_json::to_string(&ReviewStatus::Commented).unwrap(),
            "\"commented\""
        );
        assert_eq!(
            serde_json::to_string(&ReviewStatus::Dismissed).unwrap(),
            "\"dismissed\""
        );

        let deserialized: ReviewStatus = serde_json::from_str("\"changes_requested\"").unwrap();
        assert_eq!(deserialized, ReviewStatus::ChangesRequested);
    }

    #[test]
    fn test_issue_state_serialization() {
        assert_eq!(
            serde_json::to_string(&IssueState::Open).unwrap(),
            "\"open\""
        );
        assert_eq!(
            serde_json::to_string(&IssueState::Closed).unwrap(),
            "\"closed\""
        );

        let deserialized: IssueState = serde_json::from_str("\"closed\"").unwrap();
        assert_eq!(deserialized, IssueState::Closed);
    }

    #[test]
    fn test_activity_type_serialization() {
        assert_eq!(
            serde_json::to_string(&ActivityType::PrOpened).unwrap(),
            "\"pr_opened\""
        );
        assert_eq!(
            serde_json::to_string(&ActivityType::PrMerged).unwrap(),
            "\"pr_merged\""
        );
        assert_eq!(
            serde_json::to_string(&ActivityType::PrClosed).unwrap(),
            "\"pr_closed\""
        );
        assert_eq!(
            serde_json::to_string(&ActivityType::ReviewSubmitted).unwrap(),
            "\"review_submitted\""
        );
        assert_eq!(
            serde_json::to_string(&ActivityType::CommentAdded).unwrap(),
            "\"comment_added\""
        );
        assert_eq!(
            serde_json::to_string(&ActivityType::CiCompleted).unwrap(),
            "\"ci_completed\""
        );
        assert_eq!(
            serde_json::to_string(&ActivityType::IssueOpened).unwrap(),
            "\"issue_opened\""
        );
        assert_eq!(
            serde_json::to_string(&ActivityType::IssueClosed).unwrap(),
            "\"issue_closed\""
        );

        let deserialized: ActivityType = serde_json::from_str("\"review_submitted\"").unwrap();
        assert_eq!(deserialized, ActivityType::ReviewSubmitted);
    }

    #[test]
    fn test_workspace_state_serialization() {
        assert_eq!(
            serde_json::to_string(&WorkspaceState::Active).unwrap(),
            "\"active\""
        );
        assert_eq!(
            serde_json::to_string(&WorkspaceState::Suspended).unwrap(),
            "\"suspended\""
        );
        assert_eq!(
            serde_json::to_string(&WorkspaceState::Archived).unwrap(),
            "\"archived\""
        );

        let deserialized: WorkspaceState = serde_json::from_str("\"suspended\"").unwrap();
        assert_eq!(deserialized, WorkspaceState::Suspended);
    }

    // ── T-009: Core struct roundtrip tests ──────────────────────────

    #[test]
    fn test_repo_json_roundtrip() {
        let repo = Repo {
            id: "r-1".to_string(),
            org: "mpiton".to_string(),
            name: "prism".to_string(),
            full_name: "mpiton/prism".to_string(),
            url: "https://github.com/mpiton/prism".to_string(),
            default_branch: "main".to_string(),
            is_archived: false,
            enabled: true,
            local_path: Some("/home/user/repos/prism".to_string()),
            last_sync_at: None,
        };
        let json = serde_json::to_string(&repo).unwrap();
        assert!(json.contains("\"fullName\""));
        assert!(json.contains("\"defaultBranch\""));
        assert!(json.contains("\"isArchived\""));
        assert!(json.contains("\"localPath\""));
        assert!(json.contains("\"lastSyncAt\""));
        let deserialized: Repo = serde_json::from_str(&json).unwrap();
        assert_eq!(repo, deserialized);
    }

    #[test]
    fn test_pull_request_json_roundtrip() {
        let pr = PullRequest {
            id: "pr-1".to_string(),
            number: 42,
            title: "Add feature".to_string(),
            author: "mpiton".to_string(),
            state: PrState::Open,
            ci_status: CiStatus::Success,
            priority: Priority::High,
            repo_id: "r-1".to_string(),
            url: "https://github.com/mpiton/prism/pull/42".to_string(),
            labels: vec!["enhancement".to_string(), "frontend".to_string()],
            additions: 50,
            deletions: 10,
            head_ref_name: "fix/test-branch".to_string(),
            created_at: "2026-03-24T10:00:00Z".to_string(),
            updated_at: "2026-03-24T12:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&pr).unwrap();
        assert!(json.contains("\"ciStatus\""));
        assert!(json.contains("\"repoId\""));
        assert!(json.contains("\"createdAt\""));
        assert!(json.contains("\"updatedAt\""));
        let deserialized: PullRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(pr, deserialized);
    }

    #[test]
    fn test_review_request_json_roundtrip() {
        let rr = ReviewRequest {
            id: "rr-1".to_string(),
            pull_request_id: "pr-1".to_string(),
            reviewer: "alice".to_string(),
            status: ReviewStatus::Pending,
            requested_at: "2026-03-24T10:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&rr).unwrap();
        assert!(json.contains("\"pullRequestId\""));
        assert!(json.contains("\"requestedAt\""));
        let deserialized: ReviewRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(rr, deserialized);
    }

    #[test]
    fn test_review_summary_json_roundtrip() {
        let rs = ReviewSummary {
            total_reviews: 3,
            approved: 1,
            changes_requested: 1,
            pending: 1,
            reviewers: vec!["alice".to_string(), "bob".to_string()],
        };
        let json = serde_json::to_string(&rs).unwrap();
        assert!(json.contains("\"totalReviews\""));
        assert!(json.contains("\"changesRequested\""));
        let deserialized: ReviewSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(rs, deserialized);
    }

    #[test]
    fn test_issue_json_roundtrip() {
        let issue = Issue {
            id: "i-1".to_string(),
            number: 10,
            title: "Bug report".to_string(),
            author: "bob".to_string(),
            state: IssueState::Open,
            priority: Priority::Critical,
            repo_id: "r-1".to_string(),
            url: "https://github.com/mpiton/prism/issues/10".to_string(),
            labels: vec!["bug".to_string()],
            created_at: "2026-03-24T10:00:00Z".to_string(),
            updated_at: "2026-03-24T12:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&issue).unwrap();
        assert!(json.contains("\"repoId\""));
        assert!(json.contains("\"createdAt\""));
        let deserialized: Issue = serde_json::from_str(&json).unwrap();
        assert_eq!(issue, deserialized);
    }

    #[test]
    fn test_activity_json_roundtrip() {
        let activity = Activity {
            id: "a-1".to_string(),
            activity_type: ActivityType::PrOpened,
            actor: "mpiton".to_string(),
            repo_id: "r-1".to_string(),
            pull_request_id: Some("pr-1".to_string()),
            issue_id: None,
            message: "Opened PR #42".to_string(),
            created_at: "2026-03-24T10:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&activity).unwrap();
        assert!(json.contains("\"activityType\""));
        assert!(json.contains("\"repoId\""));
        assert!(json.contains("\"pullRequestId\""));
        assert!(json.contains("\"issueId\""));
        assert!(json.contains("\"createdAt\""));
        let deserialized: Activity = serde_json::from_str(&json).unwrap();
        assert_eq!(activity, deserialized);
    }

    #[test]
    fn test_workspace_json_roundtrip() {
        let ws = Workspace {
            id: "ws-1".to_string(),
            repo_id: "r-1".to_string(),
            pull_request_number: 42,
            state: WorkspaceState::Active,
            worktree_path: Some("/home/user/.prism/workspaces/prism/worktrees/pr-42".to_string()),
            session_id: Some("session-abc".to_string()),
            created_at: "2026-03-24T10:00:00Z".to_string(),
            updated_at: "2026-03-24T12:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&ws).unwrap();
        assert!(json.contains("\"repoId\""));
        assert!(json.contains("\"pullRequestNumber\""));
        assert!(json.contains("\"worktreePath\""));
        assert!(json.contains("\"sessionId\""));
        let deserialized: Workspace = serde_json::from_str(&json).unwrap();
        assert_eq!(ws, deserialized);
    }

    #[test]
    fn test_review_json_roundtrip() {
        let review = Review {
            id: "rev-1".to_string(),
            pull_request_id: "pr-1".to_string(),
            reviewer: "alice".to_string(),
            status: ReviewStatus::Approved,
            body: Some("Looks good!".to_string()),
            submitted_at: "2026-03-24T10:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&review).unwrap();
        assert!(json.contains("\"pullRequestId\""));
        assert!(json.contains("\"submittedAt\""));
        let deserialized: Review = serde_json::from_str(&json).unwrap();
        assert_eq!(review, deserialized);

        // Verify null body roundtrips
        let no_body = Review {
            body: None,
            ..review.clone()
        };
        let json2 = serde_json::to_string(&no_body).unwrap();
        assert!(json2.contains("\"body\":null"));
        let deserialized2: Review = serde_json::from_str(&json2).unwrap();
        assert_eq!(no_body, deserialized2);
    }

    // ── T-010: Composite struct roundtrip tests ─────────────────────

    #[test]
    fn test_workspace_summary_json_roundtrip() {
        let ws = WorkspaceSummary {
            id: "ws-1".to_string(),
            state: WorkspaceState::Active,
            last_note_content: Some("LGTM, ready to merge".to_string()),
        };
        let json = serde_json::to_string(&ws).unwrap();
        assert!(json.contains("\"lastNoteContent\""));
        let deserialized: WorkspaceSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(ws, deserialized);
    }

    #[test]
    fn test_pr_with_review_json_roundtrip() {
        let pr_with_review = PullRequestWithReview {
            pull_request: PullRequest {
                id: "pr-1".to_string(),
                number: 42,
                title: "Add feature".to_string(),
                author: "mpiton".to_string(),
                state: PrState::Open,
                ci_status: CiStatus::Success,
                priority: Priority::High,
                repo_id: "r-1".to_string(),
                url: "https://github.com/mpiton/prism/pull/42".to_string(),
                labels: vec!["enhancement".to_string()],
                additions: 50,
                deletions: 10,
                head_ref_name: "fix/test-branch".to_string(),
                created_at: "2026-03-24T10:00:00Z".to_string(),
                updated_at: "2026-03-24T12:00:00Z".to_string(),
            },
            review_summary: ReviewSummary {
                total_reviews: 2,
                approved: 1,
                changes_requested: 0,
                pending: 1,
                reviewers: vec!["alice".to_string(), "bob".to_string()],
            },
            workspace: Some(WorkspaceSummary {
                id: "ws-1".to_string(),
                state: WorkspaceState::Active,
                last_note_content: None,
            }),
        };
        let json = serde_json::to_string(&pr_with_review).unwrap();
        assert!(json.contains("\"pullRequest\""));
        assert!(json.contains("\"reviewSummary\""));
        assert!(json.contains("\"workspace\""));
        let deserialized: PullRequestWithReview = serde_json::from_str(&json).unwrap();
        assert_eq!(pr_with_review, deserialized);
    }

    #[test]
    fn test_dashboard_data_json_roundtrip() {
        let pr_with_review = PullRequestWithReview {
            pull_request: PullRequest {
                id: "pr-99".to_string(),
                number: 99,
                title: "Dashboard endpoint".to_string(),
                author: "alice".to_string(),
                state: PrState::Open,
                ci_status: CiStatus::Success,
                priority: Priority::High,
                repo_id: "r-1".to_string(),
                url: "https://github.com/mpiton/prism/pull/99".to_string(),
                labels: vec![],
                additions: 50,
                deletions: 10,
                head_ref_name: "fix/test-branch".to_string(),
                created_at: "2026-03-24T10:00:00Z".to_string(),
                updated_at: "2026-03-24T12:00:00Z".to_string(),
            },
            review_summary: ReviewSummary {
                total_reviews: 1,
                approved: 1,
                changes_requested: 0,
                pending: 0,
                reviewers: vec!["bob".to_string()],
            },
            workspace: None,
        };
        let dashboard = DashboardData {
            review_requests: vec![pr_with_review],
            my_pull_requests: vec![],
            assigned_issues: vec![],
            recent_activity: vec![],
            workspaces: vec![],
            synced_at: Some("2026-03-24T14:00:00Z".to_string()),
        };
        let json = serde_json::to_string(&dashboard).unwrap();
        assert!(json.contains("\"reviewRequests\""));
        assert!(json.contains("\"myPullRequests\""));
        assert!(json.contains("\"assignedIssues\""));
        assert!(json.contains("\"recentActivity\""));
        assert!(json.contains("\"syncedAt\""));
        assert!(json.contains("\"pullRequest\""));
        assert!(json.contains("\"reviewSummary\""));
        let deserialized: DashboardData = serde_json::from_str(&json).unwrap();
        assert_eq!(dashboard, deserialized);
    }

    #[test]
    fn test_dashboard_stats_json_roundtrip() {
        let stats = DashboardStats {
            pending_reviews: 5,
            open_prs: 12,
            open_issues: 3,
            active_workspaces: 2,
            unread_activity: 8,
        };
        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"pendingReviews\""));
        assert!(json.contains("\"openPrs\""));
        assert!(json.contains("\"openIssues\""));
        assert!(json.contains("\"activeWorkspaces\""));
        assert!(json.contains("\"unreadActivity\""));
        let deserialized: DashboardStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats, deserialized);
    }

    #[test]
    fn test_personal_stats_json_roundtrip() {
        let stats = PersonalStats {
            prs_merged_this_week: 3,
            avg_review_response_hours: 2.5,
            reviews_given_this_week: 7,
            active_workspace_count: 1,
        };
        let json = serde_json::to_string(&stats).unwrap();
        assert!(json.contains("\"prsMergedThisWeek\""));
        assert!(json.contains("\"avgReviewResponseHours\""));
        assert!(json.contains("\"reviewsGivenThisWeek\""));
        assert!(json.contains("\"activeWorkspaceCount\""));
        let deserialized: PersonalStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats, deserialized);
    }

    // ── T-011: IPC payload roundtrip tests ─────────────────────────

    #[test]
    fn test_open_workspace_request_json() {
        let req = OpenWorkspaceRequest {
            repo_id: "r-1".to_string(),
            pull_request_number: 42,
            branch: "fix/bug-42".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"repoId\""));
        assert!(json.contains("\"pullRequestNumber\""));
        assert!(json.contains("\"branch\""));
        let deserialized: OpenWorkspaceRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, deserialized);
    }

    #[test]
    fn test_open_workspace_response_json() {
        let resp = OpenWorkspaceResponse {
            workspace_id: "ws-1".to_string(),
            worktree_path: "/home/user/.prism/workspaces/prism/worktrees/pr-42".to_string(),
            pty_id: "pty-uuid-123".to_string(),
            session_id: Some("session-abc".to_string()),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"workspaceId\""));
        assert!(json.contains("\"worktreePath\""));
        assert!(json.contains("\"ptyId\""));
        assert!(json.contains("\"sessionId\""));
        let deserialized: OpenWorkspaceResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, deserialized);

        // Verify null sessionId roundtrips correctly
        let resp_no_session = OpenWorkspaceResponse {
            workspace_id: "ws-2".to_string(),
            worktree_path: "/tmp/worktree".to_string(),
            pty_id: "pty-uuid-456".to_string(),
            session_id: None,
        };
        let json2 = serde_json::to_string(&resp_no_session).unwrap();
        assert!(json2.contains("\"sessionId\":null"));
        let deserialized2: OpenWorkspaceResponse = serde_json::from_str(&json2).unwrap();
        assert_eq!(resp_no_session, deserialized2);
    }

    #[test]
    fn test_pty_input_json() {
        let input = PtyInput {
            workspace_id: "ws-1".to_string(),
            data: "ls -la\n".to_string(),
        };
        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("\"workspaceId\""));
        assert!(json.contains("\"data\""));
        let deserialized: PtyInput = serde_json::from_str(&json).unwrap();
        assert_eq!(input, deserialized);
    }

    #[test]
    fn test_pty_output_json() {
        let output = PtyOutput {
            workspace_id: "ws-1".to_string(),
            data: "total 42\ndrwxr-xr-x 2 user user 4096 Mar 24 10:00 src\n".to_string(),
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("\"workspaceId\""));
        assert!(json.contains("\"data\""));
        let deserialized: PtyOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(output, deserialized);
    }

    #[test]
    fn test_pty_resize_json() {
        let resize = PtyResize {
            workspace_id: "ws-1".to_string(),
            cols: 120,
            rows: 40,
        };
        let json = serde_json::to_string(&resize).unwrap();
        assert!(json.contains("\"workspaceId\""));
        assert!(json.contains("\"cols\""));
        assert!(json.contains("\"rows\""));
        let deserialized: PtyResize = serde_json::from_str(&json).unwrap();
        assert_eq!(resize, deserialized);
    }

    #[test]
    fn test_app_config_json_roundtrip() {
        let config = AppConfig {
            poll_interval_secs: 120,
            max_active_workspaces: 5,
            archive_delay_hours: 12,
            archive_delay_closed_hours: 72,
            auto_suspend_minutes: 30,
            github_token: Some("test-token".to_string()),
            data_dir: Some("/custom/data".to_string()),
            workspaces_dir: None,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"pollIntervalSecs\""));
        assert!(json.contains("\"maxActiveWorkspaces\""));
        assert!(json.contains("\"archiveDelayHours\""));
        assert!(json.contains("\"archiveDelayClosedHours\""));
        assert!(json.contains("\"githubToken\""));
        assert!(json.contains("\"dataDir\""));
        assert!(json.contains("\"workspacesDir\":null"));
        let deserialized: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_app_config_defaults() {
        let config = AppConfig::default();
        assert_eq!(config.poll_interval_secs, 300);
        assert_eq!(config.max_active_workspaces, 3);
        assert_eq!(config.archive_delay_hours, 24);
        assert_eq!(config.archive_delay_closed_hours, 48);
        assert!(config.github_token.is_none());
        assert!(config.data_dir.is_none());
        assert!(config.workspaces_dir.is_none());

        // Default config should roundtrip through JSON
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_memory_stats_serializes_camel_case() {
        let stats = MemoryStats {
            rss_bytes: 50_000_000,
            db_size_bytes: 1_234_567,
        };
        let json = serde_json::to_value(&stats).unwrap();
        assert_eq!(json["rssBytes"], 50_000_000);
        assert_eq!(json["dbSizeBytes"], 1_234_567);
        let roundtrip: MemoryStats = serde_json::from_value(json).unwrap();
        assert_eq!(stats, roundtrip);
    }

    #[test]
    fn test_workspace_note_json_roundtrip() {
        let note = WorkspaceNote {
            id: "wn-1".to_string(),
            workspace_id: "ws-1".to_string(),
            content: "Review feedback applied".to_string(),
            created_at: "2026-03-24T10:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&note).unwrap();
        assert!(json.contains("\"workspaceId\""));
        assert!(json.contains("\"createdAt\""));
        let deserialized: WorkspaceNote = serde_json::from_str(&json).unwrap();
        assert_eq!(note, deserialized);
    }
}
