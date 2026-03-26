mod cache;
mod commands;
mod config;
mod error;
mod github;
mod notifications;
pub mod types;
mod workspace;

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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::auth_set_token,
            commands::auth_get_status,
            commands::auth_logout,
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
