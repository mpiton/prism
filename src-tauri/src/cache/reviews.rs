use std::collections::HashSet;

use sqlx::SqlitePool;

use crate::error::AppError;
use crate::types::{Review, ReviewRequest, ReviewStatus, ReviewSummary};

/// Row representation matching the `review_requests` table columns.
#[derive(sqlx::FromRow)]
struct ReviewRequestRow {
    id: String,
    pull_request_id: String,
    reviewer: String,
    status: String,
    requested_at: String,
}

/// Row representation matching the `reviews` table columns.
#[derive(sqlx::FromRow)]
struct ReviewRow {
    id: String,
    pull_request_id: String,
    reviewer: String,
    status: String,
    body: Option<String>,
    submitted_at: String,
}

fn review_status_to_str(s: &ReviewStatus) -> &'static str {
    match s {
        ReviewStatus::Pending => "pending",
        ReviewStatus::Approved => "approved",
        ReviewStatus::ChangesRequested => "changes_requested",
        ReviewStatus::Commented => "commented",
        ReviewStatus::Dismissed => "dismissed",
    }
}

fn review_status_from_str(s: &str) -> Result<ReviewStatus, AppError> {
    match s {
        "pending" => Ok(ReviewStatus::Pending),
        "approved" => Ok(ReviewStatus::Approved),
        "changes_requested" => Ok(ReviewStatus::ChangesRequested),
        "commented" => Ok(ReviewStatus::Commented),
        "dismissed" => Ok(ReviewStatus::Dismissed),
        _ => Err(AppError::Config(format!("unknown ReviewStatus: {s}"))),
    }
}

impl TryFrom<ReviewRequestRow> for ReviewRequest {
    type Error = AppError;

    fn try_from(row: ReviewRequestRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            pull_request_id: row.pull_request_id,
            reviewer: row.reviewer,
            status: review_status_from_str(&row.status)?,
            requested_at: row.requested_at,
        })
    }
}

impl TryFrom<ReviewRow> for Review {
    type Error = AppError;

    fn try_from(row: ReviewRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            pull_request_id: row.pull_request_id,
            reviewer: row.reviewer,
            status: review_status_from_str(&row.status)?,
            body: row.body,
            submitted_at: row.submitted_at,
        })
    }
}

/// Explicit column list for `review_requests` SELECT queries.
const RR_COLS: &str = "id, pull_request_id, reviewer, status, requested_at";

/// Explicit column list for reviews SELECT queries.
const REV_COLS: &str = "id, pull_request_id, reviewer, status, body, submitted_at";

// ── review_requests CRUD ──────────────────────────────────────────

/// Insert or update a review request. On conflict (same `id`), updates all fields.
/// Uses `RETURNING` for an atomic read-after-write.
#[allow(dead_code)]
pub async fn upsert_review_request(
    pool: &SqlitePool,
    rr: &ReviewRequest,
) -> Result<ReviewRequest, AppError> {
    let sql = format!(
        "INSERT INTO review_requests (id, pull_request_id, reviewer, status, requested_at)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT(id) DO UPDATE SET
             reviewer = excluded.reviewer,
             status = excluded.status,
             requested_at = excluded.requested_at
         RETURNING {RR_COLS}"
    );

    let row: ReviewRequestRow = sqlx::query_as(&sql)
        .bind(&rr.id)
        .bind(&rr.pull_request_id)
        .bind(&rr.reviewer)
        .bind(review_status_to_str(&rr.status))
        .bind(&rr.requested_at)
        .fetch_one(pool)
        .await?;

    ReviewRequest::try_from(row)
}

/// Return all review requests for a given PR, ordered by `requested_at ASC`.
#[allow(dead_code)]
pub async fn get_review_requests_by_pr(
    pool: &SqlitePool,
    pull_request_id: &str,
) -> Result<Vec<ReviewRequest>, AppError> {
    let sql = format!(
        "SELECT {RR_COLS} FROM review_requests WHERE pull_request_id = $1 ORDER BY requested_at ASC"
    );
    let rows: Vec<ReviewRequestRow> = sqlx::query_as(&sql)
        .bind(pull_request_id)
        .fetch_all(pool)
        .await?;

    rows.into_iter().map(ReviewRequest::try_from).collect()
}

/// Return all review requests assigned to a specific reviewer, ordered by `requested_at DESC`.
#[allow(dead_code)]
pub async fn get_review_requests_for_user(
    pool: &SqlitePool,
    reviewer: &str,
) -> Result<Vec<ReviewRequest>, AppError> {
    let sql = format!(
        "SELECT {RR_COLS} FROM review_requests WHERE reviewer = $1 ORDER BY requested_at DESC"
    );
    let rows: Vec<ReviewRequestRow> = sqlx::query_as(&sql).bind(reviewer).fetch_all(pool).await?;

    rows.into_iter().map(ReviewRequest::try_from).collect()
}

/// Compute an aggregated review summary for a pull request from its review requests.
///
/// Counts approved, pending, and `changes_requested` review request statuses.
/// `total_reviews` is the total number of review requests (including commented/dismissed).
/// Reviewers are deduplicated (a reviewer re-requested after dismissal counts once).
#[allow(dead_code)]
pub async fn compute_review_summary(
    pool: &SqlitePool,
    pull_request_id: &str,
) -> Result<ReviewSummary, AppError> {
    let requests = get_review_requests_by_pr(pool, pull_request_id).await?;

    let mut approved: u32 = 0;
    let mut changes_requested: u32 = 0;
    let mut pending: u32 = 0;
    let mut seen = HashSet::new();

    for rr in &requests {
        match rr.status {
            ReviewStatus::Approved => approved += 1,
            ReviewStatus::ChangesRequested => changes_requested += 1,
            ReviewStatus::Pending => pending += 1,
            ReviewStatus::Commented | ReviewStatus::Dismissed => {}
        }
        seen.insert(rr.reviewer.clone());
    }

    let total_reviews = u32::try_from(requests.len())
        .map_err(|_| AppError::Config(format!("too many reviews: {}", requests.len())))?;

    Ok(ReviewSummary {
        total_reviews,
        approved,
        changes_requested,
        pending,
        reviewers: seen.into_iter().collect(),
    })
}

/// Delete all review requests for a given PR.
///
/// Used before re-inserting the current set from GitHub to evict
/// stale requests (e.g. a reviewer was un-requested).
#[allow(dead_code)]
pub async fn delete_review_requests_for_pr(
    pool: &SqlitePool,
    pull_request_id: &str,
) -> Result<u64, AppError> {
    let result = sqlx::query("DELETE FROM review_requests WHERE pull_request_id = $1")
        .bind(pull_request_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

// ── reviews CRUD ──────────────────────────────────────────────────

/// Insert or update a review. On conflict (same `id`), updates all fields.
/// Uses `RETURNING` for an atomic read-after-write.
///
/// Accepts any sqlx executor (pool, connection, or transaction).
#[allow(dead_code)]
pub async fn upsert_review<'e, E>(executor: E, review: &Review) -> Result<Review, AppError>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let sql = format!(
        "INSERT INTO reviews (id, pull_request_id, reviewer, status, body, submitted_at)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT(id) DO UPDATE SET
             reviewer = excluded.reviewer,
             status = excluded.status,
             body = excluded.body,
             submitted_at = excluded.submitted_at
         RETURNING {REV_COLS}"
    );

    let row: ReviewRow = sqlx::query_as(&sql)
        .bind(&review.id)
        .bind(&review.pull_request_id)
        .bind(&review.reviewer)
        .bind(review_status_to_str(&review.status))
        .bind(&review.body)
        .bind(&review.submitted_at)
        .fetch_one(executor)
        .await?;

    Review::try_from(row)
}

/// Return all reviews for a given PR, ordered by `submitted_at ASC`.
#[allow(dead_code)]
pub async fn get_reviews_by_pr(
    pool: &SqlitePool,
    pull_request_id: &str,
) -> Result<Vec<Review>, AppError> {
    let sql = format!(
        "SELECT {REV_COLS} FROM reviews WHERE pull_request_id = $1 ORDER BY submitted_at ASC"
    );
    let rows: Vec<ReviewRow> = sqlx::query_as(&sql)
        .bind(pull_request_id)
        .fetch_all(pool)
        .await?;

    rows.into_iter().map(Review::try_from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::db::init_db;
    use crate::cache::pull_requests::upsert_pull_request;
    use crate::cache::repos::upsert_repo;
    use crate::types::{CiStatus, PrState, Priority, PullRequest, Repo};

    async fn test_pool() -> (SqlitePool, tempfile::TempDir) {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = init_db(&tmp.path().join("test.db")).await.unwrap();
        (pool, tmp)
    }

    fn sample_repo() -> Repo {
        Repo {
            id: "repo-1".to_string(),
            org: "mpiton".to_string(),
            name: "prism".to_string(),
            full_name: "mpiton/prism".to_string(),
            url: "https://github.com/mpiton/prism".to_string(),
            default_branch: "main".to_string(),
            is_archived: false,
            enabled: true,
            local_path: None,
            last_sync_at: None,
        }
    }

    fn sample_pr() -> PullRequest {
        PullRequest {
            id: "pr-1".to_string(),
            number: 42,
            title: "Add feature".to_string(),
            author: "alice".to_string(),
            state: PrState::Open,
            ci_status: CiStatus::Success,
            priority: Priority::High,
            repo_id: "repo-1".to_string(),
            url: "https://github.com/mpiton/prism/pull/42".to_string(),
            labels: vec![],
            additions: 50,
            deletions: 10,
            created_at: "2026-03-01T10:00:00Z".to_string(),
            updated_at: "2026-03-20T15:00:00Z".to_string(),
        }
    }

    fn sample_review_request(id: &str, reviewer: &str, status: ReviewStatus) -> ReviewRequest {
        ReviewRequest {
            id: id.to_string(),
            pull_request_id: "pr-1".to_string(),
            reviewer: reviewer.to_string(),
            status,
            requested_at: "2026-03-20T10:00:00Z".to_string(),
        }
    }

    fn sample_review(id: &str, reviewer: &str, status: ReviewStatus) -> Review {
        Review {
            id: id.to_string(),
            pull_request_id: "pr-1".to_string(),
            reviewer: reviewer.to_string(),
            status,
            body: Some("LGTM".to_string()),
            submitted_at: "2026-03-20T12:00:00Z".to_string(),
        }
    }

    /// Seed the DB with a repo and PR for FK constraints.
    async fn seed(pool: &SqlitePool) {
        upsert_repo(pool, &sample_repo()).await.unwrap();
        upsert_pull_request(pool, &sample_pr()).await.unwrap();
    }

    #[tokio::test]
    async fn test_upsert_review_request() {
        let (pool, _tmp) = test_pool().await;
        seed(&pool).await;

        let rr = sample_review_request("rr-1", "bob", ReviewStatus::Pending);
        let result = upsert_review_request(&pool, &rr).await.unwrap();

        assert_eq!(result.id, "rr-1");
        assert_eq!(result.pull_request_id, "pr-1");
        assert_eq!(result.reviewer, "bob");
        assert_eq!(result.status, ReviewStatus::Pending);
        assert_eq!(result.requested_at, "2026-03-20T10:00:00Z");

        pool.close().await;
    }

    #[tokio::test]
    async fn test_upsert_review_request_duplicate() {
        let (pool, _tmp) = test_pool().await;
        seed(&pool).await;

        let rr = sample_review_request("rr-1", "bob", ReviewStatus::Pending);
        upsert_review_request(&pool, &rr).await.unwrap();

        // Update same ID with new status
        let updated = ReviewRequest {
            status: ReviewStatus::Approved,
            ..rr
        };
        let result = upsert_review_request(&pool, &updated).await.unwrap();

        assert_eq!(result.id, "rr-1");
        assert_eq!(result.status, ReviewStatus::Approved);

        // Should still be only one row
        let all = get_review_requests_by_pr(&pool, "pr-1").await.unwrap();
        assert_eq!(all.len(), 1);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_get_review_requests_for_user() {
        let (pool, _tmp) = test_pool().await;
        seed(&pool).await;

        // Create a second PR for variety
        let pr2 = PullRequest {
            id: "pr-2".to_string(),
            number: 43,
            title: "Another PR".to_string(),
            ..sample_pr()
        };
        upsert_pull_request(&pool, &pr2).await.unwrap();

        let rr1 = sample_review_request("rr-1", "bob", ReviewStatus::Pending);
        let rr2 = ReviewRequest {
            id: "rr-2".to_string(),
            pull_request_id: "pr-2".to_string(),
            reviewer: "bob".to_string(),
            status: ReviewStatus::Approved,
            requested_at: "2026-03-21T10:00:00Z".to_string(),
        };
        let rr3 = sample_review_request("rr-3", "alice", ReviewStatus::Pending);

        upsert_review_request(&pool, &rr1).await.unwrap();
        upsert_review_request(&pool, &rr2).await.unwrap();
        upsert_review_request(&pool, &rr3).await.unwrap();

        let bob_reviews = get_review_requests_for_user(&pool, "bob").await.unwrap();
        assert_eq!(bob_reviews.len(), 2);
        // Ordered by requested_at DESC — rr2 is newer
        assert_eq!(bob_reviews[0].id, "rr-2");
        assert_eq!(bob_reviews[1].id, "rr-1");

        let alice_reviews = get_review_requests_for_user(&pool, "alice").await.unwrap();
        assert_eq!(alice_reviews.len(), 1);
        assert_eq!(alice_reviews[0].id, "rr-3");

        let nobody = get_review_requests_for_user(&pool, "nobody").await.unwrap();
        assert!(nobody.is_empty());

        pool.close().await;
    }

    #[tokio::test]
    async fn test_compute_review_summary_all_approved() {
        let (pool, _tmp) = test_pool().await;
        seed(&pool).await;

        let rr1 = sample_review_request("rr-1", "bob", ReviewStatus::Approved);
        let rr2 = sample_review_request("rr-2", "charlie", ReviewStatus::Approved);

        upsert_review_request(&pool, &rr1).await.unwrap();
        upsert_review_request(&pool, &rr2).await.unwrap();

        let summary = compute_review_summary(&pool, "pr-1").await.unwrap();

        assert_eq!(summary.total_reviews, 2);
        assert_eq!(summary.approved, 2);
        assert_eq!(summary.changes_requested, 0);
        assert_eq!(summary.pending, 0);
        assert_eq!(summary.reviewers.len(), 2);
        assert!(summary.reviewers.contains(&"bob".to_string()));
        assert!(summary.reviewers.contains(&"charlie".to_string()));

        pool.close().await;
    }

    #[tokio::test]
    async fn test_compute_review_summary_mixed() {
        let (pool, _tmp) = test_pool().await;
        seed(&pool).await;

        let rr1 = sample_review_request("rr-1", "bob", ReviewStatus::Approved);
        let rr2 = sample_review_request("rr-2", "charlie", ReviewStatus::ChangesRequested);
        let rr3 = sample_review_request("rr-3", "diana", ReviewStatus::Pending);
        let rr4 = sample_review_request("rr-4", "eve", ReviewStatus::Commented);

        upsert_review_request(&pool, &rr1).await.unwrap();
        upsert_review_request(&pool, &rr2).await.unwrap();
        upsert_review_request(&pool, &rr3).await.unwrap();
        upsert_review_request(&pool, &rr4).await.unwrap();

        let summary = compute_review_summary(&pool, "pr-1").await.unwrap();

        assert_eq!(summary.total_reviews, 4);
        assert_eq!(summary.approved, 1);
        assert_eq!(summary.changes_requested, 1);
        assert_eq!(summary.pending, 1);
        assert_eq!(summary.reviewers.len(), 4);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_compute_review_summary_empty() {
        let (pool, _tmp) = test_pool().await;
        seed(&pool).await;

        let summary = compute_review_summary(&pool, "pr-1").await.unwrap();

        assert_eq!(summary.total_reviews, 0);
        assert_eq!(summary.approved, 0);
        assert_eq!(summary.changes_requested, 0);
        assert_eq!(summary.pending, 0);
        assert!(summary.reviewers.is_empty());

        pool.close().await;
    }

    #[tokio::test]
    async fn test_upsert_review() {
        let (pool, _tmp) = test_pool().await;
        seed(&pool).await;

        let review = sample_review("rev-1", "bob", ReviewStatus::Approved);
        let result = upsert_review(&pool, &review).await.unwrap();

        assert_eq!(result.id, "rev-1");
        assert_eq!(result.pull_request_id, "pr-1");
        assert_eq!(result.reviewer, "bob");
        assert_eq!(result.status, ReviewStatus::Approved);
        assert_eq!(result.body, Some("LGTM".to_string()));
        assert_eq!(result.submitted_at, "2026-03-20T12:00:00Z");

        // Update: change status and body
        let updated = Review {
            status: ReviewStatus::ChangesRequested,
            body: Some("Needs fixes".to_string()),
            ..review
        };
        let result2 = upsert_review(&pool, &updated).await.unwrap();
        assert_eq!(result2.status, ReviewStatus::ChangesRequested);
        assert_eq!(result2.body, Some("Needs fixes".to_string()));

        // Null body
        let no_body = Review {
            id: "rev-2".to_string(),
            body: None,
            ..sample_review("rev-2", "charlie", ReviewStatus::Commented)
        };
        let result3 = upsert_review(&pool, &no_body).await.unwrap();
        assert_eq!(result3.body, None);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_get_reviews_by_pr() {
        let (pool, _tmp) = test_pool().await;
        seed(&pool).await;

        let rev1 = Review {
            submitted_at: "2026-03-20T10:00:00Z".to_string(),
            ..sample_review("rev-1", "bob", ReviewStatus::Approved)
        };
        let rev2 = Review {
            submitted_at: "2026-03-20T14:00:00Z".to_string(),
            ..sample_review("rev-2", "charlie", ReviewStatus::ChangesRequested)
        };

        upsert_review(&pool, &rev1).await.unwrap();
        upsert_review(&pool, &rev2).await.unwrap();

        let reviews = get_reviews_by_pr(&pool, "pr-1").await.unwrap();
        assert_eq!(reviews.len(), 2);
        // Ordered by submitted_at ASC
        assert_eq!(reviews[0].id, "rev-1");
        assert_eq!(reviews[1].id, "rev-2");

        // No reviews for another PR
        let empty = get_reviews_by_pr(&pool, "pr-nonexistent").await.unwrap();
        assert!(empty.is_empty());

        pool.close().await;
    }

    #[test]
    fn test_unknown_review_status_returns_error() {
        assert!(
            review_status_from_str("APPROVED").is_err(),
            "wrong case should fail"
        );
        assert!(review_status_from_str("bogus").is_err());
        assert!(review_status_from_str("").is_err());
    }
}
