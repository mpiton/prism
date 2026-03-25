use std::sync::LazyLock;
use std::time::Duration;

use crate::error::AppError;

const SERVICE_NAME: &str = "prism-github";
const ACCOUNT_NAME: &str = "default";
#[allow(dead_code)] // Used by validate_token, called from T-026 Tauri commands
const GITHUB_API_URL: &str = "https://api.github.com";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Shared HTTP client with connection pooling and timeout.
static HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent("prism")
        .build()
        .expect("failed to build HTTP client")
});

/// Trait abstracting token storage for testability.
pub(crate) trait TokenStore {
    fn store_token(&self, token: &str) -> Result<(), AppError>;
    fn get_token(&self) -> Result<Option<String>, AppError>;
    fn delete_token(&self) -> Result<(), AppError>;
}

/// Real implementation using the OS keychain via `keyring`.
struct KeyringTokenStore;

impl TokenStore for KeyringTokenStore {
    fn store_token(&self, token: &str) -> Result<(), AppError> {
        let entry = keyring::Entry::new(SERVICE_NAME, ACCOUNT_NAME)
            .map_err(|e| AppError::Auth(format!("keyring error: {e}")))?;
        entry
            .set_password(token)
            .map_err(|e| AppError::Auth(format!("failed to store token: {e}")))
    }

    fn get_token(&self) -> Result<Option<String>, AppError> {
        let entry = keyring::Entry::new(SERVICE_NAME, ACCOUNT_NAME)
            .map_err(|e| AppError::Auth(format!("keyring error: {e}")))?;
        match entry.get_password() {
            Ok(token) => Ok(Some(token)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AppError::Auth(format!("failed to get token: {e}"))),
        }
    }

    fn delete_token(&self) -> Result<(), AppError> {
        let entry = keyring::Entry::new(SERVICE_NAME, ACCOUNT_NAME)
            .map_err(|e| AppError::Auth(format!("keyring error: {e}")))?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AppError::Auth(format!("failed to delete token: {e}"))),
        }
    }
}

/// Stores a GitHub token in the system keychain.
/// Rejects empty or whitespace-only tokens.
#[allow(dead_code)] // Called from T-026 Tauri commands
pub fn store_token(token: &str) -> Result<(), AppError> {
    if token.trim().is_empty() {
        return Err(AppError::Auth("token must not be empty".into()));
    }
    KeyringTokenStore.store_token(token)
}

/// Retrieves the GitHub token from the system keychain.
/// Returns `Ok(None)` if no token is stored. Propagates keychain errors.
#[allow(dead_code)] // Called from T-026 Tauri commands
pub fn get_token() -> Result<Option<String>, AppError> {
    KeyringTokenStore.get_token()
}

/// Deletes the GitHub token from the system keychain.
#[allow(dead_code)] // Called from T-026 Tauri commands
pub fn delete_token() -> Result<(), AppError> {
    KeyringTokenStore.delete_token()
}

/// Validates a GitHub token by calling GET /user on the GitHub API.
/// Returns the authenticated username on success.
#[allow(dead_code)] // Called from T-026 Tauri commands
pub async fn validate_token(token: &str) -> Result<String, AppError> {
    validate_token_with_url(GITHUB_API_URL, token).await
}

/// Internal: validates against a configurable base URL (for testing).
async fn validate_token_with_url(base_url: &str, token: &str) -> Result<String, AppError> {
    let resp = HTTP_CLIENT
        .get(format!("{base_url}/user"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| AppError::Auth(format!("request failed: {e}")))?;

    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(AppError::Auth("invalid or expired token".into()));
    }
    if !resp.status().is_success() {
        return Err(AppError::Auth(format!(
            "GitHub API error: status {}",
            resp.status()
        )));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::Auth(format!("parse error: {e}")))?;

    body["login"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| AppError::Auth("missing login in response".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    /// In-memory mock for `TokenStore`.
    struct MockTokenStore {
        store: RefCell<HashMap<&'static str, String>>,
    }

    impl MockTokenStore {
        fn new() -> Self {
            Self {
                store: RefCell::new(HashMap::new()),
            }
        }
    }

    impl TokenStore for MockTokenStore {
        fn store_token(&self, token: &str) -> Result<(), AppError> {
            self.store
                .borrow_mut()
                .insert(ACCOUNT_NAME, token.to_string());
            Ok(())
        }

        fn get_token(&self) -> Result<Option<String>, AppError> {
            Ok(self.store.borrow().get(ACCOUNT_NAME).cloned())
        }

        fn delete_token(&self) -> Result<(), AppError> {
            self.store.borrow_mut().remove(ACCOUNT_NAME);
            Ok(())
        }
    }

    #[test]
    fn test_store_and_get_token() {
        let store = MockTokenStore::new();
        store.store_token("ghp_test123").unwrap();
        let token = store.get_token().unwrap();
        assert_eq!(token, Some("ghp_test123".to_string()));
    }

    #[test]
    fn test_get_token_none() {
        let store = MockTokenStore::new();
        let token = store.get_token().unwrap();
        assert_eq!(token, None);
    }

    #[test]
    fn test_delete_token() {
        let store = MockTokenStore::new();
        store.store_token("ghp_to_delete").unwrap();
        store.delete_token().unwrap();
        assert_eq!(store.get_token().unwrap(), None);
    }

    #[tokio::test]
    async fn test_validate_token_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/user")
            .match_header("Authorization", "Bearer ghp_valid")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"login": "testuser", "id": 12345}"#)
            .create_async()
            .await;

        let result = validate_token_with_url(&server.url(), "ghp_valid").await;
        assert_eq!(result.unwrap(), "testuser");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_validate_token_invalid() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/user")
            .match_header("Authorization", "Bearer ghp_invalid")
            .with_status(401)
            .with_body(r#"{"message": "Bad credentials"}"#)
            .create_async()
            .await;

        let result = validate_token_with_url(&server.url(), "ghp_invalid").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("invalid or expired token"),
            "expected 'invalid or expired token' in '{err}'"
        );
        mock.assert_async().await;
    }

    #[test]
    fn test_store_empty_token_rejected() {
        let err = store_token("").unwrap_err().to_string();
        assert!(
            err.contains("token must not be empty"),
            "expected 'token must not be empty' in '{err}'"
        );
    }

    #[test]
    fn test_store_whitespace_token_rejected() {
        let err = store_token("   ").unwrap_err().to_string();
        assert!(
            err.contains("token must not be empty"),
            "expected 'token must not be empty' in '{err}'"
        );
    }

    #[tokio::test]
    async fn test_validate_token_missing_login() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/user")
            .match_header("Authorization", "Bearer ghp_no_login")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id": 12345, "name": "Test User"}"#)
            .create_async()
            .await;

        let result = validate_token_with_url(&server.url(), "ghp_no_login").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("missing login in response"),
            "expected 'missing login in response' in '{err}'"
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_validate_token_server_error() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/user")
            .match_header("Authorization", "Bearer ghp_token")
            .with_status(500)
            .with_body(r#"{"message": "Internal Server Error"}"#)
            .create_async()
            .await;

        let result = validate_token_with_url(&server.url(), "ghp_token").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("GitHub API error: status 500"),
            "expected 'GitHub API error: status 500' in '{err}'"
        );
        mock.assert_async().await;
    }
}
