//! Background polling loop for GitHub data synchronization (T-034).
//!
//! Spawns a tokio task that periodically calls [`sync_dashboard`] and
//! emits Tauri events with the results. The poll interval is re-read
//! from the config table on every iteration so changes take effect
//! without restarting the app.

use std::time::Duration;

use log::{info, warn};
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter};

use crate::cache::config::get_config;
use crate::cache::sync::sync_dashboard;
use crate::error::AppError;
use crate::github::client::GitHubClient;
use crate::types::DashboardStats;

// ── Trait for testability ────────────────────────────────────────

/// Abstracts the polling loop's external dependencies.
///
/// Production code uses [`RealPollingContext`]; tests supply a mock.
pub(crate) trait PollingContext: Send + Sync + 'static {
    /// Execute one sync cycle, returning fresh dashboard stats.
    fn sync(&self) -> impl std::future::Future<Output = Result<DashboardStats, AppError>> + Send;

    /// Read the current poll interval from persistent config.
    fn poll_interval_secs(&self) -> impl std::future::Future<Output = u64> + Send;

    /// Notify the frontend of a successful sync.
    fn emit_updated(&self, stats: &DashboardStats);

    /// Notify the frontend of a sync failure.
    fn emit_sync_error(&self, error: &str);
}

// ── Real implementation ──────────────────────────────────────────

#[allow(dead_code)] // TODO(T-035+): remove after wiring polling into app setup
struct RealPollingContext {
    client: GitHubClient,
    pool: SqlitePool,
    username: String,
    app_handle: AppHandle,
}

impl PollingContext for RealPollingContext {
    async fn sync(&self) -> Result<DashboardStats, AppError> {
        sync_dashboard(&self.client, &self.pool, &self.username).await
    }

    async fn poll_interval_secs(&self) -> u64 {
        get_config(&self.pool)
            .await
            .map_or(300, |c| c.poll_interval_secs)
    }

    fn emit_updated(&self, stats: &DashboardStats) {
        if let Err(e) = self.app_handle.emit("github:updated", stats) {
            warn!("failed to emit github:updated: {e}");
        }
    }

    fn emit_sync_error(&self, error: &str) {
        if let Err(e) = self.app_handle.emit("github:sync_error", error) {
            warn!("failed to emit github:sync_error: {e}");
        }
    }
}

// ── Core loop ────────────────────────────────────────────────────

/// Execute a single polling iteration: sync then emit.
///
/// Returns `true` on success, `false` on error.
pub(crate) async fn poll_once(ctx: &(impl PollingContext + ?Sized)) -> bool {
    match ctx.sync().await {
        Ok(stats) => {
            info!(
                "polling sync complete: {} pending reviews",
                stats.pending_reviews
            );
            ctx.emit_updated(&stats);
            true
        }
        Err(e) => {
            warn!("polling sync failed: {e}");
            ctx.emit_sync_error(&e.to_string());
            false
        }
    }
}

/// Run the polling loop until the task is cancelled.
///
/// Syncs immediately on start, then sleeps for the configured interval.
#[allow(dead_code)] // TODO(T-035+): remove after wiring polling into app setup
async fn polling_loop(ctx: impl PollingContext) {
    loop {
        poll_once(&ctx).await;
        let interval = ctx.poll_interval_secs().await;
        tokio::time::sleep(Duration::from_secs(interval)).await;
    }
}

/// Spawn the background polling task.
///
/// The returned [`JoinHandle`] can be used to cancel the loop via
/// `handle.abort()` (e.g. on app shutdown or token change).
#[allow(dead_code)] // TODO(T-035+): remove after wiring polling into app setup
#[must_use = "dropping the handle makes the polling loop uncancellable"]
pub(crate) fn start_polling(
    app_handle: AppHandle,
    pool: SqlitePool,
    client: GitHubClient,
    username: String,
) -> tokio::task::JoinHandle<()> {
    info!("starting background polling for {username}");
    let ctx = RealPollingContext {
        client,
        pool,
        username,
        app_handle,
    };
    tokio::spawn(polling_loop(ctx))
}

// ── Tests ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    /// Collects events emitted during a test run.
    #[derive(Debug, Default, Clone)]
    struct Recorder {
        updates: Arc<Mutex<Vec<DashboardStats>>>,
        errors: Arc<Mutex<Vec<String>>>,
    }

    /// Mock context that returns pre-configured sync results.
    struct MockCtx {
        results: Mutex<VecDeque<Result<DashboardStats, AppError>>>,
        recorder: Recorder,
    }

    impl MockCtx {
        fn new(results: Vec<Result<DashboardStats, AppError>>) -> (Self, Recorder) {
            let recorder = Recorder::default();
            let ctx = Self {
                results: Mutex::new(results.into()),
                recorder: recorder.clone(),
            };
            (ctx, recorder)
        }
    }

    impl PollingContext for MockCtx {
        async fn sync(&self) -> Result<DashboardStats, AppError> {
            self.results
                .lock()
                .expect("lock poisoned")
                .pop_front()
                .expect("MockCtx: no more results configured for this test")
        }

        async fn poll_interval_secs(&self) -> u64 {
            1
        }

        fn emit_updated(&self, stats: &DashboardStats) {
            self.recorder
                .updates
                .lock()
                .expect("lock poisoned")
                .push(stats.clone());
        }

        fn emit_sync_error(&self, error: &str) {
            self.recorder
                .errors
                .lock()
                .expect("lock poisoned")
                .push(error.to_string());
        }
    }

    fn make_stats(pending: u32) -> DashboardStats {
        DashboardStats {
            pending_reviews: pending,
            open_prs: 5,
            open_issues: 2,
            active_workspaces: 1,
            unread_activity: 0,
        }
    }

    #[tokio::test]
    async fn test_polling_executes_sync() {
        let stats = make_stats(3);
        let (ctx, recorder) = MockCtx::new(vec![Ok(stats.clone())]);

        let ok = poll_once(&ctx).await;

        assert!(ok, "poll_once should return true on success");
        let updates = recorder.updates.lock().unwrap();
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0], stats);
        assert!(
            recorder.errors.lock().unwrap().is_empty(),
            "no errors should be emitted on success"
        );
    }

    #[tokio::test]
    async fn test_polling_emits_event() {
        let s1 = make_stats(2);
        let s2 = make_stats(5);
        let (ctx, recorder) = MockCtx::new(vec![Ok(s1.clone()), Ok(s2.clone())]);

        poll_once(&ctx).await;
        poll_once(&ctx).await;

        let updates = recorder.updates.lock().unwrap();
        assert_eq!(updates.len(), 2, "should emit one event per sync");
        assert_eq!(updates[0], s1);
        assert_eq!(updates[1], s2);
    }

    #[tokio::test]
    async fn test_polling_handles_sync_error() {
        let err = AppError::GitHub("rate limit exceeded".into());
        let (ctx, recorder) = MockCtx::new(vec![Err(err)]);

        let ok = poll_once(&ctx).await;

        assert!(!ok, "poll_once should return false on error");
        let errors = recorder.errors.lock().unwrap();
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0].contains("rate limit exceeded"),
            "error message should contain the original error"
        );
        assert!(
            recorder.updates.lock().unwrap().is_empty(),
            "no updates should be emitted on error"
        );
    }
}
