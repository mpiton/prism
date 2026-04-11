//! GitHub notifications — REST API fetcher and model mapping.
//!
//! The GitHub Notifications API is only exposed via REST (not GraphQL),
//! so this module bridges the REST `/notifications` endpoint into the
//! typed domain model used by the rest of the application.

use std::time::Duration;

use serde::Deserialize;

use crate::error::AppError;
use crate::types::{Notification, NotificationSubjectType};

const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 100;

/// Raw notification envelope as returned by `GET /notifications`.
#[derive(Debug, Clone, Deserialize)]
struct RawNotification {
    id: String,
    unread: bool,
    reason: String,
    updated_at: String,
    subject: RawSubject,
    repository: RawRepository,
}

#[derive(Debug, Clone, Deserialize)]
struct RawSubject {
    title: String,
    url: Option<String>,
    #[serde(rename = "type")]
    subject_type: String,
}

#[derive(Debug, Clone, Deserialize)]
struct RawRepository {
    full_name: String,
    html_url: String,
}

/// Parse a GitHub subject-type string into the typed enum.
///
/// Unknown types fall back to [`NotificationSubjectType::Other`] rather
/// than failing the whole fetch — the API may add new kinds in the future.
fn parse_subject_type(raw: &str) -> NotificationSubjectType {
    match raw {
        "PullRequest" => NotificationSubjectType::PullRequest,
        "Issue" => NotificationSubjectType::Issue,
        "Release" => NotificationSubjectType::Release,
        "Discussion" => NotificationSubjectType::Discussion,
        "CheckSuite" => NotificationSubjectType::CheckSuite,
        "Commit" => NotificationSubjectType::Commit,
        _ => NotificationSubjectType::Other,
    }
}

/// Convert a GitHub API URL (from `subject.url`) into the public HTML URL
/// suitable for opening in a browser.
///
/// Falls back to `repo_html_url` when the API URL is missing or cannot be
/// translated (e.g., `Discussion`, `CheckSuite` — which have no direct HTML URL).
pub(crate) fn build_html_url(
    api_url: Option<&str>,
    subject_type: &NotificationSubjectType,
    repo_html_url: &str,
) -> String {
    let Some(api_url) = api_url else {
        return repo_html_url.to_string();
    };

    let Some(stripped) = api_url.strip_prefix("https://api.github.com/repos") else {
        return repo_html_url.to_string();
    };

    let path = match subject_type {
        NotificationSubjectType::PullRequest => stripped.replacen("/pulls/", "/pull/", 1),
        NotificationSubjectType::Issue | NotificationSubjectType::Release => stripped.to_string(),
        _ => return repo_html_url.to_string(),
    };

    format!("https://github.com{path}")
}

/// Map a raw GitHub response notification into the domain [`Notification`].
fn map_notification(raw: RawNotification) -> Notification {
    let subject_type = parse_subject_type(&raw.subject.subject_type);
    let url = build_html_url(
        raw.subject.url.as_deref(),
        &subject_type,
        &raw.repository.html_url,
    );

    Notification {
        id: raw.id,
        repo: raw.repository.full_name,
        title: raw.subject.title,
        notification_type: subject_type,
        reason: raw.reason,
        unread: raw.unread,
        updated_at: raw.updated_at,
        url,
    }
}

/// Fetch notifications from the GitHub REST API.
///
/// Retries on connection and timeout errors only (max 3 attempts with
/// exponential backoff). Other transient failures (DNS, TLS handshake) are
/// surfaced immediately — this matches the behaviour of `execute_graphql`.
///
/// Returns up to 100 notifications (one REST page). Older notifications are
/// silently truncated; subsequent pages are intentionally not followed to
/// keep the command stateless and cheap.
///
/// * `rest_base_url` is the REST API origin (e.g., `https://api.github.com`).
/// * `all` — when `false`, only unread notifications are returned.
pub(crate) async fn fetch_notifications(
    client: &reqwest::Client,
    token: &str,
    rest_base_url: &str,
    all: bool,
) -> Result<Vec<Notification>, AppError> {
    // per_page=100 is the documented maximum for GitHub REST endpoints.
    let url = format!("{rest_base_url}/notifications?all={all}&per_page=100");

    let mut last_error = None;
    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            let backoff = Duration::from_millis(INITIAL_BACKOFF_MS * 2u64.pow(attempt - 1));
            tokio::time::sleep(backoff).await;
        }

        match send(client, token, &url).await {
            Ok(response) => return handle_response(response).await,
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

async fn send(
    client: &reqwest::Client,
    token: &str,
    url: &str,
) -> Result<reqwest::Response, reqwest::Error> {
    client
        .get(url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
}

async fn handle_response(response: reqwest::Response) -> Result<Vec<Notification>, AppError> {
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(AppError::Auth("invalid or expired token".into()));
    }

    if response.status() == reqwest::StatusCode::FORBIDDEN {
        // Both headers must be present AND parse for us to treat the 403 as
        // a rate limit. If either header is missing/garbage, fall through to
        // a plain `forbidden` error rather than emitting an epoch-0 reset.
        let headers = response.headers();
        let remaining = headers
            .get("X-RateLimit-Remaining")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u32>().ok());
        let reset = headers
            .get("X-RateLimit-Reset")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok());
        if let (Some(0), Some(reset)) = (remaining, reset) {
            let reset_at = i64::try_from(reset)
                .ok()
                .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
                .map_or_else(|| reset.to_string(), |dt| dt.to_rfc3339());
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

    let raw: Vec<RawNotification> = response
        .json()
        .await
        .map_err(|e| AppError::GitHub(format!("failed to parse notifications response: {e}")))?;

    Ok(raw.into_iter().map(map_notification).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_notification() -> RawNotification {
        RawNotification {
            id: "42".into(),
            unread: true,
            reason: "review_requested".into(),
            updated_at: "2026-04-01T10:00:00Z".into(),
            subject: RawSubject {
                title: "Fix the bug".into(),
                url: Some("https://api.github.com/repos/octocat/Hello-World/pulls/7".into()),
                subject_type: "PullRequest".into(),
            },
            repository: RawRepository {
                full_name: "octocat/Hello-World".into(),
                html_url: "https://github.com/octocat/Hello-World".into(),
            },
        }
    }

    #[test]
    fn parse_subject_type_maps_known_types() {
        assert_eq!(
            parse_subject_type("PullRequest"),
            NotificationSubjectType::PullRequest
        );
        assert_eq!(parse_subject_type("Issue"), NotificationSubjectType::Issue);
        assert_eq!(
            parse_subject_type("Release"),
            NotificationSubjectType::Release
        );
        assert_eq!(
            parse_subject_type("Discussion"),
            NotificationSubjectType::Discussion
        );
        assert_eq!(
            parse_subject_type("CheckSuite"),
            NotificationSubjectType::CheckSuite
        );
        assert_eq!(
            parse_subject_type("Commit"),
            NotificationSubjectType::Commit
        );
    }

    #[test]
    fn parse_subject_type_falls_back_to_other_for_unknown() {
        assert_eq!(
            parse_subject_type("SomethingNew"),
            NotificationSubjectType::Other
        );
    }

    #[test]
    fn build_html_url_converts_pull_request_api_url() {
        let url = build_html_url(
            Some("https://api.github.com/repos/octocat/Hello-World/pulls/42"),
            &NotificationSubjectType::PullRequest,
            "https://github.com/octocat/Hello-World",
        );
        assert_eq!(url, "https://github.com/octocat/Hello-World/pull/42");
    }

    #[test]
    fn build_html_url_keeps_issue_path() {
        let url = build_html_url(
            Some("https://api.github.com/repos/octocat/Hello-World/issues/7"),
            &NotificationSubjectType::Issue,
            "https://github.com/octocat/Hello-World",
        );
        assert_eq!(url, "https://github.com/octocat/Hello-World/issues/7");
    }

    #[test]
    fn build_html_url_falls_back_to_repo_when_subject_url_missing() {
        let url = build_html_url(
            None,
            &NotificationSubjectType::Discussion,
            "https://github.com/octocat/Hello-World",
        );
        assert_eq!(url, "https://github.com/octocat/Hello-World");
    }

    #[test]
    fn build_html_url_falls_back_for_unmapped_type() {
        let url = build_html_url(
            Some("https://api.github.com/repos/octocat/Hello-World/check-suites/1"),
            &NotificationSubjectType::CheckSuite,
            "https://github.com/octocat/Hello-World",
        );
        assert_eq!(url, "https://github.com/octocat/Hello-World");
    }

    #[test]
    fn build_html_url_falls_back_when_prefix_unknown() {
        let url = build_html_url(
            Some("https://custom.example.com/something"),
            &NotificationSubjectType::PullRequest,
            "https://github.com/octocat/Hello-World",
        );
        assert_eq!(url, "https://github.com/octocat/Hello-World");
    }

    #[test]
    fn map_notification_converts_raw_to_domain() {
        let n = map_notification(raw_notification());
        assert_eq!(n.id, "42");
        assert!(n.unread);
        assert_eq!(n.reason, "review_requested");
        assert_eq!(n.title, "Fix the bug");
        assert_eq!(n.repo, "octocat/Hello-World");
        assert_eq!(n.notification_type, NotificationSubjectType::PullRequest);
        assert_eq!(n.url, "https://github.com/octocat/Hello-World/pull/7");
        assert_eq!(n.updated_at, "2026-04-01T10:00:00Z");
    }

    fn test_client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .user_agent("prism-test")
            .build()
            .expect("failed to build test client")
    }

    #[tokio::test]
    async fn fetch_notifications_returns_parsed_list() {
        let mut server = mockito::Server::new_async().await;
        let body = r#"[{
            "id": "1",
            "unread": true,
            "reason": "review_requested",
            "updated_at": "2026-04-01T10:00:00Z",
            "subject": {
                "title": "Fix the bug",
                "url": "https://api.github.com/repos/octocat/Hello-World/pulls/42",
                "type": "PullRequest"
            },
            "repository": {
                "full_name": "octocat/Hello-World",
                "html_url": "https://github.com/octocat/Hello-World"
            }
        }, {
            "id": "2",
            "unread": false,
            "reason": "mention",
            "updated_at": "2026-04-02T10:00:00Z",
            "subject": {
                "title": "Crash on startup",
                "url": "https://api.github.com/repos/octocat/Hello-World/issues/9",
                "type": "Issue"
            },
            "repository": {
                "full_name": "octocat/Hello-World",
                "html_url": "https://github.com/octocat/Hello-World"
            }
        }]"#;

        let mock = server
            .mock("GET", "/notifications?all=false&per_page=100")
            .match_header("Authorization", "Bearer ghp_test_token")
            .match_header("Accept", "application/vnd.github+json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create_async()
            .await;

        let result =
            fetch_notifications(&test_client(), "ghp_test_token", &server.url(), false).await;
        let notifications = result.expect("should return notifications");

        assert_eq!(notifications.len(), 2);
        assert_eq!(notifications[0].id, "1");
        assert_eq!(
            notifications[0].notification_type,
            NotificationSubjectType::PullRequest
        );
        assert_eq!(
            notifications[0].url,
            "https://github.com/octocat/Hello-World/pull/42"
        );
        assert!(notifications[0].unread);

        assert_eq!(notifications[1].id, "2");
        assert_eq!(
            notifications[1].notification_type,
            NotificationSubjectType::Issue
        );
        assert_eq!(
            notifications[1].url,
            "https://github.com/octocat/Hello-World/issues/9"
        );
        assert!(!notifications[1].unread);

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn fetch_notifications_returns_empty_list() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/notifications?all=false&per_page=100")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("[]")
            .create_async()
            .await;

        let result =
            fetch_notifications(&test_client(), "ghp_test_token", &server.url(), false).await;

        let notifications = result.expect("should return empty list");
        assert!(notifications.is_empty());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn fetch_notifications_handles_all_true_query_param() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/notifications?all=true&per_page=100")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("[]")
            .create_async()
            .await;

        let result =
            fetch_notifications(&test_client(), "ghp_test_token", &server.url(), true).await;

        assert!(result.is_ok());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn fetch_notifications_maps_401_to_auth_error() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/notifications?all=false&per_page=100")
            .with_status(401)
            .with_body(r#"{"message": "Bad credentials"}"#)
            .create_async()
            .await;

        let err = fetch_notifications(&test_client(), "ghp_bad", &server.url(), false)
            .await
            .expect_err("should fail with auth error");

        assert!(
            matches!(err, AppError::Auth(_)),
            "expected AppError::Auth, got {err:?}"
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn fetch_notifications_maps_rate_limit_to_rate_limit_error() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/notifications?all=false&per_page=100")
            .with_status(403)
            .with_header("X-RateLimit-Remaining", "0")
            .with_header("X-RateLimit-Reset", "1700000000")
            .with_body(r#"{"message": "API rate limit exceeded"}"#)
            .create_async()
            .await;

        let err = fetch_notifications(&test_client(), "ghp_test", &server.url(), false)
            .await
            .expect_err("should fail with rate limit error");

        assert!(
            matches!(err, AppError::RateLimit { .. }),
            "expected AppError::RateLimit, got {err:?}"
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn fetch_notifications_maps_403_without_rate_limit_to_github_error() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/notifications?all=false&per_page=100")
            .with_status(403)
            .with_body(r#"{"message": "Forbidden"}"#)
            .create_async()
            .await;

        let err = fetch_notifications(&test_client(), "ghp_test", &server.url(), false)
            .await
            .expect_err("should fail with github error");

        assert!(
            matches!(err, AppError::GitHub(_)),
            "expected AppError::GitHub, got {err:?}"
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn fetch_notifications_403_with_remaining_zero_but_no_reset_falls_back_to_github_error() {
        // Defensive: if X-RateLimit-Reset is absent, we must NOT emit a
        // rate-limit error with an epoch-0 timestamp — fall through to
        // a plain `forbidden` error instead.
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/notifications?all=false&per_page=100")
            .with_status(403)
            .with_header("X-RateLimit-Remaining", "0")
            // deliberately no X-RateLimit-Reset header
            .with_body(r#"{"message": "Forbidden"}"#)
            .create_async()
            .await;

        let err = fetch_notifications(&test_client(), "ghp_test", &server.url(), false)
            .await
            .expect_err("should fail");

        assert!(
            matches!(err, AppError::GitHub(_)),
            "expected AppError::GitHub (not RateLimit), got {err:?}"
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn fetch_notifications_retries_on_network_error() {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(100))
            .user_agent("prism-test")
            .build()
            .expect("failed to build test client");

        let start = std::time::Instant::now();
        let err = fetch_notifications(&client, "ghp_test", "http://127.0.0.1:1", false)
            .await
            .expect_err("should fail after retries");
        let elapsed = start.elapsed();

        assert!(
            matches!(err, AppError::GitHub(_)),
            "expected AppError::GitHub, got {err:?}"
        );
        assert!(
            err.to_string().contains("retries"),
            "expected 'retries' in {err}"
        );
        assert!(
            elapsed >= Duration::from_millis(500),
            "expected retries with backoff, elapsed only {elapsed:?}"
        );
    }

    #[tokio::test]
    async fn fetch_notifications_maps_malformed_json_to_github_error() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/notifications?all=false&per_page=100")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("not valid json")
            .create_async()
            .await;

        let err = fetch_notifications(&test_client(), "ghp_test", &server.url(), false)
            .await
            .expect_err("should fail to parse");

        assert!(
            matches!(err, AppError::GitHub(_)),
            "expected AppError::GitHub, got {err:?}"
        );
        mock.assert_async().await;
    }
}
