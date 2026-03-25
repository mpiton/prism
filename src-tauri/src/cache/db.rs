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
}
