use std::path::Path;
use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;
use sqlx::SqlitePool;

use super::pty::PtyManager;
use crate::error::AppError;
use crate::github::client::GitHubClient;
use crate::github::queries::{PullRequestDetail, pull_request_detail};
use crate::types::WorkspaceNote;

/// Matches a Claude Code session ID from stdout.
///
/// Captures the ID token after patterns like:
/// - `Session: <id>`
/// - `session id: <id>`
/// - `Resuming session <id>`
static SESSION_ID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:session(?:\s+id)?)[:\s=]+([a-zA-Z0-9_-]{8,})").unwrap());

/// Matches session errors in Claude Code output (not found, corrupted, etc.).
static SESSION_ERROR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:session\s+(?:not\s+found|expired|invalid|corrupted)|(?:could\s+not|cannot|failed\s+to)\s+(?:find|load|resume)\s+session|no\s+session\s+(?:with|found)|error\s+loading\s+session|\.jsonl.*(?:corrupt|invalid|missing))",
    )
    .unwrap()
});

/// Matches auth / 401 errors in Claude Code output.
static AUTH_ERROR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:\b401\b|\bunauthorized\b|\bauth(?:entication)?\s+(?:error|fail(?:ed)?)\b|\btoken\s+expired\b)",
    )
    .unwrap()
});

/// Extracts a Claude Code session ID from a single stdout line.
///
/// Returns `Some(id)` if a session-id pattern is found, `None` otherwise.
/// Rejects plain words (must contain at least one digit, dash, or underscore).
#[allow(dead_code)]
pub fn detect_session_id(stdout_line: &str) -> Option<String> {
    SESSION_ID_RE
        .captures(stdout_line)
        .map(|caps| caps[1].to_string())
        .filter(|id| {
            id.bytes()
                .any(|b| b == b'-' || b == b'_' || b.is_ascii_digit())
        })
}

/// Detects a session error from a Claude Code stdout line.
///
/// Returns `Some(reason)` if the line indicates the session could not be
/// resumed (not found, corrupted `.jsonl`, expired, etc.), `None` otherwise.
#[allow(dead_code)]
pub fn detect_session_error(stdout_line: &str) -> Option<String> {
    SESSION_ERROR_RE
        .find(stdout_line)
        .map(|m| m.as_str().to_string())
}

/// Returns `true` if the stdout line contains an auth/401 error indicator.
#[allow(dead_code)]
pub fn detect_auth_error(stdout_line: &str) -> bool {
    AUTH_ERROR_RE.is_match(stdout_line)
}

/// Rejects strings containing ASCII control characters (newlines, carriage
/// returns, null bytes, etc.) to prevent PTY command injection.
fn reject_control_chars(value: &str, label: &str) -> Result<(), AppError> {
    if value.chars().any(char::is_control) {
        return Err(AppError::Workspace(format!(
            "{label} must not contain control characters"
        )));
    }
    Ok(())
}

/// Launches Claude Code in the PTY.
///
/// Writes `claude\n` to the PTY's stdin. The caller is responsible for
/// spawning the PTY with `cwd` pointing at the correct worktree.
#[allow(dead_code)]
pub fn launch_claude(pty_manager: &PtyManager, pty_id: &str) -> Result<(), AppError> {
    pty_manager.write_pty(pty_id, b"claude\n")
}

/// Resumes an existing Claude Code session in the PTY.
///
/// Writes `claude --resume <session_id>\n` to the PTY's stdin.
/// Rejects `session_id` values containing control characters.
#[allow(dead_code)]
pub fn resume_claude(
    pty_manager: &PtyManager,
    pty_id: &str,
    session_id: &str,
) -> Result<(), AppError> {
    reject_control_chars(session_id, "session_id")?;
    let cmd = format!("claude --resume {session_id}\n");
    pty_manager.write_pty(pty_id, cmd.as_bytes())
}

/// Renames the current Claude Code session inside the PTY.
///
/// Writes the `/session rename <name>` slash-command to the PTY's stdin.
/// Rejects `name` values containing control characters.
#[allow(dead_code)]
pub fn rename_claude_session(
    pty_manager: &PtyManager,
    pty_id: &str,
    name: &str,
) -> Result<(), AppError> {
    reject_control_chars(name, "session name")?;
    let cmd = format!("/session rename {name}\n");
    pty_manager.write_pty(pty_id, cmd.as_bytes())
}

// ── Session recovery ────────────────────────────────────────────

/// Recovers from a failed `claude --resume` by starting a fresh session.
///
/// Clears the old `session_id` in the database and launches a new Claude
/// instance in the PTY. The caller should watch stdout for the new session
/// ID (via [`detect_session_id`]) and persist it with
/// [`crate::cache::workspaces::update_claude_session`].
#[allow(dead_code)]
pub async fn handle_session_recovery(
    pty_manager: &PtyManager,
    pty_id: &str,
    pool: &SqlitePool,
    workspace_id: &str,
    error_reason: &str,
) -> Result<(), AppError> {
    tracing::warn!(
        workspace_id = workspace_id,
        reason = error_reason,
        "Claude session resume failed, starting new session"
    );

    // Clear the stale session_id so the workspace no longer references
    // the broken session.
    crate::cache::workspaces::update_claude_session(pool, workspace_id, None)
        .await
        .map_err(|e| {
            tracing::error!(workspace_id = workspace_id, error = %e, "failed to clear session during recovery");
            e
        })?;

    // Launch a fresh Claude instance in the PTY.
    launch_claude(pty_manager, pty_id)?;

    Ok(())
}

// ── CLAUDE.md generation ─────────────────────────────────────────

/// Review comment context for CLAUDE.md generation.
#[derive(Debug, Clone)]
pub struct ReviewThreadContext {
    pub path: Option<String>,
    pub comments: Vec<ThreadComment>,
}

/// A single comment within a review thread.
#[derive(Debug, Clone)]
pub struct ThreadComment {
    pub author: String,
    pub body: String,
}

/// A changed file in the PR.
#[derive(Debug, Clone)]
pub struct ChangedFile {
    pub path: String,
    pub additions: i64,
    pub deletions: i64,
}

/// All the PR context needed to render a CLAUDE.md.
#[derive(Debug, Clone)]
pub struct PrContext {
    pub title: String,
    pub number: i64,
    pub body: String,
    pub author: String,
    pub head_branch: String,
    pub base_branch: String,
    pub repo_name: String,
    pub url: String,
    pub reviews: Vec<(String, String)>, // (reviewer, state)
    pub unresolved_threads: Vec<ReviewThreadContext>,
    pub changed_files: Vec<ChangedFile>,
}

/// Renders the CLAUDE.md content from PR context (pure function).
#[allow(dead_code)]
pub fn render_claude_md(ctx: &PrContext) -> String {
    use std::fmt::Write;

    let mut md = String::with_capacity(2048);

    // Header
    let _ = writeln!(md, "# PR #{} — {}", ctx.number, ctx.title);
    md.push('\n');
    let _ = write!(
        md,
        "- **Author**: {}\n- **Branch**: `{}` → `{}`\n- **Repo**: {}\n- **URL**: {}\n",
        ctx.author, ctx.head_branch, ctx.base_branch, ctx.repo_name, ctx.url
    );
    md.push('\n');

    // Description (fenced as untrusted content)
    if !ctx.body.is_empty() {
        md.push_str("## Description\n\n");
        md.push_str("```text\n");
        md.push_str(&escape_fenced_text(&ctx.body));
        md.push_str("\n```\n\n");
    }

    // Reviews
    if !ctx.reviews.is_empty() {
        md.push_str("## Reviews\n\n");
        for (reviewer, state) in &ctx.reviews {
            let _ = writeln!(md, "- **{reviewer}**: {state}");
        }
        md.push('\n');
    }

    // Unresolved review threads (skip empty threads)
    let non_empty_threads: Vec<_> = ctx
        .unresolved_threads
        .iter()
        .filter(|t| !t.comments.is_empty())
        .collect();
    if !non_empty_threads.is_empty() {
        md.push_str("## Unresolved Review Comments\n\n");
        for thread in &non_empty_threads {
            let path = thread.path.as_deref().unwrap_or("(no file)");
            let _ = writeln!(md, "### `{path}`");
            md.push('\n');
            for comment in &thread.comments {
                let _ = writeln!(md, "**{}**:", comment.author);
                md.push_str("```text\n");
                md.push_str(&escape_fenced_text(&comment.body));
                md.push_str("\n```\n\n");
            }
        }
    }

    // Changed files
    if !ctx.changed_files.is_empty() {
        md.push_str("## Changed Files\n\n");
        md.push_str("| File | +/- |\n|------|-----|\n");
        for f in &ctx.changed_files {
            let escaped = escape_table_cell(&f.path);
            let _ = writeln!(md, "| `{escaped}` | +{}/-{} |", f.additions, f.deletions);
        }
        md.push('\n');
    }

    md
}

/// Escapes text for safe inclusion inside a fenced code block.
/// Neutralizes triple backticks that would prematurely close the fence.
fn escape_fenced_text(text: &str) -> String {
    text.replace("```", "` ` `")
}

/// Escapes a string for safe use inside a markdown table cell.
fn escape_table_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

/// Parses `repo_id` ("owner/name") into (owner, name).
fn parse_repo_id(repo_id: &str) -> Result<(&str, &str), AppError> {
    let (owner, name) = repo_id
        .split_once('/')
        .ok_or_else(|| AppError::Workspace("repo_id must be in 'owner/name' format".into()))?;
    if owner.is_empty() || name.is_empty() {
        return Err(AppError::Workspace(
            "repo_id owner and name must not be empty".into(),
        ));
    }
    if name.contains('/') {
        return Err(AppError::Workspace(
            "repo_id must not contain extra path segments".into(),
        ));
    }
    Ok((owner, name))
}

/// Extracts a `PrContext` from the GraphQL response.
fn extract_pr_context(
    data: &pull_request_detail::ResponseData,
    repo_id: &str,
) -> Result<PrContext, AppError> {
    let pr = data
        .repository
        .as_ref()
        .and_then(|r| r.pull_request.as_ref())
        .ok_or_else(|| AppError::NotFound("pull request not found".into()))?;

    let author = pr
        .author
        .as_ref()
        .map_or("unknown".to_string(), |a| a.login.clone());

    let reviews: Vec<(String, String)> = pr
        .reviews
        .as_ref()
        .and_then(|r| r.nodes.as_ref())
        .map(|nodes| {
            nodes
                .iter()
                .flatten()
                .map(|r| {
                    let reviewer = r
                        .author
                        .as_ref()
                        .map_or("unknown".to_string(), |a| a.login.clone());
                    // Debug format matches GraphQL enum variant names (APPROVED, CHANGES_REQUESTED, etc.)
                    let state = format!("{:?}", r.state);
                    (reviewer, state)
                })
                .collect()
        })
        .unwrap_or_default();

    let unresolved_threads: Vec<ReviewThreadContext> = pr
        .review_threads
        .as_ref()
        .and_then(|rt| rt.nodes.as_ref())
        .map(|nodes| {
            nodes
                .iter()
                .flatten()
                .filter(|t| !t.is_resolved)
                .map(|t| {
                    let comments: Vec<ThreadComment> = t
                        .comments
                        .as_ref()
                        .and_then(|c| c.nodes.as_ref())
                        .map(|cn| {
                            cn.iter()
                                .flatten()
                                .map(|c| ThreadComment {
                                    author: c
                                        .author
                                        .as_ref()
                                        .map_or("unknown".to_string(), |a| a.login.clone()),
                                    body: c.body.clone(),
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    ReviewThreadContext {
                        path: t.path.clone(),
                        comments,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let changed_files: Vec<ChangedFile> = pr
        .files
        .as_ref()
        .and_then(|f| f.nodes.as_ref())
        .map(|nodes| {
            nodes
                .iter()
                .flatten()
                .map(|f| ChangedFile {
                    path: f.path.clone(),
                    additions: f.additions,
                    deletions: f.deletions,
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(PrContext {
        title: pr.title.clone(),
        number: pr.number,
        body: pr.body.clone().unwrap_or_default(),
        author,
        head_branch: pr.head_ref_name.clone(),
        base_branch: pr.base_ref_name.clone(),
        repo_name: repo_id.to_string(),
        url: pr.url.clone(),
        reviews,
        unresolved_threads,
        changed_files,
    })
}

// ── Suspension notes ────────────────────────────────────────────

const SUSPENSION_NOTE_TIMEOUT: Duration = Duration::from_secs(30);
const SUSPENSION_NOTE_PROMPT: &str = "Summarize the current work state in 2-3 sentences";

/// Maximum stdout size (16 KiB). Output beyond this is discarded to avoid
/// persisting runaway output. The limit is enforced while streaming — the
/// child is killed as soon as the cap is exceeded.
const MAX_STDOUT_BYTES: usize = 16_384;

/// Runs a command with a timeout, capturing stdout with a size cap.
///
/// Returns trimmed stdout on success, empty string on timeout or error.
/// Streams stdout incrementally and kills the child if output exceeds
/// [`MAX_STDOUT_BYTES`]. On timeout, `kill_on_drop(true)` sends SIGKILL.
///
/// This is the testable building block: tests can pass any command
/// (e.g. `echo`) instead of the real `claude` binary.
pub(crate) async fn run_headless_with_timeout(
    program: &str,
    args: &[&str],
    cwd: &Path,
    timeout: Duration,
) -> String {
    use std::process::Stdio;
    use tokio::io::AsyncReadExt;
    use tokio::process::Command;

    let Ok(mut child) = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
    else {
        return String::new();
    };

    let Some(stdout) = child.stdout.take() else {
        return String::new();
    };

    // Stream stdout with a size cap: read at most MAX_STDOUT_BYTES + 1 to
    // detect overflow, then kill the child immediately if exceeded.
    let read_and_wait = async {
        let mut buf = Vec::with_capacity(MAX_STDOUT_BYTES);
        let mut limited = stdout.take((MAX_STDOUT_BYTES as u64) + 1);
        let n = limited.read_to_end(&mut buf).await?;
        if n > MAX_STDOUT_BYTES {
            return Err(std::io::Error::other("output exceeded size limit"));
        }
        let status = child.wait().await?;
        Ok((buf, status))
    };

    // On timeout the future is dropped → child is dropped → SIGKILL.
    match tokio::time::timeout(timeout, read_and_wait).await {
        Ok(Ok((buf, status))) if status.success() => {
            String::from_utf8_lossy(&buf).trim().to_string()
        }
        _ => String::new(),
    }
}

/// Runs Claude Code headless to generate a workspace suspension note.
///
/// Resumes the given session with `claude --resume <session_id> -p "<prompt>"`
/// so the note reflects the actual suspended conversation, not a fresh context.
/// Returns the note text, or empty string on timeout (30 s) or error.
pub(crate) async fn generate_suspension_note(session_id: &str, worktree_path: &Path) -> String {
    run_headless_with_timeout(
        "claude",
        &[
            "--resume",
            session_id,
            "-p",
            SUSPENSION_NOTE_PROMPT,
            "--output-format",
            "text",
        ],
        worktree_path,
        SUSPENSION_NOTE_TIMEOUT,
    )
    .await
}

/// Stores a suspension note in the database.
///
/// Trims the content and rejects empty/whitespace-only notes with an error.
/// Creates a [`WorkspaceNote`] and persists it via `cache::workspaces::add_note`.
pub(crate) async fn store_suspension_note(
    pool: &SqlitePool,
    workspace_id: &str,
    content: String,
) -> Result<WorkspaceNote, AppError> {
    use chrono::{SecondsFormat, Utc};

    let trimmed = content.trim().to_string();
    if trimmed.is_empty() {
        return Err(AppError::Workspace(
            "suspension note content must not be empty".into(),
        ));
    }

    let note = WorkspaceNote {
        id: uuid::Uuid::new_v4().to_string(),
        workspace_id: workspace_id.to_string(),
        content: trimmed,
        created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
    };

    crate::cache::workspaces::add_note(pool, &note).await
}

/// Generates a suspension note and stores it in the database.
///
/// Only runs if `session_id` is `Some` (Claude Code was active in this
/// workspace). Returns `None` if Claude wasn't active or if the headless
/// command failed / timed out (empty output).
pub async fn create_suspension_note(
    pool: &SqlitePool,
    workspace_id: &str,
    worktree_path: &Path,
    session_id: Option<&str>,
) -> Result<Option<WorkspaceNote>, AppError> {
    let Some(sid) = session_id else {
        return Ok(None);
    };

    let content = generate_suspension_note(sid, worktree_path).await;
    if content.is_empty() {
        return Ok(None);
    }

    store_suspension_note(pool, workspace_id, content)
        .await
        .map(Some)
}

/// Fetches PR details from GitHub and generates a CLAUDE.md in the worktree.
///
/// `repo_id` must be in "owner/name" format (e.g. "mpiton/prism").
#[allow(dead_code)]
pub async fn generate_claude_md(
    client: &GitHubClient,
    repo_id: &str,
    pr_number: i64,
    worktree_path: &Path,
) -> Result<(), AppError> {
    let (owner, name) = parse_repo_id(repo_id)?;

    let variables = pull_request_detail::Variables {
        owner: owner.to_string(),
        name: name.to_string(),
        number: pr_number,
    };

    let data = client
        .execute_graphql::<PullRequestDetail>(variables)
        .await?;

    let ctx = extract_pr_context(&data, repo_id)?;
    let content = render_claude_md(&ctx);

    let file_path = worktree_path.join("CLAUDE.md");
    tokio::fs::write(&file_path, content.as_bytes())
        .await
        .map_err(AppError::Io)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    // ── Pure-function tests ───────────────────────────────────────────

    #[test]
    fn test_detect_session_id_from_output() {
        // Standard UUID format
        assert_eq!(
            detect_session_id("Session: a1b2c3d4-e5f6-7890-abcd-ef1234567890"),
            Some("a1b2c3d4-e5f6-7890-abcd-ef1234567890".into())
        );

        // Case-insensitive
        assert_eq!(
            detect_session_id("session id: my-session-id-12345678"),
            Some("my-session-id-12345678".into())
        );

        // With equals sign
        assert_eq!(
            detect_session_id("SESSION=abcdef12-3456-7890-abcd-ef1234567890"),
            Some("abcdef12-3456-7890-abcd-ef1234567890".into())
        );

        // Embedded in other text
        assert_eq!(
            detect_session_id("  Resuming session: prism-pr-42  "),
            Some("prism-pr-42".into())
        );
    }

    #[test]
    fn test_detect_session_id_no_match() {
        assert_eq!(detect_session_id("Hello, world!"), None);
        assert_eq!(detect_session_id(""), None);
        assert_eq!(detect_session_id("No session here"), None);
        // Too short — fewer than 8 chars after "session:"
        assert_eq!(detect_session_id("session: abc"), None);
        // Plain words rejected — must contain a digit, dash, or underscore
        assert_eq!(detect_session_id("session: completed"), None);
        assert_eq!(detect_session_id("session: established"), None);
    }

    #[test]
    fn test_detect_auth_error() {
        assert!(detect_auth_error("Error: 401 Unauthorized"));
        assert!(detect_auth_error("authentication error: token invalid"));
        assert!(detect_auth_error("auth fail: please re-authenticate"));
        assert!(detect_auth_error("Your token expired, please login again"));
        assert!(detect_auth_error("HTTP 401 — unauthorized"));

        // Non-auth messages
        assert!(!detect_auth_error("Connection established"));
        assert!(!detect_auth_error("Session started successfully"));
        assert!(!detect_auth_error(""));
        // "401" embedded inside a token — must not match
        assert!(!detect_auth_error("Session: abc401def"));
    }

    #[test]
    fn test_reject_control_chars() {
        assert!(reject_control_chars("valid-name", "test").is_ok());
        assert!(reject_control_chars("also valid 123", "test").is_ok());

        let err = reject_control_chars("has\nnewline", "field").unwrap_err();
        assert!(err.to_string().contains("control characters"));

        assert!(reject_control_chars("has\rreturn", "field").is_err());
        assert!(reject_control_chars("has\0null", "field").is_err());
    }

    // ── PTY integration tests ─────────────────────────────────────────

    /// Helper: spawns a PTY in a temp dir and returns (pty_id, output_receiver).
    fn spawn_test_pty(manager: &PtyManager) -> (String, mpsc::Receiver<Vec<u8>>) {
        let tmp = std::env::temp_dir();
        let (tx, rx) = mpsc::channel();

        let pty_id = manager
            .spawn(&tmp, 80, 24, move |_id, data| {
                let _ = tx.send(data.to_vec());
            })
            .expect("spawn should succeed");

        (pty_id, rx)
    }

    #[tokio::test]
    async fn test_launch_claude_command() {
        let manager = PtyManager::new();
        let (pty_id, rx) = spawn_test_pty(&manager);

        // Give the shell time to start.
        tokio::time::sleep(Duration::from_millis(200)).await;

        let result = launch_claude(&manager, &pty_id);
        assert!(result.is_ok(), "launch_claude should succeed: {result:?}");

        // The command "claude\n" should appear in the PTY output (echoed by shell).
        let mut output = String::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            if let Ok(data) = rx.recv_timeout(Duration::from_millis(100)) {
                output.push_str(&String::from_utf8_lossy(&data));
                if output.contains("claude") {
                    break;
                }
            }
        }
        assert!(
            output.contains("claude"),
            "PTY output should contain 'claude', got: {output:?}"
        );

        manager.kill(&pty_id).unwrap();
    }

    #[tokio::test]
    async fn test_resume_claude_command() {
        let manager = PtyManager::new();
        let (pty_id, rx) = spawn_test_pty(&manager);

        tokio::time::sleep(Duration::from_millis(200)).await;

        let session_id = "test-session-12345678";
        let result = resume_claude(&manager, &pty_id, session_id);
        assert!(result.is_ok(), "resume_claude should succeed: {result:?}");

        let mut output = String::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            if let Ok(data) = rx.recv_timeout(Duration::from_millis(100)) {
                output.push_str(&String::from_utf8_lossy(&data));
                if output.contains("--resume") && output.contains(session_id) {
                    break;
                }
            }
        }
        assert!(
            output.contains("--resume") && output.contains(session_id),
            "PTY output should contain 'claude --resume {session_id}', got: {output:?}"
        );

        manager.kill(&pty_id).unwrap();
    }

    #[tokio::test]
    async fn test_resume_claude_rejects_newline() {
        let manager = PtyManager::new();
        let (pty_id, _rx) = spawn_test_pty(&manager);

        tokio::time::sleep(Duration::from_millis(200)).await;

        let result = resume_claude(&manager, &pty_id, "legit-id\nrm -rf /");
        assert!(result.is_err(), "should reject session_id with newline");

        manager.kill(&pty_id).unwrap();
    }

    #[tokio::test]
    async fn test_rename_claude_session_command() {
        let manager = PtyManager::new();
        let (pty_id, rx) = spawn_test_pty(&manager);

        tokio::time::sleep(Duration::from_millis(200)).await;

        let result = rename_claude_session(&manager, &pty_id, "prism-pr-42");
        assert!(
            result.is_ok(),
            "rename_claude_session should succeed: {result:?}"
        );

        let mut output = String::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            if let Ok(data) = rx.recv_timeout(Duration::from_millis(100)) {
                output.push_str(&String::from_utf8_lossy(&data));
                if output.contains("prism-pr-42") {
                    break;
                }
            }
        }
        assert!(
            output.contains("prism-pr-42"),
            "PTY output should contain session name, got: {output:?}"
        );

        manager.kill(&pty_id).unwrap();
    }

    #[tokio::test]
    async fn test_rename_claude_session_rejects_newline() {
        let manager = PtyManager::new();
        let (pty_id, _rx) = spawn_test_pty(&manager);

        tokio::time::sleep(Duration::from_millis(200)).await;

        let result = rename_claude_session(&manager, &pty_id, "my-feature\nrm -rf /");
        assert!(result.is_err(), "should reject name with newline");

        manager.kill(&pty_id).unwrap();
    }

    // ── CLAUDE.md generation tests ───────────────────────────────────

    fn sample_pr_context() -> PrContext {
        PrContext {
            title: "feat: add user authentication".into(),
            number: 42,
            body: "This PR adds OAuth2 authentication.\n\n## Changes\n- Added auth middleware\n- Added login page".into(),
            author: "octocat".into(),
            head_branch: "feat/auth".into(),
            base_branch: "main".into(),
            repo_name: "mpiton/prism".into(),
            url: "https://github.com/mpiton/prism/pull/42".into(),
            reviews: vec![
                ("alice".into(), "APPROVED".into()),
                ("bob".into(), "CHANGES_REQUESTED".into()),
            ],
            unresolved_threads: vec![],
            changed_files: vec![
                ChangedFile { path: "src/auth/mod.rs".into(), additions: 120, deletions: 5 },
                ChangedFile { path: "src/auth/middleware.rs".into(), additions: 80, deletions: 0 },
                ChangedFile { path: "src/main.rs".into(), additions: 3, deletions: 1 },
            ],
        }
    }

    #[test]
    fn test_generate_claude_md_content() {
        let ctx = sample_pr_context();
        let md = render_claude_md(&ctx);

        // Title and PR number
        assert!(
            md.contains("feat: add user authentication"),
            "should contain PR title"
        );
        assert!(md.contains("#42"), "should contain PR number");

        // PR body is fenced
        assert!(
            md.contains("OAuth2 authentication"),
            "should contain PR body"
        );
        assert!(
            md.contains("```text\nThis PR adds OAuth2"),
            "body should be inside a fenced block"
        );

        // Branch info
        assert!(md.contains("feat/auth"), "should contain head branch");
        assert!(md.contains("main"), "should contain base branch");

        // Changed files
        assert!(md.contains("src/auth/mod.rs"), "should list changed files");
        assert!(
            md.contains("src/auth/middleware.rs"),
            "should list changed files"
        );
        assert!(md.contains("src/main.rs"), "should list changed files");

        // Author
        assert!(md.contains("octocat"), "should contain author");

        // URL
        assert!(
            md.contains("https://github.com/mpiton/prism/pull/42"),
            "should contain URL"
        );
    }

    #[test]
    fn test_generate_claude_md_with_reviews() {
        let mut ctx = sample_pr_context();
        ctx.unresolved_threads = vec![
            ReviewThreadContext {
                path: Some("src/auth/mod.rs".into()),
                comments: vec![
                    ThreadComment {
                        author: "bob".into(),
                        body: "This needs error handling for expired tokens.".into(),
                    },
                    ThreadComment {
                        author: "octocat".into(),
                        body: "Good point, will fix.".into(),
                    },
                ],
            },
            ReviewThreadContext {
                path: Some("src/main.rs".into()),
                comments: vec![ThreadComment {
                    author: "alice".into(),
                    body: "Consider using middleware instead of manual check.".into(),
                }],
            },
        ];

        let md = render_claude_md(&ctx);

        // Review states
        assert!(
            md.contains("alice") && md.contains("APPROVED"),
            "should show alice's approval"
        );
        assert!(
            md.contains("bob") && md.contains("CHANGES_REQUESTED"),
            "should show bob's changes requested"
        );

        // Unresolved threads — comments are fenced
        assert!(
            md.contains("error handling for expired tokens"),
            "should contain unresolved comment body"
        );
        assert!(
            md.contains("```text\nThis needs error handling"),
            "comment body should be inside a fenced block"
        );
        assert!(
            md.contains("src/auth/mod.rs"),
            "should contain thread file path"
        );
        assert!(
            md.contains("Consider using middleware"),
            "should contain second thread comment"
        );
    }

    #[test]
    fn test_parse_repo_id_valid() {
        let (owner, name) = parse_repo_id("mpiton/prism").unwrap();
        assert_eq!(owner, "mpiton");
        assert_eq!(name, "prism");
    }

    #[test]
    fn test_parse_repo_id_invalid() {
        assert!(parse_repo_id("no-slash").is_err());
        assert!(parse_repo_id("/name").is_err());
        assert!(parse_repo_id("owner/").is_err());
        assert!(parse_repo_id("").is_err());
        assert!(parse_repo_id("owner/name/extra").is_err());
    }

    #[test]
    fn test_generate_claude_md_empty_body() {
        let mut ctx = sample_pr_context();
        ctx.body = String::new();
        ctx.reviews = vec![];
        ctx.unresolved_threads = vec![];
        ctx.changed_files = vec![];

        let md = render_claude_md(&ctx);

        // Should still have title and basic structure
        assert!(
            md.contains("feat: add user authentication"),
            "should still contain title"
        );
        assert!(md.contains("#42"), "should still contain PR number");

        // Should not have empty sections that look broken
        assert!(
            !md.contains("##\n\n##"),
            "should not have empty section headers back-to-back"
        );
    }

    #[test]
    fn test_render_skips_empty_threads() {
        let mut ctx = sample_pr_context();
        ctx.unresolved_threads = vec![
            ReviewThreadContext {
                path: Some("src/empty.rs".into()),
                comments: vec![], // empty — should be skipped
            },
            ReviewThreadContext {
                path: Some("src/real.rs".into()),
                comments: vec![ThreadComment {
                    author: "reviewer".into(),
                    body: "Fix this.".into(),
                }],
            },
        ];

        let md = render_claude_md(&ctx);
        assert!(
            md.contains("src/real.rs"),
            "should include non-empty thread"
        );
        assert!(
            !md.contains("src/empty.rs"),
            "should skip empty comment thread"
        );
    }

    #[test]
    fn test_render_escapes_backticks_in_body() {
        let mut ctx = sample_pr_context();
        ctx.body = "Before ```rust\nlet x = 1;\n``` after".into();

        let md = render_claude_md(&ctx);
        assert!(
            !md.contains("```rust"),
            "triple backticks in body should be escaped"
        );
        assert!(md.contains("` ` `rust"), "escaped backticks should appear");
    }

    #[test]
    fn test_render_escapes_pipe_in_file_path() {
        let mut ctx = sample_pr_context();
        ctx.changed_files = vec![ChangedFile {
            path: "src/a|b.rs".into(),
            additions: 1,
            deletions: 0,
        }];

        let md = render_claude_md(&ctx);
        assert!(
            md.contains("src/a\\|b.rs"),
            "pipe in path should be escaped"
        );
        // Table should still have correct column count
        let table_row = md
            .lines()
            .find(|l| l.contains("a\\|b.rs"))
            .expect("should find escaped path line");
        assert_eq!(
            table_row.matches('|').count(),
            4,
            "table row should have 4 pipe delimiters"
        );
    }

    // ── Suspension note tests ────────────────────────────────────────

    #[tokio::test]
    async fn test_generate_note_success() {
        let tmp = tempfile::TempDir::new().unwrap();
        let content = run_headless_with_timeout(
            "echo",
            &["Working on auth refactor. Tests passing."],
            tmp.path(),
            Duration::from_secs(5),
        )
        .await;

        assert_eq!(content, "Working on auth refactor. Tests passing.");
    }

    #[tokio::test]
    async fn test_generate_note_timeout() {
        let tmp = tempfile::TempDir::new().unwrap();
        let content =
            run_headless_with_timeout("sleep", &["60"], tmp.path(), Duration::from_secs(1)).await;

        assert!(
            content.is_empty(),
            "should return empty on timeout, got: {content:?}"
        );
    }

    #[tokio::test]
    async fn test_generate_note_command_failure() {
        let tmp = tempfile::TempDir::new().unwrap();
        let content = run_headless_with_timeout(
            "false", // exits with status 1
            &[],
            tmp.path(),
            Duration::from_secs(5),
        )
        .await;

        assert!(content.is_empty(), "should return empty on non-zero exit");
    }

    #[tokio::test]
    async fn test_generate_note_stored_in_db() {
        use crate::cache::db::init_db;
        use crate::cache::repos::upsert_repo;
        use crate::cache::workspaces::{create_workspace, get_notes};
        use crate::types::{Repo, Workspace, WorkspaceState};

        let tmp = tempfile::TempDir::new().unwrap();
        let pool = init_db(&tmp.path().join("test.db")).await.unwrap();

        let repo = Repo {
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
        };
        upsert_repo(&pool, &repo).await.unwrap();

        let ws = Workspace {
            id: "ws-1".to_string(),
            repo_id: "repo-1".to_string(),
            pull_request_number: 42,
            state: WorkspaceState::Active,
            worktree_path: Some("/tmp/worktree".to_string()),
            session_id: Some("session-123".to_string()),
            created_at: "2026-03-20T10:00:00Z".to_string(),
            updated_at: "2026-03-20T10:00:00Z".to_string(),
        };
        create_workspace(&pool, &ws).await.unwrap();

        // Store a note
        let note = store_suspension_note(
            &pool,
            "ws-1",
            "Working on auth refactor. Tests passing.".to_string(),
        )
        .await
        .unwrap();

        assert_eq!(note.workspace_id, "ws-1");
        assert_eq!(note.content, "Working on auth refactor. Tests passing.");
        assert!(!note.id.is_empty());
        assert!(!note.created_at.is_empty());

        // Verify it's retrievable
        let notes = get_notes(&pool, "ws-1").await.unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].content, "Working on auth refactor. Tests passing.");

        pool.close().await;
    }

    #[tokio::test]
    async fn test_create_suspension_note_skips_without_session() {
        use crate::cache::db::init_db;
        use crate::cache::repos::upsert_repo;
        use crate::cache::workspaces::{create_workspace, get_notes};
        use crate::types::{Repo, Workspace, WorkspaceState};

        let tmp = tempfile::TempDir::new().unwrap();
        let pool = init_db(&tmp.path().join("test.db")).await.unwrap();

        let repo = Repo {
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
        };
        upsert_repo(&pool, &repo).await.unwrap();

        let ws = Workspace {
            id: "ws-1".to_string(),
            repo_id: "repo-1".to_string(),
            pull_request_number: 42,
            state: WorkspaceState::Active,
            worktree_path: Some("/tmp/worktree".to_string()),
            session_id: None,
            created_at: "2026-03-20T10:00:00Z".to_string(),
            updated_at: "2026-03-20T10:00:00Z".to_string(),
        };
        create_workspace(&pool, &ws).await.unwrap();

        // No Claude session → should return None, no note stored
        let result = create_suspension_note(&pool, "ws-1", tmp.path(), None)
            .await
            .unwrap();
        assert!(
            result.is_none(),
            "should return None without active session"
        );

        let notes = get_notes(&pool, "ws-1").await.unwrap();
        assert!(notes.is_empty(), "no note should be stored");

        pool.close().await;
    }

    #[tokio::test]
    async fn test_store_suspension_note_rejects_empty() {
        use crate::cache::db::init_db;
        use crate::cache::repos::upsert_repo;
        use crate::cache::workspaces::create_workspace;
        use crate::types::{Repo, Workspace, WorkspaceState};

        let tmp = tempfile::TempDir::new().unwrap();
        let pool = init_db(&tmp.path().join("test.db")).await.unwrap();

        let repo = Repo {
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
        };
        upsert_repo(&pool, &repo).await.unwrap();

        let ws = Workspace {
            id: "ws-1".to_string(),
            repo_id: "repo-1".to_string(),
            pull_request_number: 42,
            state: WorkspaceState::Active,
            worktree_path: Some("/tmp/worktree".to_string()),
            session_id: None,
            created_at: "2026-03-20T10:00:00Z".to_string(),
            updated_at: "2026-03-20T10:00:00Z".to_string(),
        };
        create_workspace(&pool, &ws).await.unwrap();

        // Empty content should be rejected
        let err = store_suspension_note(&pool, "ws-1", String::new()).await;
        assert!(err.is_err(), "should reject empty content");

        // Whitespace-only content should be rejected
        let err = store_suspension_note(&pool, "ws-1", "   \n  ".to_string()).await;
        assert!(err.is_err(), "should reject whitespace-only content");

        pool.close().await;
    }

    // ── Session recovery tests ──────────────────────────────────────

    #[test]
    fn test_resume_session_not_found() {
        // Session-not-found patterns
        assert!(
            detect_session_error("Error: session not found").is_some(),
            "should detect 'session not found'"
        );
        assert!(
            detect_session_error("Could not find session abc-123").is_some(),
            "should detect 'could not find session'"
        );
        assert!(
            detect_session_error("Failed to resume session").is_some(),
            "should detect 'failed to resume session'"
        );
        assert!(
            detect_session_error("Session expired, please start a new one").is_some(),
            "should detect 'session expired'"
        );
        assert!(
            detect_session_error("Cannot find session with that ID").is_some(),
            "should detect 'cannot find session'"
        );
        assert!(
            detect_session_error("Error loading session data").is_some(),
            "should detect 'error loading session'"
        );

        // Corrupted session patterns
        assert!(
            detect_session_error("Session corrupted").is_some(),
            "should detect 'session corrupted'"
        );
        assert!(
            detect_session_error("session.jsonl is corrupt or invalid").is_some(),
            "should detect '.jsonl corrupt'"
        );
        assert!(
            detect_session_error("data.jsonl file missing or inaccessible").is_some(),
            "should detect '.jsonl missing'"
        );
        assert!(
            detect_session_error("Session invalid, cannot resume").is_some(),
            "should detect 'session invalid'"
        );

        // Normal output — should NOT trigger
        assert!(
            detect_session_error("Session started successfully").is_none(),
            "should not match normal session start"
        );
        assert!(
            detect_session_error("Resuming session abc-123").is_none(),
            "should not match successful resume"
        );
        assert!(
            detect_session_error("Session: a1b2c3d4").is_none(),
            "should not match session ID display"
        );
        assert!(
            detect_session_error("").is_none(),
            "should not match empty string"
        );
        assert!(
            detect_session_error("Hello, world!").is_none(),
            "should not match unrelated output"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_fallback_new_session() {
        use crate::cache::db::init_db;
        use crate::cache::repos::upsert_repo;
        use crate::cache::workspaces::{create_workspace, get_workspace};
        use crate::types::{Repo, Workspace, WorkspaceState};

        // Setup: DB with a workspace that has a stale session_id
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = init_db(&tmp.path().join("test.db")).await.unwrap();

        let repo = Repo {
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
        };
        upsert_repo(&pool, &repo).await.unwrap();

        let ws = Workspace {
            id: "ws-recover".to_string(),
            repo_id: "repo-1".to_string(),
            pull_request_number: 99,
            state: WorkspaceState::Active,
            worktree_path: Some("/tmp/worktree".to_string()),
            session_id: Some("old-broken-session".to_string()),
            created_at: "2026-03-20T10:00:00Z".to_string(),
            updated_at: "2026-03-20T10:00:00Z".to_string(),
        };
        create_workspace(&pool, &ws).await.unwrap();

        // Verify the workspace starts with a session_id
        let before = get_workspace(&pool, "ws-recover")
            .await
            .unwrap()
            .expect("workspace should exist");
        assert_eq!(
            before.session_id.as_deref(),
            Some("old-broken-session"),
            "workspace should start with a stale session_id"
        );

        // Setup: PTY for handle_session_recovery to write to
        let pty_manager = PtyManager::new();
        let (pty_id, _rx) = spawn_test_pty(&pty_manager);
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Act: call handle_session_recovery end-to-end
        let result = handle_session_recovery(
            &pty_manager,
            &pty_id,
            &pool,
            "ws-recover",
            "session not found",
        )
        .await;
        assert!(result.is_ok(), "recovery should succeed: {result:?}");

        // Verify: session_id was cleared in DB
        let after = get_workspace(&pool, "ws-recover")
            .await
            .unwrap()
            .expect("workspace should exist");
        assert!(
            after.session_id.is_none(),
            "session_id should be cleared after recovery"
        );

        // PTY output ("claude\n") is verified by test_launch_claude_command.

        pty_manager.kill(&pty_id).unwrap();
        pool.close().await;
    }
}
