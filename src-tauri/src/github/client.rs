#![allow(dead_code)] // Used by T-030 (mapping) and T-032 (sync)

use std::time::Duration;

use graphql_client::GraphQLQuery;

use crate::error::AppError;

const GITHUB_GRAPHQL_URL: &str = "https://api.github.com/graphql";
const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 100;

/// Parsed GitHub API rate limit info from response headers.
#[derive(Debug, Clone)]
pub struct RateLimit {
    pub remaining: u32,
    pub reset: u64,
}

/// GitHub GraphQL API client with rate limiting and retry support.
pub struct GitHubClient {
    client: reqwest::Client,
    token: String,
    graphql_url: String,
}

impl GitHubClient {
    /// Creates a new client targeting the GitHub GraphQL API.
    pub fn new(token: impl Into<String>) -> Result<Self, AppError> {
        Self::with_url(token, GITHUB_GRAPHQL_URL)
    }

    /// Creates a client with a custom GraphQL endpoint (for testing).
    pub(crate) fn with_url(
        token: impl Into<String>,
        graphql_url: impl Into<String>,
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
        })
    }

    /// Executes a GraphQL query and returns the typed response data.
    ///
    /// Retries on network errors with exponential backoff (max 3 retries).
    /// Returns `AppError::Auth` on 401, `AppError::GitHub` on rate limit or other HTTP errors,
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
        let rate_limit = Self::parse_rate_limit(response.headers());

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AppError::Auth("invalid or expired token".into()));
        }

        if response.status() == reqwest::StatusCode::FORBIDDEN {
            if let Some(rl) = &rate_limit
                && rl.remaining == 0
            {
                let reset_at =
                    chrono::DateTime::from_timestamp(i64::try_from(rl.reset).unwrap_or(0), 0)
                        .map_or_else(|| rl.reset.to_string(), |dt| dt.to_rfc3339());
                return Err(AppError::RateLimit { reset_at });
            }
            return Err(AppError::GitHub("forbidden".into()));
        }

        if !response.status().is_success() {
            return Err(AppError::GitHub(format!(
                "unexpected status: {}",
                response.status()
            )));
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

    /// Parses rate limit headers from GitHub API response headers.
    fn parse_rate_limit(headers: &reqwest::header::HeaderMap) -> Option<RateLimit> {
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

        let rl = GitHubClient::parse_rate_limit(&headers).expect("should parse rate limit headers");
        assert_eq!(rl.remaining, 42);
        assert_eq!(rl.reset, 1_700_000_000);
    }

    #[test]
    fn test_rate_limit_header_parsing_missing() {
        let headers = reqwest::header::HeaderMap::new();
        let rate_limit = GitHubClient::parse_rate_limit(&headers);

        assert!(
            rate_limit.is_none(),
            "should return None when headers are missing"
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
        assert!(
            err.to_string().contains("forbidden"),
            "expected 'forbidden' in '{err}'"
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
