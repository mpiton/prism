use serde::Serialize;

use crate::github::auth;

/// Authentication status returned by [`auth_get_status`].
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub connected: bool,
    pub username: Option<String>,
}

/// Validates and stores a GitHub token. Returns the authenticated username.
#[tauri::command]
pub async fn auth_set_token(token: String) -> Result<String, String> {
    let token = token.trim();
    let username = auth::validate_token(token).await.map_err(String::from)?;
    auth::store_token(token).map_err(String::from)?;
    Ok(username)
}

/// Returns the current authentication status.
///
/// If a token is stored, validates it against GitHub to confirm it is still
/// active and returns `connected: true` with the username.
/// If no token or validation fails, returns `connected: false`.
#[tauri::command]
pub async fn auth_get_status() -> Result<AuthStatus, String> {
    let token = auth::get_token().map_err(String::from)?;
    match token {
        Some(t) => match auth::validate_token(&t).await {
            Ok(username) => Ok(AuthStatus {
                connected: true,
                username: Some(username),
            }),
            Err(_) => Ok(AuthStatus {
                connected: false,
                username: None,
            }),
        },
        None => Ok(AuthStatus {
            connected: false,
            username: None,
        }),
    }
}

/// Deletes the stored GitHub token (logout).
/// Synchronous: keyring deletion is a blocking syscall with no async I/O.
#[tauri::command]
pub fn auth_logout() -> Result<(), String> {
    auth::delete_token().map_err(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_status_serializes_connected() {
        let status = AuthStatus {
            connected: true,
            username: Some("testuser".into()),
        };
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["connected"], true);
        assert_eq!(json["username"], "testuser");
    }

    #[test]
    fn test_auth_status_serializes_disconnected() {
        let status = AuthStatus {
            connected: false,
            username: None,
        };
        let json = serde_json::to_value(&status).unwrap();
        assert_eq!(json["connected"], false);
        assert!(json["username"].is_null());
    }

    #[test]
    fn test_auth_status_uses_camel_case() {
        let status = AuthStatus {
            connected: true,
            username: Some("user".into()),
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("connected"));
        assert!(json.contains("username"));
        // Verify camelCase — no snake_case keys
        assert!(!json.contains("user_name"));
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
    fn test_auth_logout_returns_ok_type() {
        // Verify the function signature compiles — actual keyring
        // interaction tested in integration tests
        let _: fn() -> Result<(), String> = auth_logout;
    }

    #[test]
    fn test_auth_status_debug_impl() {
        let status = AuthStatus {
            connected: true,
            username: Some("user".into()),
        };
        let debug = format!("{status:?}");
        assert!(debug.contains("AuthStatus"));
        assert!(debug.contains("true"));
        assert!(debug.contains("user"));
    }
}
