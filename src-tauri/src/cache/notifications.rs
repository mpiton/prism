use chrono::Utc;
use sqlx::SqlitePool;

use crate::error::AppError;

/// Check whether a notification has already been sent for the given event.
#[allow(dead_code)]
pub async fn has_been_notified(
    pool: &SqlitePool,
    event_type: &str,
    event_id: &str,
) -> Result<bool, AppError> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT 1 FROM notification_log WHERE event_type = $1 AND event_id = $2")
            .bind(event_type)
            .bind(event_id)
            .fetch_optional(pool)
            .await?;

    Ok(row.is_some())
}

/// Record that a notification was sent for the given event.
///
/// Uses `INSERT OR IGNORE` for deduplication via the
/// `UNIQUE(event_type, event_id)` constraint.
#[allow(dead_code)]
pub async fn mark_notified(
    pool: &SqlitePool,
    event_type: &str,
    event_id: &str,
) -> Result<(), AppError> {
    let now = Utc::now().to_rfc3339();
    // Opaque surrogate PK — dedup is enforced by UNIQUE(event_type, event_id).
    let id = format!("{event_type}\x1F{event_id}");

    sqlx::query(
        "INSERT OR IGNORE INTO notification_log (id, event_type, event_id, notified_at) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(&id)
    .bind(event_type)
    .bind(event_id)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::db::init_db;

    async fn test_pool() -> (SqlitePool, tempfile::TempDir) {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = init_db(&tmp.path().join("test.db")).await.unwrap();
        (pool, tmp)
    }

    #[tokio::test]
    async fn test_has_been_notified_false() {
        let (pool, _tmp) = test_pool().await;

        let result = has_been_notified(&pool, "review_submitted", "evt-1")
            .await
            .unwrap();
        assert!(!result, "should not be notified for unknown event");

        pool.close().await;
    }

    #[tokio::test]
    async fn test_mark_notified_then_check() {
        let (pool, _tmp) = test_pool().await;

        mark_notified(&pool, "pr_opened", "pr-42").await.unwrap();

        let result = has_been_notified(&pool, "pr_opened", "pr-42")
            .await
            .unwrap();
        assert!(result, "should be notified after marking");

        // Different event_id → still not notified
        let other = has_been_notified(&pool, "pr_opened", "pr-99")
            .await
            .unwrap();
        assert!(!other, "different event_id should not match");

        pool.close().await;
    }

    #[tokio::test]
    async fn test_notification_dedup() {
        let (pool, _tmp) = test_pool().await;

        // Mark the same event twice — should not error
        mark_notified(&pool, "review_submitted", "rev-1")
            .await
            .unwrap();
        mark_notified(&pool, "review_submitted", "rev-1")
            .await
            .unwrap();

        let result = has_been_notified(&pool, "review_submitted", "rev-1")
            .await
            .unwrap();
        assert!(result);

        // Verify only one row exists (dedup via UNIQUE constraint)
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM notification_log \
             WHERE event_type = $1 AND event_id = $2",
        )
        .bind("review_submitted")
        .bind("rev-1")
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(count.0, 1, "dedup should prevent duplicate rows");

        pool.close().await;
    }
}
