#![allow(dead_code)] // TODO(T-034): remove after wiring up polling
//! GitHub data synchronization (T-032).
//!
//! Fetches dashboard data from GitHub GraphQL API and persists it
//! into the local `SQLite` cache using upsert semantics.

use std::collections::HashSet;

use sqlx::SqlitePool;

use crate::cache::dashboard::compute_dashboard_stats;
use crate::cache::issues::upsert_issue;
use crate::cache::pull_requests::upsert_pull_request;
use crate::cache::repos::{list_repos, update_last_sync};
use crate::cache::reviews::{delete_review_requests_for_pr, upsert_review, upsert_review_request};
use crate::error::AppError;
use crate::github::client::GitHubClient;
use crate::github::models::{map_issue, map_pr, map_review};
use crate::github::queries::DashboardData;
use crate::github::queries::dashboard_data::{self, IssueFields, PrFields};
use crate::types::{DashboardStats, Repo, ReviewRequest, ReviewStatus};

/// Synchronize dashboard data from GitHub API into the local cache.
///
/// 1. Read enabled repos from DB
/// 2. Build GitHub search query variables
/// 3. Execute GraphQL `DashboardData` query
/// 4. Upsert PRs (deduplicated), reviews, review requests, and issues
/// 5. Update `last_sync_at` per repo
/// 6. Return dashboard stats
pub async fn sync_dashboard(
    client: &GitHubClient,
    pool: &SqlitePool,
    username: &str,
) -> Result<DashboardStats, AppError> {
    let repos = list_repos(pool).await?;
    let enabled: Vec<&Repo> = repos.iter().filter(|r| r.enabled).collect();

    if enabled.is_empty() {
        return compute_dashboard_stats(pool, username).await;
    }

    let variables = build_query_variables(username, &enabled)?;
    let data = client.execute_graphql::<DashboardData>(variables).await?;

    persist_response(pool, &data).await?;

    // Update last_sync_at for all synced repos.
    // If a repo is concurrently deleted between list_repos() and here,
    // update_last_sync returns NotFound and we abort — partial timestamp
    // update is worse than no update.
    let now = chrono::Utc::now().to_rfc3339();
    for repo in &enabled {
        update_last_sync(pool, &repo.id, &now).await?;
    }

    compute_dashboard_stats(pool, username).await
}

/// Validate that a repo `full_name` matches the `owner/repo` pattern.
///
/// Prevents search query injection via malicious `full_name` values.
fn validate_full_name(full_name: &str) -> Result<(), AppError> {
    let valid = full_name.split_once('/').is_some_and(|(owner, repo)| {
        !owner.is_empty()
            && !repo.is_empty()
            && owner
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
            && repo
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
    });

    if valid {
        Ok(())
    } else {
        Err(AppError::Config(format!(
            "invalid repo full_name: {full_name}"
        )))
    }
}

/// Validate that a GitHub username contains only safe characters.
///
/// Prevents search query injection via malicious username values.
/// GitHub usernames: alphanumeric and hyphens, max 39 chars.
fn validate_username(username: &str) -> Result<(), AppError> {
    let valid = !username.is_empty()
        && username.len() <= 39
        && username
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_');

    if valid {
        Ok(())
    } else {
        Err(AppError::Config(format!(
            "invalid GitHub username: {username}"
        )))
    }
}

/// Build the GraphQL search variables from username and active repos.
///
/// Validates both `username` and each repo's `full_name` before
/// interpolating into the search query string.
fn build_query_variables(
    username: &str,
    repos: &[&Repo],
) -> Result<dashboard_data::Variables, AppError> {
    validate_username(username)?;
    for repo in repos {
        validate_full_name(&repo.full_name)?;
    }

    let repo_filter: String = repos
        .iter()
        .map(|r| format!("repo:{}", r.full_name))
        .collect::<Vec<_>>()
        .join(" ");

    Ok(dashboard_data::Variables {
        review_query: format!("type:pr {repo_filter} review-requested:{username} state:open"),
        my_prs_query: format!("type:pr {repo_filter} author:{username} state:open"),
        issues_query: format!("type:issue {repo_filter} author:{username} state:open"),
        first: 100,
    })
}

/// Persist the GraphQL response into the local cache.
///
/// Extracts PRs from `review_requests` and `my_pull_requests` search results,
/// deduplicates by PR ID, then upserts all entities including reviews
/// and review requests. Issues are extracted from `assigned_issues`.
async fn persist_response(
    pool: &SqlitePool,
    data: &dashboard_data::ResponseData,
) -> Result<(), AppError> {
    // Collect all PR fields from both search results, deduplicated by ID
    let mut seen_pr_ids: HashSet<String> = HashSet::new();

    if let Some(nodes) = &data.review_requests.nodes {
        for node in nodes.iter().filter_map(|n| n.as_ref()) {
            if let Some(pr_fields) = node.as_pr_fields()
                && seen_pr_ids.insert(pr_fields.id.clone())
            {
                persist_single_pr(pool, pr_fields).await?;
            }
        }
    }

    if let Some(nodes) = &data.my_pull_requests.nodes {
        for node in nodes.iter().filter_map(|n| n.as_ref()) {
            if let Some(pr_fields) = node.as_pr_fields()
                && seen_pr_ids.insert(pr_fields.id.clone())
            {
                persist_single_pr(pool, pr_fields).await?;
            }
        }
    }

    // Extract and persist issues from assigned_issues search
    if let Some(nodes) = &data.assigned_issues.nodes {
        for node in nodes.iter().filter_map(|n| n.as_ref()) {
            if let Some(issue_fields) = node.as_issue_fields() {
                let issue = map_issue(issue_fields)?;
                upsert_issue(pool, &issue).await?;
            }
        }
    }

    Ok(())
}

/// Persist a single PR with its associated reviews and review requests.
pub(crate) async fn persist_single_pr(
    pool: &SqlitePool,
    pr_fields: &PrFields,
) -> Result<(), AppError> {
    let pr = map_pr(pr_fields)?;
    upsert_pull_request(pool, &pr).await?;

    // Upsert reviews from the PR's reviews connection
    if let Some(reviews_conn) = &pr_fields.reviews
        && let Some(nodes) = &reviews_conn.nodes
    {
        for node in nodes.iter().filter_map(|n| n.as_ref()) {
            let review = map_review(node, &pr.id);
            upsert_review(pool, &review).await?;
        }
    }

    // Delete then re-insert review requests to evict stale ones
    // (e.g. a reviewer was un-requested on GitHub).
    delete_review_requests_for_pr(pool, &pr.id).await?;
    if let Some(rr_conn) = &pr_fields.review_requests
        && let Some(nodes) = &rr_conn.nodes
    {
        for node in nodes.iter().filter_map(|n| n.as_ref()) {
            if let Some(reviewer) = extract_reviewer_login(node) {
                let rr = ReviewRequest {
                    id: format!("{}-{}", pr.id, reviewer),
                    pull_request_id: pr.id.clone(),
                    reviewer,
                    status: ReviewStatus::Pending,
                    requested_at: pr.updated_at.clone(),
                };
                upsert_review_request(pool, &rr).await?;
            }
        }
    }

    Ok(())
}

/// Extract the reviewer login from a review request node.
///
/// Returns `None` if the requested reviewer is a bot or unknown type.
fn extract_reviewer_login(node: &dashboard_data::PrFieldsReviewRequestsNodes) -> Option<String> {
    let reviewer = node.requested_reviewer.as_ref()?;
    match reviewer {
        dashboard_data::PrFieldsReviewRequestsNodesRequestedReviewer::User(u) => {
            Some(u.login.clone())
        }
        dashboard_data::PrFieldsReviewRequestsNodesRequestedReviewer::Team(t) => {
            Some(t.name.clone())
        }
        _ => None,
    }
}

/// Trait for extracting `PrFields` from search result nodes.
///
/// Implemented for each search result node type to provide uniform extraction
/// despite `graphql_client` generating separate types per aliased field.
trait AsPrFields {
    fn as_pr_fields(&self) -> Option<&PrFields>;
}

/// Trait for extracting `IssueFields` from search result nodes.
trait AsIssueFields {
    fn as_issue_fields(&self) -> Option<&IssueFields>;
}

impl AsPrFields for dashboard_data::DashboardDataReviewRequestsNodes {
    fn as_pr_fields(&self) -> Option<&PrFields> {
        match self {
            dashboard_data::DashboardDataReviewRequestsNodes::PullRequest(pr) => Some(pr),
            _ => None,
        }
    }
}

impl AsPrFields for dashboard_data::DashboardDataMyPullRequestsNodes {
    fn as_pr_fields(&self) -> Option<&PrFields> {
        match self {
            dashboard_data::DashboardDataMyPullRequestsNodes::PullRequest(pr) => Some(pr),
            _ => None,
        }
    }
}

impl AsIssueFields for dashboard_data::DashboardDataAssignedIssuesNodes {
    fn as_issue_fields(&self) -> Option<&IssueFields> {
        match self {
            dashboard_data::DashboardDataAssignedIssuesNodes::Issue(issue) => Some(issue),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::db::init_db;
    use crate::cache::issues::get_issues_by_repo;
    use crate::cache::pull_requests::get_pull_request;
    use crate::cache::repos::upsert_repo;
    use crate::cache::reviews::get_reviews_by_pr;
    use crate::github::queries::dashboard_data::{
        IssueFieldsAuthor, IssueFieldsAuthorOn, IssueFieldsRepository, IssueState as GqlIssueState,
        PrFieldsAuthor, PrFieldsAuthorOn, PrFieldsRepository, PrFieldsReviews,
        PrFieldsReviewsNodes, PrFieldsReviewsNodesAuthor, PrFieldsReviewsNodesAuthorOn,
        PullRequestReviewState, PullRequestState,
    };

    async fn test_pool() -> (SqlitePool, tempfile::TempDir) {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = init_db(&tmp.path().join("test.db")).await.unwrap();
        (pool, tmp)
    }

    fn sample_repo() -> Repo {
        Repo {
            id: "org/repo".to_string(),
            org: "org".to_string(),
            name: "repo".to_string(),
            full_name: "org/repo".to_string(),
            url: "https://github.com/org/repo".to_string(),
            default_branch: "main".to_string(),
            is_archived: false,
            enabled: true,
            local_path: None,
            last_sync_at: None,
        }
    }

    fn make_pr_fields(id: &str, number: i64, title: &str) -> PrFields {
        PrFields {
            id: id.to_string(),
            number,
            title: title.to_string(),
            author: Some(PrFieldsAuthor {
                login: "octocat".to_string(),
                on: PrFieldsAuthorOn::User,
            }),
            state: PullRequestState::OPEN,
            is_draft: false,
            url: format!("https://github.com/org/repo/pull/{number}"),
            created_at: "2026-03-01T10:00:00Z".to_string(),
            updated_at: "2026-03-20T15:00:00Z".to_string(),
            additions: 50,
            deletions: 10,
            head_ref_name: "fix/bug".to_string(),
            repository: PrFieldsRepository {
                name_with_owner: "org/repo".to_string(),
            },
            labels: None,
            review_requests: None,
            reviews: None,
            commits: None,
        }
    }

    fn make_issue_fields(id: &str, number: i64, title: &str) -> IssueFields {
        IssueFields {
            id: id.to_string(),
            number,
            title: title.to_string(),
            author: Some(IssueFieldsAuthor {
                login: "octocat".to_string(),
                on: IssueFieldsAuthorOn::User,
            }),
            state: GqlIssueState::OPEN,
            url: format!("https://github.com/org/repo/issues/{number}"),
            created_at: "2026-03-01T10:00:00Z".to_string(),
            updated_at: "2026-03-20T15:00:00Z".to_string(),
            repository: IssueFieldsRepository {
                name_with_owner: "org/repo".to_string(),
            },
            labels: None,
        }
    }

    fn make_review_node(
        id: &str,
        reviewer: &str,
        state: PullRequestReviewState,
    ) -> PrFieldsReviewsNodes {
        PrFieldsReviewsNodes {
            id: id.to_string(),
            author: Some(PrFieldsReviewsNodesAuthor {
                login: reviewer.to_string(),
                on: PrFieldsReviewsNodesAuthorOn::User,
            }),
            state,
            created_at: "2026-03-20T14:00:00Z".to_string(),
            submitted_at: Some("2026-03-20T14:05:00Z".to_string()),
        }
    }

    // ── Tests ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_sync_empty_repos() {
        let (pool, _tmp) = test_pool().await;

        let stats = compute_dashboard_stats(&pool, "alice").await.unwrap();

        assert_eq!(stats.pending_reviews, 0);
        assert_eq!(stats.open_prs, 0);
        assert_eq!(stats.open_issues, 0);
        assert_eq!(stats.active_workspaces, 0);
        assert_eq!(stats.unread_activity, 0);
        pool.close().await;
    }

    #[tokio::test]
    async fn test_sync_inserts_new_prs() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let pr = make_pr_fields("PR_1", 42, "Fix login bug");
        persist_single_pr(&pool, &pr).await.unwrap();

        let result = get_pull_request(&pool, "PR_1").await.unwrap();
        assert_eq!(result.title, "Fix login bug");
        assert_eq!(result.number, 42);
        assert_eq!(result.author, "octocat");
        assert_eq!(result.repo_id, "org/repo");
        pool.close().await;
    }

    #[tokio::test]
    async fn test_sync_updates_existing_pr() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let pr = make_pr_fields("PR_1", 42, "Fix login bug");
        persist_single_pr(&pool, &pr).await.unwrap();

        let updated = make_pr_fields("PR_1", 42, "Fix login bug (v2)");
        persist_single_pr(&pool, &updated).await.unwrap();

        let result = get_pull_request(&pool, "PR_1").await.unwrap();
        assert_eq!(result.title, "Fix login bug (v2)");
        pool.close().await;
    }

    #[tokio::test]
    async fn test_sync_inserts_reviews() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let mut pr = make_pr_fields("PR_1", 42, "PR with reviews");
        pr.reviews = Some(PrFieldsReviews {
            nodes: Some(vec![
                Some(make_review_node(
                    "REV_1",
                    "alice",
                    PullRequestReviewState::APPROVED,
                )),
                Some(make_review_node(
                    "REV_2",
                    "bob",
                    PullRequestReviewState::CHANGES_REQUESTED,
                )),
            ]),
        });

        persist_single_pr(&pool, &pr).await.unwrap();

        let reviews = get_reviews_by_pr(&pool, "PR_1").await.unwrap();
        assert_eq!(reviews.len(), 2);
        assert_eq!(reviews[0].reviewer, "alice");
        assert_eq!(reviews[1].reviewer, "bob");
        pool.close().await;
    }

    #[tokio::test]
    async fn test_sync_inserts_issues() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let issue = make_issue_fields("ISSUE_1", 99, "Bug report");
        let mapped = map_issue(&issue).unwrap();
        upsert_issue(&pool, &mapped).await.unwrap();

        let issues = get_issues_by_repo(&pool, "org/repo").await.unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].title, "Bug report");
        assert_eq!(issues[0].number, 99);
        pool.close().await;
    }

    #[tokio::test]
    async fn test_sync_updates_last_sync() {
        let (pool, _tmp) = test_pool().await;
        let repo = sample_repo();
        upsert_repo(&pool, &repo).await.unwrap();

        let now = "2026-03-26T10:00:00Z";
        update_last_sync(&pool, &repo.id, now).await.unwrap();

        let updated = crate::cache::repos::get_repo(&pool, &repo.id)
            .await
            .unwrap();
        assert_eq!(updated.last_sync_at.as_deref(), Some(now));
        pool.close().await;
    }

    #[tokio::test]
    async fn test_sync_handles_rate_limit() {
        // Use mockito to verify that sync_dashboard propagates RateLimit
        // errors from the GitHub client.
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/graphql")
            .with_status(403)
            .with_header("X-RateLimit-Remaining", "0")
            .with_header("X-RateLimit-Reset", "1700000000")
            .with_body(r#"{"message": "API rate limit exceeded"}"#)
            .create_async()
            .await;

        let (pool, _tmp) = test_pool().await;
        let repo = sample_repo();
        upsert_repo(&pool, &repo).await.unwrap();

        let client =
            GitHubClient::with_url("ghp_test_token", format!("{}/graphql", server.url())).unwrap();

        let result = sync_dashboard(&client, &pool, "octocat").await;

        let err = result.unwrap_err();
        assert!(
            matches!(err, AppError::RateLimit { .. }),
            "expected RateLimit error, got {err:?}"
        );
        mock.assert_async().await;
        pool.close().await;
    }

    #[tokio::test]
    async fn test_build_query_variables() {
        let repo = sample_repo();
        let repos = vec![&repo];
        let vars = build_query_variables("octocat", &repos).unwrap();

        assert!(vars.review_query.contains("review-requested:octocat"));
        assert!(vars.review_query.contains("repo:org/repo"));
        assert!(vars.my_prs_query.contains("author:octocat"));
        assert!(vars.issues_query.contains("author:octocat"));
        assert_eq!(vars.first, 100);
    }

    #[tokio::test]
    async fn test_build_query_variables_multiple_repos() {
        let repo1 = Repo {
            id: "r1".to_string(),
            full_name: "org/alpha".to_string(),
            ..sample_repo()
        };
        let repo2 = Repo {
            id: "r2".to_string(),
            full_name: "org/beta".to_string(),
            ..sample_repo()
        };
        let repos = vec![&repo1, &repo2];
        let vars = build_query_variables("user", &repos).unwrap();

        assert!(vars.review_query.contains("repo:org/alpha"));
        assert!(vars.review_query.contains("repo:org/beta"));
    }

    #[test]
    fn test_validate_full_name_valid() {
        assert!(validate_full_name("org/repo").is_ok());
        assert!(validate_full_name("my-org/my-repo").is_ok());
        assert!(validate_full_name("user_name/repo.js").is_ok());
    }

    #[test]
    fn test_validate_full_name_invalid() {
        assert!(validate_full_name("").is_err());
        assert!(validate_full_name("noslash").is_err());
        assert!(validate_full_name("/repo").is_err());
        assert!(validate_full_name("org/").is_err());
        assert!(validate_full_name("org/repo extra-token").is_err());
        assert!(validate_full_name("org/repo review-requested:attacker").is_err());
    }

    #[tokio::test]
    async fn test_sync_deduplicates_prs_via_persist_response() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        // Same PR appears in both review_requests and my_pull_requests
        let pr = make_pr_fields("PR_1", 42, "Dedup PR");
        let response = dashboard_data::ResponseData {
            review_requests: dashboard_data::DashboardDataReviewRequests {
                nodes: Some(vec![Some(
                    dashboard_data::DashboardDataReviewRequestsNodes::PullRequest(pr.clone()),
                )]),
            },
            my_pull_requests: dashboard_data::DashboardDataMyPullRequests {
                nodes: Some(vec![Some(
                    dashboard_data::DashboardDataMyPullRequestsNodes::PullRequest(pr),
                )]),
            },
            assigned_issues: dashboard_data::DashboardDataAssignedIssues { nodes: None },
        };

        persist_response(&pool, &response).await.unwrap();

        // PR should exist exactly once
        let result = get_pull_request(&pool, "PR_1").await.unwrap();
        assert_eq!(result.title, "Dedup PR");
        pool.close().await;
    }

    #[test]
    fn test_validate_username_valid() {
        assert!(validate_username("octocat").is_ok());
        assert!(validate_username("my-user").is_ok());
        assert!(validate_username("user_name").is_ok());
        assert!(validate_username("a").is_ok());
    }

    #[test]
    fn test_validate_username_invalid() {
        assert!(validate_username("").is_err());
        assert!(
            validate_username("user name").is_err(),
            "spaces not allowed"
        );
        assert!(
            validate_username("alice state:closed").is_err(),
            "injection attempt"
        );
        assert!(validate_username(&"a".repeat(40)).is_err(), "too long");
    }

    #[tokio::test]
    async fn test_sync_evicts_stale_review_requests() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        // First sync: PR has review request from alice
        let mut pr = make_pr_fields("PR_1", 42, "PR with reviewer");
        pr.review_requests = Some(dashboard_data::PrFieldsReviewRequests {
            total_count: 1,
            nodes: Some(vec![Some(dashboard_data::PrFieldsReviewRequestsNodes {
                requested_reviewer: Some(
                    dashboard_data::PrFieldsReviewRequestsNodesRequestedReviewer::User(
                        dashboard_data::PrFieldsReviewRequestsNodesRequestedReviewerOnUser {
                            login: "alice".to_string(),
                        },
                    ),
                ),
            })]),
        });
        persist_single_pr(&pool, &pr).await.unwrap();

        let rrs = crate::cache::reviews::get_review_requests_by_pr(&pool, "PR_1")
            .await
            .unwrap();
        assert_eq!(rrs.len(), 1);
        assert_eq!(rrs[0].reviewer, "alice");

        // Second sync: alice was un-requested, now only bob
        pr.review_requests = Some(dashboard_data::PrFieldsReviewRequests {
            total_count: 1,
            nodes: Some(vec![Some(dashboard_data::PrFieldsReviewRequestsNodes {
                requested_reviewer: Some(
                    dashboard_data::PrFieldsReviewRequestsNodesRequestedReviewer::User(
                        dashboard_data::PrFieldsReviewRequestsNodesRequestedReviewerOnUser {
                            login: "bob".to_string(),
                        },
                    ),
                ),
            })]),
        });
        persist_single_pr(&pool, &pr).await.unwrap();

        let rrs = crate::cache::reviews::get_review_requests_by_pr(&pool, "PR_1")
            .await
            .unwrap();
        assert_eq!(rrs.len(), 1, "alice should be evicted");
        assert_eq!(rrs[0].reviewer, "bob");
        pool.close().await;
    }
}
