use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use sqlx::SqlitePool;
use tauri::{Emitter, Manager};
use tracing::{info, warn};

use crate::cache::activity::{mark_all_read, mark_read};
use crate::cache::config::{get_config, set_config};
use crate::cache::dashboard::{assemble_dashboard_data, compute_dashboard_stats};
use crate::cache::repos::{get_repo, list_repos, set_local_path, set_repo_enabled};
use crate::cache::stats::compute_personal_stats;
use crate::cache::sync::sync_dashboard;
use crate::cache::workspaces::{get_notes, list_workspaces, update_workspace_state};
use crate::error::AppError;
use crate::github::auth;
use crate::github::client::GitHubClient;
use crate::types::{
    AppConfig, ClaudeSessionPayload, DashboardData, DashboardStats, MemoryStats,
    OpenWorkspaceRequest, OpenWorkspaceResponse, PartialAppConfig, PersonalStats, Repo, Workspace,
    WorkspaceListEntry, WorkspaceNote, WorkspaceState, merge_partial_config,
};
use crate::workspace::pty::PtyManager;
use crate::workspace::worktree;

/// Authentication status returned by [`auth_get_status`].
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub connected: bool,
    pub username: Option<String>,
    /// Non-null when a transient error (network, rate-limit) prevents validation.
    pub error: Option<String>,
}

/// Maps a token validation result to an [`AuthStatus`].
///
/// - `Ok(username)` → connected with username
/// - `Err(AppError::Auth(_))` → disconnected (invalid/expired token)
/// - `Err(other)` → disconnected with error detail (transient failure)
fn status_from_validation(result: Result<String, AppError>) -> AuthStatus {
    match result {
        Ok(username) => AuthStatus {
            connected: true,
            username: Some(username),
            error: None,
        },
        Err(AppError::Auth(_)) => AuthStatus {
            connected: false,
            username: None,
            error: None,
        },
        Err(e) => AuthStatus {
            connected: false,
            username: None,
            error: Some(e.to_string()),
        },
    }
}

/// Core token validation and storage logic, callable from tests without Tauri state.
async fn set_token_inner(token: String) -> Result<String, String> {
    let token = token.trim().to_string();
    let username = auth::validate_token(&token).await.map_err(String::from)?;
    tokio::task::spawn_blocking(move || auth::store_token(&token))
        .await
        .map_err(|e| format!("task join error: {e}"))?
        .map_err(String::from)?;
    Ok(username)
}

/// Validates and stores a GitHub token. Returns the authenticated username.
///
/// Also updates the cached username so subsequent dashboard calls use the
/// new identity without re-validating the token. Restarts the background
/// polling loop and emits `auth:restored` so the frontend can recover from
/// an `auth:expired` state.
#[tauri::command]
pub async fn auth_set_token(
    token: String,
    cached: tauri::State<'_, GithubUsername>,
    app_handle: tauri::AppHandle,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<String, String> {
    let username = set_token_inner(token).await?;
    match cached.0.lock() {
        Ok(mut guard) => *guard = Some(username.clone()),
        Err(e) => warn!("failed to update cached username: {e}"),
    }

    // Restart polling with the new token and notify the frontend
    crate::try_start_polling(app_handle.clone(), pool.inner().clone()).await;
    if let Err(e) = app_handle.emit("auth:restored", &username) {
        warn!("failed to emit auth:restored: {e}");
    }

    Ok(username)
}

/// Returns the current authentication status.
///
/// **Note:** this command performs a live HTTP request to the GitHub API
/// to validate the stored token. Callers should debounce or cache results
/// (e.g. via `TanStack Query` `staleTime`) to avoid excessive API usage.
///
/// Auth errors (invalid/expired token) return `connected: false`.
/// Transient errors (network, rate-limit, keychain) return `connected: false`
/// with an `error` field so the frontend can distinguish offline from
/// logged-out.
#[tauri::command]
pub async fn auth_get_status() -> Result<AuthStatus, String> {
    let token: Option<String> = match tokio::task::spawn_blocking(auth::get_token).await {
        Ok(Ok(t)) => t,
        Ok(Err(e)) => {
            return Ok(AuthStatus {
                connected: false,
                username: None,
                error: Some(e.to_string()),
            });
        }
        Err(e) => {
            return Ok(AuthStatus {
                connected: false,
                username: None,
                error: Some(format!("task join error: {e}")),
            });
        }
    };
    match token {
        Some(ref t) => Ok(status_from_validation(auth::validate_token(t).await)),
        None => Ok(AuthStatus {
            connected: false,
            username: None,
            error: None,
        }),
    }
}

/// Deletes the stored GitHub token and clears the cached username (logout).
///
/// Credential deletion runs first; the cache is only cleared after the
/// token is confirmed removed, avoiding an inconsistent state where the
/// cache is empty but the credential still exists.
#[tauri::command]
pub async fn auth_logout(cached: tauri::State<'_, GithubUsername>) -> Result<(), String> {
    tokio::task::spawn_blocking(auth::delete_token)
        .await
        .map_err(|e| format!("task join error: {e}"))?
        .map_err(String::from)?;
    match cached.0.lock() {
        Ok(mut guard) => *guard = None,
        Err(e) => warn!("failed to clear cached username (mutex poisoned): {e}"),
    }
    Ok(())
}

/// Cached GitHub username, populated on first dashboard access.
///
/// Avoids re-validating the token (HTTP call) on every dashboard load.
/// Cleared on logout so subsequent calls fail fast with "not authenticated".
#[derive(Default)]
pub struct GithubUsername(pub(crate) Mutex<Option<String>>);

/// Resolves the authenticated GitHub username.
///
/// Returns the cached value if available; otherwise reads the stored token
/// from the keychain, validates it against the GitHub API, and caches the
/// resulting username for future calls.
async fn resolve_username(cached: &GithubUsername) -> Result<String, String> {
    // Read path: best-effort — if the lock is poisoned, skip the cache
    // and fall through to token re-validation rather than breaking the
    // dashboard permanently.
    if let Ok(guard) = cached.0.lock()
        && let Some(ref u) = *guard
    {
        return Ok(u.clone());
    }

    let token = tokio::task::spawn_blocking(auth::get_token)
        .await
        .map_err(|e| format!("task join error: {e}"))?
        .map_err(String::from)?
        .ok_or_else(|| "not authenticated: no token stored".to_string())?;

    let username = auth::validate_token(&token).await.map_err(String::from)?;

    // Write path: best-effort — the username was already resolved, so a
    // poisoned lock only prevents caching; the next call will re-validate.
    match cached.0.lock() {
        Ok(mut guard) => *guard = Some(username.clone()),
        Err(e) => warn!("failed to cache username (mutex poisoned): {e}"),
    }

    Ok(username)
}

/// Returns the full dashboard data for the authenticated user.
///
/// Reads the DB pool and cached username from Tauri managed state.
/// On the first call (or after logout), validates the stored token to
/// resolve the username, caching it for subsequent calls.
#[tauri::command]
pub async fn github_get_dashboard(
    pool: tauri::State<'_, SqlitePool>,
    cached: tauri::State<'_, GithubUsername>,
) -> Result<DashboardData, String> {
    let username = resolve_username(&cached).await?;
    assemble_dashboard_data(&pool, &username)
        .await
        .map_err(|e| e.to_string())
}

/// Returns dashboard statistics (counts) from the local cache.
///
/// After the first call (which may validate the token via HTTP to resolve
/// the username), subsequent calls are pure DB queries with no network I/O.
/// Use for displaying badge counts, status indicators, etc.
#[tauri::command]
pub async fn github_get_stats(
    pool: tauri::State<'_, SqlitePool>,
    cached: tauri::State<'_, GithubUsername>,
) -> Result<DashboardStats, String> {
    let username = resolve_username(&cached).await?;
    compute_dashboard_stats(&pool, &username)
        .await
        .map_err(|e| e.to_string())
}

/// Returns personal statistics for the authenticated user.
#[tauri::command]
pub async fn stats_personal(
    pool: tauri::State<'_, SqlitePool>,
    cached: tauri::State<'_, GithubUsername>,
) -> Result<PersonalStats, String> {
    let username = resolve_username(&cached).await?;
    compute_personal_stats(&pool, &username)
        .await
        .map_err(|e| e.to_string())
}

/// Resolves the authenticated username and token in a single pass.
///
/// Used by commands that need the raw token (e.g., `github_force_sync`).
/// Reads the token once from the keychain, checks the username cache,
/// and validates only if the cache is empty — avoiding the TOCTOU window
/// of reading the keychain twice.
async fn resolve_credentials(cached: &GithubUsername) -> Result<(String, String), String> {
    let token = tokio::task::spawn_blocking(auth::get_token)
        .await
        .map_err(|e| format!("task join error: {e}"))?
        .map_err(String::from)?
        .ok_or_else(|| "not authenticated: no token stored".to_string())?;

    // Fast path: return cached username + token
    if let Ok(guard) = cached.0.lock()
        && let Some(ref u) = *guard
    {
        return Ok((u.clone(), token));
    }

    // Slow path: validate token, cache username
    let username = auth::validate_token(&token).await.map_err(String::from)?;

    match cached.0.lock() {
        Ok(mut guard) => *guard = Some(username.clone()),
        Err(e) => warn!("failed to cache username (mutex poisoned): {e}"),
    }

    Ok((username, token))
}

/// Core force-sync logic shared by the IPC command and the tray handler.
///
/// Returns the fresh stats so callers can update the tray badge or
/// perform other post-sync work without introducing bidirectional coupling.
pub(crate) async fn run_force_sync(
    app_handle: &tauri::AppHandle,
    pool: &SqlitePool,
    cached: &GithubUsername,
) -> Result<DashboardStats, String> {
    use std::sync::atomic::Ordering;

    /// RAII guard that resets the in-flight flag on drop (cancellation-safe).
    struct ResetOnDrop<'a>(&'a std::sync::atomic::AtomicBool);
    impl Drop for ResetOnDrop<'_> {
        fn drop(&mut self) {
            self.0.store(false, Ordering::Release);
        }
    }

    let sync_guard = app_handle.state::<crate::SyncInFlight>();
    if sync_guard
        .0
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("sync already in progress".to_string());
    }
    let _reset = ResetOnDrop(&sync_guard.0);

    let (username, token) = resolve_credentials(cached).await?;
    let client = GitHubClient::new(&token).map_err(|e| e.to_string())?;
    let stats = sync_dashboard(&client, pool, &username)
        .await
        .map_err(|e| e.to_string())?;
    if let Err(e) = app_handle.emit("github:updated", &stats) {
        warn!("failed to emit github:updated after force sync: {e}");
    }

    // Post-sync: archive workspaces for merged/closed PRs past the configured delay.
    let config = match get_config(pool).await {
        Ok(c) => c,
        Err(e) => {
            warn!("force-sync workspace cleanup: failed to read config: {e}");
            return Ok(stats);
        }
    };
    let pty_state: tauri::State<'_, PtyManagerState> = app_handle.state();
    match workspace_cleanup_inner(
        pool,
        &pty_state,
        config.archive_delay_hours,
        config.archive_delay_closed_hours,
    )
    .await
    {
        Ok(archived_ids) => {
            for ws_id in &archived_ids {
                let payload = crate::types::WorkspaceStateChanged {
                    workspace_id: ws_id.clone(),
                    new_state: WorkspaceState::Archived,
                };
                if let Err(e) = app_handle.emit("workspace:state_changed", &payload) {
                    warn!("force-sync cleanup: failed to emit state_changed for '{ws_id}': {e}");
                }
            }
            if !archived_ids.is_empty() {
                info!(
                    "force-sync cleanup: archived {} workspace(s)",
                    archived_ids.len()
                );
            }
        }
        Err(e) => warn!("force-sync workspace cleanup failed: {e}"),
    }

    Ok(stats)
}

/// Triggers an immediate GitHub data sync, bypassing the polling timer.
///
/// Reads the stored token and cached username in a single pass, creates
/// a temporary [`GitHubClient`], runs `sync_dashboard`, and emits a
/// `github:updated` event with the resulting [`DashboardStats`] payload.
/// The frontend should listen for this event to refresh its data.
#[tauri::command]
pub async fn github_force_sync(
    pool: tauri::State<'_, SqlitePool>,
    cached: tauri::State<'_, GithubUsername>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let stats = run_force_sync(&app_handle, &pool, &cached).await?;
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    if let Err(e) = crate::tray::update_tray_badge(&app_handle, stats.pending_reviews) {
        warn!("failed to update tray badge after force sync: {e}");
    }
    Ok(())
}

/// Returns the full application configuration (query).
#[tauri::command]
pub async fn config_get(pool: tauri::State<'_, SqlitePool>) -> Result<AppConfig, String> {
    get_config(&pool).await.map_err(|e| e.to_string())
}

/// Merges a partial update into the current config and persists it (command).
///
/// Reads the current config, applies the partial overrides, and writes
/// the merged result back. Returns the full config as written (including
/// any clamped values).
///
/// **Concurrency note:** The read-merge-write is not wrapped in a single
/// transaction, so concurrent `config_set` calls could overwrite each other.
/// Acceptable for a single-window desktop app; revisit if multi-window
/// settings editing is added.
#[tauri::command]
pub async fn config_set(
    pool: tauri::State<'_, SqlitePool>,
    partial: PartialAppConfig,
) -> Result<AppConfig, String> {
    let current = get_config(&pool).await.map_err(|e| e.to_string())?;
    let merged = merge_partial_config(&current, &partial);
    set_config(&pool, &merged).await.map_err(|e| e.to_string())
}

/// Returns all repos ordered by `full_name` (query).
#[tauri::command]
pub async fn repos_list(pool: tauri::State<'_, SqlitePool>) -> Result<Vec<Repo>, String> {
    list_repos(&pool).await.map_err(|e| e.to_string())
}

/// Sets a repo's `enabled` flag and returns the updated repo (command).
///
/// Tauri 2 renames `repo_id` → `repoId` for the JS caller.
/// The frontend invokes this as `{ repoId, enabled }`.
#[tauri::command]
pub async fn repos_set_enabled(
    pool: tauri::State<'_, SqlitePool>,
    repo_id: String,
    enabled: bool,
) -> Result<Repo, String> {
    set_repo_enabled(&pool, &repo_id, enabled)
        .await
        .map_err(|e| e.to_string())
}

/// Sets or clears the local clone path for a repo (command).
///
/// Tauri 2 renames `repo_id` → `repoId` for the JS caller.
/// Pass `path: null` from the frontend to clear the local path.
/// Empty/whitespace-only strings are normalised to `None`.
/// Non-absolute paths are rejected.
#[tauri::command]
pub async fn repos_set_local_path(
    pool: tauri::State<'_, SqlitePool>,
    repo_id: String,
    path: Option<String>,
) -> Result<Repo, String> {
    let normalized = path.as_deref().map(str::trim).filter(|p| !p.is_empty());

    if let Some(p) = normalized
        && !std::path::Path::new(p).is_absolute()
    {
        return Err("path must be an absolute path".to_string());
    }

    set_local_path(&pool, &repo_id, normalized)
        .await
        .map_err(|e| e.to_string())
}

/// Marks a single activity as read. Returns `true` if the activity was
/// actually updated (i.e. it was previously unread), `false` otherwise.
///
/// Tauri 2 renames `activity_id` → `activityId` for the JS caller.
#[tauri::command]
pub async fn activity_mark_read(
    pool: tauri::State<'_, SqlitePool>,
    activity_id: String,
) -> Result<bool, String> {
    mark_read(&pool, &activity_id)
        .await
        .map_err(|e| e.to_string())
}

/// Marks all unread activities as read. Returns the number of rows updated.
///
/// The underlying `mark_all_read` returns `u64`, but we cast to `u32` at the
/// IPC boundary so the value fits safely in JavaScript's `number` (IEEE-754).
#[tauri::command]
pub async fn activity_mark_all_read(pool: tauri::State<'_, SqlitePool>) -> Result<u32, String> {
    let count = mark_all_read(&pool).await.map_err(|e| e.to_string())?;
    #[allow(clippy::cast_possible_truncation)] // Desktop SQLite — count will never exceed u32::MAX
    Ok(count as u32)
}

/// Returns all workspaces ordered by `updated_at` DESC (query).
///
/// No filter is applied — all states (active, suspended, archived) are returned.
/// The frontend can filter client-side or the caller can add a `state` parameter
/// in a future iteration.
#[tauri::command]
pub async fn workspace_list(pool: tauri::State<'_, SqlitePool>) -> Result<Vec<Workspace>, String> {
    list_workspaces(&pool, None)
        .await
        .map_err(|e| e.to_string())
}

/// Returns enriched workspace list entries with git status, CI, notes (query).
#[tauri::command]
pub async fn workspace_list_enriched(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<WorkspaceListEntry>, String> {
    crate::cache::workspace_enrichment::assemble_workspace_list_entries(&pool)
        .await
        .map_err(|e| e.to_string())
}

/// Returns all notes for a workspace ordered by `created_at` ASC (query).
///
/// Tauri 2 renames `workspace_id` → `workspaceId` for the JS caller.
#[tauri::command]
pub async fn workspace_get_notes(
    pool: tauri::State<'_, SqlitePool>,
    workspace_id: String,
) -> Result<Vec<WorkspaceNote>, String> {
    get_notes(&pool, &workspace_id)
        .await
        .map_err(|e| e.to_string())
}

// ── PTY state management (T-069) ────────────────────────────────

/// Managed state wrapping the [`PtyManager`] and a workspace→pty mapping.
///
/// The workspace→pty map tracks which PTY belongs to which workspace so
/// that LRU eviction can kill the correct PTY when suspending a workspace.
pub struct PtyManagerState {
    pub manager: PtyManager,
    workspace_ptys: Mutex<HashMap<String, String>>,
    /// Throttle map: `workspace_id` → last time `update_last_active` was called.
    /// Used to avoid a DB write on every keystroke.
    last_touch: Mutex<HashMap<String, std::time::Instant>>,
}

/// Minimum interval between `last_active_at` DB writes per workspace.
const LAST_ACTIVE_THROTTLE: std::time::Duration = std::time::Duration::from_secs(5);

impl PtyManagerState {
    pub fn new() -> Self {
        Self {
            manager: PtyManager::new(),
            workspace_ptys: Mutex::new(HashMap::new()),
            last_touch: Mutex::new(HashMap::new()),
        }
    }

    /// Returns `true` if enough time has elapsed since the last touch for
    /// this workspace (or if it has never been touched). Updates the stored
    /// timestamp on success.
    pub fn should_touch_last_active(&self, workspace_id: &str) -> bool {
        let now = std::time::Instant::now();
        let mut map = match self.last_touch.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        match map.get(workspace_id) {
            Some(last) if now.duration_since(*last) < LAST_ACTIVE_THROTTLE => false,
            _ => {
                map.insert(workspace_id.to_string(), now);
                true
            }
        }
    }

    /// Records a `workspace_id` → `pty_id` mapping.
    pub fn register(&self, workspace_id: &str, pty_id: &str) {
        if let Ok(mut map) = self.workspace_ptys.lock() {
            map.insert(workspace_id.to_string(), pty_id.to_string());
        }
    }

    /// Removes the mapping by workspace ID and returns the `pty_id` if present.
    /// Also clears the corresponding `last_touch` throttle entry.
    pub fn unregister(&self, workspace_id: &str) -> Option<String> {
        if let Ok(mut touch) = self.last_touch.lock() {
            touch.remove(workspace_id);
        }
        self.workspace_ptys.lock().ok()?.remove(workspace_id)
    }

    /// Forward lookup: find the PTY ID associated with a workspace ID.
    pub fn lookup_pty_by_workspace(&self, workspace_id: &str) -> Option<String> {
        let map = match self.workspace_ptys.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        map.get(workspace_id).cloned()
    }
}

impl Default for PtyManagerState {
    fn default() -> Self {
        Self::new()
    }
}

// ── Session monitoring ──────────────────────────────────────────

/// Monitors PTY output for Claude Code session events.
///
/// Line-buffers chunks from the PTY reader and scans each complete line
/// for session IDs and auth errors. On first session-ID match, persists
/// to the database and emits a `workspace:claude_session` event.
struct SessionMonitor {
    workspace_id: String,
    pool: SqlitePool,
    app_handle: tauri::AppHandle,
    line_buffer: Mutex<String>,
    session_detected: Arc<AtomicBool>,
}

impl SessionMonitor {
    fn new(workspace_id: String, pool: SqlitePool, app_handle: tauri::AppHandle) -> Self {
        Self {
            workspace_id,
            pool,
            app_handle,
            line_buffer: Mutex::new(String::new()),
            session_detected: Arc::new(AtomicBool::new(false)),
        }
    }

    fn process_chunk(&self, data: &[u8]) {
        let text = String::from_utf8_lossy(data);

        // Drain complete lines while holding the lock, then release before
        // processing to avoid blocking other callers during emit/spawn.
        let lines: Vec<String> = {
            let mut buf = self.line_buffer.lock().unwrap_or_else(|e| {
                warn!("SessionMonitor line_buffer mutex was poisoned, recovering");
                e.into_inner()
            });
            buf.push_str(&text);
            let mut lines = Vec::new();
            while let Some(pos) = buf.find('\n') {
                lines.push(buf.drain(..=pos).collect());
            }
            lines
        };

        for line in &lines {
            // Atomic compare-and-swap: only the first thread to detect a
            // session ID enters the persistence branch.
            if self
                .session_detected
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                if let Some(session_id) = crate::workspace::claude::detect_session_id(line) {
                    let pool = self.pool.clone();
                    let ws_id = self.workspace_id.clone();
                    let app_handle = self.app_handle.clone();
                    let sid = session_id.clone();
                    let flag = self.session_detected.clone();
                    tokio::spawn(async move {
                        if let Err(e) = crate::cache::workspaces::update_claude_session(
                            &pool,
                            &ws_id,
                            Some(&sid),
                        )
                        .await
                        {
                            // Reset flag so subsequent lines can retry detection
                            flag.store(false, Ordering::Release);
                            warn!("failed to persist claude session id (will retry): {e}");
                            return;
                        }
                        if let Err(e) = app_handle.emit(
                            "workspace:claude_session",
                            ClaudeSessionPayload {
                                workspace_id: ws_id,
                                session_id: sid,
                            },
                        ) {
                            warn!("failed to emit workspace:claude_session: {e}");
                        }
                    });
                } else {
                    // No session in this line — reset so the next line can try.
                    self.session_detected.store(false, Ordering::Release);
                }
            }

            if crate::workspace::claude::detect_auth_error(line)
                && let Err(e) = self.app_handle.emit("auth:expired", ())
            {
                warn!("failed to emit auth:expired: {e}");
            }
        }
    }
}

/// Builds a PTY output callback that forwards data to the frontend AND
/// monitors for Claude Code session events.
fn make_pty_callback(
    workspace_id: String,
    pool: SqlitePool,
    app_handle: tauri::AppHandle,
) -> impl Fn(&str, &[u8]) + Send + 'static {
    let monitor = Arc::new(SessionMonitor::new(
        workspace_id.clone(),
        pool,
        app_handle.clone(),
    ));

    move |_pty_id: &str, data: &[u8]| {
        let payload = crate::types::PtyOutput {
            workspace_id: workspace_id.clone(),
            data: String::from_utf8_lossy(data).to_string(),
        };
        if let Err(e) = app_handle.emit("workspace:stdout", &payload) {
            warn!("failed to emit workspace:stdout: {e}");
        }
        monitor.process_chunk(data);
    }
}

// ── Workspace open inner logic (T-069) ──────────────────────────

/// Core logic for `workspace_open`, testable without Tauri state.
///
/// The `workspace_id` is pre-generated by the caller so the `on_pty_output`
/// closure can capture the real workspace UUID (not the PTY UUID).
///
/// LRU eviction is deferred until the new workspace is fully created — if
/// worktree/PTY/DB steps fail, no existing workspace is disrupted.
pub(crate) async fn workspace_open_inner(
    pool: &SqlitePool,
    pty_state: &PtyManagerState,
    workspace_id: &str,
    req: &OpenWorkspaceRequest,
    on_pty_output: impl Fn(&str, &[u8]) + Send + 'static,
) -> Result<OpenWorkspaceResponse, AppError> {
    // 1. Get repo; clone if no local_path
    let repo = get_repo(pool, &req.repo_id).await?;

    // 2. Read config (needed for base_dir and LRU limit)
    let config = get_config(pool).await?;

    let base_dir = config
        .workspaces_dir
        .as_deref()
        .map(PathBuf::from)
        .or_else(|| worktree::default_base_dir().ok())
        .ok_or_else(|| AppError::Workspace("cannot determine workspaces base directory".into()))?;

    let local_path = if let Some(lp) = repo.local_path.as_deref() {
        PathBuf::from(lp)
    } else {
        // Auto-clone: clone into {base_dir}/{org}/{repo}/repo
        let repos_dir = base_dir.join(&repo.full_name);
        let clone_path = worktree::clone_repo(&repo.url, "repo", &repos_dir).await?;
        // Persist local_path so future opens skip the clone
        let clone_str = clone_path
            .to_str()
            .ok_or_else(|| AppError::Workspace("clone path is not valid UTF-8".into()))?;
        crate::cache::repos::set_local_path(pool, &repo.id, Some(clone_str)).await?;
        clone_path
    };

    // 3. Create worktree
    let wt_path = worktree::create_worktree(
        &local_path,
        &req.branch,
        req.pull_request_number,
        &repo.name,
        &base_dir,
    )
    .await?;

    // 4. Spawn PTY in the worktree
    let pty_id = match pty_state.manager.spawn(&wt_path, 80, 24, on_pty_output) {
        Ok(id) => id,
        Err(e) => {
            let _ = worktree::remove_worktree(&local_path, &wt_path).await;
            return Err(e);
        }
    };

    // 5. Atomic DELETE archived + INSERT new workspace (transaction prevents
    //    data loss if INSERT fails, and avoids resource leaks if DELETE fails).
    let now = chrono::Utc::now().to_rfc3339();
    let insert_result: Result<(), AppError> = async {
        let mut tx = pool.begin().await?;
        sqlx::query(
            "DELETE FROM workspaces WHERE repo_id = $1 AND pull_request_number = $2 AND state = 'archived'",
        )
        .bind(&req.repo_id)
        .bind(req.pull_request_number)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO workspaces (id, repo_id, pull_request_number, state, worktree_path, session_id, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(workspace_id)
        .bind(&req.repo_id)
        .bind(req.pull_request_number)
        .bind("active")
        .bind(wt_path.to_string_lossy().as_ref())
        .bind(Option::<&str>::None)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }
    .await;
    if let Err(e) = insert_result {
        let _ = pty_state.manager.kill(&pty_id);
        let _ = worktree::remove_worktree(&local_path, &wt_path).await;
        return Err(e);
    }

    // 6. Track workspace → pty mapping
    pty_state.register(workspace_id, &pty_id);

    // 7. LRU eviction — deferred until AFTER the new workspace is safely created.
    //    Best-effort: eviction failures must not fail the already-created workspace.
    if let Err(e) = crate::workspace::lifecycle::enforce_max_active(
        pool,
        pty_state,
        config.max_active_workspaces,
    )
    .await
    {
        warn!("LRU eviction failed: {e}");
    }

    Ok(OpenWorkspaceResponse {
        workspace_id: workspace_id.to_string(),
        worktree_path: wt_path.to_string_lossy().to_string(),
        pty_id,
        session_id: None,
    })
}

// ── Workspace state transitions (T-070) ─────���──────────────────

/// Spawns a background task to generate and store a suspension note.
///
/// No-op if the workspace has no `session_id` or `worktree_path`.
/// Fire-and-forget: failures are logged, never propagated.
fn spawn_suspension_note(pool: &SqlitePool, ws: &Workspace) {
    if let (Some(wt_path), Some(session_id)) = (ws.worktree_path.clone(), ws.session_id.clone()) {
        let pool = pool.clone();
        let ws_id = ws.id.clone();
        tokio::spawn(async move {
            if let Err(e) = crate::workspace::claude::create_suspension_note(
                &pool,
                &ws_id,
                Path::new(&wt_path),
                Some(&session_id),
            )
            .await
            {
                warn!("failed to generate suspension note for '{ws_id}': {e}");
            }
        });
    }
}

/// Suspends an active workspace: kills its PTY and sets state to Suspended.
///
/// Emits no event — the Tauri command wrapper handles event emission.
/// Spawns a background note-generation task via [`spawn_suspension_note`].
pub(crate) async fn workspace_suspend_inner(
    pool: &SqlitePool,
    pty_state: &PtyManagerState,
    workspace_id: &str,
) -> Result<Workspace, AppError> {
    // Kill the PTY if one is tracked (before DB update — safe direction of failure)
    if let Some(pty_id) = pty_state.unregister(workspace_id)
        && let Err(e) = pty_state.manager.kill(&pty_id)
    {
        warn!("failed to kill PTY {pty_id} during suspend: {e}");
    }

    // Atomic transition: WHERE state = 'active' prevents TOCTOU races
    let suspended = update_workspace_state(
        pool,
        workspace_id,
        &WorkspaceState::Suspended,
        Some(&WorkspaceState::Active),
    )
    .await?;

    // Best-effort background note generation using the post-update workspace.
    spawn_suspension_note(pool, &suspended);

    Ok(suspended)
}

/// Resumes a suspended workspace: spawns a new PTY in the existing worktree.
///
/// Returns an [`OpenWorkspaceResponse`] with the new PTY ID.
/// Archived workspaces cannot be resumed (worktree is deleted).
pub(crate) async fn workspace_resume_inner(
    pool: &SqlitePool,
    pty_state: &PtyManagerState,
    workspace_id: &str,
    on_pty_output: impl Fn(&str, &[u8]) + Send + 'static,
) -> Result<OpenWorkspaceResponse, AppError> {
    let ws = crate::cache::workspaces::get_workspace(pool, workspace_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("workspace '{workspace_id}'")))?;

    if ws.state != WorkspaceState::Suspended {
        return Err(AppError::Workspace(format!(
            "cannot resume workspace in state '{:?}' — must be Suspended",
            ws.state
        )));
    }

    let wt_path_str = ws.worktree_path.as_deref().ok_or_else(|| {
        AppError::Workspace(format!(
            "workspace '{workspace_id}' has no worktree_path — cannot resume"
        ))
    })?;
    let wt_path = PathBuf::from(wt_path_str);

    // Verify worktree still exists on disk before spawning PTY
    if !wt_path.exists() {
        return Err(AppError::Workspace(format!(
            "workspace '{workspace_id}' worktree no longer exists at '{}' — cannot resume",
            wt_path.display()
        )));
    }

    // Spawn a new PTY in the existing worktree
    let pty_id = pty_state.manager.spawn(&wt_path, 80, 24, on_pty_output)?;

    // Atomic transition: WHERE state = 'suspended' prevents TOCTOU races.
    // If this fails, kill the PTY to avoid orphaning it.
    let updated = match update_workspace_state(
        pool,
        workspace_id,
        &WorkspaceState::Active,
        Some(&WorkspaceState::Suspended),
    )
    .await
    {
        Ok(ws) => ws,
        Err(e) => {
            if let Err(kill_err) = pty_state.manager.kill(&pty_id) {
                warn!("failed to kill orphaned PTY {pty_id} after DB error: {kill_err}");
            }
            return Err(e);
        }
    };

    // Track workspace → pty mapping
    pty_state.register(workspace_id, &pty_id);

    Ok(OpenWorkspaceResponse {
        workspace_id: workspace_id.to_string(),
        worktree_path: wt_path.to_string_lossy().to_string(),
        pty_id,
        session_id: updated.session_id,
    })
}

/// Archives a workspace: kills PTY if active, removes worktree, sets state to Archived.
///
/// Accepts the pre-fetched `Workspace` to avoid a redundant DB query.
/// `repo_local_path` is optional — when `None`, worktree removal is skipped
/// (best-effort: a missing `local_path` should not block archiving).
pub(crate) async fn workspace_archive_inner(
    pool: &SqlitePool,
    pty_state: &PtyManagerState,
    ws: &Workspace,
    repo_local_path: Option<&Path>,
) -> Result<Workspace, AppError> {
    if ws.state == WorkspaceState::Archived {
        return Err(AppError::Workspace(format!(
            "workspace '{}' is already archived",
            ws.id
        )));
    }

    // Kill PTY if tracked (Active state)
    if let Some(pty_id) = pty_state.unregister(&ws.id)
        && let Err(e) = pty_state.manager.kill(&pty_id)
    {
        warn!("failed to kill PTY {pty_id} during archive: {e}");
    }

    // Remove worktree from disk.
    // Best-effort: filesystem errors and missing local_path do not fail the
    // archive operation. The workspace is marked Archived in the DB regardless,
    // balancing filesystem transience against database consistency.
    if let (Some(wt_path_str), Some(repo_path)) = (&ws.worktree_path, repo_local_path) {
        let wt_path = PathBuf::from(wt_path_str);
        if wt_path.exists()
            && let Err(e) = worktree::remove_worktree(repo_path, &wt_path).await
        {
            warn!("failed to remove worktree during archive: {e}");
        }
    } else if ws.worktree_path.is_some() && repo_local_path.is_none() {
        warn!(
            "repo has no local_path — worktree for workspace '{}' will not be removed from disk",
            ws.id
        );
    }

    // Atomic transition: guard on current state for consistency with suspend/resume
    update_workspace_state(pool, &ws.id, &WorkspaceState::Archived, Some(&ws.state)).await
}

/// Fetches PR details from GitHub and generates CLAUDE.md in the worktree.
/// Creates a `GitHubClient` from the stored token on-demand.
async fn generate_claude_md_with_token(
    repo_id: &str,
    pr_number: u32,
    worktree_path: &str,
) -> Result<(), AppError> {
    let token = tokio::task::spawn_blocking(auth::get_token)
        .await
        .map_err(|e| AppError::Workspace(format!("token task join error: {e}")))?
        .map_err(|e| AppError::Auth(format!("failed to get token: {e}")))?
        .ok_or_else(|| AppError::Auth("no GitHub token stored".into()))?;

    let client = GitHubClient::new(&token)?;
    crate::workspace::claude::generate_claude_md(
        &client,
        repo_id,
        i64::from(pr_number),
        std::path::Path::new(worktree_path),
    )
    .await
}

/// Opens a workspace for a PR: creates worktree, spawns PTY, persists in DB.
///
/// Pre-generates the workspace UUID so the `workspace:stdout` events carry
/// the correct `workspaceId` (not the PTY UUID).
#[tauri::command]
pub async fn workspace_open(
    pool: tauri::State<'_, SqlitePool>,
    pty_state: tauri::State<'_, PtyManagerState>,
    app_handle: tauri::AppHandle,
    req: OpenWorkspaceRequest,
) -> Result<OpenWorkspaceResponse, String> {
    let workspace_id = uuid::Uuid::new_v4().to_string();

    let on_output = make_pty_callback(
        workspace_id.clone(),
        pool.inner().clone(),
        app_handle.clone(),
    );

    let resp = workspace_open_inner(&pool, &pty_state, &workspace_id, &req, on_output)
        .await
        .map_err(|e| e.to_string())?;

    // Background: generate CLAUDE.md then launch Claude
    {
        let manager = pty_state.manager.clone();
        let pty_id = resp.pty_id.clone();
        let repo_id = req.repo_id.clone();
        let pr_number = req.pull_request_number;
        let wt_path = resp.worktree_path.clone();
        tokio::spawn(async move {
            // 1. Generate CLAUDE.md (best-effort)
            match generate_claude_md_with_token(&repo_id, pr_number, &wt_path).await {
                Ok(()) => info!("CLAUDE.md generated for PR #{pr_number}"),
                Err(e) => warn!("CLAUDE.md generation failed for PR #{pr_number}: {e}"),
            }
            // 2. Launch Claude in PTY
            if let Err(e) = crate::workspace::claude::launch_claude(&manager, &pty_id) {
                warn!("failed to launch claude in workspace: {e}");
                return;
            }
            // 3. Brief pause for Claude to start, then rename session
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let name = format!("prism-pr-{pr_number}");
            if let Err(e) =
                crate::workspace::claude::rename_claude_session(&manager, &pty_id, &name)
            {
                warn!("failed to rename claude session: {e}");
            }
        });
    }

    Ok(resp)
}

/// Suspends an active workspace: kills PTY, sets state to Suspended.
///
/// Emits `workspace:state_changed` with the new state.
/// Tauri 2 renames `workspace_id` → `workspaceId` for the JS caller.
#[tauri::command]
pub async fn workspace_suspend(
    pool: tauri::State<'_, SqlitePool>,
    pty_state: tauri::State<'_, PtyManagerState>,
    app_handle: tauri::AppHandle,
    workspace_id: String,
) -> Result<(), String> {
    let ws = workspace_suspend_inner(&pool, &pty_state, &workspace_id)
        .await
        .map_err(|e| e.to_string())?;
    let payload = crate::types::WorkspaceStateChanged {
        workspace_id: ws.id,
        new_state: ws.state,
    };
    if let Err(e) = app_handle.emit("workspace:state_changed", &payload) {
        warn!("failed to emit workspace:state_changed: {e}");
    }
    Ok(())
}

/// Resumes a suspended workspace: spawns new PTY, sets state to Active.
///
/// Returns an [`OpenWorkspaceResponse`] with the new PTY ID.
/// Emits `workspace:state_changed` with the new state.
/// Tauri 2 renames `workspace_id` → `workspaceId` for the JS caller.
#[tauri::command]
pub async fn workspace_resume(
    pool: tauri::State<'_, SqlitePool>,
    pty_state: tauri::State<'_, PtyManagerState>,
    app_handle: tauri::AppHandle,
    workspace_id: String,
) -> Result<OpenWorkspaceResponse, String> {
    let on_output = make_pty_callback(
        workspace_id.clone(),
        pool.inner().clone(),
        app_handle.clone(),
    );

    let resp = workspace_resume_inner(&pool, &pty_state, &workspace_id, on_output)
        .await
        .map_err(|e| e.to_string())?;

    let payload = crate::types::WorkspaceStateChanged {
        workspace_id: resp.workspace_id.clone(),
        new_state: WorkspaceState::Active,
    };
    if let Err(e) = app_handle.emit("workspace:state_changed", &payload) {
        warn!("failed to emit workspace:state_changed: {e}");
    }

    // Background: launch or resume Claude
    {
        let manager = pty_state.manager.clone();
        let pty_id = resp.pty_id.clone();
        let session_id = resp.session_id.clone();
        let pool_bg = pool.inner().clone();
        let ws_id = workspace_id.clone();
        tokio::spawn(async move {
            // Unlike workspace_open, resume has no preparatory async work before
            // Claude starts emitting its first frame. Give the frontend a short
            // window to remount the terminal and subscribe to workspace:stdout
            // so the initial Claude UI isn't lost during resume.
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;

            // Resume or launch Claude first — must not be gated on a rename lookup
            if let Some(sid) = &session_id {
                if let Err(e) = crate::workspace::claude::resume_claude(&manager, &pty_id, sid) {
                    warn!("failed to resume claude session: {e}");
                    // Launch fresh first (safe ordering: workspace always has a process)
                    if let Err(e2) = crate::workspace::claude::launch_claude(&manager, &pty_id) {
                        warn!("failed to launch claude as fallback: {e2}");
                        return;
                    }
                    // Then clear stale session_id (compare-and-swap)
                    if let Err(e) =
                        crate::cache::workspaces::clear_stale_session(&pool_bg, &ws_id, Some(sid))
                            .await
                    {
                        warn!("failed to clear stale session: {e}");
                    }
                }
            } else if let Err(e) = crate::workspace::claude::launch_claude(&manager, &pty_id) {
                warn!("failed to launch claude: {e}");
                return;
            }

            // Best-effort rename — fetch pr_number after Claude is already running
            if let Ok(Some(ws)) = crate::cache::workspaces::get_workspace(&pool_bg, &ws_id).await {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                let name = format!("prism-pr-{}", ws.pull_request_number);
                if let Err(e) =
                    crate::workspace::claude::rename_claude_session(&manager, &pty_id, &name)
                {
                    warn!("failed to rename claude session: {e}");
                }
            } else {
                warn!("cannot get workspace for claude rename: {ws_id}");
            }
        });
    }

    Ok(resp)
}

/// Archives a workspace: kills PTY if active, removes worktree, sets state to Archived.
///
/// Works from both Active and Suspended states.
/// Emits `workspace:state_changed` with the new state.
/// Tauri 2 renames `workspace_id` → `workspaceId` for the JS caller.
#[tauri::command]
pub async fn workspace_archive(
    pool: tauri::State<'_, SqlitePool>,
    pty_state: tauri::State<'_, PtyManagerState>,
    app_handle: tauri::AppHandle,
    workspace_id: String,
) -> Result<(), String> {
    // Pre-fetch workspace (passed to inner to avoid double query)
    let ws = crate::cache::workspaces::get_workspace(&pool, &workspace_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("workspace '{workspace_id}' not found"))?;

    // Look up repo local path — best-effort (None skips worktree removal)
    let local_path: Option<PathBuf> = match crate::cache::repos::get_repo(&pool, &ws.repo_id).await
    {
        Ok(repo) => repo.local_path.as_deref().map(PathBuf::from),
        Err(e) => {
            warn!(
                "could not fetch repo '{}' for worktree removal — skipping: {e}",
                ws.repo_id
            );
            None
        }
    };

    let archived = workspace_archive_inner(&pool, &pty_state, &ws, local_path.as_deref())
        .await
        .map_err(|e| e.to_string())?;

    let payload = crate::types::WorkspaceStateChanged {
        workspace_id: archived.id,
        new_state: archived.state,
    };
    if let Err(e) = app_handle.emit("workspace:state_changed", &payload) {
        warn!("failed to emit workspace:state_changed: {e}");
    }
    Ok(())
}

// ── Workspace cleanup (T-071) ────────────────────────────────────

/// Archives workspaces whose PR is merged (> `archive_delay_hours`) or
/// closed (> `archive_delay_closed_hours`). Returns the IDs of archived
/// workspaces so the caller can emit per-workspace events.
///
/// Accepts delay values as parameters so tests can control timing.
/// Delay values are capped at ~100 years to avoid `Duration` overflow.
pub(crate) async fn workspace_cleanup_inner(
    pool: &SqlitePool,
    pty_state: &PtyManagerState,
    archive_delay_hours: u64,
    archive_delay_closed_hours: u64,
) -> Result<Vec<String>, AppError> {
    use chrono::{Duration, SecondsFormat, Utc};

    // Cap at ~100 years to avoid Duration::hours overflow (i64 * 3_600_000_000_000 ns).
    const MAX_DELAY_HOURS: i64 = 365 * 24 * 100;

    let now = Utc::now();
    let delay_merged = i64::try_from(archive_delay_hours)
        .unwrap_or(MAX_DELAY_HOURS)
        .min(MAX_DELAY_HOURS);
    let delay_closed = i64::try_from(archive_delay_closed_hours)
        .unwrap_or(MAX_DELAY_HOURS)
        .min(MAX_DELAY_HOURS);
    let merged_cutoff =
        (now - Duration::hours(delay_merged)).to_rfc3339_opts(SecondsFormat::Millis, true);
    let closed_cutoff =
        (now - Duration::hours(delay_closed)).to_rfc3339_opts(SecondsFormat::Millis, true);

    // Find workspace IDs eligible for cleanup via a single JOIN query.
    let ids: Vec<(String,)> = sqlx::query_as(
        "SELECT w.id FROM workspaces w \
         JOIN pull_requests p ON w.repo_id = p.repo_id AND w.pull_request_number = p.number \
         WHERE w.state != 'archived' \
         AND ( \
             (p.state = 'merged' AND p.updated_at < $1) \
             OR (p.state = 'closed' AND p.updated_at < $2) \
         )",
    )
    .bind(&merged_cutoff)
    .bind(&closed_cutoff)
    .fetch_all(pool)
    .await?;

    let mut archived_ids = Vec::new();
    for (ws_id,) in &ids {
        let Some(ws) = crate::cache::workspaces::get_workspace(pool, ws_id).await? else {
            continue;
        };

        let local_path: Option<PathBuf> =
            match crate::cache::repos::get_repo(pool, &ws.repo_id).await {
                Ok(repo) => repo.local_path.as_deref().map(PathBuf::from),
                Err(e) => {
                    warn!("cleanup: could not fetch repo '{}': {e}", ws.repo_id);
                    None
                }
            };

        match workspace_archive_inner(pool, pty_state, &ws, local_path.as_deref()).await {
            Ok(_) => archived_ids.push(ws.id.clone()),
            Err(e) => warn!("cleanup: failed to archive workspace '{}': {e}", ws.id),
        }
    }

    Ok(archived_ids)
}

/// Auto-archives workspaces whose PR has been merged or closed past the
/// configured delay. Returns the number of workspaces archived.
///
/// Emits `workspace:state_changed` for each archived workspace so the
/// frontend stays in sync.
#[tauri::command]
pub async fn workspace_cleanup(
    pool: tauri::State<'_, SqlitePool>,
    pty_state: tauri::State<'_, PtyManagerState>,
    app_handle: tauri::AppHandle,
) -> Result<u32, String> {
    let config = get_config(&pool).await.map_err(|e| e.to_string())?;
    let archived_ids = workspace_cleanup_inner(
        &pool,
        &pty_state,
        config.archive_delay_hours,
        config.archive_delay_closed_hours,
    )
    .await
    .map_err(|e| e.to_string())?;

    for ws_id in &archived_ids {
        let payload = crate::types::WorkspaceStateChanged {
            workspace_id: ws_id.clone(),
            new_state: WorkspaceState::Archived,
        };
        if let Err(e) = app_handle.emit("workspace:state_changed", &payload) {
            warn!("cleanup: failed to emit workspace:state_changed for '{ws_id}': {e}");
        }
    }

    Ok(u32::try_from(archived_ids.len()).unwrap_or(u32::MAX))
}

/// Writes data to a PTY's stdin and touches `last_active_at` for the
/// associated workspace so the lifecycle auto-suspend timer is reset.
///
/// Tauri 2 renames `workspace_id` → `workspaceId` for the JS caller.
#[tauri::command]
pub async fn pty_write(
    pool: tauri::State<'_, SqlitePool>,
    pty_state: tauri::State<'_, PtyManagerState>,
    workspace_id: String,
    data: String,
) -> Result<(), String> {
    let pty_id = pty_state
        .lookup_pty_by_workspace(&workspace_id)
        .ok_or_else(|| format!("no PTY for workspace '{workspace_id}'"))?;

    pty_state
        .manager
        .write_pty(&pty_id, data.as_bytes())
        .map_err(|e| e.to_string())?;

    // Best-effort: update last_active_at, throttled to at most once per 5s per workspace.
    if pty_state.should_touch_last_active(&workspace_id)
        && let Err(e) = crate::cache::workspaces::update_last_active(&pool, &workspace_id).await
    {
        warn!("pty_write: failed to touch last_active_at for workspace '{workspace_id}': {e}");
    }

    Ok(())
}

/// Resizes a PTY to new dimensions.
///
/// Tauri 2 renames `workspace_id` → `workspaceId` for the JS caller.
#[tauri::command]
pub async fn pty_resize(
    pty_state: tauri::State<'_, PtyManagerState>,
    workspace_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let pty_id = pty_state
        .lookup_pty_by_workspace(&workspace_id)
        .ok_or_else(|| format!("no PTY for workspace '{workspace_id}'"))?;

    pty_state
        .manager
        .resize(&pty_id, cols, rows)
        .map_err(|e| e.to_string())
}

/// Kills a PTY process and removes it from the manager.
///
/// Idempotent: succeeds even if the PTY was already cleaned up by the
/// reader task (EOF), since `AppError::NotFound` is treated as success.
/// Tauri 2 renames `workspace_id` → `workspaceId` for the JS caller.
#[tauri::command]
pub async fn pty_kill(
    pty_state: tauri::State<'_, PtyManagerState>,
    workspace_id: String,
) -> Result<(), String> {
    let pty_id = pty_state
        .lookup_pty_by_workspace(&workspace_id)
        .ok_or_else(|| format!("no PTY for workspace '{workspace_id}'"))?;

    match pty_state.manager.kill(&pty_id) {
        Ok(()) | Err(AppError::NotFound(_)) => {
            let _ = pty_state.unregister(&workspace_id);
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

// ── Debug / Memory monitoring (T-087) ───────────────────────────

/// Reads the RSS (Resident Set Size) of the current process from `/proc/self/statm`.
///
/// Returns 0 on non-Linux platforms or if the read fails (best-effort).
pub(crate) fn get_process_rss_bytes() -> u64 {
    #[cfg(target_os = "linux")]
    {
        let Ok(statm) = std::fs::read_to_string("/proc/self/statm") else {
            return 0;
        };
        let rss_pages: u64 = statm
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        rss_pages * (page_size::get() as u64)
    }
    #[cfg(not(target_os = "linux"))]
    {
        0
    }
}

/// Returns the file size in bytes, or 0 if the file does not exist / is unreadable.
pub(crate) fn get_file_size_bytes(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// Returns memory usage statistics for the debug section in Settings.
///
/// Best-effort: returns 0 for any metric that cannot be read rather than
/// failing the entire call.
#[tauri::command]
pub async fn debug_memory_usage(app_handle: tauri::AppHandle) -> Result<MemoryStats, String> {
    let rss_bytes = get_process_rss_bytes();

    let db_size_bytes = app_handle
        .path()
        .app_data_dir()
        .map(|d| get_file_size_bytes(&d.join("prism.db")))
        .unwrap_or(0);

    Ok(MemoryStats {
        rss_bytes,
        db_size_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_status_serializes_connected() {
        let status = AuthStatus {
            connected: true,
            username: Some("testuser".into()),
            error: None,
        };
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["connected"], true);
        assert_eq!(json["username"], "testuser");
        assert!(json["error"].is_null());
    }

    #[test]
    fn test_auth_status_serializes_disconnected() {
        let status = AuthStatus {
            connected: false,
            username: None,
            error: None,
        };
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["connected"], false);
        assert!(json["username"].is_null());
    }

    #[test]
    fn test_auth_status_serializes_with_error() {
        let status = AuthStatus {
            connected: false,
            username: None,
            error: Some("GitHub API error: request failed: timeout".into()),
        };
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["connected"], false);
        assert!(json["error"].as_str().unwrap().contains("timeout"));
    }

    #[tokio::test]
    async fn test_auth_set_token_rejects_empty() {
        let result = set_token_inner("".into()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[tokio::test]
    async fn test_auth_set_token_rejects_whitespace() {
        let result = set_token_inner("   ".into()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    // -- status_from_validation tests cover auth_get_status branching logic --

    #[test]
    fn test_status_from_validation_success() {
        let status = status_from_validation(Ok("octocat".into()));
        assert!(status.connected);
        assert_eq!(status.username.as_deref(), Some("octocat"));
        assert!(status.error.is_none());
    }

    #[test]
    fn test_status_from_validation_auth_error() {
        let status = status_from_validation(Err(AppError::Auth("invalid or expired token".into())));
        assert!(!status.connected);
        assert!(status.username.is_none());
        assert!(status.error.is_none());
    }

    #[test]
    fn test_status_from_validation_transient_error() {
        let status =
            status_from_validation(Err(AppError::GitHub("request failed: timeout".into())));
        assert!(!status.connected);
        assert!(status.username.is_none());
        let err = status.error.unwrap();
        assert!(err.contains("timeout"), "expected 'timeout' in '{err}'");
    }

    #[test]
    fn test_status_from_validation_rate_limit() {
        let status = status_from_validation(Err(AppError::RateLimit {
            reset_at: "2026-03-25T19:00:00Z".into(),
        }));
        assert!(!status.connected);
        assert!(status.username.is_none());
        let err = status.error.unwrap();
        assert!(
            err.contains("rate limited"),
            "expected 'rate limited' in '{err}'"
        );
    }

    // -- GithubUsername + resolve_username tests --

    #[test]
    fn test_github_username_default_is_none() {
        let cached = GithubUsername::default();
        let guard = cached.0.lock().unwrap();
        assert!(guard.is_none());
    }

    #[tokio::test]
    async fn test_resolve_username_returns_cached_value() {
        let cached = GithubUsername(Mutex::new(Some("alice".into())));
        let result = resolve_username(&cached).await.unwrap();
        assert_eq!(result, "alice");
    }

    // -- DashboardStats IPC contract tests (T-036) --

    #[test]
    fn test_dashboard_stats_serializes_camel_case() {
        let stats = DashboardStats {
            pending_reviews: 3,
            open_prs: 7,
            open_issues: 2,
            total_workspaces: 1,
            unread_activity: 5,
        };
        let json = serde_json::to_value(&stats).unwrap();
        assert_eq!(json["pendingReviews"], 3);
        assert_eq!(json["openPrs"], 7);
        assert_eq!(json["openIssues"], 2);
        assert_eq!(json["totalWorkspaces"], 1);
        assert_eq!(json["unreadActivity"], 5);
    }

    // -- resolve_credentials tests (T-036) --

    #[tokio::test]
    async fn test_resolve_credentials_errors_when_no_token_stored() {
        // With an empty cache and no keychain entry, resolve_credentials
        // should return an authentication error (no token stored).
        // The cached-username fast path requires a real keychain fixture
        // and is therefore not covered here.
        let cached = GithubUsername::default();
        let result = resolve_credentials(&cached).await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("not authenticated") || err.contains("token"),
            "expected auth error, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_resolve_credentials_skips_poisoned_cache() {
        let cached = GithubUsername::default();
        let cached_ref = &cached;
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = cached_ref.0.lock().unwrap();
            panic!("intentional poison");
        }));

        // Poisoned lock should not produce a lock error — it falls through
        // to token validation (which fails without a keychain entry).
        let result = resolve_credentials(&cached).await;
        match result {
            Ok(_) => {}
            Err(err) => assert!(
                !err.contains("lock error"),
                "poisoned lock should not bubble up, got: {err}"
            ),
        }
    }

    // -- Repo IPC contract tests (T-037) --

    #[test]
    fn test_repo_serializes_camel_case() {
        let repo = crate::types::Repo {
            id: "r-1".into(),
            org: "mpiton".into(),
            name: "prism".into(),
            full_name: "mpiton/prism".into(),
            url: "https://github.com/mpiton/prism".into(),
            default_branch: "main".into(),
            is_archived: false,
            enabled: true,
            local_path: Some("/home/user/prism".into()),
            last_sync_at: None,
        };
        let json = serde_json::to_value(&repo).unwrap();
        assert_eq!(json["id"], "r-1");
        assert_eq!(json["fullName"], "mpiton/prism");
        assert_eq!(json["defaultBranch"], "main");
        assert_eq!(json["isArchived"], false);
        assert_eq!(json["localPath"], "/home/user/prism");
        assert!(json["lastSyncAt"].is_null());
    }

    #[test]
    fn test_repo_list_serializes_as_array() {
        let repos = vec![
            crate::types::Repo {
                id: "r-1".into(),
                org: "mpiton".into(),
                name: "alpha".into(),
                full_name: "mpiton/alpha".into(),
                url: "https://github.com/mpiton/alpha".into(),
                default_branch: "main".into(),
                is_archived: false,
                enabled: true,
                local_path: None,
                last_sync_at: None,
            },
            crate::types::Repo {
                id: "r-2".into(),
                org: "mpiton".into(),
                name: "beta".into(),
                full_name: "mpiton/beta".into(),
                url: "https://github.com/mpiton/beta".into(),
                default_branch: "develop".into(),
                is_archived: true,
                enabled: false,
                local_path: None,
                last_sync_at: Some("2026-03-26T10:00:00Z".into()),
            },
        ];
        let json = serde_json::to_value(&repos).unwrap();
        let arr = json.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["fullName"], "mpiton/alpha");
        assert_eq!(arr[1]["isArchived"], true);
        assert_eq!(arr[1]["lastSyncAt"], "2026-03-26T10:00:00Z");
    }

    // -- AppConfig IPC contract tests (T-038) --

    #[test]
    fn test_config_serializes_camel_case() {
        let config = AppConfig {
            poll_interval_secs: 120,
            max_active_workspaces: 5,
            archive_delay_hours: 12,
            archive_delay_closed_hours: 72,
            auto_suspend_minutes: 30,
            github_token: Some("ghp_test".into()),
            data_dir: None,
            workspaces_dir: Some("/ws".into()),
            claude_auth_mode: "oauth".to_string(),
            claude_auto_generate_md: false,
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["pollIntervalSecs"], 120);
        assert_eq!(json["maxActiveWorkspaces"], 5);
        assert_eq!(json["archiveDelayHours"], 12);
        assert_eq!(json["archiveDelayClosedHours"], 72);
        assert_eq!(json["githubToken"], "ghp_test");
        assert!(json["dataDir"].is_null());
        assert_eq!(json["workspacesDir"], "/ws");
    }

    #[test]
    fn test_partial_config_deserializes_from_frontend() {
        // Frontend sends only the fields it wants to update
        let json = serde_json::json!({
            "pollIntervalSecs": 60
        });
        let partial: PartialAppConfig = serde_json::from_value(json).unwrap();
        assert_eq!(partial.poll_interval_secs, Some(60));
        assert!(partial.max_active_workspaces.is_none());
        assert!(partial.github_token.is_none());
        assert!(partial.data_dir.is_none());
        assert!(partial.workspaces_dir.is_none());
    }

    #[test]
    fn test_partial_config_empty_object() {
        let json = serde_json::json!({});
        let partial: PartialAppConfig = serde_json::from_value(json).unwrap();
        assert!(partial.poll_interval_secs.is_none());
        assert!(partial.max_active_workspaces.is_none());
    }

    #[test]
    fn test_partial_config_deserializes_explicit_null_as_clear() {
        let json = serde_json::json!({
            "githubToken": null
        });
        let partial: PartialAppConfig = serde_json::from_value(json).unwrap();
        assert_eq!(
            partial.github_token,
            Some(None),
            "explicit null should produce Some(None), not None"
        );
        // poll_interval_secs absent → None (don't touch)
        assert!(partial.poll_interval_secs.is_none());
    }

    #[test]
    fn test_merge_partial_config_overrides_only_provided_fields() {
        let base = AppConfig {
            poll_interval_secs: 300,
            max_active_workspaces: 3,
            archive_delay_hours: 24,
            archive_delay_closed_hours: 48,
            auto_suspend_minutes: 30,
            github_token: None,
            data_dir: None,
            workspaces_dir: None,
            claude_auth_mode: "oauth".to_string(),
            claude_auto_generate_md: false,
        };
        let partial = PartialAppConfig {
            poll_interval_secs: Some(60),
            max_active_workspaces: None,
            archive_delay_hours: None,
            archive_delay_closed_hours: None,
            auto_suspend_minutes: None,
            github_token: None,
            data_dir: None,
            workspaces_dir: None,
            claude_auth_mode: None,
            claude_auto_generate_md: None,
        };
        let merged = merge_partial_config(&base, &partial);
        assert_eq!(merged.poll_interval_secs, 60);
        assert_eq!(merged.max_active_workspaces, 3); // unchanged
    }

    #[test]
    fn test_merge_partial_config_clears_optional_with_explicit_null() {
        let base = AppConfig {
            poll_interval_secs: 300,
            max_active_workspaces: 3,
            archive_delay_hours: 24,
            archive_delay_closed_hours: 48,
            auto_suspend_minutes: 30,
            github_token: Some("ghp_old".into()),
            data_dir: Some("/data".into()),
            workspaces_dir: None,
            claude_auth_mode: "oauth".to_string(),
            claude_auto_generate_md: false,
        };
        // Double-option: Some(None) means "explicitly set to null"
        let partial = PartialAppConfig {
            poll_interval_secs: None,
            max_active_workspaces: None,
            archive_delay_hours: None,
            archive_delay_closed_hours: None,
            auto_suspend_minutes: None,
            github_token: Some(None), // clear it
            data_dir: None,           // leave as-is
            workspaces_dir: None,
            claude_auth_mode: None,
            claude_auto_generate_md: None,
        };
        let merged = merge_partial_config(&base, &partial);
        assert!(
            merged.github_token.is_none(),
            "github_token should be cleared"
        );
        assert_eq!(
            merged.data_dir.as_deref(),
            Some("/data"),
            "data_dir unchanged"
        );
    }

    #[tokio::test]
    async fn test_resolve_username_skips_poisoned_cache() {
        // A poisoned lock must not permanently break the dashboard;
        // resolve_username should fall through to token validation.
        let cached = GithubUsername::default();

        // Poison the lock by panicking inside a thread that holds it.
        let cached_ref = &cached;
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = cached_ref.0.lock().unwrap();
            panic!("intentional poison");
        }));

        // Lock is now poisoned — resolve_username should NOT return
        // a lock error; it should skip the cache and try token validation.
        // The result may be Ok (if a real token exists in the keychain) or
        // Err (no token) — either is acceptable as long as it's not a lock error.
        let result = resolve_username(&cached).await;
        match result {
            Ok(_) => {}
            Err(err) => assert!(
                !err.contains("lock error"),
                "poisoned lock should not bubble up as lock error, got: {err}"
            ),
        }
    }

    // -- Workspace IPC contract tests (T-040) --

    #[test]
    fn test_workspace_serializes_camel_case() {
        let ws = crate::types::Workspace {
            id: "ws-1".into(),
            repo_id: "repo-1".into(),
            pull_request_number: 42,
            state: crate::types::WorkspaceState::Active,
            worktree_path: Some("/home/user/.prism/workspaces/prism/worktrees/pr-42".into()),
            session_id: Some("session-abc".into()),
            created_at: "2026-03-27T10:00:00Z".into(),
            updated_at: "2026-03-27T10:00:00Z".into(),
        };
        let json = serde_json::to_value(&ws).unwrap();
        assert_eq!(json["id"], "ws-1");
        assert_eq!(json["repoId"], "repo-1");
        assert_eq!(json["pullRequestNumber"], 42);
        assert_eq!(json["state"], "active");
        assert_eq!(
            json["worktreePath"],
            "/home/user/.prism/workspaces/prism/worktrees/pr-42"
        );
        assert_eq!(json["sessionId"], "session-abc");
        assert_eq!(json["createdAt"], "2026-03-27T10:00:00Z");
        assert_eq!(json["updatedAt"], "2026-03-27T10:00:00Z");
    }

    #[test]
    fn test_workspace_note_serializes_camel_case() {
        let note = crate::types::WorkspaceNote {
            id: "wn-1".into(),
            workspace_id: "ws-1".into(),
            content: "LGTM".into(),
            created_at: "2026-03-27T11:00:00Z".into(),
        };
        let json = serde_json::to_value(&note).unwrap();
        assert_eq!(json["id"], "wn-1");
        assert_eq!(json["workspaceId"], "ws-1");
        assert_eq!(json["content"], "LGTM");
        assert_eq!(json["createdAt"], "2026-03-27T11:00:00Z");
    }

    #[tokio::test]
    async fn test_workspace_list_via_pool() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = crate::cache::db::init_db(&tmp.path().join("test.db"))
            .await
            .unwrap();

        let repo = crate::types::Repo {
            id: "repo-1".into(),
            org: "mpiton".into(),
            name: "prism".into(),
            full_name: "mpiton/prism".into(),
            url: "https://github.com/mpiton/prism".into(),
            default_branch: "main".into(),
            is_archived: false,
            enabled: true,
            local_path: None,
            last_sync_at: None,
        };
        crate::cache::repos::upsert_repo(&pool, &repo)
            .await
            .unwrap();

        // Empty list
        let result = crate::cache::workspaces::list_workspaces(&pool, None)
            .await
            .unwrap();
        assert!(result.is_empty());

        // Create two workspaces
        let ws1 = crate::types::Workspace {
            id: "ws-1".into(),
            repo_id: "repo-1".into(),
            pull_request_number: 1,
            state: crate::types::WorkspaceState::Active,
            worktree_path: Some("/ws/pr-1".into()),
            session_id: None,
            created_at: "2026-03-27T10:00:00Z".into(),
            updated_at: "2026-03-27T10:00:00Z".into(),
        };
        let ws2 = crate::types::Workspace {
            id: "ws-2".into(),
            repo_id: "repo-1".into(),
            pull_request_number: 2,
            state: crate::types::WorkspaceState::Suspended,
            worktree_path: None,
            session_id: None,
            created_at: "2026-03-27T11:00:00Z".into(),
            updated_at: "2026-03-27T11:00:00Z".into(),
        };
        crate::cache::workspaces::create_workspace(&pool, &ws1)
            .await
            .unwrap();
        crate::cache::workspaces::create_workspace(&pool, &ws2)
            .await
            .unwrap();

        // List all — both returned, ordered by updated_at DESC
        let result = crate::cache::workspaces::list_workspaces(&pool, None)
            .await
            .unwrap();
        assert_eq!(result.len(), 2);
        // ws2 has later updated_at so it comes first
        assert_eq!(result[0].id, "ws-2");
        assert_eq!(result[1].id, "ws-1");

        // Verify serialization works for the IPC contract
        let json = serde_json::to_value(&result).unwrap();
        let arr = json.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["pullRequestNumber"], 2);
        assert_eq!(arr[1]["state"], "active");

        pool.close().await;
    }

    #[tokio::test]
    async fn test_workspace_get_notes_via_pool() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = crate::cache::db::init_db(&tmp.path().join("test.db"))
            .await
            .unwrap();

        let repo = crate::types::Repo {
            id: "repo-1".into(),
            org: "mpiton".into(),
            name: "prism".into(),
            full_name: "mpiton/prism".into(),
            url: "https://github.com/mpiton/prism".into(),
            default_branch: "main".into(),
            is_archived: false,
            enabled: true,
            local_path: None,
            last_sync_at: None,
        };
        crate::cache::repos::upsert_repo(&pool, &repo)
            .await
            .unwrap();

        let ws = crate::types::Workspace {
            id: "ws-1".into(),
            repo_id: "repo-1".into(),
            pull_request_number: 42,
            state: crate::types::WorkspaceState::Active,
            worktree_path: Some("/ws/pr-42".into()),
            session_id: None,
            created_at: "2026-03-27T10:00:00Z".into(),
            updated_at: "2026-03-27T10:00:00Z".into(),
        };
        crate::cache::workspaces::create_workspace(&pool, &ws)
            .await
            .unwrap();

        // No notes yet
        let notes = crate::cache::workspaces::get_notes(&pool, "ws-1")
            .await
            .unwrap();
        assert!(notes.is_empty());

        // Add two notes
        let n1 = crate::types::WorkspaceNote {
            id: "wn-1".into(),
            workspace_id: "ws-1".into(),
            content: "First note".into(),
            created_at: "2026-03-27T10:00:00Z".into(),
        };
        let n2 = crate::types::WorkspaceNote {
            id: "wn-2".into(),
            workspace_id: "ws-1".into(),
            content: "Second note".into(),
            created_at: "2026-03-27T11:00:00Z".into(),
        };
        crate::cache::workspaces::add_note(&pool, &n1)
            .await
            .unwrap();
        crate::cache::workspaces::add_note(&pool, &n2)
            .await
            .unwrap();

        // Get notes — ordered by created_at ASC
        let notes = crate::cache::workspaces::get_notes(&pool, "ws-1")
            .await
            .unwrap();
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].id, "wn-1");
        assert_eq!(notes[1].id, "wn-2");

        // No notes for non-existent workspace
        let empty = crate::cache::workspaces::get_notes(&pool, "nonexistent")
            .await
            .unwrap();
        assert!(empty.is_empty());

        // Verify serialization for IPC contract
        let json = serde_json::to_value(&notes).unwrap();
        let arr = json.as_array().unwrap();
        assert_eq!(arr[0]["workspaceId"], "ws-1");
        assert_eq!(arr[1]["content"], "Second note");

        pool.close().await;
    }

    // -- Activity IPC integration tests (T-039) --

    #[tokio::test]
    async fn test_activity_mark_read_via_pool() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = crate::cache::db::init_db(&tmp.path().join("test.db"))
            .await
            .unwrap();

        let repo = crate::types::Repo {
            id: "repo-1".into(),
            org: "mpiton".into(),
            name: "prism".into(),
            full_name: "mpiton/prism".into(),
            url: "https://github.com/mpiton/prism".into(),
            default_branch: "main".into(),
            is_archived: false,
            enabled: true,
            local_path: None,
            last_sync_at: None,
        };
        crate::cache::repos::upsert_repo(&pool, &repo)
            .await
            .unwrap();

        let activity = crate::types::Activity {
            id: "act-1".into(),
            activity_type: crate::types::ActivityType::PrOpened,
            actor: "mpiton".into(),
            repo_id: "repo-1".into(),
            pull_request_id: None,
            issue_id: None,
            message: "Opened PR #42".into(),
            is_read: false,
            created_at: "2026-03-27T10:00:00Z".into(),
        };
        crate::cache::activity::insert_activity(&pool, &activity)
            .await
            .unwrap();

        // First call — unread → true
        let result = mark_read(&pool, "act-1").await.unwrap();
        assert!(result);

        // Second call — already read → false
        let result = mark_read(&pool, "act-1").await.unwrap();
        assert!(!result);

        // Non-existent ID → false
        let result = mark_read(&pool, "nonexistent").await.unwrap();
        assert!(!result);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_activity_mark_all_read_returns_count() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = crate::cache::db::init_db(&tmp.path().join("test.db"))
            .await
            .unwrap();

        let repo = crate::types::Repo {
            id: "repo-1".into(),
            org: "mpiton".into(),
            name: "prism".into(),
            full_name: "mpiton/prism".into(),
            url: "https://github.com/mpiton/prism".into(),
            default_branch: "main".into(),
            is_archived: false,
            enabled: true,
            local_path: None,
            last_sync_at: None,
        };
        crate::cache::repos::upsert_repo(&pool, &repo)
            .await
            .unwrap();

        let a1 = crate::types::Activity {
            id: "act-1".into(),
            activity_type: crate::types::ActivityType::PrOpened,
            actor: "mpiton".into(),
            repo_id: "repo-1".into(),
            pull_request_id: None,
            issue_id: None,
            message: "First".into(),
            is_read: false,
            created_at: "2026-03-27T10:00:00Z".into(),
        };
        let a2 = crate::types::Activity {
            id: "act-2".into(),
            activity_type: crate::types::ActivityType::PrMerged,
            actor: "mpiton".into(),
            repo_id: "repo-1".into(),
            pull_request_id: None,
            issue_id: None,
            message: "Second".into(),
            is_read: false,
            created_at: "2026-03-27T11:00:00Z".into(),
        };
        crate::cache::activity::insert_activity(&pool, &a1)
            .await
            .unwrap();
        crate::cache::activity::insert_activity(&pool, &a2)
            .await
            .unwrap();

        // Both unread → count = 2
        let count = mark_all_read(&pool).await.unwrap();
        assert_eq!(count, 2);

        // All already read → count = 0
        let count = mark_all_read(&pool).await.unwrap();
        assert_eq!(count, 0);

        pool.close().await;
    }

    // -- Workspace open + PTY command tests (T-069) --

    /// Helper: creates a test DB pool + temp dir.
    async fn test_pool() -> (SqlitePool, tempfile::TempDir) {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = crate::cache::db::init_db(&tmp.path().join("test.db"))
            .await
            .unwrap();
        (pool, tmp)
    }

    /// Helper: inserts a sample repo, optionally setting local_path.
    async fn insert_test_repo(pool: &SqlitePool, local_path: Option<&str>) {
        let repo = crate::types::Repo {
            id: "repo-1".into(),
            org: "mpiton".into(),
            name: "prism".into(),
            full_name: "mpiton/prism".into(),
            url: "https://github.com/mpiton/prism".into(),
            default_branch: "main".into(),
            is_archived: false,
            enabled: true,
            local_path: None,
            last_sync_at: None,
        };
        crate::cache::repos::upsert_repo(pool, &repo).await.unwrap();
        if let Some(path) = local_path {
            crate::cache::repos::set_local_path(pool, "repo-1", Some(path))
                .await
                .unwrap();
        }
    }

    /// Helper: creates a bare remote + local clone with a feature branch.
    /// Returns `(tempdir_guard, local_repo_path)`.
    async fn setup_git_repo() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::TempDir::new().unwrap();
        let remote = tmp.path().join("remote.git");
        let local = tmp.path().join("local");

        async fn sh(program: &str, args: &[&str], cwd: &std::path::Path) {
            let output = tokio::process::Command::new(program)
                .args(args)
                .current_dir(cwd)
                .output()
                .await
                .unwrap();
            assert!(
                output.status.success(),
                "{program} {:?} failed: {}",
                args,
                String::from_utf8_lossy(&output.stderr)
            );
        }

        sh(
            "git",
            &["init", "--bare", &remote.to_string_lossy()],
            tmp.path(),
        )
        .await;
        sh(
            "git",
            &["clone", &remote.to_string_lossy(), &local.to_string_lossy()],
            tmp.path(),
        )
        .await;
        sh("git", &["config", "user.email", "test@test.com"], &local).await;
        sh("git", &["config", "user.name", "Test"], &local).await;
        sh("git", &["commit", "--allow-empty", "-m", "initial"], &local).await;
        sh("git", &["push", "origin", "HEAD"], &local).await;
        sh("git", &["checkout", "-b", "feature-42"], &local).await;
        sh(
            "git",
            &["commit", "--allow-empty", "-m", "feature work"],
            &local,
        )
        .await;
        sh("git", &["push", "origin", "feature-42"], &local).await;
        sh("git", &["checkout", "-"], &local).await;

        (tmp, local)
    }

    #[tokio::test]
    async fn test_workspace_open_no_local_path_triggers_clone() {
        let (pool, _tmp) = test_pool().await;
        insert_test_repo(&pool, None).await;

        let pty_state = PtyManagerState::new();
        let req = crate::types::OpenWorkspaceRequest {
            repo_id: "repo-1".into(),
            pull_request_number: 42,
            branch: "feature-42".into(),
        };

        // With no local_path the function attempts an auto-clone.
        // In CI without network access it will fail with a Git error
        // (not a Workspace error), proving the clone path was taken.
        let ws_id = uuid::Uuid::new_v4().to_string();
        let result = workspace_open_inner(&pool, &pty_state, &ws_id, &req, |_, _| {}).await;
        assert!(result.is_err(), "should fail (no real clone target)");
        let err = result.unwrap_err();
        // The error comes from the clone/worktree attempt (Git or Workspace),
        // never the old "no local_path" error.
        assert!(
            matches!(err, AppError::Git(_) | AppError::Workspace(_)),
            "expected Git or Workspace error from clone path, got: {err}"
        );
        assert!(
            !err.to_string().contains("no local_path"),
            "should not get old 'no local_path' error, got: {err}"
        );

        pool.close().await;
    }

    #[tokio::test]
    async fn test_workspace_open_success() {
        let (pool, _db_tmp) = test_pool().await;
        let (_git_tmp, local_repo) = setup_git_repo().await;
        let ws_base = tempfile::TempDir::new().unwrap();

        insert_test_repo(&pool, Some(&local_repo.to_string_lossy())).await;

        // Set workspaces_dir in config so the worktree goes to our temp dir
        crate::cache::config::set_config(
            &pool,
            &crate::types::AppConfig {
                poll_interval_secs: 300,
                max_active_workspaces: 3,
                archive_delay_hours: 24,
                archive_delay_closed_hours: 48,
                auto_suspend_minutes: 30,
                github_token: None,
                data_dir: None,
                workspaces_dir: Some(ws_base.path().to_string_lossy().to_string()),
                claude_auth_mode: "oauth".to_string(),
                claude_auto_generate_md: false,
            },
        )
        .await
        .unwrap();

        let pty_state = PtyManagerState::new();
        let req = crate::types::OpenWorkspaceRequest {
            repo_id: "repo-1".into(),
            pull_request_number: 42,
            branch: "feature-42".into(),
        };

        let ws_id = uuid::Uuid::new_v4().to_string();
        let resp = workspace_open_inner(&pool, &pty_state, &ws_id, &req, |_, _| {}).await;
        assert!(resp.is_ok(), "workspace_open failed: {resp:?}");

        let resp = resp.unwrap();
        assert_eq!(resp.workspace_id, ws_id);
        assert!(!resp.pty_id.is_empty());
        assert!(resp.worktree_path.contains("pr-42"));
        assert!(resp.session_id.is_none());

        // Verify workspace was created in DB
        let workspaces = crate::cache::workspaces::list_workspaces(&pool, None)
            .await
            .unwrap();
        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].id, resp.workspace_id);
        assert_eq!(workspaces[0].state, WorkspaceState::Active);

        // Cleanup
        pty_state.manager.kill(&resp.pty_id).unwrap();
        pool.close().await;
    }

    #[tokio::test]
    async fn test_workspace_open_lru_eviction() {
        let (pool, _db_tmp) = test_pool().await;
        let (_git_tmp, local_repo) = setup_git_repo().await;
        let ws_base = tempfile::TempDir::new().unwrap();

        insert_test_repo(&pool, Some(&local_repo.to_string_lossy())).await;

        // Set max_active_workspaces = 1
        crate::cache::config::set_config(
            &pool,
            &crate::types::AppConfig {
                poll_interval_secs: 300,
                max_active_workspaces: 1,
                archive_delay_hours: 24,
                archive_delay_closed_hours: 48,
                auto_suspend_minutes: 30,
                github_token: None,
                data_dir: None,
                workspaces_dir: Some(ws_base.path().to_string_lossy().to_string()),
                claude_auth_mode: "oauth".to_string(),
                claude_auto_generate_md: false,
            },
        )
        .await
        .unwrap();

        // Pre-insert an active workspace (simulates existing session)
        let existing_ws = crate::types::Workspace {
            id: "ws-existing".into(),
            repo_id: "repo-1".into(),
            pull_request_number: 1,
            state: WorkspaceState::Active,
            worktree_path: Some("/tmp/ws/pr-1".into()),
            session_id: None,
            created_at: "2026-03-28T10:00:00Z".into(),
            updated_at: "2026-03-28T10:00:00Z".into(),
        };
        crate::cache::workspaces::create_workspace(&pool, &existing_ws)
            .await
            .unwrap();

        // Register a fake PTY mapping for the existing workspace
        let pty_state = PtyManagerState::new();
        pty_state.register("ws-existing", "fake-pty-id");

        // Open a new workspace — should succeed and evict the existing one
        let req = crate::types::OpenWorkspaceRequest {
            repo_id: "repo-1".into(),
            pull_request_number: 42,
            branch: "feature-42".into(),
        };
        let ws_id = uuid::Uuid::new_v4().to_string();
        let resp = workspace_open_inner(&pool, &pty_state, &ws_id, &req, |_, _| {}).await;
        assert!(resp.is_ok(), "workspace_open should succeed: {resp:?}");
        let resp = resp.unwrap();

        // Verify the existing workspace was suspended via LRU eviction
        let all = crate::cache::workspaces::list_workspaces(&pool, None)
            .await
            .unwrap();
        let existing = all.iter().find(|w| w.id == "ws-existing").unwrap();
        assert_eq!(
            existing.state,
            WorkspaceState::Suspended,
            "existing workspace should be suspended via LRU eviction"
        );

        // New workspace should be active
        let new_ws = all.iter().find(|w| w.id == ws_id).unwrap();
        assert_eq!(new_ws.state, WorkspaceState::Active);

        // Cleanup
        pty_state.manager.kill(&resp.pty_id).unwrap();
        pool.close().await;
    }

    // -- Workspace state transitions (T-070) --

    #[tokio::test]
    async fn test_suspend_kills_pty() {
        let (pool, _db_tmp) = test_pool().await;
        let (_git_tmp, local_repo) = setup_git_repo().await;
        let ws_base = tempfile::TempDir::new().unwrap();

        insert_test_repo(&pool, Some(&local_repo.to_string_lossy())).await;

        crate::cache::config::set_config(
            &pool,
            &crate::types::AppConfig {
                poll_interval_secs: 300,
                max_active_workspaces: 3,
                archive_delay_hours: 24,
                archive_delay_closed_hours: 48,
                auto_suspend_minutes: 30,
                github_token: None,
                data_dir: None,
                workspaces_dir: Some(ws_base.path().to_string_lossy().to_string()),
                claude_auth_mode: "oauth".to_string(),
                claude_auto_generate_md: false,
            },
        )
        .await
        .unwrap();

        let pty_state = PtyManagerState::new();
        let req = crate::types::OpenWorkspaceRequest {
            repo_id: "repo-1".into(),
            pull_request_number: 42,
            branch: "feature-42".into(),
        };

        let ws_id = uuid::Uuid::new_v4().to_string();
        let resp = workspace_open_inner(&pool, &pty_state, &ws_id, &req, |_, _| {})
            .await
            .unwrap();

        // Suspend the workspace
        let suspended = workspace_suspend_inner(&pool, &pty_state, &ws_id).await;
        assert!(suspended.is_ok(), "suspend should succeed: {suspended:?}");

        // PTY should be killed — writing to it should fail
        let write_result = pty_state.manager.write_pty(&resp.pty_id, b"test");
        assert!(write_result.is_err(), "PTY should be killed after suspend");

        // DB state should be Suspended
        let ws = crate::cache::workspaces::get_workspace(&pool, &ws_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ws.state, WorkspaceState::Suspended);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_resume_spawns_pty() {
        let (pool, _db_tmp) = test_pool().await;
        let (_git_tmp, local_repo) = setup_git_repo().await;
        let ws_base = tempfile::TempDir::new().unwrap();

        insert_test_repo(&pool, Some(&local_repo.to_string_lossy())).await;

        crate::cache::config::set_config(
            &pool,
            &crate::types::AppConfig {
                poll_interval_secs: 300,
                max_active_workspaces: 3,
                archive_delay_hours: 24,
                archive_delay_closed_hours: 48,
                auto_suspend_minutes: 30,
                github_token: None,
                data_dir: None,
                workspaces_dir: Some(ws_base.path().to_string_lossy().to_string()),
                claude_auth_mode: "oauth".to_string(),
                claude_auto_generate_md: false,
            },
        )
        .await
        .unwrap();

        let pty_state = PtyManagerState::new();
        let req = crate::types::OpenWorkspaceRequest {
            repo_id: "repo-1".into(),
            pull_request_number: 42,
            branch: "feature-42".into(),
        };

        let ws_id = uuid::Uuid::new_v4().to_string();
        let _resp = workspace_open_inner(&pool, &pty_state, &ws_id, &req, |_, _| {})
            .await
            .unwrap();

        // Suspend first
        workspace_suspend_inner(&pool, &pty_state, &ws_id)
            .await
            .unwrap();

        // Resume — should spawn a new PTY
        let resume_resp = workspace_resume_inner(&pool, &pty_state, &ws_id, |_, _| {}).await;
        assert!(
            resume_resp.is_ok(),
            "resume should succeed: {resume_resp:?}"
        );

        let resume_resp = resume_resp.unwrap();
        assert_eq!(resume_resp.workspace_id, ws_id);
        assert!(!resume_resp.pty_id.is_empty());

        // PTY should be functional — write should succeed
        let write_result = pty_state
            .manager
            .write_pty(&resume_resp.pty_id, b"echo resumed\n");
        assert!(write_result.is_ok(), "PTY should be usable after resume");

        // DB state should be Active
        let ws = crate::cache::workspaces::get_workspace(&pool, &ws_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ws.state, WorkspaceState::Active);

        // Cleanup
        pty_state.manager.kill(&resume_resp.pty_id).unwrap();
        pool.close().await;
    }

    #[tokio::test]
    async fn test_archive_removes_worktree() {
        let (pool, _db_tmp) = test_pool().await;
        let (_git_tmp, local_repo) = setup_git_repo().await;
        let ws_base = tempfile::TempDir::new().unwrap();

        insert_test_repo(&pool, Some(&local_repo.to_string_lossy())).await;

        crate::cache::config::set_config(
            &pool,
            &crate::types::AppConfig {
                poll_interval_secs: 300,
                max_active_workspaces: 3,
                archive_delay_hours: 24,
                archive_delay_closed_hours: 48,
                auto_suspend_minutes: 30,
                github_token: None,
                data_dir: None,
                workspaces_dir: Some(ws_base.path().to_string_lossy().to_string()),
                claude_auth_mode: "oauth".to_string(),
                claude_auto_generate_md: false,
            },
        )
        .await
        .unwrap();

        let pty_state = PtyManagerState::new();
        let req = crate::types::OpenWorkspaceRequest {
            repo_id: "repo-1".into(),
            pull_request_number: 42,
            branch: "feature-42".into(),
        };

        let ws_id = uuid::Uuid::new_v4().to_string();
        let resp = workspace_open_inner(&pool, &pty_state, &ws_id, &req, |_, _| {})
            .await
            .unwrap();

        let wt_path = PathBuf::from(&resp.worktree_path);
        assert!(wt_path.exists(), "worktree should exist before archive");

        // Archive — should kill PTY, remove worktree, set state to Archived
        let ws = crate::cache::workspaces::get_workspace(&pool, &ws_id)
            .await
            .unwrap()
            .unwrap();
        let archived = workspace_archive_inner(&pool, &pty_state, &ws, Some(&local_repo)).await;
        assert!(archived.is_ok(), "archive should succeed: {archived:?}");

        // PTY should be killed
        let write_result = pty_state.manager.write_pty(&resp.pty_id, b"test");
        assert!(write_result.is_err(), "PTY should be killed after archive");

        // Worktree should be removed
        assert!(
            !wt_path.exists(),
            "worktree should be removed after archive"
        );

        // DB state should be Archived with no worktree_path
        let ws = crate::cache::workspaces::get_workspace(&pool, &ws_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ws.state, WorkspaceState::Archived);
        assert!(
            ws.worktree_path.is_none(),
            "worktree_path should be cleared"
        );

        pool.close().await;
    }

    #[tokio::test]
    async fn test_archive_already_suspended() {
        let (pool, _db_tmp) = test_pool().await;
        let (_git_tmp, local_repo) = setup_git_repo().await;
        let ws_base = tempfile::TempDir::new().unwrap();

        insert_test_repo(&pool, Some(&local_repo.to_string_lossy())).await;

        crate::cache::config::set_config(
            &pool,
            &crate::types::AppConfig {
                poll_interval_secs: 300,
                max_active_workspaces: 3,
                archive_delay_hours: 24,
                archive_delay_closed_hours: 48,
                auto_suspend_minutes: 30,
                github_token: None,
                data_dir: None,
                workspaces_dir: Some(ws_base.path().to_string_lossy().to_string()),
                claude_auth_mode: "oauth".to_string(),
                claude_auto_generate_md: false,
            },
        )
        .await
        .unwrap();

        let pty_state = PtyManagerState::new();
        let req = crate::types::OpenWorkspaceRequest {
            repo_id: "repo-1".into(),
            pull_request_number: 42,
            branch: "feature-42".into(),
        };

        let ws_id = uuid::Uuid::new_v4().to_string();
        let resp = workspace_open_inner(&pool, &pty_state, &ws_id, &req, |_, _| {})
            .await
            .unwrap();

        let wt_path = PathBuf::from(&resp.worktree_path);

        // Suspend first
        workspace_suspend_inner(&pool, &pty_state, &ws_id)
            .await
            .unwrap();

        // Archive from suspended state — should still remove worktree
        let ws = crate::cache::workspaces::get_workspace(&pool, &ws_id)
            .await
            .unwrap()
            .unwrap();
        let archived = workspace_archive_inner(&pool, &pty_state, &ws, Some(&local_repo)).await;
        assert!(
            archived.is_ok(),
            "archive from suspended should succeed: {archived:?}"
        );

        assert!(
            !wt_path.exists(),
            "worktree should be removed after archive"
        );

        let ws = crate::cache::workspaces::get_workspace(&pool, &ws_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ws.state, WorkspaceState::Archived);
        assert!(ws.worktree_path.is_none());

        pool.close().await;
    }

    #[tokio::test]
    async fn test_resume_archived_fails() {
        let (pool, _db_tmp) = test_pool().await;
        let (_git_tmp, local_repo) = setup_git_repo().await;
        let ws_base = tempfile::TempDir::new().unwrap();

        insert_test_repo(&pool, Some(&local_repo.to_string_lossy())).await;

        crate::cache::config::set_config(
            &pool,
            &crate::types::AppConfig {
                poll_interval_secs: 300,
                max_active_workspaces: 3,
                archive_delay_hours: 24,
                archive_delay_closed_hours: 48,
                auto_suspend_minutes: 30,
                github_token: None,
                data_dir: None,
                workspaces_dir: Some(ws_base.path().to_string_lossy().to_string()),
                claude_auth_mode: "oauth".to_string(),
                claude_auto_generate_md: false,
            },
        )
        .await
        .unwrap();

        let pty_state = PtyManagerState::new();
        let req = crate::types::OpenWorkspaceRequest {
            repo_id: "repo-1".into(),
            pull_request_number: 42,
            branch: "feature-42".into(),
        };

        let ws_id = uuid::Uuid::new_v4().to_string();
        let _resp = workspace_open_inner(&pool, &pty_state, &ws_id, &req, |_, _| {})
            .await
            .unwrap();

        // Archive the workspace
        let ws = crate::cache::workspaces::get_workspace(&pool, &ws_id)
            .await
            .unwrap()
            .unwrap();
        workspace_archive_inner(&pool, &pty_state, &ws, Some(&local_repo))
            .await
            .unwrap();

        // Resume should fail — archived workspaces cannot be resumed
        let resume_result = workspace_resume_inner(&pool, &pty_state, &ws_id, |_, _| {}).await;
        assert!(
            resume_result.is_err(),
            "resume of archived workspace should fail"
        );
        let err = resume_result.unwrap_err();
        assert!(
            err.to_string().contains("cannot resume"),
            "error should mention 'cannot resume': {err}"
        );

        pool.close().await;
    }

    #[tokio::test]
    async fn test_pty_write_forwards() {
        let pty_state = PtyManagerState::new();
        let (tx, rx) = std::sync::mpsc::channel();
        let tmp = std::env::temp_dir();

        let pty_id = pty_state
            .manager
            .spawn(&tmp, 80, 24, move |_id, data| {
                let _ = tx.send(data.to_vec());
            })
            .unwrap();

        let workspace_id = "ws-test-write";
        pty_state.register(workspace_id, &pty_id);

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // Verify lookup works
        assert_eq!(
            pty_state.lookup_pty_by_workspace(workspace_id),
            Some(pty_id.clone())
        );

        // Write via workspace→pty lookup (same path as Tauri command)
        let looked_up = pty_state.lookup_pty_by_workspace(workspace_id).unwrap();
        let result = pty_state
            .manager
            .write_pty(&looked_up, b"echo pty_write_test\n");
        assert!(result.is_ok());

        // Verify output contains our marker
        let mut found = false;
        let mut output = String::new();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            if let Ok(data) = rx.recv_timeout(std::time::Duration::from_millis(100)) {
                output.push_str(&String::from_utf8_lossy(&data));
                if output.contains("pty_write_test") {
                    found = true;
                    break;
                }
            }
        }
        assert!(found, "should see 'pty_write_test' in PTY output");

        pty_state.manager.kill(&pty_id).unwrap();
    }

    #[tokio::test]
    async fn test_pty_resize_forwards() {
        let pty_state = PtyManagerState::new();
        let tmp = std::env::temp_dir();

        let pty_id = pty_state.manager.spawn(&tmp, 80, 24, |_, _| {}).unwrap();

        let workspace_id = "ws-test-resize";
        pty_state.register(workspace_id, &pty_id);

        let looked_up = pty_state.lookup_pty_by_workspace(workspace_id).unwrap();
        let result = pty_state.manager.resize(&looked_up, 120, 40);
        assert!(result.is_ok(), "resize should succeed: {result:?}");

        pty_state.manager.kill(&pty_id).unwrap();
    }

    // -- Workspace cleanup (T-071) --

    /// Helper: inserts a PR into the DB for cleanup tests.
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
            head_ref_name: "fix/test-branch".to_string(),
            created_at: "2026-03-01T00:00:00Z".to_string(),
            updated_at: updated_at.to_string(),
        };
        crate::cache::pull_requests::upsert_pull_request(pool, &pr)
            .await
            .unwrap();
    }

    /// Helper: inserts a workspace for cleanup tests (no real worktree).
    fn sample_cleanup_workspace(
        id: &str,
        pr_number: u32,
        state: WorkspaceState,
    ) -> crate::types::Workspace {
        crate::types::Workspace {
            id: id.to_string(),
            repo_id: "repo-1".to_string(),
            pull_request_number: pr_number,
            state,
            worktree_path: None,
            session_id: None,
            created_at: "2026-03-20T10:00:00Z".to_string(),
            updated_at: "2026-03-20T10:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn test_cleanup_merged_pr() {
        let (pool, _tmp) = test_pool().await;
        insert_test_repo(&pool, None).await;
        let pty_state = PtyManagerState::new();

        // Merged PR with old updated_at (well past 24h delay)
        insert_test_pr(
            &pool,
            "pr-1",
            42,
            crate::types::PrState::Merged,
            "2026-03-01T00:00:00Z",
        )
        .await;

        // Workspace linked to this PR
        let ws = sample_cleanup_workspace("ws-1", 42, WorkspaceState::Active);
        crate::cache::workspaces::create_workspace(&pool, &ws)
            .await
            .unwrap();

        let archived = workspace_cleanup_inner(&pool, &pty_state, 24, 48)
            .await
            .unwrap();
        assert_eq!(archived.len(), 1, "should archive 1 workspace");
        assert_eq!(archived[0], "ws-1");

        let ws = crate::cache::workspaces::get_workspace(&pool, "ws-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ws.state, WorkspaceState::Archived);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_cleanup_closed_pr() {
        let (pool, _tmp) = test_pool().await;
        insert_test_repo(&pool, None).await;
        let pty_state = PtyManagerState::new();

        // Closed PR with old updated_at (well past 48h delay)
        insert_test_pr(
            &pool,
            "pr-2",
            99,
            crate::types::PrState::Closed,
            "2026-03-01T00:00:00Z",
        )
        .await;

        let ws = sample_cleanup_workspace("ws-2", 99, WorkspaceState::Suspended);
        crate::cache::workspaces::create_workspace(&pool, &ws)
            .await
            .unwrap();

        let archived = workspace_cleanup_inner(&pool, &pty_state, 24, 48)
            .await
            .unwrap();
        assert_eq!(
            archived.len(),
            1,
            "should archive 1 workspace for closed PR"
        );
        assert_eq!(archived[0], "ws-2");

        let ws = crate::cache::workspaces::get_workspace(&pool, "ws-2")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ws.state, WorkspaceState::Archived);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_cleanup_respects_delay() {
        let (pool, _tmp) = test_pool().await;
        insert_test_repo(&pool, None).await;
        let pty_state = PtyManagerState::new();

        // Merged PR with very recent updated_at (within delay)
        let recent = chrono::Utc::now().to_rfc3339();
        insert_test_pr(&pool, "pr-3", 10, crate::types::PrState::Merged, &recent).await;

        let ws = sample_cleanup_workspace("ws-3", 10, WorkspaceState::Active);
        crate::cache::workspaces::create_workspace(&pool, &ws)
            .await
            .unwrap();

        let archived = workspace_cleanup_inner(&pool, &pty_state, 24, 48)
            .await
            .unwrap();
        assert!(
            archived.is_empty(),
            "should not archive workspace within delay"
        );

        let ws = crate::cache::workspaces::get_workspace(&pool, "ws-3")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            ws.state,
            WorkspaceState::Active,
            "workspace should remain active"
        );

        pool.close().await;
    }

    #[tokio::test]
    async fn test_cleanup_nothing_to_clean() {
        let (pool, _tmp) = test_pool().await;
        insert_test_repo(&pool, None).await;
        let pty_state = PtyManagerState::new();

        // Open PR — should never be cleaned up
        insert_test_pr(
            &pool,
            "pr-4",
            50,
            crate::types::PrState::Open,
            "2026-03-01T00:00:00Z",
        )
        .await;

        let ws = sample_cleanup_workspace("ws-4", 50, WorkspaceState::Active);
        crate::cache::workspaces::create_workspace(&pool, &ws)
            .await
            .unwrap();

        let archived = workspace_cleanup_inner(&pool, &pty_state, 24, 48)
            .await
            .unwrap();
        assert!(
            archived.is_empty(),
            "should not archive workspace for open PR"
        );

        // No workspaces at all
        let (pool2, _tmp2) = test_pool().await;
        let archived = workspace_cleanup_inner(&pool2, &pty_state, 24, 48)
            .await
            .unwrap();
        assert!(
            archived.is_empty(),
            "should return empty with no workspaces"
        );

        pool.close().await;
        pool2.close().await;
    }

    // -- Memory monitoring tests (T-087) --

    #[test]
    fn test_get_process_rss_bytes_returns_nonzero_on_linux() {
        let rss = get_process_rss_bytes();
        #[cfg(target_os = "linux")]
        assert!(rss > 0, "RSS should be > 0 on Linux, got {rss}");
        #[cfg(not(target_os = "linux"))]
        assert_eq!(rss, 0, "RSS should be 0 on non-Linux");
    }

    #[test]
    fn test_get_file_size_bytes_existing_file() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "hello world").unwrap();
        let size = get_file_size_bytes(tmp.path());
        assert_eq!(size, 11, "expected 11 bytes for 'hello world'");
    }

    #[test]
    fn test_get_file_size_bytes_missing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("nonexistent.db");
        let size = get_file_size_bytes(&missing);
        assert_eq!(size, 0, "missing file should return 0");
    }

    // -- SessionMonitor line-buffering + detection tests --

    mod session_monitor_tests {
        use crate::workspace::claude::{detect_auth_error, detect_session_id};

        /// Simplified line-buffer that mimics `SessionMonitor::process_chunk`
        /// without requiring a `SqlitePool` or `tauri::AppHandle`.
        struct TestLineBuffer {
            buffer: String,
            detected_sessions: Vec<String>,
            detected_auth_errors: u32,
        }

        impl TestLineBuffer {
            fn new() -> Self {
                Self {
                    buffer: String::new(),
                    detected_sessions: vec![],
                    detected_auth_errors: 0,
                }
            }

            fn process_chunk(&mut self, data: &[u8]) {
                self.buffer.push_str(&String::from_utf8_lossy(data));
                while let Some(pos) = self.buffer.find('\n') {
                    let line: String = self.buffer.drain(..=pos).collect();
                    if let Some(sid) = detect_session_id(&line) {
                        self.detected_sessions.push(sid);
                    }
                    if detect_auth_error(&line) {
                        self.detected_auth_errors += 1;
                    }
                }
            }
        }

        #[test]
        fn test_detects_session_id_from_complete_line() {
            let mut buf = TestLineBuffer::new();
            buf.process_chunk(b"Session: abc-12345678\n");
            assert_eq!(buf.detected_sessions, vec!["abc-12345678"]);
        }

        #[test]
        fn test_buffers_partial_lines() {
            let mut buf = TestLineBuffer::new();
            buf.process_chunk(b"Session: abc-");
            assert!(
                buf.detected_sessions.is_empty(),
                "should not detect from partial line"
            );
            buf.process_chunk(b"12345678\n");
            assert_eq!(buf.detected_sessions, vec!["abc-12345678"]);
        }

        #[test]
        fn test_detects_multiple_sessions_across_chunks() {
            let mut buf = TestLineBuffer::new();
            buf.process_chunk(
                b"Session: first-session-1\nSome other output\nSession: second-session-2\n",
            );
            assert_eq!(buf.detected_sessions.len(), 2);
            assert_eq!(buf.detected_sessions[0], "first-session-1");
            assert_eq!(buf.detected_sessions[1], "second-session-2");
        }

        #[test]
        fn test_detects_auth_error() {
            let mut buf = TestLineBuffer::new();
            buf.process_chunk(b"Error: 401 Unauthorized\n");
            assert_eq!(buf.detected_auth_errors, 1);
        }

        #[test]
        fn test_no_false_positive_without_newline() {
            let mut buf = TestLineBuffer::new();
            buf.process_chunk(b"Session: abc-12345678");
            assert!(
                buf.detected_sessions.is_empty(),
                "incomplete line should not trigger detection"
            );
            assert!(
                !buf.buffer.is_empty(),
                "partial line should remain in buffer"
            );
        }

        #[test]
        fn test_handles_mixed_content() {
            let mut buf = TestLineBuffer::new();
            buf.process_chunk(b"Starting shell...\nSession: my-session_42\nReady.\n");
            assert_eq!(buf.detected_sessions, vec!["my-session_42"]);
            assert_eq!(buf.detected_auth_errors, 0);
        }
    }
}
