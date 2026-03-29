use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use uuid::Uuid;

use crate::error::AppError;

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
        let mut cmd = CommandBuilder::new(&shell);
        cmd.cwd(cwd);

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
        let reader_task = tokio::task::spawn_blocking(move || {
            let mut reader = reader;
            let mut buf = [0u8; 4096];
            loop {
                match std::io::Read::read(&mut reader, &mut buf) {
                    Ok(0) => break,
                    Ok(n) => on_output(&id_for_task, &buf[..n]),
                    Err(e) => {
                        log::warn!("pty reader error for {id_for_task}: {e}");
                        break;
                    }
                }
            }
            // Child exited — remove stale entry from the map.
            if let Ok(mut ptys) = ptys_for_task.lock() {
                ptys.remove(&id_for_task);
            }
        });

        let handle = PtyHandle {
            writer: Arc::new(Mutex::new(writer)),
            master: Arc::new(Mutex::new(pair.master)),
            child,
            reader_task,
            killed: Arc::new(AtomicBool::new(false)),
        };

        self.ptys
            .lock()
            .map_err(|e| AppError::Pty(format!("lock poisoned: {e}")))?
            .insert(pty_id.clone(), handle);

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
