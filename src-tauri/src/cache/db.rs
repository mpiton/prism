use std::path::Path;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

use crate::error::AppError;

/// Initialize the `SQLite` database pool at the given path.
///
/// Creates the parent directory and file if missing, runs all
/// compile-time embedded migrations, and returns a reusable pool.
#[allow(dead_code)] // Will be called from Tauri setup in a later task
pub async fn init_db(db_path: &Path) -> Result<SqlitePool, AppError> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            AppError::Io(std::io::Error::new(
                e.kind(),
                format!(
                    "failed to create database directory '{}': {e}",
                    parent.display()
                ),
            ))
        })?;
    }

    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    sqlx::migrate!("src/cache/migrations").run(&pool).await?;

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_init_db_creates_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("prism_test.db");

        let pool = init_db(&db_path).await.unwrap();

        assert!(db_path.exists(), "database file should be created");
        pool.close().await;
    }

    #[tokio::test]
    async fn test_init_db_applies_migrations() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("prism_test.db");

        let pool = init_db(&db_path).await.unwrap();

        // sqlx creates the _sqlx_migrations tracking table when the
        // migrator runs, even if no migration files exist yet.
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='_sqlx_migrations'",
        )
        .fetch_one(&pool)
        .await
        .expect("query should succeed");

        assert_eq!(row.0, 1, "_sqlx_migrations table should exist");
        pool.close().await;
    }

    #[tokio::test]
    async fn test_init_db_idempotent() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("prism_test.db");

        // First call — creates the database
        let pool1 = init_db(&db_path).await.unwrap();
        pool1.close().await;

        // Second call — should succeed without errors on existing DB
        let pool2 = init_db(&db_path).await.unwrap();
        pool2.close().await;

        assert!(db_path.exists(), "database file should still exist");
    }

    // ── T-015: Migration schema tests ─────────────────────────────

    /// All 10 application tables must exist after migration.
    #[tokio::test]
    async fn test_migration_creates_all_tables() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = init_db(&tmp.path().join("test.db")).await.unwrap();

        let expected_tables = [
            "repos",
            "pull_requests",
            "review_requests",
            "reviews",
            "issues",
            "activity",
            "workspaces",
            "workspace_notes",
            "config",
            "notification_log",
        ];

        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE '\\__%' ESCAPE '\\' ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let table_names: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();

        assert_eq!(
            table_names.len(),
            expected_tables.len(),
            "expected exactly {} tables, found: {table_names:?}",
            expected_tables.len()
        );

        for table in &expected_tables {
            assert!(
                table_names.contains(table),
                "table '{table}' should exist, found: {table_names:?}"
            );
        }

        pool.close().await;
    }

    /// All expected indexes must exist after migration.
    #[tokio::test]
    async fn test_migration_creates_all_indexes() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = init_db(&tmp.path().join("test.db")).await.unwrap();

        let expected_indexes = [
            "idx_activity_created_at",
            "idx_activity_is_read",
            "idx_activity_issue_id",
            "idx_activity_pull_request_id",
            "idx_activity_repo_id",
            "idx_issues_repo_id",
            "idx_issues_state",
            "idx_pull_requests_repo_id",
            "idx_pull_requests_state",
            "idx_review_requests_pull_request_id",
            "idx_review_requests_reviewer",
            "idx_reviews_pull_request_id",
            "idx_workspace_notes_workspace_id",
            "idx_workspaces_repo_id",
            "idx_workspaces_state",
        ];

        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%' ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        let index_names: Vec<&str> = rows.iter().map(|r| r.0.as_str()).collect();

        assert_eq!(
            index_names.len(),
            expected_indexes.len(),
            "expected exactly {} indexes, found: {index_names:?}",
            expected_indexes.len()
        );

        for idx in &expected_indexes {
            assert!(
                index_names.contains(idx),
                "index '{idx}' should exist, found: {index_names:?}"
            );
        }

        pool.close().await;
    }

    /// Default config rows must be inserted by the migration.
    #[tokio::test]
    async fn test_migration_inserts_default_config() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = init_db(&tmp.path().join("test.db")).await.unwrap();

        let poll: (String,) =
            sqlx::query_as("SELECT value FROM config WHERE key = 'poll_interval_secs'")
                .fetch_one(&pool)
                .await
                .expect("poll_interval_secs should exist");
        assert_eq!(poll.0, "300");

        let max_ws: (String,) =
            sqlx::query_as("SELECT value FROM config WHERE key = 'max_active_workspaces'")
                .fetch_one(&pool)
                .await
                .expect("max_active_workspaces should exist");
        assert_eq!(max_ws.0, "3");

        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM config")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count.0, 2, "config should have exactly 2 default rows");

        pool.close().await;
    }
}
