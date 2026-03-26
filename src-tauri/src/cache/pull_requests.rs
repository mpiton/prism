use sqlx::SqlitePool;

use crate::error::AppError;
use crate::types::{CiStatus, PrState, Priority, PullRequest};

/// Row representation matching the `pull_requests` table columns.
/// Enums stored as TEXT, labels as JSON TEXT, number as INTEGER.
#[derive(sqlx::FromRow)]
pub(crate) struct PullRequestRow {
    pub(crate) id: String,
    pub(crate) number: i64,
    pub(crate) title: String,
    pub(crate) author: String,
    pub(crate) state: String,
    pub(crate) ci_status: String,
    pub(crate) priority: String,
    pub(crate) repo_id: String,
    pub(crate) url: String,
    pub(crate) labels: String,
    pub(crate) additions: i64,
    pub(crate) deletions: i64,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

fn pr_state_to_str(s: &PrState) -> &'static str {
    match s {
        PrState::Open => "open",
        PrState::Closed => "closed",
        PrState::Merged => "merged",
        PrState::Draft => "draft",
    }
}

fn pr_state_from_str(s: &str) -> Result<PrState, AppError> {
    match s {
        "open" => Ok(PrState::Open),
        "closed" => Ok(PrState::Closed),
        "merged" => Ok(PrState::Merged),
        "draft" => Ok(PrState::Draft),
        _ => Err(AppError::Config(format!("unknown PrState: {s}"))),
    }
}

fn ci_status_to_str(s: &CiStatus) -> &'static str {
    match s {
        CiStatus::Pending => "pending",
        CiStatus::Running => "running",
        CiStatus::Success => "success",
        CiStatus::Failure => "failure",
        CiStatus::Cancelled => "cancelled",
    }
}

fn ci_status_from_str(s: &str) -> Result<CiStatus, AppError> {
    match s {
        "pending" => Ok(CiStatus::Pending),
        "running" => Ok(CiStatus::Running),
        "success" => Ok(CiStatus::Success),
        "failure" => Ok(CiStatus::Failure),
        "cancelled" => Ok(CiStatus::Cancelled),
        _ => Err(AppError::Config(format!("unknown CiStatus: {s}"))),
    }
}

fn priority_to_str(p: &Priority) -> &'static str {
    match p {
        Priority::Low => "low",
        Priority::Medium => "medium",
        Priority::High => "high",
        Priority::Critical => "critical",
    }
}

fn priority_from_str(s: &str) -> Result<Priority, AppError> {
    match s {
        "low" => Ok(Priority::Low),
        "medium" => Ok(Priority::Medium),
        "high" => Ok(Priority::High),
        "critical" => Ok(Priority::Critical),
        _ => Err(AppError::Config(format!("unknown Priority: {s}"))),
    }
}

impl TryFrom<PullRequestRow> for PullRequest {
    type Error = AppError;

    fn try_from(row: PullRequestRow) -> Result<Self, Self::Error> {
        let labels: Vec<String> = serde_json::from_str(&row.labels)
            .map_err(|e| AppError::Config(format!("labels for PR '{}': {e}", row.id)))?;
        let number = u32::try_from(row.number)
            .map_err(|_| AppError::Config(format!("invalid PR number: {}", row.number)))?;
        let additions = u32::try_from(row.additions).map_err(|_| {
            AppError::Config(format!(
                "invalid additions for PR '{}': {}",
                row.id, row.additions
            ))
        })?;
        let deletions = u32::try_from(row.deletions).map_err(|_| {
            AppError::Config(format!(
                "invalid deletions for PR '{}': {}",
                row.id, row.deletions
            ))
        })?;

        Ok(Self {
            id: row.id,
            number,
            title: row.title,
            author: row.author,
            state: pr_state_from_str(&row.state)?,
            ci_status: ci_status_from_str(&row.ci_status)?,
            priority: priority_from_str(&row.priority)?,
            repo_id: row.repo_id,
            url: row.url,
            labels,
            additions,
            deletions,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

/// Explicit column list for all SELECT queries.
pub(crate) const PR_COLS: &str = "id, number, title, author, state, ci_status, priority, repo_id, url, labels, additions, deletions, created_at, updated_at";

/// Insert or update a pull request. On conflict (same `id`), updates all fields.
/// Uses `RETURNING` for an atomic read-after-write.
#[allow(dead_code)]
pub async fn upsert_pull_request(
    pool: &SqlitePool,
    pr: &PullRequest,
) -> Result<PullRequest, AppError> {
    let labels_json =
        serde_json::to_string(&pr.labels).map_err(|e| AppError::Config(e.to_string()))?;

    let sql = format!(
        "INSERT INTO pull_requests (id, number, title, author, state, ci_status, priority, repo_id, url, labels, additions, deletions, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
         ON CONFLICT(id) DO UPDATE SET
             number = excluded.number,
             title = excluded.title,
             author = excluded.author,
             state = excluded.state,
             ci_status = excluded.ci_status,
             priority = excluded.priority,
             url = excluded.url,
             labels = excluded.labels,
             additions = excluded.additions,
             deletions = excluded.deletions,
             updated_at = excluded.updated_at
         RETURNING {PR_COLS}"
    );

    let row: PullRequestRow = sqlx::query_as(&sql)
        .bind(&pr.id)
        .bind(i64::from(pr.number))
        .bind(&pr.title)
        .bind(&pr.author)
        .bind(pr_state_to_str(&pr.state))
        .bind(ci_status_to_str(&pr.ci_status))
        .bind(priority_to_str(&pr.priority))
        .bind(&pr.repo_id)
        .bind(&pr.url)
        .bind(&labels_json)
        .bind(i64::from(pr.additions))
        .bind(i64::from(pr.deletions))
        .bind(&pr.created_at)
        .bind(&pr.updated_at)
        .fetch_one(pool)
        .await?;

    PullRequest::try_from(row)
}

/// Return all pull requests for a given repo, ordered by `updated_at DESC`.
#[allow(dead_code)]
pub async fn get_pull_requests_by_repo(
    pool: &SqlitePool,
    repo_id: &str,
) -> Result<Vec<PullRequest>, AppError> {
    let sql =
        format!("SELECT {PR_COLS} FROM pull_requests WHERE repo_id = $1 ORDER BY updated_at DESC");
    let rows: Vec<PullRequestRow> = sqlx::query_as(&sql).bind(repo_id).fetch_all(pool).await?;

    rows.into_iter().map(PullRequest::try_from).collect()
}

/// Return a single pull request by ID, or `AppError::NotFound`.
#[allow(dead_code)]
pub async fn get_pull_request(pool: &SqlitePool, id: &str) -> Result<PullRequest, AppError> {
    let sql = format!("SELECT {PR_COLS} FROM pull_requests WHERE id = $1");
    let row: Option<PullRequestRow> = sqlx::query_as(&sql).bind(id).fetch_optional(pool).await?;

    row.map(PullRequest::try_from)
        .transpose()?
        .ok_or_else(|| AppError::NotFound(format!("pull_request '{id}'")))
}

/// Delete pull requests for a repo whose IDs are not in `current_ids`.
/// Returns the number of deleted rows.
///
/// When `current_ids` is empty, returns `Ok(0)` unless `allow_full_delete`
/// is `true`, in which case all PRs for the repo are deleted.
/// Large `current_ids` lists are chunked to stay within `SQLite`'s parameter
/// limit (999).
#[allow(dead_code)]
pub async fn delete_stale_prs(
    pool: &SqlitePool,
    repo_id: &str,
    current_ids: &[String],
    allow_full_delete: bool,
) -> Result<u64, AppError> {
    // SQLite SQLITE_MAX_VARIABLE_NUMBER=999 minus $1 for repo_id
    const CHUNK_SIZE: usize = 998;

    if current_ids.is_empty() {
        if !allow_full_delete {
            return Ok(0);
        }
        let result = sqlx::query("DELETE FROM pull_requests WHERE repo_id = $1")
            .bind(repo_id)
            .execute(pool)
            .await?;
        return Ok(result.rows_affected());
    }
    let mut total_deleted: u64 = 0;

    for chunk in current_ids.chunks(CHUNK_SIZE) {
        let placeholders: Vec<String> = (2..=chunk.len() + 1).map(|i| format!("${i}")).collect();
        let sql = format!(
            "DELETE FROM pull_requests WHERE repo_id = $1 AND id NOT IN ({})",
            placeholders.join(", ")
        );

        let mut query = sqlx::query(&sql).bind(repo_id);
        for id in chunk {
            query = query.bind(id);
        }

        let result = query.execute(pool).await?;
        total_deleted += result.rows_affected();
    }

    Ok(total_deleted)
}

/// Sort key for ordering pull requests by priority.
/// Higher value = higher priority. Dominant factor is the priority enum,
/// with state and CI status as tiebreakers.
#[allow(dead_code)]
pub fn priority_sort_weight(pr: &PullRequest) -> i64 {
    let priority_weight = match pr.priority {
        Priority::Critical => 400,
        Priority::High => 300,
        Priority::Medium => 200,
        Priority::Low => 100,
    };

    let state_weight = match pr.state {
        PrState::Open => 50,
        PrState::Draft => 25,
        PrState::Closed | PrState::Merged => 0,
    };

    let ci_weight = match pr.ci_status {
        CiStatus::Failure => 30,
        CiStatus::Pending | CiStatus::Running => 10,
        CiStatus::Success | CiStatus::Cancelled => 0,
    };

    priority_weight + state_weight + ci_weight
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::db::init_db;
    use crate::cache::repos::upsert_repo;
    use crate::types::Repo;

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

    fn sample_pr(id: &str, number: u32, title: &str) -> PullRequest {
        PullRequest {
            id: id.to_string(),
            number,
            title: title.to_string(),
            author: "alice".to_string(),
            state: PrState::Open,
            ci_status: CiStatus::Success,
            priority: Priority::Medium,
            repo_id: "repo-1".to_string(),
            url: format!("https://github.com/mpiton/prism/pull/{number}"),
            labels: vec!["bug".to_string(), "urgent".to_string()],
            additions: 50,
            deletions: 10,
            created_at: "2026-03-01T10:00:00Z".to_string(),
            updated_at: "2026-03-20T15:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn test_upsert_pr_insert() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let pr = sample_pr("pr-1", 42, "Fix login bug");
        let result = upsert_pull_request(&pool, &pr).await.unwrap();

        assert_eq!(result.id, "pr-1");
        assert_eq!(result.number, 42);
        assert_eq!(result.title, "Fix login bug");
        assert_eq!(result.author, "alice");
        assert_eq!(result.state, PrState::Open);
        assert_eq!(result.ci_status, CiStatus::Success);
        assert_eq!(result.priority, Priority::Medium);
        assert_eq!(result.repo_id, "repo-1");
        assert_eq!(result.labels, vec!["bug", "urgent"]);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_upsert_pr_update() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let pr = sample_pr("pr-1", 42, "Fix login bug");
        upsert_pull_request(&pool, &pr).await.unwrap();

        let mut updated = pr.clone();
        updated.title = "Fix login bug (v2)".to_string();
        updated.state = PrState::Merged;
        updated.ci_status = CiStatus::Success;
        updated.labels = vec!["bug".to_string()];
        updated.created_at = "2099-01-01T00:00:00Z".to_string(); // should be ignored on update

        let result = upsert_pull_request(&pool, &updated).await.unwrap();

        assert_eq!(result.title, "Fix login bug (v2)");
        assert_eq!(result.state, PrState::Merged);
        assert_eq!(result.labels, vec!["bug"]);
        assert_eq!(
            result.created_at, "2026-03-01T10:00:00Z",
            "created_at should be preserved from original insert"
        );

        pool.close().await;
    }

    #[tokio::test]
    async fn test_get_prs_by_repo() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let pr1 = sample_pr("pr-1", 1, "First PR");
        let mut pr2 = sample_pr("pr-2", 2, "Second PR");
        pr2.updated_at = "2026-03-25T10:00:00Z".to_string();

        upsert_pull_request(&pool, &pr1).await.unwrap();
        upsert_pull_request(&pool, &pr2).await.unwrap();

        let prs = get_pull_requests_by_repo(&pool, "repo-1").await.unwrap();

        assert_eq!(prs.len(), 2);
        // Ordered by updated_at DESC — pr2 is newer
        assert_eq!(prs[0].id, "pr-2");
        assert_eq!(prs[1].id, "pr-1");

        pool.close().await;
    }

    #[tokio::test]
    async fn test_get_prs_by_repo_empty() {
        let (pool, _tmp) = test_pool().await;

        let prs = get_pull_requests_by_repo(&pool, "nonexistent")
            .await
            .unwrap();

        assert!(prs.is_empty());

        pool.close().await;
    }

    #[tokio::test]
    async fn test_labels_json_roundtrip() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let mut pr = sample_pr("pr-1", 1, "Labels test");
        pr.labels = vec![
            "enhancement".to_string(),
            "good first issue".to_string(),
            "help wanted".to_string(),
        ];

        upsert_pull_request(&pool, &pr).await.unwrap();

        let result = get_pull_request(&pool, "pr-1").await.unwrap();

        assert_eq!(
            result.labels,
            vec!["enhancement", "good first issue", "help wanted"]
        );

        pool.close().await;
    }

    #[tokio::test]
    async fn test_priority_score_computation() {
        let critical_open_failing = PullRequest {
            priority: Priority::Critical,
            state: PrState::Open,
            ci_status: CiStatus::Failure,
            ..sample_pr("pr-1", 1, "Critical")
        };

        let low_merged_success = PullRequest {
            priority: Priority::Low,
            state: PrState::Merged,
            ci_status: CiStatus::Success,
            ..sample_pr("pr-2", 2, "Low")
        };

        let high_open_success = PullRequest {
            priority: Priority::High,
            state: PrState::Open,
            ci_status: CiStatus::Success,
            ..sample_pr("pr-3", 3, "High")
        };

        let score_critical = priority_sort_weight(&critical_open_failing);
        let score_low = priority_sort_weight(&low_merged_success);
        let score_high = priority_sort_weight(&high_open_success);

        assert!(
            score_critical > score_high,
            "critical+open+failure ({score_critical}) > high+open+success ({score_high})"
        );
        assert!(
            score_high > score_low,
            "high+open+success ({score_high}) > low+merged+success ({score_low})"
        );
        assert!(score_critical > 0);
        assert!(score_low > 0);
    }

    #[tokio::test]
    async fn test_delete_stale_prs() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let pr1 = sample_pr("pr-1", 1, "Keep me");
        let pr2 = sample_pr("pr-2", 2, "Delete me");
        let pr3 = sample_pr("pr-3", 3, "Keep me too");

        upsert_pull_request(&pool, &pr1).await.unwrap();
        upsert_pull_request(&pool, &pr2).await.unwrap();
        upsert_pull_request(&pool, &pr3).await.unwrap();

        let current_ids = vec!["pr-1".to_string(), "pr-3".to_string()];
        let deleted = delete_stale_prs(&pool, "repo-1", &current_ids, false)
            .await
            .unwrap();

        assert_eq!(deleted, 1, "should delete 1 stale PR");

        let remaining = get_pull_requests_by_repo(&pool, "repo-1").await.unwrap();
        assert_eq!(remaining.len(), 2);

        let ids: Vec<&str> = remaining.iter().map(|p| p.id.as_str()).collect();
        assert!(ids.contains(&"pr-1"));
        assert!(ids.contains(&"pr-3"));
        assert!(!ids.contains(&"pr-2"));

        pool.close().await;
    }

    #[tokio::test]
    async fn test_delete_stale_prs_empty_ids_without_flag() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();
        upsert_pull_request(&pool, &sample_pr("pr-1", 1, "PR"))
            .await
            .unwrap();

        // Empty current_ids without allow_full_delete → no-op
        let deleted = delete_stale_prs(&pool, "repo-1", &[], false).await.unwrap();
        assert_eq!(deleted, 0);

        let remaining = get_pull_requests_by_repo(&pool, "repo-1").await.unwrap();
        assert_eq!(remaining.len(), 1, "PR should still exist");

        // Empty current_ids with allow_full_delete → wipes all
        let deleted = delete_stale_prs(&pool, "repo-1", &[], true).await.unwrap();
        assert_eq!(deleted, 1);

        let remaining = get_pull_requests_by_repo(&pool, "repo-1").await.unwrap();
        assert!(remaining.is_empty(), "all PRs should be deleted");

        pool.close().await;
    }

    #[test]
    fn test_unknown_enum_values_return_error() {
        assert!(pr_state_from_str("OPEN").is_err(), "wrong case should fail");
        assert!(pr_state_from_str("bogus").is_err());
        assert!(ci_status_from_str("unknown").is_err());
        assert!(ci_status_from_str("").is_err());
        assert!(priority_from_str("").is_err());
        assert!(
            priority_from_str("CRITICAL").is_err(),
            "wrong case should fail"
        );
    }
}
