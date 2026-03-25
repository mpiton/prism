use serde::Serialize;

use crate::error::AppError;
use crate::github::auth;

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

/// Validates and stores a GitHub token. Returns the authenticated username.
#[tauri::command]
pub async fn auth_set_token(token: String) -> Result<String, String> {
    let token = token.trim().to_string();
    let username = auth::validate_token(&token).await.map_err(String::from)?;
    tokio::task::spawn_blocking(move || auth::store_token(&token))
        .await
        .map_err(|e| format!("task join error: {e}"))?
        .map_err(String::from)?;
    Ok(username)
}

/// Returns the current authentication status.
///
/// If a token is stored, validates it against GitHub to confirm it is still
/// active and returns `connected: true` with the username.
/// Auth errors (invalid/expired token) return `connected: false`.
/// Transient errors (network, rate-limit) return `connected: false` with an
/// `error` field so the frontend can distinguish offline from logged-out.
#[tauri::command]
pub async fn auth_get_status() -> Result<AuthStatus, String> {
    let token: Option<String> = tokio::task::spawn_blocking(auth::get_token)
        .await
        .map_err(|e| format!("task join error: {e}"))?
        .map_err(String::from)?;
    match token {
        Some(ref t) => Ok(status_from_validation(auth::validate_token(t).await)),
        None => Ok(AuthStatus {
            connected: false,
            username: None,
            error: None,
        }),
    }
}

/// Deletes the stored GitHub token (logout).
#[tauri::command]
pub async fn auth_logout() -> Result<(), String> {
    tokio::task::spawn_blocking(auth::delete_token)
        .await
        .map_err(|e| format!("task join error: {e}"))?
        .map_err(String::from)
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
        let result = auth_set_token("".into()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[tokio::test]
    async fn test_auth_set_token_rejects_whitespace() {
        let result = auth_set_token("   ".into()).await;
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
        let status =
            status_from_validation(Err(AppError::Auth("invalid or expired token".into())));
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
        assert!(
            err.contains("timeout"),
            "expected 'timeout' in '{err}'"
        );
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
}
