use std::sync::Mutex;

use log::warn;
use serde::Serialize;
use sqlx::SqlitePool;
use tauri::Emitter;

use crate::cache::config::{get_config, set_config};
use crate::cache::dashboard::{assemble_dashboard_data, compute_dashboard_stats};
use crate::cache::repos::{list_repos, set_local_path, set_repo_enabled};
use crate::cache::sync::sync_dashboard;
use crate::error::AppError;
use crate::github::auth;
use crate::github::client::GitHubClient;
use crate::types::{
    AppConfig, DashboardData, DashboardStats, PartialAppConfig, Repo, merge_partial_config,
};

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
/// new identity without re-validating the token.
#[tauri::command]
pub async fn auth_set_token(
    token: String,
    cached: tauri::State<'_, GithubUsername>,
) -> Result<String, String> {
    let username = set_token_inner(token).await?;
    match cached.0.lock() {
        Ok(mut guard) => *guard = Some(username.clone()),
        Err(e) => warn!("failed to update cached username: {e}"),
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
    let (username, token) = resolve_credentials(&cached).await?;
    let client = GitHubClient::new(&token).map_err(|e| e.to_string())?;

    let stats = sync_dashboard(&client, &pool, &username)
        .await
        .map_err(|e| e.to_string())?;

    if let Err(e) = app_handle.emit("github:updated", &stats) {
        warn!("failed to emit github:updated after force sync: {e}");
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
            active_workspaces: 1,
            unread_activity: 5,
        };
        let json = serde_json::to_value(&stats).unwrap();
        assert_eq!(json["pendingReviews"], 3);
        assert_eq!(json["openPrs"], 7);
        assert_eq!(json["openIssues"], 2);
        assert_eq!(json["activeWorkspaces"], 1);
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
            github_token: Some("ghp_test".into()),
            data_dir: None,
            workspaces_dir: Some("/ws".into()),
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["pollIntervalSecs"], 120);
        assert_eq!(json["maxActiveWorkspaces"], 5);
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
            github_token: None,
            data_dir: None,
            workspaces_dir: None,
        };
        let partial = PartialAppConfig {
            poll_interval_secs: Some(60),
            max_active_workspaces: None,
            github_token: None,
            data_dir: None,
            workspaces_dir: None,
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
            github_token: Some("ghp_old".into()),
            data_dir: Some("/data".into()),
            workspaces_dir: None,
        };
        // Double-option: Some(None) means "explicitly set to null"
        let partial = PartialAppConfig {
            poll_interval_secs: None,
            max_active_workspaces: None,
            github_token: Some(None), // clear it
            data_dir: None,           // leave as-is
            workspaces_dir: None,
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
}
