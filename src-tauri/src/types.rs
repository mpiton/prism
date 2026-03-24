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
}
