use sqlx::SqlitePool;

use crate::error::AppError;
use crate::types::{Issue, IssueState, Priority};

/// Row representation matching the `issues` table columns.
/// Enums stored as TEXT, labels as JSON TEXT, number as INTEGER.
#[derive(sqlx::FromRow)]
struct IssueRow {
    id: String,
    number: i64,
    title: String,
    author: String,
    state: String,
    priority: String,
    repo_id: String,
    url: String,
    labels: String,
    created_at: String,
    updated_at: String,
}

fn issue_state_to_str(s: &IssueState) -> &'static str {
    match s {
        IssueState::Open => "open",
        IssueState::Closed => "closed",
    }
}

fn issue_state_from_str(s: &str) -> Result<IssueState, AppError> {
    match s {
        "open" => Ok(IssueState::Open),
        "closed" => Ok(IssueState::Closed),
        _ => Err(AppError::Config(format!("unknown IssueState: {s}"))),
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

impl TryFrom<IssueRow> for Issue {
    type Error = AppError;

    fn try_from(row: IssueRow) -> Result<Self, Self::Error> {
        let labels: Vec<String> = serde_json::from_str(&row.labels)
            .map_err(|e| AppError::Config(format!("labels for issue '{}': {e}", row.id)))?;

        Ok(Self {
            id: row.id,
            number: u32::try_from(row.number)
                .map_err(|_| AppError::Config(format!("invalid issue number: {}", row.number)))?,
            title: row.title,
            author: row.author,
            state: issue_state_from_str(&row.state)?,
            priority: priority_from_str(&row.priority)?,
            repo_id: row.repo_id,
            url: row.url,
            labels,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

/// Explicit column list for all SELECT queries on `issues`.
const ISSUE_COLS: &str =
    "id, number, title, author, state, priority, repo_id, url, labels, created_at, updated_at";

/// Insert or update an issue. On conflict (same `id`), updates all fields
/// except `created_at`. Uses `RETURNING` for an atomic read-after-write.
///
/// Accepts any sqlx executor (pool, connection, or transaction).
#[allow(dead_code)]
pub async fn upsert_issue<'e, E>(executor: E, issue: &Issue) -> Result<Issue, AppError>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let labels_json =
        serde_json::to_string(&issue.labels).map_err(|e| AppError::Config(e.to_string()))?;

    let sql = format!(
        "INSERT INTO issues (id, number, title, author, state, priority, repo_id, url, labels, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
         ON CONFLICT(id) DO UPDATE SET
             number = excluded.number,
             title = excluded.title,
             author = excluded.author,
             state = excluded.state,
             priority = excluded.priority,
             repo_id = excluded.repo_id,
             url = excluded.url,
             labels = excluded.labels,
             updated_at = excluded.updated_at
         RETURNING {ISSUE_COLS}"
    );

    let row: IssueRow = sqlx::query_as(&sql)
        .bind(&issue.id)
        .bind(i64::from(issue.number))
        .bind(&issue.title)
        .bind(&issue.author)
        .bind(issue_state_to_str(&issue.state))
        .bind(priority_to_str(&issue.priority))
        .bind(&issue.repo_id)
        .bind(&issue.url)
        .bind(&labels_json)
        .bind(&issue.created_at)
        .bind(&issue.updated_at)
        .fetch_one(executor)
        .await?;

    Issue::try_from(row)
}

/// Return all issues for a given repo, ordered by `updated_at DESC`.
#[allow(dead_code)]
pub async fn get_issues_by_repo(pool: &SqlitePool, repo_id: &str) -> Result<Vec<Issue>, AppError> {
    let sql =
        format!("SELECT {ISSUE_COLS} FROM issues WHERE repo_id = $1 ORDER BY updated_at DESC");
    let rows: Vec<IssueRow> = sqlx::query_as(&sql).bind(repo_id).fetch_all(pool).await?;

    rows.into_iter().map(Issue::try_from).collect()
}

/// Return all issues created by the given author, across all repos,
/// ordered by `updated_at DESC`.
#[allow(dead_code)]
pub async fn get_issues_for_author(
    pool: &SqlitePool,
    author: &str,
) -> Result<Vec<Issue>, AppError> {
    let sql = format!("SELECT {ISSUE_COLS} FROM issues WHERE author = $1 ORDER BY updated_at DESC");
    let rows: Vec<IssueRow> = sqlx::query_as(&sql).bind(author).fetch_all(pool).await?;

    rows.into_iter().map(Issue::try_from).collect()
}

/// Delete issues for a repo whose IDs are not in `current_ids`.
/// Returns the number of deleted rows.
///
/// When `current_ids` is empty, returns `Ok(0)` unless `allow_full_delete`
/// is `true`, in which case all issues for the repo are deleted.
///
/// Uses a fetch-then-delete strategy: first retrieves existing IDs for the
/// repo, computes the set difference in Rust, then deletes stale IDs in
/// chunks that respect `SQLite`'s parameter limit (999).
#[allow(dead_code)]
pub async fn delete_stale_issues(
    pool: &SqlitePool,
    repo_id: &str,
    current_ids: &[String],
    allow_full_delete: bool,
) -> Result<u64, AppError> {
    const CHUNK_SIZE: usize = 998;

    if current_ids.is_empty() {
        if !allow_full_delete {
            return Ok(0);
        }
        let result = sqlx::query("DELETE FROM issues WHERE repo_id = $1")
            .bind(repo_id)
            .execute(pool)
            .await?;
        return Ok(result.rows_affected());
    }

    // Fetch all existing IDs for this repo
    let existing: Vec<(String,)> = sqlx::query_as("SELECT id FROM issues WHERE repo_id = $1")
        .bind(repo_id)
        .fetch_all(pool)
        .await?;

    // Compute stale IDs (exist in DB but not in current_ids)
    let keep: std::collections::HashSet<&str> = current_ids.iter().map(String::as_str).collect();
    let stale_ids: Vec<&str> = existing
        .iter()
        .map(|(id,)| id.as_str())
        .filter(|id| !keep.contains(id))
        .collect();

    if stale_ids.is_empty() {
        return Ok(0);
    }

    let mut total_deleted: u64 = 0;

    for chunk in stale_ids.chunks(CHUNK_SIZE) {
        let placeholders: Vec<String> = (1..=chunk.len()).map(|i| format!("${i}")).collect();
        let sql = format!(
            "DELETE FROM issues WHERE id IN ({})",
            placeholders.join(", ")
        );

        let mut query = sqlx::query(&sql);
        for id in chunk {
            query = query.bind(id);
        }

        let result = query.execute(pool).await?;
        total_deleted += result.rows_affected();
    }

    Ok(total_deleted)
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

    fn sample_issue(id: &str, number: u32, title: &str) -> Issue {
        Issue {
            id: id.to_string(),
            number,
            title: title.to_string(),
            author: "alice".to_string(),
            state: IssueState::Open,
            priority: Priority::Medium,
            repo_id: "repo-1".to_string(),
            url: format!("https://github.com/mpiton/prism/issues/{number}"),
            labels: vec!["bug".to_string(), "urgent".to_string()],
            created_at: "2026-03-01T10:00:00Z".to_string(),
            updated_at: "2026-03-20T15:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn test_upsert_issue_insert_and_update() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let issue = sample_issue("issue-1", 10, "Login broken");
        let result = upsert_issue(&pool, &issue).await.unwrap();

        assert_eq!(result.id, "issue-1");
        assert_eq!(result.number, 10);
        assert_eq!(result.title, "Login broken");
        assert_eq!(result.state, IssueState::Open);
        assert_eq!(result.priority, Priority::Medium);
        assert_eq!(result.labels, vec!["bug", "urgent"]);

        // Update: change title, state, and created_at (should be preserved)
        let mut updated = issue.clone();
        updated.title = "Login broken (fixed)".to_string();
        updated.state = IssueState::Closed;
        updated.created_at = "2099-01-01T00:00:00Z".to_string();

        let result = upsert_issue(&pool, &updated).await.unwrap();

        assert_eq!(result.title, "Login broken (fixed)");
        assert_eq!(result.state, IssueState::Closed);
        assert_eq!(
            result.created_at, "2026-03-01T10:00:00Z",
            "created_at should be preserved from original insert"
        );

        pool.close().await;
    }

    #[tokio::test]
    async fn test_get_issues_by_repo() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let i1 = sample_issue("issue-1", 1, "First");
        let mut i2 = sample_issue("issue-2", 2, "Second");
        i2.updated_at = "2026-03-25T10:00:00Z".to_string();

        upsert_issue(&pool, &i1).await.unwrap();
        upsert_issue(&pool, &i2).await.unwrap();

        let issues = get_issues_by_repo(&pool, "repo-1").await.unwrap();

        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].id, "issue-2", "ordered by updated_at DESC");
        assert_eq!(issues[1].id, "issue-1");

        // Empty repo returns empty vec
        let empty = get_issues_by_repo(&pool, "nonexistent").await.unwrap();
        assert!(empty.is_empty());

        pool.close().await;
    }

    #[tokio::test]
    async fn test_get_issues_for_author() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let i1 = sample_issue("issue-1", 1, "Alice issue");
        let mut i2 = sample_issue("issue-2", 2, "Bob issue");
        i2.author = "bob".to_string();
        let mut i3 = sample_issue("issue-3", 3, "Alice issue 2");
        i3.updated_at = "2026-03-25T10:00:00Z".to_string();

        upsert_issue(&pool, &i1).await.unwrap();
        upsert_issue(&pool, &i2).await.unwrap();
        upsert_issue(&pool, &i3).await.unwrap();

        let alice_issues = get_issues_for_author(&pool, "alice").await.unwrap();
        assert_eq!(alice_issues.len(), 2);
        assert_eq!(alice_issues[0].id, "issue-3", "ordered by updated_at DESC");
        assert_eq!(alice_issues[1].id, "issue-1");

        let bob_issues = get_issues_for_author(&pool, "bob").await.unwrap();
        assert_eq!(bob_issues.len(), 1);
        assert_eq!(bob_issues[0].id, "issue-2");

        let nobody = get_issues_for_author(&pool, "nobody").await.unwrap();
        assert!(nobody.is_empty());

        pool.close().await;
    }

    #[tokio::test]
    async fn test_issue_labels_json_roundtrip() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let mut issue = sample_issue("issue-1", 1, "Labels test");
        issue.labels = vec![
            "enhancement".to_string(),
            "good first issue".to_string(),
            "help wanted".to_string(),
        ];

        upsert_issue(&pool, &issue).await.unwrap();

        let issues = get_issues_by_repo(&pool, "repo-1").await.unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(
            issues[0].labels,
            vec!["enhancement", "good first issue", "help wanted"]
        );

        // Empty labels
        let mut empty_labels = sample_issue("issue-2", 2, "No labels");
        empty_labels.labels = vec![];
        upsert_issue(&pool, &empty_labels).await.unwrap();

        let result = get_issues_by_repo(&pool, "repo-1").await.unwrap();
        let no_label_issue = result.iter().find(|i| i.id == "issue-2").unwrap();
        assert!(no_label_issue.labels.is_empty());

        pool.close().await;
    }

    #[tokio::test]
    async fn test_delete_stale_issues() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let i1 = sample_issue("issue-1", 1, "Keep");
        let i2 = sample_issue("issue-2", 2, "Delete");
        let i3 = sample_issue("issue-3", 3, "Keep too");

        upsert_issue(&pool, &i1).await.unwrap();
        upsert_issue(&pool, &i2).await.unwrap();
        upsert_issue(&pool, &i3).await.unwrap();

        let current_ids = vec!["issue-1".to_string(), "issue-3".to_string()];
        let deleted = delete_stale_issues(&pool, "repo-1", &current_ids, false)
            .await
            .unwrap();

        assert_eq!(deleted, 1, "should delete 1 stale issue");

        let remaining = get_issues_by_repo(&pool, "repo-1").await.unwrap();
        assert_eq!(remaining.len(), 2);
        let ids: Vec<&str> = remaining.iter().map(|i| i.id.as_str()).collect();
        assert!(ids.contains(&"issue-1"));
        assert!(ids.contains(&"issue-3"));
        assert!(!ids.contains(&"issue-2"));

        // Empty ids without allow_full_delete → no-op
        let deleted = delete_stale_issues(&pool, "repo-1", &[], false)
            .await
            .unwrap();
        assert_eq!(deleted, 0);
        assert_eq!(get_issues_by_repo(&pool, "repo-1").await.unwrap().len(), 2);

        // Empty ids with allow_full_delete → wipes all
        let deleted = delete_stale_issues(&pool, "repo-1", &[], true)
            .await
            .unwrap();
        assert_eq!(deleted, 2);
        assert!(
            get_issues_by_repo(&pool, "repo-1")
                .await
                .unwrap()
                .is_empty()
        );

        pool.close().await;
    }

    #[tokio::test]
    async fn test_delete_stale_issues_multi_chunk() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        // Create CHUNK_SIZE + 2 issues total (1000)
        let total = 1000_u32;
        let keep_count = 3_u32;

        for i in 1..=total {
            let issue = sample_issue(&format!("issue-{i}"), i, &format!("Issue {i}"));
            upsert_issue(&pool, &issue).await.unwrap();
        }

        // Keep only 3 issues — the remaining 997 are stale and span 2 chunks
        let current_ids: Vec<String> = (1..=keep_count).map(|i| format!("issue-{i}")).collect();

        let deleted = delete_stale_issues(&pool, "repo-1", &current_ids, false)
            .await
            .unwrap();

        assert_eq!(
            deleted,
            u64::from(total - keep_count),
            "should delete all stale issues across chunk boundaries"
        );

        let remaining = get_issues_by_repo(&pool, "repo-1").await.unwrap();
        assert_eq!(remaining.len(), keep_count as usize);

        let remaining_ids: std::collections::HashSet<String> =
            remaining.into_iter().map(|i| i.id).collect();
        for i in 1..=keep_count {
            assert!(remaining_ids.contains(&format!("issue-{i}")));
        }

        pool.close().await;
    }

    #[test]
    fn test_unknown_enum_values_return_error() {
        assert!(
            issue_state_from_str("OPEN").is_err(),
            "wrong case should fail"
        );
        assert!(issue_state_from_str("bogus").is_err());
        assert!(priority_from_str("").is_err());
        assert!(
            priority_from_str("CRITICAL").is_err(),
            "wrong case should fail"
        );
    }
}
