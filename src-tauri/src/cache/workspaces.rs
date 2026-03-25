use chrono::{SecondsFormat, Utc};
use sqlx::SqlitePool;

use crate::error::AppError;
use crate::types::{Workspace, WorkspaceNote, WorkspaceState};

/// RFC 3339 timestamp with millisecond precision for stable lexicographic ordering.
#[allow(dead_code)]
fn now_utc_millis() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// Row representation matching the `workspaces` table columns.
/// `last_active_at` is a DB-internal timestamp not exposed on the domain struct.
#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct WorkspaceRow {
    id: String,
    repo_id: String,
    pull_request_number: i64,
    state: String,
    worktree_path: Option<String>,
    session_id: Option<String>,
    last_active_at: Option<String>,
    created_at: String,
    updated_at: String,
}

/// Row representation matching the `workspace_notes` table columns.
#[derive(sqlx::FromRow)]
struct WorkspaceNoteRow {
    id: String,
    workspace_id: String,
    content: String,
    created_at: String,
}

fn workspace_state_to_str(s: &WorkspaceState) -> &'static str {
    match s {
        WorkspaceState::Active => "active",
        WorkspaceState::Suspended => "suspended",
        WorkspaceState::Archived => "archived",
    }
}

fn workspace_state_from_str(s: &str) -> Result<WorkspaceState, AppError> {
    match s {
        "active" => Ok(WorkspaceState::Active),
        "suspended" => Ok(WorkspaceState::Suspended),
        "archived" => Ok(WorkspaceState::Archived),
        _ => Err(AppError::Config(format!("unknown WorkspaceState: {s}"))),
    }
}

impl TryFrom<WorkspaceRow> for Workspace {
    type Error = AppError;

    fn try_from(row: WorkspaceRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            repo_id: row.repo_id,
            pull_request_number: u32::try_from(row.pull_request_number).map_err(|_| {
                AppError::Config(format!(
                    "invalid pull_request_number: {}",
                    row.pull_request_number
                ))
            })?,
            state: workspace_state_from_str(&row.state)?,
            worktree_path: row.worktree_path,
            session_id: row.session_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

impl From<WorkspaceNoteRow> for WorkspaceNote {
    fn from(row: WorkspaceNoteRow) -> Self {
        Self {
            id: row.id,
            workspace_id: row.workspace_id,
            content: row.content,
            created_at: row.created_at,
        }
    }
}

/// Explicit column list for workspace queries.
/// Includes `last_active_at` (populated by `WorkspaceRow`) which is discarded
/// by `TryFrom<WorkspaceRow> for Workspace` — it is a DB-internal timestamp.
const WS_COLS: &str = "id, repo_id, pull_request_number, state, worktree_path, session_id, last_active_at, created_at, updated_at";

const NOTE_COLS: &str = "id, workspace_id, content, created_at";

// ── Workspace CRUD ────────────────────────────────────────────────

/// Create a new workspace. Uses `RETURNING` for atomic read-after-write.
#[allow(dead_code)]
pub async fn create_workspace(pool: &SqlitePool, ws: &Workspace) -> Result<Workspace, AppError> {
    let sql = format!(
        "INSERT INTO workspaces (id, repo_id, pull_request_number, state, worktree_path, session_id, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING {WS_COLS}"
    );

    let row: WorkspaceRow = sqlx::query_as(&sql)
        .bind(&ws.id)
        .bind(&ws.repo_id)
        .bind(i64::from(ws.pull_request_number))
        .bind(workspace_state_to_str(&ws.state))
        .bind(&ws.worktree_path)
        .bind(&ws.session_id)
        .bind(&ws.created_at)
        .bind(&ws.updated_at)
        .fetch_one(pool)
        .await?;

    Workspace::try_from(row)
}

/// Retrieve a single workspace by ID.
#[allow(dead_code)]
pub async fn get_workspace(pool: &SqlitePool, id: &str) -> Result<Option<Workspace>, AppError> {
    let sql = format!("SELECT {WS_COLS} FROM workspaces WHERE id = $1");
    let row: Option<WorkspaceRow> = sqlx::query_as(&sql).bind(id).fetch_optional(pool).await?;

    row.map(Workspace::try_from).transpose()
}

/// List workspaces filtered by state, ordered by `updated_at DESC`.
#[allow(dead_code)]
pub async fn list_workspaces(
    pool: &SqlitePool,
    state: Option<&WorkspaceState>,
) -> Result<Vec<Workspace>, AppError> {
    let rows: Vec<WorkspaceRow> = if let Some(st) = state {
        let sql = format!(
            "SELECT {WS_COLS} FROM workspaces WHERE state = $1 ORDER BY updated_at DESC, id ASC"
        );
        sqlx::query_as(&sql)
            .bind(workspace_state_to_str(st))
            .fetch_all(pool)
            .await?
    } else {
        let sql = format!("SELECT {WS_COLS} FROM workspaces ORDER BY updated_at DESC, id ASC");
        sqlx::query_as(&sql).fetch_all(pool).await?
    };

    rows.into_iter().map(Workspace::try_from).collect()
}

/// Update the workspace state and `updated_at` timestamp.
/// Returns the updated workspace via `RETURNING`.
#[allow(dead_code)]
pub async fn update_workspace_state(
    pool: &SqlitePool,
    id: &str,
    new_state: &WorkspaceState,
) -> Result<Workspace, AppError> {
    let now = now_utc_millis();
    let sql = format!(
        "UPDATE workspaces SET state = $1, updated_at = $2 WHERE id = $3 RETURNING {WS_COLS}"
    );

    let row: Option<WorkspaceRow> = sqlx::query_as(&sql)
        .bind(workspace_state_to_str(new_state))
        .bind(&now)
        .bind(id)
        .fetch_optional(pool)
        .await?;

    row.map(Workspace::try_from)
        .transpose()?
        .ok_or_else(|| AppError::NotFound(format!("workspace '{id}'")))
}

/// Update the Claude Code session ID and `updated_at` timestamp.
/// Returns the updated workspace via `RETURNING`.
#[allow(dead_code)]
pub async fn update_claude_session(
    pool: &SqlitePool,
    id: &str,
    session_id: Option<&str>,
) -> Result<Workspace, AppError> {
    let now = now_utc_millis();
    let sql = format!(
        "UPDATE workspaces SET session_id = $1, updated_at = $2 WHERE id = $3 RETURNING {WS_COLS}"
    );

    let row: Option<WorkspaceRow> = sqlx::query_as(&sql)
        .bind(session_id)
        .bind(&now)
        .bind(id)
        .fetch_optional(pool)
        .await?;

    row.map(Workspace::try_from)
        .transpose()?
        .ok_or_else(|| AppError::NotFound(format!("workspace '{id}'")))
}

/// Touch the `last_active_at` DB-internal timestamp. Returns `true` if updated.
#[allow(dead_code)]
pub async fn update_last_active(pool: &SqlitePool, id: &str) -> Result<bool, AppError> {
    let now = now_utc_millis();
    let result = sqlx::query("UPDATE workspaces SET last_active_at = $1 WHERE id = $2")
        .bind(&now)
        .bind(id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

/// Archive a workspace: set state to `Archived`, clear `worktree_path`,
/// and update `updated_at`. Returns the archived workspace.
#[allow(dead_code)]
pub async fn archive_workspace(pool: &SqlitePool, id: &str) -> Result<Workspace, AppError> {
    let now = now_utc_millis();
    let sql = format!(
        "UPDATE workspaces SET state = $1, worktree_path = NULL, updated_at = $2 WHERE id = $3 RETURNING {WS_COLS}"
    );

    let row: Option<WorkspaceRow> = sqlx::query_as(&sql)
        .bind(workspace_state_to_str(&WorkspaceState::Archived))
        .bind(&now)
        .bind(id)
        .fetch_optional(pool)
        .await?;

    row.map(Workspace::try_from)
        .transpose()?
        .ok_or_else(|| AppError::NotFound(format!("workspace '{id}'")))
}

// ── Workspace Notes CRUD ──────────────────────────────────────────

/// Add a note to a workspace. Uses `RETURNING` for atomic read-after-write.
#[allow(dead_code)]
pub async fn add_note(pool: &SqlitePool, note: &WorkspaceNote) -> Result<WorkspaceNote, AppError> {
    let sql = format!(
        "INSERT INTO workspace_notes (id, workspace_id, content, created_at)
         VALUES ($1, $2, $3, $4)
         RETURNING {NOTE_COLS}"
    );

    let row: WorkspaceNoteRow = sqlx::query_as(&sql)
        .bind(&note.id)
        .bind(&note.workspace_id)
        .bind(&note.content)
        .bind(&note.created_at)
        .fetch_one(pool)
        .await?;

    Ok(WorkspaceNote::from(row))
}

/// Get all notes for a workspace, ordered by `created_at ASC` (oldest first).
#[allow(dead_code)]
pub async fn get_notes(
    pool: &SqlitePool,
    workspace_id: &str,
) -> Result<Vec<WorkspaceNote>, AppError> {
    let sql = format!(
        "SELECT {NOTE_COLS} FROM workspace_notes WHERE workspace_id = $1 ORDER BY created_at ASC, id ASC"
    );
    let rows: Vec<WorkspaceNoteRow> = sqlx::query_as(&sql)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?;

    Ok(rows.into_iter().map(WorkspaceNote::from).collect())
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

    fn sample_workspace(id: &str, pr_number: u32) -> Workspace {
        Workspace {
            id: id.to_string(),
            repo_id: "repo-1".to_string(),
            pull_request_number: pr_number,
            state: WorkspaceState::Active,
            worktree_path: Some(format!(
                "/home/user/.prism/workspaces/prism/worktrees/pr-{pr_number}"
            )),
            session_id: None,
            created_at: "2026-03-20T10:00:00Z".to_string(),
            updated_at: "2026-03-20T10:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn test_create_workspace() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let ws = sample_workspace("ws-1", 42);
        let result = create_workspace(&pool, &ws).await.unwrap();

        assert_eq!(result.id, "ws-1");
        assert_eq!(result.repo_id, "repo-1");
        assert_eq!(result.pull_request_number, 42);
        assert_eq!(result.state, WorkspaceState::Active);
        assert!(result.worktree_path.is_some());
        assert!(result.session_id.is_none());

        // Different id but same (repo_id, pull_request_number) → UNIQUE constraint
        let ws2 = sample_workspace("ws-other", 42);
        let dup = create_workspace(&pool, &ws2).await;
        assert!(dup.is_err());

        pool.close().await;
    }

    #[tokio::test]
    async fn test_get_workspace() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let ws = sample_workspace("ws-1", 42);
        create_workspace(&pool, &ws).await.unwrap();

        let found = get_workspace(&pool, "ws-1").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().pull_request_number, 42);

        let missing = get_workspace(&pool, "nonexistent").await.unwrap();
        assert!(missing.is_none());

        pool.close().await;
    }

    #[tokio::test]
    async fn test_list_workspaces_by_state() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let ws1 = sample_workspace("ws-1", 1);
        let ws2 = sample_workspace("ws-2", 2);
        create_workspace(&pool, &ws1).await.unwrap();
        create_workspace(&pool, &ws2).await.unwrap();

        // Suspend ws-2
        update_workspace_state(&pool, "ws-2", &WorkspaceState::Suspended)
            .await
            .unwrap();

        // List all
        let all = list_workspaces(&pool, None).await.unwrap();
        assert_eq!(all.len(), 2);

        // List active only
        let active = list_workspaces(&pool, Some(&WorkspaceState::Active))
            .await
            .unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, "ws-1");

        // List suspended only
        let suspended = list_workspaces(&pool, Some(&WorkspaceState::Suspended))
            .await
            .unwrap();
        assert_eq!(suspended.len(), 1);
        assert_eq!(suspended[0].id, "ws-2");

        // List archived — none yet
        let archived = list_workspaces(&pool, Some(&WorkspaceState::Archived))
            .await
            .unwrap();
        assert!(archived.is_empty());

        pool.close().await;
    }

    #[tokio::test]
    async fn test_update_workspace_state() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let ws = sample_workspace("ws-1", 42);
        create_workspace(&pool, &ws).await.unwrap();

        let updated = update_workspace_state(&pool, "ws-1", &WorkspaceState::Suspended)
            .await
            .unwrap();
        assert_eq!(updated.state, WorkspaceState::Suspended);
        assert_ne!(
            updated.updated_at, ws.updated_at,
            "updated_at should change"
        );

        // Non-existent workspace returns NotFound
        let err = update_workspace_state(&pool, "nonexistent", &WorkspaceState::Active).await;
        assert!(err.is_err());

        pool.close().await;
    }

    #[tokio::test]
    async fn test_update_claude_session() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let ws = sample_workspace("ws-1", 42);
        create_workspace(&pool, &ws).await.unwrap();

        // Set session
        let updated = update_claude_session(&pool, "ws-1", Some("session-abc"))
            .await
            .unwrap();
        assert_eq!(updated.session_id.as_deref(), Some("session-abc"));
        assert_ne!(updated.updated_at, ws.updated_at);

        // Clear session
        let cleared = update_claude_session(&pool, "ws-1", None).await.unwrap();
        assert!(cleared.session_id.is_none());

        // Non-existent workspace
        let err = update_claude_session(&pool, "nonexistent", Some("s")).await;
        assert!(err.is_err());

        pool.close().await;
    }

    #[tokio::test]
    async fn test_update_last_active() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let ws = sample_workspace("ws-1", 42);
        create_workspace(&pool, &ws).await.unwrap();

        // Verify last_active_at starts as NULL
        let before: Option<String> =
            sqlx::query_scalar("SELECT last_active_at FROM workspaces WHERE id = $1")
                .bind("ws-1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(before.is_none(), "last_active_at should be NULL initially");

        let updated = update_last_active(&pool, "ws-1").await.unwrap();
        assert!(updated);

        // Verify last_active_at is now set
        let after: Option<String> =
            sqlx::query_scalar("SELECT last_active_at FROM workspaces WHERE id = $1")
                .bind("ws-1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(after.is_some(), "last_active_at should be set after update");

        let missing = update_last_active(&pool, "nonexistent").await.unwrap();
        assert!(!missing);

        pool.close().await;
    }

    #[tokio::test]
    async fn test_archive_workspace_sets_timestamp() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let ws = sample_workspace("ws-1", 42);
        create_workspace(&pool, &ws).await.unwrap();

        let archived = archive_workspace(&pool, "ws-1").await.unwrap();

        assert_eq!(archived.state, WorkspaceState::Archived);
        assert!(
            archived.worktree_path.is_none(),
            "worktree_path should be cleared on archive"
        );
        assert_ne!(
            archived.updated_at, ws.updated_at,
            "updated_at should be refreshed"
        );

        // Archiving a non-existent workspace returns NotFound
        let err = archive_workspace(&pool, "nonexistent").await;
        assert!(err.is_err());

        pool.close().await;
    }

    #[tokio::test]
    async fn test_add_note() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let ws = sample_workspace("ws-1", 42);
        create_workspace(&pool, &ws).await.unwrap();

        let note = WorkspaceNote {
            id: "wn-1".to_string(),
            workspace_id: "ws-1".to_string(),
            content: "LGTM, ready to merge".to_string(),
            created_at: "2026-03-20T11:00:00Z".to_string(),
        };

        let result = add_note(&pool, &note).await.unwrap();
        assert_eq!(result.id, "wn-1");
        assert_eq!(result.workspace_id, "ws-1");
        assert_eq!(result.content, "LGTM, ready to merge");

        // Duplicate ID should fail
        let dup = add_note(&pool, &note).await;
        assert!(dup.is_err());

        pool.close().await;
    }

    #[tokio::test]
    async fn test_get_notes_ordered() {
        let (pool, _tmp) = test_pool().await;
        upsert_repo(&pool, &sample_repo()).await.unwrap();

        let ws = sample_workspace("ws-1", 42);
        create_workspace(&pool, &ws).await.unwrap();

        let note1 = WorkspaceNote {
            id: "wn-1".to_string(),
            workspace_id: "ws-1".to_string(),
            content: "First note".to_string(),
            created_at: "2026-03-20T10:00:00Z".to_string(),
        };

        let note2 = WorkspaceNote {
            id: "wn-2".to_string(),
            workspace_id: "ws-1".to_string(),
            content: "Second note".to_string(),
            created_at: "2026-03-20T11:00:00Z".to_string(),
        };

        let note3 = WorkspaceNote {
            id: "wn-3".to_string(),
            workspace_id: "ws-1".to_string(),
            content: "Third note".to_string(),
            created_at: "2026-03-20T12:00:00Z".to_string(),
        };

        // Insert in reverse order to verify ordering by created_at ASC
        add_note(&pool, &note3).await.unwrap();
        add_note(&pool, &note1).await.unwrap();
        add_note(&pool, &note2).await.unwrap();

        let notes = get_notes(&pool, "ws-1").await.unwrap();
        assert_eq!(notes.len(), 3);
        assert_eq!(notes[0].id, "wn-1", "oldest first");
        assert_eq!(notes[1].id, "wn-2");
        assert_eq!(notes[2].id, "wn-3", "newest last");

        // No notes for another workspace
        let empty = get_notes(&pool, "nonexistent").await.unwrap();
        assert!(empty.is_empty());

        pool.close().await;
    }

    #[test]
    fn test_unknown_workspace_state_returns_error() {
        assert!(workspace_state_from_str("ACTIVE").is_err(), "wrong case");
        assert!(workspace_state_from_str("bogus").is_err());
        assert!(workspace_state_from_str("").is_err());
    }
}
