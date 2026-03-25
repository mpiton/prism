use chrono::Utc;
use sqlx::SqlitePool;

use crate::error::AppError;
use crate::types::{Activity, ActivityType};

/// Row representation matching the `activity` table columns.
#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct ActivityRow {
    id: String,
    activity_type: String,
    actor: String,
    repo_id: String,
    pull_request_id: Option<String>,
    issue_id: Option<String>,
    message: String,
    is_read: i64,
    created_at: String,
}

fn activity_type_to_str(t: &ActivityType) -> &'static str {
    match t {
        ActivityType::PrOpened => "pr_opened",
        ActivityType::PrMerged => "pr_merged",
        ActivityType::PrClosed => "pr_closed",
        ActivityType::ReviewSubmitted => "review_submitted",
        ActivityType::CommentAdded => "comment_added",
        ActivityType::CiCompleted => "ci_completed",
        ActivityType::IssueOpened => "issue_opened",
        ActivityType::IssueClosed => "issue_closed",
    }
}

fn activity_type_from_str(s: &str) -> Result<ActivityType, AppError> {
    match s {
        "pr_opened" => Ok(ActivityType::PrOpened),
        "pr_merged" => Ok(ActivityType::PrMerged),
        "pr_closed" => Ok(ActivityType::PrClosed),
        "review_submitted" => Ok(ActivityType::ReviewSubmitted),
        "comment_added" => Ok(ActivityType::CommentAdded),
        "ci_completed" => Ok(ActivityType::CiCompleted),
        "issue_opened" => Ok(ActivityType::IssueOpened),
        "issue_closed" => Ok(ActivityType::IssueClosed),
        _ => Err(AppError::Config(format!("unknown ActivityType: {s}"))),
    }
}

impl TryFrom<ActivityRow> for Activity {
    type Error = AppError;

    fn try_from(row: ActivityRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            activity_type: activity_type_from_str(&row.activity_type)?,
            actor: row.actor,
            repo_id: row.repo_id,
            pull_request_id: row.pull_request_id,
            issue_id: row.issue_id,
            message: row.message,
            created_at: row.created_at,
        })
    }
}

/// Explicit column list for all queries on `activity`.
/// `is_read` is populated by `ActivityRow` (DB DEFAULT 0 on INSERT) but
/// discarded by `TryFrom<Activity>` — it is a DB-internal flag, not part
/// of the domain struct.
const ACTIVITY_COLS: &str =
    "id, activity_type, actor, repo_id, pull_request_id, issue_id, message, is_read, created_at";

/// Insert a new activity event. Uses `RETURNING` for atomic read-after-write.
#[allow(dead_code)]
pub async fn insert_activity(pool: &SqlitePool, activity: &Activity) -> Result<Activity, AppError> {
    let sql = format!(
        "INSERT INTO activity (id, activity_type, actor, repo_id, pull_request_id, issue_id, message, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING {ACTIVITY_COLS}"
    );

    let row: ActivityRow = sqlx::query_as(&sql)
        .bind(&activity.id)
        .bind(activity_type_to_str(&activity.activity_type))
        .bind(&activity.actor)
        .bind(&activity.repo_id)
        .bind(&activity.pull_request_id)
        .bind(&activity.issue_id)
        .bind(&activity.message)
        .bind(&activity.created_at)
        .fetch_one(pool)
        .await?;

    Activity::try_from(row)
}

/// Return recent activity events, paginated and ordered by `created_at DESC`.
#[allow(dead_code)]
pub async fn get_recent_activity(
    pool: &SqlitePool,
    limit: i64,
    offset: i64,
) -> Result<Vec<Activity>, AppError> {
    let sql =
        format!("SELECT {ACTIVITY_COLS} FROM activity ORDER BY created_at DESC LIMIT $1 OFFSET $2");
    let rows: Vec<ActivityRow> = sqlx::query_as(&sql)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    rows.into_iter().map(Activity::try_from).collect()
}

/// Mark a single activity as read. Returns `true` if a row was updated.
#[allow(dead_code)]
pub async fn mark_read(pool: &SqlitePool, id: &str) -> Result<bool, AppError> {
    let result = sqlx::query("UPDATE activity SET is_read = 1 WHERE id = $1 AND is_read = 0")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

/// Mark all unread activities as read. Returns the number of rows updated.
#[allow(dead_code)]
pub async fn mark_all_read(pool: &SqlitePool) -> Result<u64, AppError> {
    let result = sqlx::query("UPDATE activity SET is_read = 1 WHERE is_read = 0")
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

/// Retrieve a single activity by ID.
#[allow(dead_code)]
pub async fn get_activity_by_id(pool: &SqlitePool, id: &str) -> Result<Option<Activity>, AppError> {
    let sql = format!("SELECT {ACTIVITY_COLS} FROM activity WHERE id = $1");
    let row: Option<ActivityRow> = sqlx::query_as(&sql).bind(id).fetch_optional(pool).await?;

    row.map(Activity::try_from).transpose()
}

/// Delete activity events older than `retention_days`. Returns deleted row count.
///
/// Computes the threshold date in Rust (via `chrono`) and passes it as an
/// opaque ISO-8601 string to avoid SQL string concatenation.
#[allow(dead_code)]
pub async fn cleanup_old_activity(pool: &SqlitePool, retention_days: i64) -> Result<u64, AppError> {
    let threshold = (Utc::now() - chrono::Duration::days(retention_days))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();

    let result = sqlx::query("DELETE FROM activity WHERE created_at < $1")
        .bind(&threshold)
        .execute(pool)
        .await?;

    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::db::init_db;
    use crate::cache::repos::upsert_repo;
    use crate::types::Repo;

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

    fn sample_activity(id: &str, activity_type: ActivityType, message: &str) -> Activity {
        Activity {
            id: id.to_string(),
            activity_type,
            actor: "mpiton".to_string(),
            repo_id: "repo-1".to_string(),
            pull_request_id: None,
            issue_id: None,
            message: message.to_string(),
            created_at: "2026-03-20T10:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn test_insert_activity() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let activity = sample_activity("act-1", ActivityType::PrOpened, "Opened PR #42");
        let result = insert_activity(&pool, &activity).await.unwrap();

        assert_eq!(result.id, "act-1");
        assert_eq!(result.activity_type, ActivityType::PrOpened);
        assert_eq!(result.actor, "mpiton");
        assert_eq!(result.repo_id, "repo-1");
        assert_eq!(result.pull_request_id, None);
        assert_eq!(result.issue_id, None);
        assert_eq!(result.message, "Opened PR #42");
        assert_eq!(result.created_at, "2026-03-20T10:00:00Z");

        // Duplicate insert should fail (PRIMARY KEY constraint)
        let dup = insert_activity(&pool, &activity).await;
        assert!(dup.is_err());

        pool.close().await;
    }

    #[tokio::test]
    async fn test_get_recent_activity_order() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let mut a1 = sample_activity("act-1", ActivityType::PrOpened, "First");
        a1.created_at = "2026-03-20T10:00:00Z".to_string();

        let mut a2 = sample_activity("act-2", ActivityType::ReviewSubmitted, "Second");
        a2.created_at = "2026-03-21T10:00:00Z".to_string();

        let mut a3 = sample_activity("act-3", ActivityType::PrMerged, "Third");
        a3.created_at = "2026-03-22T10:00:00Z".to_string();

        insert_activity(&pool, &a1).await.unwrap();
        insert_activity(&pool, &a2).await.unwrap();
        insert_activity(&pool, &a3).await.unwrap();

        // Get all — most recent first
        let activities = get_recent_activity(&pool, 10, 0).await.unwrap();
        assert_eq!(activities.len(), 3);
        assert_eq!(activities[0].id, "act-3");
        assert_eq!(activities[1].id, "act-2");
        assert_eq!(activities[2].id, "act-1");

        // Pagination: limit 2, offset 0
        let page1 = get_recent_activity(&pool, 2, 0).await.unwrap();
        assert_eq!(page1.len(), 2);
        assert_eq!(page1[0].id, "act-3");
        assert_eq!(page1[1].id, "act-2");

        // Pagination: limit 2, offset 2
        let page2 = get_recent_activity(&pool, 2, 2).await.unwrap();
        assert_eq!(page2.len(), 1);
        assert_eq!(page2[0].id, "act-1");

        pool.close().await;
    }

    #[tokio::test]
    async fn test_get_recent_activity_empty() {
        let (pool, _tmp) = test_pool().await;

        let activities = get_recent_activity(&pool, 10, 0).await.unwrap();
        assert!(activities.is_empty());

        pool.close().await;
    }

    #[tokio::test]
    async fn test_mark_read() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let activity = sample_activity("act-1", ActivityType::CommentAdded, "New comment");
        insert_activity(&pool, &activity).await.unwrap();

        // Mark as read — should succeed
        let updated = mark_read(&pool, "act-1").await.unwrap();
        assert!(updated);

        // Mark again — already read, no row updated
        let already_read = mark_read(&pool, "act-1").await.unwrap();
        assert!(!already_read);

        // Non-existent ID
        let missing = mark_read(&pool, "nonexistent").await.unwrap();
        assert!(!missing);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_mark_all_read() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let a1 = sample_activity("act-1", ActivityType::PrOpened, "First");
        let a2 = sample_activity("act-2", ActivityType::PrMerged, "Second");
        insert_activity(&pool, &a1).await.unwrap();
        insert_activity(&pool, &a2).await.unwrap();

        // Mark one as read first
        mark_read(&pool, "act-1").await.unwrap();

        // Mark all — only act-2 should be updated
        let count = mark_all_read(&pool).await.unwrap();
        assert_eq!(count, 1);

        // Mark all again — none should be updated
        let count = mark_all_read(&pool).await.unwrap();
        assert_eq!(count, 0);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_get_activity_by_id() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let activity = sample_activity("act-1", ActivityType::PrOpened, "Opened PR #42");
        insert_activity(&pool, &activity).await.unwrap();

        let found = get_activity_by_id(&pool, "act-1").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "act-1");

        let missing = get_activity_by_id(&pool, "nonexistent").await.unwrap();
        assert!(missing.is_none());

        pool.close().await;
    }

    #[tokio::test]
    async fn test_cleanup_old_activity() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        // Insert an old activity (2020) and a recent one
        let mut old = sample_activity("act-old", ActivityType::IssueOpened, "Old event");
        old.created_at = "2020-01-01T00:00:00Z".to_string();

        let recent = sample_activity("act-new", ActivityType::IssueClosed, "New event");

        insert_activity(&pool, &old).await.unwrap();
        insert_activity(&pool, &recent).await.unwrap();

        // Cleanup with 30-day retention — should delete the old one
        let deleted = cleanup_old_activity(&pool, 30).await.unwrap();
        assert_eq!(deleted, 1);

        // Verify only the recent one remains
        let remaining = get_recent_activity(&pool, 10, 0).await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, "act-new");

        pool.close().await;
    }
}
