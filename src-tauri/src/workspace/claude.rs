use std::sync::LazyLock;

use regex::Regex;

use super::pty::PtyManager;
use crate::error::AppError;

/// Matches a Claude Code session ID from stdout.
///
/// Captures the ID token after patterns like:
/// - `Session: <id>`
/// - `session id: <id>`
/// - `Resuming session <id>`
static SESSION_ID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:session(?:\s+id)?)[:\s=]+([a-zA-Z0-9_-]{8,})").unwrap());

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
}
