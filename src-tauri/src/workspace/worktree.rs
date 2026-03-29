use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::process::Command;
use tokio::time::timeout;

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
    if !is_single_normal || repo_name.contains('\\') {
        return Err(AppError::Workspace(format!(
            "invalid repo_name: {repo_name:?} (must be a single normal path component)"
        )));
    }

    Ok(base_dir
        .join(repo_name)
        .join("worktrees")
        .join(format!("pr-{pr_number}")))
}

/// Runs a git command in the given directory and returns stdout on success.
///
/// Times out after [`GIT_TIMEOUT`] to prevent indefinite hangs on network operations.
/// The spawned child process is killed when the timeout fires (`kill_on_drop`).
/// Accepts [`OsString`] args so paths with non-UTF-8 bytes are passed verbatim to git.
async fn run_git(args: &[OsString], cwd: &Path) -> Result<String, AppError> {
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
        return Err(AppError::Git(format!(
            "git {} failed: {}",
            args_display,
            stderr.trim()
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
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
            AppError::Workspace(format!("failed to create worktree directory: {e}"))
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
        &["worktree".into(), "list".into(), "--porcelain".into()],
        repo_local_path,
    )
    .await?;

    let mut paths = Vec::new();
    let mut is_first = true;

    for line in output.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
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
