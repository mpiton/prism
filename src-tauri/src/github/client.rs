#![allow(dead_code)] // Used by T-030 (mapping) and T-032 (sync)

use std::time::Duration;

use graphql_client::GraphQLQuery;

use crate::error::AppError;
use crate::github::notifications;
use crate::types::Notification;

const GITHUB_GRAPHQL_URL: &str = "https://api.github.com/graphql";
const GITHUB_REST_BASE_URL: &str = "https://api.github.com";
const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 100;

/// Parsed GitHub API rate limit info from response headers.
#[derive(Debug, Clone)]
pub struct RateLimit {
    pub remaining: u32,
    pub reset: u64,
}

/// Parses GitHub rate-limit headers into a [`RateLimit`].
///
/// Returns `None` when either `X-RateLimit-Remaining` or `X-RateLimit-Reset`
/// is missing or cannot be parsed. Shared between the GraphQL (`client.rs`)
/// and REST (`notifications.rs`) code paths so status coverage and header
/// semantics stay in sync.
pub(crate) fn parse_rate_limit(headers: &reqwest::header::HeaderMap) -> Option<RateLimit> {
    let remaining = headers
        .get("X-RateLimit-Remaining")?
        .to_str()
        .ok()?
        .parse()
        .ok()?;
    let reset = headers
        .get("X-RateLimit-Reset")?
        .to_str()
        .ok()?
        .parse()
        .ok()?;
    Some(RateLimit { remaining, reset })
}

/// Maps rate-limit headers to [`AppError::RateLimit`] when the quota is
/// exhausted (`remaining == 0` AND a parseable `reset` timestamp).
///
/// Returns `None` when either header is missing, garbage, or when the
/// remaining quota is non-zero — callers should then treat the response
/// as a plain error rather than emitting an epoch-0 reset. Shared between
/// `client.rs` and `notifications.rs`.
pub(crate) fn rate_limit_error_from(headers: &reqwest::header::HeaderMap) -> Option<AppError> {
    let rl = parse_rate_limit(headers)?;
    if rl.remaining != 0 {
        return None;
    }
    let reset_at = i64::try_from(rl.reset)
        .ok()
        .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
        .map_or_else(|| rl.reset.to_string(), |dt| dt.to_rfc3339());
    Some(AppError::RateLimit { reset_at })
}

/// GitHub API client with GraphQL + REST support, rate limiting, and retries.
pub struct GitHubClient {
    client: reqwest::Client,
    token: String,
    graphql_url: String,
    rest_base_url: String,
}

impl GitHubClient {
    /// Creates a new client targeting the production GitHub API.
    pub fn new(token: impl Into<String>) -> Result<Self, AppError> {
        Self::with_urls(token, GITHUB_GRAPHQL_URL, GITHUB_REST_BASE_URL)
    }

    /// Creates a client with a custom GraphQL endpoint (for testing).
    pub(crate) fn with_url(
        token: impl Into<String>,
        graphql_url: impl Into<String>,
    ) -> Result<Self, AppError> {
        Self::with_urls(token, graphql_url, GITHUB_REST_BASE_URL)
    }

    /// Creates a client with custom GraphQL and REST endpoints (for testing).
    pub(crate) fn with_urls(
        token: impl Into<String>,
        graphql_url: impl Into<String>,
        rest_base_url: impl Into<String>,
    ) -> Result<Self, AppError> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("prism")
            .build()
            .map_err(|e| AppError::GitHub(format!("failed to build HTTP client: {e}")))?;

        Ok(Self {
            client,
            token: token.into(),
            graphql_url: graphql_url.into(),
            rest_base_url: rest_base_url.into(),
        })
    }

    /// Fetches notifications from the authenticated user's inbox.
    ///
    /// When `all` is `false`, only unread notifications are returned.
    pub async fn list_notifications(&self, all: bool) -> Result<Vec<Notification>, AppError> {
        notifications::fetch_notifications(&self.client, &self.token, &self.rest_base_url, all)
            .await
    }

    /// Executes a GraphQL query and returns the typed response data.
    ///
    /// Retries on network errors with exponential backoff (max 3 retries).
    /// Returns `AppError::Auth` on 401, `AppError::RateLimit` when the 403 response
    /// carries rate-limit headers with `remaining == 0`, `AppError::GitHub` on other HTTP errors,
    /// and `AppError::GraphQL` on GraphQL-level errors.
    pub async fn execute_graphql<Q: GraphQLQuery>(
        &self,
        variables: Q::Variables,
    ) -> Result<Q::ResponseData, AppError> {
        let body = Q::build_query(variables);

        let mut last_error = None;
        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                let backoff = Duration::from_millis(INITIAL_BACKOFF_MS * 2u64.pow(attempt - 1));
                tokio::time::sleep(backoff).await;
            }

            match self.send_request(&body).await {
                Ok(response) => return self.handle_response::<Q>(response).await,
                Err(e) if e.is_connect() || e.is_timeout() => {
                    last_error = Some(e);
                }
                Err(e) => {
                    return Err(AppError::GitHub(format!("request failed: {e}")));
                }
            }
        }

        Err(AppError::GitHub(format!(
            "request failed after {MAX_RETRIES} retries: {}",
            last_error.map_or_else(|| "unknown error".to_string(), |e| e.to_string())
        )))
    }

    async fn send_request(
        &self,
        body: &impl serde::Serialize,
    ) -> Result<reqwest::Response, reqwest::Error> {
        self.client
            .post(&self.graphql_url)
            .header("Authorization", format!("Bearer {}", self.token))
            .json(body)
            .send()
            .await
    }

    async fn handle_response<Q: GraphQLQuery>(
        &self,
        response: reqwest::Response,
    ) -> Result<Q::ResponseData, AppError> {
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AppError::Auth("invalid or expired token".into()));
        }

        // Treat both 403 Forbidden and 429 Too Many Requests as potential
        // rate-limit responses — GitHub uses either for primary/secondary
        // limits. Shared helper keeps this in sync with the REST path.
        let status = response.status();
        if status == reqwest::StatusCode::FORBIDDEN
            || status == reqwest::StatusCode::TOO_MANY_REQUESTS
        {
            if let Some(err) = rate_limit_error_from(response.headers()) {
                return Err(err);
            }
            // Preserve the real status so callers can tell 403 from 429.
            return Err(AppError::GitHub(format!("{status}")));
        }

        if !status.is_success() {
            return Err(AppError::GitHub(format!("unexpected status: {status}")));
        }

        let body: graphql_client::Response<Q::ResponseData> = response
            .json()
            .await
            .map_err(|e| AppError::GitHub(format!("failed to parse response: {e}")))?;

        if let Some(errors) = body.errors
            && !errors.is_empty()
        {
            let messages: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
            return Err(AppError::GraphQL(messages.join("; ")));
        }

        body.data
            .ok_or_else(|| AppError::GraphQL("no data in response".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphql_client::QueryBody;
    use serde::{Deserialize, Serialize};

    // Minimal test query implementing GraphQLQuery manually
    struct TestQuery;

    #[derive(Debug, Serialize)]
    struct TestVariables {
        login: String,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct TestResponseData {
        viewer: TestViewer,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct TestViewer {
        login: String,
    }

    impl GraphQLQuery for TestQuery {
        type Variables = TestVariables;
        type ResponseData = TestResponseData;

        fn build_query(variables: Self::Variables) -> QueryBody<Self::Variables> {
            QueryBody {
                variables,
                query: "query TestQuery($login: String!) { viewer { login } }",
                operation_name: "TestQuery",
            }
        }
    }

    fn test_variables() -> TestVariables {
        TestVariables {
            login: "testuser".into(),
        }
    }

    #[tokio::test]
    async fn test_execute_graphql_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/graphql")
            .match_header("Authorization", "Bearer ghp_test_token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_header("X-RateLimit-Remaining", "4999")
            .with_header("X-RateLimit-Reset", "1700000000")
            .with_body(r#"{"data": {"viewer": {"login": "testuser"}}}"#)
            .create_async()
            .await;

        let client =
            GitHubClient::with_url("ghp_test_token", format!("{}/graphql", server.url())).unwrap();
        let result = client.execute_graphql::<TestQuery>(test_variables()).await;

        let data = result.unwrap();
        assert_eq!(data.viewer.login, "testuser");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_execute_graphql_401() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/graphql")
            .with_status(401)
            .with_body(r#"{"message": "Bad credentials"}"#)
            .create_async()
            .await;

        let client =
            GitHubClient::with_url("ghp_bad_token", format!("{}/graphql", server.url())).unwrap();
        let result = client.execute_graphql::<TestQuery>(test_variables()).await;

        let err = result.unwrap_err();
        assert!(
            matches!(err, AppError::Auth(_)),
            "expected AppError::Auth, got {err:?}"
        );
        assert!(
            err.to_string().contains("invalid or expired token"),
            "expected 'invalid or expired token' in '{err}'"
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_execute_graphql_rate_limited() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/graphql")
            .with_status(403)
            .with_header("X-RateLimit-Remaining", "0")
            .with_header("X-RateLimit-Reset", "1700000000")
            .with_body(r#"{"message": "API rate limit exceeded"}"#)
            .create_async()
            .await;

        let client =
            GitHubClient::with_url("ghp_test_token", format!("{}/graphql", server.url())).unwrap();
        let result = client.execute_graphql::<TestQuery>(test_variables()).await;

        let err = result.unwrap_err();
        assert!(
            matches!(err, AppError::RateLimit { .. }),
            "expected AppError::RateLimit, got {err:?}"
        );
        let msg = err.to_string();
        assert!(
            msg.contains("rate limited"),
            "expected 'rate limited' in '{msg}'"
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_execute_graphql_network_error_retry() {
        // Point to a port with no listener to trigger connection error.
        let client = GitHubClient {
            client: reqwest::Client::builder()
                .timeout(Duration::from_millis(100))
                .user_agent("prism")
                .build()
                .expect("failed to build test client"),
            token: "ghp_test_token".into(),
            graphql_url: "http://127.0.0.1:1/graphql".into(),
            rest_base_url: "http://127.0.0.1:1".into(),
        };

        let start = std::time::Instant::now();
        let result = client.execute_graphql::<TestQuery>(test_variables()).await;
        let elapsed = start.elapsed();

        let err = result.unwrap_err();
        assert!(
            matches!(err, AppError::GitHub(_)),
            "expected AppError::GitHub, got {err:?}"
        );
        assert!(
            err.to_string().contains("retries"),
            "expected 'retries' in '{}'",
            err
        );
        // With 3 retries and backoff (100ms, 200ms, 400ms), should take at least 700ms
        assert!(
            elapsed >= Duration::from_millis(500),
            "expected retries with backoff, but elapsed only {elapsed:?}"
        );
    }

    #[test]
    fn test_rate_limit_header_parsing() {
        use reqwest::header::{HeaderMap, HeaderValue};

        let mut headers = HeaderMap::new();
        headers.insert("X-RateLimit-Remaining", HeaderValue::from_static("42"));
        headers.insert("X-RateLimit-Reset", HeaderValue::from_static("1700000000"));

        let rl = parse_rate_limit(&headers).expect("should parse rate limit headers");
        assert_eq!(rl.remaining, 42);
        assert_eq!(rl.reset, 1_700_000_000);
    }

    #[test]
    fn test_rate_limit_header_parsing_missing() {
        let headers = reqwest::header::HeaderMap::new();
        let rate_limit = parse_rate_limit(&headers);

        assert!(
            rate_limit.is_none(),
            "should return None when headers are missing"
        );
    }

    #[test]
    fn test_rate_limit_error_from_returns_none_when_quota_remaining() {
        use reqwest::header::{HeaderMap, HeaderValue};

        let mut headers = HeaderMap::new();
        headers.insert("X-RateLimit-Remaining", HeaderValue::from_static("100"));
        headers.insert("X-RateLimit-Reset", HeaderValue::from_static("1700000000"));

        assert!(
            rate_limit_error_from(&headers).is_none(),
            "quota remaining should not produce a rate-limit error"
        );
    }

    #[test]
    fn test_rate_limit_error_from_returns_rate_limit_when_exhausted() {
        use reqwest::header::{HeaderMap, HeaderValue};

        let mut headers = HeaderMap::new();
        headers.insert("X-RateLimit-Remaining", HeaderValue::from_static("0"));
        headers.insert("X-RateLimit-Reset", HeaderValue::from_static("1700000000"));

        let err = rate_limit_error_from(&headers).expect("should produce rate-limit error");
        assert!(
            matches!(err, AppError::RateLimit { .. }),
            "expected RateLimit variant, got {err:?}"
        );
    }

    #[test]
    fn test_rate_limit_error_from_returns_none_when_headers_missing() {
        let headers = reqwest::header::HeaderMap::new();
        assert!(
            rate_limit_error_from(&headers).is_none(),
            "missing headers should not produce a rate-limit error"
        );
    }

    #[tokio::test]
    async fn test_execute_graphql_403_without_rate_limit_headers() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/graphql")
            .with_status(403)
            .with_body(r#"{"message": "Forbidden"}"#)
            .create_async()
            .await;

        let client =
            GitHubClient::with_url("ghp_test_token", format!("{}/graphql", server.url())).unwrap();
        let result = client.execute_graphql::<TestQuery>(test_variables()).await;

        let err = result.unwrap_err();
        assert!(
            matches!(err, AppError::GitHub(_)),
            "expected AppError::GitHub, got {err:?}"
        );
        // The fallback message now preserves the real HTTP status so callers
        // can distinguish 403 Forbidden from 429 Too Many Requests.
        let msg = err.to_string();
        assert!(
            msg.contains("403"),
            "expected '403' in error message, got '{msg}'"
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_execute_graphql_429_without_rate_limit_headers_preserves_status() {
        // Regression test for the coderabbit finding: a 429 falling through
        // to the fallback branch must NOT be reported as "forbidden" — it
        // should preserve the real HTTP status in the error message.
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/graphql")
            .with_status(429)
            .with_body(r#"{"message": "Too Many Requests"}"#)
            .create_async()
            .await;

        let client =
            GitHubClient::with_url("ghp_test_token", format!("{}/graphql", server.url())).unwrap();
        let err = client
            .execute_graphql::<TestQuery>(test_variables())
            .await
            .expect_err("should fail with github error");

        assert!(
            matches!(err, AppError::GitHub(_)),
            "expected AppError::GitHub, got {err:?}"
        );
        let msg = err.to_string();
        assert!(
            msg.contains("429"),
            "expected '429' in error message, got '{msg}'"
        );
        assert!(
            !msg.contains("forbidden"),
            "error must not claim 'forbidden' for a 429, got '{msg}'"
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_execute_graphql_graphql_errors() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/graphql")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"data": null, "errors": [{"message": "Field 'foo' not found"}]}"#)
            .create_async()
            .await;

        let client =
            GitHubClient::with_url("ghp_test_token", format!("{}/graphql", server.url())).unwrap();
        let result = client.execute_graphql::<TestQuery>(test_variables()).await;

        let err = result.unwrap_err();
        assert!(
            matches!(err, AppError::GraphQL(_)),
            "expected AppError::GraphQL, got {err:?}"
        );
        assert!(
            err.to_string().contains("Field 'foo' not found"),
            "expected error message in '{err}'"
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_execute_graphql_null_data() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/graphql")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"data": null}"#)
            .create_async()
            .await;

        let client =
            GitHubClient::with_url("ghp_test_token", format!("{}/graphql", server.url())).unwrap();
        let result = client.execute_graphql::<TestQuery>(test_variables()).await;

        let err = result.unwrap_err();
        assert!(
            matches!(err, AppError::GraphQL(_)),
            "expected AppError::GraphQL, got {err:?}"
        );
        assert!(
            err.to_string().contains("no data"),
            "expected 'no data' in '{err}'"
        );
        mock.assert_async().await;
    }
}
