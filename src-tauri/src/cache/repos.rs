use sqlx::SqlitePool;

use crate::error::AppError;
use crate::types::Repo;

/// Row representation matching the `repos` table columns.
/// Handles `INTEGER → bool` conversion for `is_archived` and `enabled`.
#[derive(sqlx::FromRow)]
struct RepoRow {
    id: String,
    org: String,
    name: String,
    full_name: String,
    url: String,
    default_branch: String,
    is_archived: bool,
    enabled: bool,
    local_path: Option<String>,
    last_sync_at: Option<String>,
}

impl From<RepoRow> for Repo {
    fn from(row: RepoRow) -> Self {
        Self {
            id: row.id,
            org: row.org,
            name: row.name,
            full_name: row.full_name,
            url: row.url,
            default_branch: row.default_branch,
            is_archived: row.is_archived,
            enabled: row.enabled,
            local_path: row.local_path,
            last_sync_at: row.last_sync_at,
        }
    }
}

/// Explicit column list used in all SELECT queries to avoid `SELECT *` fragility.
const REPO_COLS: &str =
    "id, org, name, full_name, url, default_branch, is_archived, enabled, local_path, last_sync_at";

/// Insert or update a repo. On conflict (same `id`), updates GitHub-provided
/// fields while preserving local state (`enabled`, `local_path`, `last_sync_at`).
/// Uses `RETURNING` for an atomic read-after-write.
#[allow(dead_code)]
pub async fn upsert_repo(pool: &SqlitePool, repo: &Repo) -> Result<Repo, AppError> {
    let sql = format!(
        "INSERT INTO repos (id, org, name, full_name, url, default_branch, is_archived)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT(id) DO UPDATE SET
             org = excluded.org,
             name = excluded.name,
             full_name = excluded.full_name,
             url = excluded.url,
             default_branch = excluded.default_branch,
             is_archived = excluded.is_archived
         RETURNING {REPO_COLS}"
    );

    let row: RepoRow = sqlx::query_as(&sql)
        .bind(&repo.id)
        .bind(&repo.org)
        .bind(&repo.name)
        .bind(&repo.full_name)
        .bind(&repo.url)
        .bind(&repo.default_branch)
        .bind(repo.is_archived)
        .fetch_one(pool)
        .await?;

    Ok(Repo::from(row))
}

/// Return all repos ordered by `full_name`.
#[allow(dead_code)]
pub async fn list_repos(pool: &SqlitePool) -> Result<Vec<Repo>, AppError> {
    let sql = format!("SELECT {REPO_COLS} FROM repos ORDER BY full_name");
    let rows: Vec<RepoRow> = sqlx::query_as(&sql).fetch_all(pool).await?;

    Ok(rows.into_iter().map(Repo::from).collect())
}

/// Return a single repo by ID, or `AppError::NotFound`.
#[allow(dead_code)]
pub async fn get_repo(pool: &SqlitePool, id: &str) -> Result<Repo, AppError> {
    let sql = format!("SELECT {REPO_COLS} FROM repos WHERE id = $1");
    let row: Option<RepoRow> = sqlx::query_as(&sql).bind(id).fetch_optional(pool).await?;

    row.map(Repo::from)
        .ok_or_else(|| AppError::NotFound(format!("repo '{id}'")))
}

/// Toggle a repo's `enabled` flag and return the updated repo.
/// Uses `RETURNING` for an atomic read-after-write.
#[allow(dead_code)]
pub async fn toggle_repo(pool: &SqlitePool, id: &str, enabled: bool) -> Result<Repo, AppError> {
    let sql = format!("UPDATE repos SET enabled = $1 WHERE id = $2 RETURNING {REPO_COLS}");
    let row: Option<RepoRow> = sqlx::query_as(&sql)
        .bind(enabled)
        .bind(id)
        .fetch_optional(pool)
        .await?;

    row.map(Repo::from)
        .ok_or_else(|| AppError::NotFound(format!("repo '{id}'")))
}

/// Set or clear the local clone path for a repo.
/// Uses `RETURNING` for an atomic read-after-write.
#[allow(dead_code)]
pub async fn set_local_path(
    pool: &SqlitePool,
    id: &str,
    path: Option<&str>,
) -> Result<Repo, AppError> {
    let sql = format!("UPDATE repos SET local_path = $1 WHERE id = $2 RETURNING {REPO_COLS}");
    let row: Option<RepoRow> = sqlx::query_as(&sql)
        .bind(path)
        .bind(id)
        .fetch_optional(pool)
        .await?;

    row.map(Repo::from)
        .ok_or_else(|| AppError::NotFound(format!("repo '{id}'")))
}

/// Update the `last_sync_at` timestamp for a repo.
#[allow(dead_code)]
pub async fn update_last_sync(
    pool: &SqlitePool,
    id: &str,
    synced_at: &str,
) -> Result<(), AppError> {
    let rows_affected = sqlx::query("UPDATE repos SET last_sync_at = $1 WHERE id = $2")
        .bind(synced_at)
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();

    if rows_affected == 0 {
        return Err(AppError::NotFound(format!("repo '{id}'")));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::db::init_db;

    /// Helper: create a test DB pool with migrations applied.
    /// Returns `(pool, _tmp)` — keep `_tmp` alive to prevent early cleanup.
    async fn test_pool() -> (SqlitePool, tempfile::TempDir) {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = init_db(&tmp.path().join("test.db")).await.unwrap();
        (pool, tmp)
    }

    /// Helper: create a sample Repo for testing.
    fn sample_repo(id: &str, org: &str, name: &str) -> Repo {
        Repo {
            id: id.to_string(),
            org: org.to_string(),
            name: name.to_string(),
            full_name: format!("{org}/{name}"),
            url: format!("https://github.com/{org}/{name}"),
            default_branch: "main".to_string(),
            is_archived: false,
            enabled: true,
            local_path: None,
            last_sync_at: None,
        }
    }

    #[tokio::test]
    async fn test_upsert_repo_insert() {
        let (pool, _tmp) = test_pool().await;
        let repo = sample_repo("r-1", "mpiton", "prism");

        let result = upsert_repo(&pool, &repo).await.unwrap();

        assert_eq!(result.id, "r-1");
        assert_eq!(result.org, "mpiton");
        assert_eq!(result.name, "prism");
        assert_eq!(result.full_name, "mpiton/prism");
        assert!(result.enabled, "new repos should be enabled by default");
        assert!(result.local_path.is_none());
        assert!(result.last_sync_at.is_none());

        pool.close().await;
    }

    #[tokio::test]
    async fn test_upsert_repo_update() {
        let (pool, _tmp) = test_pool().await;
        let repo = sample_repo("r-1", "mpiton", "prism");
        upsert_repo(&pool, &repo).await.unwrap();

        // Toggle enabled off, then upsert again — enabled should be preserved
        toggle_repo(&pool, "r-1", false).await.unwrap();

        let mut updated = repo.clone();
        updated.url = "https://github.com/mpiton/prism-v2".to_string();

        let result = upsert_repo(&pool, &updated).await.unwrap();

        assert_eq!(result.url, "https://github.com/mpiton/prism-v2");
        assert!(
            !result.enabled,
            "upsert should preserve local enabled state"
        );

        pool.close().await;
    }

    #[tokio::test]
    async fn test_list_repos_empty() {
        let (pool, _tmp) = test_pool().await;

        let repos = list_repos(&pool).await.unwrap();

        assert!(repos.is_empty());

        pool.close().await;
    }

    #[tokio::test]
    async fn test_list_repos_multiple() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo("r-1", "mpiton", "alpha"))
            .await
            .unwrap();
        upsert_repo(&pool, &sample_repo("r-2", "mpiton", "beta"))
            .await
            .unwrap();
        upsert_repo(&pool, &sample_repo("r-3", "other", "gamma"))
            .await
            .unwrap();

        let repos = list_repos(&pool).await.unwrap();

        assert_eq!(repos.len(), 3);
        // Ordered by full_name
        assert_eq!(repos[0].full_name, "mpiton/alpha");
        assert_eq!(repos[1].full_name, "mpiton/beta");
        assert_eq!(repos[2].full_name, "other/gamma");

        pool.close().await;
    }

    #[tokio::test]
    async fn test_toggle_repo() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo("r-1", "mpiton", "prism"))
            .await
            .unwrap();

        let disabled = toggle_repo(&pool, "r-1", false).await.unwrap();
        assert!(!disabled.enabled);

        let enabled = toggle_repo(&pool, "r-1", true).await.unwrap();
        assert!(enabled.enabled);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_set_local_path() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo("r-1", "mpiton", "prism"))
            .await
            .unwrap();

        let with_path = set_local_path(&pool, "r-1", Some("/home/user/repos/prism"))
            .await
            .unwrap();
        assert_eq!(
            with_path.local_path.as_deref(),
            Some("/home/user/repos/prism")
        );

        let cleared = set_local_path(&pool, "r-1", None).await.unwrap();
        assert!(cleared.local_path.is_none());

        pool.close().await;
    }

    #[tokio::test]
    async fn test_get_repo_not_found() {
        let (pool, _tmp) = test_pool().await;

        let result = get_repo(&pool, "nonexistent").await;

        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), AppError::NotFound(_)),
            "should return NotFound for missing repo"
        );

        pool.close().await;
    }
}
