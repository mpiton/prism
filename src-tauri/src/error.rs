use thiserror::Error;

/// Centralized error enum for the `PRism` backend.
/// Each variant maps to a specific domain failure.
/// Implements `Display` via thiserror and `Into<String>` for Tauri IPC.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("GitHub API error: {0}")]
    GitHub(String),

    #[error("GraphQL error: {0}")]
    GraphQL(String),

    #[error("authentication error: {0}")]
    Auth(String),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("workspace error: {0}")]
    Workspace(String),

    #[error("PTY error: {0}")]
    Pty(String),

    #[error("git error: {0}")]
    Git(String),

    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("rate limited until {reset_at}")]
    RateLimit { reset_at: String },
}

impl From<AppError> for String {
    fn from(err: AppError) -> Self {
        err.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_error_display_database() {
        let sqlx_err = sqlx::Error::RowNotFound;
        let err = AppError::Database(sqlx_err);
        let msg = err.to_string();
        assert!(
            msg.contains("database error"),
            "expected 'database error' in '{msg}'"
        );
    }

    #[test]
    fn test_app_error_display_github() {
        let err = AppError::GitHub("rate limit exceeded".into());
        assert_eq!(err.to_string(), "GitHub API error: rate limit exceeded");
    }

    #[test]
    fn test_app_error_into_string() {
        let err = AppError::NotFound("PR #42".into());
        let s: String = err.into();
        assert_eq!(s, "not found: PR #42");

        let err = AppError::RateLimit {
            reset_at: "2026-03-24T18:00:00Z".into(),
        };
        let s: String = err.into();
        assert_eq!(s, "rate limited until 2026-03-24T18:00:00Z");
    }

    #[test]
    fn test_app_error_from_sqlx() {
        let sqlx_err = sqlx::Error::RowNotFound;
        let app_err: AppError = sqlx_err.into();
        assert!(
            matches!(app_err, AppError::Database(_)),
            "expected Database variant"
        );
    }

    #[test]
    fn test_app_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let app_err: AppError = io_err.into();
        assert!(matches!(app_err, AppError::Io(_)), "expected Io variant");
        assert!(
            app_err.to_string().contains("file missing"),
            "expected 'file missing' in '{}'",
            app_err
        );
    }

    #[test]
    fn test_app_error_display_all_string_variants() {
        let cases = [
            (
                AppError::GraphQL("bad query".into()),
                "GraphQL error: bad query",
            ),
            (
                AppError::Auth("token expired".into()),
                "authentication error: token expired",
            ),
            (
                AppError::Config("missing key".into()),
                "configuration error: missing key",
            ),
            (
                AppError::Workspace("not found".into()),
                "workspace error: not found",
            ),
            (
                AppError::Pty("spawn failed".into()),
                "PTY error: spawn failed",
            ),
            (
                AppError::Git("detached HEAD".into()),
                "git error: detached HEAD",
            ),
        ];
        for (err, expected) in cases {
            assert_eq!(err.to_string(), expected);
        }
    }

    #[test]
    fn test_app_error_display_migrate() {
        let err = AppError::Migrate(sqlx::migrate::MigrateError::VersionMissing(1));
        let msg = err.to_string();
        assert!(
            msg.contains("migration error"),
            "expected 'migration error' in '{msg}'"
        );
    }
}
