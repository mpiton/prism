#![allow(dead_code)] // TODO(T-034): remove after wiring up polling
//! GitHub data synchronization (T-032).
//!
//! Fetches dashboard data from GitHub GraphQL API and persists it
//! into the local `SQLite` cache using upsert semantics.

use std::collections::HashSet;

use sqlx::SqlitePool;

use crate::cache::activity::upsert_activity;
use crate::cache::dashboard::compute_dashboard_stats;
use crate::cache::issues::upsert_issue;
use crate::cache::pull_requests::upsert_pull_request;
use crate::cache::repos::list_repos;
use crate::cache::reviews::upsert_review;
use crate::error::AppError;
use crate::github::client::GitHubClient;
use crate::github::models::{map_issue, map_pr, map_review};
use crate::github::queries::dashboard_data::{self, IssueFields, PrFields};
use crate::github::queries::recent_activity;
use crate::github::queries::{DashboardData, RecentActivity};
use crate::types::{Activity, ActivityType, DashboardStats, Repo};

/// Synchronize dashboard data from GitHub API into the local cache.
///
/// 1. Read enabled repos from DB
/// 2. Build GitHub search query variables
/// 3. Execute GraphQL `DashboardData` query
/// 4. Persist all data atomically in a single transaction
/// 5. Update `last_sync_at` per repo (same transaction)
/// 6. Return dashboard stats
///
/// Note: the `first: 100` page size may truncate results for users with
/// very large dashboards. Pagination support is deferred to a future task
/// (requires adding `after` cursor variables to the GraphQL query).
///
/// Note: this sync only upserts data present in the current API response.
/// PRs that were merged/closed since the last sync drop out of `state:open`
/// searches but remain in the cache with their last known state. A full
/// reconciliation pass is deferred to a future task.
#[tracing::instrument(skip(client, pool, username))]
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

    // All DB writes in a single transaction for atomicity.
    let mut tx = pool.begin().await?;

    #[allow(clippy::explicit_auto_deref)] // &mut *tx required: Transaction → SqliteConnection
    persist_response(&mut *tx, &data).await?;

    let now = chrono::Utc::now().to_rfc3339();
    for repo in &enabled {
        #[allow(clippy::explicit_auto_deref)]
        sqlx::query("UPDATE repos SET last_sync_at = $1 WHERE id = $2")
            .bind(&now)
            .bind(&repo.id)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;

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
/// GitHub usernames: alphanumeric and hyphens only, max 39 chars.
fn validate_username(username: &str) -> Result<(), AppError> {
    let valid = !username.is_empty()
        && username.len() <= 39
        && username.chars().all(|c| c.is_alphanumeric() || c == '-');

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
    conn: &mut sqlx::SqliteConnection,
    data: &dashboard_data::ResponseData,
) -> Result<(), AppError> {
    let mut seen_pr_ids: HashSet<String> = HashSet::new();

    if let Some(nodes) = &data.review_requests.nodes {
        for node in nodes.iter().filter_map(|n| n.as_ref()) {
            if let Some(pr_fields) = node.as_pr_fields()
                && seen_pr_ids.insert(pr_fields.id.clone())
            {
                persist_single_pr(&mut *conn, pr_fields).await?;
            }
        }
    }

    if let Some(nodes) = &data.my_pull_requests.nodes {
        for node in nodes.iter().filter_map(|n| n.as_ref()) {
            if let Some(pr_fields) = node.as_pr_fields()
                && seen_pr_ids.insert(pr_fields.id.clone())
            {
                persist_single_pr(&mut *conn, pr_fields).await?;
            }
        }
    }

    if let Some(nodes) = &data.assigned_issues.nodes {
        for node in nodes.iter().filter_map(|n| n.as_ref()) {
            if let Some(issue_fields) = node.as_issue_fields() {
                let issue = map_issue(issue_fields)?;
                upsert_issue(&mut *conn, &issue).await?;
            }
        }
    }

    Ok(())
}

/// Persist a single PR with its associated reviews and review requests.
///
/// All writes use the provided connection (part of the caller's transaction)
/// to ensure atomicity: if any write fails, the entire sync rolls back.
pub(crate) async fn persist_single_pr(
    conn: &mut sqlx::SqliteConnection,
    pr_fields: &PrFields,
) -> Result<(), AppError> {
    let pr = map_pr(pr_fields)?;
    upsert_pull_request(&mut *conn, &pr).await?;

    // Upsert reviews
    if let Some(reviews_conn) = &pr_fields.reviews
        && let Some(nodes) = &reviews_conn.nodes
    {
        for node in nodes.iter().filter_map(|n| n.as_ref()) {
            let review = map_review(node, &pr.id);
            upsert_review(&mut *conn, &review).await?;
        }
    }

    // Delete then re-insert review requests to evict stale ones
    // (e.g. a reviewer was un-requested on GitHub).
    sqlx::query("DELETE FROM review_requests WHERE pull_request_id = $1")
        .bind(&pr.id)
        .execute(&mut *conn)
        .await?;

    if let Some(rr_conn) = &pr_fields.review_requests
        && let Some(nodes) = &rr_conn.nodes
    {
        for node in nodes.iter().filter_map(|n| n.as_ref()) {
            if let Some(reviewer) = extract_reviewer_login(node) {
                sqlx::query(
                    "INSERT INTO review_requests (id, pull_request_id, reviewer, status, requested_at) \
                     VALUES ($1, $2, $3, $4, $5)",
                )
                .bind(format!("{}-{}", pr.id, reviewer))
                .bind(&pr.id)
                .bind(&reviewer)
                .bind("pending")
                .bind(&pr.updated_at)
                .execute(&mut *conn)
                .await?;
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

// ── Activity sync (T-033) ──────────────────────────────────────

/// Validate that a `since` value contains only ISO 8601 safe characters.
///
/// Prevents search query injection via malicious `since` values.
/// Accepts date (`2026-03-01`) or datetime (`2026-03-01T10:00:00Z`),
/// including timezone offsets (`+05:30`) and fractional seconds (`.123`).
fn validate_since(since: &str) -> Result<(), AppError> {
    let bytes = since.as_bytes();
    let valid = since.len() >= 10
        && bytes.get(4) == Some(&b'-')
        && bytes.get(7) == Some(&b'-')
        && since
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | ':' | 'T' | 'Z' | '+' | '.'));

    if valid {
        Ok(())
    } else {
        Err(AppError::Config(format!("invalid since value: {since}")))
    }
}

/// Synchronize recent activity from GitHub API into the local cache.
///
/// 1. Build search query: `involves:{username} updated:>{since}`
/// 2. Execute GraphQL `RecentActivity` query
/// 3. Map PR and Issue nodes to `Activity` items
/// 4. Deduplicate by ID in memory
/// 5. Insert new items (skip already-known IDs via `INSERT OR IGNORE`)
/// 6. Return the count of newly inserted activities
///
/// Note: `pull_request_id` and `issue_id` FK fields on `Activity` are left
/// `None` because the referenced entities may not exist in the local cache
/// (e.g. mentions from repos not tracked by `sync_dashboard`). The entity
/// node ID is embedded in the deterministic `activity.id` for traceability.
pub async fn sync_activity(
    client: &GitHubClient,
    pool: &SqlitePool,
    username: &str,
    since: &str,
) -> Result<u32, AppError> {
    validate_username(username)?;
    validate_since(since)?;

    let variables = recent_activity::Variables {
        activity_query: format!("involves:{username} updated:>{since}"),
        first: 100,
    };

    let data = client.execute_graphql::<RecentActivity>(variables).await?;

    let activities = map_activity_nodes(&data);
    let inserted = persist_activity_batch(pool, &activities).await?;

    Ok(inserted)
}

/// Map all search nodes from a `RecentActivity` response to `Activity` items.
///
/// Skips unrecognized node types (e.g. future GitHub schema additions).
/// Activity type is derived from the item state (open/merged/closed).
/// The activity ID is deterministic and state-specific:
/// `activity-{pr|issue}-{node_id}-{state}` so that state transitions
/// (e.g. open → merged) produce distinct records in the activity feed.
fn map_activity_nodes(data: &recent_activity::ResponseData) -> Vec<Activity> {
    let mut activities = Vec::new();

    let Some(nodes) = &data.search.nodes else {
        return activities;
    };

    for node in nodes.iter().filter_map(|n| n.as_ref()) {
        match node {
            recent_activity::RecentActivitySearchNodes::PullRequest(pr) => {
                let (activity_type, state_tag) = match &pr.state {
                    recent_activity::PullRequestState::MERGED => (ActivityType::PrMerged, "merged"),
                    recent_activity::PullRequestState::CLOSED => (ActivityType::PrClosed, "closed"),
                    _ => (ActivityType::PrOpened, "open"),
                };
                let actor = pr
                    .author
                    .as_ref()
                    .map_or_else(|| "ghost".to_string(), |a| a.login.clone());

                activities.push(Activity {
                    id: format!("activity-pr-{}-{state_tag}", pr.id),
                    activity_type,
                    actor,
                    repo_id: pr.repository.name_with_owner.clone(),
                    pull_request_id: None,
                    issue_id: None,
                    message: format!("PR #{}: {}", pr.number, pr.title),
                    created_at: pr.updated_at.clone(),
                });
            }
            recent_activity::RecentActivitySearchNodes::Issue(issue) => {
                let (activity_type, state_tag) = match &issue.state {
                    recent_activity::IssueState::CLOSED => (ActivityType::IssueClosed, "closed"),
                    _ => (ActivityType::IssueOpened, "open"),
                };
                let actor = issue
                    .author
                    .as_ref()
                    .map_or_else(|| "ghost".to_string(), |a| a.login.clone());

                activities.push(Activity {
                    id: format!("activity-issue-{}-{state_tag}", issue.id),
                    activity_type,
                    actor,
                    repo_id: issue.repository.name_with_owner.clone(),
                    pull_request_id: None,
                    issue_id: None,
                    message: format!("Issue #{}: {}", issue.number, issue.title),
                    created_at: issue.updated_at.clone(),
                });
            }
            _ => {}
        }
    }

    activities
}

/// Persist a batch of `Activity` items, deduplicating by ID.
///
/// Uses a single transaction for atomicity and performance.
/// In-memory dedup via `HashSet` prevents redundant SQL round-trips
/// when the same node appears multiple times in the API response.
/// `INSERT OR IGNORE` skips activities already in the database.
///
/// Returns the count of newly inserted rows.
async fn persist_activity_batch(
    pool: &SqlitePool,
    activities: &[Activity],
) -> Result<u32, AppError> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut inserted: u32 = 0;
    let mut tx = pool.begin().await?;

    for activity in activities {
        #[allow(clippy::explicit_auto_deref)]
        if seen.insert(activity.id.clone()) && upsert_activity(&mut *tx, activity).await? {
            inserted += 1;
        }
    }

    tx.commit().await?;
    Ok(inserted)
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
        pool.close().await;
    }

    #[tokio::test]
    async fn test_sync_inserts_new_prs() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let pr = make_pr_fields("PR_1", 42, "Fix login bug");
        {
            let mut conn = pool.acquire().await.unwrap();
            persist_single_pr(&mut conn, &pr).await.unwrap();
        }

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

        {
            let mut conn = pool.acquire().await.unwrap();
            let pr = make_pr_fields("PR_1", 42, "Fix login bug");
            persist_single_pr(&mut conn, &pr).await.unwrap();
            let updated = make_pr_fields("PR_1", 42, "Fix login bug (v2)");
            persist_single_pr(&mut conn, &updated).await.unwrap();
        }

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

        {
            let mut conn = pool.acquire().await.unwrap();
            persist_single_pr(&mut conn, &pr).await.unwrap();
        }

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
        sqlx::query("UPDATE repos SET last_sync_at = $1 WHERE id = $2")
            .bind(now)
            .bind(&repo.id)
            .execute(&pool)
            .await
            .unwrap();

        let updated = crate::cache::repos::get_repo(&pool, &repo.id)
            .await
            .unwrap();
        assert_eq!(updated.last_sync_at.as_deref(), Some(now));
        pool.close().await;
    }

    #[tokio::test]
    async fn test_sync_handles_rate_limit() {
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

        {
            let mut conn = pool.acquire().await.unwrap();
            persist_response(&mut conn, &response).await.unwrap();
        }

        let result = get_pull_request(&pool, "PR_1").await.unwrap();
        assert_eq!(result.title, "Dedup PR");
        pool.close().await;
    }

    #[test]
    fn test_validate_username_valid() {
        assert!(validate_username("octocat").is_ok());
        assert!(validate_username("my-user").is_ok());
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
        assert!(
            validate_username("user_name").is_err(),
            "underscores not allowed in GitHub usernames"
        );
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
        {
            let mut conn = pool.acquire().await.unwrap();
            persist_single_pr(&mut conn, &pr).await.unwrap();
        }

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
        {
            let mut conn = pool.acquire().await.unwrap();
            persist_single_pr(&mut conn, &pr).await.unwrap();
        }

        let rrs = crate::cache::reviews::get_review_requests_by_pr(&pool, "PR_1")
            .await
            .unwrap();
        assert_eq!(rrs.len(), 1, "alice should be evicted");
        assert_eq!(rrs[0].reviewer, "bob");
        pool.close().await;
    }

    #[tokio::test]
    async fn test_sync_transaction_atomicity() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        // Persist via transaction (same path as sync_dashboard)
        let pr = make_pr_fields("PR_TX", 99, "Transaction test");
        let response = dashboard_data::ResponseData {
            review_requests: dashboard_data::DashboardDataReviewRequests {
                nodes: Some(vec![Some(
                    dashboard_data::DashboardDataReviewRequestsNodes::PullRequest(pr),
                )]),
            },
            my_pull_requests: dashboard_data::DashboardDataMyPullRequests { nodes: None },
            assigned_issues: dashboard_data::DashboardDataAssignedIssues { nodes: None },
        };

        let mut tx = pool.begin().await.unwrap();
        persist_response(&mut *tx, &response).await.unwrap();
        sqlx::query("UPDATE repos SET last_sync_at = $1 WHERE id = $2")
            .bind("2026-03-26T12:00:00Z")
            .bind("org/repo")
            .execute(&mut *tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();

        // Verify both PR and last_sync_at were persisted
        let result = get_pull_request(&pool, "PR_TX").await.unwrap();
        assert_eq!(result.title, "Transaction test");
        let repo = crate::cache::repos::get_repo(&pool, "org/repo")
            .await
            .unwrap();
        assert_eq!(repo.last_sync_at.as_deref(), Some("2026-03-26T12:00:00Z"));
        pool.close().await;
    }

    // ── Activity sync tests (T-033) ──────────────────────────────

    fn make_activity_pr_node(
        id: &str,
        number: i64,
        title: &str,
        state: recent_activity::PullRequestState,
    ) -> recent_activity::RecentActivitySearchNodes {
        recent_activity::RecentActivitySearchNodes::PullRequest(recent_activity::PrFields {
            id: id.to_string(),
            number,
            title: title.to_string(),
            author: Some(recent_activity::PrFieldsAuthor {
                login: "octocat".to_string(),
                on: recent_activity::PrFieldsAuthorOn::User,
            }),
            state,
            is_draft: false,
            url: format!("https://github.com/org/repo/pull/{number}"),
            created_at: "2026-03-20T10:00:00Z".to_string(),
            updated_at: "2026-03-25T15:00:00Z".to_string(),
            additions: 10,
            deletions: 5,
            head_ref_name: "fix/something".to_string(),
            repository: recent_activity::PrFieldsRepository {
                name_with_owner: "org/repo".to_string(),
            },
            labels: None,
            review_requests: None,
            reviews: None,
            commits: None,
        })
    }

    fn make_activity_issue_node(
        id: &str,
        number: i64,
        title: &str,
        state: recent_activity::IssueState,
    ) -> recent_activity::RecentActivitySearchNodes {
        recent_activity::RecentActivitySearchNodes::Issue(recent_activity::IssueFields {
            id: id.to_string(),
            number,
            title: title.to_string(),
            author: Some(recent_activity::IssueFieldsAuthor {
                login: "alice".to_string(),
                on: recent_activity::IssueFieldsAuthorOn::User,
            }),
            state,
            url: format!("https://github.com/org/repo/issues/{number}"),
            created_at: "2026-03-20T10:00:00Z".to_string(),
            updated_at: "2026-03-25T16:00:00Z".to_string(),
            repository: recent_activity::IssueFieldsRepository {
                name_with_owner: "org/repo".to_string(),
            },
            labels: None,
        })
    }

    fn make_activity_response(
        nodes: Vec<recent_activity::RecentActivitySearchNodes>,
    ) -> recent_activity::ResponseData {
        recent_activity::ResponseData {
            search: recent_activity::RecentActivitySearch {
                nodes: Some(nodes.into_iter().map(Some).collect()),
            },
        }
    }

    #[tokio::test]
    async fn test_sync_activity_inserts_mentions() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let response = make_activity_response(vec![
            make_activity_pr_node(
                "PR_A",
                10,
                "Fix auth",
                recent_activity::PullRequestState::OPEN,
            ),
            make_activity_issue_node(
                "ISSUE_B",
                20,
                "Bug in login",
                recent_activity::IssueState::OPEN,
            ),
        ]);

        let activities = map_activity_nodes(&response);
        assert_eq!(activities.len(), 2);

        let inserted = persist_activity_batch(&pool, &activities).await.unwrap();
        assert_eq!(inserted, 2);

        // Verify the PR activity (state-specific ID: open)
        let pr_act = crate::cache::activity::get_activity_by_id(&pool, "activity-pr-PR_A-open")
            .await
            .unwrap()
            .expect("PR activity should exist");
        assert_eq!(pr_act.activity_type, ActivityType::PrOpened);
        assert_eq!(pr_act.actor, "octocat");
        assert_eq!(pr_act.repo_id, "org/repo");
        assert_eq!(pr_act.pull_request_id, None);
        assert!(pr_act.message.contains("PR #10"));

        // Verify the Issue activity (state-specific ID: open)
        let issue_act =
            crate::cache::activity::get_activity_by_id(&pool, "activity-issue-ISSUE_B-open")
                .await
                .unwrap()
                .expect("Issue activity should exist");
        assert_eq!(issue_act.activity_type, ActivityType::IssueOpened);
        assert_eq!(issue_act.actor, "alice");
        assert_eq!(issue_act.issue_id, None);
        assert!(issue_act.message.contains("Issue #20"));

        pool.close().await;
    }

    #[tokio::test]
    async fn test_sync_activity_dedup() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        // Same PR appears twice in the response (can happen with search queries)
        let response = make_activity_response(vec![
            make_activity_pr_node(
                "PR_DUP",
                42,
                "Same PR",
                recent_activity::PullRequestState::OPEN,
            ),
            make_activity_pr_node(
                "PR_DUP",
                42,
                "Same PR",
                recent_activity::PullRequestState::OPEN,
            ),
        ]);

        let activities = map_activity_nodes(&response);
        // Both map to the same activity-pr-PR_DUP-open ID
        assert_eq!(activities.len(), 2);

        // persist_activity_batch deduplicates by ID
        let inserted = persist_activity_batch(&pool, &activities).await.unwrap();
        assert_eq!(inserted, 1, "duplicate should be skipped");

        // Second call: already in DB, INSERT OR IGNORE skips
        let inserted_again = persist_activity_batch(&pool, &activities).await.unwrap();
        assert_eq!(
            inserted_again, 0,
            "already-persisted items should be skipped"
        );

        pool.close().await;
    }

    #[tokio::test]
    async fn test_sync_activity_empty() {
        let (pool, _tmp) = test_pool().await;

        let response = recent_activity::ResponseData {
            search: recent_activity::RecentActivitySearch { nodes: None },
        };

        let activities = map_activity_nodes(&response);
        assert!(activities.is_empty());

        let inserted = persist_activity_batch(&pool, &activities).await.unwrap();
        assert_eq!(inserted, 0);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_sync_activity_state_transitions() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        // First sync: PR is open
        let response_open = make_activity_response(vec![make_activity_pr_node(
            "PR_X",
            99,
            "My PR",
            recent_activity::PullRequestState::OPEN,
        )]);
        let activities_open = map_activity_nodes(&response_open);
        let inserted = persist_activity_batch(&pool, &activities_open)
            .await
            .unwrap();
        assert_eq!(inserted, 1);

        // Second sync: same PR is now merged — should produce a NEW record
        let response_merged = make_activity_response(vec![make_activity_pr_node(
            "PR_X",
            99,
            "My PR",
            recent_activity::PullRequestState::MERGED,
        )]);
        let activities_merged = map_activity_nodes(&response_merged);
        let inserted = persist_activity_batch(&pool, &activities_merged)
            .await
            .unwrap();
        assert_eq!(
            inserted, 1,
            "merged state should produce a new activity record"
        );

        // Both records exist in DB
        let open = crate::cache::activity::get_activity_by_id(&pool, "activity-pr-PR_X-open")
            .await
            .unwrap();
        assert!(open.is_some());
        assert_eq!(open.unwrap().activity_type, ActivityType::PrOpened);

        let merged = crate::cache::activity::get_activity_by_id(&pool, "activity-pr-PR_X-merged")
            .await
            .unwrap();
        assert!(merged.is_some());
        assert_eq!(merged.unwrap().activity_type, ActivityType::PrMerged);

        pool.close().await;
    }

    #[test]
    fn test_validate_since_valid() {
        assert!(validate_since("2026-03-01").is_ok());
        assert!(validate_since("2026-03-01T10:00:00Z").is_ok());
        assert!(validate_since("2026-01-15T23:59:59Z").is_ok());
        // Timezone offsets
        assert!(validate_since("2026-03-01T10:00:00+05:30").is_ok());
        assert!(validate_since("2026-03-01T10:00:00-05:30").is_ok());
        // Fractional seconds
        assert!(validate_since("2026-03-01T10:00:00.123Z").is_ok());
    }

    #[test]
    fn test_validate_since_invalid() {
        assert!(validate_since("").is_err(), "empty");
        assert!(validate_since("short").is_err(), "too short");
        assert!(
            validate_since("2026-03-01 involves:evil").is_err(),
            "injection attempt"
        );
        assert!(
            validate_since("2026-03-01T10:00:00Z extra").is_err(),
            "space in value"
        );
    }
}
