use std::collections::HashMap;

use sqlx::SqlitePool;

use crate::error::AppError;
use crate::types::{
    DashboardData, DashboardStats, PullRequest, PullRequestWithReview, ReviewStatus, Workspace,
    WorkspaceState, WorkspaceSummary,
};

use super::activity::get_recent_activity;
use super::issues::get_issues_for_author;
use super::pull_requests::{PR_COLS, PullRequestRow};
use super::reviews::{compute_review_summary, get_review_requests_for_user};
use super::workspaces::list_workspaces;

/// Assemble the full dashboard data for a given user.
///
/// Composes data from multiple cache tables into a single [`DashboardData`]:
/// 1. PRs where the user has a **pending** review request
/// 2. PRs authored by the user
/// 3. Issues authored by the user
/// 4. Recent activity (last 50 events)
/// 5. All workspaces
///
/// For each PR, joins the [`ReviewSummary`] and optional [`WorkspaceSummary`].
/// Computes `synced_at` as the most recent `last_sync_at` across all repos.
pub async fn assemble_dashboard_data(
    pool: &SqlitePool,
    username: &str,
) -> Result<DashboardData, AppError> {
    // 1. Fetch all workspaces once for lookup (Active preferred over Suspended/Archived)
    let all_workspaces = list_workspaces(pool, None).await?;
    let workspace_map = build_workspace_map(&all_workspaces);

    // 2. PRs where user has a pending review request (filter out completed reviews)
    let review_reqs = get_review_requests_for_user(pool, username).await?;
    let review_pr_ids: Vec<String> = {
        let set: std::collections::HashSet<String> = review_reqs
            .iter()
            .filter(|rr| rr.status == ReviewStatus::Pending)
            .map(|rr| rr.pull_request_id.clone())
            .collect();
        set.into_iter().collect()
    };
    let review_prs = fetch_prs_by_ids(pool, &review_pr_ids).await?;
    let review_requests = enrich_prs(pool, review_prs, &workspace_map).await?;

    // 3. PRs authored by the user
    let my_prs = fetch_prs_by_author(pool, username).await?;
    let my_pull_requests = enrich_prs(pool, my_prs, &workspace_map).await?;

    // 4. Issues authored by user (the issues table has no assignee column;
    //    in this schema "assigned" maps to "authored" for the current user)
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
///
/// When multiple workspaces exist for the same PR (e.g. an archived and an active one),
/// the most relevant state wins: Active > Suspended > Archived.
/// Ties are broken by `updated_at` (newest first), then `id` (lexicographic).
fn build_workspace_map(workspaces: &[Workspace]) -> HashMap<(String, u32), Workspace> {
    let mut map: HashMap<(String, u32), Workspace> = HashMap::new();

    for ws in workspaces {
        let key = (ws.repo_id.clone(), ws.pull_request_number);
        let dominated = match map.get(&key) {
            Some(existing) => {
                let new_rank = (
                    workspace_state_rank(&ws.state),
                    ws.updated_at.as_str(),
                    ws.id.as_str(),
                );
                let old_rank = (
                    workspace_state_rank(&existing.state),
                    existing.updated_at.as_str(),
                    existing.id.as_str(),
                );
                new_rank > old_rank
            }
            None => true,
        };
        if dominated {
            map.insert(key, ws.clone());
        }
    }

    map
}

/// Rank workspace states: Active (2) > Suspended (1) > Archived (0).
fn workspace_state_rank(state: &WorkspaceState) -> u8 {
    match state {
        WorkspaceState::Active => 2,
        WorkspaceState::Suspended => 1,
        WorkspaceState::Archived => 0,
    }
}

/// Fetch pull requests by a list of IDs, chunked to respect `SQLite`'s parameter limit.
async fn fetch_prs_by_ids(pool: &SqlitePool, ids: &[String]) -> Result<Vec<PullRequest>, AppError> {
    const CHUNK_SIZE: usize = 998;

    if ids.is_empty() {
        return Ok(vec![]);
    }
    let mut all_rows: Vec<PullRequestRow> = Vec::with_capacity(ids.len());

    for chunk in ids.chunks(CHUNK_SIZE) {
        let placeholders: Vec<String> = (1..=chunk.len()).map(|i| format!("${i}")).collect();
        let sql = format!(
            "SELECT {PR_COLS} FROM pull_requests WHERE id IN ({}) AND state IN ('open', 'draft') ORDER BY updated_at DESC, id DESC",
            placeholders.join(", ")
        );

        let mut query = sqlx::query_as::<_, PullRequestRow>(&sql);
        for id in chunk {
            query = query.bind(id);
        }
        all_rows.extend(query.fetch_all(pool).await?);
    }

    // Re-sort across chunks to maintain consistent ordering
    all_rows.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| b.id.cmp(&a.id))
    });
    all_rows.into_iter().map(PullRequest::try_from).collect()
}

/// Fetch open/draft pull requests authored by a given user.
async fn fetch_prs_by_author(
    pool: &SqlitePool,
    author: &str,
) -> Result<Vec<PullRequest>, AppError> {
    let sql = format!(
        "SELECT {PR_COLS} FROM pull_requests \
         WHERE author = $1 AND state IN ('open', 'draft') \
         ORDER BY updated_at DESC, id DESC"
    );
    let rows: Vec<PullRequestRow> = sqlx::query_as(&sql).bind(author).fetch_all(pool).await?;

    rows.into_iter().map(PullRequest::try_from).collect()
}

/// Enrich a list of PRs with review summaries and optional workspace summaries.
///
/// Uses per-PR queries for review summaries and workspace notes. Acceptable for
/// personal dashboards (typically < 50 PRs). For larger scale, batch both
/// `review_requests` and `workspace_notes` with `WHERE ... IN (...)` queries.
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

/// Saturating conversion from SQL `COUNT(*)` (i64) to u32.
fn count_to_u32(count: i64) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// Compute dashboard counter stats via a single SQL query with sub-selects.
///
/// Returns a [`DashboardStats`] with counts of:
/// - `pending_reviews`: review requests for the user with status `pending` (open/draft PRs only)
/// - `open_prs`: pull requests authored by the user in `open` or `draft` state
/// - `open_issues`: issues authored by the user in `open` state
/// - `active_workspaces`: all workspaces in `active` state (global, not user-scoped)
/// - `unread_activity`: all activity events with `is_read = 0` (global, not user-scoped)
pub async fn compute_dashboard_stats(
    pool: &SqlitePool,
    username: &str,
) -> Result<DashboardStats, AppError> {
    let row: (i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT \
         (SELECT COUNT(*) FROM review_requests rr \
          JOIN pull_requests pr ON pr.id = rr.pull_request_id \
          WHERE rr.reviewer = $1 AND rr.status = 'pending' \
          AND pr.state IN ('open', 'draft')), \
         (SELECT COUNT(*) FROM pull_requests WHERE author = $1 AND state IN ('open', 'draft')), \
         (SELECT COUNT(*) FROM issues WHERE author = $1 AND state = 'open'), \
         (SELECT COUNT(*) FROM workspaces WHERE state = 'active'), \
         (SELECT COUNT(*) FROM activity WHERE is_read = 0)",
    )
    .bind(username)
    .fetch_one(pool)
    .await?;

    Ok(DashboardStats {
        pending_reviews: count_to_u32(row.0),
        open_prs: count_to_u32(row.1),
        open_issues: count_to_u32(row.2),
        active_workspaces: count_to_u32(row.3),
        unread_activity: count_to_u32(row.4),
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
    use crate::cache::activity::insert_activity;
    use crate::cache::db::init_db;
    use crate::cache::issues::upsert_issue;
    use crate::cache::pull_requests::upsert_pull_request;
    use crate::cache::repos::upsert_repo;
    use crate::cache::reviews::upsert_review_request;
    use crate::cache::workspaces::{add_note, create_workspace};
    use crate::types::{
        Activity, ActivityType, CiStatus, Issue, IssueState, PrState, Priority, Repo, WorkspaceNote,
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
            additions: 50,
            deletions: 10,
            head_ref_name: "fix/test-branch".to_string(),
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
        pool.close().await;
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
        pool.close().await;
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
        pool.close().await;
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
        pool.close().await;
    }

    /// Workspace map prefers Active over Archived when both exist for the same PR.
    #[tokio::test]
    async fn test_workspace_map_prefers_active_over_archived() {
        let archived = Workspace {
            id: "ws-archived".to_string(),
            repo_id: "repo-1".to_string(),
            pull_request_number: 1,
            state: WorkspaceState::Archived,
            worktree_path: None,
            session_id: None,
            created_at: "2026-03-01T10:00:00Z".to_string(),
            updated_at: "2026-03-01T10:00:00Z".to_string(),
        };
        let active = Workspace {
            id: "ws-active".to_string(),
            repo_id: "repo-1".to_string(),
            pull_request_number: 1,
            state: WorkspaceState::Active,
            worktree_path: Some("/tmp/ws".to_string()),
            session_id: None,
            created_at: "2026-03-20T10:00:00Z".to_string(),
            updated_at: "2026-03-20T10:00:00Z".to_string(),
        };

        // Insert Archived first, then Active — Active should win
        let map = build_workspace_map(&[archived.clone(), active.clone()]);
        assert_eq!(map.len(), 1);
        assert_eq!(map[&("repo-1".to_string(), 1)].id, "ws-active");

        // Insert Active first, then Archived — Active should still win
        let map = build_workspace_map(&[active, archived]);
        assert_eq!(map.len(), 1);
        assert_eq!(map[&("repo-1".to_string(), 1)].id, "ws-active");
    }

    /// Regression: non-pending review requests must NOT appear in dashboard review_requests.
    #[tokio::test]
    async fn test_assemble_dashboard_excludes_non_pending_reviews() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        // PR authored by bob
        let pr = sample_pr("pr-1", 1, "bob");
        upsert_pull_request(&pool, &pr).await.unwrap();

        // alice already approved this PR — should NOT show in her review queue
        let rr_approved = crate::types::ReviewRequest {
            id: "rr-1".to_string(),
            pull_request_id: "pr-1".to_string(),
            reviewer: "alice".to_string(),
            status: ReviewStatus::Approved,
            requested_at: "2026-03-20T10:00:00Z".to_string(),
        };
        upsert_review_request(&pool, &rr_approved).await.unwrap();

        let data = assemble_dashboard_data(&pool, "alice").await.unwrap();

        // Approved review should not appear in review_requests
        assert!(
            data.review_requests.is_empty(),
            "approved review request should be excluded from dashboard"
        );
        pool.close().await;
    }

    #[tokio::test]
    async fn test_compute_stats_empty() {
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
    async fn test_compute_stats_with_data() {
        let (pool, _tmp) = test_pool().await;
        let repo = sample_repo();
        upsert_repo(&pool, &repo).await.unwrap();

        // 2 open PRs by alice, 1 merged (should not count)
        let pr1 = sample_pr("pr-1", 1, "alice");
        upsert_pull_request(&pool, &pr1).await.unwrap();
        let pr2 = sample_pr("pr-2", 2, "alice");
        upsert_pull_request(&pool, &pr2).await.unwrap();
        let mut pr_merged = sample_pr("pr-3", 3, "alice");
        pr_merged.state = PrState::Merged;
        upsert_pull_request(&pool, &pr_merged).await.unwrap();

        // 1 pending review for alice, 1 approved (should not count)
        let pr_bob = sample_pr("pr-4", 4, "bob");
        upsert_pull_request(&pool, &pr_bob).await.unwrap();
        let rr_pending = sample_review_request("rr-1", "pr-4", "alice");
        upsert_review_request(&pool, &rr_pending).await.unwrap();
        let rr_approved = crate::types::ReviewRequest {
            id: "rr-2".to_string(),
            pull_request_id: "pr-4".to_string(),
            reviewer: "alice".to_string(),
            status: ReviewStatus::Approved,
            requested_at: "2026-03-20T10:00:00Z".to_string(),
        };
        upsert_review_request(&pool, &rr_approved).await.unwrap();

        // 1 open issue by alice, 1 closed (should not count)
        let issue_open = Issue {
            id: "issue-1".to_string(),
            number: 1,
            title: "Bug".to_string(),
            author: "alice".to_string(),
            state: IssueState::Open,
            priority: Priority::Medium,
            repo_id: "repo-1".to_string(),
            url: "https://github.com/mpiton/prism/issues/1".to_string(),
            labels: vec![],
            created_at: "2026-03-01T10:00:00Z".to_string(),
            updated_at: "2026-03-20T15:00:00Z".to_string(),
        };
        upsert_issue(&pool, &issue_open).await.unwrap();
        let issue_closed = Issue {
            id: "issue-2".to_string(),
            number: 2,
            title: "Fixed".to_string(),
            author: "alice".to_string(),
            state: IssueState::Closed,
            priority: Priority::Low,
            repo_id: "repo-1".to_string(),
            url: "https://github.com/mpiton/prism/issues/2".to_string(),
            labels: vec![],
            created_at: "2026-03-01T10:00:00Z".to_string(),
            updated_at: "2026-03-20T15:00:00Z".to_string(),
        };
        upsert_issue(&pool, &issue_closed).await.unwrap();

        // 1 active workspace
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

        // 2 unread activities, 1 read (should not count)
        let act1 = Activity {
            id: "act-1".to_string(),
            activity_type: ActivityType::PrOpened,
            actor: "bob".to_string(),
            repo_id: "repo-1".to_string(),
            pull_request_id: Some("pr-4".to_string()),
            issue_id: None,
            message: "Bob opened PR".to_string(),
            created_at: "2026-03-20T10:00:00Z".to_string(),
        };
        insert_activity(&pool, &act1).await.unwrap();
        let act2 = Activity {
            id: "act-2".to_string(),
            activity_type: ActivityType::CommentAdded,
            actor: "charlie".to_string(),
            repo_id: "repo-1".to_string(),
            pull_request_id: None,
            issue_id: Some("issue-1".to_string()),
            message: "Charlie commented".to_string(),
            created_at: "2026-03-20T11:00:00Z".to_string(),
        };
        insert_activity(&pool, &act2).await.unwrap();
        // Mark act-1 as read
        sqlx::query("UPDATE activity SET is_read = 1 WHERE id = $1")
            .bind("act-1")
            .execute(&pool)
            .await
            .unwrap();

        let stats = compute_dashboard_stats(&pool, "alice").await.unwrap();

        assert_eq!(stats.pending_reviews, 1, "only pending review requests");
        assert_eq!(stats.open_prs, 2, "only open/draft PRs by alice");
        assert_eq!(stats.open_issues, 1, "only open issues by alice");
        assert_eq!(stats.active_workspaces, 1, "only active workspaces");
        assert_eq!(stats.unread_activity, 1, "only unread activity");
        pool.close().await;
    }
}
