use chrono::{Duration, SecondsFormat, Utc};
use sqlx::SqlitePool;
use tauri::Manager;
use tracing::{info, warn};

use crate::commands::{PtyManagerState, workspace_cleanup_inner, workspace_suspend_inner};
use crate::error::AppError;

/// Find active workspaces whose idle time exceeds `auto_suspend_minutes`.
///
/// Uses `last_active_at` (with `updated_at` as fallback for workspaces that
/// never received PTY input) to determine idle time.
pub(crate) async fn find_expired_active_workspaces(
    pool: &SqlitePool,
    auto_suspend_minutes: u64,
) -> Result<Vec<String>, AppError> {
    // Cap at ~100 years to avoid Duration overflow (i64 * 60_000_000_000 ns).
    const MAX_SUSPEND_MINUTES: i64 = 365 * 24 * 60 * 100;

    let minutes = i64::try_from(auto_suspend_minutes)
        .unwrap_or(MAX_SUSPEND_MINUTES)
        .min(MAX_SUSPEND_MINUTES);
    let cutoff =
        (Utc::now() - Duration::minutes(minutes)).to_rfc3339_opts(SecondsFormat::Millis, true);

    let ids: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM workspaces \
         WHERE state = 'active' \
         AND COALESCE(last_active_at, updated_at) < $1",
    )
    .bind(&cutoff)
    .fetch_all(pool)
    .await?;

    Ok(ids.into_iter().map(|(id,)| id).collect())
}

/// Suspend active workspaces that have exceeded the idle timeout.
///
/// Returns the IDs of workspaces that were successfully suspended.
pub(crate) async fn auto_suspend_expired(
    pool: &SqlitePool,
    pty_state: &PtyManagerState,
    auto_suspend_minutes: u64,
) -> Result<Vec<String>, AppError> {
    let expired_ids = find_expired_active_workspaces(pool, auto_suspend_minutes).await?;

    let mut suspended_ids = Vec::new();
    for ws_id in &expired_ids {
        match workspace_suspend_inner(pool, pty_state, ws_id).await {
            Ok(_) => {
                info!("lifecycle: auto-suspended idle workspace '{ws_id}'");
                suspended_ids.push(ws_id.clone());
            }
            Err(e) => {
                warn!("lifecycle: failed to auto-suspend workspace '{ws_id}': {e}");
            }
        }
    }

    Ok(suspended_ids)
}

/// Enforce the maximum number of active workspaces (LRU eviction).
///
/// Counts active workspaces and, if the count exceeds `max`, suspends
/// the oldest ones (by `last_active_at` ASC, falling back to `updated_at`)
/// until the active count is within the limit.
///
/// Returns the IDs of workspaces that were successfully suspended.
pub(crate) async fn enforce_max_active(
    pool: &SqlitePool,
    pty_state: &PtyManagerState,
    max: u32,
) -> Result<Vec<String>, AppError> {
    let max = max as usize;

    // Oldest first — candidates for eviction are at the front.
    let candidates: Vec<(String,)> = sqlx::query_as(
        "SELECT id FROM workspaces \
         WHERE state = 'active' \
         ORDER BY COALESCE(last_active_at, updated_at) ASC",
    )
    .fetch_all(pool)
    .await?;

    if candidates.len() <= max {
        return Ok(Vec::new());
    }

    let to_evict = candidates.len() - max;
    let mut suspended_ids = Vec::new();

    for (ws_id,) in candidates.into_iter().take(to_evict) {
        match workspace_suspend_inner(pool, pty_state, &ws_id).await {
            Ok(_) => {
                info!("lifecycle: LRU-evicted workspace '{ws_id}'");
                suspended_ids.push(ws_id);
            }
            Err(e) => {
                warn!("lifecycle: failed to LRU-evict workspace '{ws_id}': {e}");
            }
        }
    }

    Ok(suspended_ids)
}

/// Run one lifecycle tick: auto-suspend idle workspaces and auto-archive
/// workspaces whose PRs have been merged/closed past the configured delay.
///
/// Returns `(suspended_ids, archived_ids)`.
#[tracing::instrument(skip(pool, pty_state))]
pub(crate) async fn run_lifecycle_tick(
    pool: &SqlitePool,
    pty_state: &PtyManagerState,
    auto_suspend_minutes: u64,
    archive_delay_hours: u64,
    archive_delay_closed_hours: u64,
) -> (Vec<String>, Vec<String>) {
    let suspended = auto_suspend_expired(pool, pty_state, auto_suspend_minutes)
        .await
        .unwrap_or_else(|e| {
            warn!("lifecycle: auto-suspend check failed: {e}");
            Vec::new()
        });

    let archived = workspace_cleanup_inner(
        pool,
        pty_state,
        archive_delay_hours,
        archive_delay_closed_hours,
    )
    .await
    .unwrap_or_else(|e| {
        warn!("lifecycle: auto-archive check failed: {e}");
        Vec::new()
    });

    (suspended, archived)
}

/// Interval between lifecycle ticks in seconds.
const LIFECYCLE_INTERVAL_SECS: u64 = 60;

/// Start the background lifecycle task. Runs every 60 seconds.
///
/// Each tick reads the current config from the database, so changes to
/// `auto_suspend_minutes` or `archive_delay_*` take effect without restart.
///
/// Emits `workspace:state_changed` for each suspended/archived workspace
/// so the frontend stays in sync.
pub fn start_workspace_lifecycle(
    app_handle: tauri::AppHandle,
    pool: SqlitePool,
) -> tauri::async_runtime::JoinHandle<()> {
    use tauri::Emitter;

    tauri::async_runtime::spawn(async move {
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(LIFECYCLE_INTERVAL_SECS));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // Skip the immediate first tick — let the app finish startup.
        interval.tick().await;

        loop {
            interval.tick().await;

            let pty_state: tauri::State<'_, PtyManagerState> = app_handle.state();

            let config = match crate::cache::config::get_config(&pool).await {
                Ok(c) => c,
                Err(e) => {
                    warn!("lifecycle: failed to read config: {e}");
                    continue;
                }
            };

            let (suspended, archived) = run_lifecycle_tick(
                &pool,
                &pty_state,
                config.auto_suspend_minutes,
                config.archive_delay_hours,
                config.archive_delay_closed_hours,
            )
            .await;

            // Emit structured payloads matching the WorkspaceStateChanged contract.
            // If a workspace was suspended and then archived in the same tick,
            // skip the Suspended event — only emit the final Archived state.
            for ws_id in &suspended {
                if archived.contains(ws_id) {
                    continue;
                }
                let payload = crate::types::WorkspaceStateChanged {
                    workspace_id: ws_id.clone(),
                    new_state: crate::types::WorkspaceState::Suspended,
                };
                if let Err(e) = app_handle.emit("workspace:state_changed", &payload) {
                    warn!("lifecycle: failed to emit state_changed for '{ws_id}': {e}");
                }
            }
            for ws_id in &archived {
                let payload = crate::types::WorkspaceStateChanged {
                    workspace_id: ws_id.clone(),
                    new_state: crate::types::WorkspaceState::Archived,
                };
                if let Err(e) = app_handle.emit("workspace:state_changed", &payload) {
                    warn!("lifecycle: failed to emit state_changed for '{ws_id}': {e}");
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::db::init_db;
    use crate::cache::repos::upsert_repo;
    use crate::cache::workspaces::{create_workspace, get_workspace, update_last_active};
    use crate::types::{Repo, Workspace, WorkspaceState};

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

    fn sample_workspace(id: &str, pr_number: u32) -> Workspace {
        Workspace {
            id: id.to_string(),
            repo_id: "repo-1".to_string(),
            pull_request_number: pr_number,
            state: WorkspaceState::Active,
            worktree_path: Some(format!(
                "/home/user/.prism/workspaces/prism/worktrees/pr-{pr_number}"
            )),
            session_id: None,
            created_at: "2026-03-20T10:00:00Z".to_string(),
            updated_at: "2026-03-20T10:00:00Z".to_string(),
        }
    }

    async fn insert_test_pr(
        pool: &SqlitePool,
        id: &str,
        number: u32,
        state: crate::types::PrState,
        updated_at: &str,
    ) {
        use crate::types::{CiStatus, Priority, PullRequest};
        let pr = PullRequest {
            id: id.to_string(),
            number,
            title: format!("PR #{number}"),
            author: "alice".to_string(),
            state,
            ci_status: CiStatus::Success,
            priority: Priority::Medium,
            repo_id: "repo-1".to_string(),
            url: format!("https://github.com/mpiton/prism/pull/{number}"),
            labels: vec![],
            additions: 10,
            deletions: 5,
            created_at: "2026-03-01T00:00:00Z".to_string(),
            updated_at: updated_at.to_string(),
        };
        crate::cache::pull_requests::upsert_pull_request(pool, &pr)
            .await
            .unwrap();
    }

    // ── Auto-suspend tests ───────────────────────────────────────────

    #[tokio::test]
    async fn test_auto_suspend_after_timeout() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let ws = sample_workspace("ws-1", 42);
        create_workspace(&pool, &ws).await.unwrap();

        // Set last_active_at to 60 minutes ago (well past 30-minute timeout)
        let old_ts =
            (Utc::now() - Duration::minutes(60)).to_rfc3339_opts(SecondsFormat::Millis, true);
        sqlx::query("UPDATE workspaces SET last_active_at = $1 WHERE id = $2")
            .bind(&old_ts)
            .bind("ws-1")
            .execute(&pool)
            .await
            .unwrap();

        let pty_state = PtyManagerState::new();
        let suspended = auto_suspend_expired(&pool, &pty_state, 30).await.unwrap();

        assert_eq!(suspended.len(), 1);
        assert_eq!(suspended[0], "ws-1");

        let ws = get_workspace(&pool, "ws-1").await.unwrap().unwrap();
        assert_eq!(ws.state, WorkspaceState::Suspended);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_no_suspend_if_active() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let ws = sample_workspace("ws-1", 42);
        create_workspace(&pool, &ws).await.unwrap();

        // Touch last_active_at to now — workspace is active
        update_last_active(&pool, "ws-1").await.unwrap();

        let pty_state = PtyManagerState::new();
        let suspended = auto_suspend_expired(&pool, &pty_state, 30).await.unwrap();

        assert!(
            suspended.is_empty(),
            "should not suspend recently active workspace"
        );

        let ws = get_workspace(&pool, "ws-1").await.unwrap().unwrap();
        assert_eq!(ws.state, WorkspaceState::Active);

        pool.close().await;
    }

    // ── LRU eviction tests ─────────────────────────────────────────

    #[tokio::test]
    async fn test_lru_no_eviction_under_limit() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        // 2 active workspaces, max = 3 → no eviction
        create_workspace(&pool, &sample_workspace("ws-1", 42))
            .await
            .unwrap();
        create_workspace(&pool, &sample_workspace("ws-2", 43))
            .await
            .unwrap();

        let ts1 = (Utc::now() - Duration::minutes(10)).to_rfc3339_opts(SecondsFormat::Millis, true);
        let ts2 = (Utc::now() - Duration::minutes(5)).to_rfc3339_opts(SecondsFormat::Millis, true);
        sqlx::query("UPDATE workspaces SET last_active_at = $1 WHERE id = $2")
            .bind(&ts1)
            .bind("ws-1")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE workspaces SET last_active_at = $1 WHERE id = $2")
            .bind(&ts2)
            .bind("ws-2")
            .execute(&pool)
            .await
            .unwrap();

        let pty_state = PtyManagerState::new();
        let evicted = enforce_max_active(&pool, &pty_state, 3).await.unwrap();

        assert!(evicted.is_empty(), "should not evict when under limit");
        assert_eq!(
            get_workspace(&pool, "ws-1").await.unwrap().unwrap().state,
            WorkspaceState::Active
        );
        assert_eq!(
            get_workspace(&pool, "ws-2").await.unwrap().unwrap().state,
            WorkspaceState::Active
        );

        pool.close().await;
    }

    #[tokio::test]
    async fn test_lru_evicts_oldest() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        // 4 active workspaces, max = 3 → evict 1 (the oldest)
        for (id, pr) in [("ws-1", 42), ("ws-2", 43), ("ws-3", 44), ("ws-4", 45)] {
            create_workspace(&pool, &sample_workspace(id, pr))
                .await
                .unwrap();
        }

        // ws-1: 40 min ago (oldest), ws-2: 30, ws-3: 20, ws-4: 10
        for (id, mins) in [("ws-1", 40), ("ws-2", 30), ("ws-3", 20), ("ws-4", 10)] {
            let ts =
                (Utc::now() - Duration::minutes(mins)).to_rfc3339_opts(SecondsFormat::Millis, true);
            sqlx::query("UPDATE workspaces SET last_active_at = $1 WHERE id = $2")
                .bind(&ts)
                .bind(id)
                .execute(&pool)
                .await
                .unwrap();
        }

        let pty_state = PtyManagerState::new();
        let evicted = enforce_max_active(&pool, &pty_state, 3).await.unwrap();

        assert_eq!(evicted.len(), 1);
        assert_eq!(evicted[0], "ws-1");

        assert_eq!(
            get_workspace(&pool, "ws-1").await.unwrap().unwrap().state,
            WorkspaceState::Suspended
        );
        // Others remain active
        for id in ["ws-2", "ws-3", "ws-4"] {
            assert_eq!(
                get_workspace(&pool, id).await.unwrap().unwrap().state,
                WorkspaceState::Active
            );
        }

        pool.close().await;
    }

    #[tokio::test]
    async fn test_lru_evicts_multiple() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        // 5 active workspaces, max = 3 → evict 2 (the 2 oldest)
        for (id, pr) in [
            ("ws-1", 42),
            ("ws-2", 43),
            ("ws-3", 44),
            ("ws-4", 45),
            ("ws-5", 46),
        ] {
            create_workspace(&pool, &sample_workspace(id, pr))
                .await
                .unwrap();
        }

        // ws-1: 50 min ago (oldest), ws-2: 40, ws-3: 30, ws-4: 20, ws-5: 10
        for (id, mins) in [
            ("ws-1", 50),
            ("ws-2", 40),
            ("ws-3", 30),
            ("ws-4", 20),
            ("ws-5", 10),
        ] {
            let ts =
                (Utc::now() - Duration::minutes(mins)).to_rfc3339_opts(SecondsFormat::Millis, true);
            sqlx::query("UPDATE workspaces SET last_active_at = $1 WHERE id = $2")
                .bind(&ts)
                .bind(id)
                .execute(&pool)
                .await
                .unwrap();
        }

        let pty_state = PtyManagerState::new();
        let evicted = enforce_max_active(&pool, &pty_state, 3).await.unwrap();

        assert_eq!(evicted.len(), 2);
        assert!(evicted.contains(&"ws-1".to_string()));
        assert!(evicted.contains(&"ws-2".to_string()));

        // ws-1 and ws-2 suspended
        for id in ["ws-1", "ws-2"] {
            assert_eq!(
                get_workspace(&pool, id).await.unwrap().unwrap().state,
                WorkspaceState::Suspended
            );
        }
        // ws-3, ws-4, ws-5 remain active
        for id in ["ws-3", "ws-4", "ws-5"] {
            assert_eq!(
                get_workspace(&pool, id).await.unwrap().unwrap().state,
                WorkspaceState::Active
            );
        }

        pool.close().await;
    }

    #[tokio::test]
    async fn test_lru_evicts_all_when_max_zero() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        for (id, pr) in [("ws-1", 42), ("ws-2", 43)] {
            create_workspace(&pool, &sample_workspace(id, pr))
                .await
                .unwrap();
        }
        for (id, mins) in [("ws-1", 20), ("ws-2", 10)] {
            let ts =
                (Utc::now() - Duration::minutes(mins)).to_rfc3339_opts(SecondsFormat::Millis, true);
            sqlx::query("UPDATE workspaces SET last_active_at = $1 WHERE id = $2")
                .bind(&ts)
                .bind(id)
                .execute(&pool)
                .await
                .unwrap();
        }

        let pty_state = PtyManagerState::new();
        let evicted = enforce_max_active(&pool, &pty_state, 0).await.unwrap();

        assert_eq!(evicted.len(), 2);
        for id in ["ws-1", "ws-2"] {
            assert_eq!(
                get_workspace(&pool, id).await.unwrap().unwrap().state,
                WorkspaceState::Suspended
            );
        }

        pool.close().await;
    }

    #[tokio::test]
    async fn test_lru_falls_back_to_updated_at_when_last_active_null() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        // Create workspaces with different updated_at but NULL last_active_at
        for (id, pr) in [("ws-1", 42), ("ws-2", 43), ("ws-3", 44)] {
            create_workspace(&pool, &sample_workspace(id, pr))
                .await
                .unwrap();
        }

        // Set updated_at directly (last_active_at stays NULL)
        // ws-1 oldest, ws-3 newest
        for (id, mins) in [("ws-1", 30), ("ws-2", 20), ("ws-3", 10)] {
            let ts =
                (Utc::now() - Duration::minutes(mins)).to_rfc3339_opts(SecondsFormat::Millis, true);
            sqlx::query(
                "UPDATE workspaces SET updated_at = $1, last_active_at = NULL WHERE id = $2",
            )
            .bind(&ts)
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        }

        let pty_state = PtyManagerState::new();
        let evicted = enforce_max_active(&pool, &pty_state, 2).await.unwrap();

        assert_eq!(evicted.len(), 1);
        assert_eq!(
            evicted[0], "ws-1",
            "should evict oldest by updated_at fallback"
        );
        assert_eq!(
            get_workspace(&pool, "ws-1").await.unwrap().unwrap().state,
            WorkspaceState::Suspended
        );

        pool.close().await;
    }

    // ── Auto-archive via lifecycle tick ───────────────────────────────

    #[tokio::test]
    async fn test_auto_archive_merged_pr() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        // Merged PR with old updated_at (well past 24h delay)
        insert_test_pr(
            &pool,
            "pr-1",
            42,
            crate::types::PrState::Merged,
            "2026-03-01T00:00:00Z",
        )
        .await;

        // Suspended workspace linked to this PR
        let mut ws = sample_workspace("ws-1", 42);
        ws.state = WorkspaceState::Suspended;
        ws.worktree_path = None;
        create_workspace(&pool, &ws).await.unwrap();

        let pty_state = PtyManagerState::new();
        let (_, archived) = run_lifecycle_tick(&pool, &pty_state, 30, 24, 48).await;

        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0], "ws-1");

        let ws = get_workspace(&pool, "ws-1").await.unwrap().unwrap();
        assert_eq!(ws.state, WorkspaceState::Archived);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_auto_archive_respects_delay() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        // Merged PR with very recent updated_at (within the 24h delay)
        let recent = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        insert_test_pr(&pool, "pr-1", 42, crate::types::PrState::Merged, &recent).await;

        let mut ws = sample_workspace("ws-1", 42);
        ws.state = WorkspaceState::Suspended;
        ws.worktree_path = None;
        create_workspace(&pool, &ws).await.unwrap();

        let pty_state = PtyManagerState::new();
        let (_, archived) = run_lifecycle_tick(&pool, &pty_state, 30, 24, 48).await;

        assert!(
            archived.is_empty(),
            "should not archive workspace within delay"
        );

        let ws = get_workspace(&pool, "ws-1").await.unwrap().unwrap();
        assert_eq!(
            ws.state,
            WorkspaceState::Suspended,
            "workspace should remain suspended"
        );

        pool.close().await;
    }
}
