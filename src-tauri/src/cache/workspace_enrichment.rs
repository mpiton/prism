use std::path::Path;

use sqlx::SqlitePool;
use tracing::{debug, warn};

use crate::cache::workspaces::list_workspaces;
use crate::error::AppError;
use crate::types::{CiStatus, Workspace, WorkspaceListEntry};
use crate::workspace::worktree::{get_ahead_behind, get_branch_name, get_disk_usage_mb};

/// Row for batch CI status + PR URL lookup by joining workspaces to `pull_requests`.
#[derive(sqlx::FromRow)]
struct CiStatusRow {
    workspace_id: String,
    ci_status: String,
    url: String,
}

/// Row for batch last-note lookup.
#[derive(sqlx::FromRow)]
struct LastNoteRow {
    workspace_id: String,
    content: String,
}

/// Enrich a single workspace with git/filesystem info.
///
/// Best-effort: failures return default values (no branch, 0 ahead/behind, no disk usage).
async fn enrich_workspace_git(ws: &Workspace) -> (Option<String>, u32, u32, Option<u64>) {
    let Some(raw_path) = ws.worktree_path.as_deref() else {
        return (None, 0, 0, None);
    };
    let path = Path::new(raw_path);
    if !path.exists() {
        debug!(workspace_id = %ws.id, "worktree path absent, skipping git enrichment");
        return (None, 0, 0, None);
    }

    let (branch_result, ahead_behind, disk) = futures::join!(
        get_branch_name(path),
        get_ahead_behind(path),
        get_disk_usage_mb(path),
    );

    (branch_result.ok(), ahead_behind.0, ahead_behind.1, disk)
}

/// Assemble enriched workspace list entries by joining DB data with git/filesystem info.
///
/// Best-effort per workspace: a failing git operation does not block the entire query.
pub async fn assemble_workspace_list_entries(
    pool: &SqlitePool,
) -> Result<Vec<WorkspaceListEntry>, AppError> {
    let workspaces = list_workspaces(pool, None).await?;
    if workspaces.is_empty() {
        return Ok(Vec::new());
    }

    // Batch: CI status + PR URL from pull_requests table
    let ci_rows: Vec<CiStatusRow> = sqlx::query_as(
        "SELECT w.id AS workspace_id, p.ci_status, p.url
         FROM workspaces w
         JOIN pull_requests p ON p.repo_id = w.repo_id AND p.number = w.pull_request_number",
    )
    .fetch_all(pool)
    .await?;

    let ci_map: std::collections::HashMap<String, (Option<CiStatus>, String)> = ci_rows
        .into_iter()
        .map(|r| {
            let ci = CiStatus::from_str_opt(&r.ci_status);
            if ci.is_none() {
                warn!(value = r.ci_status, "unrecognised ci_status in DB");
            }
            (r.workspace_id, (ci, r.url))
        })
        .collect();

    // Batch: latest note per workspace (SQLite window function)
    let note_rows: Vec<LastNoteRow> = sqlx::query_as(
        "SELECT workspace_id, content FROM (
           SELECT workspace_id, content,
                  ROW_NUMBER() OVER (PARTITION BY workspace_id ORDER BY created_at DESC, id DESC) AS rn
           FROM workspace_notes
         ) WHERE rn = 1",
    )
    .fetch_all(pool)
    .await?;

    let note_map: std::collections::HashMap<String, String> = note_rows
        .into_iter()
        .map(|r| (r.workspace_id, r.content))
        .collect();

    // Enrich workspaces with git info — parallel per workspace via tokio::join!
    let git_futs: Vec<_> = workspaces.iter().map(enrich_workspace_git).collect();
    let git_results = futures::future::join_all(git_futs).await;

    let entries: Vec<WorkspaceListEntry> = workspaces
        .into_iter()
        .zip(git_results)
        .map(|(ws, (branch, ahead, behind, disk_usage_mb))| {
            let (ci_status, github_url) = ci_map
                .get(&ws.id)
                .map_or((None, None), |(ci, url)| (ci.clone(), Some(url.clone())));
            // Currently 0 or 1 — derived from the single `session_id` column.
            let session_count = u32::from(ws.session_id.is_some());
            let last_note = note_map.get(&ws.id).cloned();

            WorkspaceListEntry {
                workspace: ws,
                branch,
                ahead,
                behind,
                ci_status,
                github_url,
                session_count,
                disk_usage_mb,
                last_note,
            }
        })
        .collect();

    Ok(entries)
}
