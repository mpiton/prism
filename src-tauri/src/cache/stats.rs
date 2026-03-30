use sqlx::SqlitePool;

use crate::error::AppError;
use crate::types::PersonalStats;

/// Compute personal statistics for the authenticated user.
///
/// Queries:
/// - `prs_merged_this_week`: PRs authored by the user whose state changed to `merged`
///   this week. Uses `updated_at` as proxy because the schema has no `merged_at` column;
///   this is approximate — a PR re-synced this week will be counted even if merged earlier.
/// - `avg_review_response_hours`: average hours between review request and review submission
///   for this user as reviewer (all time). `0.0` when no data.
/// - `reviews_given_this_week`: reviews submitted by the user in the current ISO week.
/// - `active_workspace_count`: workspaces in `active` state (global, not user-scoped —
///   the workspaces table has no owner column).
pub async fn compute_personal_stats(
    pool: &SqlitePool,
    username: &str,
) -> Result<PersonalStats, AppError> {
    // Week boundary: Monday 00:00:00 UTC of the current ISO week.
    // `weekday 1` advances to next Monday, `-7 days` rolls back to the most recent Monday.
    // This is correct for all days of the week including Sunday (unlike `weekday 0, -6 days`).
    let monday = "date('now', 'weekday 1', '-7 days')";

    let sql = format!(
        "SELECT \
         (SELECT COUNT(*) FROM pull_requests \
          WHERE author = $1 AND state = 'merged' \
          AND updated_at >= {monday}), \
         (SELECT COALESCE(AVG( \
            (julianday(rv.submitted_at) - julianday(rr.requested_at)) * 24.0 \
          ), 0.0) \
          FROM reviews rv \
          JOIN review_requests rr \
            ON rv.pull_request_id = rr.pull_request_id \
           AND rv.reviewer = rr.reviewer \
          WHERE rv.reviewer = $1), \
         (SELECT COUNT(*) FROM reviews \
          WHERE reviewer = $1 \
          AND submitted_at >= {monday}), \
         (SELECT COUNT(*) FROM workspaces WHERE state = 'active')"
    );

    let row: (i64, f64, i64, i64) = sqlx::query_as(&sql).bind(username).fetch_one(pool).await?;

    // Saturating — COUNT(*) on a local SQLite DB will never exceed u32::MAX.
    Ok(PersonalStats {
        prs_merged_this_week: u32::try_from(row.0).unwrap_or(u32::MAX),
        avg_review_response_hours: if row.1 < 0.0 { 0.0 } else { row.1 },
        reviews_given_this_week: u32::try_from(row.2).unwrap_or(u32::MAX),
        active_workspace_count: u32::try_from(row.3).unwrap_or(u32::MAX),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::db::init_db;
    use crate::cache::pull_requests::upsert_pull_request;
    use crate::cache::repos::upsert_repo;
    use crate::cache::reviews::{upsert_review, upsert_review_request};
    use crate::cache::workspaces::create_workspace;
    use crate::types::{
        CiStatus, PrState, Priority, PullRequest, Repo, Review, ReviewRequest, ReviewStatus,
        Workspace, WorkspaceState,
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

    fn sample_pr(id: &str, number: u32, author: &str, state: PrState) -> PullRequest {
        PullRequest {
            id: id.to_string(),
            number,
            title: format!("PR #{number}"),
            author: author.to_string(),
            state,
            ci_status: CiStatus::Success,
            priority: Priority::High,
            repo_id: "repo-1".to_string(),
            url: format!("https://github.com/mpiton/prism/pull/{number}"),
            labels: vec![],
            additions: 10,
            deletions: 5,
            created_at: "2026-03-01T10:00:00Z".to_string(),
            updated_at: "2026-03-30T10:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn test_compute_stats_empty() {
        let (pool, _tmp) = test_pool().await;

        let stats = compute_personal_stats(&pool, "alice").await.unwrap();

        assert_eq!(stats.prs_merged_this_week, 0);
        assert_eq!(stats.avg_review_response_hours, 0.0);
        assert_eq!(stats.reviews_given_this_week, 0);
        assert_eq!(stats.active_workspace_count, 0);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_compute_stats_with_data() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        // 2 merged PRs this week by alice
        let pr1 = sample_pr("pr-1", 1, "alice", PrState::Merged);
        upsert_pull_request(&pool, &pr1).await.unwrap();
        let pr2 = sample_pr("pr-2", 2, "alice", PrState::Merged);
        upsert_pull_request(&pool, &pr2).await.unwrap();

        // 1 open PR (should not count as merged)
        let pr3 = sample_pr("pr-3", 3, "alice", PrState::Open);
        upsert_pull_request(&pool, &pr3).await.unwrap();

        // 1 merged PR by someone else (should not count)
        let pr4 = sample_pr("pr-4", 4, "bob", PrState::Merged);
        upsert_pull_request(&pool, &pr4).await.unwrap();

        // Review request + review by alice (for avg response hours)
        let pr5 = sample_pr("pr-5", 5, "charlie", PrState::Open);
        upsert_pull_request(&pool, &pr5).await.unwrap();
        let rr = ReviewRequest {
            id: "rr-1".to_string(),
            pull_request_id: "pr-5".to_string(),
            reviewer: "alice".to_string(),
            status: ReviewStatus::Approved,
            requested_at: "2026-03-30T10:00:00Z".to_string(),
        };
        upsert_review_request(&pool, &rr).await.unwrap();
        let review = Review {
            id: "rev-1".to_string(),
            pull_request_id: "pr-5".to_string(),
            reviewer: "alice".to_string(),
            status: ReviewStatus::Approved,
            body: Some("LGTM".to_string()),
            submitted_at: "2026-03-30T14:00:00Z".to_string(), // 4 hours later
        };
        upsert_review(&pool, &review).await.unwrap();

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

        // 1 suspended workspace (should not count)
        let ws2 = Workspace {
            id: "ws-2".to_string(),
            repo_id: "repo-1".to_string(),
            pull_request_number: 2,
            state: WorkspaceState::Suspended,
            worktree_path: None,
            session_id: None,
            created_at: "2026-03-20T10:00:00Z".to_string(),
            updated_at: "2026-03-20T10:00:00Z".to_string(),
        };
        create_workspace(&pool, &ws2).await.unwrap();

        let stats = compute_personal_stats(&pool, "alice").await.unwrap();

        assert_eq!(
            stats.prs_merged_this_week, 2,
            "2 merged PRs by alice this week"
        );
        assert!(
            (stats.avg_review_response_hours - 4.0).abs() < 0.01,
            "avg review response ~4 hours, got {}",
            stats.avg_review_response_hours
        );
        assert_eq!(
            stats.reviews_given_this_week, 1,
            "1 review by alice this week"
        );
        assert_eq!(stats.active_workspace_count, 1, "1 active workspace");

        pool.close().await;
    }
}
