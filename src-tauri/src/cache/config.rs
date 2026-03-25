use log::warn;
use sqlx::SqlitePool;

use crate::error::AppError;
use crate::types::AppConfig;

/// Keys used in the `config` key-value table.
const KEY_POLL_INTERVAL: &str = "poll_interval_secs";
const KEY_MAX_WORKSPACES: &str = "max_active_workspaces";
const KEY_GITHUB_TOKEN: &str = "github_token";
const KEY_DATA_DIR: &str = "data_dir";
const KEY_WORKSPACES_DIR: &str = "workspaces_dir";

/// Minimum allowed value for `poll_interval_secs`.
const MIN_POLL_INTERVAL_SECS: u64 = 30;

/// Row from the `config` key-value table.
#[derive(sqlx::FromRow)]
struct ConfigRow {
    key: String,
    value: String,
}

/// Read the full application configuration from the `config` table.
///
/// Missing keys are filled with [`AppConfig::default()`] values.
#[allow(dead_code)]
pub async fn get_config(pool: &SqlitePool) -> Result<AppConfig, AppError> {
    let rows: Vec<ConfigRow> = sqlx::query_as("SELECT key, value FROM config")
        .fetch_all(pool)
        .await?;

    let mut config = AppConfig::default();

    for row in rows {
        match row.key.as_str() {
            KEY_POLL_INTERVAL => match row.value.parse::<u64>() {
                Ok(v) => config.poll_interval_secs = v,
                Err(_) => warn!(
                    "ignoring non-parseable config value for '{}': '{}', using default",
                    KEY_POLL_INTERVAL, row.value
                ),
            },
            KEY_MAX_WORKSPACES => match row.value.parse::<u32>() {
                Ok(v) => config.max_active_workspaces = v,
                Err(_) => warn!(
                    "ignoring non-parseable config value for '{}': '{}', using default",
                    KEY_MAX_WORKSPACES, row.value
                ),
            },
            KEY_GITHUB_TOKEN => {
                config.github_token = Some(row.value);
            }
            KEY_DATA_DIR => {
                config.data_dir = Some(row.value);
            }
            KEY_WORKSPACES_DIR => {
                config.workspaces_dir = Some(row.value);
            }
            _ => {} // ignore unknown keys for forward-compat
        }
    }

    Ok(config)
}

/// Persist the given configuration to the `config` table.
///
/// All writes are wrapped in a transaction for atomicity.
/// `Option::None` fields are deleted so that future reads fall back
/// to [`AppConfig::default()`]. Values below documented minimums are
/// clamped with a warning. Returns the configuration as re-read from DB.
#[allow(dead_code)]
pub async fn set_config(pool: &SqlitePool, config: &AppConfig) -> Result<AppConfig, AppError> {
    let poll_interval = if config.poll_interval_secs < MIN_POLL_INTERVAL_SECS {
        warn!(
            "poll_interval_secs {} is below the minimum of {}; clamping",
            config.poll_interval_secs, MIN_POLL_INTERVAL_SECS
        );
        MIN_POLL_INTERVAL_SECS
    } else {
        config.poll_interval_secs
    };

    let mut tx = pool.begin().await?;

    upsert_key(&mut *tx, KEY_POLL_INTERVAL, &poll_interval.to_string()).await?;
    upsert_key(
        &mut *tx,
        KEY_MAX_WORKSPACES,
        &config.max_active_workspaces.to_string(),
    )
    .await?;

    set_optional_key(&mut *tx, KEY_GITHUB_TOKEN, config.github_token.as_deref()).await?;
    set_optional_key(&mut *tx, KEY_DATA_DIR, config.data_dir.as_deref()).await?;
    set_optional_key(
        &mut *tx,
        KEY_WORKSPACES_DIR,
        config.workspaces_dir.as_deref(),
    )
    .await?;

    tx.commit().await?;

    get_config(pool).await
}

/// Upsert a single key-value pair.
async fn upsert_key(
    conn: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    key: &str,
    value: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO config (key, value) VALUES ($1, $2) \
         ON CONFLICT(key) DO UPDATE SET value = $2",
    )
    .bind(key)
    .bind(value)
    .execute(conn)
    .await?;
    Ok(())
}

/// Delete a key from the config table.
async fn delete_key(
    conn: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    key: &str,
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM config WHERE key = $1")
        .bind(key)
        .execute(conn)
        .await?;
    Ok(())
}

/// Upsert if `Some`, delete if `None`.
async fn set_optional_key(
    conn: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    key: &str,
    value: Option<&str>,
) -> Result<(), AppError> {
    match value {
        Some(v) => upsert_key(conn, key, v).await,
        None => delete_key(conn, key).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::db::init_db;

    async fn test_pool() -> (SqlitePool, tempfile::TempDir) {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = init_db(&tmp.path().join("test.db")).await.unwrap();
        (pool, tmp)
    }

    #[tokio::test]
    async fn test_get_config_defaults() {
        let (pool, _tmp) = test_pool().await;

        let config = get_config(&pool).await.unwrap();

        // Migration inserts poll_interval_secs=300 and max_active_workspaces=3
        assert_eq!(config.poll_interval_secs, 300);
        assert_eq!(config.max_active_workspaces, 3);
        assert!(config.github_token.is_none());
        assert!(config.data_dir.is_none());
        assert!(config.workspaces_dir.is_none());

        pool.close().await;
    }

    #[tokio::test]
    async fn test_set_config_partial() {
        let (pool, _tmp) = test_pool().await;

        // Read defaults, modify one field
        let mut config = get_config(&pool).await.unwrap();
        config.poll_interval_secs = 60;

        let result = set_config(&pool, &config).await.unwrap();
        assert_eq!(result.poll_interval_secs, 60);
        // Other fields unchanged
        assert_eq!(result.max_active_workspaces, 3);
        assert!(result.github_token.is_none());

        pool.close().await;
    }

    #[tokio::test]
    async fn test_set_config_full() {
        let (pool, _tmp) = test_pool().await;

        let config = AppConfig {
            poll_interval_secs: 120,
            max_active_workspaces: 5,
            github_token: Some("ghp_test".to_string()),
            data_dir: Some("/custom/data".to_string()),
            workspaces_dir: Some("/custom/ws".to_string()),
        };

        let result = set_config(&pool, &config).await.unwrap();
        assert_eq!(result.poll_interval_secs, 120);
        assert_eq!(result.max_active_workspaces, 5);
        assert_eq!(result.github_token.as_deref(), Some("ghp_test"));
        assert_eq!(result.data_dir.as_deref(), Some("/custom/data"));
        assert_eq!(result.workspaces_dir.as_deref(), Some("/custom/ws"));

        // Clear optional fields — they should revert to None
        let mut cleared = result;
        cleared.github_token = None;
        cleared.data_dir = None;

        let after = set_config(&pool, &cleared).await.unwrap();
        assert!(after.github_token.is_none());
        assert!(after.data_dir.is_none());
        // workspaces_dir should remain
        assert_eq!(after.workspaces_dir.as_deref(), Some("/custom/ws"));

        pool.close().await;
    }

    #[tokio::test]
    async fn test_set_config_clamps_poll_interval() {
        let (pool, _tmp) = test_pool().await;

        let mut config = get_config(&pool).await.unwrap();
        config.poll_interval_secs = 5; // below minimum of 30

        let result = set_config(&pool, &config).await.unwrap();
        assert_eq!(
            result.poll_interval_secs, MIN_POLL_INTERVAL_SECS,
            "should clamp to minimum"
        );

        pool.close().await;
    }
}
