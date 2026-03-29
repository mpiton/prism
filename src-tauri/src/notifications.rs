use log::info;
use serde::Serialize;
use sqlx::SqlitePool;

use crate::cache::notifications::{has_been_notified, mark_notified};
use crate::error::AppError;
use crate::types::{CiStatus, DashboardData, DashboardStats, PullRequest};

/// Payload emitted with each notification Tauri event.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationPayload {
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

    fn send_native(&self, title: &str, body: &str) {
        if let Err(e) = notify_rust::Notification::new()
            .appname("PRism")
            .summary(title)
            .body(body)
            .show()
        {
            log::warn!("failed to show native notification: {e}");
        }
    }
}

impl NotificationSender for TauriNotificationSender {
    fn emit_review_request(&self, payload: &NotificationPayload) {
        use tauri::Emitter;
        if let Err(e) = self.app_handle.emit("notification:review_request", payload) {
            log::warn!("failed to emit notification:review_request: {e}");
        }
        self.send_native(
            "New Review Request",
            &format!("PR #{}: {}", payload.pr_number, payload.pr_title),
        );
    }

    fn emit_ci_failure(&self, payload: &NotificationPayload) {
        use tauri::Emitter;
        if let Err(e) = self.app_handle.emit("notification:ci_failure", payload) {
            log::warn!("failed to emit notification:ci_failure: {e}");
        }
        self.send_native(
            "CI Failed",
            &format!("PR #{}: {}", payload.pr_number, payload.pr_title),
        );
    }

    fn emit_pr_approved(&self, payload: &NotificationPayload) {
        use tauri::Emitter;
        if let Err(e) = self.app_handle.emit("notification:pr_approved", payload) {
            log::warn!("failed to emit notification:pr_approved: {e}");
        }
        self.send_native(
            "PR Approved",
            &format!("PR #{}: {}", payload.pr_number, payload.pr_title),
        );
    }
}

/// Check new dashboard data for notification-worthy events.
///
/// Compares `old_stats` vs `new_stats` and scans `new_data` for events
/// not yet recorded in `notification_log`. For each new event, emits a
/// Tauri event + native notification and marks it as notified to prevent
/// duplicates on subsequent syncs.
///
/// Returns the number of notifications emitted.
pub async fn check_and_notify(
    pool: &SqlitePool,
    sender: &(impl NotificationSender + ?Sized),
    _old_stats: &DashboardStats,
    _new_stats: &DashboardStats,
    new_data: &DashboardData,
) -> Result<u32, AppError> {
    let mut count = 0u32;

    // 1. New review requests
    for pr_with_review in &new_data.review_requests {
        let pr = &pr_with_review.pull_request;
        if !has_been_notified(pool, "review_request", &pr.id).await? {
            mark_notified(pool, "review_request", &pr.id).await?;
            sender.emit_review_request(&NotificationPayload::from_pr(pr));
            count += 1;
        }
    }

    // 2. CI failures on authored PRs
    for pr_with_review in &new_data.my_pull_requests {
        let pr = &pr_with_review.pull_request;
        if pr.ci_status == CiStatus::Failure
            && !has_been_notified(pool, "ci_failure", &pr.id).await?
        {
            mark_notified(pool, "ci_failure", &pr.id).await?;
            sender.emit_ci_failure(&NotificationPayload::from_pr(pr));
            count += 1;
        }
    }

    // 3. PR approvals on authored PRs
    for pr_with_review in &new_data.my_pull_requests {
        let pr = &pr_with_review.pull_request;
        if pr_with_review.review_summary.approved > 0
            && !has_been_notified(pool, "pr_approved", &pr.id).await?
        {
            mark_notified(pool, "pr_approved", &pr.id).await?;
            sender.emit_pr_approved(&NotificationPayload::from_pr(pr));
            count += 1;
        }
    }

    info!("check_and_notify: emitted {count} notification(s)");
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::db::init_db;
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
            self.review_requests.lock().unwrap().push(payload.clone());
        }

        fn emit_ci_failure(&self, payload: &NotificationPayload) {
            self.ci_failures.lock().unwrap().push(payload.clone());
        }

        fn emit_pr_approved(&self, payload: &NotificationPayload) {
            self.pr_approvals.lock().unwrap().push(payload.clone());
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

    fn zero_stats() -> DashboardStats {
        DashboardStats {
            pending_reviews: 0,
            open_prs: 0,
            open_issues: 0,
            active_workspaces: 0,
            unread_activity: 0,
        }
    }

    #[tokio::test]
    async fn test_notify_new_review_request() {
        let (pool, _tmp) = test_pool().await;
        let sender = MockSender::default();

        let old_stats = zero_stats();
        let new_stats = DashboardStats {
            pending_reviews: 1,
            ..zero_stats()
        };
        let pr = make_pr("pr-1", 42, CiStatus::Success);
        let data = DashboardData {
            review_requests: vec![make_pr_with_review(pr, 0)],
            ..empty_data()
        };

        let count = check_and_notify(&pool, &sender, &old_stats, &new_stats, &data)
            .await
            .unwrap();

        assert_eq!(count, 1);
        let notifs = sender.review_requests.lock().unwrap();
        assert_eq!(notifs.len(), 1);
        assert_eq!(notifs[0].pr_number, 42);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_no_duplicate_notification() {
        let (pool, _tmp) = test_pool().await;
        let sender = MockSender::default();

        let old_stats = zero_stats();
        let new_stats = DashboardStats {
            pending_reviews: 1,
            ..zero_stats()
        };
        let pr = make_pr("pr-1", 42, CiStatus::Success);
        let data = DashboardData {
            review_requests: vec![make_pr_with_review(pr, 0)],
            ..empty_data()
        };

        // First call — should notify
        let count1 = check_and_notify(&pool, &sender, &old_stats, &new_stats, &data)
            .await
            .unwrap();
        assert_eq!(count1, 1);

        // Second call with same data — should NOT re-notify (dedup)
        let count2 = check_and_notify(&pool, &sender, &old_stats, &new_stats, &data)
            .await
            .unwrap();
        assert_eq!(count2, 0);

        // Verify only one notification total
        assert_eq!(sender.review_requests.lock().unwrap().len(), 1);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_notify_ci_failure() {
        let (pool, _tmp) = test_pool().await;
        let sender = MockSender::default();

        let old_stats = zero_stats();
        let new_stats = DashboardStats {
            open_prs: 1,
            ..zero_stats()
        };
        let pr = make_pr("pr-2", 99, CiStatus::Failure);
        let data = DashboardData {
            my_pull_requests: vec![make_pr_with_review(pr, 0)],
            ..empty_data()
        };

        let count = check_and_notify(&pool, &sender, &old_stats, &new_stats, &data)
            .await
            .unwrap();

        assert_eq!(count, 1);
        let notifs = sender.ci_failures.lock().unwrap();
        assert_eq!(notifs.len(), 1);
        assert_eq!(notifs[0].pr_number, 99);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_notify_pr_approved() {
        let (pool, _tmp) = test_pool().await;
        let sender = MockSender::default();

        let old_stats = zero_stats();
        let new_stats = DashboardStats {
            open_prs: 1,
            ..zero_stats()
        };
        let pr = make_pr("pr-3", 10, CiStatus::Success);
        let data = DashboardData {
            my_pull_requests: vec![make_pr_with_review(pr, 1)],
            ..empty_data()
        };

        let count = check_and_notify(&pool, &sender, &old_stats, &new_stats, &data)
            .await
            .unwrap();

        assert_eq!(count, 1);
        let notifs = sender.pr_approvals.lock().unwrap();
        assert_eq!(notifs.len(), 1);
        assert_eq!(notifs[0].pr_number, 10);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_no_notification_for_existing_event() {
        let (pool, _tmp) = test_pool().await;
        let sender = MockSender::default();

        // Pre-mark the event as already notified
        mark_notified(&pool, "review_request", "pr-1")
            .await
            .unwrap();

        let old_stats = zero_stats();
        let new_stats = DashboardStats {
            pending_reviews: 1,
            ..zero_stats()
        };
        let pr = make_pr("pr-1", 42, CiStatus::Success);
        let data = DashboardData {
            review_requests: vec![make_pr_with_review(pr, 0)],
            ..empty_data()
        };

        let count = check_and_notify(&pool, &sender, &old_stats, &new_stats, &data)
            .await
            .unwrap();

        assert_eq!(count, 0, "should not notify for already-notified event");
        assert!(
            sender.review_requests.lock().unwrap().is_empty(),
            "no notification should be emitted"
        );

        pool.close().await;
    }
}
