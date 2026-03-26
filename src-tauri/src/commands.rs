use std::sync::Mutex;

use log::warn;
use serde::Serialize;
use sqlx::SqlitePool;

use crate::cache::dashboard::assemble_dashboard_data;
use crate::error::AppError;
use crate::github::auth;
use crate::types::DashboardData;

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
