use tauri::Manager;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tracing::warn;

pub(crate) const TRAY_ID: &str = "prism_tray";
pub(crate) const MENU_SHOW: &str = "show_prism";
pub(crate) const MENU_FORCE_SYNC: &str = "force_sync";
pub(crate) const MENU_QUIT: &str = "quit";

pub(crate) const MENU_SHOW_LABEL: &str = "Show PRism";
pub(crate) const MENU_FORCE_SYNC_LABEL: &str = "Force Sync";
pub(crate) const MENU_QUIT_LABEL: &str = "Quit";

/// Format the tray tooltip based on pending review count.
pub fn format_tray_tooltip(pending_count: u32) -> String {
    match pending_count {
        0 => "PRism — No pending reviews".to_string(),
        1 => "PRism — 1 pending review".to_string(),
        n => format!("PRism — {n} pending reviews"),
    }
}

/// Build and register the system tray icon with context menu.
///
/// Creates a tray with three menu items: Show `PRism`, Force Sync, Quit.
/// Left-click toggles window visibility; right-click shows the menu.
pub fn setup_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show_item = MenuItem::with_id(app, MENU_SHOW, MENU_SHOW_LABEL, true, None::<&str>)?;
    let sync_item = MenuItem::with_id(
        app,
        MENU_FORCE_SYNC,
        MENU_FORCE_SYNC_LABEL,
        true,
        None::<&str>,
    )?;
    let quit_item = MenuItem::with_id(app, MENU_QUIT, MENU_QUIT_LABEL, true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;

    let menu = Menu::with_items(app, &[&show_item, &sync_item, &separator, &quit_item])?;

    let tooltip = format_tray_tooltip(0);

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(
            app.default_window_icon()
                .cloned()
                .ok_or("no default icon")?,
        )
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip(tooltip)
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(handle_tray_icon_event)
        .build(app)?;

    Ok(())
}

/// Update the tray tooltip to reflect the current pending review count.
pub fn update_tray_badge(app_handle: &tauri::AppHandle, pending_count: u32) -> Result<(), String> {
    let tray = app_handle
        .tray_by_id(TRAY_ID)
        .ok_or_else(|| format!("tray icon '{TRAY_ID}' not found"))?;

    let tooltip = format_tray_tooltip(pending_count);
    tray.set_tooltip(Some(&tooltip))
        .map_err(|e| format!("failed to set tooltip: {e}"))
}

#[allow(clippy::needless_pass_by_value)] // Signature imposed by Tauri on_menu_event callback
fn handle_menu_event(app: &tauri::AppHandle, event: tauri::menu::MenuEvent) {
    match event.id().as_ref() {
        MENU_SHOW => {
            if let Some(window) = app.get_webview_window("main") {
                if let Err(e) = window.show() {
                    warn!("tray: failed to show window: {e}");
                }
                if let Err(e) = window.set_focus() {
                    warn!("tray: failed to focus window: {e}");
                }
            } else {
                warn!("tray: main window not found for MENU_SHOW");
            }
        }
        MENU_FORCE_SYNC => {
            let handle = app.clone();
            tauri::async_runtime::spawn(async move {
                let pool = handle.state::<sqlx::SqlitePool>();
                let cached = handle.state::<crate::commands::GithubUsername>();
                match crate::commands::run_force_sync(&handle, &pool, &cached).await {
                    Ok(stats) => {
                        if let Err(e) = update_tray_badge(&handle, stats.pending_reviews) {
                            warn!("tray: failed to update badge after sync: {e}");
                        }
                    }
                    Err(e) => warn!("tray force sync failed: {e}"),
                }
            });
        }
        MENU_QUIT => {
            app.exit(0);
        }
        other => {
            warn!("tray: unhandled menu event: {other}");
        }
    }
}

#[allow(clippy::needless_pass_by_value)] // Signature imposed by Tauri on_tray_icon_event callback
fn handle_tray_icon_event(tray: &tauri::tray::TrayIcon, event: TrayIconEvent) {
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        let app = tray.app_handle();
        if let Some(window) = app.get_webview_window("main") {
            let visible = window.is_visible().unwrap_or(false);
            if visible {
                if let Err(e) = window.hide() {
                    warn!("tray: failed to hide window: {e}");
                }
            } else {
                if let Err(e) = window.show() {
                    warn!("tray: failed to show window: {e}");
                }
                if let Err(e) = window.set_focus() {
                    warn!("tray: failed to focus window: {e}");
                }
            }
        } else {
            warn!("tray: main window not found for tray icon click");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tray_menu_items_have_distinct_ids() {
        let ids = [MENU_SHOW, MENU_FORCE_SYNC, MENU_QUIT];
        for (i, a) in ids.iter().enumerate() {
            for (j, b) in ids.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "menu IDs must be unique");
                }
            }
        }
    }

    #[test]
    fn test_tray_menu_items_labels_are_correct() {
        assert_eq!(MENU_SHOW_LABEL, "Show PRism");
        assert_eq!(MENU_FORCE_SYNC_LABEL, "Force Sync");
        assert_eq!(MENU_QUIT_LABEL, "Quit");
    }

    #[test]
    fn test_tray_id_is_set() {
        assert_eq!(TRAY_ID, "prism_tray");
    }

    #[test]
    fn test_badge_update_zero_reviews() {
        let tooltip = format_tray_tooltip(0);
        assert_eq!(tooltip, "PRism — No pending reviews");
    }

    #[test]
    fn test_badge_update_one_review() {
        let tooltip = format_tray_tooltip(1);
        assert_eq!(tooltip, "PRism — 1 pending review");
    }

    #[test]
    fn test_badge_update_multiple_reviews() {
        assert_eq!(format_tray_tooltip(5), "PRism — 5 pending reviews");
        assert_eq!(format_tray_tooltip(42), "PRism — 42 pending reviews");
        assert_eq!(format_tray_tooltip(100), "PRism — 100 pending reviews");
    }
}
