mod cache;
mod commands;
mod config;
mod error;
mod github;
mod notifications;
pub mod types;
mod workspace;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // Initialize SQLite database
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("prism.db");
            let pool = tauri::async_runtime::block_on(cache::db::init_db(&db_path))
                .map_err(|e| e.to_string())?;
            app.manage(pool);

            // Cached GitHub username — populated on first dashboard access
            app.manage(commands::GithubUsername::default());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth_set_token,
            commands::auth_get_status,
            commands::auth_logout,
            commands::github_get_dashboard,
            commands::github_get_stats,
            commands::github_force_sync,
            commands::repos_list,
            commands::repos_set_enabled,
            commands::repos_set_local_path,
            commands::config_get,
            commands::config_set,
            commands::activity_mark_read,
            commands::activity_mark_all_read,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_harness_runs_successfully() {
        assert_eq!(1 + 1, 2, "Test harness is functional");
    }
}
