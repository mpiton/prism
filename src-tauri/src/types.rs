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

/// GitHub repository.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Repo {
    pub id: String,
    pub name: String,
    pub full_name: String,
    pub url: String,
    pub default_branch: String,
    pub is_archived: bool,
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
            name: "prism".to_string(),
            full_name: "mpiton/prism".to_string(),
            url: "https://github.com/mpiton/prism".to_string(),
            default_branch: "main".to_string(),
            is_archived: false,
        };
        let json = serde_json::to_string(&repo).unwrap();
        assert!(json.contains("\"fullName\""));
        assert!(json.contains("\"defaultBranch\""));
        assert!(json.contains("\"isArchived\""));
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
