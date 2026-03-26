#![allow(dead_code)] // Used by T-032 (sync)

//! Mapping functions from `graphql_client` generated types to `PRism` domain types.
//!
//! Converts `dashboard_data::PrFields`, `IssueFields`, and related GraphQL
//! response types into the Rust structs defined in `crate::types`.

use crate::error::AppError;
use crate::github::queries::dashboard_data::{
    self, IssueFields, PrFields, PullRequestReviewState, PullRequestState, StatusState,
};
use crate::types::{CiStatus, PrState, Priority, PullRequest, ReviewStatus};

/// Maps a GraphQL `PullRequestState` + `isDraft` to a `PRism` `PrState`.
///
/// Draft is only applied to `OPEN` PRs — closed/merged PRs retain their
/// terminal state even if `isDraft` is still true on the GitHub side.
pub fn map_pr_state(state: &PullRequestState, is_draft: bool) -> PrState {
    match state {
        PullRequestState::CLOSED => PrState::Closed,
        PullRequestState::MERGED => PrState::Merged,
        PullRequestState::OPEN | PullRequestState::Other(_) => {
            if is_draft {
                PrState::Draft
            } else {
                PrState::Open
            }
        }
    }
}

/// Maps a GraphQL `StatusCheckRollup.state` to a `PRism` `CiStatus`.
pub fn map_ci_status(commits: Option<&dashboard_data::PrFieldsCommits>) -> CiStatus {
    let rollup = commits
        .and_then(|c| c.nodes.as_ref())
        .and_then(|nodes| nodes.first())
        .and_then(|node| node.as_ref())
        .and_then(|n| n.commit.status_check_rollup.as_ref());

    match rollup {
        Some(r) => match &r.state {
            StatusState::SUCCESS => CiStatus::Success,
            StatusState::FAILURE | StatusState::ERROR => CiStatus::Failure,
            StatusState::PENDING | StatusState::EXPECTED | StatusState::Other(_) => {
                CiStatus::Pending
            }
        },
        None => CiStatus::Pending,
    }
}

/// Trait to extract label names from `graphql_client` generated label connection types.
trait LabelNode {
    fn name(&self) -> &str;
}

impl LabelNode for dashboard_data::PrFieldsLabelsNodes {
    fn name(&self) -> &str {
        &self.name
    }
}

impl LabelNode for dashboard_data::IssueFieldsLabelsNodes {
    fn name(&self) -> &str {
        &self.name
    }
}

/// Extracts label names from a GraphQL labels connection `nodes` array.
fn collect_label_names<T: LabelNode>(nodes: Option<&Vec<Option<T>>>) -> Vec<String> {
    nodes
        .map(|ns| {
            ns.iter()
                .filter_map(|n| n.as_ref())
                .map(|n| n.name().to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// Maps a GraphQL `PrFields` to a `PRism` `PullRequest`.
///
/// The `repo_id` is `repository.nameWithOwner` (e.g. "org/repo"), the canonical
/// repo identifier used throughout `PRism`.
/// Priority is set to `Priority::Medium` by default — actual scoring is done by T-031.
pub fn map_pr(pr: &PrFields) -> Result<PullRequest, AppError> {
    let author = pr
        .author
        .as_ref()
        .map_or_else(|| "ghost".to_string(), |a| a.login.clone());

    let number: u32 = pr
        .number
        .try_into()
        .map_err(|_| AppError::GitHub(format!("invalid PR number: {}", pr.number)))?;

    let state = map_pr_state(&pr.state, pr.is_draft);
    let ci_status = map_ci_status(pr.commits.as_ref());
    let labels = collect_label_names(pr.labels.as_ref().and_then(|l| l.nodes.as_ref()));

    Ok(PullRequest {
        id: pr.id.clone(),
        number,
        title: pr.title.clone(),
        author,
        state,
        ci_status,
        priority: Priority::Medium,
        repo_id: pr.repository.name_with_owner.clone(),
        url: pr.url.clone(),
        labels,
        additions: u32::try_from(pr.additions).map_err(|_| {
            AppError::GitHub(format!(
                "invalid additions for PR '{}': {}",
                pr.id, pr.additions
            ))
        })?,
        deletions: u32::try_from(pr.deletions).map_err(|_| {
            AppError::GitHub(format!(
                "invalid deletions for PR '{}': {}",
                pr.id, pr.deletions
            ))
        })?,
        created_at: pr.created_at.clone(),
        updated_at: pr.updated_at.clone(),
    })
}

/// Maps a GraphQL `PullRequestReviewState` to a `PRism` `ReviewStatus`.
pub fn map_review_status(state: &PullRequestReviewState) -> ReviewStatus {
    match state {
        PullRequestReviewState::APPROVED => ReviewStatus::Approved,
        PullRequestReviewState::CHANGES_REQUESTED => ReviewStatus::ChangesRequested,
        PullRequestReviewState::COMMENTED => ReviewStatus::Commented,
        PullRequestReviewState::DISMISSED => ReviewStatus::Dismissed,
        PullRequestReviewState::PENDING | PullRequestReviewState::Other(_) => ReviewStatus::Pending,
    }
}

/// Maps a GraphQL `PrFieldsReviewsNodes` to a `PRism` `Review`.
///
/// Uses the stable GitHub node ID for the review `id`.
/// `submitted_at` uses the GraphQL `submittedAt` field (when the reviewer
/// clicked "Submit"), falling back to `createdAt` if null (draft reviews).
pub fn map_review(
    review: &dashboard_data::PrFieldsReviewsNodes,
    pull_request_id: &str,
) -> crate::types::Review {
    let reviewer = review
        .author
        .as_ref()
        .map_or_else(|| "ghost".to_string(), |a| a.login.clone());

    let submitted_at = review
        .submitted_at
        .as_deref()
        .unwrap_or(&review.created_at)
        .to_string();

    crate::types::Review {
        id: review.id.clone(),
        pull_request_id: pull_request_id.to_string(),
        reviewer,
        status: map_review_status(&review.state),
        body: None,
        submitted_at,
    }
}

/// Maps a GraphQL `IssueFields` to a `PRism` `Issue`.
///
/// Priority is set to `Priority::Medium` by default.
pub fn map_issue(issue: &IssueFields) -> Result<crate::types::Issue, AppError> {
    let author = issue
        .author
        .as_ref()
        .map_or_else(|| "ghost".to_string(), |a| a.login.clone());

    let number: u32 = issue
        .number
        .try_into()
        .map_err(|_| AppError::GitHub(format!("invalid issue number: {}", issue.number)))?;

    let labels = collect_label_names(issue.labels.as_ref().and_then(|l| l.nodes.as_ref()));

    let state = match &issue.state {
        dashboard_data::IssueState::CLOSED => crate::types::IssueState::Closed,
        dashboard_data::IssueState::OPEN | dashboard_data::IssueState::Other(_) => {
            crate::types::IssueState::Open
        }
    };

    Ok(crate::types::Issue {
        id: issue.id.clone(),
        number,
        title: issue.title.clone(),
        author,
        state,
        priority: Priority::Medium,
        repo_id: issue.repository.name_with_owner.clone(),
        url: issue.url.clone(),
        labels,
        created_at: issue.created_at.clone(),
        updated_at: issue.updated_at.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::queries::dashboard_data::{
        IssueFields, IssueFieldsAuthor, IssueFieldsAuthorOn, IssueFieldsLabels,
        IssueFieldsLabelsNodes, IssueFieldsRepository, IssueState as GqlIssueState, PrFields,
        PrFieldsAuthor, PrFieldsAuthorOn, PrFieldsCommits, PrFieldsCommitsNodes,
        PrFieldsCommitsNodesCommit, PrFieldsCommitsNodesCommitStatusCheckRollup, PrFieldsLabels,
        PrFieldsLabelsNodes, PrFieldsRepository, PrFieldsReviewsNodes, PrFieldsReviewsNodesAuthor,
        PrFieldsReviewsNodesAuthorOn, PullRequestReviewState, PullRequestState, StatusState,
    };

    // ── Helpers ──────────────────────────────────────────────────

    fn make_pr_fields(overrides: PrFieldsOverrides) -> PrFields {
        PrFields {
            id: overrides.id.unwrap_or_else(|| "PR_123".to_string()),
            number: overrides.number.unwrap_or(42),
            title: overrides.title.unwrap_or_else(|| "Fix bug".to_string()),
            author: Some(PrFieldsAuthor {
                login: overrides.author.unwrap_or_else(|| "octocat".to_string()),
                on: PrFieldsAuthorOn::User,
            }),
            state: overrides.state.unwrap_or(PullRequestState::OPEN),
            is_draft: overrides.is_draft.unwrap_or(false),
            url: overrides
                .url
                .unwrap_or_else(|| "https://github.com/org/repo/pull/42".to_string()),
            created_at: overrides
                .created_at
                .unwrap_or_else(|| "2026-03-01T10:00:00Z".to_string()),
            updated_at: overrides
                .updated_at
                .unwrap_or_else(|| "2026-03-01T12:00:00Z".to_string()),
            additions: overrides.additions.unwrap_or(50),
            deletions: overrides.deletions.unwrap_or(10),
            head_ref_name: overrides
                .head_ref_name
                .unwrap_or_else(|| "fix/bug-42".to_string()),
            repository: PrFieldsRepository {
                name_with_owner: overrides.repo.unwrap_or_else(|| "org/repo".to_string()),
            },
            labels: overrides.labels,
            review_requests: None,
            reviews: None,
            commits: overrides.commits,
        }
    }

    #[derive(Default)]
    struct PrFieldsOverrides {
        id: Option<String>,
        number: Option<i64>,
        title: Option<String>,
        author: Option<String>,
        state: Option<PullRequestState>,
        is_draft: Option<bool>,
        url: Option<String>,
        created_at: Option<String>,
        updated_at: Option<String>,
        additions: Option<i64>,
        deletions: Option<i64>,
        head_ref_name: Option<String>,
        repo: Option<String>,
        labels: Option<PrFieldsLabels>,
        commits: Option<PrFieldsCommits>,
    }

    fn make_issue_fields(overrides: IssueFieldsOverrides) -> IssueFields {
        IssueFields {
            id: overrides.id.unwrap_or_else(|| "ISSUE_99".to_string()),
            number: overrides.number.unwrap_or(99),
            title: overrides.title.unwrap_or_else(|| "Bug report".to_string()),
            author: Some(IssueFieldsAuthor {
                login: overrides.author.unwrap_or_else(|| "octocat".to_string()),
                on: IssueFieldsAuthorOn::User,
            }),
            state: overrides.state.unwrap_or(GqlIssueState::OPEN),
            url: overrides
                .url
                .unwrap_or_else(|| "https://github.com/org/repo/issues/99".to_string()),
            created_at: overrides
                .created_at
                .unwrap_or_else(|| "2026-03-01T10:00:00Z".to_string()),
            updated_at: overrides
                .updated_at
                .unwrap_or_else(|| "2026-03-01T12:00:00Z".to_string()),
            repository: IssueFieldsRepository {
                name_with_owner: overrides.repo.unwrap_or_else(|| "org/repo".to_string()),
            },
            labels: overrides.labels,
        }
    }

    #[derive(Default)]
    struct IssueFieldsOverrides {
        id: Option<String>,
        number: Option<i64>,
        title: Option<String>,
        author: Option<String>,
        state: Option<GqlIssueState>,
        url: Option<String>,
        created_at: Option<String>,
        updated_at: Option<String>,
        repo: Option<String>,
        labels: Option<IssueFieldsLabels>,
    }

    fn make_commits_with_status(state: StatusState) -> Option<PrFieldsCommits> {
        Some(PrFieldsCommits {
            nodes: Some(vec![Some(PrFieldsCommitsNodes {
                commit: PrFieldsCommitsNodesCommit {
                    status_check_rollup: Some(PrFieldsCommitsNodesCommitStatusCheckRollup {
                        state,
                    }),
                },
            })]),
        })
    }

    fn make_labels(names: &[&str]) -> Option<PrFieldsLabels> {
        Some(PrFieldsLabels {
            nodes: Some(
                names
                    .iter()
                    .map(|n| {
                        Some(PrFieldsLabelsNodes {
                            name: n.to_string(),
                        })
                    })
                    .collect(),
            ),
        })
    }

    fn make_issue_labels(names: &[&str]) -> Option<IssueFieldsLabels> {
        Some(IssueFieldsLabels {
            nodes: Some(
                names
                    .iter()
                    .map(|n| {
                        Some(IssueFieldsLabelsNodes {
                            name: n.to_string(),
                        })
                    })
                    .collect(),
            ),
        })
    }

    // ── PR mapping tests ─────────────────────────────────────────

    #[test]
    fn test_map_pr_open() {
        let pr = make_pr_fields(PrFieldsOverrides {
            state: Some(PullRequestState::OPEN),
            ..Default::default()
        });
        let result = map_pr(&pr).unwrap();

        assert_eq!(result.id, "PR_123");
        assert_eq!(result.number, 42);
        assert_eq!(result.title, "Fix bug");
        assert_eq!(result.author, "octocat");
        assert_eq!(result.state, PrState::Open);
        assert_eq!(result.repo_id, "org/repo");
    }

    #[test]
    fn test_map_pr_closed() {
        let pr = make_pr_fields(PrFieldsOverrides {
            state: Some(PullRequestState::CLOSED),
            ..Default::default()
        });
        let result = map_pr(&pr).unwrap();
        assert_eq!(result.state, PrState::Closed);
    }

    #[test]
    fn test_map_pr_merged() {
        let pr = make_pr_fields(PrFieldsOverrides {
            state: Some(PullRequestState::MERGED),
            ..Default::default()
        });
        let result = map_pr(&pr).unwrap();
        assert_eq!(result.state, PrState::Merged);
    }

    #[test]
    fn test_map_pr_draft() {
        let pr = make_pr_fields(PrFieldsOverrides {
            state: Some(PullRequestState::OPEN),
            is_draft: Some(true),
            ..Default::default()
        });
        let result = map_pr(&pr).unwrap();
        assert_eq!(result.state, PrState::Draft);
    }

    #[test]
    fn test_map_pr_labels() {
        let pr = make_pr_fields(PrFieldsOverrides {
            labels: make_labels(&["bug", "critical", "enhancement"]),
            ..Default::default()
        });
        let result = map_pr(&pr).unwrap();
        assert_eq!(result.labels, vec!["bug", "critical", "enhancement"]);
    }

    #[test]
    fn test_map_pr_closed_draft_is_closed_not_draft() {
        let pr = make_pr_fields(PrFieldsOverrides {
            state: Some(PullRequestState::CLOSED),
            is_draft: Some(true),
            ..Default::default()
        });
        let result = map_pr(&pr).unwrap();
        assert_eq!(result.state, PrState::Closed);
    }

    #[test]
    fn test_map_pr_merged_draft_is_merged_not_draft() {
        let pr = make_pr_fields(PrFieldsOverrides {
            state: Some(PullRequestState::MERGED),
            is_draft: Some(true),
            ..Default::default()
        });
        let result = map_pr(&pr).unwrap();
        assert_eq!(result.state, PrState::Merged);
    }

    #[test]
    fn test_map_pr_null_fields() {
        let mut pr = make_pr_fields(PrFieldsOverrides::default());
        pr.author = None;
        pr.labels = None;
        pr.commits = None;

        let result = map_pr(&pr).unwrap();
        assert_eq!(result.author, "ghost");
        assert!(result.labels.is_empty());
        assert_eq!(result.ci_status, CiStatus::Pending);
    }

    #[test]
    fn test_map_pr_negative_number_returns_error() {
        let pr = make_pr_fields(PrFieldsOverrides {
            number: Some(-1),
            ..Default::default()
        });
        let err = map_pr(&pr).unwrap_err();
        assert!(err.to_string().contains("invalid PR number"));
    }

    #[test]
    fn test_map_pr_labels_with_null_node() {
        let labels = Some(PrFieldsLabels {
            nodes: Some(vec![
                Some(PrFieldsLabelsNodes {
                    name: "bug".to_string(),
                }),
                None,
                Some(PrFieldsLabelsNodes {
                    name: "fix".to_string(),
                }),
            ]),
        });
        let pr = make_pr_fields(PrFieldsOverrides {
            labels,
            ..Default::default()
        });
        let result = map_pr(&pr).unwrap();
        assert_eq!(result.labels, vec!["bug", "fix"]);
    }

    // ── Review mapping tests ─────────────────────────────────────

    #[test]
    fn test_map_review_approved() {
        let review_node = PrFieldsReviewsNodes {
            id: "PRR_abc123".to_string(),
            author: Some(PrFieldsReviewsNodesAuthor {
                login: "reviewer1".to_string(),
                on: PrFieldsReviewsNodesAuthorOn::User,
            }),
            state: PullRequestReviewState::APPROVED,
            created_at: "2026-03-01T14:00:00Z".to_string(),
            submitted_at: Some("2026-03-01T14:05:00Z".to_string()),
        };
        let result = map_review(&review_node, "PR_123");

        assert_eq!(result.id, "PRR_abc123");
        assert_eq!(result.reviewer, "reviewer1");
        assert_eq!(result.status, ReviewStatus::Approved);
        assert_eq!(result.pull_request_id, "PR_123");
        assert_eq!(result.submitted_at, "2026-03-01T14:05:00Z");
    }

    #[test]
    fn test_map_review_changes_requested() {
        let review_node = PrFieldsReviewsNodes {
            id: "PRR_def456".to_string(),
            author: Some(PrFieldsReviewsNodesAuthor {
                login: "reviewer2".to_string(),
                on: PrFieldsReviewsNodesAuthorOn::User,
            }),
            state: PullRequestReviewState::CHANGES_REQUESTED,
            created_at: "2026-03-01T15:00:00Z".to_string(),
            submitted_at: Some("2026-03-01T15:00:00Z".to_string()),
        };
        let result = map_review(&review_node, "PR_456");

        assert_eq!(result.reviewer, "reviewer2");
        assert_eq!(result.status, ReviewStatus::ChangesRequested);
    }

    #[test]
    fn test_map_review_commented() {
        let review_node = PrFieldsReviewsNodes {
            id: "PRR_ghi789".to_string(),
            author: Some(PrFieldsReviewsNodesAuthor {
                login: "commenter".to_string(),
                on: PrFieldsReviewsNodesAuthorOn::User,
            }),
            state: PullRequestReviewState::COMMENTED,
            created_at: "2026-03-01T16:00:00Z".to_string(),
            submitted_at: None,
        };
        let result = map_review(&review_node, "PR_789");
        assert_eq!(result.status, ReviewStatus::Commented);
    }

    #[test]
    fn test_map_review_dismissed() {
        let review_node = PrFieldsReviewsNodes {
            id: "PRR_jkl101".to_string(),
            author: Some(PrFieldsReviewsNodesAuthor {
                login: "dismisser".to_string(),
                on: PrFieldsReviewsNodesAuthorOn::User,
            }),
            state: PullRequestReviewState::DISMISSED,
            created_at: "2026-03-01T17:00:00Z".to_string(),
            submitted_at: Some("2026-03-01T17:00:00Z".to_string()),
        };
        let result = map_review(&review_node, "PR_101");
        assert_eq!(result.status, ReviewStatus::Dismissed);
    }

    #[test]
    fn test_map_review_null_author_returns_ghost() {
        let review_node = PrFieldsReviewsNodes {
            id: "PRR_ghost".to_string(),
            author: None,
            state: PullRequestReviewState::APPROVED,
            created_at: "2026-03-01T14:00:00Z".to_string(),
            submitted_at: Some("2026-03-01T14:00:00Z".to_string()),
        };
        let result = map_review(&review_node, "PR_123");
        assert_eq!(result.reviewer, "ghost");
    }

    #[test]
    fn test_map_review_submitted_at_falls_back_to_created_at() {
        let review_node = PrFieldsReviewsNodes {
            id: "PRR_fallback".to_string(),
            author: None,
            state: PullRequestReviewState::PENDING,
            created_at: "2026-03-01T10:00:00Z".to_string(),
            submitted_at: None,
        };
        let result = map_review(&review_node, "PR_999");
        assert_eq!(result.submitted_at, "2026-03-01T10:00:00Z");
    }

    // ── Issue mapping tests ──────────────────────────────────────

    #[test]
    fn test_map_issue_open() {
        let issue = make_issue_fields(IssueFieldsOverrides {
            labels: make_issue_labels(&["bug", "p1"]),
            ..Default::default()
        });
        let result = map_issue(&issue).unwrap();

        assert_eq!(result.id, "ISSUE_99");
        assert_eq!(result.number, 99);
        assert_eq!(result.title, "Bug report");
        assert_eq!(result.author, "octocat");
        assert_eq!(result.state, crate::types::IssueState::Open);
        assert_eq!(result.labels, vec!["bug", "p1"]);
        assert_eq!(result.repo_id, "org/repo");
    }

    #[test]
    fn test_map_issue_closed() {
        let issue = make_issue_fields(IssueFieldsOverrides {
            state: Some(GqlIssueState::CLOSED),
            ..Default::default()
        });
        let result = map_issue(&issue).unwrap();
        assert_eq!(result.state, crate::types::IssueState::Closed);
    }

    #[test]
    fn test_map_issue_custom_author() {
        let issue = make_issue_fields(IssueFieldsOverrides {
            author: Some("assignee-user".to_string()),
            ..Default::default()
        });
        let result = map_issue(&issue).unwrap();
        assert_eq!(result.author, "assignee-user");
    }

    #[test]
    fn test_map_issue_negative_number_returns_error() {
        let issue = make_issue_fields(IssueFieldsOverrides {
            number: Some(-5),
            ..Default::default()
        });
        let err = map_issue(&issue).unwrap_err();
        assert!(err.to_string().contains("invalid issue number"));
    }

    // ── CI status mapping tests ──────────────────────────────────

    #[test]
    fn test_map_ci_status_success() {
        let commits = make_commits_with_status(StatusState::SUCCESS);
        let result = map_ci_status(commits.as_ref());
        assert_eq!(result, CiStatus::Success);
    }

    #[test]
    fn test_map_ci_status_failure() {
        let commits = make_commits_with_status(StatusState::FAILURE);
        let result = map_ci_status(commits.as_ref());
        assert_eq!(result, CiStatus::Failure);
    }

    #[test]
    fn test_map_ci_status_error() {
        let commits = make_commits_with_status(StatusState::ERROR);
        let result = map_ci_status(commits.as_ref());
        assert_eq!(result, CiStatus::Failure);
    }

    #[test]
    fn test_map_ci_status_pending() {
        let commits = make_commits_with_status(StatusState::PENDING);
        let result = map_ci_status(commits.as_ref());
        assert_eq!(result, CiStatus::Pending);
    }

    #[test]
    fn test_map_ci_status_null() {
        let result = map_ci_status(None);
        assert_eq!(result, CiStatus::Pending);
    }
}
