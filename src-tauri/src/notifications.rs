#![allow(dead_code)] // Call-site integration deferred to polling loop wiring task

use serde::Serialize;
use sqlx::SqlitePool;
use tauri::Emitter;
use tracing::{info, warn};

use crate::cache::notifications::{
    clear_notification, clear_stale_notifications, try_claim_notification,
};
use crate::error::AppError;
use crate::types::{CiStatus, DashboardData, PullRequest};

/// Payload emitted with each notification Tauri event.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NotificationPayload {
    pub pr_number: u32,
    pub pr_title: String,
    pub repo_id: String,
    pub url: String,
}

impl NotificationPayload {
    fn from_pr(pr: &PullRequest) -> Self {
        Self {
            pr_number: pr.number,
            pr_title: pr.title.clone(),
            repo_id: pr.repo_id.clone(),
            url: pr.url.clone(),
        }
    }
}

/// Abstracts notification emission for testability.
///
/// The real implementation ([`TauriNotificationSender`]) emits Tauri events
/// and sends native desktop notifications via `notify-rust`.
pub(crate) trait NotificationSender: Send + Sync {
    fn emit_review_request(&self, payload: &NotificationPayload);
    fn emit_ci_failure(&self, payload: &NotificationPayload);
    fn emit_pr_approved(&self, payload: &NotificationPayload);
}

/// Real implementation using Tauri events + native desktop notifications.
pub(crate) struct TauriNotificationSender {
    app_handle: tauri::AppHandle,
}

impl TauriNotificationSender {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self { app_handle }
    }

    fn send_native(title: &str, body: &str) {
        if let Err(e) = notify_rust::Notification::new()
            .appname("PRism")
            .summary(title)
            .body(body)
            .show()
        {
            warn!("failed to show native notification: {e}");
        }
    }
}

impl NotificationSender for TauriNotificationSender {
    fn emit_review_request(&self, payload: &NotificationPayload) {
        if let Err(e) = self.app_handle.emit("notification:review_request", payload) {
            warn!("failed to emit notification:review_request: {e}");
        }
        Self::send_native(
            "New Review Request",
            &format!("PR #{}: {}", payload.pr_number, payload.pr_title),
        );
    }

    fn emit_ci_failure(&self, payload: &NotificationPayload) {
        if let Err(e) = self.app_handle.emit("notification:ci_failure", payload) {
            warn!("failed to emit notification:ci_failure: {e}");
        }
        Self::send_native(
            "CI Failed",
            &format!("PR #{}: {}", payload.pr_number, payload.pr_title),
        );
    }

    fn emit_pr_approved(&self, payload: &NotificationPayload) {
        if let Err(e) = self.app_handle.emit("notification:pr_approved", payload) {
            warn!("failed to emit notification:pr_approved: {e}");
        }
        Self::send_native(
            "PR Approved",
            &format!("PR #{}: {}", payload.pr_number, payload.pr_title),
        );
    }
}

/// Check new dashboard data for notification-worthy events.
///
/// Uses [`try_claim_notification`] for atomic deduplication — a single
/// `INSERT ... ON CONFLICT DO NOTHING` that returns whether the caller
/// won the claim, eliminating TOCTOU races between check and mark.
///
/// Delivery guarantee: **at-most-once** per `(event_type, event_id)`.
/// The claim is persisted before emission. If the Tauri event or native
/// notification fails after claiming, the notification will not be
/// retried. This avoids duplicate spam at the cost of rare silent drops.
///
/// CI failure notifications are transient: when a PR's CI status moves
/// away from `Failure`, the dedup entry is cleared so a future failure
/// on the same PR can re-trigger a notification.
///
/// Returns the number of notifications emitted.
pub(crate) async fn check_and_notify(
    pool: &SqlitePool,
    sender: &(impl NotificationSender + ?Sized),
    new_data: &DashboardData,
) -> Result<u32, AppError> {
    let mut count = 0u32;

    // 1. New review requests — keyed on pr.id. Stale entries are
    //    cleared for PRs that left the review queue, so re-requests
    //    naturally re-trigger.
    let review_pr_ids: Vec<&str> = new_data
        .review_requests
        .iter()
        .map(|prwr| prwr.pull_request.id.as_str())
        .collect();
    clear_stale_notifications(pool, "review_request", &review_pr_ids).await?;

    for pr_with_review in &new_data.review_requests {
        let pr = &pr_with_review.pull_request;
        if try_claim_notification(pool, "review_request", &pr.id).await? {
            sender.emit_review_request(&NotificationPayload::from_pr(pr));
            count += 1;
        }
    }

    // 2. CI failures and PR approvals on authored PRs
    for pr_with_review in &new_data.my_pull_requests {
        let pr = &pr_with_review.pull_request;

        // CI failure: claim + emit. When CI is no longer failing, clear
        // the entry so a future failure can re-notify.
        if pr.ci_status == CiStatus::Failure {
            if try_claim_notification(pool, "ci_failure", &pr.id).await? {
                sender.emit_ci_failure(&NotificationPayload::from_pr(pr));
                count += 1;
            }
        } else {
            clear_notification(pool, "ci_failure", &pr.id).await?;
        }

        // PR approval: clear when no longer approved so a re-approval
        // after changes_requested can re-trigger a notification.
        if pr_with_review.review_summary.approved > 0 {
            if try_claim_notification(pool, "pr_approved", &pr.id).await? {
                sender.emit_pr_approved(&NotificationPayload::from_pr(pr));
                count += 1;
            }
        } else {
            clear_notification(pool, "pr_approved", &pr.id).await?;
        }
    }

    info!("check_and_notify: emitted {count} notification(s)");
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::db::init_db;
    use crate::cache::notifications::has_been_notified;
    use crate::types::*;
    use std::sync::{Arc, Mutex};

    async fn test_pool() -> (SqlitePool, tempfile::TempDir) {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = init_db(&tmp.path().join("test.db")).await.unwrap();
        (pool, tmp)
    }

    #[derive(Debug, Default, Clone)]
    struct MockSender {
        review_requests: Arc<Mutex<Vec<NotificationPayload>>>,
        ci_failures: Arc<Mutex<Vec<NotificationPayload>>>,
        pr_approvals: Arc<Mutex<Vec<NotificationPayload>>>,
    }

    impl NotificationSender for MockSender {
        fn emit_review_request(&self, payload: &NotificationPayload) {
            self.review_requests
                .lock()
                .expect("MockSender lock poisoned")
                .push(payload.clone());
        }

        fn emit_ci_failure(&self, payload: &NotificationPayload) {
            self.ci_failures
                .lock()
                .expect("MockSender lock poisoned")
                .push(payload.clone());
        }

        fn emit_pr_approved(&self, payload: &NotificationPayload) {
            self.pr_approvals
                .lock()
                .expect("MockSender lock poisoned")
                .push(payload.clone());
        }
    }

    fn make_pr(id: &str, number: u32, ci_status: CiStatus) -> PullRequest {
        PullRequest {
            id: id.to_string(),
            number,
            title: format!("PR #{number}"),
            author: "mpiton".to_string(),
            state: PrState::Open,
            ci_status,
            priority: Priority::Medium,
            repo_id: "r-1".to_string(),
            url: format!("https://github.com/test/repo/pull/{number}"),
            labels: vec![],
            additions: 10,
            deletions: 5,
            created_at: "2026-03-29T10:00:00Z".to_string(),
            updated_at: "2026-03-29T10:00:00Z".to_string(),
        }
    }

    fn make_pr_with_review(pr: PullRequest, approved: u32) -> PullRequestWithReview {
        PullRequestWithReview {
            pull_request: pr,
            review_summary: ReviewSummary {
                total_reviews: approved,
                approved,
                changes_requested: 0,
                pending: if approved == 0 { 1 } else { 0 },
                reviewers: vec!["reviewer".to_string()],
            },
            workspace: None,
        }
    }

    fn empty_data() -> DashboardData {
        DashboardData {
            review_requests: vec![],
            my_pull_requests: vec![],
            assigned_issues: vec![],
            recent_activity: vec![],
            workspaces: vec![],
            synced_at: Some("2026-03-29T10:00:00Z".to_string()),
        }
    }

    #[tokio::test]
    async fn test_notify_new_review_request() {
        let (pool, _tmp) = test_pool().await;
        let sender = MockSender::default();

        let pr = make_pr("pr-1", 42, CiStatus::Success);
        let data = DashboardData {
            review_requests: vec![make_pr_with_review(pr, 0)],
            ..empty_data()
        };

        let count = check_and_notify(&pool, &sender, &data).await.unwrap();

        assert_eq!(count, 1);
        let notifs = sender.review_requests.lock().expect("lock poisoned");
        assert_eq!(notifs.len(), 1);
        assert_eq!(notifs[0].pr_number, 42);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_no_duplicate_notification() {
        let (pool, _tmp) = test_pool().await;
        let sender = MockSender::default();

        let pr = make_pr("pr-1", 42, CiStatus::Success);
        let data = DashboardData {
            review_requests: vec![make_pr_with_review(pr, 0)],
            ..empty_data()
        };

        // First call — should notify
        let count1 = check_and_notify(&pool, &sender, &data).await.unwrap();
        assert_eq!(count1, 1);

        // Second call with same data — should NOT re-notify (atomic dedup)
        let count2 = check_and_notify(&pool, &sender, &data).await.unwrap();
        assert_eq!(count2, 0);

        // Verify only one notification total
        assert_eq!(
            sender.review_requests.lock().expect("lock poisoned").len(),
            1
        );

        pool.close().await;
    }

    #[tokio::test]
    async fn test_notify_ci_failure() {
        let (pool, _tmp) = test_pool().await;
        let sender = MockSender::default();

        let pr = make_pr("pr-2", 99, CiStatus::Failure);
        let data = DashboardData {
            my_pull_requests: vec![make_pr_with_review(pr, 0)],
            ..empty_data()
        };

        let count = check_and_notify(&pool, &sender, &data).await.unwrap();

        assert_eq!(count, 1);
        let notifs = sender.ci_failures.lock().expect("lock poisoned");
        assert_eq!(notifs.len(), 1);
        assert_eq!(notifs[0].pr_number, 99);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_notify_pr_approved() {
        let (pool, _tmp) = test_pool().await;
        let sender = MockSender::default();

        let pr = make_pr("pr-3", 10, CiStatus::Success);
        let data = DashboardData {
            my_pull_requests: vec![make_pr_with_review(pr, 1)],
            ..empty_data()
        };

        let count = check_and_notify(&pool, &sender, &data).await.unwrap();

        assert_eq!(count, 1);
        let notifs = sender.pr_approvals.lock().expect("lock poisoned");
        assert_eq!(notifs.len(), 1);
        assert_eq!(notifs[0].pr_number, 10);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_no_notification_for_existing_event() {
        let (pool, _tmp) = test_pool().await;
        let sender = MockSender::default();

        // Pre-claim using pr.id
        try_claim_notification(&pool, "review_request", "pr-1")
            .await
            .unwrap();

        let pr = make_pr("pr-1", 42, CiStatus::Success);
        let data = DashboardData {
            review_requests: vec![make_pr_with_review(pr, 0)],
            ..empty_data()
        };

        let count = check_and_notify(&pool, &sender, &data).await.unwrap();

        assert_eq!(count, 0, "should not notify for already-claimed event");
        assert!(
            sender
                .review_requests
                .lock()
                .expect("lock poisoned")
                .is_empty(),
            "no notification should be emitted"
        );

        pool.close().await;
    }

    #[tokio::test]
    async fn test_ci_failure_renotifies_after_recovery() {
        let (pool, _tmp) = test_pool().await;
        let sender = MockSender::default();

        // 1. CI fails → notify
        let pr_fail = make_pr("pr-4", 50, CiStatus::Failure);
        let data_fail = DashboardData {
            my_pull_requests: vec![make_pr_with_review(pr_fail, 0)],
            ..empty_data()
        };
        let count = check_and_notify(&pool, &sender, &data_fail).await.unwrap();
        assert_eq!(count, 1);

        // 2. CI passes → clears the dedup entry
        let pr_pass = make_pr("pr-4", 50, CiStatus::Success);
        let data_pass = DashboardData {
            my_pull_requests: vec![make_pr_with_review(pr_pass, 0)],
            ..empty_data()
        };
        check_and_notify(&pool, &sender, &data_pass).await.unwrap();

        // Verify entry was cleared
        let still_notified = has_been_notified(&pool, "ci_failure", "pr-4")
            .await
            .unwrap();
        assert!(
            !still_notified,
            "ci_failure entry should be cleared on recovery"
        );

        // 3. CI fails again → should re-notify
        let pr_fail_again = make_pr("pr-4", 50, CiStatus::Failure);
        let data_fail_again = DashboardData {
            my_pull_requests: vec![make_pr_with_review(pr_fail_again, 0)],
            ..empty_data()
        };
        let count2 = check_and_notify(&pool, &sender, &data_fail_again)
            .await
            .unwrap();
        assert_eq!(count2, 1, "should re-notify after CI recovery + re-failure");

        assert_eq!(sender.ci_failures.lock().expect("lock poisoned").len(), 2);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_pr_approved_renotifies_after_changes_requested() {
        let (pool, _tmp) = test_pool().await;
        let sender = MockSender::default();

        // 1. PR approved → notify
        let pr_approved = make_pr("pr-5", 60, CiStatus::Success);
        let data_approved = DashboardData {
            my_pull_requests: vec![make_pr_with_review(pr_approved, 1)],
            ..empty_data()
        };
        let count = check_and_notify(&pool, &sender, &data_approved)
            .await
            .unwrap();
        assert_eq!(count, 1);

        // 2. Changes requested → approved drops to 0, clears dedup entry
        let pr_changes = make_pr("pr-5", 60, CiStatus::Success);
        let data_changes = DashboardData {
            my_pull_requests: vec![make_pr_with_review(pr_changes, 0)],
            ..empty_data()
        };
        check_and_notify(&pool, &sender, &data_changes)
            .await
            .unwrap();

        // Verify entry was cleared
        let still_notified = has_been_notified(&pool, "pr_approved", "pr-5")
            .await
            .unwrap();
        assert!(
            !still_notified,
            "pr_approved entry should be cleared when no longer approved"
        );

        // 3. Re-approved → should re-notify
        let pr_reapproved = make_pr("pr-5", 60, CiStatus::Success);
        let data_reapproved = DashboardData {
            my_pull_requests: vec![make_pr_with_review(pr_reapproved, 1)],
            ..empty_data()
        };
        let count2 = check_and_notify(&pool, &sender, &data_reapproved)
            .await
            .unwrap();
        assert_eq!(count2, 1, "should re-notify after re-approval");

        assert_eq!(sender.pr_approvals.lock().expect("lock poisoned").len(), 2);

        pool.close().await;
    }
}
