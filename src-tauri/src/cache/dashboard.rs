use std::collections::HashMap;

use sqlx::SqlitePool;

use crate::error::AppError;
use crate::types::{
    DashboardData, PullRequest, PullRequestWithReview, Workspace, WorkspaceSummary,
};

use super::activity::get_recent_activity;
use super::issues::get_issues_for_author;
use super::pull_requests::{PR_COLS, PullRequestRow};
use super::reviews::{compute_review_summary, get_review_requests_for_user};
use super::workspaces::list_workspaces;

/// Assemble the full dashboard data for a given user.
///
/// Composes data from multiple cache tables into a single [`DashboardData`]:
/// 1. PRs where the user is a requested reviewer
/// 2. PRs authored by the user
/// 3. Issues authored by the user
/// 4. Recent activity (last 50 events)
/// 5. All workspaces
///
/// For each PR, joins the [`ReviewSummary`] and optional [`WorkspaceSummary`].
/// Computes `synced_at` as the most recent `last_sync_at` across all repos.
#[allow(dead_code)]
pub async fn assemble_dashboard_data(
    pool: &SqlitePool,
    username: &str,
) -> Result<DashboardData, AppError> {
    // 1. Fetch all workspaces once for lookup
    let all_workspaces = list_workspaces(pool, None).await?;
    let workspace_map = build_workspace_map(&all_workspaces);

    // 2. PRs where user is a requested reviewer (deduplicated)
    let review_reqs = get_review_requests_for_user(pool, username).await?;
    let review_pr_ids: Vec<String> = {
        let set: std::collections::HashSet<String> = review_reqs
            .iter()
            .map(|rr| rr.pull_request_id.clone())
            .collect();
        set.into_iter().collect()
    };
    let review_prs = fetch_prs_by_ids(pool, &review_pr_ids).await?;
    let review_requests = enrich_prs(pool, review_prs, &workspace_map).await?;

    // 3. PRs authored by the user
    let my_prs = fetch_prs_by_author(pool, username).await?;
    let my_pull_requests = enrich_prs(pool, my_prs, &workspace_map).await?;

    // 4. Issues authored by user
    let assigned_issues = get_issues_for_author(pool, username).await?;

    // 5. Recent activity
    let recent_activity = get_recent_activity(pool, 50, 0).await?;

    // 6. synced_at = most recent last_sync_at from repos
    let synced_at = get_latest_sync_at(pool).await?;

    Ok(DashboardData {
        review_requests,
        my_pull_requests,
        assigned_issues,
        recent_activity,
        workspaces: all_workspaces,
        synced_at,
    })
}

// ── Internal helpers ──────────────────────────────────────────────

/// Build a lookup map from `(repo_id, pr_number)` to `Workspace`.
fn build_workspace_map(workspaces: &[Workspace]) -> HashMap<(String, u32), Workspace> {
    workspaces
        .iter()
        .map(|ws| ((ws.repo_id.clone(), ws.pull_request_number), ws.clone()))
        .collect()
}

/// Fetch pull requests by a list of IDs.
async fn fetch_prs_by_ids(pool: &SqlitePool, ids: &[String]) -> Result<Vec<PullRequest>, AppError> {
    if ids.is_empty() {
        return Ok(vec![]);
    }

    let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("${i}")).collect();
    let sql = format!(
        "SELECT {PR_COLS} FROM pull_requests WHERE id IN ({}) ORDER BY updated_at DESC",
        placeholders.join(", ")
    );

    let mut query = sqlx::query_as::<_, PullRequestRow>(&sql);
    for id in ids {
        query = query.bind(id);
    }
    let rows = query.fetch_all(pool).await?;

    rows.into_iter().map(PullRequest::try_from).collect()
}

/// Fetch pull requests authored by a given user.
async fn fetch_prs_by_author(
    pool: &SqlitePool,
    author: &str,
) -> Result<Vec<PullRequest>, AppError> {
    let sql =
        format!("SELECT {PR_COLS} FROM pull_requests WHERE author = $1 ORDER BY updated_at DESC");
    let rows: Vec<PullRequestRow> = sqlx::query_as(&sql).bind(author).fetch_all(pool).await?;

    rows.into_iter().map(PullRequest::try_from).collect()
}

/// Enrich a list of PRs with review summaries and optional workspace summaries.
///
/// Uses per-PR queries for review summaries. Acceptable for personal dashboards
/// (typically < 50 PRs). For larger scale, batch `review_requests` with a single
/// `WHERE pull_request_id IN (...)` query.
async fn enrich_prs(
    pool: &SqlitePool,
    prs: Vec<PullRequest>,
    workspace_map: &HashMap<(String, u32), Workspace>,
) -> Result<Vec<PullRequestWithReview>, AppError> {
    let mut result = Vec::with_capacity(prs.len());

    for pr in prs {
        let review_summary = compute_review_summary(pool, &pr.id).await?;
        let workspace = match workspace_map.get(&(pr.repo_id.clone(), pr.number)) {
            Some(ws) => Some(build_workspace_summary(pool, ws).await?),
            None => None,
        };

        result.push(PullRequestWithReview {
            pull_request: pr,
            review_summary,
            workspace,
        });
    }

    Ok(result)
}

/// Build a [`WorkspaceSummary`] from a [`Workspace`], fetching only the latest note.
async fn build_workspace_summary(
    pool: &SqlitePool,
    workspace: &Workspace,
) -> Result<WorkspaceSummary, AppError> {
    let last_note_content: Option<String> = sqlx::query_scalar(
        "SELECT content FROM workspace_notes WHERE workspace_id = $1 \
         ORDER BY created_at DESC, id DESC LIMIT 1",
    )
    .bind(&workspace.id)
    .fetch_optional(pool)
    .await?;

    Ok(WorkspaceSummary {
        id: workspace.id.clone(),
        state: workspace.state.clone(),
        last_note_content,
    })
}

/// Get the most recent `last_sync_at` across all repos.
async fn get_latest_sync_at(pool: &SqlitePool) -> Result<Option<String>, AppError> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT MAX(last_sync_at) FROM repos WHERE last_sync_at IS NOT NULL")
            .fetch_optional(pool)
            .await?;

    Ok(row.and_then(|(ts,)| ts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::db::init_db;
    use crate::cache::pull_requests::upsert_pull_request;
    use crate::cache::repos::upsert_repo;
    use crate::cache::reviews::upsert_review_request;
    use crate::cache::workspaces::{add_note, create_workspace};
    use crate::types::{
        CiStatus, PrState, Priority, Repo, ReviewStatus, WorkspaceNote, WorkspaceState,
    };

    async fn test_pool() -> (SqlitePool, tempfile::TempDir) {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = init_db(&tmp.path().join("test.db")).await.unwrap();
        (pool, tmp)
    }

    fn sample_repo() -> Repo {
        Repo {
            id: "repo-1".to_string(),
            org: "mpiton".to_string(),
            name: "prism".to_string(),
            full_name: "mpiton/prism".to_string(),
            url: "https://github.com/mpiton/prism".to_string(),
            default_branch: "main".to_string(),
            is_archived: false,
            enabled: true,
            local_path: None,
            last_sync_at: None,
        }
    }

    fn sample_pr(id: &str, number: u32, author: &str) -> PullRequest {
        PullRequest {
            id: id.to_string(),
            number,
            title: format!("PR #{number}"),
            author: author.to_string(),
            state: PrState::Open,
            ci_status: CiStatus::Success,
            priority: Priority::High,
            repo_id: "repo-1".to_string(),
            url: format!("https://github.com/mpiton/prism/pull/{number}"),
            labels: vec![],
            created_at: "2026-03-01T10:00:00Z".to_string(),
            updated_at: "2026-03-20T15:00:00Z".to_string(),
        }
    }

    fn sample_review_request(id: &str, pr_id: &str, reviewer: &str) -> crate::types::ReviewRequest {
        crate::types::ReviewRequest {
            id: id.to_string(),
            pull_request_id: pr_id.to_string(),
            reviewer: reviewer.to_string(),
            status: ReviewStatus::Pending,
            requested_at: "2026-03-20T10:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn test_assemble_dashboard_empty() {
        let (pool, _tmp) = test_pool().await;

        let data = assemble_dashboard_data(&pool, "alice").await.unwrap();

        assert!(data.review_requests.is_empty());
        assert!(data.my_pull_requests.is_empty());
        assert!(data.assigned_issues.is_empty());
        assert!(data.recent_activity.is_empty());
        assert!(data.workspaces.is_empty());
        assert!(data.synced_at.is_none());
    }

    #[tokio::test]
    async fn test_assemble_dashboard_with_prs() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        // PR authored by alice
        let pr_alice = sample_pr("pr-1", 1, "alice");
        upsert_pull_request(&pool, &pr_alice).await.unwrap();

        // PR authored by bob, review requested from alice
        let pr_bob = sample_pr("pr-2", 2, "bob");
        upsert_pull_request(&pool, &pr_bob).await.unwrap();
        let rr = sample_review_request("rr-1", "pr-2", "alice");
        upsert_review_request(&pool, &rr).await.unwrap();

        // PR authored by charlie — should not appear
        let pr_charlie = sample_pr("pr-3", 3, "charlie");
        upsert_pull_request(&pool, &pr_charlie).await.unwrap();

        let data = assemble_dashboard_data(&pool, "alice").await.unwrap();

        // review_requests: bob's PR where alice is reviewer
        assert_eq!(data.review_requests.len(), 1);
        assert_eq!(data.review_requests[0].pull_request.id, "pr-2");

        // my_pull_requests: alice's PR
        assert_eq!(data.my_pull_requests.len(), 1);
        assert_eq!(data.my_pull_requests[0].pull_request.id, "pr-1");

        // charlie's PR should not appear in either
        let all_pr_ids: Vec<&str> = data
            .review_requests
            .iter()
            .chain(data.my_pull_requests.iter())
            .map(|p| p.pull_request.id.as_str())
            .collect();
        assert!(!all_pr_ids.contains(&"pr-3"));
    }

    #[tokio::test]
    async fn test_assemble_dashboard_with_workspace_summary() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        // PR authored by alice with an active workspace
        let pr = sample_pr("pr-1", 1, "alice");
        upsert_pull_request(&pool, &pr).await.unwrap();

        let ws = Workspace {
            id: "ws-1".to_string(),
            repo_id: "repo-1".to_string(),
            pull_request_number: 1,
            state: WorkspaceState::Active,
            worktree_path: Some("/tmp/ws".to_string()),
            session_id: None,
            created_at: "2026-03-20T10:00:00Z".to_string(),
            updated_at: "2026-03-20T10:00:00Z".to_string(),
        };
        create_workspace(&pool, &ws).await.unwrap();

        let note = WorkspaceNote {
            id: "note-1".to_string(),
            workspace_id: "ws-1".to_string(),
            content: "Review in progress".to_string(),
            created_at: "2026-03-20T11:00:00Z".to_string(),
        };
        add_note(&pool, &note).await.unwrap();

        let data = assemble_dashboard_data(&pool, "alice").await.unwrap();

        assert_eq!(data.my_pull_requests.len(), 1);
        let pr_with_review = &data.my_pull_requests[0];
        assert!(pr_with_review.workspace.is_some());

        let ws_summary = pr_with_review.workspace.as_ref().unwrap();
        assert_eq!(ws_summary.id, "ws-1");
        assert_eq!(ws_summary.state, WorkspaceState::Active);
        assert_eq!(
            ws_summary.last_note_content.as_deref(),
            Some("Review in progress")
        );

        // Workspace also appears in the workspaces list
        assert_eq!(data.workspaces.len(), 1);
        assert_eq!(data.workspaces[0].id, "ws-1");
    }

    #[tokio::test]
    async fn test_assemble_dashboard_review_summary_join() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        // PR with multiple review requests
        let pr = sample_pr("pr-1", 1, "alice");
        upsert_pull_request(&pool, &pr).await.unwrap();

        // bob approved, charlie has changes requested
        let rr_bob = crate::types::ReviewRequest {
            id: "rr-1".to_string(),
            pull_request_id: "pr-1".to_string(),
            reviewer: "bob".to_string(),
            status: ReviewStatus::Approved,
            requested_at: "2026-03-20T10:00:00Z".to_string(),
        };
        let rr_charlie = crate::types::ReviewRequest {
            id: "rr-2".to_string(),
            pull_request_id: "pr-1".to_string(),
            reviewer: "charlie".to_string(),
            status: ReviewStatus::ChangesRequested,
            requested_at: "2026-03-20T10:00:00Z".to_string(),
        };
        upsert_review_request(&pool, &rr_bob).await.unwrap();
        upsert_review_request(&pool, &rr_charlie).await.unwrap();

        let data = assemble_dashboard_data(&pool, "alice").await.unwrap();

        // PR should be in my_pull_requests with correct review summary
        assert_eq!(data.my_pull_requests.len(), 1);
        let summary = &data.my_pull_requests[0].review_summary;
        assert_eq!(summary.total_reviews, 2);
        assert_eq!(summary.approved, 1);
        assert_eq!(summary.changes_requested, 1);
        assert_eq!(summary.pending, 0);
        assert_eq!(summary.reviewers.len(), 2);
        assert!(summary.reviewers.contains(&"bob".to_string()));
        assert!(summary.reviewers.contains(&"charlie".to_string()));
    }
}
