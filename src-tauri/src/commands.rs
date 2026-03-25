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

/// Validates and stores a GitHub token. Returns the authenticated username.
#[tauri::command]
pub async fn auth_set_token(token: String) -> Result<String, String> {
    let token = token.trim().to_string();
    let username = auth::validate_token(&token).await.map_err(String::from)?;
    let token_clone = token.clone();
    tokio::task::spawn_blocking(move || auth::store_token(&token_clone))
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
        Some(ref t) => match auth::validate_token(t).await {
            Ok(username) => Ok(AuthStatus {
                connected: true,
                username: Some(username),
                error: None,
            }),
            Err(AppError::Auth(_)) => Ok(AuthStatus {
                connected: false,
                username: None,
                error: None,
            }),
            Err(e) => Ok(AuthStatus {
                connected: false,
                username: None,
                error: Some(e.to_string()),
            }),
        },
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

    #[test]
    fn test_auth_status_debug_impl() {
        let status = AuthStatus {
            connected: true,
            username: Some("user".into()),
            error: None,
        };
        let debug = format!("{status:?}");
        assert!(debug.contains("AuthStatus"));
        assert!(debug.contains("true"));
        assert!(debug.contains("user"));
    }
}
