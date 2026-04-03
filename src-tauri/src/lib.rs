mod cache;
mod commands;
mod config;
mod error;
mod github;
mod notifications;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod tray;
pub mod types;
mod workspace;

use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use sqlx::SqlitePool;
use tauri::Manager;
use tracing::info;

/// Guards against concurrent force-sync invocations from the tray or IPC.
pub(crate) struct SyncInFlight(pub(crate) AtomicBool);

impl Default for SyncInFlight {
    fn default() -> Self {
        Self(AtomicBool::new(false))
    }
}

/// Holds the background polling task handle for cancellation on logout/shutdown.
///
/// Managed as Tauri state. The inner `JoinHandle` is `None` until polling
/// starts at launch when valid credentials already exist in the keychain.
pub struct PollingHandle(pub(crate) Mutex<Option<tokio::task::JoinHandle<()>>>);

impl Default for PollingHandle {
    fn default() -> Self {
        Self(Mutex::new(None))
    }
}

/// Attempts to start background polling if credentials exist in the keychain.
///
/// Runs asynchronously after app setup completes. If no token is stored
/// or validation fails, polling is deferred until the next app launch.
pub(crate) async fn try_start_polling(app_handle: tauri::AppHandle, pool: sqlx::SqlitePool) {
    use crate::github::{auth, client::GitHubClient, polling::start_polling};

    // Read token from keychain (blocking — runs on a dedicated thread)
    let token = match tokio::task::spawn_blocking(auth::get_token).await {
        Ok(Ok(Some(t))) => t,
        Ok(Ok(None)) => {
            info!("no GitHub token at startup — polling deferred until next app launch");
            return;
        }
        Ok(Err(e)) => {
            info!("keychain error at startup — polling deferred: {e}");
            return;
        }
        Err(e) => {
            info!("task join error reading token: {e}");
            return;
        }
    };

    // Validate token to resolve the username
    let username = match auth::validate_token(&token).await {
        Ok(u) => u,
        Err(e) => {
            info!("token validation failed at startup — polling deferred: {e}");
            return;
        }
    };

    // Pre-populate the username cache so dashboard calls skip re-validation
    let cached = app_handle.state::<commands::GithubUsername>();
    match cached.0.lock() {
        Ok(mut guard) => {
            *guard = Some(username.clone());
        }
        Err(e) => {
            *e.into_inner() = Some(username.clone());
            tracing::warn!("GithubUsername mutex was poisoned; recovered");
        }
    }

    // Create client and start polling
    let client = match GitHubClient::new(&token) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("failed to create GitHub client at startup: {e}");
            return;
        }
    };

    let join_handle = start_polling(app_handle.clone(), pool, client, username);

    // Store handle so polling can be cancelled on logout or token change
    let state = app_handle.state::<PollingHandle>();
    match state.0.lock() {
        Ok(mut guard) => {
            if let Some(old) = guard.replace(join_handle) {
                old.abort();
                info!("replaced existing polling task; aborted previous");
            }
        }
        Err(e) => {
            // Recover from poison — store the handle anyway so it remains cancellable
            let mut guard = e.into_inner();
            if let Some(old) = guard.replace(join_handle) {
                old.abort();
                info!("replaced existing polling task after poison recovery; aborted previous");
            }
            tracing::warn!("PollingHandle mutex was poisoned; recovered");
        }
    }
}

/// Initialize the tracing subscriber with console output and non-blocking rotating file appender.
///
/// - Reads `RUST_LOG` env for level filtering (defaults to `info`).
/// - In debug builds, also logs to stdout with ANSI colors.
/// - Always writes to a daily-rotating file in the platform-specific data dir.
///
/// Returns a [`WorkerGuard`] that **must** be held for the process lifetime;
/// dropping it flushes and stops the background writer thread.
fn init_tracing() -> tracing_appender::non_blocking::WorkerGuard {
    use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

    let log_dir = log_dir_path();
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("failed to create log directory {}: {e}", log_dir.display());
    }

    let file_appender = tracing_appender::rolling::daily(&log_dir, "prism.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let registry = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false));

    if cfg!(debug_assertions) {
        registry
            .with(fmt::layer().with_writer(std::io::stdout))
            .init();
    } else {
        registry.init();
    }

    guard
}

/// Returns the platform-specific log directory path.
///
/// - Linux: `$XDG_DATA_HOME/prism/logs` or `~/.local/share/prism/logs`
/// - macOS: `~/Library/Application Support/prism/logs`
/// - Windows: `%APPDATA%/prism/logs`
fn log_dir_path() -> std::path::PathBuf {
    if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
        return std::path::PathBuf::from(data_home).join("prism/logs");
    }
    #[cfg(target_os = "windows")]
    if let Ok(appdata) = std::env::var("APPDATA") {
        return std::path::PathBuf::from(appdata).join("prism\\logs");
    }
    if let Ok(home) = std::env::var("HOME") {
        #[cfg(target_os = "macos")]
        return std::path::PathBuf::from(&home).join("Library/Application Support/prism/logs");
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        return std::path::PathBuf::from(&home).join(".local/share/prism/logs");
    }
    std::path::PathBuf::from("prism-logs")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize structured logging before Tauri setup.
    // The guard must live for the full process lifetime to flush pending log events.
    let _log_guard = init_tracing();

    let mut builder = tauri::Builder::default().plugin(tauri_plugin_opener::init());

    #[cfg(debug_assertions)]
    {
        builder = builder.plugin(tauri_plugin_pilot::init());
    }

    builder
        .setup(|app| {
            // Initialize SQLite database
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("prism.db");
            let pool = tauri::async_runtime::block_on(cache::db::init_db(&db_path))
                .map_err(|e| e.to_string())?;
            let poll_pool = pool.clone();
            app.manage(pool);

            // Cached GitHub username — populated on first dashboard access
            app.manage(commands::GithubUsername::default());

            // PTY manager — tracks spawned terminal sessions
            app.manage(commands::PtyManagerState::new());

            // Polling handle — empty until credentials are verified
            app.manage(PollingHandle::default());

            // Force-sync guard — prevents concurrent sync invocations
            app.manage(SyncInFlight::default());

            // System tray icon with context menu (desktop only)
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            tray::setup_tray(app).map_err(|e| e.to_string())?;

            // Attempt to start background polling (non-blocking)
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                try_start_polling(handle, poll_pool).await;
            });

            // Start workspace lifecycle task (auto-suspend & auto-archive).
            // Intentionally detached: the tokio runtime abort on shutdown is sufficient.
            let lifecycle_pool = app.state::<SqlitePool>().inner().clone();
            let lifecycle_handle = app.handle().clone();
            let _lifecycle_task =
                workspace::lifecycle::start_workspace_lifecycle(lifecycle_handle, lifecycle_pool);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth_set_token,
            commands::auth_get_status,
            commands::auth_logout,
            commands::github_get_dashboard,
            commands::github_get_stats,
            commands::stats_personal,
            commands::github_force_sync,
            commands::repos_list,
            commands::repos_set_enabled,
            commands::repos_set_local_path,
            commands::config_get,
            commands::config_set,
            commands::activity_mark_read,
            commands::activity_mark_all_read,
            commands::workspace_list,
            commands::workspace_list_enriched,
            commands::workspace_get_notes,
            commands::workspace_open,
            commands::workspace_suspend,
            commands::workspace_resume,
            commands::workspace_archive,
            commands::workspace_cleanup,
            commands::pty_write,
            commands::pty_resize,
            commands::pty_kill,
            commands::debug_memory_usage,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_harness_runs_successfully() {
        assert_eq!(1 + 1, 2, "Test harness is functional");
    }

    #[test]
    fn test_polling_handle_default_is_none() {
        let handle = PollingHandle::default();
        let guard = handle.0.lock().expect("lock should not be poisoned");
        assert!(guard.is_none(), "default PollingHandle should be None");
    }

    #[tokio::test]
    async fn test_polling_handle_stores_join_handle() {
        let handle = PollingHandle::default();
        let jh = tokio::spawn(async {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        });
        *handle.0.lock().unwrap() = Some(jh);
        assert!(
            handle.0.lock().unwrap().is_some(),
            "PollingHandle should hold the JoinHandle"
        );

        // Cleanup: abort the spawned task
        handle.0.lock().unwrap().take().unwrap().abort();
    }

    #[tokio::test]
    async fn test_polling_handle_abort_cancels_task() {
        let handle = PollingHandle::default();
        let jh = tokio::spawn(async {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        });
        *handle.0.lock().unwrap() = Some(jh);

        // Abort via take pattern
        let taken = handle.0.lock().unwrap().take();
        assert!(taken.is_some(), "should take the handle");
        taken.unwrap().abort();

        // After take, inner should be None
        assert!(
            handle.0.lock().unwrap().is_none(),
            "after take(), PollingHandle should be None"
        );
    }
}
