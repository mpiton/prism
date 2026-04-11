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

/// Label of the main application window — shared between tray and app setup.
pub(crate) const MAIN_WINDOW_LABEL: &str = "main";
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

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let builder = builder.on_menu_event(tray::handle_menu_event);

    builder
        .setup(|app| {
            // Initialize SQLite database
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("prism.db");
            let pool = tauri::async_runtime::block_on(cache::db::init_db(&db_path))
                .map_err(|e| e.to_string())?;
            let reconciled = tauri::async_runtime::block_on(
                cache::workspaces::suspend_orphaned_active_workspaces(&pool),
            )
            .map_err(|e| e.to_string())?;
            if reconciled > 0 {
                info!("startup: downgraded {reconciled} orphaned active workspace(s) to suspended");
            }
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

            // Set window icon from bundled PNG
            if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
                let png_bytes = include_bytes!("../icons/icon.png");
                if let Ok(img) =
                    image::load_from_memory_with_format(png_bytes, image::ImageFormat::Png)
                {
                    let rgba = img.to_rgba8();
                    let (w, h) = rgba.dimensions();
                    let icon = tauri::image::Image::new_owned(rgba.into_raw(), w, h);
                    let _ = window.set_icon(icon);
                }
            }

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
            commands::github_list_notifications,
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

    /// Tokens that must NEVER appear in any CSP (production or development).
    /// The sweep is token-based (whitespace-split), so `http:` matches only a
    /// bare scheme-wide allowance — specific hosts like `http://ipc.localhost`
    /// are not flagged because they are distinct tokens, and they are pinned
    /// separately by the exact-allowlist assertions.
    const CSP_FORBIDDEN_TOKENS: &[&str] = &[
        "'unsafe-eval'",
        "'unsafe-hashes'",
        "'wasm-unsafe-eval'",
        "http:",
        "https:",
        "*",
    ];

    /// Directives that the CSP object MUST contain — no more, no less. Adding
    /// a new directive to `tauri.conf.json` without updating this list (and
    /// adding an assertion for it in `assert_csp_environment`) is a test
    /// failure. This guards against silent drift where someone adds e.g.
    /// `worker-src *` and no test catches it.
    const CSP_EXPECTED_DIRECTIVES: &[&str] = &[
        "default-src",
        "script-src",
        "style-src",
        "img-src",
        "font-src",
        "connect-src",
        "object-src",
        "base-uri",
        "form-action",
        "frame-ancestors",
    ];

    /// Parses a CSP directive value (string) into a deduplicated set of tokens,
    /// splitting on whitespace. Used so tests compare *exact* allowlists rather
    /// than falling back to substring checks that let stray entries slip through.
    fn csp_tokens(value: &str) -> std::collections::BTreeSet<&str> {
        value.split_whitespace().collect()
    }

    /// Returns the string value of a CSP directive, panicking with a message
    /// that distinguishes a missing key from a non-string value.
    fn csp_directive<'a>(csp: &'a serde_json::Value, directive: &str, label: &str) -> &'a str {
        let value = &csp[directive];
        assert!(
            !value.is_null(),
            "{label} CSP is missing required directive `{directive}`"
        );
        value.as_str().unwrap_or_else(|| {
            panic!("{label} CSP directive `{directive}` must be a string, got {value}")
        })
    }

    /// Runs the full set of CSP assertions for one environment (production or dev).
    /// Kept as a helper so the `#[test]` stays under the clippy line budget.
    fn assert_csp_environment(csp: &serde_json::Value, label: &str, is_prod: bool) {
        use std::collections::BTreeSet;

        let csp_obj = csp.as_object().unwrap_or_else(|| {
            panic!("{label} CSP must be a non-null object — XSS can escalate to RCE in Tauri apps")
        });

        // Directive key set must match exactly. Prevents silent addition of
        // unchecked directives (e.g. `worker-src *`).
        let actual_keys: BTreeSet<&str> = csp_obj.keys().map(String::as_str).collect();
        let expected_keys: BTreeSet<&str> = CSP_EXPECTED_DIRECTIVES.iter().copied().collect();
        assert_eq!(
            actual_keys, expected_keys,
            "{label} CSP directive key set must match the expected set exactly"
        );

        // script-src: exactly 'self' in both environments. Tauri 2.x injects its
        // own script hashes at build time; no relaxation needed.
        assert_eq!(
            csp_tokens(csp_directive(csp, "script-src", label)),
            BTreeSet::from(["'self'"]),
            "{label} CSP script-src must be exactly 'self'"
        );

        // style-src: exactly `'self' 'unsafe-inline'` in both environments.
        // Justification for 'unsafe-inline': xterm.js creates <style> elements
        // at runtime via document.createElement('style') (see xterm.mjs). Those
        // tags are blocked by a strict style-src without 'unsafe-inline'. The
        // terminal is a core PRism feature, so this relaxation is intentional
        // and locked down here to prevent silent drift.
        assert_eq!(
            csp_tokens(csp_directive(csp, "style-src", label)),
            BTreeSet::from(["'self'", "'unsafe-inline'"]),
            "{label} CSP style-src must be exactly \"'self' 'unsafe-inline'\" \
             (required by xterm.js dynamic <style> elements)"
        );

        // default-src: exactly 'self' so forgotten directives still default to
        // self-only.
        assert_eq!(
            csp_tokens(csp_directive(csp, "default-src", label)),
            BTreeSet::from(["'self'"]),
            "{label} CSP default-src must be exactly 'self'"
        );

        // font-src: exactly 'self' — fonts are bundled via @fontsource.
        assert_eq!(
            csp_tokens(csp_directive(csp, "font-src", label)),
            BTreeSet::from(["'self'"]),
            "{label} CSP font-src must be exactly 'self'"
        );

        // object-src: blocks plugin-based code execution (Flash, etc.).
        assert_eq!(
            csp_directive(csp, "object-src", label),
            "'none'",
            "{label} CSP object-src must be 'none'"
        );

        // frame-ancestors / form-action / base-uri: clickjacking, form-hijacking,
        // and base-URL-injection defenses.
        for (directive, reason) in [
            ("frame-ancestors", "prevents clickjacking"),
            ("form-action", "prevents form submission to external URLs"),
            ("base-uri", "prevents base URL injection"),
        ] {
            assert_eq!(
                csp_directive(csp, directive, label),
                "'none'",
                "{label} CSP {directive} must be 'none' — {reason}"
            );
        }

        // img-src: exact allowlist enforced in BOTH prod and dev. Having the
        // same policy in both environments prevents silent drift where a dev
        // relaxation (e.g. `https://api.example.com`) never gets removed
        // before it leaks to prod. The `data:` and `*` absence checks are
        // kept as explicit assertions so failures point at the actual
        // violation rather than just a generic allowlist mismatch.
        //
        // Justification for each allowlist entry:
        //   'self'                             — bundled app assets
        //   asset: / http://asset.localhost    — Tauri built-in asset protocol
        //   blob:                              — runtime-generated image
        //                                        previews (e.g. pasted
        //                                        screenshots, drag-and-drop)
        //   https://avatars.githubusercontent.com — GitHub user avatars
        let img_tokens = csp_tokens(csp_directive(csp, "img-src", label));
        assert!(
            !img_tokens.contains("data:"),
            "{label} CSP img-src must not contain data: — blocks data-URL exfiltration"
        );
        assert!(
            !img_tokens.contains("*"),
            "{label} CSP img-src must not contain wildcards"
        );
        assert_eq!(
            img_tokens,
            BTreeSet::from([
                "'self'",
                "asset:",
                "http://asset.localhost",
                "blob:",
                "https://avatars.githubusercontent.com",
            ]),
            "{label} CSP img-src must match the exact allowlist"
        );

        // connect-src: exact allowlist per environment (no substring matching).
        // Fails loud if a new endpoint sneaks in — forcing a conscious review.
        let connect_tokens = csp_tokens(csp_directive(csp, "connect-src", label));
        assert!(
            !connect_tokens.contains("*"),
            "{label} CSP connect-src must not contain wildcards"
        );
        let expected_connect: BTreeSet<&str> = if is_prod {
            ["ipc:", "http://ipc.localhost"].into_iter().collect()
        } else {
            [
                "ipc:",
                "http://ipc.localhost",
                "ws://localhost:1420",
                "ws://localhost:1421",
                "http://localhost:1420",
            ]
            .into_iter()
            .collect()
        };
        assert_eq!(
            connect_tokens, expected_connect,
            "{label} CSP connect-src must match the exact allowlist"
        );

        // Global forbidden-token sweep — runs on BOTH production and dev CSPs.
        // This is the single biggest guard against accidental regressions (e.g.
        // someone adds 'unsafe-eval' to devCsp to fix a dev issue and that
        // relaxation then silently leaks into prod in a future copy-paste).
        // The sweep is token-based (whitespace-split), so specific hosts like
        // `http://ipc.localhost` are never confused with bare `http:` scheme
        // allowances — the two are distinct tokens. No scheme-token exception
        // is needed; don't add one back (doing so introduces a load-order
        // invariant where the exact-allowlist checks above must run first).
        assert_no_forbidden_tokens(csp, label);
    }

    /// Walks every directive in the given CSP object and asserts that none of
    /// `CSP_FORBIDDEN_TOKENS` appear. Relies on token-based equality: the set
    /// of `{ "http://ipc.localhost" }` does NOT contain the token `"http:"`,
    /// so specific-host entries pass through cleanly while bare scheme-wide
    /// allowances are rejected.
    fn assert_no_forbidden_tokens(csp: &serde_json::Value, label: &str) {
        let csp_obj = csp
            .as_object()
            .unwrap_or_else(|| panic!("{label} CSP must be an object by this point"));
        for (directive, value) in csp_obj {
            let value_str = value
                .as_str()
                .unwrap_or_else(|| panic!("{label} CSP {directive} must be a string"));
            let tokens = csp_tokens(value_str);
            for forbidden in CSP_FORBIDDEN_TOKENS {
                assert!(
                    !tokens.contains(forbidden),
                    "{label} CSP {directive} must not contain {forbidden} \
                     — this is a hard-forbidden token"
                );
            }
        }
    }

    #[test]
    fn test_csp_config_rejects_unsafe_directives() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let conf_path = std::path::Path::new(manifest_dir).join("tauri.conf.json");
        let content =
            std::fs::read_to_string(&conf_path).expect("tauri.conf.json should be readable");
        let config: serde_json::Value =
            serde_json::from_str(&content).expect("tauri.conf.json should be valid JSON");

        for (key, label) in [("csp", "production"), ("devCsp", "development")] {
            let csp = &config["app"]["security"][key];
            assert_csp_environment(csp, label, key == "csp");
        }
    }
}
