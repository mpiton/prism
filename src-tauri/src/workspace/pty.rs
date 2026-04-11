use std::collections::HashMap;
use std::ffi::OsString;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use uuid::Uuid;

use crate::error::AppError;

/// Environment variables that can cause shells to source arbitrary files
/// or alter execution semantics. Removed from the PTY environment to prevent
/// code execution from untrusted repositories and avoid silent UX breakage.
const DANGEROUS_ENV_VARS: &[&str] = &[
    "BASH_ENV",       // bash sources this file in non-interactive mode
    "ENV",            // sh/bash sources this file for non-interactive shells
    "ZDOTDIR",        // zsh looks for startup files in this directory
    "PROMPT_COMMAND", // bash executes this before each prompt (interactive)
    "PS0",            // bash 4.4+ expands this before command execution (supports $(...))
    "SHELLOPTS",      // bash option flags inherited by child shells (e.g. extdebug, noexec)
    "BASHOPTS",       // bash 4.0+ shopt options inherited by child shells
];

/// Normalizes a shell binary name to its canonical lowercase form,
/// stripping a trailing `.exe` extension when present.
///
/// Splits on both `/` and `\` so that Windows-style paths (e.g.
/// `C:\Program Files\Git\bin\bash.exe`) are handled correctly even on
/// platforms where `Path` does not recognize backslash as a separator.
fn normalized_shell_name(shell_path: &str) -> String {
    let basename = shell_path.rsplit(['/', '\\']).next().unwrap_or("");
    let lower = basename.to_ascii_lowercase();
    lower
        .strip_suffix(".exe")
        .map(str::to_string)
        .unwrap_or(lower)
}

/// Returns shell isolation flags that prevent loading user/system configuration files.
///
/// Different shells use different flags to skip startup files:
/// - bash: `--noprofile --norc` (skip `~/.bash_profile` and `~/.bashrc`)
/// - zsh: `--no-rcs --no-globalrcs` (skip all zshrc files including `/etc/zshrc`)
/// - fish: `--no-config` (skip `config.fish`)
/// - Others (including bare `sh`): no flags
///
/// `sh` deliberately receives no flags. On Debian/Ubuntu and derivatives,
/// `/bin/sh` is `dash`, which rejects bash-specific flags (`dash: 0: Illegal
/// option --`) and would prevent the PTY from spawning. On macOS and RHEL
/// `sh` is bash in POSIX mode, but `dash` is already safer by default
/// (it does not source user rc files in non-login mode), so the residual
/// risk on `sh = bash` systems is mitigated by the `HOME` pinning and
/// dangerous-env-var stripping that runs unconditionally.
///
/// Windows variants like `bash.exe` are handled via [`normalized_shell_name`].
fn isolation_flags_for_shell(shell_path: &str) -> Vec<&'static str> {
    match normalized_shell_name(shell_path).as_str() {
        "bash" => vec!["--noprofile", "--norc"],
        "zsh" => vec!["--no-rcs", "--no-globalrcs"],
        "fish" => vec!["--no-config"],
        _ => vec![],
    }
}

/// Computes the full set of environment variable names to strip from the
/// PTY environment, including both the static `DANGEROUS_ENV_VARS` list and
/// any `BASH_FUNC_*` exported functions discovered in the parent process env.
///
/// Uses [`std::env::vars_os`] so that non-UTF-8 environment entries (which
/// are valid on Unix) do not cause a panic.
fn env_vars_to_remove() -> Vec<OsString> {
    let mut vars: Vec<OsString> = DANGEROUS_ENV_VARS.iter().map(OsString::from).collect();

    // Bash encodes exported shell functions as BASH_FUNC_<name>%% env vars.
    // These are loaded by the interpreter before --norc/--noprofile take effect,
    // so they must be stripped explicitly.
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("BASH_FUNC_") {
            vars.push(key);
        }
    }

    vars
}

/// Configures the command environment to prevent untrusted code execution.
///
/// - Removes environment variables that cause shells to source arbitrary files
/// - Strips `BASH_FUNC_*` exported functions inherited from the parent process
/// - Explicitly sets `HOME` to the user's real home directory (preserving the
///   inherited value via [`std::env::var_os`] to remain non-UTF-8 safe)
fn configure_safe_environment(cmd: &mut CommandBuilder) {
    for var in env_vars_to_remove() {
        cmd.env_remove(&var);
    }

    if let Some(home) = std::env::var_os("HOME") {
        cmd.env("HOME", home);
    }
}

/// Builds a `CommandBuilder` for spawning an isolated PTY shell.
///
/// This is the single source of truth for PTY command construction:
/// applies isolation flags ([`isolation_flags_for_shell`]), sanitizes
/// the environment ([`configure_safe_environment`]), and pins the working
/// directory. Extracted from [`PtyManager::spawn`] so that command
/// construction can be unit-tested for security regressions.
fn build_isolated_pty_command(shell: &str, cwd: &Path) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(shell);
    for flag in isolation_flags_for_shell(shell) {
        cmd.arg(flag);
    }
    configure_safe_environment(&mut cmd);
    cmd.cwd(cwd);
    cmd
}

/// Handle for a single PTY session.
///
/// Holds the writer and master (per-PTY locks), the child process,
/// a background reader task handle, and a `killed` flag that prevents
/// `Drop` from issuing a redundant kill after an explicit `kill()`.
#[allow(dead_code)]
struct PtyHandle {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    reader_task: tokio::task::JoinHandle<()>,
    killed: Arc<AtomicBool>,
}

impl Drop for PtyHandle {
    fn drop(&mut self) {
        self.reader_task.abort();
        if !self.killed.load(Ordering::Acquire) {
            // Best-effort cleanup — only kill if not already killed explicitly.
            let _ = self.child.kill();
        }
    }
}

/// Manages multiple PTY sessions, keyed by UUID string.
///
/// Thread-safe via internal `Arc<Mutex<…>>`. Clone to share across tasks.
#[allow(dead_code)]
#[derive(Clone)]
pub struct PtyManager {
    ptys: Arc<Mutex<HashMap<String, PtyHandle>>>,
}

impl PtyManager {
    /// Creates a new, empty `PtyManager`.
    #[allow(dead_code)]
    #[must_use]
    pub fn new() -> Self {
        Self {
            ptys: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Spawns a new PTY running the user's default shell.
    ///
    /// Returns the PTY id (UUID v4). A background `spawn_blocking` task reads
    /// stdout and forwards each chunk to `on_output(pty_id, data)`. When the
    /// child exits (reader EOF), the session is automatically removed from the
    /// manager.
    ///
    /// The `on_output` callback must be `Send + 'static` because it is moved
    /// into the blocking reader task.
    ///
    /// # Panics
    ///
    /// Panics if called outside a Tokio runtime context (uses
    /// `tokio::task::spawn_blocking` internally).
    #[allow(dead_code)]
    #[tracing::instrument(skip(self, on_output, cwd))]
    pub fn spawn(
        &self,
        cwd: &Path,
        cols: u16,
        rows: u16,
        on_output: impl Fn(&str, &[u8]) + Send + 'static,
    ) -> Result<String, AppError> {
        let pty_system = native_pty_system();

        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| AppError::Pty(format!("failed to open pty: {e}")))?;

        let shell = if cfg!(windows) {
            std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into())
        } else {
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into())
        };
        let cmd = build_isolated_pty_command(&shell, cwd);

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| AppError::Pty(format!("failed to spawn shell: {e}")))?;

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| AppError::Pty(format!("failed to take pty writer: {e}")))?;

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| AppError::Pty(format!("failed to clone pty reader: {e}")))?;

        let pty_id = Uuid::new_v4().to_string();

        // Clone ptys Arc so the reader task can remove stale entries on EOF.
        let ptys_for_task = Arc::clone(&self.ptys);
        let id_for_task = pty_id.clone();
        let reader_closed = Arc::new(AtomicBool::new(false));
        let reader_closed_for_task = Arc::clone(&reader_closed);
        let reader_task = tokio::task::spawn_blocking(move || {
            let mut reader = reader;
            let mut buf = [0u8; 4096];
            loop {
                match std::io::Read::read(&mut reader, &mut buf) {
                    Ok(0) => break,
                    Ok(n) => on_output(&id_for_task, &buf[..n]),
                    Err(e) => {
                        tracing::warn!("pty reader error for {id_for_task}: {e}");
                        break;
                    }
                }
            }
            // Signal that the reader has exited (for race detection on insert).
            reader_closed_for_task.store(true, Ordering::Release);
            // Child exited — remove stale entry from the map.
            // Release the map lock before dropping the PtyHandle so that
            // Drop::drop() (which may call child.kill()) runs outside the lock.
            if let Ok(mut ptys) = ptys_for_task.lock() {
                let removed = ptys.remove(&id_for_task);
                drop(ptys);
                drop(removed);
            }
        });

        let handle = PtyHandle {
            writer: Arc::new(Mutex::new(writer)),
            master: Arc::new(Mutex::new(pair.master)),
            child,
            reader_task,
            killed: Arc::new(AtomicBool::new(false)),
        };

        // Insert the handle, then check if the reader already exited before
        // the insert completed (immediate EOF race). If so, clean up now.
        let mut ptys = self
            .ptys
            .lock()
            .map_err(|e| AppError::Pty(format!("lock poisoned: {e}")))?;
        ptys.insert(pty_id.clone(), handle);
        if reader_closed.load(Ordering::Acquire) {
            let removed = ptys.remove(&pty_id);
            drop(ptys);
            drop(removed);
        }

        Ok(pty_id)
    }

    /// Writes data to the PTY's stdin.
    ///
    /// Acquires only the per-PTY writer lock, not the global map lock during I/O.
    #[allow(dead_code)]
    pub fn write_pty(&self, pty_id: &str, data: &[u8]) -> Result<(), AppError> {
        let writer = {
            let ptys = self
                .ptys
                .lock()
                .map_err(|e| AppError::Pty(format!("lock poisoned: {e}")))?;

            let handle = ptys
                .get(pty_id)
                .ok_or_else(|| AppError::NotFound(format!("pty {pty_id}")))?;

            Arc::clone(&handle.writer)
        }; // map lock released here

        let mut writer = writer
            .lock()
            .map_err(|e| AppError::Pty(format!("writer lock poisoned: {e}")))?;

        writer
            .write_all(data)
            .map_err(|e| AppError::Pty(format!("write failed: {e}")))?;
        writer
            .flush()
            .map_err(|e| AppError::Pty(format!("flush failed: {e}")))?;

        Ok(())
    }

    /// Resizes a PTY to new dimensions.
    ///
    /// Acquires only the per-PTY master lock, not the global map lock during
    /// the `ioctl` syscall.
    #[allow(dead_code)]
    pub fn resize(&self, pty_id: &str, cols: u16, rows: u16) -> Result<(), AppError> {
        let master = {
            let ptys = self
                .ptys
                .lock()
                .map_err(|e| AppError::Pty(format!("lock poisoned: {e}")))?;

            let handle = ptys
                .get(pty_id)
                .ok_or_else(|| AppError::NotFound(format!("pty {pty_id}")))?;

            Arc::clone(&handle.master)
        }; // map lock released here

        let master = master
            .lock()
            .map_err(|e| AppError::Pty(format!("master lock poisoned: {e}")))?;

        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| AppError::Pty(format!("resize failed: {e}")))?;

        Ok(())
    }

    /// Kills a PTY process and removes it from the manager.
    ///
    /// The map lock is released before performing the kill syscall.
    /// Sets the `killed` flag so `Drop` does not issue a redundant kill.
    #[allow(dead_code)]
    #[tracing::instrument(skip(self))]
    pub fn kill(&self, pty_id: &str) -> Result<(), AppError> {
        let mut handle = {
            let mut ptys = self
                .ptys
                .lock()
                .map_err(|e| AppError::Pty(format!("lock poisoned: {e}")))?;

            ptys.remove(pty_id)
                .ok_or_else(|| AppError::NotFound(format!("pty {pty_id}")))?
        }; // map lock released here

        handle.reader_task.abort();

        handle
            .child
            .kill()
            .map_err(|e| AppError::Pty(format!("kill failed: {e}")))?;

        // Mark as killed so Drop does not call kill() again.
        handle.killed.store(true, Ordering::Release);

        Ok(())
    }
}

impl Default for PtyManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;

    /// Helper: spawns a PTY in a temp dir with a channel-based output collector.
    fn spawn_test_pty(manager: &PtyManager) -> (String, mpsc::Receiver<Vec<u8>>) {
        let tmp = std::env::temp_dir();
        let (tx, rx) = mpsc::channel();

        let pty_id = manager
            .spawn(&tmp, 80, 24, move |_id, data| {
                let _ = tx.send(data.to_vec());
            })
            .expect("spawn_pty should succeed");

        (pty_id, rx)
    }

    #[tokio::test]
    async fn test_spawn_pty() {
        let manager = PtyManager::new();
        let (pty_id, _rx) = spawn_test_pty(&manager);

        assert!(
            Uuid::parse_str(&pty_id).is_ok(),
            "pty_id should be a valid UUID: {pty_id}"
        );

        let ptys = manager.ptys.lock().unwrap();
        assert!(ptys.contains_key(&pty_id), "manager should track the pty");

        drop(ptys);
        manager.kill(&pty_id).unwrap();
    }

    #[tokio::test]
    async fn test_write_to_pty() {
        let manager = PtyManager::new();
        let (pty_id, rx) = spawn_test_pty(&manager);

        tokio::time::sleep(Duration::from_millis(200)).await;

        let result = manager.write_pty(&pty_id, b"echo hello_pty_test\n");
        assert!(result.is_ok(), "write_pty should succeed: {result:?}");

        // Accumulate output chunks and look for our marker string.
        // PTY output may include prompts and ANSI escapes, so exact line
        // matching is too fragile — `contains` is appropriate here.
        let mut found = false;
        let mut output = String::new();
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            if let Ok(data) = rx.recv_timeout(Duration::from_millis(100)) {
                output.push_str(&String::from_utf8_lossy(&data));
                if output.contains("hello_pty_test") {
                    found = true;
                    break;
                }
            }
        }
        assert!(found, "should see 'hello_pty_test' in PTY output");

        manager.kill(&pty_id).unwrap();
    }

    #[tokio::test]
    async fn test_resize_pty() {
        let manager = PtyManager::new();
        let (pty_id, _rx) = spawn_test_pty(&manager);

        let result = manager.resize(&pty_id, 120, 40);
        assert!(result.is_ok(), "resize should succeed: {result:?}");

        manager.kill(&pty_id).unwrap();
    }

    #[tokio::test]
    async fn test_kill_pty() {
        let manager = PtyManager::new();
        let (pty_id, _rx) = spawn_test_pty(&manager);

        let result = manager.kill(&pty_id);
        assert!(result.is_ok(), "kill should succeed: {result:?}");

        let ptys = manager.ptys.lock().unwrap();
        assert!(
            !ptys.contains_key(&pty_id),
            "pty should be removed after kill"
        );
    }

    #[tokio::test]
    async fn test_kill_pty_not_found() {
        let manager = PtyManager::new();

        let result = manager.kill("nonexistent-id");
        assert!(result.is_err(), "kill nonexistent should fail");

        let err = result.unwrap_err();
        assert!(
            matches!(err, AppError::NotFound(_)),
            "expected NotFound error, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_multiple_ptys() {
        let manager = PtyManager::new();

        let (id1, _rx1) = spawn_test_pty(&manager);
        let (id2, _rx2) = spawn_test_pty(&manager);
        let (id3, _rx3) = spawn_test_pty(&manager);

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);

        {
            let ptys = manager.ptys.lock().unwrap();
            assert_eq!(ptys.len(), 3, "should have 3 ptys");
        }

        manager.kill(&id2).unwrap();
        {
            let ptys = manager.ptys.lock().unwrap();
            assert_eq!(ptys.len(), 2, "should have 2 ptys after killing one");
            assert!(ptys.contains_key(&id1));
            assert!(!ptys.contains_key(&id2));
            assert!(ptys.contains_key(&id3));
        }

        manager.kill(&id1).unwrap();
        manager.kill(&id3).unwrap();
    }

    // ── Shell isolation tests ──────────────────────────────────────

    #[test]
    fn test_isolation_flags_bash() {
        let flags = isolation_flags_for_shell("/bin/bash");
        assert_eq!(flags, vec!["--noprofile", "--norc"]);
    }

    #[test]
    fn test_isolation_flags_zsh() {
        let flags = isolation_flags_for_shell("/bin/zsh");
        assert_eq!(flags, vec!["--no-rcs", "--no-globalrcs"]);
    }

    #[test]
    fn test_isolation_flags_fish() {
        let flags = isolation_flags_for_shell("/usr/bin/fish");
        assert_eq!(flags, vec!["--no-config"]);
    }

    #[test]
    fn test_isolation_flags_sh_returns_empty() {
        // /bin/sh is dash on Debian/Ubuntu, which rejects bash flags with
        // "Illegal option --". Returning empty flags keeps PTY spawn working
        // on dash systems and is acceptable on bash-as-sh systems because
        // HOME pinning and env sanitization still apply unconditionally.
        let flags = isolation_flags_for_shell("/bin/sh");
        assert!(flags.is_empty());
    }

    #[test]
    fn test_isolation_flags_dash_returns_empty() {
        let flags = isolation_flags_for_shell("/bin/dash");
        assert!(flags.is_empty());
    }

    #[test]
    fn test_isolation_flags_unknown_shell() {
        let flags = isolation_flags_for_shell("/usr/bin/csh");
        assert!(flags.is_empty());
    }

    #[test]
    fn test_isolation_flags_bare_name() {
        let flags = isolation_flags_for_shell("bash");
        assert_eq!(flags, vec!["--noprofile", "--norc"]);
    }

    #[test]
    fn test_isolation_flags_full_path_with_usr_local() {
        let flags = isolation_flags_for_shell("/usr/local/bin/zsh");
        assert_eq!(flags, vec!["--no-rcs", "--no-globalrcs"]);
    }

    #[test]
    fn test_isolation_flags_windows_bash_exe() {
        // Git for Windows ships bash.exe — strip the .exe to match.
        let flags = isolation_flags_for_shell("C:\\Program Files\\Git\\bin\\bash.exe");
        assert_eq!(flags, vec!["--noprofile", "--norc"]);
    }

    #[test]
    fn test_isolation_flags_windows_zsh_exe() {
        let flags = isolation_flags_for_shell("zsh.exe");
        assert_eq!(flags, vec!["--no-rcs", "--no-globalrcs"]);
    }

    #[test]
    fn test_isolation_flags_uppercase_shell_name() {
        // Windows file systems are case-insensitive; normalize to lowercase.
        let flags = isolation_flags_for_shell("BASH.EXE");
        assert_eq!(flags, vec!["--noprofile", "--norc"]);
    }

    #[test]
    fn test_dangerous_env_vars_contains_known_threats() {
        assert!(DANGEROUS_ENV_VARS.contains(&"BASH_ENV"));
        assert!(DANGEROUS_ENV_VARS.contains(&"ENV"));
        assert!(DANGEROUS_ENV_VARS.contains(&"ZDOTDIR"));
        assert!(DANGEROUS_ENV_VARS.contains(&"PROMPT_COMMAND"));
        assert!(DANGEROUS_ENV_VARS.contains(&"PS0"));
        assert!(DANGEROUS_ENV_VARS.contains(&"SHELLOPTS"));
        assert!(DANGEROUS_ENV_VARS.contains(&"BASHOPTS"));
    }

    #[test]
    fn test_env_vars_to_remove_includes_all_dangerous_vars() {
        let removed = env_vars_to_remove();
        let names: Vec<String> = removed
            .iter()
            .map(|v| v.to_string_lossy().into_owned())
            .collect();
        for &expected in DANGEROUS_ENV_VARS {
            assert!(
                names.contains(&expected.to_string()),
                "env_vars_to_remove() should include {expected}"
            );
        }
    }

    #[test]
    fn test_build_isolated_pty_command_strips_dangerous_vars() {
        // Regression guard: if spawn() ever stops calling configure_safe_environment,
        // or if a new dangerous var is added without sanitization, this fails.
        let cmd = build_isolated_pty_command("/bin/bash", Path::new("/tmp"));
        for &var in DANGEROUS_ENV_VARS {
            assert!(
                cmd.get_env(var).is_none(),
                "{var} must be absent from PTY environment after isolation"
            );
        }
    }

    #[test]
    fn test_build_isolated_pty_command_sets_cwd() {
        let cmd = build_isolated_pty_command("/bin/bash", Path::new("/tmp"));
        let cwd = cmd
            .get_cwd()
            .map(|c| c.to_string_lossy().into_owned())
            .unwrap_or_default();
        assert_eq!(cwd, "/tmp");
    }

    #[test]
    fn test_build_isolated_pty_command_preserves_home_when_set() {
        // The test runner inherits HOME from the developer/CI environment,
        // so we expect it to be set in the PTY env after build.
        if std::env::var_os("HOME").is_some() {
            let cmd = build_isolated_pty_command("/bin/bash", Path::new("/tmp"));
            assert!(
                cmd.get_env("HOME").is_some(),
                "HOME should be preserved in PTY environment when set in parent"
            );
        }
    }

    #[tokio::test]
    async fn test_exit_removes_stale_entry() {
        let manager = PtyManager::new();
        let (pty_id, _rx) = spawn_test_pty(&manager);

        // Give the shell time to start
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Tell the shell to exit
        manager.write_pty(&pty_id, b"exit\n").unwrap();

        // Wait for the reader task to clean up the stale entry
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut removed = false;
        while std::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(100)).await;
            let ptys = manager.ptys.lock().unwrap();
            if !ptys.contains_key(&pty_id) {
                removed = true;
                break;
            }
        }
        assert!(removed, "pty entry should be removed after shell exits");
    }
}
