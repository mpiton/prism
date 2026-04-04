use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::process::Command;
use tokio::time::timeout;
use tracing::warn;

use crate::error::AppError;

/// Git operations timeout (covers slow fetches on large repos).
const GIT_TIMEOUT: Duration = Duration::from_secs(120);

/// Returns the default base directory for `PRism` workspaces: `~/.prism/workspaces`.
#[allow(dead_code)]
pub fn default_base_dir() -> Result<PathBuf, AppError> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| {
            AppError::Workspace("HOME or USERPROFILE environment variable must be set".into())
        })?;
    Ok(home.join(".prism").join("workspaces"))
}

/// Constructs the worktree directory path for a specific PR.
///
/// Format: `{base_dir}/{repo_name}/worktrees/pr-{pr_number}`
///
/// Rejects `repo_name` values that are not a single normal path component
/// (blocks `..`, `.`, `/`, `\`, absolute paths, and Windows drive prefixes).
#[allow(dead_code)]
pub fn build_worktree_path(
    base_dir: &Path,
    repo_name: &str,
    pr_number: u32,
) -> Result<PathBuf, AppError> {
    let mut components = Path::new(repo_name).components();
    let is_single_normal = matches!(components.next(), Some(std::path::Component::Normal(_)))
        && components.next().is_none();

    // Also reject backslashes explicitly: on Unix they are valid filename chars
    // but would produce incorrect paths on Windows.
    if !is_single_normal || repo_name.contains('/') || repo_name.contains('\\') {
        return Err(AppError::Workspace(format!(
            "invalid repo_name: {repo_name:?} (must be a single normal path component)"
        )));
    }

    Ok(base_dir
        .join(repo_name)
        .join("worktrees")
        .join(format!("pr-{pr_number}")))
}

/// Classifies git stderr output into a specific, user-friendly [`AppError`].
///
/// Parses known error patterns from git output and returns a descriptive error
/// instead of raw stderr. Unknown patterns fall back to a generic git error.
fn classify_git_error(stderr: &str, args_display: &str) -> AppError {
    let stderr_lower = stderr.to_lowercase();

    // Branch not found (during fetch) — handles English and French git locales.
    if stderr_lower.contains("couldn't find remote ref")
        || stderr_lower.contains("remote ref does not exist")
        || stderr_lower.contains("impossible de trouver la r\u{00e9}f\u{00e9}rence distante")
    {
        // Extract the ref name from the last word of the matching line.
        let branch = stderr
            .lines()
            .find(|l| {
                let low = l.to_lowercase();
                low.contains("remote ref") || low.contains("r\u{00e9}f\u{00e9}rence distante")
            })
            .and_then(|l| l.rsplit_once(' '))
            .map_or("unknown", |(_, name)| name.trim());
        return AppError::Git(format!(
            "Branch '{branch}' not found on remote. Verify the branch exists and try again."
        ));
    }

    // Permission denied — English and French
    if stderr_lower.contains("permission denied") || stderr_lower.contains("permission non accord")
    {
        warn!(stderr = stderr.trim(), "git permission denied");
        return AppError::Git(
            "Permission denied during git operation. Check file and directory permissions.".into(),
        );
    }

    // Worktree already checked out / locked — English and French
    if stderr_lower.contains("is already checked out")
        || stderr_lower.contains("already locked")
        || stderr_lower.contains("est d\u{00e9}j\u{00e0}")
    {
        warn!(stderr = stderr.trim(), "git worktree already in use");
        return AppError::Workspace("Worktree is already in use by another working copy.".into());
    }

    // Not a git repository — English and French
    if stderr_lower.contains("not a git repository")
        || stderr_lower.contains("n'est un d\u{00e9}p\u{00f4}t git")
        || stderr_lower.contains("pas un d\u{00e9}p\u{00f4}t git")
    {
        return AppError::Git(
            "Path is not a valid git repository. Check the repository configuration.".into(),
        );
    }

    // Default: generic git error — log stderr for debugging, keep user message clean.
    warn!(
        command = args_display,
        stderr = stderr.trim(),
        "git command failed"
    );
    AppError::Git(format!(
        "git {args_display} failed. Check the logs for details."
    ))
}

/// Clones a GitHub repository into `{base_dir}/{repo_name}`.
///
/// Returns the path to the cloned repository.
/// Times out after [`GIT_TIMEOUT`] (120s — enough for most repos).
pub async fn clone_repo(
    repo_url: &str,
    repo_name: &str,
    base_dir: &Path,
) -> Result<PathBuf, AppError> {
    let clone_path = base_dir.join(repo_name);

    if clone_path.exists() {
        // Already cloned — just fetch latest
        tracing::info!("repo already cloned at {}, fetching", clone_path.display());
        run_git(&["fetch".into(), "--all".into()], &clone_path).await?;
        return Ok(clone_path);
    }

    if let Some(parent) = clone_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| AppError::Workspace(format!("failed to create clone dir: {e}")))?;
    }

    tracing::info!("cloning {} into {}", repo_url, clone_path.display());

    let clone_url = if repo_url.starts_with("http") {
        repo_url.to_string()
    } else {
        format!("https://github.com/{repo_url}.git")
    };

    run_git(
        &[
            "clone".into(),
            clone_url.into(),
            clone_path.as_os_str().to_os_string(),
        ],
        base_dir,
    )
    .await?;

    Ok(clone_path)
}

/// Runs a git command in the given directory and returns stdout on success.
///
/// Times out after [`GIT_TIMEOUT`] to prevent indefinite hangs on network operations.
/// The spawned child process is killed when the timeout fires (`kill_on_drop`).
/// Accepts [`OsString`] args so paths with non-UTF-8 bytes are passed verbatim to git.
pub(crate) async fn run_git(args: &[OsString], cwd: &Path) -> Result<String, AppError> {
    let cmd_label = args
        .first()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    let mut cmd = Command::new("git");
    cmd.kill_on_drop(true).args(args).current_dir(cwd);

    let output = timeout(GIT_TIMEOUT, cmd.output())
        .await
        .map_err(|_| {
            AppError::Git(format!(
                "git {cmd_label} timed out after {}s",
                GIT_TIMEOUT.as_secs()
            ))
        })?
        .map_err(|e| AppError::Git(format!("failed to run git {cmd_label}: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let args_display = args
            .iter()
            .map(|s| s.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ");
        return Err(classify_git_error(&stderr, &args_display));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Returns the current branch name for a worktree directory.
///
/// Returns `"HEAD"` (literal) for detached HEAD states (git's native behavior).
/// Returns `Err` if the path is not a git repository or `run_git` fails.
pub(crate) async fn get_branch_name(worktree_path: &Path) -> Result<String, AppError> {
    let output = run_git(
        &["rev-parse".into(), "--abbrev-ref".into(), "HEAD".into()],
        worktree_path,
    )
    .await?;
    Ok(output.trim().to_string())
}

/// Returns (ahead, behind) counts relative to the upstream tracking branch.
///
/// Returns `(0, 0)` when there is no upstream (detached HEAD, no tracking branch).
pub(crate) async fn get_ahead_behind(worktree_path: &Path) -> (u32, u32) {
    let result = run_git(
        &[
            "rev-list".into(),
            "--count".into(),
            "--left-right".into(),
            "HEAD...@{upstream}".into(),
        ],
        worktree_path,
    )
    .await;

    match result {
        Ok(output) => {
            let parts: Vec<&str> = output.trim().split('\t').collect();
            if parts.len() == 2 {
                let ahead = parts[0].parse().unwrap_or(0);
                let behind = parts[1].parse().unwrap_or(0);
                (ahead, behind)
            } else {
                (0, 0)
            }
        }
        Err(_) => (0, 0),
    }
}

/// Best-effort disk usage of a worktree in megabytes.
///
/// Uses `du -sk` on Unix. Returns `None` on Windows or on any error.
pub(crate) async fn get_disk_usage_mb(worktree_path: &Path) -> Option<u64> {
    #[cfg(not(windows))]
    {
        const DU_TIMEOUT: Duration = Duration::from_secs(10);

        let mut cmd = Command::new("du");
        cmd.args(["-sk"]).arg(worktree_path).kill_on_drop(true);

        let output = timeout(DU_TIMEOUT, cmd.output()).await.ok()?.ok()?;
        if !output.status.success() {
            return None;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let kb: u64 = stdout.split_whitespace().next()?.parse().ok()?;
        Some(kb / 1024)
    }
    #[cfg(windows)]
    {
        let _ = worktree_path;
        None
    }
}

/// Creates a git worktree for reviewing a pull request.
///
/// 1. Fetches the branch from origin
/// 2. Creates a worktree at `{base_dir}/{repo_name}/worktrees/pr-{pr_number}/`
///
/// Returns the path to the created worktree directory.
///
/// Note: parent directories created before the git commands are not removed on
/// failure. Retries are not blocked (the existence check only tests the final
/// worktree path), but stale intermediate directories may accumulate on repeated
/// failures.
#[allow(dead_code)]
pub async fn create_worktree(
    repo_local_path: &Path,
    branch: &str,
    pr_number: u32,
    repo_name: &str,
    base_dir: &Path,
) -> Result<PathBuf, AppError> {
    // Validate that repo_local_path exists and is a git repository.
    // Use tokio::fs::metadata to distinguish NotFound from PermissionDenied.
    match tokio::fs::metadata(repo_local_path).await {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => {
            return Err(AppError::Git(format!(
                "Repository path is not a directory: {}",
                repo_local_path.display()
            )));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(AppError::Git(format!(
                "Repository path does not exist: {}",
                repo_local_path.display()
            )));
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            return Err(AppError::Git(format!(
                "Permission denied while accessing repository path: {}",
                repo_local_path.display()
            )));
        }
        Err(e) => {
            return Err(AppError::Git(format!(
                "Failed to access repository path {}: {e}",
                repo_local_path.display()
            )));
        }
    }

    // Use `git rev-parse --git-dir` to validate the repo. This works for both
    // regular and bare repositories (bare repos have no `.git` subdirectory).
    // Let the classified error from run_git propagate (timeout, permission, etc.).
    run_git(&["rev-parse".into(), "--git-dir".into()], repo_local_path).await?;

    let wt_path = build_worktree_path(base_dir, repo_name, pr_number)?;

    if wt_path.exists() {
        return Err(AppError::Workspace(format!(
            "worktree already exists at {}",
            wt_path.display()
        )));
    }

    // Ensure parent directories exist before running git commands.
    if let Some(parent) = wt_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                AppError::Workspace(format!(
                    "Permission denied: cannot create worktree directory at {}. Check folder permissions.",
                    parent.display()
                ))
            } else {
                AppError::Workspace(format!("failed to create worktree directory: {e}"))
            }
        })?;
    }

    // Fetch the branch from origin. The `--` separator prevents branch names
    // that start with `-` from being misinterpreted as git flags.
    run_git(
        &["fetch".into(), "origin".into(), "--".into(), branch.into()],
        repo_local_path,
    )
    .await?;

    // Create the worktree at the computed path (detached HEAD on the remote branch).
    run_git(
        &[
            "worktree".into(),
            "add".into(),
            wt_path.as_os_str().into(),
            format!("origin/{branch}").into(),
        ],
        repo_local_path,
    )
    .await?;

    Ok(wt_path)
}

/// Removes a git worktree forcefully.
///
/// Uses `git worktree remove --force` to handle dirty worktrees.
#[allow(dead_code)]
pub async fn remove_worktree(repo_local_path: &Path, worktree_path: &Path) -> Result<(), AppError> {
    run_git(
        &[
            "worktree".into(),
            "remove".into(),
            "--force".into(),
            worktree_path.as_os_str().into(),
        ],
        repo_local_path,
    )
    .await?;
    Ok(())
}

/// Lists all worktree paths for a repository (excluding the main working tree).
///
/// Parses the output of `git worktree list --porcelain`.
#[allow(dead_code)]
pub async fn list_worktrees(repo_local_path: &Path) -> Result<Vec<PathBuf>, AppError> {
    let output = run_git(
        &[
            "worktree".into(),
            "list".into(),
            "--porcelain".into(),
            "-z".into(),
        ],
        repo_local_path,
    )
    .await?;

    let mut paths = Vec::new();
    let mut is_first = true;

    for field in output.split('\0') {
        if let Some(path) = field.strip_prefix("worktree ") {
            if is_first {
                // Skip the main working tree (always the first entry)
                is_first = false;
                continue;
            }
            paths.push(PathBuf::from(path));
        }
    }

    Ok(paths)
}

/// Checks whether a worktree directory exists on disk.
#[allow(dead_code)]
pub fn worktree_exists(path: &Path) -> bool {
    path.exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::process::Command;

    /// Runs a shell command, panicking on failure.
    async fn sh(program: &str, args: &[&str], cwd: &Path) {
        let output = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .output()
            .await
            .unwrap();
        assert!(
            output.status.success(),
            "{program} {:?} failed in {}: {}",
            args,
            cwd.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// Creates a test git repo with a bare remote and a `feature-42` branch.
    ///
    /// Returns `(tempdir_guard, local_repo_path, base_dir_for_worktrees)`.
    async fn setup_test_repo() -> (TempDir, PathBuf, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let remote = tmp.path().join("remote.git");
        let local = tmp.path().join("local");
        let base_dir = tmp.path().join("workspaces");

        let remote_str = remote.to_string_lossy().to_string();
        let local_str = local.to_string_lossy().to_string();

        // Create bare remote
        sh("git", &["init", "--bare", &remote_str], tmp.path()).await;

        // Clone it
        sh("git", &["clone", &remote_str, &local_str], tmp.path()).await;

        // Configure git user
        sh("git", &["config", "user.email", "test@test.com"], &local).await;
        sh("git", &["config", "user.name", "Test"], &local).await;

        // Create initial commit and push
        sh("git", &["commit", "--allow-empty", "-m", "initial"], &local).await;
        sh("git", &["push", "origin", "HEAD"], &local).await;

        // Create feature branch with a commit and push
        sh("git", &["checkout", "-b", "feature-42"], &local).await;
        sh(
            "git",
            &["commit", "--allow-empty", "-m", "feature work"],
            &local,
        )
        .await;
        sh("git", &["push", "origin", "feature-42"], &local).await;

        // Go back to default branch
        sh("git", &["checkout", "-"], &local).await;

        (tmp, local, base_dir)
    }

    #[test]
    fn test_worktree_path_construction() {
        let base = PathBuf::from("/home/user/.prism/workspaces");

        let path = build_worktree_path(&base, "prism", 42).unwrap();
        assert_eq!(
            path,
            PathBuf::from("/home/user/.prism/workspaces/prism/worktrees/pr-42")
        );

        let path2 = build_worktree_path(&base, "other-repo", 123).unwrap();
        assert_eq!(
            path2,
            PathBuf::from("/home/user/.prism/workspaces/other-repo/worktrees/pr-123")
        );

        // Edge case: PR number 0
        let path3 = build_worktree_path(&base, "repo", 0).unwrap();
        assert_eq!(
            path3,
            PathBuf::from("/home/user/.prism/workspaces/repo/worktrees/pr-0")
        );

        // Path traversal rejected
        assert!(build_worktree_path(&base, "../escape", 1).is_err());
        assert!(build_worktree_path(&base, "owner/repo", 1).is_err());
        assert!(build_worktree_path(&base, "a\\b", 1).is_err());
        assert!(build_worktree_path(&base, "", 1).is_err());
        // Single dot rejected (would resolve to current directory)
        assert!(build_worktree_path(&base, ".", 1).is_err());
        // Trailing slash rejected (Path normalizes it away but raw string has '/')
        assert!(build_worktree_path(&base, "repo/", 1).is_err());
    }

    #[test]
    fn test_default_base_dir_returns_result() {
        // HOME is always set in test environments
        let result = default_base_dir();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(
            path.to_string_lossy().ends_with(".prism/workspaces"),
            "expected path ending with .prism/workspaces, got: {path:?}"
        );
    }

    #[tokio::test]
    async fn test_create_worktree_success() {
        let (_tmp, local, base_dir) = setup_test_repo().await;

        let result = create_worktree(&local, "feature-42", 42, "test-repo", &base_dir).await;
        assert!(result.is_ok(), "create_worktree failed: {result:?}");

        let wt_path = result.unwrap();
        assert!(wt_path.exists(), "worktree directory should exist");
        assert_eq!(
            wt_path,
            base_dir.join("test-repo").join("worktrees").join("pr-42")
        );

        // Worktree .git is a file (not a directory) pointing to the main repo
        let git_file = wt_path.join(".git");
        assert!(git_file.exists(), ".git should exist in worktree");
        assert!(git_file.is_file(), ".git in a worktree should be a file");
    }

    #[tokio::test]
    async fn test_create_worktree_branch_not_found() {
        let (_tmp, local, base_dir) = setup_test_repo().await;

        let result =
            create_worktree(&local, "nonexistent-branch", 99, "test-repo", &base_dir).await;
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(
            matches!(err, AppError::Git(_)),
            "expected Git error, got: {err}"
        );
        let msg = err.to_string();
        assert!(
            msg.contains("not found") && msg.contains("nonexistent-branch"),
            "expected user-friendly branch-not-found message, got: {msg}"
        );
    }

    #[tokio::test]
    async fn test_create_worktree_already_exists() {
        let (_tmp, local, base_dir) = setup_test_repo().await;

        // First creation succeeds
        create_worktree(&local, "feature-42", 42, "test-repo", &base_dir)
            .await
            .unwrap();

        // Second creation with same PR number fails
        let result = create_worktree(&local, "feature-42", 42, "test-repo", &base_dir).await;
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(
            matches!(err, AppError::Workspace(_)),
            "expected Workspace error, got: {err}"
        );
        assert!(
            err.to_string().contains("already exists"),
            "error should mention 'already exists': {err}"
        );
    }

    #[tokio::test]
    async fn test_remove_worktree() {
        let (_tmp, local, base_dir) = setup_test_repo().await;

        let wt_path = create_worktree(&local, "feature-42", 42, "test-repo", &base_dir)
            .await
            .unwrap();
        assert!(wt_path.exists());

        let result = remove_worktree(&local, &wt_path).await;
        assert!(result.is_ok(), "remove_worktree failed: {result:?}");
        assert!(
            !wt_path.exists(),
            "worktree directory should be removed after git worktree remove"
        );
    }

    #[tokio::test]
    async fn test_remove_worktree_not_found() {
        let (tmp, local, _base_dir) = setup_test_repo().await;

        let fake_path = tmp.path().join("nonexistent-worktree");
        let result = remove_worktree(&local, &fake_path).await;
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(
            matches!(err, AppError::Git(_)),
            "expected Git error, got: {err}"
        );
    }

    #[test]
    fn test_classify_git_error_branch_not_found() {
        let stderr = "fatal: couldn't find remote ref nonexistent-branch\n";
        let err = classify_git_error(stderr, "fetch origin -- nonexistent-branch");
        assert!(matches!(err, AppError::Git(_)), "expected Git, got: {err}");
        let msg = err.to_string();
        assert!(msg.contains("not found"), "missing 'not found' in: {msg}");
        assert!(
            msg.contains("nonexistent-branch"),
            "missing branch name in: {msg}"
        );
    }

    #[test]
    fn test_classify_git_error_permission_denied() {
        let stderr = "fatal: could not create work tree dir '/tmp/wt/pr-42': Permission denied\n";
        let err = classify_git_error(stderr, "worktree add /tmp/wt/pr-42 origin/main");
        assert!(matches!(err, AppError::Git(_)), "expected Git, got: {err}");
        let msg = err.to_string();
        assert!(
            msg.to_lowercase().contains("permission denied"),
            "missing 'permission denied' in: {msg}"
        );
    }

    #[test]
    fn test_classify_git_error_not_a_repo() {
        let stderr = "fatal: not a git repository (or any of the parent directories): .git\n";
        let err = classify_git_error(stderr, "fetch origin -- main");
        assert!(matches!(err, AppError::Git(_)), "expected Git, got: {err}");
        let msg = err.to_string();
        assert!(
            msg.contains("not a valid git repository"),
            "missing 'not a valid git repository' in: {msg}"
        );
    }

    #[test]
    fn test_classify_git_error_already_checked_out() {
        let stderr = "fatal: 'feature-42' is already checked out at '/path/to/other'\n";
        let err = classify_git_error(stderr, "worktree add /tmp/wt feature-42");
        assert!(
            matches!(err, AppError::Workspace(_)),
            "expected Workspace, got: {err}"
        );
        assert!(
            err.to_string().contains("already in use"),
            "missing 'already in use' in: {err}"
        );
    }

    #[test]
    fn test_classify_git_error_generic_fallback() {
        let stderr = "error: some unknown git problem\n";
        let err = classify_git_error(stderr, "status");
        assert!(matches!(err, AppError::Git(_)), "expected Git, got: {err}");
        let msg = err.to_string();
        assert!(
            msg.contains("git status failed"),
            "expected generic fallback, got: {msg}"
        );
        // Ensure raw stderr is NOT leaked in the user-facing message
        assert!(
            !msg.contains("some unknown git problem"),
            "raw stderr should not appear in user-facing message: {msg}"
        );
    }

    #[tokio::test]
    async fn test_create_worktree_invalid_repo_path() {
        let tmp = TempDir::new().unwrap();
        let fake_repo = tmp.path().join("not-a-repo");
        tokio::fs::create_dir_all(&fake_repo).await.unwrap();
        let base_dir = tmp.path().join("workspaces");

        let result = create_worktree(&fake_repo, "main", 1, "test-repo", &base_dir).await;
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(
            matches!(err, AppError::Git(_)),
            "expected Git error, got: {err}"
        );
        assert!(
            err.to_string().contains("not a valid git repository"),
            "expected repo validation error, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_create_worktree_repo_path_not_found() {
        let tmp = TempDir::new().unwrap();
        let missing_path = tmp.path().join("does-not-exist");
        let base_dir = tmp.path().join("workspaces");

        let result = create_worktree(&missing_path, "main", 1, "test-repo", &base_dir).await;
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(
            matches!(err, AppError::Git(_)),
            "expected Git error, got: {err}"
        );
        assert!(
            err.to_string().contains("does not exist"),
            "expected path-not-found error, got: {err}"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_worktree_error_permission() {
        use std::os::unix::fs::PermissionsExt;

        let (_tmp, local, base_dir) = setup_test_repo().await;

        // Create only the repo-level dir (not worktrees/) and make it
        // non-writable so create_dir_all in create_worktree hits PermissionDenied
        // when trying to create the worktrees/ subdirectory.
        let repo_root = base_dir.join("test-repo");
        tokio::fs::create_dir_all(&repo_root).await.unwrap();

        let mut perms = tokio::fs::metadata(&repo_root).await.unwrap().permissions();
        perms.set_mode(0o555); // r-xr-xr-x — no write
        tokio::fs::set_permissions(&repo_root, perms.clone())
            .await
            .unwrap();

        let result = create_worktree(&local, "feature-42", 42, "test-repo", &base_dir).await;

        // Restore permissions for cleanup
        perms.set_mode(0o755);
        tokio::fs::set_permissions(&repo_root, perms).await.unwrap();

        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.to_lowercase().contains("permission"),
            "expected permission-related error, got: {err_msg}"
        );
    }

    #[tokio::test]
    async fn test_list_worktrees() {
        let (_tmp, local, base_dir) = setup_test_repo().await;

        // No extra worktrees initially
        let empty = list_worktrees(&local).await.unwrap();
        assert!(empty.is_empty(), "should have no extra worktrees initially");

        // Create one worktree
        let wt_path = create_worktree(&local, "feature-42", 42, "test-repo", &base_dir)
            .await
            .unwrap();

        let worktrees = list_worktrees(&local).await.unwrap();
        assert_eq!(worktrees.len(), 1, "should list one worktree");
        assert_eq!(
            worktrees[0], wt_path,
            "listed path should match created worktree"
        );
    }
}
