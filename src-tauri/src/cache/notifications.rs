use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::AppError;

/// Check whether a notification has already been sent for the given event.
#[allow(dead_code)] // Called from notifications module; call-site integration deferred
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
/// Deduplication is enforced by the `UNIQUE(event_type, event_id)`
/// constraint via `ON CONFLICT ... DO NOTHING`. The PK is a random
/// UUID to avoid any separator-collision risk with composite keys.
#[allow(dead_code)] // Called from notifications module; call-site integration deferred
pub async fn mark_notified(
    pool: &SqlitePool,
    event_type: &str,
    event_id: &str,
) -> Result<(), AppError> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO notification_log (id, event_type, event_id, notified_at) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT(event_type, event_id) DO NOTHING",
    )
    .bind(&id)
    .bind(event_type)
    .bind(event_id)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(())
}

/// Atomically claim a notification slot.
///
/// Inserts into `notification_log` with `ON CONFLICT DO NOTHING` and
/// returns `true` if the row was newly inserted (i.e. the caller "won"
/// the claim). Returns `false` if the event was already recorded.
///
/// This eliminates the TOCTOU race between `has_been_notified` and
/// `mark_notified` — a single INSERT is atomic at the `SQLite` level.
#[allow(dead_code)] // Called from notifications module; call-site integration deferred
pub async fn try_claim_notification(
    pool: &SqlitePool,
    event_type: &str,
    event_id: &str,
) -> Result<bool, AppError> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let result = sqlx::query(
        "INSERT INTO notification_log (id, event_type, event_id, notified_at) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT(event_type, event_id) DO NOTHING",
    )
    .bind(&id)
    .bind(event_type)
    .bind(event_id)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Remove a notification entry, allowing the event to re-trigger.
///
/// Used for transient events like CI failures that can recur after
/// the underlying condition is resolved and then re-appears.
#[allow(dead_code)] // Called from notifications module; call-site integration deferred
pub async fn clear_notification(
    pool: &SqlitePool,
    event_type: &str,
    event_id: &str,
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM notification_log WHERE event_type = $1 AND event_id = $2")
        .bind(event_type)
        .bind(event_id)
        .execute(pool)
        .await?;

    Ok(())
}

/// Remove all notification entries of a given type whose `event_id` is NOT
/// in the provided set of current IDs.
///
/// Used to clear stale `review_request` entries when PRs leave the
/// review queue, so re-requests naturally re-trigger notifications.
#[allow(dead_code)] // Called from notifications module; call-site integration deferred
pub async fn clear_stale_notifications(
    pool: &SqlitePool,
    event_type: &str,
    current_ids: &[&str],
) -> Result<(), AppError> {
    if current_ids.is_empty() {
        // No active items — clear all entries of this type.
        sqlx::query("DELETE FROM notification_log WHERE event_type = $1")
            .bind(event_type)
            .execute(pool)
            .await?;
    } else {
        // Build a parameterised IN-clause. SQLite supports up to 999
        // parameters; review_request lists are far smaller.
        let placeholders: Vec<String> = (0..current_ids.len())
            .map(|i| format!("${}", i + 2)) // $1 is event_type
            .collect();
        let sql = format!(
            "DELETE FROM notification_log WHERE event_type = $1 AND event_id NOT IN ({})",
            placeholders.join(", ")
        );
        let mut query = sqlx::query(&sql).bind(event_type);
        for id in current_ids {
            query = query.bind(*id);
        }
        query.execute(pool).await?;
    }

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

    #[tokio::test]
    async fn test_try_claim_notification_returns_true_on_first_insert() {
        let (pool, _tmp) = test_pool().await;

        let claimed = try_claim_notification(&pool, "review_request", "pr-1")
            .await
            .unwrap();
        assert!(claimed, "first claim should succeed");

        let claimed_again = try_claim_notification(&pool, "review_request", "pr-1")
            .await
            .unwrap();
        assert!(!claimed_again, "second claim should return false");

        pool.close().await;
    }

    #[tokio::test]
    async fn test_clear_notification_allows_reclaim() {
        let (pool, _tmp) = test_pool().await;

        // Claim
        let claimed = try_claim_notification(&pool, "ci_failure", "pr-1")
            .await
            .unwrap();
        assert!(claimed);

        // Clear
        clear_notification(&pool, "ci_failure", "pr-1")
            .await
            .unwrap();

        // Re-claim should succeed
        let reclaimed = try_claim_notification(&pool, "ci_failure", "pr-1")
            .await
            .unwrap();
        assert!(reclaimed, "should be able to reclaim after clearing");

        pool.close().await;
    }

    #[tokio::test]
    async fn test_clear_stale_notifications() {
        let (pool, _tmp) = test_pool().await;

        // Claim three review_request entries
        try_claim_notification(&pool, "review_request", "pr-1")
            .await
            .unwrap();
        try_claim_notification(&pool, "review_request", "pr-2")
            .await
            .unwrap();
        try_claim_notification(&pool, "review_request", "pr-3")
            .await
            .unwrap();

        // Only pr-1 and pr-3 are still in the queue
        clear_stale_notifications(&pool, "review_request", &["pr-1", "pr-3"])
            .await
            .unwrap();

        // pr-2 should be cleared (can reclaim)
        assert!(
            !has_been_notified(&pool, "review_request", "pr-2")
                .await
                .unwrap(),
            "stale entry should be cleared"
        );

        // pr-1 and pr-3 should still be claimed
        assert!(
            has_been_notified(&pool, "review_request", "pr-1")
                .await
                .unwrap(),
            "active entry should remain"
        );
        assert!(
            has_been_notified(&pool, "review_request", "pr-3")
                .await
                .unwrap(),
            "active entry should remain"
        );

        pool.close().await;
    }

    #[tokio::test]
    async fn test_clear_stale_notifications_empty_list_clears_all() {
        let (pool, _tmp) = test_pool().await;

        try_claim_notification(&pool, "review_request", "pr-1")
            .await
            .unwrap();
        try_claim_notification(&pool, "review_request", "pr-2")
            .await
            .unwrap();

        // Empty current list → clear all review_request entries
        clear_stale_notifications(&pool, "review_request", &[])
            .await
            .unwrap();

        assert!(
            !has_been_notified(&pool, "review_request", "pr-1")
                .await
                .unwrap()
        );
        assert!(
            !has_been_notified(&pool, "review_request", "pr-2")
                .await
                .unwrap()
        );

        pool.close().await;
    }
}
